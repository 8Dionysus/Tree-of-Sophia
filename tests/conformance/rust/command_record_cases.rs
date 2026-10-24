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

fn encode_fixture_hex(raw: &[u8]) -> String {
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn retained_sign_source_seed(repository: &Path, root: &Path) -> Value {
    use std::collections::BTreeSet;
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let directory = repository.join("tests/conformance/rust/fixtures/sign-source-seed-v1");
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("provenance.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["schema_version"],
        "tos_retained_sign_source_seed_v1"
    );
    assert_eq!(manifest["historical_capture"]["status"], "PASS");
    let raw = fs::read(directory.join("source.tar.gz")).unwrap();
    assert!(raw.len() <= 4 * 1024 * 1024);
    assert_eq!(
        Digest256::of_bytes(&raw).to_hex(),
        manifest["archive_sha256"]
    );
    let packet_raw = fs::read(directory.join("seed.json")).unwrap();
    assert!(packet_raw.len() <= 262144);
    assert_eq!(
        Digest256::of_bytes(&packet_raw).to_hex(),
        manifest["seed_sha256"]
    );
    let mut packet: Value = serde_json::from_slice(&packet_raw).unwrap();
    let decoder = flate2::read::GzDecoder::new(raw.as_slice());
    let mut archive = tar::Archive::new(super::BoundedReader::new(decoder, 32 * 1024 * 1024));
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut total = 0usize;
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().into_owned();
        let name = path.to_str().unwrap().trim_end_matches('/').to_owned();
        RelativePath::parse(&name).unwrap();
        let target = root.join(&name);
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            let row = manifest["directories"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["path"] == name)
                .unwrap();
            assert!(directories.insert(name));
            fs::create_dir_all(&target).unwrap();
            fs::set_permissions(
                target,
                fs::Permissions::from_mode(row["mode"].as_u64().unwrap() as u32),
            )
            .unwrap();
        } else {
            assert!(kind.is_file());
            let row = manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["path"] == name)
                .unwrap();
            let size = usize::try_from(entry.size()).unwrap();
            assert!(size <= 4 * 1024 * 1024);
            total = total.checked_add(size).unwrap();
            assert!(total <= 32 * 1024 * 1024 && files.len() < 4096);
            assert_eq!(size as u64, row["bytes"].as_u64().unwrap());
            assert!(files.insert(name));
            let mut bytes = Vec::with_capacity(size);
            entry.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes.len(), size);
            assert_eq!(Digest256::of_bytes(&bytes).to_hex(), row["sha256"]);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, bytes).unwrap();
            fs::set_permissions(
                target,
                fs::Permissions::from_mode(row["mode"].as_u64().unwrap() as u32),
            )
            .unwrap();
        }
    }
    assert_eq!(files.len(), manifest["files"].as_array().unwrap().len());
    assert_eq!(
        directories.len(),
        manifest["directories"].as_array().unwrap().len()
    );
    for name in ["assessment-owner.json", "sign-owner.json"] {
        let path = root.join(name);
        let mut config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        config["uid"] = Value::from(fs::metadata(root).unwrap().uid());
        for key in [
            "source_root",
            "journal_directory",
            "promotion_assessment_owner_config",
        ] {
            if let Some(value) = config.get_mut(key) {
                let relative = value
                    .as_str()
                    .unwrap()
                    .strip_prefix("/captured/tos-sign-fixture")
                    .unwrap()
                    .trim_start_matches('/');
                *value = Value::from(root.join(relative).to_str().unwrap());
            }
        }
        fs::write(&path, canonical_json(&config)).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut authored = serde_json::Map::new();
    let mut modes = serde_json::Map::new();
    for name in files.iter().filter(|name| name.starts_with("ToS/")) {
        assert!(tos_source_store::is_authored_source_path_v1(name));
        let path = root.join(name);
        authored.insert(
            name.clone(),
            Value::from(encode_fixture_hex(&fs::read(&path).unwrap())),
        );
        modes.insert(
            name.clone(),
            Value::from(fs::metadata(path).unwrap().mode() & 0o7777),
        );
    }
    packet["owner"] = Value::from(root.join("sign-owner.json").to_str().unwrap());
    packet["assessment_owner"] = Value::from(root.join("assessment-owner.json").to_str().unwrap());
    packet["config_raw"] = Value::from(encode_fixture_hex(
        &fs::read(root.join("sign-owner.json")).unwrap(),
    ));
    packet["head"] = Value::from(
        root.join(packet["head"].as_str().unwrap())
            .to_str()
            .unwrap(),
    );
    packet["authored"] = Value::Object(authored);
    packet["modes"] = Value::Object(modes);
    packet
}

#[test]
fn sign_uses_current_native_content_assessment_and_replays_its_original_package() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
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
    let mut oracle = retained_sign_source_seed(&repository, isolated.path());
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
        "rust/crates/tos-command/src/source_assessment_journal.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_forms_publication.rs",
        "rust/crates/tos-compiler/src/source_witness_catalog.rs",
        "rust/crates/tos-command/src/source_private_profile.rs",
        "rust/crates/tos-command/src/source_private_claim.rs",
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-command/src/source_private_owner_store.rs",
        "rust/crates/tos-command/src/source_corpus_index_projection.rs",
        "rust/crates/tos-compiler/src/source_bibliographic.rs",
        "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
        "rust/crates/tos-command/src/source_creation.rs",
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_revisions.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "rust/crates/tos-command/src/source_sign.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
    ]
    .into_iter()
    .chain(
        tos_command::source_claims::CLAIM_GROUNDING_RULE_INPUTS
            .iter()
            .copied(),
    )
    .chain(
        tos_command::source_claims::CLAIM_REVISION_RULE_INPUTS
            .iter()
            .copied(),
    ) {
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
    let owner_path = PathBuf::from(required(&oracle, "owner"));
    let owner = owner_path.as_path();
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
    let (mut describe_local, mut describe_assessment) =
        profile_workers(&cut, &image, deadline, &cancellation);
    let promotion = tos_command::source_creation::describe_sign_promotion_from_captures(
        owner,
        &context,
        &cut,
        &software,
        &components,
        &mut describe_local,
        &mut describe_assessment,
        limits,
        &cancellation,
    )
    .unwrap();
    let promotion: Value = serde_json::from_slice(&bytes(&promotion)).unwrap();
    assert_eq!(promotion["eligible"], true);
    assert_eq!(promotion["grants_issuance_authority"], false);
    oracle["preview_request"]["record"]["promotion_basis"] = promotion["basis"].clone();
    oracle["request"] = oracle["preview_request"].clone();
    context.request_raw = canonical_json(&oracle["preview_request"]);
    describe_local.finish(deadline, &cancellation).unwrap();
    // Sign's assessment owner already finalized its exact worker operation.
    // Finalizing a second time is correctly refused as a coverage mismatch.
    // Re-enter the native Sign owner with changed current v2 scope inputs.
    // These are distinct route checks: requested-use fencing, Sign risk floor,
    // and exact native-content readiness. Each refusal precedes package creation.
    let assessment_owner_path = PathBuf::from(required(&oracle, "assessment_owner"));
    let assessment_owner = assessment_owner_path.as_path();
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
    let preview_json: Value = serde_json::from_slice(&bytes(&preview)).unwrap();
    assert_eq!(
        preview_json["prepared_source"]["id"],
        oracle["preview_request"]["record"]["record_id"]
    );
    let prepared_record: Value = serde_json::from_slice(&prepared.files()["sign.json"]).unwrap();
    assert_eq!(prepared_record, oracle["preview_request"]["record"]);
    assert_eq!(preview_json["grants_admission"], false);
    assert_eq!(
        prepared.home().as_str(),
        "ToS/source-witnesses/signs/synthetic-one"
    );
    let mut outputs = serde_json::Map::new();
    for (name, raw) in prepared.files() {
        outputs.insert(name.clone(), Value::from(encode_fixture_hex(raw)));
    }
    assert!(outputs.contains_key("sign.json") && outputs.contains_key("sign.human-forms.json"));
    let forms: Value = serde_json::from_slice(&prepared.files()["sign.human-forms.json"]).unwrap();
    assert!(forms["forms"].as_array().unwrap().iter().all(|form| {
        form["bindings"]
            .as_object()
            .unwrap()
            .values()
            .any(|binding| binding["pointer"] == "/promotion_basis")
    }));
    oracle["outputs"] = Value::Object(outputs);
    oracle["request"]["operation"] = Value::from("sign.promote");
    oracle["request"]["command_id"] = Value::from("synthetic-sign-once");
    oracle["request"]["expected_configuration"] = preview_json["owner_configuration"].clone();
    oracle["request"]["expected_source"] = Value::Null;
    oracle["request"]["expected_revision"] = Value::Null;
    oracle["request"]["expected_dependencies"] = preview_json["expected_dependencies"].clone();
    local_worker.finish(deadline, &cancellation).unwrap();
    // Apply independently reconstructs the native preview's exact current
    // basis, configuration, dependency values and ordered package buffers.
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
fn assert_prepared_file_refs(preview: &JsonValue, files: &BTreeMap<String, Vec<u8>>) {
    let JsonValue::Object(refs) = preview.object_get("prepared_files").unwrap() else {
        panic!("prepared file refs object")
    };
    assert_eq!(refs.len(), files.len());
    for (name, raw) in files {
        let (_, reference) = refs
            .iter()
            .find(|(path, _)| path.as_str() == Some(name.as_str()))
            .unwrap_or_else(|| panic!("prepared file ref missing: {name}"));
        let sha256 = Digest256::of_bytes(raw).to_prefixed();
        assert_eq!(
            reference.object_get("sha256").and_then(JsonValue::as_str),
            Some(sha256.as_str()),
            "prepared file digest {name}"
        );
        assert_eq!(
            reference.object_get("bytes").and_then(JsonValue::as_u64),
            Some(raw.len() as u64),
            "prepared file size {name}"
        );
    }
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
    owner!("rust/crates/tos-command/src/source_native_cli.rs");
    owner!("rust/crates/tos-command/src/source_command.rs");
    owner!("rust/crates/tos-command/src/source_revisions.rs");
    owner!("rust/crates/tos-command/src/source_sign_native.rs");
    owner!("rust/crates/tos-compiler/src/source_bibliographic_versions.rs");
    owner!("rust/crates/tos-compiler/src/source_witness_catalog.rs");
    owner!("rust/crates/tos-command/src/source_forms.rs");
    owner!("rust/crates/tos-validation/src/assessment.rs");
    owner!("rust/crates/tos-command/src/source_private_profile.rs");
    owner!("rust/crates/tos-command/src/source_text_owner.rs");
    owner!("rust/crates/tos-command/src/source_private_owner_store.rs");
    owner!("rust/crates/tos-command/src/source_forms_publication.rs");
    owner!("rust/crates/tos-command/src/source_private_claim.rs");
    owner!("rust/crates/tos-command/src/source_creation_store.rs");
    owner!("rust/crates/tos-command/src/source_work_transaction.rs");
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
/// Exact current native metadata rule inputs; historical fixture implementation
/// names describe the capture and are never selected as executable software.
pub(super) fn native_metadata_rule_files(
    repository: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    owners: &[&str],
) {
    for path in [
        "rust/crates/tos-command/src/source_assessment_journal.rs",
        "rust/crates/tos-command/src/source_claim_publication.rs",
        "rust/crates/tos-command/src/source_command.rs",
        "rust/crates/tos-command/src/source_corpus_index_projection.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_expression_edition.rs",
        "rust/crates/tos-command/src/source_expression_responsibility.rs",
        "rust/crates/tos-command/src/source_private_claim.rs",
        "rust/crates/tos-command/src/source_private_owner_store.rs",
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-command/src/source_sign_native.rs",
        "rust/crates/tos-command/src/source_item_deposit.rs",
        "rust/crates/tos-command/src/source_item_inventory.rs",
        "rust/crates/tos-command/src/source_item_inventory_extended.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_private_assessment_sources.rs",
        "rust/crates/tos-command/src/source_private_profile.rs",
        "rust/crates/tos-command/src/source_read_owner.rs",
        "rust/crates/tos-command/src/source_revisions.rs",
        "rust/crates/tos-command/src/source_work_expression.rs",
        "rust/crates/tos-command/src/source_work_transaction.rs",
        "rust/crates/tos-compiler/src/source_bibliographic.rs",
        "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
        "rust/crates/tos-compiler/src/source_bibliographic_versions.rs",
        "rust/crates/tos-compiler/src/source_witness_catalog.rs",
        "rust/crates/tos-validation/src/biblio_rules.rs",
        "rust/crates/tos-validation/src/native_compound.rs",
    ]
    .iter()
    .copied()
    .chain(owners.iter().copied())
    .chain(
        tos_command::source_claims::CLAIM_GROUNDING_RULE_INPUTS
            .iter()
            .copied(),
    )
    .chain(
        tos_command::source_claims::CLAIM_REVISION_RULE_INPUTS
            .iter()
            .copied(),
    ) {
        files
            .entry(path.into())
            .or_insert_with(|| fs::read(repository.join(path)).unwrap());
    }
}

pub(super) fn materialize_native_fixture_software(root: &Path, files: &BTreeMap<String, Vec<u8>>) {
    // The creation fence rereads the selected implementation at the isolated
    // owner root. Frozen Python fixtures do not contain the current Rust cut.
    for (reference, raw) in files.iter().filter(|(name, _)| !name.starts_with("ToS/")) {
        RelativePath::parse(reference).unwrap();
        let target = root.join(reference);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(fs::read(target).unwrap(), *raw);
    }
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
            max_selected_object_bytes: 33_554_432,
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
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tos_command::source_creation::prepare_source_creation_from_captures;
    use tos_command::source_creation_store::{
        CreationDurability, CreationFilesystem, IsolatedCreationRoot,
        execute_isolated_creation_from_captures,
    };
    use tos_validation::source_cut::CutSchemaExecutor;

    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let cancellation = std::sync::Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(240);
    // Seven fresh protected loads; five callbacks prove ten live query leases.
    // This is a high-level fixture ledger, not child-process/schema IO telemetry.
    let mut held_list_fixture_counts = [0usize; 3];
    let mut general_fixed_header_resource_count = 0usize;

    // Observe only LIST authorization callbacks; Node inspect has a separate authority.
    // All state below is fixed stack state and borrowed text, never retained payloads.
    struct ListCandidateObserver<'a> {
        inner: super::selected_metadata_authority::Authority,
        graph: &'a str,
        subject: &'a str,
        predicate: &'a str,
        negative_subject: &'a str,
        counts: &'a std::cell::Cell<[usize; 4]>,
    }
    impl<'hold> tos_query::InspectCurrentAuthority<'hold> for ListCandidateObserver<'_> {
        fn disclosure_metadata_state_upper_bound(
            &self,
        ) -> Result<usize, tos_query::search_v2::SearchV2Error> {
            tos_query::InspectCurrentAuthority::disclosure_metadata_state_upper_bound(&self.inner)
        }
        fn abort_probe(&self) -> Option<std::sync::Arc<dyn tos_query::AbortProbe>> {
            tos_query::InspectCurrentAuthority::abort_probe(&self.inner)
        }
        fn policy_binding(&self) -> tos_query::search_v2::CurrentPolicyBinding {
            tos_query::InspectCurrentAuthority::policy_binding(&self.inner)
        }
        fn disclosure_scope(&self) -> tos_query::IndexedDisclosureScope {
            tos_query::InspectCurrentAuthority::disclosure_scope(&self.inner)
        }
        fn check_selected(&mut self) -> Result<(), tos_query::search_v2::SearchV2Error> {
            tos_query::InspectCurrentAuthority::check_selected(&mut self.inner)
        }
        fn authorize_current(
            &mut self,
            carrier: &tos_query::InspectedCarrier,
        ) -> Result<(), tos_query::search_v2::SearchV2Error> {
            tos_query::InspectCurrentAuthority::authorize_current(&mut self.inner, carrier)?;
            fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
                value
                    .as_object()?
                    .iter()
                    .find_map(|(key, value)| (key.as_str() == Some(name)).then_some(value))
            }
            let payload = &carrier.payload;
            assert_eq!(
                field(payload, "source_graph").and_then(JsonValue::as_str),
                Some(self.graph)
            );
            let mut counts = self.counts.get();
            counts[0] = counts[0]
                .checked_add(1)
                .expect("candidate counter overflow");
            if field(payload, "kind_id").and_then(JsonValue::as_str) == Some("claim") {
                counts[1] = counts[1].checked_add(1).expect("Claim counter overflow");
                let source = field(field(payload, "attributes").unwrap(), "source_claim").unwrap();
                let subject = field(source, "subject_ref")
                    .and_then(JsonValue::as_str)
                    .unwrap();
                let predicate = field(source, "predicate")
                    .and_then(JsonValue::as_str)
                    .unwrap();
                if subject == self.subject && predicate == self.predicate {
                    counts[2] = counts[2].checked_add(1).expect("matching counter overflow");
                }
                if subject == self.negative_subject {
                    counts[3] = counts[3].checked_add(1).expect("negative counter overflow");
                }
            }
            self.counts.set(counts);
            Ok(())
        }
        fn acquire_disclosure(
            &mut self,
            scope: &tos_query::IndexedDisclosureScope,
            observed: &[tos_query::ObservedInspectCarrier],
        ) -> Result<
            Box<dyn tos_query::InspectDisclosureLease + 'hold>,
            tos_query::search_v2::SearchV2Error,
        > {
            tos_query::InspectCurrentAuthority::acquire_disclosure(&mut self.inner, scope, observed)
        }
    }
    // Moving the existing Authority adds only borrowed fields and fixed counters.
    // This replaces two whole-graph DOMs; it does not enlarge the query/adapter caps.
    let observer_extra_bytes = std::mem::size_of::<ListCandidateObserver<'_>>()
        .checked_sub(std::mem::size_of::<
            super::selected_metadata_authority::Authority,
        >())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<std::cell::Cell<[usize; 4]>>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<[usize; 4]>()))
        .expect("fixed observer state overflow");
    assert!(observer_extra_bytes <= 512);

    // Actual native contract and buffer producer sources. The capture is byte
    // evidence; it is not a build proof.
    let inputs = [
        "rust/crates/tos-command/src/temporal_chronology.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_forms_publication.rs",
        "rust/crates/tos-compiler/src/source_witness_catalog.rs",
        "rust/crates/tos-command/src/source_private_profile.rs",
        "rust/crates/tos-command/src/source_private_claim.rs",
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-command/src/source_private_owner_store.rs",
        "rust/crates/tos-command/src/source_corpus_index_projection.rs",
        "rust/crates/tos-compiler/src/source_bibliographic.rs",
        "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
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
        "ToS/contracts/chronology-version-record.schema.json",
        "ToS/contracts/temporal-crosswalk-record.schema.json",
        "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    ];
    let mut files: BTreeMap<String, Vec<u8>> = inputs
        .iter()
        .map(|name| (name.to_string(), fs::read(repository.join(name)).unwrap()))
        .collect();
    native_metadata_rule_files(&repository, &mut files, &[]);
    let mut required_schemas = files
        .keys()
        .filter(|path| path.starts_with("ToS/contracts/") && path.ends_with(".schema.json"))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    // Synthetic source assertion template only. The chronology case below
    // publishes it through the real native Claim creator before freezing S.
    let (claim_inputs, template, mut claim_revision_owner, _) =
        super::command_claim_cases::claim_fixture();
    for (name, raw) in claim_inputs {
        files.entry(name).or_insert(raw);
    }
    // Explicit NEW synthetic source assertion, not a historical witness or an
    // annotation of the immutable original60. Native creation below owns v1.
    let member_path =
        "ToS/source-witnesses/relations/synthetic-frozen-day-source/source-claims.jsonl".to_owned();
    let evidence =
        "ToS/research-packets/foundation-laboratory-2026-07/SYNTHETIC_FROZEN_DAY_SOURCE.md";
    let subject = "tos.historical-event.synthetic.frozen-day-source";
    let day_object = serde_json::json!({"kind":"date-assertion","role":"historical-time",
        "calendar":"GregorianProleptic","year_numbering":"astronomical","certainty":"exact",
        "value":"1945-04-02","source_wording":{"language":"en",
            "text":"Explicit synthetic source assertion: 1945-04-02; not historical verification."},
        "extensions":{"temporal_day_projection":{"profile":"tos_source_full_day_projection_v1",
            "trs_ref":{"id":"tos.trs.synthetic.full-day-v1","version":"1"},
            "calendar_rules_version":"proleptic-gregorian-civil-v1","end_exclusive":"1945-04-03",
            "precision_assessment":{"precision":"day","posture":"synthetic-explicit-source-declaration"}}}});
    let member = serde_json::json!({"schema_version":"tos_source_temporal_claim_v1",
        "claim_id":"tos.claim.synthetic.frozen-day-source","claim_type":"relation","claim_version":1,
        "assertion_layer":"scholarly_report","subject_ref":subject,"predicate":"historical_dating",
        "object":day_object,"maker":template["maker"],"evidence_refs":[evidence],
        "provenance_event_ref":"tos.event.source-creation.synthetic-frozen-day-source",
        "epistemic_status":"reported","review_status":"unreviewed","visibility":"public_metadata_only",
        "qualifiers":{"statement":"Explicit synthetic full-day source; no historical assessment.",
            "statement_language":"en","statement_script":"Latn"}});
    files.insert(evidence.into(), b"Explicit NEW synthetic full-day source assertion for bounded owner projection; not historical verification or original60 evidence.\n".to_vec());
    let event = serde_json::json!({"schema_version":"tos_historical_record_v1",
        "record_type":"historical-event","record_id":subject,"record_version":1,
        "preferred_label":"Synthetic frozen full-day source scenario","identity_status":"provisional",
        "source_refs":[evidence],"external_identifiers":[],"same_as_posture":"no_equivalence_claim",
        "visibility":"public_metadata_only"});
    files.insert(
        "ToS/source-witnesses/history/synthetic-frozen-day-source/historical-event.json".into(),
        bytes(&parse(&canonical_json(&event))),
    );
    claim_revision_owner["schema_version"] = serde_json::json!("tos_local_claim_revision_owner_v2");
    claim_revision_owner["source_path"] = serde_json::json!(member_path);
    claim_revision_owner["claim_id"] = member["claim_id"].clone();
    claim_revision_owner["allowed_fields"] = serde_json::json!(["qualifiers"]);
    claim_revision_owner["allowed_evidence_refs"] = serde_json::json!([evidence]);
    claim_revision_owner["allowed_object_refs"] = serde_json::json!([]);
    claim_revision_owner["allowed_object_values"] = serde_json::json!([day_object]);
    claim_revision_owner["allowed_form_ids"] =
        serde_json::json!(["tos.form.synthetic-frozen-day-source.statement"]);
    claim_revision_owner["allowed_form_field_ids"] = serde_json::json!(["claim.statement"]);
    // Exact profile route plus direct owner roots, before independent input B.
    // Existing full registry routes/ref closure remains selected below.
    for root in [
        "source-temporal-claim",
        "historical-claim",
        "historical-record",
        "corpus-record",
        "source-claim-record",
        "source-structured-value",
        "claim-display-fields",
        // Direct full-producer contracts, not registry profile descendants:
        // bibliographic prepare_impl and native corpus output/schema custody.
        "source-witness-bibliographic-graph",
        "tos-corpus-index",
    ] {
        required_schemas.insert(format!("ToS/contracts/{root}.schema.json"));
    }
    // The unchanged catalog owner enumerates ALL registry profile routes, even
    // when this finite fixture has no instance of a particular profile.
    for (registry_path, rows_key, profile_key) in [
        (
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
            "types",
            "source_record_profile",
        ),
        (
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
            "relations",
            "source_claim_profile",
        ),
    ] {
        let registry: Value = serde_json::from_slice(&files[registry_path]).unwrap();
        for entry in registry[rows_key].as_array().unwrap() {
            if let Some(profile) = entry.get(profile_key) {
                for route in profile["schemas"].as_array().unwrap() {
                    required_schemas.insert(route["schema_ref"].as_str().unwrap().to_owned());
                    for dependency in route["schema_dependencies"].as_array().unwrap() {
                        required_schemas.insert(dependency.as_str().unwrap().to_owned());
                    }
                }
            }
        }
    }
    // Resolve the existing declared resource graph offline. Unrelated schemas
    // are not selected, and a missing/incompatible required schema is a failure.
    let schema_ids = files
        .iter()
        .filter(|(path, _)| path.starts_with("ToS/contracts/") && path.ends_with(".schema.json"))
        .map(|(path, raw)| {
            let schema: Value = serde_json::from_slice(raw).unwrap();
            (schema["$id"].as_str().unwrap().to_owned(), path.clone())
        })
        .collect::<BTreeMap<_, _>>();
    let mut pending = required_schemas.iter().cloned().collect::<Vec<_>>();
    let mut selected_schema_files = BTreeMap::new();
    let mut schema_bytes = 0usize;
    let mut schema_visits = 0usize;
    while let Some(path) = pending.pop() {
        if selected_schema_files.contains_key(&path) {
            continue;
        }
        assert!(selected_schema_files.len() < 128);
        let raw = files.get(&path).expect("required fixture schema absent");
        assert!(raw.len() <= 4 * 1024 * 1024);
        schema_bytes = schema_bytes.checked_add(raw.len()).unwrap();
        assert!(schema_bytes <= 16 * 1024 * 1024);
        let schema: Value = serde_json::from_slice(raw).unwrap();
        let id = schema["$id"].as_str().unwrap();
        assert!(
            id == format!("https://tree-of-sophia.local/{path}")
                || id == format!("https://treeofsophia.local/{path}"),
            "selected schema dispatcher identity: {path}"
        );
        let mut values = vec![&schema];
        while let Some(value) = values.pop() {
            schema_visits = schema_visits.checked_add(1).unwrap();
            assert!(schema_visits <= 1_000_000);
            match value {
                Value::Object(fields) => {
                    if !std::ptr::eq(value, &schema) {
                        assert!(
                            !fields.contains_key("$id"),
                            "unsupported nested schema ID in fixture closure"
                        );
                    }
                    for (key, value) in fields {
                        if key == "$ref" || key == "$dynamicRef" {
                            let reference = value.as_str().unwrap();
                            let resource = reference.split('#').next().unwrap();
                            let uri = if resource.is_empty() {
                                id.to_owned()
                            } else if resource.starts_with("https://") {
                                resource.to_owned()
                            } else {
                                assert!(!resource.contains(':') && !resource.starts_with('/'));
                                format!("{}/{}", id.rsplit_once('/').unwrap().0, resource)
                            };
                            let dependency = schema_ids
                                .get(&uri)
                                .expect("offline fixture schema reference absent");
                            if let Some((_, fragment)) = reference.split_once('#') {
                                if !fragment.is_empty() {
                                    assert!(
                                        fragment.starts_with('/'),
                                        "unsupported named schema anchor in selected fixture"
                                    );
                                    let target: Value =
                                        serde_json::from_slice(&files[dependency]).unwrap();
                                    assert!(
                                        target.pointer(fragment).is_some(),
                                        "offline schema fragment absent: {reference}"
                                    );
                                }
                            }
                            if !selected_schema_files.contains_key(dependency) {
                                pending.push(dependency.clone());
                            }
                        } else {
                            values.push(value);
                        }
                    }
                }
                Value::Array(rows) => values.extend(rows),
                _ => {}
            }
        }
        selected_schema_files.insert(path, raw.clone());
    }
    files.retain(|path, _| {
        !path.starts_with("ToS/contracts/")
            || !path.ends_with(".schema.json")
            || selected_schema_files.contains_key(path)
    });
    eprintln!(
        "initial source fixture selected schemas={} bytes={} refs offline closed; metadata dispatch cap128",
        selected_schema_files.len(),
        schema_bytes
    );

    // Full producer inputs remain in actual S, including the unchanged raw
    // GitHub NODE schema. The metadata worker derives its OWN strict closure;
    // this is not a URI rewrite, schema filter or a second smaller source cut.
    for name in [
        "ToS/contracts/tos-source-home.schema.json",
        "ToS/contracts/tos-node-contract.schema.json",
    ] {
        let raw = fs::read(repository.join(name)).unwrap();
        files.insert(name.into(), raw.clone());
        selected_schema_files.insert(name.into(), raw);
    }
    let home_path = "ToS/source_home.manifest.json";
    let home_raw = fs::read(repository.join(home_path)).unwrap();
    let home: Value = serde_json::from_slice(&home_raw).unwrap();
    for branch in home["branches"].as_array().unwrap() {
        let owner = branch["owner_surface"].as_str().unwrap();
        RelativePath::parse(owner).unwrap();
        files.insert(owner.into(), fs::read(repository.join(owner)).unwrap());
    }
    files.insert(home_path.into(), home_raw);
    for name in [
        "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json",
        "rust/crates/tos-compiler/src/lib.rs",
        "rust/crates/tos-compiler/src/source_corpus.rs",
        "rust/crates/tos-validation/src/bin/tos-schema-worker.rs",
        "access/contracts/runtime-data.v1.json",
    ] {
        files.insert(name.into(), fs::read(repository.join(name)).unwrap());
    }
    // The prior 62-schema closure plus both declared manual-authoring profile schemas.
    assert_eq!(selected_schema_files.len(), 64);
    assert!(files.len() <= 512 && files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024);

    let (captured, software, components) = captured_components(&files, deadline, &cancellation);
    let temporary = tempfile::tempdir().unwrap();
    let authored: BTreeMap<_, _> = files
        .iter()
        .filter(|(path, _)| path.starts_with("ToS/"))
        .map(|(path, raw)| (path.clone(), raw.clone()))
        .collect();
    for path in authored.keys() {
        assert!(
            tos_source_store::is_authored_source_path_v1(path),
            "initial source fixture member outside authored carrier: {path}"
        );
    }
    let store = temporary.path().join("selected-store");
    // One independent pre-S input B for actual Claim creation where required.
    // S is a standalone root, not successor(B); the main chain stays S/T/U/V.
    let creation_input = super::validation_cut_cases::write_cut_store(&authored, &store);
    drop(open_cut(&store, creation_input, deadline, &cancellation));

    let image = super::command_form_cases::schema_image(
        tos_validation::executor::ExecutorBudget::laboratory(),
        deadline,
        &cancellation,
    );

    let inline_policy = |kind: &str, profile: &str| {
        let definition = serde_json::json!({"schema_version":"tos_chronology_source_policy_v1",
            "policy_id":format!("tos.source-policy.synthetic-{}",kind.to_ascii_lowercase()),"policy_version":1,
            "kind":kind,"profile":profile,"rules":{"membership":"PresentInPinnedCut","history":"ExactRevisionOnly","ordering":"DeclaredSequence"},
            "basis":"Explicit synthetic source selection definition; no access grant"});
        let reference = serde_json::json!({"id":definition["policy_id"],"version":1,
            "digest":Digest256::of_bytes(&bytes(&parse(&canonical_json(&definition)))).to_prefixed()});
        serde_json::json!({"definition":definition,"reference":reference,"scope":"InlineSourceDefinitionNotAuthorization"})
    };
    let snapshot_name = "tos.snapshot.synthetic-frozen-member-cut";
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
        (
            "tos_local_profile_create_owner_v1",
            "chronology-version",
            "ToS/source-witnesses/chronologies/new-subject/chronology-version.json",
        ),
        (
            "tos_local_profile_create_owner_v1",
            "temporal-crosswalk",
            "ToS/source-witnesses/temporal-crosswalks/new-subject/temporal-crosswalk.json",
        ),
    ] {
        let mut files = files.clone();
        let mut revision = creation_input;
        let mut member = member.clone();
        let mut member_raw = Vec::new();
        let mut member_ref = Value::Null;
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
        for (path, bytes) in &files {
            let target = isolated.path().join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, bytes).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        }
        if kind == "chronology-version" {
            use super::command_text_cases::{
                alignment_image_digest, alignment_native_cli, authored_text_files,
            };
            let uid = fs::metadata(isolated.path()).unwrap().uid();
            let source_home = Path::new(&member_path).parent().unwrap();
            assert!(!isolated.path().join(source_home).exists());
            fs::create_dir_all(isolated.path().join(source_home).parent().unwrap()).unwrap();
            let owner = isolated.path().join("frozen-member-create-owner.json");
            let configuration = serde_json::json!({"schema_version":"tos_local_claim_create_owner_v2",
                "uid":uid,"principal_id":member["maker"]["agent_ref"],"maker_type":member["maker"]["maker_type"],
                "source_root":isolated.path(),"source_path":member_path,
                "authority_ref":"synthetic-test-only:bounded-native-claim-creation",
                "expires_at":"2099-01-01T00:00:00Z","provenance_event_id":member["provenance_event_ref"],
                "allowed_operations":["claims.create"],"allowed_claim_ids":[member["claim_id"]],
                "allowed_subject_refs":[member["subject_ref"]],"allowed_object_refs":[],"allowed_object_values":[member["object"]],
                "allowed_predicates":[member["predicate"]],"allowed_evidence_refs":member["evidence_refs"]});
            fs::write(&owner, canonical_json(&configuration)).unwrap();
            fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
            let native = PathBuf::from(
                std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
            );
            let schema_worker = PathBuf::from(
                std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("schema worker required"),
            );
            let invocation_path = isolated.path().join("frozen-member-create-invocation.json");
            let invocation = serde_json::json!({"schema_version":"tos_local_native_claim_invocation_v1",
                "owner_config":owner,"native_executable":native,"native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
                "corpus_store":store,"source_revision":creation_input.0.to_prefixed(),"original_source_revision":creation_input.0.to_prefixed(),
                "software_capture":captured.capture,"software_restored_root":captured.restored,
                "software_selection":{"source_git_commit":captured.selection.source_git_commit,"source_git_tree":captured.selection.source_git_tree,
                    "capture_manifest_sha256":captured.selection.capture_manifest_sha256.to_prefixed()},
                "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
                "schema_worker":{"absolute_path":schema_worker,"sha256":alignment_image_digest(&schema_worker).to_prefixed()},
                "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
                    "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
            fs::write(&invocation_path, canonical_json(&invocation)).unwrap();
            fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
            let mut create = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","claims":[member]});
            eprintln!("temporal profile={kind} phase=actual-member-prepare-create input=B");
            let prepared =
                alignment_native_cli(&repository, &owner, &invocation_path, &create, deadline);
            assert_eq!(prepared["grants_admission"], false);
            create["operation"] = serde_json::json!("claims.create");
            create["command_id"] =
                serde_json::json!(format!("synthetic:{kind}-native-member-baseline"));
            create["expected_configuration"] = prepared["result"]["owner_configuration"].clone();
            create["expected_revision"] = Value::Null;
            create["expected_dependencies"] = prepared["result"]["expected_dependencies"].clone();
            create["expected_inputs"] = prepared["result"]["source_bindings"].clone();
            eprintln!("temporal profile={kind} phase=actual-member-claims.create input=B");
            let created =
                alignment_native_cli(&repository, &owner, &invocation_path, &create, deadline);
            assert_eq!(created["result"]["replayed"], false);
            assert_eq!(created["result"]["receipt"]["grants_admission"], false);
            let published = fs::read_dir(isolated.path().join(source_home))
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    assert!(entry.file_type().unwrap().is_file());
                    (
                        entry.file_name().into_string().unwrap(),
                        fs::read(entry.path()).unwrap(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                published
                    .keys()
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>(),
                std::collections::BTreeSet::from([
                    "source-claims.jsonl",
                    "source-create-request.json",
                    "source-create-environment.json",
                    "source-create-provenance.jsonl",
                    "source-create-receipt.json"
                ])
            );
            let physical_receipt: Value =
                serde_json::from_slice(&published["source-create-receipt.json"]).unwrap();
            assert_eq!(physical_receipt, created["result"]["receipt"]);
            let physical_request: Value =
                serde_json::from_slice(&published["source-create-request.json"]).unwrap();
            assert_eq!(physical_request, create);
            member_raw = published["source-claims.jsonl"].clone();
            member = serde_json::from_slice(&member_raw).unwrap();
            assert_eq!(member["claim_version"], 1);
            member_ref = created["result"]["receipt"]["claims"][0].clone();
            // Exact-ref object order is not part of SourceCommandInputV1
            // identity. Compare its declared canonical representation.
            assert_eq!(
                bytes(&parse(&canonical_json(&member_ref))),
                bytes(&tos_command::source_forms::metadata_subject(&parse(&member_raw)).unwrap())
            );
            let current = authored_text_files(isolated.path());
            for (name, raw) in &published {
                assert_eq!(
                    current.get(&source_home.join(name).to_str().unwrap().to_owned()),
                    Some(raw)
                );
            }
            // B remains independently selectable evidence of actual creation;
            // the actual generated package is frozen in S with no base link.
            revision = super::validation_cut_cases::write_cut_store(&current, &store);
            let s_manifest: Value = serde_json::from_slice(
                &fs::read(
                    store
                        .join("revisions")
                        .join(revision.0.to_hex())
                        .join("snapshot.json"),
                )
                .unwrap(),
            )
            .unwrap();
            assert!(s_manifest["base_revision"].is_null());
            assert_ne!(revision, creation_input);
            files.retain(|name, _| !name.starts_with("ToS/"));
            files.extend(current);
            eprintln!(
                "temporal profile={kind} actual-native-member pre-S B={} standalone S={}; main chain bound4 (B is not an ancestor)",
                creation_input.0.to_prefixed(),
                revision.0.to_prefixed()
            );
        }
        let cut = open_cut(&store, revision, deadline, &cancellation);
        // Independent actual software inventory of the COMPLETE frozen S,
        // including the native-created five-file Claim package. This capture
        // describes S; it does not create, relabel or replace that source cut.
        let model_at_s = if kind == "chronology-version" {
            let git_fixture = tempfile::tempdir().unwrap();
            let git_root = git_fixture.path();
            let git = |args: &[&str]| {
                assert!(
                    Instant::now() < deadline
                        && !cancellation.load(std::sync::atomic::Ordering::SeqCst)
                );
                let mut command = std::process::Command::new("git");
                command
                    .arg("-c")
                    .arg("core.hooksPath=/dev/null")
                    .arg("-C")
                    .arg(git_root)
                    .args(args);
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("GIT_") {
                        command.env_remove(key);
                    }
                }
                let output = command.output().unwrap();
                assert!(
                    output.status.success(),
                    "bounded S inventory Git operation failed"
                );
                assert!(
                    Instant::now() < deadline
                        && !cancellation.load(std::sync::atomic::Ordering::SeqCst)
                );
                output.stdout
            };
            git(&["init", "-q"]);
            let mut actual_s = BTreeMap::new();
            let mut stream = cut.stream(revision).unwrap();
            let membership = stream.expectation();
            while let Some(entry) = stream.next_member(deadline, &cancellation).unwrap() {
                assert_eq!(entry.revision, revision);
                assert!(
                    actual_s
                        .insert(entry.path.as_str().to_owned(), entry.raw)
                        .is_none()
                );
            }
            assert_eq!(stream.coverage(), Some(membership));
            // Narrow actual recipe admission before the extra Git/capture
            // copies. Product reader/writer limits and whole-case caps stay put.
            assert!(actual_s.values().map(Vec::len).sum::<usize>() <= 2 * 1024 * 1024);
            assert_eq!(actual_s.get(&member_path), Some(&member_raw));
            let mut inventory = files
                .iter()
                .filter(|(path, _)| !path.starts_with("ToS/"))
                .map(|(p, b)| (p.clone(), b.clone()))
                .collect::<BTreeMap<_, _>>();
            inventory.extend(actual_s);
            assert!(
                inventory.len() <= 512
                    && inventory.values().map(Vec::len).sum::<usize>() <= 8 * 1024 * 1024
            );
            for (name, raw) in &inventory {
                let target = git_root.join(name);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(&target, raw).unwrap();
                fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
            }
            // Force is Git's explicit exact-fixture membership, NOT a resource
            // or admission force. Retained JSONL package files must be tracked.
            git(&["add", "-f", "--", "."]);
            git(&[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "Exact native-created S inventory",
            ]);
            let commit = String::from_utf8(git(&["rev-parse", "HEAD^{commit}"])).unwrap();
            // The convenience wrapper starts its own 120s clock. Reuse its
            // exact existing owner operations with this SAME family deadline.
            let capture_home = tempfile::tempdir().unwrap();
            let capture_path = capture_home.path().join("software-capture");
            let restored_path = capture_home.path().join("software-restored");
            let selection = super::source_cut_cases::capture_software_archive(
                git_root,
                commit.trim(),
                &["ToS", "rust", "access"],
                &capture_path,
                deadline,
                &cancellation,
            );
            super::source_cut_cases::restore_software_archive(
                &capture_path,
                &restored_path,
                &selection,
                deadline,
                &cancellation,
            );
            let capture = super::source_cut_cases::SoftwareCaptureFixture {
                temporary: capture_home,
                capture: capture_path,
                restored: restored_path,
                selection,
            };
            let inventory_reader = tos_source_store::SoftwareCaptureReader::open(
                &capture.capture,
                &capture.restored,
                capture.selection.clone(),
                tos_source_store::ReadLimits {
                    max_manifest_bytes: 2 * 1024 * 1024,
                    max_manifest_entries: 512,
                    max_selected_object_bytes: 16 * 1024 * 1024,
                    json: JsonLimits::default(),
                },
                deadline,
                &cancellation,
            )
            .unwrap();
            let model = super::compiler_source_cases::native_selected_model_at_existing_cut(
                &cut,
                revision,
                &inventory_reader,
                temporary.path().join("frozen-S-model/knowledge.sqlite3"),
                deadline,
                &cancellation,
            );
            assert_eq!(
                model
                    .open()
                    .unwrap()
                    .corpus_original_receipt()
                    .unwrap()
                    .origin
                    .native_producer
                    .as_ref()
                    .unwrap()
                    .source_revision,
                revision.0.to_hex()
            );
            Some((capture, inventory_reader, model))
        } else {
            None
        };
        // G12 SOURCE binding only, ORIGINAL chronology branch. Reserve before any
        // owned extraction/typed clone/serialization; never refund this subset.
        let g12_debit = if kind == "chronology-version" {
            (4_194_304usize, 32_768usize)
        } else {
            (0usize, 0usize)
        };
        let g12 = if kind == "chronology-version" {
            use tos_foundation::query_context::{
                ContextResolution, Cutoff, QueryContext, SourceSnapshot, SourceSnapshotRef,
                SystemState, resolve_query_context,
            };
            use tos_foundation::{
                StableId, canonical_bytes_v1_with_state_budget_and_visits,
                parse_json_with_state_budget,
            };
            // Borrow immutable bytes: no full-oracle DOM or hidden raw Vec.
            let raw_oracle =
                include_bytes!("../foundation-wave-v1/oracles/temporal-acceptance-vectors.json");
            assert!(raw_oracle.len() <= 262_144);
            assert_eq!(
                Digest256::of_bytes(raw_oracle).to_hex(),
                "fda08fd958914a3605afab12e7b78012a3f5534ecfdffe2ab31cb2cb3a26eecd"
            );
            let source42: &[u8] = br#"{"context":{},"input":{"claim_revisions":["claim-A@r1"],"evidence_policy_version":"evidence-v1","selection_policy_version":"selection-v1","selection_request":{"attributed_knowledge_as_of":null,"source_publication_as_of":null,"system_state":"Current"},"source_snapshot_ref":{"resolved_system_cutoff":"2026-10-19T05:00:00Z","revision_fence":"source-rev-42","snapshot_id":"S1"}},"operation":"create_chronology_version","source_profile":"ToS-Temporal-Core-Contract-v1"}"#;
            let source58: &[u8] = br#"{"context":{"captured_at":"2026-10-19T05:00:00Z","immutable":true,"snapshot":"S1"},"input":{"create":{"claim_revisions":["claim-A@r1"],"evidence_policy_version":"evidence-v1","selection_policy_version":"selection-v1","selection_request":{"attributed_knowledge_as_of":null,"source_publication_as_of":null,"system_state":"Current"},"source_snapshot_ref":{"resolved_system_cutoff":"2026-10-19T05:00:00Z","revision_fence":"source-rev-42","snapshot_id":"S1"}},"later_query_at":"2026-10-20T05:00:00Z","query":{"source_snapshot_ref":{"revision_fence":"source-rev-42","snapshot_id":"S1"},"temporal":{"chronology_version":"CV-created","world_valid_time":{"position":"1945","trs_ref":"TRS:GregorianProleptic:fixture-v1"}}}},"operation":"create_chronology_version_then_list_claims","source_profile":"ToS-Temporal-Core-Contract-v1"}"#;
            // R independently checks these exact source excerpts; no expected/id
            // fields enter resolution or the native owner request.
            let limits = JsonLimits {
                max_bytes: 2048,
                max_depth: 16,
                max_visits: 512,
                max_integer_digits: 4096,
            };
            assert_eq!(
                (
                    16_777_216usize.checked_sub(g12_debit.0).unwrap(),
                    1_000_000usize.checked_sub(g12_debit.1).unwrap()
                ),
                (12_582_912, 967_232)
            );
            let mut visits = g12_debit.1;
            // Fixed reservation covers raw512KiB (unused when borrowed), excerpts,
            // typed/JSON overlap512KiB and static128KiB; old DOM2MiB is unspent.
            assert!(
                524_288usize
                    .checked_add(2_097_152)
                    .unwrap()
                    .checked_add(262_144)
                    .unwrap()
                    .checked_add(524_288)
                    .unwrap()
                    .checked_add(131_072)
                    .unwrap()
                    <= g12_debit.0
            );
            let mut roots = Vec::with_capacity(2);
            assert!(roots.capacity() * std::mem::size_of::<JsonValue>() <= 65_536);
            for (raw, length, digest) in [
                (
                    source42,
                    465usize,
                    "64d7dae4b197b2ddab7c1113817d039645b2480b917fe72b145a3dfc42d91f26",
                ),
                (
                    source58,
                    820usize,
                    "7efaa7fd87defefb5acf4f75442aa7594a89bf6fabfc2c2688ac3ad66936ab06",
                ),
            ] {
                assert_eq!(raw.len(), length);
                assert_eq!(Digest256::of_bytes(raw).to_hex(), digest);
                let document =
                    parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, 65_536)
                        .unwrap();
                visits = visits.checked_sub(document.visits()).unwrap();
                let root = document.into_root();
                let fields = root.as_object().unwrap();
                assert_eq!(fields.len(), 4);
                for key in ["operation", "source_profile", "input", "context"] {
                    assert!(root.object_get(key).is_some());
                }
                let (emitted, used) = canonical_bytes_v1_with_state_budget_and_visits(
                    &root,
                    CanonicalProfile::SourceCommandInputV1,
                    limits,
                    65_536,
                )
                .unwrap();
                visits = visits.checked_sub(used).unwrap();
                assert!(emitted.len() <= 2048 && emitted.capacity() <= 65_536);
                roots.push(root);
            }
            let original42 = roots.remove(0);
            let original58 = roots.remove(0);
            assert_eq!(
                original42.object_get("operation").unwrap().as_str(),
                Some("create_chronology_version")
            );
            assert_eq!(
                original58.object_get("operation").unwrap().as_str(),
                Some("create_chronology_version_then_list_claims")
            );
            for source in [&original42, &original58] {
                assert_eq!(
                    source.object_get("source_profile").unwrap().as_str(),
                    Some("ToS-Temporal-Core-Contract-v1")
                );
            }
            assert!(
                original42
                    .object_get("context")
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .is_empty()
            );
            let input = original42.object_get("input").unwrap();
            let later = original58.object_get("input").unwrap();
            assert_eq!(later.object_get("create").unwrap(), input);
            let origin = original58.object_get("context").unwrap();
            assert_eq!(origin.as_object().unwrap().len(), 3);
            assert_eq!(origin.object_get("snapshot").unwrap().as_str(), Some("S1"));
            assert_eq!(
                origin.object_get("immutable").unwrap().as_bool(),
                Some(true)
            );
            let source_snapshot = input.object_get("source_snapshot_ref").unwrap();
            assert_eq!(source_snapshot.as_object().unwrap().len(), 3);
            assert_eq!(
                source_snapshot.object_get("snapshot_id").unwrap().as_str(),
                Some("S1")
            );
            assert_eq!(
                source_snapshot
                    .object_get("revision_fence")
                    .unwrap()
                    .as_str(),
                Some("source-rev-42")
            );
            assert!(source_snapshot.object_get("system_trs_ref").is_none());
            let t1 = source_snapshot
                .object_get("resolved_system_cutoff")
                .unwrap();
            let t2 = later.object_get("later_query_at").unwrap();
            assert_eq!(t1.as_str(), Some("2026-10-19T05:00:00Z"));
            assert_eq!(t2.as_str(), Some("2026-10-20T05:00:00Z"));
            assert_eq!(origin.object_get("captured_at").unwrap(), t1);
            assert_eq!(t1.as_str().unwrap().len(), 20);
            assert_eq!(t2.as_str().unwrap().len(), 20);
            let selection = input.object_get("selection_request").unwrap();
            assert_eq!(selection.as_object().unwrap().len(), 3);
            assert_eq!(
                selection.object_get("system_state").unwrap().as_str(),
                Some("Current")
            );
            assert!(
                selection
                    .object_get("attributed_knowledge_as_of")
                    .unwrap()
                    .is_null()
            );
            assert!(
                selection
                    .object_get("source_publication_as_of")
                    .unwrap()
                    .is_null()
            );
            assert_eq!(
                input
                    .object_get("claim_revisions")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                input
                    .object_get("claim_revisions")
                    .unwrap()
                    .as_array()
                    .unwrap()[0]
                    .as_str(),
                Some("claim-A@r1")
            );
            // Actual IDs/ref/cut are explicit SOURCE supplements, never selected
            // by name spelling or inferred from these symbolic source tokens.
            assert!(snapshot_name.len() <= 1024);
            assert_eq!(member_ref.as_object().unwrap().len(), 3);
            assert!(member_ref["id"].as_str().unwrap().len() <= 1024);
            assert_eq!(member_ref["digest"].as_str().unwrap().len(), 71);
            assert_eq!(member_ref["version"].as_u64(), Some(1));
            Digest256::from_prefixed(member_ref["digest"].as_str().unwrap()).unwrap();
            // Prepay finite typed clones, policy construction and serialization
            // visits BEFORE resolve (its API itself has no budget parameter).
            visits = visits.checked_sub(8192).unwrap();
            let creation_snapshot = SourceSnapshot {
                reference: SourceSnapshotRef {
                    id: StableId::parse(snapshot_name).unwrap(),
                    revision,
                },
                current_cutoff: Cutoff {
                    value: t1.clone(),
                    trs: None,
                },
            };
            let creation_request = QueryContext {
                system_state: Some(SystemState::Current),
                attributed_knowledge: Some(None),
                source_publication_as_of: Some(None),
                world_valid_time: None,
                display_perspective: None,
            };
            assert!(
                std::mem::size_of_val(&creation_snapshot)
                    + std::mem::size_of_val(&creation_request)
                    + 16 * (snapshot_name.len() + 40)
                    + 16_384
                    <= 524_288
            );
            // Exactly ONE additional pure resolver; not a world operation.
            let ContextResolution::Resolved(resolved) =
                resolve_query_context(&creation_snapshot, &creation_request, None).unwrap()
            else {
                panic!("explicit supplied Current must resolve");
            };
            assert_eq!(
                resolved.selection.source_snapshot,
                creation_snapshot.reference
            );
            assert_eq!(resolved.selection.system_as_of.value, *t1);
            assert!(resolved.selection.system_as_of.trs.is_none());
            assert!(resolved.selection.attributed_knowledge.is_none());
            assert!(resolved.selection.source_publication_as_of.is_none());
            assert!(resolved.chronology_version.is_none());
            let mut policies = Vec::with_capacity(2);
            assert!(policies.capacity() * std::mem::size_of::<Value>() <= 65_536);
            for (token, key, kind, profile) in [
                (
                    "evidence-v1",
                    "evidence_policy_version",
                    "Evidence",
                    "ExactPinnedMemberEvidence-v1",
                ),
                (
                    "selection-v1",
                    "selection_policy_version",
                    "Selection",
                    "ExplicitExactMembers-v1",
                ),
            ] {
                assert_eq!(input.object_get(key).unwrap().as_str(), Some(token));
                let policy = inline_policy(kind, profile);
                let expected_definition = serde_json::json!({
                    "schema_version":"tos_chronology_source_policy_v1",
                    "policy_id":format!("tos.source-policy.synthetic-{}",kind.to_ascii_lowercase()),
                    "policy_version":1,"kind":kind,"profile":profile,
                    "rules":{"membership":"PresentInPinnedCut","history":"ExactRevisionOnly","ordering":"DeclaredSequence"},
                    "basis":"Explicit synthetic source selection definition; no access grant"});
                assert_eq!(policy["definition"], expected_definition);
                assert_eq!(policy["scope"], "InlineSourceDefinitionNotAuthorization");
                let raw = canonical_json(&policy);
                assert!(raw.len() <= 2048 && raw.capacity() <= 65_536);
                let doc =
                    parse_json_with_state_budget(&raw, JsonMode::PublishedStrict, limits, 65_536)
                        .unwrap();
                visits = visits.checked_sub(doc.visits()).unwrap();
                let definition = doc.root().object_get("definition").unwrap();
                let (canonical, used) = canonical_bytes_v1_with_state_budget_and_visits(
                    definition,
                    CanonicalProfile::SourceCommandInputV1,
                    limits,
                    65_536,
                )
                .unwrap();
                visits = visits.checked_sub(used).unwrap();
                assert!(canonical.len() <= 2048 && canonical.capacity() <= 65_536);
                assert_eq!(
                    policy["reference"],
                    serde_json::json!({
                    "id":policy["definition"]["policy_id"],"version":1,
                    "digest":Digest256::of_bytes(&canonical).to_prefixed()})
                );
                policies.push(policy);
            }
            let cutoff_raw = tos_foundation::emit_value_preserved_json(
                &resolved.selection.system_as_of.value,
                limits,
            )
            .unwrap();
            assert!(cutoff_raw.len() <= 64 && cutoff_raw.capacity() <= 65_536);
            let frozen = serde_json::json!({"schema_version":"tos_frozen_chronology_v1",
                "source_snapshot":{"id":resolved.selection.source_snapshot.id.as_str(),
                    "revision":resolved.selection.source_snapshot.revision.0.to_prefixed()},
                "system_as_of":{"value":serde_json::from_slice::<Value>(&cutoff_raw).unwrap(),"trs":null},
                "attributed_knowledge":null,"source_publication_as_of":null,
                "selection_mode":"ExplicitExactMembers","members":[member_ref],
                "policies":{"evidence":policies[0],"selection":policies[1]}});
            let frozen_raw = canonical_json(&frozen);
            assert!(frozen_raw.len() <= 8192 && frozen_raw.capacity() <= 65_536);
            let frozen_limits = JsonLimits {
                max_bytes: 8192,
                ..limits
            };
            let checked = parse_json_with_state_budget(
                &frozen_raw,
                JsonMode::PublishedStrict,
                frozen_limits,
                65_536,
            )
            .unwrap();
            visits = visits.checked_sub(checked.visits()).unwrap();
            let (_, used) = canonical_bytes_v1_with_state_budget_and_visits(
                checked.root(),
                CanonicalProfile::SourceCommandInputV1,
                frozen_limits,
                65_536,
            )
            .unwrap();
            visits = visits.checked_sub(used).unwrap();
            assert!(visits < g12_debit.1);
            eprintln!(
                "G12 ORIGINAL Current freeze: pure_resolvers=1 world_delta=0 debit={}/{} remaining_visits={} noTRS source42=465 source58=820",
                g12_debit.0, g12_debit.1, visits
            );
            Some((frozen, t1.clone(), t2.clone(), original42, original58))
        } else {
            None
        };
        let frozen = if let Some((frozen, ..)) = &g12 {
            frozen.clone()
        } else {
            serde_json::json!({"schema_version":"tos_frozen_chronology_v1",
        "source_snapshot":{"id":snapshot_name,"revision":revision.0.to_prefixed()},
        "system_as_of":{"value":1,"trs":null},"attributed_knowledge":null,"source_publication_as_of":null,
        "selection_mode":"ExplicitExactMembers","members":[member_ref],"policies":{
            "evidence":inline_policy("Evidence","ExactPinnedMemberEvidence-v1"),
            "selection":inline_policy("Selection","ExplicitExactMembers-v1")}})
        };
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
        if kind == "chronology-version" {
            record["schema_version"] = serde_json::json!("tos_chronology_version_record_v1");
            record["semantic_content"] = frozen.clone();
        }
        if kind == "temporal-crosswalk" {
            // Exact accepted T2 payload; fixture metadata identity remains the
            // explicitly granted provisional create identity above.
            let example: Value = serde_json::from_str(
                r#"{
  "schema_version": "tos_temporal_crosswalk_record_v1",
  "record_type": "temporal-crosswalk",
  "record_id": "tos.temporal-crosswalk.synthetic-a",
  "preferred_label": "Synthetic finite coordinate mapping",
  "identity_status": "provisional",
  "source_refs": [
    "synthetic:crosswalk-fixture-v1"
  ],
  "external_identifiers": [],
  "same_as_posture": "not_assessed",
  "record_version": 1,
  "visibility": "public_metadata_only",
  "semantic_content": {
    "profile": "tos_stored_finite_crosswalk_v1",
    "source_trs": {
      "id": "synthetic-axis-a",
      "version": "r1"
    },
    "target_trs": {
      "id": "synthetic-axis-b",
      "version": "r2"
    },
    "rule_profile": "finite-mapping-v1",
    "rules_version": "synthetic-rule-r1",
    "applicability": {
      "min": 10,
      "max": 12,
      "unit": "step",
      "origin": "explicit-zero"
    },
    "rows": {
      "10": 100,
      "12": 120
    },
    "predecessor_source_record_ref": null,
    "provenance": {
      "fixture_kind": "synthetic",
      "evidence_ref": "synthetic:mapping-source-r1"
    }
  }
}
"#,
            )
            .unwrap();
            record["schema_version"] = example["schema_version"].clone();
            record["semantic_content"] = example["semantic_content"].clone();
            // Explicit synthetic source evidence, outside mapping rules/bounds.
            // The owner preserves this Int; JavaScript receives only opaque text.
            record["semantic_content"]["provenance"]["synthetic_opaque_integer"] =
                serde_json::json!(9007199254740993u64);
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
                config["profile_type_id"] = serde_json::json!(format!("tos.entity.{kind}"));
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
        assert_prepared_file_refs(&preview, prepared.files());
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
        // Authored source is reselected from the isolated owner root; software
        // components are reselected from the exact restored capture bound above.
        for path in [
            "ToS/contracts/human-form.schema.json",
            "rust/crates/tos-command/src/source_serialization.rs",
        ] {
            let target = if path.starts_with("ToS/") {
                isolated.path().join(path)
            } else {
                captured.restored.join(path)
            };
            let original = fs::read(&target).unwrap();
            let mut substituted = original.clone();
            substituted.push(b'\n');
            fs::write(&target, substituted).unwrap();
            let result = filesystem.publish_isolated(
                &serialized,
                &cut,
                &software,
                &components,
                deadline,
                &cancellation,
            );
            let actual = match &result {
                Ok(_) => "Ok(_)".to_owned(),
                Err(error) => format!("Err({error:?})"),
            };
            let expected = if path.starts_with("ToS/") {
                matches!(&result, Err(SourceCommandError::Conflict(_)))
            } else {
                matches!(
                    &result,
                    Err(SourceCommandError::Unsupported(
                        "software command input custody read incomplete"
                    ))
                )
            };
            fs::write(&target, original).unwrap();
            assert!(expected, "{kind} unchanged dependency {path}: got {actual}");
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
        let executed_preview = executed.prepared().preview().unwrap();
        assert_prepared_file_refs(&executed_preview, executed.prepared().files());
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
        if matches!(kind, "chronology-version" | "temporal-crosswalk") {
            // New source profiles use the existing real writer/CAS and cold
            // native read. Baseline Claim v1 above and its v2 below are issued
            // by actual existing Claim creation/revision owners.
            use super::command_text_cases::{
                alignment_image_digest, alignment_native_cli, authored_text_files,
            };
            use tos_command::temporal_chronology::{
                ChronologyLimits, bind_chronology_record_from_cut, load_chronology_from_owner,
            };
            use tos_foundation::query_context::{Cutoff, SourceSnapshot, SourceSnapshotRef};
            use tos_foundation::{ExactRecordRef, RecordVersion, StableId};
            let exact_ref = |v: &Value| ExactRecordRef {
                id: StableId::parse(v["id"].as_str().unwrap()).unwrap(),
                version: RecordVersion::new(v["version"].as_u64().unwrap()).unwrap(),
                digest: Digest256::from_prefixed(v["digest"].as_str().unwrap()).unwrap(),
            };
            let current_authored = || {
                let mut data = authored_text_files(isolated.path());
                data.remove("ToS/source-witnesses/.historical-create.writer.lock");
                data
            };
            let with_software = |data: &BTreeMap<String, Vec<u8>>| {
                let mut selected = files
                    .iter()
                    .filter(|(name, _)| !name.starts_with("ToS/"))
                    .map(|(name, raw)| (name.clone(), raw.clone()))
                    .collect::<BTreeMap<_, _>>();
                selected.extend(data.clone());
                selected
            };
            let v1_raw = fs::read(isolated.path().join(relative)).unwrap();
            let v1_record: Value = serde_json::from_slice(&v1_raw).unwrap();
            let v1_ref: Value = serde_json::from_slice(&bytes(
                &tos_command::source_forms::metadata_subject(&parse(&v1_raw)).unwrap(),
            ))
            .unwrap();
            assert_eq!(v1_record, request["record"]);
            let after_create =
                super::command_form_cases::successor(&current_authored(), &store, revision);
            let native = PathBuf::from(
                std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
            );
            let worker_path = PathBuf::from(
                std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("schema worker required"),
            );
            let revision_path = isolated.path().join("temporal-revision-owner.json");
            let revision_config = serde_json::json!({"schema_version":"tos_local_profile_revision_owner_v1",
                "uid":uid,"principal_id":"software:test-fixture","source_root":isolated.path(),"source_path":relative,
                "authority_ref":"synthetic-test-only:metadata-revision-not-admission","allowed_form_ids":["tos.form.creation.fixture-name"],
                "allowed_operations":["record.revise"],"expires_at":"2099-01-01T00:00:00Z",
                "record_id":identity,"allowed_fields":["semantic_content"],"profile_type_id":format!("tos.entity.{kind}")});
            let revision_config_raw = canonical_json(&revision_config);
            fs::write(&revision_path, &revision_config_raw).unwrap();
            fs::set_permissions(&revision_path, fs::Permissions::from_mode(0o600)).unwrap();
            let revision_fs = CreationFilesystem::select_isolated(
                &isolated,
                &revision_path,
                deadline,
                &cancellation,
            )
            .unwrap();
            let invocation_path = temporary
                .path()
                .join(format!("{kind}-revision-invocation.json"));
            let mut invocation = serde_json::json!({"schema_version":"tos_local_native_source_invocation_v1",
                "owner_config":revision_path,"owner_context":null,"assessment_schema_worker":null,
                "native_executable":native,"native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
                "corpus_store":store,"source_revision":after_create.0.to_prefixed(),"original_source_revision":after_create.0.to_prefixed(),
                "software_capture":captured.capture,"software_restored_root":captured.restored,
                "software_selection":{"source_git_commit":captured.selection.source_git_commit,"source_git_tree":captured.selection.source_git_tree,
                    "capture_manifest_sha256":captured.selection.capture_manifest_sha256.to_prefixed()},
                "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
                "schema_worker":{"absolute_path":worker_path,"sha256":alignment_image_digest(&worker_path).to_prefixed()},
                "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
                    "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
            let write_invocation = |v: &Value| {
                fs::write(&invocation_path, canonical_json(v)).unwrap();
                fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
            };
            let mut current_revision = after_create;
            if kind == "chronology-version" {
                let mut claim_owner = claim_revision_owner.clone();
                claim_owner["uid"] = serde_json::json!(uid);
                claim_owner["source_root"] = serde_json::json!(isolated.path());
                let claim_owner_path = isolated.path().join("frozen-member-revision-owner.json");
                fs::write(&claim_owner_path, canonical_json(&claim_owner)).unwrap();
                fs::set_permissions(&claim_owner_path, fs::Permissions::from_mode(0o600)).unwrap();
                let mut claim_invocation = invocation.clone();
                claim_invocation["schema_version"] =
                    serde_json::json!("tos_local_native_claim_invocation_v1");
                claim_invocation["owner_config"] = serde_json::json!(claim_owner_path);
                claim_invocation["original_source_revision"] =
                    serde_json::json!(creation_input.0.to_prefixed());
                claim_invocation
                    .as_object_mut()
                    .unwrap()
                    .remove("owner_context");
                claim_invocation
                    .as_object_mut()
                    .unwrap()
                    .remove("assessment_schema_worker");
                write_invocation(&claim_invocation);
                let mut correction = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-revise",
                    "fields":{"qualifiers":{"statement":"Actual synthetic v2 correction after freezing the v1 packet"}},
                    "forms":[{"form_id":claim_owner["allowed_form_ids"][0],"field_id":"claim.statement"}],
                    "reason":"Synthetic fixture actual Claim revision; retain original frozen member"});
                eprintln!(
                    "temporal profile={kind} phase=actual-member-prepare-revise original=B current=T"
                );
                let prepared_claim = alignment_native_cli(
                    &repository,
                    &claim_owner_path,
                    &invocation_path,
                    &correction,
                    deadline,
                );
                correction["operation"] = serde_json::json!("claim.revise");
                correction["command_id"] = serde_json::json!("synthetic:freeze-member-v2");
                for (key, value) in [
                    ("expected_configuration", "owner_configuration"),
                    ("expected_source", "source"),
                    ("expected_revision", "revision"),
                    ("expected_dependencies", "expected_dependencies"),
                    ("expected_inputs", "source_bindings"),
                ] {
                    correction[key] = prepared_claim["result"][value].clone();
                }
                let claim_written = alignment_native_cli(
                    &repository,
                    &claim_owner_path,
                    &invocation_path,
                    &correction,
                    deadline,
                );
                assert_eq!(claim_written["result"]["source"]["version"], 2);
                let actual_current_raw = fs::read(isolated.path().join(&member_path)).unwrap();
                let actual_current: Value = serde_json::from_slice(&actual_current_raw).unwrap();
                assert_eq!(
                    actual_current["qualifiers"]["statement"],
                    correction["fields"]["qualifiers"]["statement"]
                );
                // v2 corrects qualifiers only; the declared full-day source is unchanged.
                assert_eq!(
                    bytes(&parse(&canonical_json(&actual_current["object"]))),
                    bytes(&parse(&canonical_json(&member["object"])))
                );
                assert_ne!(actual_current, member);
                // Real native renderer + physical fixture publication; no
                // invented token/manifest or production publication claim.
                super::compiler_source_cases::publish_native_catalog_fixture_with_selected_schemas(
                    &repository,
                    isolated.path(),
                    deadline,
                    &selected_schema_files,
                );
                current_revision =
                    super::command_form_cases::successor(&current_authored(), &store, after_create);
                let current_cut = open_cut(&store, current_revision, deadline, &cancellation);
                let current_selected = with_software(&current_authored());
                let describe = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"});
                let mut current_context = cut_context(
                    &current_selected,
                    revision_config_raw.clone(),
                    canonical_json(&describe),
                    current_revision,
                );
                current_context.effective_uid = u64::from(uid);
                let snapshot = SourceSnapshot {
                    reference: SourceSnapshotRef {
                        id: StableId::parse(snapshot_name).unwrap(),
                        revision,
                    },
                    current_cutoff: Cutoff {
                        value: g12.as_ref().unwrap().2.clone(),
                        trs: None,
                    },
                };
                let mut load_worker = super::command_form_cases::schemas_with_image(
                    &current_cut,
                    &image,
                    deadline,
                    &cancellation,
                );
                let loaded = load_chronology_from_owner(
                    &revision_fs,
                    &current_context,
                    &current_cut,
                    &exact_ref(&v1_ref),
                    &context,
                    &cut,
                    &snapshot,
                    &software,
                    &components,
                    &mut load_worker,
                    ChronologyLimits::default(),
                    deadline,
                    &cancellation,
                )
                .unwrap();
                loaded.recheck(deadline, &cancellation).unwrap();
                assert_eq!(
                    loaded.bound().chronology().selection.system_as_of.value,
                    g12.as_ref().unwrap().1
                );
                assert!(
                    loaded
                        .bound()
                        .chronology()
                        .selection
                        .system_as_of
                        .trs
                        .is_none()
                );
                assert_eq!(
                    loaded.bound().chronology().evidence_policy,
                    exact_ref(&frozen["policies"]["evidence"]["reference"])
                );
                assert_eq!(
                    loaded.bound().chronology().selection_policy,
                    exact_ref(&frozen["policies"]["selection"]["reference"])
                );
                assert_eq!(
                    loaded.bound().chronology().member_claim_revisions,
                    vec![exact_ref(&member_ref)]
                );
                let packet = loaded
                    .bound()
                    .evidence()
                    .object_get("members")
                    .unwrap()
                    .as_array()
                    .unwrap();
                assert_eq!(packet.len(), 1);
                let frozen_source = packet[0].object_get("frozen_source").unwrap();
                assert_eq!(
                    frozen_source
                        .object_get("source_revision")
                        .and_then(JsonValue::as_str),
                    Some(revision.0.to_prefixed().as_str())
                );
                assert_eq!(
                    frozen_source
                        .object_get("source_ref")
                        .and_then(JsonValue::as_str),
                    Some(member_path.as_str())
                );
                assert_eq!(
                    frozen_source
                        .object_get("stream_sha256")
                        .and_then(JsonValue::as_str),
                    Some(Digest256::of_bytes(&member_raw).to_prefixed().as_str())
                );
                assert_eq!(
                    frozen_source.object_get("line").and_then(JsonValue::as_u64),
                    Some(1)
                );
                assert_eq!(
                    bytes(packet[0].object_get("record").unwrap()),
                    bytes(&parse(&canonical_json(&member)))
                );
                assert_ne!(
                    bytes(packet[0].object_get("record").unwrap()),
                    bytes(&parse(&canonical_json(&actual_current)))
                );
                assert_eq!(
                    loaded.owner_evidence().object_get("source_persisted"),
                    Some(&JsonValue::Bool(true))
                );
                let original_member = cut
                    .read_member(
                        revision,
                        &RelativePath::parse(&member_path).unwrap(),
                        8_388_608,
                        deadline,
                        &cancellation,
                    )
                    .unwrap();
                assert_eq!(original_member.raw, member_raw);
                for invalid_case in ["duplicate", "policy-digest", "Current", "wrong-fence"] {
                    let mut bad = v1_record.clone();
                    match invalid_case {
                        "duplicate" => bad["semantic_content"]["members"]
                            .as_array_mut()
                            .unwrap()
                            .push(member_ref.clone()),
                        "policy-digest" => {
                            bad["semantic_content"]["policies"]["evidence"]["reference"]["digest"] =
                                serde_json::json!(format!("sha256:{}", "0".repeat(64)))
                        }
                        "Current" => {
                            bad["semantic_content"]["system_as_of"]["value"] =
                                serde_json::json!("Current")
                        }
                        _ => {
                            bad["semantic_content"]["source_snapshot"]["revision"] =
                                serde_json::json!(current_revision.0.to_prefixed())
                        }
                    }
                    assert!(
                        bind_chronology_record_from_cut(
                            &loaded,
                            &parse(&canonical_json(&bad)),
                            &context,
                            &cut,
                            &snapshot,
                            &software,
                            &components,
                            ChronologyLimits::default(),
                            deadline,
                            &cancellation
                        )
                        .is_err()
                    );
                }
                // Held derived capture must detect real physical changes,
                // independently of frozen source custody and owner configuration.
                let live_catalog = isolated
                    .path()
                    .join("ToS/source-witnesses/catalog/claims.jsonl");
                let live_catalog_raw = fs::read(&live_catalog).unwrap();
                fs::write(&live_catalog, b"{}\n").unwrap();
                assert!(loaded.recheck(deadline, &cancellation).is_err());
                fs::write(&live_catalog, &live_catalog_raw).unwrap();
                loaded.recheck(deadline, &cancellation).unwrap();
                let mut revoked_read = revision_config.clone();
                revoked_read["allowed_operations"] = serde_json::json!([]);
                fs::write(&revision_path, canonical_json(&revoked_read)).unwrap();
                assert!(loaded.recheck(deadline, &cancellation).is_err());
                fs::write(&revision_path, &revision_config_raw).unwrap();
                loaded.recheck(deadline, &cancellation).unwrap();
                // Consume the SAME real retained owner hold after all existing
                // packet/history/revocation assertions. No model or PG is used.
                use tos_command::temporal_range_projection::{
                    OwnerDayRangeBudget, with_loaded_chronology_day_range,
                };
                let supplement = parse(&canonical_json(&serde_json::json!({
                    "profile":"tos_owner_day_range_projection_v1","postgres_type":"daterange",
                    "bound_form":"[)","calendar_rules":{"calendar":"GregorianProleptic",
                        "version":"proleptic-gregorian-civil-v1"}})));
                with_loaded_chronology_day_range(
                    loaded,
                    &exact_ref(&member_ref),
                    &supplement,
                    OwnerDayRangeBudget {
                        temporal: tos_foundation::temporal::TemporalBudget {
                            max_pairings: 128,
                            ..tos_foundation::temporal::TemporalBudget::default()
                        },
                        max_chronology_members: 1,
                        max_extra_owned_bytes: 4 * 1024 * 1024,
                        max_extra_json_visits: 65_536,
                    },
                    deadline,
                    &cancellation,
                    |view| {
                        view.recheck()?;
                        view.with_current_bytes(|raw| {
                            let public: Value = serde_json::from_slice(raw).unwrap();
                            assert_eq!(
                                public["schema_version"],
                                "tos_owner_day_range_projection_result_v1"
                            );
                            assert_eq!(public["sql_executed"], false);
                            assert_eq!(public["event_extent_inferred"], false);
                            assert_eq!(
                                public["evidence_projection"],
                                "AuthorizedSourceAndStreamFixityOnly"
                            );
                            let projected = &public["projection"];
                            assert_eq!(projected["status"], "Projected");
                            assert_eq!(projected["postgres_value"], "[1945-04-02,1945-04-03)");
                            let sidecar = &projected["sidecar"];
                            // Full old source semantics, not an invented date-only sidecar.
                            assert_eq!(
                                bytes(&parse(&canonical_json(&sidecar["raw_claim"]))),
                                bytes(&parse(&canonical_json(&member)))
                            );
                            assert_ne!(
                                bytes(&parse(&canonical_json(&sidecar["raw_claim"]))),
                                bytes(&parse(&canonical_json(&actual_current)))
                            );
                            let provenance = &sidecar["provenance"];
                            let mut keys = provenance
                                .as_object()
                                .unwrap()
                                .keys()
                                .map(String::as_str)
                                .collect::<Vec<_>>();
                            keys.sort_unstable();
                            assert_eq!(
                                keys,
                                [
                                    "chronology_ref",
                                    "frozen_source",
                                    "source_claim_ref",
                                    "version_status"
                                ]
                            );
                            let frozen = &provenance["frozen_source"];
                            assert_eq!(frozen["source_ref"], member_path);
                            assert_eq!(frozen["source_revision"], revision.0.to_prefixed());
                            assert_eq!(
                                frozen["stream_sha256"],
                                Digest256::of_bytes(&member_raw).to_prefixed()
                            );
                            assert_eq!(frozen["stream_bytes"], member_raw.len() as u64);
                            assert_eq!(frozen["line"], 1);
                            assert!(
                                !std::str::from_utf8(raw)
                                    .unwrap()
                                    .contains(isolated.path().to_str().unwrap())
                            );
                            // Owner/private evidence is retained by the hold, never emitted.
                            for forbidden in [
                                "owner_evidence",
                                "owner_configuration",
                                "configuration_raw",
                                "creation_receipt",
                                "private_journal",
                            ] {
                                assert!(
                                    !std::str::from_utf8(raw)
                                        .unwrap()
                                        .contains(&format!("\"{forbidden}\""))
                                );
                            }
                            Ok(())
                        })?;
                        // Current authority still gates the derived view inside
                        // its callback; restoration does not mint a new grant.
                        fs::write(&revision_path, canonical_json(&revoked_read)).unwrap();
                        assert!(view.recheck().is_err());
                        fs::write(&revision_path, &revision_config_raw).unwrap();
                        view.recheck()?;
                        Ok(())
                    },
                )
                .unwrap();
                // Fresh genuine loader handles, sequentially AFTER consumption
                // of the positive hold (the existing corpus fence is exclusive).
                // These are supplement negatives, not altered source evidence.
                for missing in [true, false] {
                    let mut invalid = serde_json::json!({
                        "profile":"tos_owner_day_range_projection_v1","postgres_type":"daterange",
                        "bound_form":"[)","calendar_rules":{"calendar":"GregorianProleptic",
                            "version":"proleptic-gregorian-civil-v1"}});
                    if missing {
                        invalid.as_object_mut().unwrap().remove("calendar_rules");
                    } else {
                        invalid["calendar_rules"]["version"] =
                            serde_json::json!("unsupported-synthetic-rules");
                    }
                    let reread = load_chronology_from_owner(
                        &revision_fs,
                        &current_context,
                        &current_cut,
                        &exact_ref(&v1_ref),
                        &context,
                        &cut,
                        &snapshot,
                        &software,
                        &components,
                        &mut load_worker,
                        ChronologyLimits::default(),
                        deadline,
                        &cancellation,
                    )
                    .unwrap();
                    let rejected: Result<(), _> = with_loaded_chronology_day_range(
                        reread,
                        &exact_ref(&member_ref),
                        &parse(&canonical_json(&invalid)),
                        OwnerDayRangeBudget {
                            temporal: tos_foundation::temporal::TemporalBudget {
                                max_pairings: 128,
                                ..tos_foundation::temporal::TemporalBudget::default()
                            },
                            max_chronology_members: 1,
                            max_extra_owned_bytes: 4 * 1024 * 1024,
                            max_extra_json_visits: 65_536,
                        },
                        deadline,
                        &cancellation,
                        |_| panic!("invalid supplement must not emit a public owner projection"),
                    );
                    assert!(rejected.is_err());
                    if !missing {
                        assert!(matches!(rejected, Err(tos_command::temporal_range_projection::OwnerDayRangeError::Unsupported("UnsupportedProjectionSupplement"))));
                    }
                }
                // Year-only/missing SOURCE extraction is deliberately not
                // claimed here: no extra source/cut/writer invocation is made.
                // The same persisted chronology is also composed with the
                // actual full native model built at S. Scope issuance here is
                // EXPLICITLY synthetic fixture policy, not an installed owner
                // project_claims capability or production authorization.
                use std::sync::{Arc, atomic::Ordering};
                use tos_query::search_v2::{
                    CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode,
                };
                use tos_query::{
                    IndexedDisclosureScope, InspectCurrentAuthority, InspectDisclosureLease,
                    InspectedCarrier, ObservedInspectCarrier, bind_verified_knowledge,
                };
                type QueryResult<T> = std::result::Result<T, SearchV2Error>;
                fn withdrawn() -> SearchV2Error {
                    SearchV2Error {
                        code: SearchV2ErrorCode::StalePolicy,
                        message: "synthetic exact S fixture read scope withdrawn",
                    }
                }
                struct Probe {
                    deadline: Instant,
                    cancelled: Arc<AtomicBool>,
                }
                impl tos_query::AbortProbe for Probe {
                    fn reason(&self) -> Option<tos_query::AbortReason> {
                        if self.cancelled.load(Ordering::SeqCst) {
                            Some(tos_query::AbortReason::Cancelled)
                        } else if Instant::now() >= self.deadline {
                            Some(tos_query::AbortReason::DeadlineExceeded)
                        } else {
                            None
                        }
                    }
                }
                struct Lease(Arc<AtomicBool>);
                impl InspectDisclosureLease for Lease {
                    fn recheck(&mut self) -> QueryResult<()> {
                        if self.0.load(Ordering::SeqCst) {
                            Err(withdrawn())
                        } else {
                            Ok(())
                        }
                    }
                }
                struct Authority {
                    policy: CurrentPolicyBinding,
                    scope: IndexedDisclosureScope,
                    graph: String,
                    old_source: Vec<u8>,
                    authorized: BTreeMap<String, Digest256>,
                    // Only a finalized, already authorized Claim may bind this
                    // related object. No guessed literal ID or caller member list.
                    object_binding: Option<(String, [Vec<u8>; 3])>,
                    withdrawn: Arc<AtomicBool>,
                    probe: Arc<Probe>,
                }
                impl<'hold> InspectCurrentAuthority<'hold> for Authority {
                    fn disclosure_metadata_state_upper_bound(&self) -> QueryResult<usize> {
                        Ok(65_536)
                    }
                    fn abort_probe(&self) -> Option<Arc<dyn tos_query::AbortProbe>> {
                        Some(self.probe.clone())
                    }
                    fn policy_binding(&self) -> CurrentPolicyBinding {
                        self.policy.clone()
                    }
                    fn disclosure_scope(&self) -> IndexedDisclosureScope {
                        self.scope.clone()
                    }
                    fn check_selected(&mut self) -> QueryResult<()> {
                        if self.withdrawn.load(Ordering::SeqCst) {
                            Err(withdrawn())
                        } else {
                            Ok(())
                        }
                    }
                    fn authorize_current(&mut self, carrier: &InspectedCarrier) -> QueryResult<()> {
                        self.check_selected()?;
                        assert!(carrier.id.len() <= 1024 && self.authorized.len() < 16);
                        let payload = &carrier.payload;
                        assert_eq!(
                            payload
                                .object_get("source_graph")
                                .and_then(JsonValue::as_str),
                            Some(self.graph.as_str())
                        );
                        let attrs = payload.object_get("attributes").unwrap();
                        let raw = payload
                            .object_get("source_record")
                            .unwrap()
                            .object_get("payload")
                            .unwrap();
                        let lineage = || {
                            ["source_ref", "source_line", "source_sha256"].map(|key| {
                                let value = raw.object_get(key).unwrap();
                                // Pre-admit tiny lineage fields before retaining copies.
                                match key {
                                    "source_ref" => assert!(value.as_str().unwrap().len() <= 1024),
                                    "source_sha256" => {
                                        Digest256::from_hex(value.as_str().unwrap()).unwrap();
                                    }
                                    _ => assert!(value.as_u64().is_some_and(|n| n > 0)),
                                }
                                bytes(value)
                            })
                        };
                        let kind = payload.object_get("kind_id").and_then(JsonValue::as_str);
                        let mapped = payload
                            .object_get("type_mapping")
                            .unwrap()
                            .object_get("status")
                            .and_then(JsonValue::as_str);
                        assert_eq!(mapped, Some("mapped"));
                        assert_eq!(
                            payload
                                .object_get("type_mapping")
                                .unwrap()
                                .object_get("source_kind_id")
                                .and_then(JsonValue::as_str),
                            kind
                        );
                        Digest256::from_hex(
                            payload
                                .object_get("content_revision")
                                .and_then(JsonValue::as_str)
                                .unwrap(),
                        )
                        .unwrap();
                        if kind == Some("claim") {
                            assert_eq!(
                                payload.object_get("type_id").and_then(JsonValue::as_str),
                                Some("tos.entity.claim")
                            );
                            let source = attrs.object_get("source_claim").unwrap();
                            // Full ACTUAL native-created old packet remains the grant.
                            assert_eq!(bytes(source), self.old_source);
                            assert_eq!(
                                source.object_get("visibility").and_then(JsonValue::as_str),
                                Some("public_metadata_only")
                            );
                            let semantics = payload
                                .object_get("semantics")
                                .unwrap()
                                .object_get("claim")
                                .unwrap();
                            for (source_key, semantic_key) in [
                                ("claim_id", "claim_id"),
                                ("claim_version", "claim_version"),
                                ("predicate", "source_predicate_id"),
                            ] {
                                assert_eq!(
                                    bytes(source.object_get(source_key).unwrap()),
                                    bytes(semantics.object_get(semantic_key).unwrap())
                                );
                            }
                            assert_eq!(
                                semantics
                                    .object_get("predicate_mapping_status")
                                    .and_then(JsonValue::as_str),
                                Some("mapped")
                            );
                            let id = semantics
                                .object_get("object_node_id")
                                .and_then(JsonValue::as_str)
                                .unwrap();
                            assert!(!id.is_empty() && id.len() <= 1024);
                            let binding = (id.to_owned(), lineage());
                            if let Some(previous) = &self.object_binding {
                                assert_eq!(previous, &binding);
                            } else {
                                self.object_binding = Some(binding);
                            }
                        } else {
                            assert_eq!(
                                self.scope.operation_id,
                                "tos.knowledge.temporal.project_claims"
                            );
                            assert_eq!(
                                self.scope.intended_use,
                                "read_only_public_knowledge_claim_projection_v1"
                            );
                            let (id, expected_lineage) = self
                                .object_binding
                                .as_ref()
                                .expect("related object requires prior authenticated Claim");
                            assert_eq!(&carrier.id, id);
                            assert_eq!(kind, Some("temporal-assertion"));
                            assert_eq!(
                                payload.object_get("type_id").and_then(JsonValue::as_str),
                                Some("tos.entity.temporal-assertion")
                            );
                            assert_eq!(&lineage(), expected_lineage);
                            let source = parse(&self.old_source);
                            assert_eq!(
                                bytes(attrs.object_get("claim_ref").unwrap()),
                                bytes(source.object_get("claim_id").unwrap())
                            );
                            let value = attrs.object_get("value").unwrap();
                            assert_eq!(bytes(value), bytes(source.object_get("object").unwrap()));
                            let value_digest = Digest256::of_bytes(
                                &canonical_bytes_v1(
                                    value,
                                    CanonicalProfile::SourceRecordDigestV1,
                                    JsonLimits::default(),
                                )
                                .unwrap(),
                            );
                            assert_eq!(
                                Digest256::from_hex(
                                    attrs
                                        .object_get("value_sha256")
                                        .and_then(JsonValue::as_str)
                                        .unwrap()
                                )
                                .unwrap(),
                                value_digest
                            );
                            let time = payload
                                .object_get("semantics")
                                .unwrap()
                                .object_get("time")
                                .unwrap();
                            assert_eq!(bytes(time.object_get("raw").unwrap()), bytes(value));
                        }
                        self.authorized
                            .insert(carrier.id.clone(), carrier.payload_sha256);
                        Ok(())
                    }
                    fn acquire_disclosure(
                        &mut self,
                        scope: &IndexedDisclosureScope,
                        consulted: &[ObservedInspectCarrier],
                    ) -> QueryResult<Box<dyn InspectDisclosureLease + 'hold>> {
                        self.check_selected()?;
                        assert_eq!(scope, &self.scope);
                        assert!(!consulted.is_empty() && consulted.len() <= 16);
                        // This one-member historical_dating recipe has no
                        // documentary secondary lookups or relation disclosures.
                        let expected = if scope.operation_id == "tos.knowledge.node.inspect" {
                            1
                        } else {
                            2
                        };
                        assert_eq!(consulted.len(), expected);
                        assert_eq!(self.authorized.len(), expected);
                        for row in consulted {
                            assert_eq!(row.source_graph, self.graph);
                            assert_eq!(self.authorized.get(&row.id), Some(&row.payload_sha256));
                        }
                        Ok(Box::new(Lease(self.withdrawn.clone())))
                    }
                }
                let (inventory_capture, inventory_reader, selected) = model_at_s.as_ref().unwrap();
                let recheck_inventory = || {
                    let reopened = tos_source_store::SoftwareCaptureReader::open(
                        &inventory_capture.capture,
                        &inventory_capture.restored,
                        inventory_capture.selection.clone(),
                        tos_source_store::ReadLimits {
                            max_manifest_bytes: 2 * 1024 * 1024,
                            max_manifest_entries: 512,
                            max_selected_object_bytes: 16 * 1024 * 1024,
                            json: JsonLimits::default(),
                        },
                        deadline,
                        &cancellation,
                    )
                    .unwrap();
                    assert_eq!(reopened.selection(), inventory_reader.selection());
                };
                recheck_inventory();
                let mut cold = selected.open().unwrap();
                let bound = bind_verified_knowledge(
                    &cold,
                    &selected.vocabulary,
                    &selected.descriptor_bytes,
                )
                .unwrap();
                assert_eq!(
                    bound.require_source_revision().unwrap(),
                    revision.0.to_hex()
                );
                let graph = bound
                    .source_for_adapter("reified-bibliographic-claims-v1")
                    .unwrap();
                let revoked_query = Arc::new(AtomicBool::new(false));
                let authority = |operation: &str, intended: &str| {
                    assert!(matches!(
                        (operation, intended),
                        (
                            "tos.knowledge.node.inspect",
                            "read_only_public_knowledge_inspect_v1"
                        ) | (
                            "tos.knowledge.temporal.project_claims",
                            "read_only_public_knowledge_claim_projection_v1"
                        )
                    ));
                    let policy = CurrentPolicyBinding {
                        scope: "synthetic-persisted-S-only".into(),
                        issuer_ref: "synthetic-fixture".into(),
                        authorization_receipt_id: "synthetic-fixture-exact-created-packet".into(),
                        policy_epoch: "synthetic-epoch".into(),
                        withdrawal_generation: "synthetic-generation".into(),
                    };
                    Authority {
                        scope: IndexedDisclosureScope {
                            operation_id: operation.into(),
                            intended_use: intended.into(),
                            carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
                            selected_model_receipt_id: bound.owner_receipt_id().into(),
                            source_cut: bound.selection().source_cut.clone(),
                            through_commit_seq: bound.selection().through_commit_seq,
                            source_membership_root: bound.selection().source_membership_root,
                            descriptor_sha256: bound.selection().vocabulary.descriptor_sha256,
                            selected_index_sha256: bound.selection().index_root_sha256,
                            policy_issuer_ref: policy.issuer_ref.clone(),
                            policy_receipt_id: policy.authorization_receipt_id.clone(),
                            policy_scope: policy.scope.clone(),
                            policy_epoch: policy.policy_epoch.clone(),
                            withdrawal_generation: policy.withdrawal_generation.clone(),
                        },
                        policy,
                        graph: graph.into(),
                        old_source: bytes(&parse(&canonical_json(&member))),
                        authorized: BTreeMap::new(),
                        object_binding: None,
                        withdrawn: revoked_query.clone(),
                        probe: Arc::new(Probe {
                            deadline,
                            cancelled: cancellation.clone(),
                        }),
                    }
                };
                let mut lookup_authority = authority(
                    "tos.knowledge.node.inspect",
                    "read_only_public_knowledge_inspect_v1",
                );
                let mut projection_authority = authority(
                    "tos.knowledge.temporal.project_claims",
                    "read_only_public_knowledge_claim_projection_v1",
                );
                let display = parse(&canonical_json(&serde_json::json!({
                    "mode":"SourcePreserving","profile_ref":{"id":"tos.display-profile.synthetic.source-preserving",
                        "version":1,"digest":Digest256::of_bytes(b"Explicit synthetic display request only; no policy admission").to_prefixed()}})));
                let owner_reread = load_chronology_from_owner(
                    &revision_fs,
                    &current_context,
                    &current_cut,
                    &exact_ref(&v1_ref),
                    &context,
                    &cut,
                    &snapshot,
                    &software,
                    &components,
                    &mut load_worker,
                    ChronologyLimits::default(),
                    deadline,
                    &cancellation,
                )
                .unwrap();
                let mut held_pair_locator = None;
                tos_command::temporal_chronology_projection::with_loaded_chronology_projection(
                    owner_reread,
                    &mut cold,
                    &bound,
                    &mut lookup_authority,
                    &mut projection_authority,
                    &display,
                    tos_command::temporal_chronology_projection::StoredChronologyProjectionBudget {
                        query: tos_query::InspectBudget {
                            max_open_vm_steps: selected.cold_limits().max_vm_steps,
                            max_read_vm_steps: 20_000_000,
                            max_matches: 16,
                            max_rows: 1000,
                            max_field_bytes: 16_384,
                            max_payload_bytes: 1_048_576,
                            max_decoded_bytes: 8 * 1024 * 1024,
                            max_response_bytes: 2 * 1024 * 1024,
                            json: JsonLimits::default(),
                        },
                        max_members: 1,
                        max_extra_owned_bytes: 16 * 1024 * 1024,
                        max_extra_json_visits: 1_000_000,
                    },
                    deadline,
                    &cancellation,
                    |view| {
                        view.recheck()?;
                        view.with_current_bytes(|raw| {
                            let output: Value = serde_json::from_slice(raw).unwrap();
                            assert_eq!(
                                output["schema_version"],
                                "tos_stored_chronology_projection_result_v1"
                            );
                            assert_eq!(output["source_revision"], revision.0.to_hex());
                            assert_eq!(output["members"].as_array().unwrap().len(), 1);
                            assert_eq!(
                                bytes(&parse(&canonical_json(&output["members"][0]["reference"]))),
                                bytes(&parse(&canonical_json(&member_ref)))
                            );
                            let rows = output["projection"]["rows"].as_array().unwrap();
                            assert_eq!(rows.len(), 1);
                            let old = &rows[0]["operand"]["claim"]["attributes"]["source_claim"];
                            assert_eq!(
                                bytes(&parse(&canonical_json(old))),
                                bytes(&parse(&canonical_json(&member)))
                            );
                            assert_ne!(
                                bytes(&parse(&canonical_json(old))),
                                bytes(&parse(&canonical_json(&actual_current)))
                            );
                            assert!(rows[0]["derived_coordinate"].is_null());
                            assert_eq!(rows[0]["derived_coordinate_status"], "Unresolved");
                            // Capture only the ACTUAL authorized selected Claim's
                            // locator. Never invent an ID or relabel another model S.
                            let claim = &rows[0]["operand"]["claim"];
                            let id = claim["id"].as_str().unwrap();
                            assert!(!id.is_empty() && id.len() <= 1024);
                            let content = claim["content_revision"].as_str().unwrap();
                            Digest256::from_hex(content).unwrap();
                            held_pair_locator = Some(parse(&canonical_json(&serde_json::json!({
                                "node_id":id,"content_revision":content
                            }))));
                            Ok(())
                        })?;
                        revoked_query.store(true, Ordering::SeqCst);
                        assert!(view.recheck().is_err());
                        revoked_query.store(false, Ordering::SeqCst);
                        view.recheck()?;
                        Ok(())
                    },
                )
                .unwrap();
                // One exact Claim inspect; projection consults that Claim and
                // its finalized object, not arbitrary related graph carriers.
                assert_eq!(lookup_authority.authorized.len(), 1);
                assert_eq!(projection_authority.authorized.len(), 2);
                let (object_id, _) = projection_authority.object_binding.as_ref().unwrap();
                assert!(projection_authority.authorized.contains_key(object_id));
                recheck_inventory();

                // A DISTINCT existing Access TEST Temporal owner, not the old
                // Node/project factory. Prior projection custody has ended.
                use super::selected_metadata_authority::{
                    Authority as TemporalAuthority, Controls,
                };
                use std::sync::atomic::AtomicUsize;
                use tos_command::temporal_claim_relation_binding::{
                    HeldClaimComparisonBudget, HeldClaimComparisonError,
                    with_loaded_chronology_envelope_comparison,
                };
                let locator = held_pair_locator.unwrap();
                let temporal_request = parse(&canonical_json(&serde_json::json!({
                    "schema_version":"tos_temporal_comparison_request_v1",
                    "source_revision":revision.0.to_hex(),
                    "left":serde_json::from_slice::<Value>(&bytes(&locator)).unwrap(),
                    "right":serde_json::from_slice::<Value>(&bytes(&locator)).unwrap()
                })));
                let temporal_controls = Arc::new(Controls::default());
                let temporal_held = Arc::new(AtomicUsize::new(0));
                let mut temporal_authority = TemporalAuthority::new(
                    &bound,
                    &tos_access::KnowledgeRequest::Temporal(temporal_request),
                    temporal_held.clone(),
                    temporal_controls.clone(),
                );
                let temporal_scope =
                    tos_query::InspectCurrentAuthority::disclosure_scope(&temporal_authority);
                assert_eq!(
                    temporal_scope.operation_id,
                    "tos.knowledge.temporal.compare"
                );
                assert_eq!(
                    temporal_scope.intended_use,
                    tos_query::TEMPORAL_INTENDED_USE
                );
                assert_eq!(temporal_held.load(Ordering::SeqCst), 0);
                // Exactly ONE sequential protected owner reread, under the
                // original schema worker/receipt/deadline bounds. No new cut.
                let pair_owner = load_chronology_from_owner(
                    &revision_fs,
                    &current_context,
                    &current_cut,
                    &exact_ref(&v1_ref),
                    &context,
                    &cut,
                    &snapshot,
                    &software,
                    &components,
                    &mut load_worker,
                    ChronologyLimits::default(),
                    deadline,
                    &cancellation,
                )
                .unwrap();
                let pair_member = exact_ref(&member_ref);
                with_loaded_chronology_envelope_comparison(
                    pair_owner,
                    &pair_member,
                    &pair_member,
                    &locator,
                    &locator,
                    &mut cold,
                    &bound,
                    &mut temporal_authority,
                    HeldClaimComparisonBudget {
                        query: tos_query::InspectBudget {
                            max_open_vm_steps: selected.cold_limits().max_vm_steps,
                            max_read_vm_steps: 20_000_000,
                            max_matches: 16,
                            max_rows: 1000,
                            max_field_bytes: 16_384,
                            max_payload_bytes: 1_048_576,
                            max_decoded_bytes: 8 * 1024 * 1024,
                            max_response_bytes: 2 * 1024 * 1024,
                            json: JsonLimits::default(),
                        },
                        max_extra_owned_bytes: 16 * 1024 * 1024,
                        max_extra_json_visits: 1_000_000,
                        max_verification_owned_bytes: 4 * 1024 * 1024,
                        max_verification_json_visits: 250_000,
                    },
                    deadline,
                    &cancellation,
                    |view| {
                        assert!(temporal_held.load(Ordering::SeqCst) > 0);
                        view.with_current_bytes(|raw| {
                            assert!(raw.len() <= 2 * 1024 * 1024);
                            let output: Value = serde_json::from_slice(raw).unwrap();
                            assert_eq!(output.as_object().unwrap().len(), 11);
                            assert_eq!(
                                output["schema_version"],
                                "tos_stored_claim_envelope_comparison_v1"
                            );
                            assert_eq!(
                                output["scope"],
                                "OwnerMemberVerifiedSourceEnvelopeComparison"
                            );
                            assert_eq!(output["a3_assessment"], "NotEstablished");
                            assert_eq!(
                                output["policy_scope"],
                                "SourceSelectionProvenanceNotAuthorization"
                            );
                            assert_eq!(output["source_revision"], revision.0.to_hex());
                            for (key, expected) in [
                                ("chronology_ref", &v1_ref),
                                ("left_member_ref", &member_ref),
                                ("right_member_ref", &member_ref),
                                (
                                    "evidence_policy_ref",
                                    &frozen["policies"]["evidence"]["reference"],
                                ),
                                (
                                    "selection_policy_ref",
                                    &frozen["policies"]["selection"]["reference"],
                                ),
                            ] {
                                assert_eq!(
                                    bytes(&parse(&canonical_json(&output[key]))),
                                    bytes(&parse(&canonical_json(expected)))
                                );
                            }
                            let native = &output["native_comparison"];
                            assert_eq!(
                                native["schema_version"],
                                "tos_temporal_comparison_result_v1"
                            );
                            assert_eq!(native["source_revision"], revision.0.to_hex());
                            // Preserve the declared spelling: this existing
                            // comparator does not admit GregorianProleptic as
                            // its lowercase calendar profile. Custody can deliver
                            // a native refusal without asserting comparability.
                            assert_eq!(native["comparison"]["status"], "unsupported");
                            assert_eq!(native["comparison"]["relation"], Value::Null);
                            assert_eq!(
                                native["comparison"]["reasons"],
                                serde_json::json!([
                                    {"side":"left","code":"calendar-not-comparable"},
                                    {"side":"left","code":"unsupported-declared-calendar"},
                                    {"side":"right","code":"calendar-not-comparable"},
                                    {"side":"right","code":"unsupported-declared-calendar"}
                                ])
                            );
                            assert_eq!(
                                native["comparison"]["basis"],
                                "normalized-source-date-envelopes"
                            );
                            for side in ["left", "right"] {
                                let old = &native[side]["claim"]["attributes"]["source_claim"];
                                assert_eq!(member["object"]["calendar"], "GregorianProleptic");
                                assert_eq!(
                                    native[side]["normalized_time"]["calendar"],
                                    member["object"]["calendar"]
                                );
                                assert_eq!(
                                    bytes(&parse(&canonical_json(old))),
                                    bytes(&parse(&canonical_json(&member)))
                                );
                                assert_ne!(
                                    bytes(&parse(&canonical_json(old))),
                                    bytes(&parse(&canonical_json(&actual_current)))
                                );
                                assert_eq!(
                                    bytes(&parse(&canonical_json(
                                        &native[side]["normalized_time"]["raw"]
                                    ))),
                                    bytes(&parse(&canonical_json(&member["object"])))
                                );
                                assert_eq!(
                                    bytes(&parse(&canonical_json(&native["request"][side]))),
                                    bytes(&locator)
                                );
                            }
                            assert_eq!(
                                native["left"]["normalized_time"],
                                native["right"]["normalized_time"]
                            );
                            let mut expected_sources = BTreeSet::new();
                            for side in ["left", "right"] {
                                for carrier in ["claim", "value"] {
                                    for source in
                                        native[side][carrier]["source_refs"].as_array().unwrap()
                                    {
                                        expected_sources.insert(source.as_str().unwrap());
                                    }
                                }
                            }
                            assert!(!expected_sources.is_empty());
                            assert_eq!(
                                native["source_refs"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|source| source.as_str().unwrap())
                                    .collect::<Vec<_>>(),
                                expected_sources.into_iter().collect::<Vec<_>>()
                            );
                            assert_eq!(native["authority_boundary"]["performs_assessment"], false);
                            for private in [
                                "owner_evidence",
                                "configuration",
                                "principal",
                                "creation_receipt",
                            ] {
                                assert!(output.get(private).is_none());
                            }
                            Ok(())
                        })?;
                        temporal_controls.revoked.store(true, Ordering::SeqCst);
                        assert!(
                            matches!(view.recheck(), Err(HeldClaimComparisonError::Query(error))
                            if error.code == tos_query::search_v2::SearchV2ErrorCode::StalePolicy)
                        );
                        temporal_controls.revoked.store(false, Ordering::SeqCst);
                        view.recheck()?;
                        temporal_controls.cancelled.store(true, Ordering::SeqCst);
                        assert!(
                            matches!(view.recheck(), Err(HeldClaimComparisonError::Query(error))
                            if error.code == tos_query::search_v2::SearchV2ErrorCode::Cancelled)
                        );
                        temporal_controls.cancelled.store(false, Ordering::SeqCst);
                        view.recheck()?;
                        let mut revoked_owner = revision_config.clone();
                        revoked_owner["allowed_operations"] = serde_json::json!([]);
                        fs::write(&revision_path, canonical_json(&revoked_owner)).unwrap();
                        assert!(view.recheck().is_err());
                        fs::write(&revision_path, &revision_config_raw).unwrap();
                        view.recheck()?;
                        assert!(temporal_held.load(Ordering::SeqCst) > 0);
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(temporal_held.load(Ordering::SeqCst), 0);
                assert!(!temporal_controls.revoked.load(Ordering::SeqCst));
                assert!(!temporal_controls.cancelled.load(Ordering::SeqCst));
                assert_eq!(fs::read(&revision_path).unwrap(), revision_config_raw);
                recheck_inventory();
                load_worker.finish(deadline, &cancellation).unwrap();
                drop(load_worker);
                drop(temporal_authority);
                drop(lookup_authority);
                drop(projection_authority);

                // BEGIN seven-context held-list fixture: ORIGINAL.
                {
                    use super::selected_metadata_authority::Authority as ListAuthority;
                    use tos_command::temporal_chronology_list::{
                        ConfiguredOwnerEnvelopeAliases, StoredChronologyListBudget,
                        StoredChronologyListError, StoredChronologyListView,
                        with_loaded_chronology_configured_owner_envelope,
                        with_loaded_chronology_metadata_list,
                    };
                    use tos_foundation::query_context::QueryContext;
                    use tos_foundation::temporal::TemporalBudget;
                    use tos_foundation::{
                        canonical_bytes_v1_with_state_budget_and_visits, emit_value_preserved_json,
                        parse_json_with_state_budget,
                    };
                    let fixture_json = JsonLimits {
                        max_bytes: 8 * 1024 * 1024,
                        max_depth: 64,
                        max_visits: 1_000_000,
                        max_integer_digits: 4096,
                    };
                    let small_json = JsonLimits {
                        max_bytes: 65_536,
                        max_depth: 64,
                        max_visits: 8192,
                        max_integer_digits: 4096,
                    };
                    let pure_json = JsonLimits {
                        max_bytes: 16_384,
                        max_depth: 64,
                        max_visits: 512,
                        max_integer_digits: 4096,
                    };
                    assert!(
                        member_raw.len() <= 65_536
                            && member_ref["id"].as_str().unwrap().len() <= 1024
                    );
                    let member_document = parse_json_with_state_budget(
                        &member_raw,
                        JsonMode::PublishedStrict,
                        small_json,
                        4 * 1024 * 1024,
                    )
                    .unwrap();
                    let frozen_record = member_document.root();
                    let frozen_subject = frozen_record
                        .object_get("subject_ref")
                        .unwrap()
                        .as_str()
                        .unwrap();
                    let frozen_predicate = frozen_record
                        .object_get("predicate")
                        .unwrap()
                        .as_str()
                        .unwrap();
                    assert!(frozen_subject.len() <= 1024 && frozen_predicate.len() <= 1024);
                    let negative_subject = "tos.fixture.held-list.absent-subject";
                    assert!(selected.graph_input_bytes.len() <= 8 * 1024 * 1024);
                    assert_eq!(
                        bound.require_source_revision().unwrap(),
                        revision.0.to_hex()
                    );
                    let selected_graph = bound
                        .source_for_adapter("reified-bibliographic-claims-v1")
                        .unwrap();
                    // The same model/bound passed cold admission, including actual scope rows/roots.
                    let candidate_count = {
                        let mut scopes =
                            selected.expectation.source_scopes.iter().filter(|scope| {
                                scope.source_graph == selected_graph
                                    && scope.adapter_profile == "reified-bibliographic-claims-v1"
                            });
                        let scope = scopes.next().expect("selected Claim scope missing");
                        assert!(scopes.next().is_none(), "selected Claim scope ambiguous");
                        let count = usize::try_from(scope.node_count)
                            .expect("selected candidate count overflow");
                        assert!(count > 0 && count.checked_add(1).unwrap() <= 250);
                        count
                    };
                    let event_path = "ToS/source-witnesses/history/synthetic-frozen-day-source/historical-event.json";
                    let event_raw = files.get(event_path).unwrap();
                    assert!(event_raw.len() <= 2048);
                    let event_document = parse_json_with_state_budget(
                        event_raw,
                        JsonMode::PublishedStrict,
                        small_json,
                        1_048_576,
                    )
                    .unwrap();
                    let event_ref_value: Value = serde_json::from_slice(&bytes(
                        &tos_command::source_forms::metadata_subject(event_document.root())
                            .unwrap(),
                    ))
                    .unwrap();
                    let actual_event_ref = exact_ref(&event_ref_value);
                    assert_eq!(
                        event_document
                            .root()
                            .object_get("record_id")
                            .unwrap()
                            .as_str(),
                        Some(actual_event_ref.id.as_str())
                    );
                    assert_eq!(
                        event_document
                            .root()
                            .object_get("record_version")
                            .unwrap()
                            .as_u64(),
                        Some(actual_event_ref.version.get())
                    );
                    assert!(actual_event_ref.id.as_str().len() <= 1024);
                    let actual_member = exact_ref(&member_ref);
                    let event_wire = serde_json::json!({"id":actual_event_ref.id.as_str(),
        "version":actual_event_ref.version.get(),"digest":actual_event_ref.digest.to_hex()});
                    let member_wire = serde_json::json!({"id":actual_member.id.as_str(),
        "version":actual_member.version.get(),"digest":actual_member.digest.to_hex()});
                    let rules_raw = canonical_json(&serde_json::json!({"temporal_systems":{
        "tos.trs.synthetic.held-list-civil-year-v1":{"calendar_algorithm":"GregorianProleptic",
        "year_numbering":"astronomical","rules_version":"synthetic-gregorian-civil-year-r1","trs_version":"1"}}}));
                    assert!(rules_raw.len() <= 65_536);
                    let rules_document = parse_json_with_state_budget(
                        &rules_raw,
                        JsonMode::PublishedStrict,
                        small_json,
                        1_048_576,
                    )
                    .unwrap();
                    let rules = rules_document.root();
                    // Only an actual returned cursor survives a call; bounded bytes, not a lease.
                    let mut returned_cursor: Option<Vec<u8>> = None;
                    for case in [
                        "metadata",
                        "empty_suffix",
                        "tagged_context",
                        "missing_carrier",
                    ] {
                        assert!(Instant::now() < deadline);
                        let early = matches!(case, "tagged_context" | "world_cursor");
                        let has_world = matches!(
                            case,
                            "missing_carrier" | "world_match" | "world_cursor" | "world_disjoint"
                        );
                        let year = if matches!(case, "world_cursor" | "world_disjoint") {
                            "1946"
                        } else {
                            "1945"
                        };
                        let mut query_context = QueryContext::default();
                        assert!(query_context.system_state.is_none());
                        assert_eq!(snapshot.current_cutoff.value, g12.as_ref().unwrap().2);
                        assert!(snapshot.current_cutoff.trs.is_none());
                        if case == "tagged_context" {
                            query_context.attributed_knowledge = Some(None);
                        }

                        if has_world {
                            let query_evidence = serde_json::json!({"declaration":"Explicit synthetic caller query only; the frozen old Claim has no supplemental temporal carrier."});
                            let raw = canonical_json(
                                &serde_json::json!({"profile_ref":"tos_world_civil_year_window_filter_v1",
                "assertion_scope":"SupplementalCarrierOnly","assertion_selection":"ExactlyOnePerPredicate",
                "non_total":"RefusePage","predicate":"Possible","coverage_rule":"GregorianCivilYearHalfOpenV1",
                "about_ref":event_wire,"position":{"type":"CalendarPosition","lexical":year,"resolution":"year",
                    "trs":{"id":"tos.trs.synthetic.held-list-civil-year-v1","version":"1"},
                    "calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical","rules_version":"synthetic-gregorian-civil-year-r1"},
                "basis":{"source":member_wire,"source_locator":member_path,"raw_evidence":query_evidence}}),
                            );
                            assert!(raw.len() <= 65_536);
                            let doc = parse_json_with_state_budget(
                                &raw,
                                JsonMode::PublishedStrict,
                                small_json,
                                1_048_576,
                            )
                            .unwrap();
                            query_context.world_valid_time = Some(doc.into_root());

                            // No extra codec/evaluator call is hidden in preflight. These are
                            // bounded emit measurements of actual arguments and persisted data.
                            let filter_bytes = emit_value_preserved_json(
                                query_context.world_valid_time.as_ref().unwrap(),
                                pure_json,
                            )
                            .unwrap()
                            .len();
                            let rules_bytes =
                                emit_value_preserved_json(rules, pure_json).unwrap().len();
                            let reference_bytes = canonical_json(&member_wire).len();
                            let packet_bytes = 0usize;
                            let expression_bound = filter_bytes
                                .checked_mul(2)
                                .unwrap()
                                .checked_add(rules_bytes)
                                .unwrap()
                                .checked_add(reference_bytes)
                                .unwrap()
                                .checked_add(packet_bytes.checked_mul(3).unwrap())
                                .unwrap()
                                .checked_add(actual_member.id.as_str().len())
                                .unwrap()
                                .checked_add(2048)
                                .unwrap();
                            assert!(
                                expression_bound <= 16_384,
                                "retain the reviewed per-foundation byte cap"
                            );
                        }
                        let negative = case == "missing_carrier";
                        let page_size = if negative { 1 } else { 2 };
                        let scan_limit = if negative {
                            candidate_count.checked_add(1).unwrap()
                        } else {
                            candidate_count
                        };
                        assert!(page_size <= 16 && scan_limit <= 250);
                        let request_raw = canonical_json(
                            &serde_json::json!({"schema_version":"tos_stored_chronology_metadata_list_request_v1",
            "filter":if negative {serde_json::json!({"subject_ref":negative_subject})}
                else {serde_json::json!({"subject_ref":frozen_subject,"predicate":frozen_predicate})},
            "page_size":page_size,"scan_limit":scan_limit,"after":null}),
                        );
                        assert!(request_raw.len() <= 65_536);
                        let mut request = parse_json_with_state_budget(
                            &request_raw,
                            JsonMode::PublishedStrict,
                            small_json,
                            1_048_576,
                        )
                        .unwrap()
                        .into_root();
                        if matches!(case, "empty_suffix" | "tagged_context" | "world_cursor") {
                            let raw = returned_cursor.as_ref().unwrap();
                            assert!(raw.len() <= 65_536 && raw.capacity() <= 131_080);
                            let cursor = parse_json_with_state_budget(
                                raw,
                                JsonMode::PublishedStrict,
                                small_json,
                                1_048_576,
                            )
                            .unwrap()
                            .into_root();
                            let JsonValue::Object(ref mut fields) = request else {
                                panic!("request object")
                            };
                            fields
                                .iter_mut()
                                .find(|(key, _)| key.as_str() == Some("after"))
                                .unwrap()
                                .1 = cursor;
                        }
                        assert!(Instant::now() < deadline);
                        let mut list_worker = super::command_form_cases::schemas_with_image(
                            &current_cut,
                            &image,
                            deadline,
                            &cancellation,
                        );
                        let loaded_list = load_chronology_from_owner(
                            &revision_fs,
                            &current_context,
                            &current_cut,
                            &exact_ref(&v1_ref),
                            &context,
                            &cut,
                            &snapshot,
                            &software,
                            &components,
                            &mut list_worker,
                            ChronologyLimits::default(),
                            deadline,
                            &cancellation,
                        )
                        .unwrap();
                        assert_eq!(
                            loaded_list
                                .bound()
                                .chronology()
                                .member_claim_revisions
                                .as_slice(),
                            std::slice::from_ref(&actual_member)
                        );
                        if case == "tagged_context" {
                            assert!(
                                loaded_list
                                    .bound()
                                    .chronology()
                                    .selection
                                    .attributed_knowledge
                                    .is_none()
                            );
                        }
                        held_list_fixture_counts[0] += 1;
                        let inspect_controls = Arc::new(Controls::default());
                        let list_controls = Arc::new(Controls::default());
                        let inspect_held = Arc::new(AtomicUsize::new(0));
                        let list_held = Arc::new(AtomicUsize::new(0));
                        let mut inspect_authority = ListAuthority::for_scope(
                            &bound,
                            "tos.knowledge.node.inspect",
                            "read_only_public_knowledge_inspect_v1",
                            inspect_held.clone(),
                            inspect_controls.clone(),
                        );
                        let inner_list_authority = ListAuthority::for_scope(
                            &bound,
                            "tos.knowledge.temporal.list_claims",
                            "read_only_public_knowledge_claim_list_v1",
                            list_held.clone(),
                            list_controls.clone(),
                        );
                        let observed_counts = std::cell::Cell::new([0usize; 4]);
                        let mut list_authority = ListCandidateObserver {
                            inner: inner_list_authority,
                            graph: selected_graph,
                            subject: frozen_subject,
                            predicate: frozen_predicate,
                            negative_subject,
                            counts: &observed_counts,
                        };
                        let mut callback_called = false;
                        let mut list_budget = StoredChronologyListBudget {
                            query: tos_query::InspectBudget {
                                max_open_vm_steps: selected.cold_limits().max_vm_steps,
                                max_read_vm_steps: 20_000_000,
                                max_matches: 16,
                                max_rows: 1000,
                                max_field_bytes: 16_384,
                                max_payload_bytes: 1_048_576,
                                max_decoded_bytes: 8 * 1024 * 1024,
                                max_response_bytes: 2 * 1024 * 1024,
                                json: JsonLimits {
                                    max_bytes: 1_048_576,
                                    max_depth: 64,
                                    max_visits: 1_000_000,
                                    max_integer_digits: 4096,
                                },
                            },
                            temporal: TemporalBudget {
                                max_expression_bytes: 49_152,
                                max_members: 24_576,
                                max_pairings: 12_288,
                            },
                            max_members: 1,
                            max_extra_owned_bytes: (16usize * 1024 * 1024)
                                .checked_sub(g12_debit.0)
                                .unwrap(),
                            max_extra_json_visits: 1_000_000usize.checked_sub(g12_debit.1).unwrap(),
                            max_verification_owned_bytes: 4 * 1024 * 1024,
                            max_verification_json_visits: 250_000,
                        };
                        let consume = {
                            |view: &mut StoredChronologyListView<'_>| {
                                callback_called = true;
                                assert!(!early);
                                let observed = observed_counts.get();
                                if case == "empty_suffix" {
                                    assert_eq!(observed, [0; 4]);
                                } else {
                                    assert_eq!(observed[0], candidate_count);
                                    assert!(observed[1] >= 1 && observed[1] <= observed[0]);
                                    assert_eq!(
                                        observed[2], 1,
                                        "filtered uniqueness includes nonmember Claims"
                                    );
                                    assert_eq!(
                                        observed[3], 0,
                                        "negative subject absent from all authenticated selected Claims"
                                    );
                                }
                                assert_eq!(inspect_held.load(Ordering::SeqCst), 1);
                                assert_eq!(list_held.load(Ordering::SeqCst), 1);
                                view.with_current_bytes(|raw| {
                    assert!(raw.len()<=2*1024*1024);
                    let public_doc=parse_json_with_state_budget(raw,JsonMode::PublishedStrict,
                        JsonLimits{max_bytes:2*1024*1024,..fixture_json},4*1024*1024).unwrap();
                    let public=public_doc.root();
                    assert_eq!(public.object_get("schema_version").unwrap().as_str(),Some("tos_stored_chronology_metadata_list_result_v1"));
                    assert_eq!(public.object_get("source_revision").unwrap().as_str(),Some(revision.0.to_prefixed().as_str()));
                    assert_eq!(bytes(public.object_get("chronology_ref").unwrap()),bytes(&parse(&canonical_json(&v1_ref))));
                    assert_eq!(public.object_get("selected_model_receipt_id").unwrap().as_str(),Some(bound.owner_receipt_id()));
                    assert_eq!(public.object_get("binding_scope").unwrap().as_str(),Some("OwnerStoredChronologyExactSelectedModelMembers"));
                    assert_eq!(public.object_get("policy_scope").unwrap().as_str(),Some("InlineSourceDefinitionNotAuthorization"));
                    let effective_selection=public.object_get("effective_context").unwrap().object_get("selection").unwrap();
                    assert_eq!(effective_selection.object_get("system_as_of").unwrap().object_get("value").unwrap(),&g12.as_ref().unwrap().1);
                    assert!(effective_selection.object_get("system_as_of").unwrap().object_get("trs").unwrap().is_null());
                    assert!(effective_selection.object_get("attributed_knowledge").unwrap().is_null());
                    assert!(effective_selection.object_get("source_publication_as_of").unwrap().is_null());
                    assert_eq!(public.object_get("a3_scope").unwrap().as_str(),Some("NotEstablished"));
                    assert_eq!(public.object_get("legacy_object_assessment").unwrap().as_str(),Some("NotExamined"));
                    assert_eq!(public.object_get("event_extent_inferred").unwrap().as_bool(),Some(false));
                    for key in ["owner_evidence","configuration","principal","creation_receipt","provider_response","projection"] {
                        assert!(public.object_get(key).is_none());
                    }
                    if negative {
                        assert_eq!(public.as_object().unwrap().len(),17);
                        assert_eq!(public.object_get("outcome").unwrap().as_str(),Some("NonTotalRefusal"));
                        for key in ["rows","next","cursor_scope","examined_candidates","exhausted"] {assert!(public.object_get(key).is_none());}
                        let refusals=public.object_get("refusals").unwrap().as_array().unwrap();assert_eq!(refusals.len(),1);
                        assert_eq!(refusals[0].object_get("domain").unwrap().as_str(),Some("WorldWindow"));
                        assert_eq!(refusals[0].object_get("reason").unwrap().as_str(),Some("MissingTemporalCarrier"));
                        assert_eq!(bytes(refusals[0].object_get("frozen_member_ref").unwrap()),bytes(&parse(&canonical_json(&member_ref))));
                    }else{
                        assert_eq!(public.as_object().unwrap().len(),22);
                        assert_eq!(public.object_get("outcome").unwrap().as_str(),Some("Page"));
                        let empty=matches!(case,"empty_suffix"|"world_disjoint");
                        let rows=public.object_get("rows").unwrap().as_array().unwrap();assert_eq!(rows.len(),if empty {0}else{1});
                        assert_eq!(public.object_get("examined_candidates").unwrap().as_u64(),Some(if case=="empty_suffix"{0}else{candidate_count as u64}));
                        assert_eq!(public.object_get("exhausted").unwrap().as_bool(),Some(case=="empty_suffix"));
                        let next=public.object_get("next").unwrap();
                        if case=="empty_suffix"{assert!(next.is_null());}else{assert!(!next.is_null());}
                        for row in rows {
                            assert_eq!(row.as_object().unwrap().len(),6);
                            assert_eq!(bytes(row.object_get("frozen_member_ref").unwrap()),bytes(&parse(&canonical_json(&member_ref))));
                            assert_eq!(row.object_get("provider_claim_digest_profile").unwrap().as_str(),Some("SourceRecordDigestV1"));
                            for key in ["node_id","content_revision"] {assert_eq!(row.object_get(key).unwrap().as_str(),locator.object_get(key).unwrap().as_str());}
                            let source=row.object_get("carrier").unwrap().object_get("attributes").unwrap().object_get("source_claim").unwrap();
                            assert_eq!(bytes(source),bytes(frozen_record));
                            assert_ne!(bytes(source),bytes(&parse(&actual_current_raw)));
                            let provider_ref=row.object_get("provider_claim_ref").unwrap();
                            assert_eq!(provider_ref.object_get("id").unwrap().as_str(),Some(actual_member.id.as_str()));
                            assert_eq!(provider_ref.object_get("version").unwrap().as_u64(),Some(actual_member.version.get()));
                            let source_digest=Digest256::of_bytes(&canonical_bytes_v1(source,CanonicalProfile::SourceRecordDigestV1,small_json).unwrap());
                            assert_eq!(Digest256::from_prefixed(provider_ref.object_get("digest").unwrap().as_str().unwrap()).unwrap(),source_digest);
                            // This is NOT owner-domain digest equality/inequality.
                            assert_eq!(source.object_get("object").unwrap().object_get("calendar").unwrap().as_str(),Some("GregorianProleptic"));
                        }
                        let assessments=public.object_get("world_assessments").unwrap().as_array().unwrap();
                        assert_eq!(assessments.len(),if has_world {1}else{0});
                        if has_world {
                            let assessment=assessments[0].object_get("result").unwrap();
                            assert_eq!(assessment.object_get("outcome").unwrap().as_str(),Some(if case=="world_disjoint"{"NoMatch"}else{"Match"}));
                            assert_eq!(assessment.object_get("classification").unwrap().as_str(),Some(if case=="world_disjoint"{"Disjoint"}else{"All"}));
                            assert_eq!(assessment.object_get("event_extent_inferred").unwrap().as_bool(),Some(false));
                            assert_eq!(public.object_get("rules_scope").unwrap().as_str(),Some("ExplicitCallerRulesSourceMatchedNotPolicyAdmission"));
                            unreachable!("the original source reaches NonTotalRefusal instead of world success");
                        }else{assert!(public.object_get("world_filter").unwrap().is_null());assert!(public.object_get("rules_scope").unwrap().is_null());}
                        if matches!(case,"metadata"|"world_match") {
                            assert!(returned_cursor.is_none());
                            let (cursor,_) = canonical_bytes_v1_with_state_budget_and_visits(next,
                                CanonicalProfile::SourceCommandInputV1,small_json,1_048_576).unwrap();
                            assert!(cursor.len()<=65_536 && cursor.capacity()<=131_080);
                            returned_cursor=Some(cursor);
                        }
                    }
                    Ok(())
                })?;
                                for controls in [&inspect_controls, &list_controls] {
                                    controls.revoked.store(true, Ordering::SeqCst);
                                    assert!(
                                        matches!(view.recheck(),Err(StoredChronologyListError::Query(error)) if error.code==tos_query::search_v2::SearchV2ErrorCode::StalePolicy)
                                    );
                                    controls.revoked.store(false, Ordering::SeqCst);
                                    view.recheck()?;
                                    controls.cancelled.store(true, Ordering::SeqCst);
                                    assert!(
                                        matches!(view.recheck(),Err(StoredChronologyListError::Query(error)) if error.code==tos_query::search_v2::SearchV2ErrorCode::Cancelled)
                                    );
                                    controls.cancelled.store(false, Ordering::SeqCst);
                                    view.recheck()?;
                                }
                                // This temporary TEST owner drift changes no source or cut.
                                assert!(revision_config_raw.len() <= 65_536);
                                let mut drift = revision_config.clone();
                                drift["allowed_operations"] = serde_json::json!([]);
                                fs::write(&revision_path, canonical_json(&drift)).unwrap();
                                assert!(view.recheck().is_err());
                                fs::write(&revision_path, &revision_config_raw).unwrap();
                                view.recheck()?;
                                assert_eq!(inspect_held.load(Ordering::SeqCst), 1);
                                assert_eq!(list_held.load(Ordering::SeqCst), 1);
                                Ok(())
                            }
                        };
                        let result = if case == "metadata" {
                            // Expected-free original45 SUBPROTOCOL only. Full manifest,
                            // policies and temporal selectors are deliberately not stripped.
                            assert_eq!(Digest256::of_bytes(include_bytes!(
                                "../foundation-wave-v1/oracles/temporal-acceptance-vectors.json")),
                                Digest256::from_hex("fda08fd958914a3605afab12e7b78012a3f5534ecfdffe2ab31cb2cb3a26eecd").unwrap());
                            let original_input = br#"{"operation":"list_claims","request_envelope":{"principal_auth_context":{"authorization":"permitted","principal_ref":"principal-fixture-reader"},"source_snapshot_ref":{"revision_fence":"source-rev-42","snapshot_id":"S1"},"temporal":{"chronology_version":"CV1"}},"source_profile":"ToS-Temporal-Core-Contract-v1","subject":"subject-A"}"#;
                            assert_eq!(original_input.len(), 333);
                            assert_eq!(Digest256::of_bytes(original_input),
                                Digest256::from_hex("ef315c29d161aee13e8a76f0d11b26cdba820456696a25458e33ce470ee464be").unwrap());
                            assert!(
                                query_context.system_state.is_none()
                                    && query_context.attributed_knowledge.is_none()
                                    && query_context.source_publication_as_of.is_none()
                                    && query_context.world_valid_time.is_none()
                                    && query_context.display_perspective.is_none()
                            );
                            assert!(!has_world);
                            assert_eq!(revision_config["principal_id"], "software:test-fixture");
                            assert_eq!(
                                revision_config["authority_ref"],
                                "synthetic-test-only:metadata-revision-not-admission"
                            );
                            assert_eq!(
                                request
                                    .object_get("filter")
                                    .unwrap()
                                    .object_get("subject_ref")
                                    .unwrap()
                                    .as_str(),
                                Some(frozen_subject)
                            );
                            assert_eq!(
                                request
                                    .object_get("filter")
                                    .unwrap()
                                    .object_get("predicate")
                                    .unwrap()
                                    .as_str(),
                                Some(frozen_predicate)
                            );
                            // Preserve the actual12MiB/967232 post-G12 budget. Debit
                            // caller document/ref/header overlap BEFORE parse/clones.
                            let caller_state = original_input
                                .len()
                                .checked_mul(128)
                                .and_then(|n| n.checked_add(4096))
                                .unwrap();
                            assert_eq!(caller_state, 46_720);
                            let caller_overhead =
                                std::mem::size_of::<tos_foundation::JsonDocument>()
                                    .checked_add(std::mem::size_of::<
                                        ConfiguredOwnerEnvelopeAliases<'_>,
                                    >())
                                    .and_then(|n| n.checked_add(4096))
                                    .unwrap();
                            list_budget.max_extra_owned_bytes = list_budget
                                .max_extra_owned_bytes
                                .checked_sub(caller_state.checked_add(caller_overhead).unwrap())
                                .unwrap();
                            list_budget.max_extra_json_visits =
                                list_budget.max_extra_json_visits.checked_sub(8192).unwrap();
                            let original_document = parse_json_with_state_budget(
                                original_input,
                                JsonMode::PublishedStrict,
                                small_json,
                                caller_state,
                            )
                            .unwrap();
                            let chronology_ref = exact_ref(&v1_ref);
                            let aliases = ConfiguredOwnerEnvelopeAliases {
                                snapshot: ("S1", &snapshot.reference),
                                revision_fence: ("source-rev-42", revision),
                                chronology: ("CV1", &chronology_ref),
                                subject: ("subject-A", frozen_subject),
                                principal: (
                                    "principal-fixture-reader",
                                    "software:test-fixture",
                                    "synthetic-test-only:metadata-revision-not-admission",
                                ),
                            };
                            with_loaded_chronology_configured_owner_envelope(
                                loaded_list,
                                &mut cold,
                                &bound,
                                &mut inspect_authority,
                                &mut list_authority,
                                &snapshot,
                                original_document.root(),
                                &aliases,
                                &request,
                                list_budget,
                                deadline,
                                &cancellation,
                                consume,
                            )
                        } else {
                            with_loaded_chronology_metadata_list(
                                loaded_list,
                                &mut cold,
                                &bound,
                                &mut inspect_authority,
                                &mut list_authority,
                                &snapshot,
                                &query_context,
                                if has_world { Some(rules) } else { None },
                                &request,
                                list_budget,
                                deadline,
                                &cancellation,
                                consume,
                            )
                        };
                        if early {
                            assert!(matches!(
                                result,
                                Err(StoredChronologyListError::InvalidRequest(_))
                            ));
                            assert!(!callback_called);
                            assert_eq!(observed_counts.get(), [0; 4]);
                            held_list_fixture_counts[2] += 1;
                        } else {
                            result.unwrap();
                            assert!(callback_called);
                            held_list_fixture_counts[1] += 2;
                        }
                        assert_eq!(inspect_held.load(Ordering::SeqCst), 0);
                        assert_eq!(list_held.load(Ordering::SeqCst), 0);
                        assert!(
                            !inspect_controls.revoked.load(Ordering::SeqCst)
                                && !inspect_controls.cancelled.load(Ordering::SeqCst)
                        );
                        assert!(
                            !list_controls.revoked.load(Ordering::SeqCst)
                                && !list_controls.cancelled.load(Ordering::SeqCst)
                        );
                        assert_eq!(fs::read(&revision_path).unwrap(), revision_config_raw);
                        assert_eq!(
                            fs::read(isolated.path().join(&member_path)).unwrap(),
                            actual_current_raw
                        );
                        assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), v1_raw);
                        drop(inspect_authority);
                        drop(list_authority);
                        list_worker.finish(deadline, &cancellation).unwrap();
                        drop(list_worker);
                        recheck_inventory();
                        eprintln!(
                            "held list fixture source=ORIGINAL case={} phase=complete early_cursor_refusal={}",
                            case, early
                        );
                    }
                    assert!(
                        returned_cursor
                            .as_ref()
                            .is_some_and(|raw| raw.len() <= 65_536)
                    );
                    // General positive composition: actual complete ORIGINAL N1,
                    // metadata ONLY. No world declaration or subset of NEW A+B.
                    {
                        use tos_command::temporal_chronology_list::{
                            ListManifestEnvelopeAliases, ListManifestSliceAliases,
                            with_loaded_chronology_manifest_slice_envelope,
                        };
                        let case = "metadata";
                        let early = false;
                        let negative = false;
                        let has_world = false;
                        let mut returned_cursor: Option<Vec<u8>> = None;
                        // New caller construction/parse/ref/cursor overlap is prepaid
                        // from ACTUAL post-G12 parent, not a reset to16MiB.
                        assert!(
                            (16usize * 1024 * 1024).checked_sub(g12_debit.0).unwrap()
                                > 1_048_576 + 4 * 1024 * 1024
                        );
                        assert!(1_000_000usize.checked_sub(g12_debit.1).unwrap() > 8192 + 250_000);
                        let general_positive_bytes = (16usize * 1024 * 1024)
                            .checked_sub(g12_debit.0)
                            .and_then(|n| n.checked_sub(3 * 1_048_576))
                            .unwrap();
                        let general_positive_visits = 1_000_000usize
                            .checked_sub(g12_debit.1)
                            .and_then(|n| n.checked_sub(8192 + 65_536))
                            .unwrap();
                        assert!(general_positive_bytes > 4 * 1024 * 1024);
                        assert!(general_positive_visits > 250_000);
                        assert!(actual_current_raw.len() <= 65_536);
                        assert!(revision_config_raw.len() <= 8192);
                        // Retained comparison input, within the prepaid2MiB
                        // consumer window; no unbounded parser for the NEW call.
                        let current_document = parse_json_with_state_budget(
                            &actual_current_raw,
                            JsonMode::PublishedStrict,
                            JsonLimits {
                                max_bytes: 65_536,
                                max_visits: 8192,
                                ..small_json
                            },
                            262_144,
                        )
                        .unwrap();
                        let cutoff = g12.as_ref().unwrap().1.as_str().unwrap();
                        assert!(cutoff.len() <= 1024);
                        let input_raw = canonical_json(&serde_json::json!({
                            "operation":"list_claims","source_profile":"ToS-Temporal-Core-Contract-v1",
                            "subject":"subject-single","request_envelope":{
                                "source_snapshot_ref":{"snapshot_id":"S1","revision_fence":"source-rev-42"},
                                "temporal":{"chronology_version":"CV1"}},
                            "snapshot_manifest":{"chronology_version_id":"CV1","frozen_knowledge_selection":{
                                "system_state":{"type":"SystemAsOf","cutoff":cutoff},
                                "source_snapshot_ref":{"snapshot_id":"S1"},"source_revision_fence":"source-rev-42",
                                "member_claim_revisions":["claim-single@r1"]}}}));
                        assert!(input_raw.len() <= 2048 && input_raw.capacity() <= 65_536);
                        let input_state = input_raw
                            .len()
                            .checked_mul(128)
                            .and_then(|n| n.checked_add(4096))
                            .unwrap();
                        assert!(input_state <= 266_240);
                        let input_document = parse_json_with_state_budget(
                            &input_raw,
                            JsonMode::PublishedStrict,
                            small_json,
                            input_state,
                        )
                        .unwrap();
                        let request_raw = canonical_json(&serde_json::json!({
                            "schema_version":"tos_stored_chronology_metadata_list_request_v1",
                            "filter":{"subject_ref":frozen_subject,"predicate":frozen_predicate},
                            "page_size":2,"scan_limit":candidate_count,"after":null}));
                        assert!(request_raw.len() <= 2048 && request_raw.capacity() <= 65_536);
                        let request_state = request_raw
                            .len()
                            .checked_mul(128)
                            .and_then(|n| n.checked_add(4096))
                            .unwrap();
                        assert!(request_state <= 266_240);
                        let request_document = parse_json_with_state_budget(
                            &request_raw,
                            JsonMode::PublishedStrict,
                            small_json,
                            request_state,
                        )
                        .unwrap();
                        assert!(
                            input_state
                                .checked_add(request_state)
                                .and_then(|n| n.checked_add(2 * 65_536 + 65_536))
                                .unwrap()
                                <= 1_048_576
                        );
                        let chronology_ref = exact_ref(&v1_ref);
                        let aliases = ListManifestSliceAliases {
                            envelope: ListManifestEnvelopeAliases::Metadata {
                                snapshot: ("S1", &snapshot.reference),
                                revision_fence: ("source-rev-42", revision),
                                chronology: ("CV1", &chronology_ref),
                                subject: ("subject-single", frozen_subject),
                            },
                            members: &[("claim-single@r1", &actual_member)],
                        };
                        let mut list_worker = super::command_form_cases::schemas_with_image(
                            &current_cut,
                            &image,
                            deadline,
                            &cancellation,
                        );
                        let loaded_list = load_chronology_from_owner(
                            &revision_fs,
                            &current_context,
                            &current_cut,
                            &exact_ref(&v1_ref),
                            &context,
                            &cut,
                            &snapshot,
                            &software,
                            &components,
                            &mut list_worker,
                            ChronologyLimits::default(),
                            deadline,
                            &cancellation,
                        )
                        .unwrap();
                        assert_eq!(
                            loaded_list
                                .bound()
                                .chronology()
                                .member_claim_revisions
                                .as_slice(),
                            std::slice::from_ref(&actual_member)
                        );
                        if case == "tagged_context" {
                            assert!(
                                loaded_list
                                    .bound()
                                    .chronology()
                                    .selection
                                    .attributed_knowledge
                                    .is_none()
                            );
                        }
                        held_list_fixture_counts[0] += 1;
                        let inspect_controls = Arc::new(Controls::default());
                        let list_controls = Arc::new(Controls::default());
                        let inspect_held = Arc::new(AtomicUsize::new(0));
                        let list_held = Arc::new(AtomicUsize::new(0));
                        let mut inspect_authority = ListAuthority::for_scope(
                            &bound,
                            "tos.knowledge.node.inspect",
                            "read_only_public_knowledge_inspect_v1",
                            inspect_held.clone(),
                            inspect_controls.clone(),
                        );
                        let inner_list_authority = ListAuthority::for_scope(
                            &bound,
                            "tos.knowledge.temporal.list_claims",
                            "read_only_public_knowledge_claim_list_v1",
                            list_held.clone(),
                            list_controls.clone(),
                        );
                        let observed_counts = std::cell::Cell::new([0usize; 4]);
                        let mut list_authority = ListCandidateObserver {
                            inner: inner_list_authority,
                            graph: selected_graph,
                            subject: frozen_subject,
                            predicate: frozen_predicate,
                            negative_subject,
                            counts: &observed_counts,
                        };
                        // New consumer additionally prepays2MiB/65536 visits:
                        // <=1MiB public parse state plus bounded canonical-compare,
                        // cursor/config/key workspace and overlap. This is a
                        // DEBIT of the same parent, not another allowance.
                        let list_budget = StoredChronologyListBudget {
                            query: tos_query::InspectBudget {
                                max_open_vm_steps: selected.cold_limits().max_vm_steps,
                                max_read_vm_steps: 20_000_000,
                                max_matches: 16,
                                max_rows: 1000,
                                max_field_bytes: 16_384,
                                max_payload_bytes: 1_048_576,
                                max_decoded_bytes: 8 * 1024 * 1024,
                                max_response_bytes: 262_144,
                                json: JsonLimits {
                                    max_bytes: 1_048_576,
                                    max_depth: 64,
                                    max_visits: 1_000_000,
                                    max_integer_digits: 4096,
                                },
                            },
                            temporal: TemporalBudget {
                                max_expression_bytes: 49_152,
                                max_members: 24_576,
                                max_pairings: 12_288,
                            },
                            max_members: 1,
                            max_extra_owned_bytes: general_positive_bytes,
                            max_extra_json_visits: general_positive_visits,
                            max_verification_owned_bytes: 4 * 1024 * 1024,
                            max_verification_json_visits: 250_000,
                        };
                        let mut callback_called = false;
                        with_loaded_chronology_manifest_slice_envelope(
                            loaded_list, &mut cold, &bound, &mut inspect_authority, &mut list_authority,
                            &snapshot, input_document.root(), &aliases, request_document.root(), None, None,
                            list_budget, deadline, &cancellation,
                            |view| {
                                callback_called = true;
                                assert!(!early);
                                let observed = observed_counts.get();
                                if case == "empty_suffix" {
                                    assert_eq!(observed, [0; 4]);
                                } else {
                                    assert_eq!(observed[0], candidate_count);
                                    assert!(observed[1] >= 1 && observed[1] <= observed[0]);
                                    assert_eq!(
                                        observed[2], 1,
                                        "filtered uniqueness includes nonmember Claims"
                                    );
                                    assert_eq!(
                                        observed[3], 0,
                                        "negative subject absent from all authenticated selected Claims"
                                    );
                                }
                                assert_eq!(inspect_held.load(Ordering::SeqCst), 1);
                                assert_eq!(list_held.load(Ordering::SeqCst), 1);
                                view.with_current_bytes(|raw| {
                    assert!(raw.len()<=65_536);
                    let public_doc=parse_json_with_state_budget(raw,JsonMode::PublishedStrict,
                        JsonLimits{max_bytes:65_536,max_visits:16_384,..fixture_json},1_048_576).unwrap();
                    let public=public_doc.root();
                    assert_eq!(public.object_get("schema_version").unwrap().as_str(),Some("tos_stored_chronology_metadata_list_result_v1"));
                    assert_eq!(public.object_get("source_revision").unwrap().as_str(),Some(revision.0.to_prefixed().as_str()));
                    assert_eq!(bytes(public.object_get("chronology_ref").unwrap()),bytes(&parse(&canonical_json(&v1_ref))));
                    assert_eq!(public.object_get("selected_model_receipt_id").unwrap().as_str(),Some(bound.owner_receipt_id()));
                    assert_eq!(public.object_get("binding_scope").unwrap().as_str(),Some("OwnerStoredChronologyExactSelectedModelMembers"));
                    assert_eq!(public.object_get("policy_scope").unwrap().as_str(),Some("InlineSourceDefinitionNotAuthorization"));
                    let effective_selection=public.object_get("effective_context").unwrap().object_get("selection").unwrap();
                    assert_eq!(effective_selection.object_get("system_as_of").unwrap().object_get("value").unwrap(),&g12.as_ref().unwrap().1);
                    assert!(effective_selection.object_get("system_as_of").unwrap().object_get("trs").unwrap().is_null());
                    assert!(effective_selection.object_get("attributed_knowledge").unwrap().is_null());
                    assert!(effective_selection.object_get("source_publication_as_of").unwrap().is_null());
                    assert_eq!(public.object_get("a3_scope").unwrap().as_str(),Some("NotEstablished"));
                    assert_eq!(public.object_get("legacy_object_assessment").unwrap().as_str(),Some("NotExamined"));
                    assert_eq!(public.object_get("event_extent_inferred").unwrap().as_bool(),Some(false));
                    for key in ["owner_evidence","configuration","principal","creation_receipt","provider_response","projection"] {
                        assert!(public.object_get(key).is_none());
                    }
                    if negative {
                        assert_eq!(public.as_object().unwrap().len(),17);
                        assert_eq!(public.object_get("outcome").unwrap().as_str(),Some("NonTotalRefusal"));
                        for key in ["rows","next","cursor_scope","examined_candidates","exhausted"] {assert!(public.object_get(key).is_none());}
                        let refusals=public.object_get("refusals").unwrap().as_array().unwrap();assert_eq!(refusals.len(),1);
                        assert_eq!(refusals[0].object_get("domain").unwrap().as_str(),Some("WorldWindow"));
                        assert_eq!(refusals[0].object_get("reason").unwrap().as_str(),Some("MissingTemporalCarrier"));
                        assert_eq!(bytes(refusals[0].object_get("frozen_member_ref").unwrap()),bytes(&parse(&canonical_json(&member_ref))));
                    }else{
                        assert_eq!(public.as_object().unwrap().len(),22);
                        assert_eq!(public.object_get("outcome").unwrap().as_str(),Some("Page"));
                        let empty=matches!(case,"empty_suffix"|"world_disjoint");
                        let rows=public.object_get("rows").unwrap().as_array().unwrap();assert_eq!(rows.len(),if empty {0}else{1});
                        assert_eq!(public.object_get("examined_candidates").unwrap().as_u64(),Some(if case=="empty_suffix"{0}else{candidate_count as u64}));
                        assert_eq!(public.object_get("exhausted").unwrap().as_bool(),Some(case=="empty_suffix"));
                        let next=public.object_get("next").unwrap();
                        if case=="empty_suffix"{assert!(next.is_null());}else{assert!(!next.is_null());}
                        for row in rows {
                            assert_eq!(row.as_object().unwrap().len(),6);
                            assert_eq!(bytes(row.object_get("frozen_member_ref").unwrap()),bytes(&parse(&canonical_json(&member_ref))));
                            assert_eq!(row.object_get("provider_claim_digest_profile").unwrap().as_str(),Some("SourceRecordDigestV1"));
                            for key in ["node_id","content_revision"] {assert_eq!(row.object_get(key).unwrap().as_str(),locator.object_get(key).unwrap().as_str());}
                            let source=row.object_get("carrier").unwrap().object_get("attributes").unwrap().object_get("source_claim").unwrap();
                            assert_eq!(bytes(source),bytes(frozen_record));
                            assert_ne!(bytes(source),bytes(current_document.root()));
                            let provider_ref=row.object_get("provider_claim_ref").unwrap();
                            assert_eq!(provider_ref.object_get("id").unwrap().as_str(),Some(actual_member.id.as_str()));
                            assert_eq!(provider_ref.object_get("version").unwrap().as_u64(),Some(actual_member.version.get()));
                            let source_digest=Digest256::of_bytes(&canonical_bytes_v1(source,CanonicalProfile::SourceRecordDigestV1,small_json).unwrap());
                            assert_eq!(Digest256::from_prefixed(provider_ref.object_get("digest").unwrap().as_str().unwrap()).unwrap(),source_digest);
                            // This is NOT owner-domain digest equality/inequality.
                            assert_eq!(source.object_get("object").unwrap().object_get("calendar").unwrap().as_str(),Some("GregorianProleptic"));
                        }
                        let assessments=public.object_get("world_assessments").unwrap().as_array().unwrap();
                        assert_eq!(assessments.len(),if has_world {1}else{0});
                        if has_world {
                            let assessment=assessments[0].object_get("result").unwrap();
                            assert_eq!(assessment.object_get("outcome").unwrap().as_str(),Some(if case=="world_disjoint"{"NoMatch"}else{"Match"}));
                            assert_eq!(assessment.object_get("classification").unwrap().as_str(),Some(if case=="world_disjoint"{"Disjoint"}else{"All"}));
                            assert_eq!(assessment.object_get("event_extent_inferred").unwrap().as_bool(),Some(false));
                            assert_eq!(public.object_get("rules_scope").unwrap().as_str(),Some("ExplicitCallerRulesSourceMatchedNotPolicyAdmission"));
                            unreachable!("the original source reaches NonTotalRefusal instead of world success");
                        }else{assert!(public.object_get("world_filter").unwrap().is_null());assert!(public.object_get("rules_scope").unwrap().is_null());}
                        if matches!(case,"metadata"|"world_match") {
                            assert!(returned_cursor.is_none());
                            let (cursor,_) = canonical_bytes_v1_with_state_budget_and_visits(next,
                                CanonicalProfile::SourceCommandInputV1,small_json,1_048_576).unwrap();
                            assert!(cursor.len()<=65_536 && cursor.capacity()<=131_080);
                            returned_cursor=Some(cursor);
                        }
                    }
                    Ok(())
                })?;
                                for controls in [&inspect_controls, &list_controls] {
                                    controls.revoked.store(true, Ordering::SeqCst);
                                    assert!(
                                        matches!(view.recheck(),Err(StoredChronologyListError::Query(error)) if error.code==tos_query::search_v2::SearchV2ErrorCode::StalePolicy)
                                    );
                                    controls.revoked.store(false, Ordering::SeqCst);
                                    view.recheck()?;
                                    controls.cancelled.store(true, Ordering::SeqCst);
                                    assert!(
                                        matches!(view.recheck(),Err(StoredChronologyListError::Query(error)) if error.code==tos_query::search_v2::SearchV2ErrorCode::Cancelled)
                                    );
                                    controls.cancelled.store(false, Ordering::SeqCst);
                                    view.recheck()?;
                                }
                                // This temporary TEST owner drift changes no source or cut.
                                assert!(revision_config_raw.len() <= 65_536);
                                let mut drift = revision_config.clone();
                                drift["allowed_operations"] = serde_json::json!([]);
                                fs::write(&revision_path, canonical_json(&drift)).unwrap();
                                assert!(view.recheck().is_err());
                                fs::write(&revision_path, &revision_config_raw).unwrap();
                                view.recheck()?;
                                assert_eq!(inspect_held.load(Ordering::SeqCst), 1);
                                assert_eq!(list_held.load(Ordering::SeqCst), 1);
                                Ok(())
                            },
                        ).unwrap();
                        assert!(callback_called);
                        assert!(
                            returned_cursor
                                .as_ref()
                                .is_some_and(|raw| raw.len() <= 65_536)
                        );
                        held_list_fixture_counts[1] += 2;
                        assert_eq!(inspect_held.load(Ordering::SeqCst), 0);
                        assert_eq!(list_held.load(Ordering::SeqCst), 0);
                        for controls in [&inspect_controls, &list_controls] {
                            assert!(!controls.revoked.load(Ordering::SeqCst));
                            assert!(!controls.cancelled.load(Ordering::SeqCst));
                        }
                        assert_eq!(fs::read(&revision_path).unwrap(), revision_config_raw);
                        assert_eq!(
                            fs::read(isolated.path().join(&member_path)).unwrap(),
                            actual_current_raw
                        );
                        assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), v1_raw);
                        drop(inspect_authority);
                        drop(list_authority);
                        list_worker.finish(deadline, &cancellation).unwrap();
                        drop(list_worker);
                        recheck_inventory();
                        eprintln!(
                            "general manifest source=ORIGINAL case=metadata_N1 phase=complete full_old_rows=1 world_calls=0"
                        );
                    }
                }
                // END seven-context held-list fixture: ORIGINAL.
            }
            // Existing PublicProfile BASE semantic_content grant suffices for
            // version2. The old body remains an exact retained owner version.
            invocation["source_revision"] = serde_json::json!(current_revision.0.to_prefixed());
            invocation["original_source_revision"] =
                serde_json::json!(current_revision.0.to_prefixed());
            write_invocation(&invocation);
            let mut next_content = v1_record["semantic_content"].clone();
            if kind == "chronology-version" {
                next_content["system_as_of"]["value"] =
                    serde_json::from_slice(&bytes(&g12.as_ref().unwrap().2)).unwrap();
            } else {
                next_content["rules_version"] = serde_json::json!("synthetic-r2");
                next_content["predecessor_source_record_ref"] = v1_ref.clone();
            }
            let mut update = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-revise",
                "fields":{"semantic_content":next_content},"forms":[{"form_id":"tos.form.creation.fixture-name","field_id":"metadata.preferred-name"}],
                "reason":"Synthetic versioned temporal source metadata; no policy admission"});
            let prepared_update = alignment_native_cli(
                &repository,
                &revision_path,
                &invocation_path,
                &update,
                deadline,
            );
            update["operation"] = serde_json::json!("record.revise");
            update["command_id"] = serde_json::json!(format!("synthetic:{kind}-v2"));
            for (key, value) in [
                ("expected_configuration", "owner_configuration"),
                ("expected_source", "source"),
                ("expected_revision", "revision"),
                ("expected_dependencies", "expected_dependencies"),
            ] {
                update[key] = prepared_update["result"][value].clone();
            }
            let written = alignment_native_cli(
                &repository,
                &revision_path,
                &invocation_path,
                &update,
                deadline,
            );
            assert_eq!(written["result"]["source"]["version"], 2);
            let v2_raw = fs::read(isolated.path().join(relative)).unwrap();
            let v2_record: Value = serde_json::from_slice(&v2_raw).unwrap();
            assert_eq!(
                v2_record["semantic_content"],
                update["fields"]["semantic_content"]
            );
            assert_eq!(v2_record["record_version"], 2);
            assert_ne!(v2_raw, v1_raw);
            // Bind actual renderer output to the actual CAS publication epoch,
            // then capture V. This adds no fifth source cut.
            super::compiler_source_cases::publish_native_catalog_fixture_with_selected_schemas(
                &repository,
                isolated.path(),
                deadline,
                &selected_schema_files,
            );
            let revised_cut =
                super::command_form_cases::successor(&current_authored(), &store, current_revision);
            invocation["source_revision"] = serde_json::json!(revised_cut.0.to_prefixed());
            write_invocation(&invocation);
            let old = alignment_native_cli(
                &repository,
                &revision_path,
                &invocation_path,
                &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"inspect-version","source":v1_ref}),
                deadline,
            );
            assert_eq!(old["result"]["record"], v1_record);
            let basename = Path::new(relative).file_name().unwrap().to_str().unwrap();
            let binding = &old["result"]["files"][basename];
            let old_raw = fs::read(
                isolated
                    .path()
                    .join(binding["archive_path"].as_str().unwrap()),
            )
            .unwrap();
            assert_eq!(old_raw, v1_raw);
            assert_eq!(
                binding["sha256"],
                Digest256::of_bytes(&old_raw).to_prefixed()
            );
            if kind == "chronology-version" {
                let current_cut = open_cut(&store, revised_cut, deadline, &cancellation);
                let selected = with_software(&current_authored());
                let describe = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"});
                let mut read_context = cut_context(
                    &selected,
                    revision_config_raw.clone(),
                    canonical_json(&describe),
                    revised_cut,
                );
                read_context.effective_uid = u64::from(uid);
                let snapshot = SourceSnapshot {
                    reference: SourceSnapshotRef {
                        id: StableId::parse(snapshot_name).unwrap(),
                        revision,
                    },
                    current_cutoff: Cutoff {
                        value: g12.as_ref().unwrap().2.clone(),
                        trs: None,
                    },
                };
                let mut read_worker = super::command_form_cases::schemas_with_image(
                    &current_cut,
                    &image,
                    deadline,
                    &cancellation,
                );
                let loaded = load_chronology_from_owner(
                    &revision_fs,
                    &read_context,
                    &current_cut,
                    &exact_ref(&v1_ref),
                    &context,
                    &cut,
                    &snapshot,
                    &software,
                    &components,
                    &mut read_worker,
                    ChronologyLimits::default(),
                    deadline,
                    &cancellation,
                )
                .unwrap();
                loaded.recheck(deadline, &cancellation).unwrap();
                let member_packet = &loaded
                    .bound()
                    .evidence()
                    .object_get("members")
                    .unwrap()
                    .as_array()
                    .unwrap()[0];
                assert_eq!(
                    bytes(member_packet.object_get("record").unwrap()),
                    bytes(&parse(&canonical_json(&member)))
                );
                assert_eq!(loaded.bound().chronology().reference, exact_ref(&v1_ref));
                assert_eq!(
                    loaded
                        .bound()
                        .chronology()
                        .selection
                        .source_snapshot
                        .revision,
                    revision
                );
                assert_eq!(
                    loaded.bound().chronology().selection.system_as_of.value,
                    g12.as_ref().unwrap().1
                );
                assert_eq!(
                    v2_record["semantic_content"]["system_as_of"]["value"],
                    serde_json::from_slice::<Value>(&bytes(&g12.as_ref().unwrap().2)).unwrap()
                );
                assert_eq!(
                    loaded
                        .owner_evidence()
                        .object_get("source")
                        .unwrap()
                        .object_get("record_sha256")
                        .unwrap()
                        .as_str(),
                    Some(Digest256::of_bytes(&v1_raw).to_prefixed().as_str())
                );
                drop(loaded);
                read_worker.finish(deadline, &cancellation).unwrap();
            }
            if kind == "temporal-crosswalk" {
                use tos_foundation::temporal::{
                    TemporalBudget, stored_crosswalk::decode_stored_crosswalk_record,
                };
                let raw_ref = serde_json::json!({"path":relative,"sha256":Digest256::of_bytes(&old_raw).to_prefixed(),"size_bytes":old_raw.len()});
                let decode = serde_json::json!({"schema_version":"tos_stored_crosswalk_decode_request_v1",
                    "record":{"source_record_ref":v1_ref,"raw_member_ref":raw_ref,"source_revision":revised_cut.0.to_prefixed(),
                        "record_bytes":std::str::from_utf8(&old_raw).unwrap()},"requested_source_record_ref":v1_ref,
                    "requested_raw_member_ref":raw_ref,"expected_source_revision":revised_cut.0.to_prefixed()});
                let decoded = decode_stored_crosswalk_record(
                    &parse(&canonical_json(&decode)),
                    &parse(b"{}"),
                    TemporalBudget::default(),
                )
                .unwrap();
                assert_eq!(
                    decoded
                        .object_get("authentication")
                        .and_then(JsonValue::as_str),
                    Some("SuppliedBytesOnly")
                );
                assert_eq!(
                    decoded
                        .object_get("conversion_policy_admission")
                        .and_then(JsonValue::as_str),
                    Some("NotEstablished")
                );

                // Opt-in composition of existing owner read, shared Safe wire,
                // and Node outer JSON. This is not native CLI wire integration.
                if let Some(node) = std::env::var_os("TOS_INTEGER_WIRE_NODE") {
                    use tos_foundation::temporal::integer_wire::{
                        IntegerWireSafety, check_integer_wire_value, encode_integer_wire_response,
                    };
                    let budget = TemporalBudget::default();
                    let persisted = parse(&canonical_json(&old["result"]["record"]));
                    let packet =
                        encode_integer_wire_response("fixture.inspect-version", &persisted, budget)
                            .unwrap();
                    assert_eq!(
                        packet
                            .object_get("representation")
                            .and_then(JsonValue::as_str),
                        Some("OpaqueRawJson")
                    );
                    let raw = packet
                        .object_get("raw_json")
                        .and_then(JsonValue::as_str)
                        .unwrap();
                    assert!(raw.contains("9007199254740993"));
                    assert_eq!(
                        packet
                            .object_get("emitted_json_sha256")
                            .and_then(JsonValue::as_str),
                        Some(Digest256::of_bytes(raw.as_bytes()).to_prefixed().as_str())
                    );
                    let request = serde_json::json!({"schema_version":"tos_local_source_command_v1",
                        "operation":"inspect-version","source":v1_ref});
                    assert_eq!(request["source"]["version"], 1);
                    assert_eq!(
                        check_integer_wire_value(&parse(&canonical_json(&request)), budget)
                            .unwrap(),
                        IntegerWireSafety::Safe
                    );
                    let outer = serde_json::json!({"record":serde_json::from_slice::<Value>(&bytes(&packet)).unwrap(),"request":request});
                    let input = canonical_json(&outer);
                    assert!(input.len() <= 128 * 1024);
                    let exchange = tempfile::tempdir().unwrap();
                    let input_path = exchange.path().join("input.json");
                    let output_path = exchange.path().join("output.json");
                    fs::write(&input_path, input).unwrap();
                    // raw_json is deliberately NEVER parsed inside JavaScript.
                    let script = r#"const fs=require('fs'); const input=fs.readFileSync(0,'utf8');
if(Buffer.byteLength(input)>131072)process.exit(2);
const outer=JSON.parse(input);
if(typeof outer.record.raw_json!=='string'||outer.record.representation!=='OpaqueRawJson')process.exit(3);
const output=JSON.stringify(outer);
if(Buffer.byteLength(output)>131072)process.exit(4);
process.stdout.write(output);"#;
                    let mut child = std::process::Command::new(node)
                        .args(["-e", script])
                        .stdin(std::process::Stdio::from(
                            fs::File::open(&input_path).unwrap(),
                        ))
                        .stdout(std::process::Stdio::from(
                            fs::File::create(&output_path).unwrap(),
                        ))
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .unwrap();
                    let node_deadline = deadline.min(Instant::now() + Duration::from_secs(10));
                    loop {
                        if let Some(status) = child.try_wait().unwrap() {
                            assert!(status.success(), "Node outer JSON roundtrip failed");
                            break;
                        }
                        if Instant::now() >= node_deadline {
                            let _ = child.kill();
                            let _ = child.wait();
                            panic!("Node outer JSON roundtrip exceeded bounded deadline");
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    assert!(fs::metadata(&output_path).unwrap().len() <= 128 * 1024);
                    let returned: Value =
                        serde_json::from_slice(&fs::read(&output_path).unwrap()).unwrap();
                    let before: Value = serde_json::from_slice(&bytes(&packet)).unwrap();
                    assert_eq!(returned["record"]["raw_json"], before["raw_json"]);
                    assert_eq!(
                        returned["record"]["emitted_json_sha256"],
                        before["emitted_json_sha256"]
                    );
                    assert_eq!(returned["request"], request);
                    // Admit the entire returned request before the real owner call.
                    assert_eq!(
                        check_integer_wire_value(
                            &parse(&canonical_json(&returned["request"])),
                            budget
                        )
                        .unwrap(),
                        IntegerWireSafety::Safe
                    );
                    let repeated = alignment_native_cli(
                        &repository,
                        &revision_path,
                        &invocation_path,
                        &returned["request"],
                        deadline,
                    );
                    assert_eq!(repeated["result"]["record"], v1_record);
                    assert_eq!(repeated["result"]["source"], old["result"]["source"]);
                    let repeated_packet = encode_integer_wire_response(
                        "fixture.inspect-version",
                        &parse(&canonical_json(&repeated["result"]["record"])),
                        budget,
                    )
                    .unwrap();
                    assert_eq!(bytes(&repeated_packet), bytes(&packet));
                    assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), v2_raw);
                    assert_eq!(
                        fs::read(
                            isolated
                                .path()
                                .join(binding["archive_path"].as_str().unwrap())
                        )
                        .unwrap(),
                        v1_raw
                    );
                }
                // C01: independent post-R2 owner research use. All earlier
                // crosswalk/Node controls and the later G10 inventory stay intact.
                {
                    use std::io::{Read, Write};
                    use tos_command::{
                        C01ResearchBindingForConformance, C01ResearchFinalReadForConformance,
                        C01ResearchSessionForConformance, C01VisitDiagnosticForConformance,
                        C01VisitFaultForConformance, with_research_crosswalk_for_conformance,
                    };
                    use tos_foundation::{ExactRecordRef, RecordVersion, StableId};
                    assert!(Instant::now() < deadline);
                    assert!(v1_raw.len() <= 8192 && v2_raw.len() <= 8192);
                    assert_eq!(config["principal_id"], "software:test-fixture");
                    assert_eq!(fs::read(&owner).unwrap(), canonical_json(&revoked));
                    assert_eq!(old_raw, v1_raw);
                    assert_eq!(old["result"]["source"], written["result"]["source"]);
                    assert_eq!(old["result"]["inspected_source"], v1_ref);
                    assert_eq!(
                        v2_record["semantic_content"]["predecessor_source_record_ref"],
                        v1_ref
                    );
                    assert_eq!(
                        v2_record["semantic_content"]["rules_version"],
                        "synthetic-r2"
                    );
                    assert_eq!(
                        v1_record["semantic_content"]["rules_version"],
                        "synthetic-rule-r1"
                    );
                    assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), v2_raw);
                    let c01_archive = binding["archive_path"].as_str().unwrap();
                    assert!(c01_archive.len() <= 4096);
                    let predecessor_path = RelativePath::parse(c01_archive).unwrap();
                    assert_eq!(fs::read(isolated.path().join(c01_archive)).unwrap(), v1_raw);
                    let exact = |v: &Value| ExactRecordRef {
                        id: StableId::parse(v["id"].as_str().unwrap()).unwrap(),
                        version: RecordVersion::new(v["version"].as_u64().unwrap()).unwrap(),
                        digest: Digest256::from_prefixed(v["digest"].as_str().unwrap()).unwrap(),
                    };
                    let current_root = exact(&written["result"]["source"]);
                    let predecessor_root = exact(&v1_ref);
                    assert_eq!(current_root.version.get(), 2);
                    assert_eq!(predecessor_root.version.get(), 1);
                    assert_eq!(current_root.id, predecessor_root.id);

                    // V13.3: these are caller-retained fixture values and ordinary
                    // native inputs, not allocations paid by the adapter's private
                    // rows. Fixed serialization scratch cannot grow. Each local
                    // strict/canonical operation has its own finite state/work
                    // bound; adapter acquisition/copies remain privately prepaid.
                    struct C01Fixed {
                        raw: Vec<u8>,
                        cap: usize,
                    }
                    impl Write for C01Fixed {
                        fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
                            if input.len() > self.cap.saturating_sub(self.raw.len()) {
                                return Err(std::io::ErrorKind::WriteZero.into());
                            }
                            self.raw.extend_from_slice(input);
                            Ok(input.len())
                        }
                        fn flush(&mut self) -> std::io::Result<()> {
                            Ok(())
                        }
                    }
                    let encode = |value: &Value, cap: usize| {
                        assert!(cap > 0 && cap <= 32768);
                        let mut output = C01Fixed {
                            raw: Vec::with_capacity(cap),
                            cap,
                        };
                        assert_eq!(output.raw.capacity(), cap);
                        serde_json::to_writer(&mut output, value).unwrap();
                        assert!(output.raw.len() <= cap && output.raw.capacity() == cap);
                        output.raw
                    };
                    let strict = |raw: &[u8], cap: usize| {
                        assert!(cap <= 32768 && raw.len() <= cap);
                        tos_foundation::parse_json_with_state_budget(
                            raw,
                            JsonMode::PublishedStrict,
                            JsonLimits {
                                max_bytes: cap,
                                max_depth: 32,
                                max_visits: 32768,
                                max_integer_digits: 4096,
                            },
                            524288,
                        )
                        .unwrap()
                    };
                    let canonical = |value: &Value, cap: usize| {
                        let raw = encode(value, cap);
                        let document = strict(&raw, cap);
                        let canonical =
                            tos_foundation::canonical_bytes_v1_with_state_budget_and_visits(
                                document.root(),
                                CanonicalProfile::SourceCommandInputV1,
                                JsonLimits {
                                    max_bytes: cap,
                                    max_depth: 32,
                                    max_visits: 32768,
                                    max_integer_digits: 4096,
                                },
                                524288,
                            )
                            .unwrap()
                            .0;
                        assert!(canonical.len() <= cap);
                        canonical
                    };
                    let payload_ref = |id: &str, version: u64, payload: &Value| {
                        assert!(id.len() <= 256 && (1..=3).contains(&version));
                        let raw = canonical(payload, 16384);
                        serde_json::json!({"id":id,"version":version,"digest":Digest256::of_bytes(&raw).to_prefixed()})
                    };
                    // Pre-sized physical reads, then the exact authored hashes;
                    // the public source gains only these five unchanged schemas.
                    let additions = [
                        (
                            "owner-local-source-context.schema.json",
                            1353usize,
                            "4261950c0f2f48001498b0bee0cbe1ff4e702af45688090e8123f080b5c17127",
                        ),
                        (
                            "knowledge-assessment-batch.schema.json",
                            3566,
                            "66144d009da2268001b356ba4ef39da09dc408e43b1edceee9c6a3c17ea924ea",
                        ),
                        (
                            "knowledge-assessment-policy.schema.json",
                            2094,
                            "77d4956a4d9f6f5ea8a78a338a655d39957c9cbe0a5d425bab77ca3ad301b511",
                        ),
                        (
                            "knowledge-assessment-authority.schema.json",
                            2011,
                            "8ae72f8e24ffba3bec8c7f4ff0d3c56d90e9e4f33b4795aabe217df3dd5e50cd",
                        ),
                        (
                            "knowledge-assessment-competence.schema.json",
                            1586,
                            "b0f6466b7eb175b9c9b4f1e5f4df0f93c853d0ebc9acb5f3606fe052ede19cb1",
                        ),
                    ];
                    for (name, size, digest) in additions {
                        let path = repository.join("ToS/contracts").join(name);
                        assert!(fs::metadata(&path).unwrap().is_file());
                        assert_eq!(fs::metadata(&path).unwrap().len(), size as u64);
                        let mut raw = vec![0u8; size];
                        let mut file = fs::File::open(path).unwrap();
                        file.read_exact(&mut raw).unwrap();
                        assert_eq!(file.read(&mut [0u8; 1]).unwrap(), 0);
                        assert_eq!(Digest256::of_bytes(&raw).to_hex(), digest);
                        let target = isolated.path().join("ToS/contracts").join(name);
                        assert!(!target.exists());
                        fs::write(&target, &raw).unwrap();
                        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
                    }
                    assert_eq!(selected_schema_files.len(), 64);
                    let c01_authored = current_authored();
                    assert!(c01_authored.len() <= 512);
                    assert!(
                        c01_authored
                            .values()
                            .try_fold(0usize, |n, r| n.checked_add(r.len()))
                            .unwrap()
                            <= 16 * 1024 * 1024
                    );
                    assert_eq!(
                        c01_authored
                            .keys()
                            .filter(
                                |p| p.starts_with("ToS/contracts/") && p.ends_with(".schema.json")
                            )
                            .count(),
                        69 // 64 selected schemas plus the five exact pinned additions above.
                    );
                    assert_eq!(c01_authored[relative], v2_raw);
                    assert_eq!(c01_authored[c01_archive], v1_raw);
                    // Standalone full W, never a fifth ancestor or a relabelled V.
                    let w_revision =
                        super::validation_cut_cases::write_cut_store(&c01_authored, &store);
                    let w = open_cut(&store, w_revision, deadline, &cancellation);
                    assert_eq!(w.current().revision(), w_revision);
                    assert!(w.current().base_revision().is_none());
                    let primary_path = RelativePath::parse(relative).unwrap();
                    for (path, raw) in [(&primary_path, &v2_raw), (&predecessor_path, &v1_raw)] {
                        let member = w.current().member(path).unwrap();
                        assert_eq!(member.size_bytes, raw.len() as u64);
                        assert_eq!(member.sha256, Digest256::of_bytes(raw));
                        let held = w
                            .read_member(w_revision, path, 8192, deadline, &cancellation)
                            .unwrap();
                        assert_eq!(held.raw, *raw);
                    }
                    assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), v2_raw);
                    assert_eq!(fs::read(isolated.path().join(c01_archive)).unwrap(), v1_raw);

                    // P is exact unchanged policy-v3, not a new policy use.
                    let policy_path = repository
                        .join("ToS/doctrine/semantic-interchange/assessment-policy.v3.json");
                    assert_eq!(fs::metadata(&policy_path).unwrap().len(), 8424);
                    let mut policy_raw = vec![0u8; 8424];
                    let mut policy_file = fs::File::open(policy_path).unwrap();
                    policy_file.read_exact(&mut policy_raw).unwrap();
                    assert_eq!(policy_file.read(&mut [0u8; 1]).unwrap(), 0);
                    assert_eq!(
                        Digest256::of_bytes(&policy_raw).to_hex(),
                        "72fb4d4c7dda4c1af2e25a1568658037a720bb04ed13b348af81f033b4312bba"
                    );
                    let policy_document = strict(&policy_raw, 16384);
                    assert_eq!(
                        policy_document
                            .root()
                            .object_get("policy_version")
                            .unwrap()
                            .as_u64(),
                        Some(3)
                    );
                    // This serde input is the exact pinned8424-byte authored P,
                    // already strict/depth/state admitted, never arbitrary JSON.
                    let p_payload: Value = serde_json::from_slice(&policy_raw).unwrap();
                    let p_ref = payload_ref("tos.policy.knowledge-assessment", 3, &p_payload);
                    let p_envelope = serde_json::json!({"id":"tos.policy.knowledge-assessment","version":3,"payload":p_payload,"origin_id":null});
                    let x_payload = serde_json::json!({"procedure_ref":"fixture:finite-crosswalk-source-observation-v1","model_ref":"fixture:not-a-real-review-model","synthetic":true});
                    let x_ref =
                        payload_ref("tos.method.synthetic-c01-source-observation", 1, &x_payload);
                    let x_envelope = serde_json::json!({"id":"tos.method.synthetic-c01-source-observation","version":1,"payload":x_payload,"origin_id":null});
                    let k_payload = serde_json::json!({"kind":"synthetic-calibration-fixture","scope":"finite-mapping-v1 source-attributed research mechanics only","evidence":"Synthetic test issuer declaration, not real professional calibration."});
                    let k_ref = payload_ref("tos.review.synthetic-c01-calibration", 1, &k_payload);
                    let k_envelope = serde_json::json!({"id":"tos.review.synthetic-c01-calibration","version":1,"payload":k_payload,"origin_id":null});
                    let e_payload = serde_json::json!({"kind":"synthetic-source-report","subject":written["result"]["source"],"rule_profile":"finite-mapping-v1","rules_version":"synthetic-r2","source_trs":{"id":"synthetic-axis-a","version":"r1"},"target_trs":{"id":"synthetic-axis-b","version":"r2"},"applicability":{"min":10,"max":12,"unit":"step","origin":"explicit-zero"},"rows":{"10":100,"12":120},"gap":11,"source_status":"SyntheticSourceDeclarationNotHistoricalEvidence"});
                    let e_ref = payload_ref("tos.file.synthetic-c01-source-a", 1, &e_payload);
                    let e_envelope = serde_json::json!({"id":"tos.file.synthetic-c01-source-a","version":1,"payload":e_payload,"origin_id":"synthetic-independent-c01-source-a"});
                    let c_payload = serde_json::json!({"schema_version":"tos_knowledge_assessment_competence_v1","competence_id":"tos.competence.synthetic-c01-assessor","competence_version":1,"actor_id":"synthetic-c01-assessor","assertion_layers":["scholarly_report"],"languages":["en"],"profile_ids":["source-observation"],"execution_profiles":[x_ref],"state":"verified","valid_from":"2026-01-01T00:00:00Z","valid_until":"2099-01-01T00:00:00Z","evidence_refs":[k_ref],"issuer_ref":"fixture:synthetic-c01-test-issuer-not-real-competence"});
                    let c_ref = payload_ref("tos.competence.synthetic-c01-assessor", 1, &c_payload);
                    let c_envelope = serde_json::json!({"id":"tos.competence.synthetic-c01-assessor","version":1,"payload":c_payload,"origin_id":null});
                    let a_payload = serde_json::json!({"schema_version":"tos_knowledge_assessment_authority_v1","authority_id":"tos.authority.synthetic-c01-assessor","authority_version":1,"actor_id":"synthetic-c01-assessor","actor_kind":"agent","policy":p_ref,"profile_ids":["source-observation"],"assertion_layers":["scholarly_report"],"languages":["en"],"uses":["research"],"subject_prefixes":["tos.temporal-crosswalk.synthetic-native-create"],"decisions":["admit","withdraw"],"competence_refs":[c_ref],"independence_group":"synthetic-c01-independent-review-group","can_supersede_others":false,"state":"active","valid_from":"2026-01-01T00:00:00Z","valid_until":"2099-01-01T00:00:00Z","issuer_ref":"fixture:synthetic-c01-test-owner-not-production-grant"});
                    let a_ref = payload_ref("tos.authority.synthetic-c01-assessor", 1, &a_payload);
                    let a_envelope = serde_json::json!({"id":"tos.authority.synthetic-c01-assessor","version":1,"payload":a_payload,"origin_id":null});

                    let journal = temporary.path().join("c01-assessment-journal");
                    let private_root = temporary.path().join("c01-private-transport");
                    // Confidential files require their OWN immediate parent700;
                    // never change the shared outer temporary directory's mode.
                    let c01_owner_home = temporary.path().join("c01-owner");
                    for dir in [&journal, &private_root, &c01_owner_home] {
                        fs::create_dir(dir).unwrap();
                        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
                        let metadata = fs::metadata(dir).unwrap();
                        assert!(metadata.is_dir());
                        assert_eq!(metadata.uid(), uid);
                        assert_eq!(metadata.permissions().mode() & 0o7777, 0o700);
                    }
                    let c01_owner_path = c01_owner_home.join("c01-assessment-owner.json");
                    let c01_context_path = c01_owner_home.join("c01-owner-context.json");
                    let c01_invocation_path = c01_owner_home.join("c01-native-invocation.json");
                    // Bound dynamic caller text BEFORE serde copies it into the
                    // finite authored trees. Ordinary fixture ownership survives
                    // the borrowed adapter call; no private-row credit is claimed.
                    for path in [
                        isolated.path(),
                        journal.as_path(),
                        private_root.as_path(),
                        c01_owner_home.as_path(),
                        c01_owner_path.as_path(),
                        c01_context_path.as_path(),
                        c01_invocation_path.as_path(),
                    ] {
                        assert!(path.to_str().is_some_and(|p| p.len() <= 4096));
                    }
                    assert!(relative.len() <= 4096 && identity.len() <= 256);
                    let owner_context = serde_json::json!({"schema_version":"tos_owner_local_source_context_v1","store_id":"sid-cccccccccccccccccccccccccccccccc","public_root":isolated.path(),"private_root":private_root,"private_prefix":"ToS/source-witnesses/owner-local/sid-cccccccccccccccccccccccccccccccc/"});
                    let owner_scope = serde_json::json!({"record":written["result"]["source"],"assertion_layer":"scholarly_report","risk":"low","languages":["en"],"maker_id":"software:test-fixture","requested_use":"research","access_allowed":true});
                    let c01_config = serde_json::json!({"schema_version":"tos_local_assessment_owner_v2","uid":uid,"principal_id":"synthetic-c01-assessor","execution_profile":x_ref,"policy":p_envelope,"authorities":[a_envelope],"competencies":[c_envelope],"records":[x_envelope,k_envelope,e_envelope],"journal_directory":journal,"source_root":isolated.path(),"source_records":[{"path":relative,"record_id":identity,"origin_id":"synthetic-crosswalk-maker"}],"subjects":{(identity.as_str()):owner_scope}});
                    let c01_config_raw = encode(&c01_config, 32768);
                    let c01_context_raw = encode(&owner_context, 8192);
                    strict(&c01_config_raw, 32768);
                    strict(&c01_context_raw, 8192);
                    fs::write(&c01_owner_path, &c01_config_raw).unwrap();
                    fs::write(&c01_context_path, &c01_context_raw).unwrap();
                    for path in [&c01_owner_path, &c01_context_path] {
                        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
                        let metadata = fs::metadata(path).unwrap();
                        assert_eq!(metadata.uid(), uid);
                        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
                    }
                    // SAME native entry/image and actual W. Public-V2 ordinary
                    // run still requires the real nonnull protected sidecar.
                    let mut c01_invocation = invocation.clone();
                    c01_invocation["owner_config"] = serde_json::json!(c01_owner_path);
                    c01_invocation["owner_context"] = serde_json::json!(c01_context_path);
                    c01_invocation["assessment_schema_worker"] =
                        c01_invocation["schema_worker"].clone();
                    c01_invocation["source_revision"] =
                        serde_json::json!(w_revision.0.to_prefixed());
                    c01_invocation["original_source_revision"] =
                        serde_json::json!(w_revision.0.to_prefixed());
                    let c01_invocation_raw = encode(&c01_invocation, 16384);
                    fs::write(&c01_invocation_path, &c01_invocation_raw).unwrap();
                    fs::set_permissions(&c01_invocation_path, fs::Permissions::from_mode(0o600))
                        .unwrap();
                    let invocation_metadata = fs::metadata(&c01_invocation_path).unwrap();
                    assert_eq!(invocation_metadata.uid(), uid);
                    assert_eq!(invocation_metadata.permissions().mode() & 0o7777, 0o600);
                    let native_calls = std::cell::Cell::new(0usize);
                    let native_call = |request: &Value| {
                        assert!(Instant::now() < deadline);
                        assert!(native_calls.get() < 3);
                        native_calls.set(native_calls.get().checked_add(1).unwrap());
                        let result = alignment_native_cli(
                            &repository,
                            &c01_owner_path,
                            &c01_invocation_path,
                            request,
                            deadline,
                        );
                        assert_eq!(result["schema_version"], "tos_local_assessment_result_v1");
                        assert_eq!(result["authentication"], "local-unix-account");
                        assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), v2_raw);
                        assert_eq!(fs::read(isolated.path().join(c01_archive)).unwrap(), v1_raw);
                        result
                    };
                    let described = native_call(
                        &serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"describe","subject_id":identity}),
                    );
                    assert_eq!(
                        described["result"]["command_context"]["subject"],
                        written["result"]["source"]
                    );
                    assert_eq!(
                        described["result"]["command_context"]["grants_authority"],
                        false
                    );
                    assert!(described["result"]["revision"].is_null());
                    let snapshot = described["owner_snapshot"].as_str().unwrap();
                    Digest256::from_prefixed(snapshot).unwrap();

                    // Exact mechanical clock format from source_serialization::instant;
                    // same positive Unix/year guards, no trusted clock/permission claim.
                    // Fixed30-byte sink prevents format! growth; fresh String copies
                    // are exact30 ASCII under the pinned constructor premise.
                    let observe_clock = || {
                        struct Clock {
                            bytes: [u8; 30],
                            len: usize,
                        }
                        impl std::fmt::Write for Clock {
                            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                                if s.len() > 30usize.saturating_sub(self.len) {
                                    return Err(std::fmt::Error);
                                }
                                self.bytes[self.len..self.len + s.len()]
                                    .copy_from_slice(s.as_bytes());
                                self.len += s.len();
                                Ok(())
                            }
                        }
                        let t = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap();
                        let z = i64::try_from(t.as_secs() / 86400)
                            .unwrap()
                            .checked_add(719468)
                            .unwrap();
                        let era = z / 146097;
                        let doe = z - era * 146097;
                        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
                        let mut year = yoe + era * 400;
                        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
                        let mp = (5 * doy + 2) / 153;
                        let day = doy - (153 * mp + 2) / 5 + 1;
                        let month = mp + if mp < 10 { 3 } else { -9 };
                        year += i64::from(month <= 2);
                        assert!((1970..=9999).contains(&year));
                        let seconds = t.as_secs() % 86400;
                        let mut output = Clock {
                            bytes: [0; 30],
                            len: 0,
                        };
                        std::fmt::Write::write_fmt(
                            &mut output,
                            format_args!(
                                "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:09}Z",
                                seconds / 3600,
                                seconds / 60 % 60,
                                seconds % 60,
                                t.subsec_nanos()
                            ),
                        )
                        .unwrap();
                        assert_eq!(output.len, 30);
                        assert!(output.bytes.is_ascii());
                        String::from(std::str::from_utf8(&output.bytes).unwrap())
                    };
                    let method = serde_json::json!({"procedure_ref":"fixture:finite-crosswalk-source-observation-v1","invocation_ref":"fixture:synthetic-c01-owner-admission","model_ref":"fixture:not-a-real-review-model","execution_profile":x_ref});
                    let evidence = serde_json::json!([{"record":e_ref,"stance":"supports","locator":"Synthetic declared table rows 10->100 and 12->120; explicit gap at 11."}]);
                    let mut assessment = serde_json::json!({"schema_version":"tos_knowledge_assessment_v1","assessment_id":"tos.review.synthetic-c01-crosswalk-use-v2","subject":written["result"]["source"],"policy":p_ref,"profile_id":"source-observation","authority":a_ref,"competence":c_ref,"reviewer":{"actor_id":"synthetic-c01-assessor","kind":"agent"},"decision":"admit","rationale":"Synthetic source-attributed finite-rule research mechanics only; no historical, calendrical or production permission.","language":"en","evidence":evidence,"counterevidence_search":{"status":"searched","note":"Synthetic fixture search posture only; no real-world research represented."},"limits":[],"method":method,"issued_at":null,"supersedes":[]});
                    let issued_j1 = observe_clock();
                    assessment["issued_at"] = serde_json::json!(issued_j1);
                    let j1_request = serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"append","subject_id":identity,"expected_subject":written["result"]["source"],"expected_snapshot":snapshot,"expected_revision":described["result"]["revision"],"command_id":"synthetic-c01-j1","assessments":[assessment]});
                    assert!(encode(&j1_request, 8192).len() <= 8192);
                    let j1 = native_call(&j1_request);
                    assert_eq!(j1["owner_snapshot"], described["owner_snapshot"]);
                    assert_eq!(j1["result"]["replayed"], false);
                    let receipt1 = &j1["result"]["receipt"];
                    assert!(receipt1["previous_revision"].is_null());
                    assert_eq!(receipt1["events"].as_array().unwrap().len(), 1);
                    let actual_j1 = &receipt1["events"][0]["assessment"];
                    assert_eq!(actual_j1, &j1_request["assessments"][0]);
                    assert_eq!(actual_j1["subject"], written["result"]["source"]);
                    assert_eq!(actual_j1["policy"], p_ref);
                    assert_eq!(actual_j1["authority"], a_ref);
                    assert_eq!(actual_j1["competence"], c_ref);
                    assert_eq!(actual_j1["decision"], "admit");
                    assert!(
                        actual_j1["issued_at"].as_str().unwrap()
                            <= receipt1["recorded_at"].as_str().unwrap()
                    );
                    let actual_j1_ref =
                        payload_ref(actual_j1["assessment_id"].as_str().unwrap(), 1, actual_j1);
                    let j1_head =
                        Digest256::from_hex(j1["result"]["revision"].as_str().unwrap()).unwrap();
                    assert_eq!(j1["result"]["current_admission"]["status"], "admitted");
                    assert_eq!(j1["result"]["current_admission"]["can_use"], true);
                    assert_eq!(
                        j1["result"]["current_admission"]["subject"],
                        written["result"]["source"]
                    );
                    assert_eq!(j1["result"]["current_admission"]["policy"], p_ref);
                    assert_eq!(j1["result"]["current_admission"]["use"], "research");
                    assert_eq!(
                        j1["result"]["current_admission"]["limits"],
                        serde_json::json!([])
                    );
                    let current_request = serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"inspect","subject_id":identity,"expected_subject":written["result"]["source"],"expected_snapshot":snapshot});
                    let retained_request = serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"inspect","subject_id":identity,"expected_subject":v1_ref,"expected_snapshot":snapshot});
                    let current_raw = encode(&current_request, 8192);
                    let retained_raw = encode(&retained_request, 8192);
                    let conversion_value = serde_json::json!({"current_position":{"value":10,"trs_ref":{"id":"synthetic-axis-a","version":"r1"},"unit":"step","origin":"explicit-zero","resolution":"exact"},"pinned_position":{"value":10,"trs_ref":{"id":"synthetic-axis-a","version":"r1"},"unit":"step","origin":"explicit-zero","resolution":"exact"},"out_of_coverage_position":{"value":13,"trs_ref":{"id":"synthetic-axis-a","version":"r1"},"unit":"step","origin":"explicit-zero","resolution":"exact"},"target_trs":{"id":"synthetic-axis-b","version":"r2"}});
                    let conversion_raw = encode(&conversion_value, 8192);
                    let conversion = strict(&conversion_raw, 8192);
                    let intake = current_raw
                        .len()
                        .checked_add(retained_raw.len())
                        .and_then(|n| n.checked_add(conversion_raw.len()))
                        .and_then(|n| n.checked_add(c01_owner_path.as_os_str().len()))
                        .and_then(|n| n.checked_add(isolated.path().as_os_str().len()))
                        .and_then(|n| n.checked_add(c01_archive.len()))
                        .and_then(|n| n.checked_add(current_root.id.as_str().len()))
                        .and_then(|n| n.checked_add(predecessor_root.id.as_str().len()))
                        .unwrap();
                    assert!(intake <= 8192);
                    // Caller-retained oracles: fixed eleven small fragments,
                    // each bounded to 4096 serialized bytes, never a private
                    // consumer row or an output clone. Native source/raw/payload
                    // domains stay independent; no additional temporal call.
                    let flush_root_fragments = [
                        canonical(
                            &serde_json::json!({"current_source_ref":written["result"]["source"]}),
                            4096,
                        ),
                        canonical(
                            &serde_json::json!({"source_revision":w_revision.0.to_prefixed()}),
                            4096,
                        ),
                        canonical(&serde_json::json!({"policy_ref":p_ref}), 4096),
                        canonical(
                            &serde_json::json!({"journal_basis":{"head":j1_head.to_hex(),"assessment_refs":[actual_j1_ref]}}),
                            4096,
                        ),
                        canonical(
                            &serde_json::json!({"rule":{"source_trs":v2_record["semantic_content"]["source_trs"],"target_trs":v2_record["semantic_content"]["target_trs"],"unit":"step","origin":"explicit-zero","rules_version":"synthetic-r2"}}),
                            4096,
                        ),
                        canonical(
                            &serde_json::json!({"source_fixity":{"current":{"path":relative,"sha256":Digest256::of_bytes(&v2_raw).to_prefixed(),"size_bytes":v2_raw.len()},"predecessor":{"path":c01_archive,"sha256":Digest256::of_bytes(&v1_raw).to_prefixed(),"size_bytes":v1_raw.len()}}}),
                            4096,
                        ),
                    ];
                    let flush_current_ref = canonical(
                        &serde_json::json!({"source_ref":written["result"]["source"]}),
                        4096,
                    );
                    let flush_predecessor_ref =
                        canonical(&serde_json::json!({"source_ref":v1_ref}), 4096);
                    let flush_current_provenance = canonical(
                        &serde_json::json!({"provenance":v2_record["semantic_content"]["provenance"]}),
                        4096,
                    );
                    let flush_predecessor_provenance = canonical(
                        &serde_json::json!({"provenance":v1_record["semantic_content"]["provenance"]}),
                        4096,
                    );
                    let flush_position = canonical(
                        &serde_json::json!({"position":{"value":100,"trs_ref":v2_record["semantic_content"]["target_trs"]}}),
                        4096,
                    );
                    let mut positive = 0usize;
                    let mut flushed = 0usize;
                    let mut version_refusals = 0usize;
                    let mut withdrawal_refusals = 0usize;
                    let mut run=|session:&mut C01ResearchSessionForConformance<'_>| -> tos_command::source_command::SourceCommandResult<()> {
                        assert!(matches!(session.attempt_retained_as_current(),Err(SourceCommandError::Conflict(_))));
                        version_refusals+=1;
                        let mut callback=|view:&mut C01ResearchFinalReadForConformance<'_>| -> tos_command::source_command::SourceCommandResult<()> {
                            let mut flush=|raw:&[u8]| -> tos_command::source_command::SourceCommandResult<()> {
                                assert!(raw.len()<=65536);
                                // Borrowed fixed assertions on this closed canonical
                                // projection. No caller DOM, third wire buffer, or
                                // claimed credit from the second flush capacity.
                                let wire=std::str::from_utf8(raw).unwrap();
                                let contains=|part:&str,fragment:&[u8]| {
                                    let fragment=std::str::from_utf8(fragment).unwrap();
                                    assert!(fragment.starts_with('{')&&fragment.ends_with('}'));
                                    assert!(part.contains(&fragment[1..fragment.len()-1]));
                                };
                                assert!(wire.starts_with("{\"current\":{"));
                                assert!(wire.ends_with('}'));
                                assert!(wire.contains("\"schema_version\":\"tos_held_research_crosswalk_v1\""));
                                assert!(wire.contains("\"scope\":\"SyntheticSourceAttributedFiniteRuleResearch\""));
                                for fragment in &flush_root_fragments { contains(wire,fragment); }
                                // These root-only boundaries are fixed by the
                                // closed output constructor; preserved record_bytes
                                // are JSON strings and contain escaped quotes.
                                let current=wire.split_once(",\"current_source_ref\":").unwrap().0;
                                let ooc=wire.split_once(",\"out_of_coverage\":").unwrap().1.split_once(",\"pinned\":").unwrap().0;
                                let pinned=wire.split_once(",\"pinned\":").unwrap().1.split_once(",\"policy_ref\":").unwrap().0;
                                for (part,scope,source_fragment,provenance_fragment,rules) in [
                                    (current,"\"scope\":\"CurrentSyntheticResearch\"",&flush_current_ref,&flush_current_provenance,"\"rules_version\":\"synthetic-r2\""),
                                    (pinned,"\"scope\":\"PinnedSourceReproductionOnly\"",&flush_predecessor_ref,&flush_predecessor_provenance,"\"rules_version\":\"synthetic-rule-r1\""),
                                ] {
                                    contains(part,source_fragment);contains(part,provenance_fragment);contains(part,&flush_position);
                                    assert!(part.contains(scope));assert!(part.contains(rules));
                                    assert!(part.contains("\"status\":\"Unique\""));
                                    assert!(part.contains("\"precision_loss\":[]"));
                                    assert!(part.contains("\"persisted\":false"));
                                    assert!(part.contains("\"scope\":\"supplied-record-verification-only\""));
                                    assert!(part.contains("\"adapter_version\":\"tos-supplied-crosswalk-record-v1\""));
                                    assert!(part.contains("9007199254740993"));
                                }
                                contains(ooc,&flush_predecessor_ref);
                                assert!(ooc.contains("\"scope\":\"PinnedSourceOutOfCoverageOnly\""));
                                assert!(ooc.contains("\"status\":\"OutOfCoverage\""));
                                assert!(ooc.contains("\"reason\":\"CrosswalkApplicabilityExceeded\""));
                                assert!(ooc.contains("\"value\":13"));
                                assert!(ooc.contains("\"persisted\":false"));
                                assert!(ooc.contains("\"scope\":\"supplied-record-verification-only\""));
                                assert!(ooc.contains("\"adapter_version\":\"tos-supplied-crosswalk-record-v1\""));
                                for forbidden in ["\"configuration\":","\"configuration_raw\":","\"owner_evidence\":","\"private_root\":","\"principal_id\":","\"receipt\":"] { assert!(!wire.contains(forbidden)); }
                                flushed+=1;Ok(())
                            };
                            view.final_read(&mut flush)?;positive+=1;Ok(())
                        };
                        session.with_current_conversion(&mut callback)?;
                        assert_eq!(positive,1);assert_eq!(flushed,1);
                        // Real J2 publication is AFTER positive completion/FINAL,
                        // before the consumer opens its fresh second context.
                        let mut withdrawal=j1_request["assessments"][0].clone();
                        withdrawal["assessment_id"]=serde_json::json!("tos.review.synthetic-c01-crosswalk-use-v2-withdrawal");
                        withdrawal["decision"]=serde_json::json!("withdraw");
                        withdrawal["supersedes"]=serde_json::json!([actual_j1_ref]);
                        let issued_j2=observe_clock();withdrawal["issued_at"]=serde_json::json!(issued_j2);
                        let request=serde_json::json!({"schema_version":"tos_local_assessment_command_v1","operation":"append","subject_id":identity,"expected_subject":written["result"]["source"],"expected_snapshot":snapshot,"expected_revision":j1["result"]["revision"],"command_id":"synthetic-c01-j2","assessments":[withdrawal]});
                        assert!(encode(&request,8192).len()<=8192);
                        let j2=native_call(&request);assert_eq!(j2["owner_snapshot"],described["owner_snapshot"]);assert_eq!(j2["result"]["replayed"],false);
                        let receipt=&j2["result"]["receipt"];assert_eq!(receipt["previous_revision"],j1["result"]["revision"]);assert_eq!(receipt["events"].as_array().unwrap().len(),1);
                        let actual=&receipt["events"][0]["assessment"];assert_eq!(actual,&request["assessments"][0]);assert_eq!(actual["decision"],"withdraw");assert_eq!(actual["supersedes"],serde_json::json!([actual_j1_ref]));assert!(actual["issued_at"].as_str().unwrap()<=receipt["recorded_at"].as_str().unwrap());
                        let j2_head=Digest256::from_hex(j2["result"]["revision"].as_str().unwrap()).unwrap();assert_ne!(j2_head,j1_head);
                        assert_eq!(j2["result"]["current_admission"]["status"],"unreviewed");assert_eq!(j2["result"]["current_admission"]["can_use"],false);
                        assert!(matches!(session.attempt_after_withdrawal(j2_head),Err(SourceCommandError::Denied(_))));withdrawal_refusals+=1;Ok(())
                    };
                    // D0 ordinary caller renderer: p=0 is a transient sink,
                    // p=1 a normal-return copy of the retained first-visit slot.
                    // Delivery is lossy; missing output does not establish no fault.
                    fn c01_render_visit(fault: &C01VisitFaultForConformance, phase: u8) {
                        use std::mem::{align_of, size_of};
                        // BEFORE buffer construction: fault/Option copies,
                        // stderr handle and 32 usize-equivalent scalar/argument/
                        // guard/hex/source-unit scratch slots. No caller DOM.
                        if phase > 1
                            || size_of::<usize>() != 8
                            || size_of::<&C01VisitFaultForConformance>() != 8
                            || size_of::<&mut [u8; 128]>() != 8
                            || size_of::<&'static [u8]>() != 16
                            || size_of::<C01VisitFaultForConformance>() > 40
                            || size_of::<Option<C01VisitFaultForConformance>>() > 48
                            || align_of::<C01VisitFaultForConformance>() > 8
                            || align_of::<Option<C01VisitFaultForConformance>>() > 8
                        {
                            return;
                        }
                        let Some(outside) = size_of::<usize>()
                            .checked_mul(32)
                            .and_then(|n| n.checked_add(size_of::<C01VisitFaultForConformance>()))
                            .and_then(|n| {
                                n.checked_add(size_of::<Option<C01VisitFaultForConformance>>())
                            })
                            .and_then(|n| n.checked_add(size_of::<std::io::Stderr>()))
                        else {
                            return;
                        };
                        if outside > 384 || outside.checked_add(128).is_none_or(|n| n > 512) {
                            return;
                        }
                        fn text(
                            raw: &mut [u8; 128],
                            used: &mut usize,
                            value: &'static [u8],
                        ) -> bool {
                            let Some(end) = used.checked_add(value.len()) else {
                                return false;
                            };
                            if end > raw.len() {
                                return false;
                            }
                            raw[*used..end].copy_from_slice(value);
                            *used = end;
                            true
                        }
                        fn hex(
                            raw: &mut [u8; 128],
                            used: &mut usize,
                            value: u64,
                            digits: usize,
                        ) -> bool {
                            if !(1..=16).contains(&digits) {
                                return false;
                            }
                            for index in 0..digits {
                                if *used >= raw.len() {
                                    return false;
                                }
                                let shift = 4 * (digits - 1 - index);
                                raw[*used] = b"0123456789abcdef"[((value >> shift) & 15) as usize];
                                *used += 1;
                            }
                            true
                        }
                        let mut raw = [0u8; 128];
                        let mut used = 0usize;
                        // Eight fixed hex fields total 55 digits; nine literals
                        // total 41 ASCII bytes. Complete record is exactly 96 bytes.
                        if !(text(&mut raw, &mut used, b"C01VISIT p=")
                            && hex(&mut raw, &mut used, u64::from(phase), 1)
                            && text(&mut raw, &mut used, b" r=")
                            && hex(&mut raw, &mut used, u64::from(fault.row()), 2)
                            && text(&mut raw, &mut used, b" o=")
                            && hex(&mut raw, &mut used, u64::from(fault.ordinal()), 2)
                            && text(&mut raw, &mut used, b" rem=")
                            && hex(&mut raw, &mut used, fault.remaining() as u64, 16)
                            && text(&mut raw, &mut used, b" req=")
                            && hex(&mut raw, &mut used, fault.requested() as u64, 16)
                            && text(&mut raw, &mut used, b" u=")
                            && hex(&mut raw, &mut used, u64::from(fault.source_unit()), 2)
                            && text(&mut raw, &mut used, b" l=")
                            && hex(&mut raw, &mut used, u64::from(fault.line()), 8)
                            && text(&mut raw, &mut used, b" c=")
                            && hex(&mut raw, &mut used, u64::from(fault.column()), 8)
                            && text(&mut raw, &mut used, b" END\n"))
                        {
                            return;
                        }
                        if used != 96 {
                            return;
                        }
                        let mut stderr = std::io::stderr();
                        // ONE attempt only; IO error/short write cannot replace
                        // the original wrapper result. No write_all or retry.
                        let _ = std::io::Write::write(&mut stderr, &raw[..used]);
                    }
                    fn c01_visit_sink(fault: &C01VisitFaultForConformance) {
                        c01_render_visit(fault, 0);
                    }
                    let visit_diagnostic = C01VisitDiagnosticForConformance::new(c01_visit_sink);
                    let c01_result = with_research_crosswalk_for_conformance(
                        C01ResearchBindingForConformance {
                            configuration_path: &c01_owner_path,
                            public_root: isolated.path(),
                            cut: &w,
                            image: &image,
                            software: &software,
                            components: &components,
                            source_revision: w_revision,
                            current_root: &current_root,
                            predecessor_root: &predecessor_root,
                            predecessor_path: &predecessor_path,
                            current_request_raw: &current_raw,
                            retained_request_raw: &retained_raw,
                            expected_j1_head: j1_head,
                            conversion: conversion.root(),
                            visit_diagnostic: Some(&visit_diagnostic),
                        },
                        deadline,
                        &cancellation,
                        &mut run,
                    );
                    if let Some(fault) = visit_diagnostic.first_visit_fault() {
                        c01_render_visit(&fault, 1);
                    }
                    c01_result.unwrap();
                    assert_eq!(
                        (
                            native_calls.get(),
                            version_refusals,
                            positive,
                            flushed,
                            withdrawal_refusals
                        ),
                        (3, 1, 1, 1, 1)
                    );
                    assert_eq!(fs::read(&c01_owner_path).unwrap(), c01_config_raw);
                    assert_eq!(fs::read(&c01_context_path).unwrap(), c01_context_raw);
                    assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), v2_raw);
                    assert_eq!(fs::read(isolated.path().join(c01_archive)).unwrap(), v1_raw);
                    assert_eq!(selected_schema_files.len(), 64); // later G10 closure unchanged
                    eprintln!(
                        "C01 source=actual-R2 phase=complete native_calls=3 contexts=2 pure_calls=6 current=J1 pinned=R1 out_of_coverage=13 withdrawal=J2"
                    );
                }
            }
        }
    }
    // Independent supported-source proof, after every original family holder
    // and model has ended. The GregorianProleptic source/refusal above is intact.
    {
        use super::command_text_cases::{
            alignment_image_digest, alignment_native_cli, authored_text_files,
        };
        use super::selected_metadata_authority::{Authority as TemporalAuthority, Controls};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tos_command::temporal_chronology::{ChronologyLimits, load_chronology_from_owner};
        use tos_command::temporal_claim_relation_binding::{
            HeldClaimComparisonBudget, HeldClaimComparisonError,
            with_loaded_chronology_envelope_comparison,
        };
        use tos_foundation::query_context::{Cutoff, SourceSnapshot, SourceSnapshotRef};
        use tos_foundation::{ExactRecordRef, RecordVersion, StableId};
        let exact_ref = |v: &Value| ExactRecordRef {
            id: StableId::parse(v["id"].as_str().unwrap()).unwrap(),
            version: RecordVersion::new(v["version"].as_u64().unwrap()).unwrap(),
            digest: Digest256::from_prefixed(v["digest"].as_str().unwrap()).unwrap(),
        };
        assert!(Instant::now() < deadline);
        assert!(
            files.len() <= 512 && files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024
        );
        let mut files = files.clone();
        let member_path =
            "ToS/source-witnesses/relations/synthetic-supported-calendar/source-claims.jsonl"
                .to_owned();
        let evidence = "ToS/research-packets/foundation-laboratory-2026-07/SYNTHETIC_SUPPORTED_NATIVE_CALENDAR.md";
        let subject = "tos.historical-event.synthetic.supported-native-calendar";
        let member = serde_json::json!({"schema_version":"tos_source_temporal_claim_v1",
            "claim_id":"tos.claim.synthetic.supported-native-calendar","claim_type":"relation","claim_version":1,
            "assertion_layer":"scholarly_report","subject_ref":subject,"predicate":"historical_dating",
            "object":{"kind":"date-assertion","role":"historical-time","calendar":"proleptic-gregorian",
                "year_numbering":"astronomical","certainty":"exact","value":"1945-04-02",
                "source_wording":{"language":"en","text":"Explicit NEW synthetic supported native calendar source: 1945-04-02; not a historical witness or calendar alias."}},
            "maker":{"maker_type":"model","agent_ref":"model:codex"},"evidence_refs":[evidence],
            "provenance_event_ref":"tos.event.source-creation.synthetic-supported-native-calendar",
            "epistemic_status":"reported","review_status":"unreviewed","visibility":"public_metadata_only",
            "qualifiers":{"statement":"Independent synthetic source for native envelope custody; no event equality or admission.","statement_language":"en","statement_script":"Latn"}});
        let event = serde_json::json!({"schema_version":"tos_historical_record_v1","record_type":"historical-event",
            "record_id":subject,"record_version":1,"preferred_label":"Synthetic supported native calendar source",
            "identity_status":"provisional","source_refs":[evidence],"external_identifiers":[],
            "same_as_posture":"no_equivalence_claim","visibility":"public_metadata_only"});
        files.insert(
            "ToS/source-witnesses/history/synthetic-supported-calendar/historical-event.json"
                .into(),
            canonical_json(&event),
        );
        files.insert(evidence.into(), b"Explicit NEW synthetic supported-calendar source only. No historical truth, original60 annotation or GregorianProleptic alias conversion.\n".to_vec());
        // BEGIN accepted NEW supplemental source assertion (not an inferred date).
        let member = {
            let event_path =
                "ToS/source-witnesses/history/synthetic-supported-calendar/historical-event.json";
            let event_raw = files.get(event_path).unwrap();
            assert!(event_raw.len() <= 2048);
            let raw_event_utf8 = std::str::from_utf8(event_raw).unwrap();
            let event_ref_value: Value = serde_json::from_slice(&bytes(
                &tos_command::source_forms::metadata_subject(&parse(event_raw)).unwrap(),
            ))
            .unwrap();
            let event_ref = exact_ref(&event_ref_value);
            assert_eq!(event_ref.id.as_str(), event["record_id"].as_str().unwrap());
            assert_eq!(
                event_ref.version.get(),
                event["record_version"].as_u64().unwrap()
            );
            assert!(event_ref.id.as_str().len() <= 1024);
            let nested_ref = serde_json::json!({"id":event_ref.id.as_str(),
                "version":event_ref.version.get(),"digest":event_ref.digest.to_hex()});
            let packet = serde_json::json!({"schema_version":"tos_temporal_claim_carrier_v1",
                "roles":[{"role":"WorldOccurrence","assertion":{
                    "profile_ref":"tos_world_civil_year_occurrence_v1",
                    "category":"OccurrencePossibleTimeBucket","coverage_rule":"GregorianCivilYearHalfOpenV1",
                    "about_ref":nested_ref,"position":{"type":"CalendarPosition","lexical":"1945",
                        "resolution":"year","trs":{"id":"tos.trs.synthetic.held-list-civil-year-v1","version":"1"},
                        "calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical",
                        "rules_version":"synthetic-gregorian-civil-year-r1"}},
                    "basis":{"source":nested_ref,"source_locator":event_path,"raw_evidence":{
                        "raw_event_utf8":raw_event_utf8,"raw_event_sha256":Digest256::of_bytes(event_raw).to_prefixed(),
                        "synthetic_declaration":"NEW synthetic fixture source assertion: the explicitly referenced event has a possible occurrence location within civil year 1945 under the declared rules. This is not a historical witness, event duration, creation time, or quotation inferred from the event identity record.",
                        "declaration_origin":"new supported-calendar Claim fixture author; event bytes establish identity/context only"}}}]});
            // All copied strings are already bounded above; no source packet or
            // whole-record echo is manufactured after its owner digest exists.
            let packet_raw = canonical_json(&packet);
            assert!(packet_raw.len() <= 4096);
            let mut augmented = member;
            augmented["qualifiers"]["temporal_claim_carrier"] = packet;
            augmented
        };
        // END accepted NEW supplemental source assertion.

        // Pre-admit the extra fixed authored record/ref/stream/locator containers
        // and bounded authoring overlap from the SAME 16MiB consumer allowance.
        // This is logical fixture workspace, not a new cap or an RSS claim.
        let mut b_fixture_retained_bytes = 1_048_576usize;
        let mut b_fixture_retained_visits = 8192usize;
        assert!(b_fixture_retained_bytes < 16 * 1024 * 1024);
        assert!(canonical_json(&member).len() <= 16_384);
        let mut second_member = member.clone();
        second_member["claim_id"] =
            serde_json::json!("tos.claim.synthetic.supported-native-calendar-b");
        second_member["object"]["value"] = serde_json::json!("1946-04-02");
        second_member["object"]["source_wording"]["text"] = serde_json::json!(
            "Explicit NEW synthetic second supported calendar assertion: 1946-04-02; separate report, not reconciliation or event truth."
        );
        let second_role = &mut second_member["qualifiers"]["temporal_claim_carrier"]["roles"][0];
        second_role["assertion"]["position"]["lexical"] = serde_json::json!("1946");
        second_role["basis"]["raw_evidence"]["synthetic_declaration"] = serde_json::json!(
            "NEW synthetic fixture source assertion: the explicitly referenced event has a possible occurrence location within civil year 1946 under the declared rules. Not event truth, duration or inference from its identity record."
        );
        assert!(canonical_json(&second_member).len() <= 16_384);
        assert_ne!(member["claim_id"], second_member["claim_id"]);

        assert!(
            files.len() <= 512 && files.values().map(Vec::len).sum::<usize>() <= 16 * 1024 * 1024
        );
        let isolated =
            IsolatedCreationRoot::create(temporary.path(), deadline, &cancellation).unwrap();
        for (path, raw) in &files {
            let target = isolated.path().join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, raw).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let uid = fs::metadata(isolated.path()).unwrap().uid();
        let store = temporary.path().join("supported-positive-store");
        // The parent may exist; native NOREPLACE publication owns the new home.
        fs::create_dir_all(
            isolated
                .path()
                .join(&member_path)
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .unwrap();
        let bp = super::validation_cut_cases::write_cut_store(
            &authored_text_files(isolated.path()),
            &store,
        );
        let owner = isolated.path().join("supported-claim-create-owner.json");
        let config = serde_json::json!({"schema_version":"tos_local_claim_create_owner_v2",
            "uid":uid,"principal_id":member["maker"]["agent_ref"],"maker_type":member["maker"]["maker_type"],
            "source_root":isolated.path(),"source_path":member_path,"authority_ref":"synthetic-test-only:supported-calendar-creation",
            "expires_at":"2099-01-01T00:00:00Z","provenance_event_id":member["provenance_event_ref"],
            "allowed_operations":["claims.create"],"allowed_claim_ids":[member["claim_id"],second_member["claim_id"]],"allowed_subject_refs":[subject],
            "allowed_object_refs":[],"allowed_object_values":[member["object"],second_member["object"]],"allowed_predicates":["historical_dating"],"allowed_evidence_refs":[evidence]});
        fs::write(&owner, canonical_json(&config)).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let native = PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        );
        let schema_worker = PathBuf::from(
            std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("schema worker required"),
        );
        let invocation_path = isolated
            .path()
            .join("supported-claim-create-invocation.json");
        let invocation = serde_json::json!({"schema_version":"tos_local_native_claim_invocation_v1",
            "owner_config":owner,"native_executable":native,"native_executable_sha256":alignment_image_digest(&native).to_prefixed(),
            "corpus_store":store,"source_revision":bp.0.to_prefixed(),"original_source_revision":bp.0.to_prefixed(),
            "software_capture":captured.capture,"software_restored_root":captured.restored,
            "software_selection":{"source_git_commit":captured.selection.source_git_commit,"source_git_tree":captured.selection.source_git_tree,
                "capture_manifest_sha256":captured.selection.capture_manifest_sha256.to_prefixed()},
            "software_components":components.members().map(|m|m.path.as_str()).collect::<Vec<_>>(),
            "schema_worker":{"absolute_path":schema_worker,"sha256":alignment_image_digest(&schema_worker).to_prefixed()},
            "budgets":{"max_revisions":4,"max_members":2048,"max_total_bytes":33554432,"max_member_bytes":8388608,
                "max_schema_receipts":128,"max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,"worker_address_space_bytes":1073741824}});
        fs::write(&invocation_path, canonical_json(&invocation)).unwrap();
        fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut create = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","claims":[member,second_member]});
        eprintln!("temporal positive phase=claim-prepare-create input=Bp");
        let preview =
            alignment_native_cli(&repository, &owner, &invocation_path, &create, deadline);
        assert_eq!(preview["grants_admission"], false);
        create["operation"] = serde_json::json!("claims.create");
        create["command_id"] = serde_json::json!("synthetic:supported-calendar-create");
        create["expected_configuration"] = preview["result"]["owner_configuration"].clone();
        create["expected_revision"] = Value::Null;
        create["expected_dependencies"] = preview["result"]["expected_dependencies"].clone();
        create["expected_inputs"] = preview["result"]["source_bindings"].clone();
        let created =
            alignment_native_cli(&repository, &owner, &invocation_path, &create, deadline);
        assert_eq!(created["result"]["replayed"], false);
        assert_eq!(created["result"]["receipt"]["grants_admission"], false);
        let source_home = Path::new(&member_path).parent().unwrap();
        let mut claim_files = BTreeMap::new();
        let mut claim_bytes = 0u64;
        for entry in fs::read_dir(isolated.path().join(source_home)).unwrap() {
            assert!(claim_files.len() < 5);
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            let size = entry.metadata().unwrap().len();
            assert!(size <= 8_388_608);
            claim_bytes = claim_bytes.checked_add(size).unwrap();
            assert!(claim_bytes <= 33_554_432);
            let raw = fs::read(entry.path()).unwrap();
            assert_eq!(raw.len() as u64, size);
            assert!(
                claim_files
                    .insert(entry.file_name().into_string().unwrap(), raw)
                    .is_none()
            );
        }
        assert_eq!(
            claim_files
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "source-claims.jsonl",
                "source-create-request.json",
                "source-create-environment.json",
                "source-create-provenance.jsonl",
                "source-create-receipt.json"
            ])
        );
        let claim_receipt: Value =
            serde_json::from_slice(&claim_files["source-create-receipt.json"]).unwrap();
        assert_eq!(claim_receipt, created["result"]["receipt"]);
        assert_eq!(
            serde_json::from_slice::<Value>(&claim_files["source-create-request.json"]).unwrap(),
            create
        );
        let member_raw = claim_files["source-claims.jsonl"].clone();
        assert!(member_raw.len() <= 131_072 && member_raw.last() == Some(&b'\n'));
        let stream_sha = Digest256::of_bytes(&member_raw);
        let mut actual_rows = member_raw.split(|byte| *byte == b'\n');
        let member_row_raw = actual_rows.next().unwrap();
        let second_row_raw = actual_rows.next().unwrap();
        assert_eq!(actual_rows.next(), Some(&b""[..]));
        assert!(actual_rows.next().is_none());
        assert!(!member_row_raw.is_empty() && !second_row_raw.is_empty());
        assert!(member_row_raw.len() <= 16_384 && second_row_raw.len() <= 16_384);
        // Borrow exact line bytes; never duplicate the whole stream or treat its
        // SHA as a record reference. State for the extra retained parse is prepaid.
        let row_limits = JsonLimits {
            max_bytes: 65_536,
            max_depth: 64,
            max_visits: 8192,
            max_integer_digits: 4096,
        };
        let row_state = |raw: &[u8]| {
            raw.len()
                .checked_mul(128)
                .and_then(|n| n.checked_add(4096))
                .unwrap()
        };
        let first_row_state = row_state(member_row_raw);
        let second_row_state = row_state(second_row_raw);
        assert!(first_row_state <= 4 * 1024 * 1024 && second_row_state <= 4 * 1024 * 1024);
        b_fixture_retained_bytes = b_fixture_retained_bytes
            .checked_add(first_row_state)
            .and_then(|n| n.checked_add(second_row_state))
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<tos_foundation::JsonDocument>()))
            .unwrap();
        b_fixture_retained_visits = b_fixture_retained_visits.checked_add(2 * 8192).unwrap();
        assert!(
            b_fixture_retained_bytes < 16 * 1024 * 1024 && b_fixture_retained_visits < 1_000_000
        );
        let first_owner_document = tos_foundation::parse_json_with_state_budget(
            member_row_raw,
            JsonMode::PublishedStrict,
            row_limits,
            first_row_state,
        )
        .unwrap();
        let second_owner_document = tos_foundation::parse_json_with_state_budget(
            second_row_raw,
            JsonMode::PublishedStrict,
            row_limits,
            second_row_state,
        )
        .unwrap();
        let member: Value = serde_json::from_slice(member_row_raw).unwrap();
        assert_eq!(claim_receipt["claims"].as_array().unwrap().len(), 2);
        let member_ref = claim_receipt["claims"][0].clone();
        let second_member_ref = claim_receipt["claims"][1].clone();
        assert_ne!(member_ref["id"], second_member_ref["id"]);
        for (i, row) in [first_owner_document.root(), second_owner_document.root()]
            .into_iter()
            .enumerate()
        {
            assert_eq!(row.object_get("claim_version").unwrap().as_u64(), Some(1));
            assert_eq!(
                bytes(row),
                bytes(&parse(&canonical_json(&create["claims"][i])))
            );
            assert_eq!(
                bytes(&parse(&canonical_json(&claim_receipt["claims"][i]))),
                bytes(&tos_command::source_forms::metadata_subject(row).unwrap())
            );
        }
        assert_eq!(
            bytes(second_owner_document.root()),
            bytes(&parse(&canonical_json(&second_member)))
        );
        assert_eq!(
            fs::read(isolated.path().join(&member_path)).unwrap(),
            member_raw
        );
        assert_eq!(Digest256::of_bytes(&member_raw), stream_sha);
        // G10 source witnesses: fixed prepaid construction window from the SAME
        // B parent, before new buffers/definition/ref/config allocations. Targets
        // and their writer receipt remain byte-exact and are never revised.
        b_fixture_retained_bytes = b_fixture_retained_bytes.checked_add(1_048_576).unwrap();
        b_fixture_retained_visits = b_fixture_retained_visits.checked_add(8192).unwrap();
        assert!(b_fixture_retained_bytes + 4 * 1024 * 1024 + 1_048_576 < 16 * 1024 * 1024);
        assert!(b_fixture_retained_visits + 250_000 + 32_768 < 1_000_000);
        // Fixed finite literal/config/reference/container construction workspace;
        // parser/buffer/callback reservations debit the rest of this SAME child.
        // Initial child is already in Rb/Rv above. Any actual checked excess
        // is withheld from the SAME parent, never an independent child/default.
        const G10_FIXED_BYTES: usize =
            4 * 1024 * 1024 + 1_048_576 + 5 * 131080 + 5 * 4096 + 69632 + 262144;
        const G10_FIXED_VISITS: usize = 250_000 + 32_768;
        let g10_used_bytes = std::cell::Cell::new(0usize);
        let g10_used_visits = std::cell::Cell::new(0usize);
        let g10_applied_bytes = std::cell::Cell::new(0usize);
        let g10_applied_visits = std::cell::Cell::new(0usize);
        let g10_byte_limit = std::cell::Cell::new(
            (16 * 1024 * 1024usize)
                .checked_sub(b_fixture_retained_bytes)
                .and_then(|n| n.checked_sub(G10_FIXED_BYTES))
                .and_then(|n| n.checked_add(1_048_576))
                .unwrap(),
        );
        let g10_visit_limit = std::cell::Cell::new(
            1_000_000usize
                .checked_sub(b_fixture_retained_visits)
                .and_then(|n| n.checked_sub(G10_FIXED_VISITS))
                .and_then(|n| n.checked_add(8192))
                .unwrap(),
        );
        let g10_charge = |bytes: usize| {
            let next = g10_used_bytes.get().checked_add(bytes).unwrap();
            assert!(
                next <= g10_byte_limit.get(),
                "G10 same-parent byte admission exhausted"
            );
            g10_used_bytes.set(next);
        };
        let g10_visit = |visits: usize| {
            let next = g10_used_visits.get().checked_add(visits).unwrap();
            assert!(
                next <= g10_visit_limit.get(),
                "G10 same-parent visit admission exhausted"
            );
            g10_used_visits.set(next);
        };
        g10_charge(262_144);
        g10_visit(1024);
        // Returned native owner state has its EXISTING delegated bounds. Every
        // adapter-owned clone of its variable preview is admitted independently
        // BEFORE cloning; no post-hoc preview size is a peak-memory promise.
        fn g10_clone_forecast(value: &Value, depth: usize, visits: &impl Fn(usize)) -> usize {
            assert!(depth <= 64);
            visits(1);
            let mut bytes = 128usize;
            match value {
                Value::String(text) => {
                    bytes = bytes
                        .checked_add(text.len().checked_mul(4).unwrap())
                        .unwrap()
                }
                Value::Array(items) => {
                    bytes = bytes.checked_add(512).unwrap();
                    for item in items {
                        bytes = bytes
                            .checked_add(g10_clone_forecast(item, depth + 1, visits))
                            .unwrap();
                    }
                }
                Value::Object(fields) => {
                    bytes = bytes.checked_add(512).unwrap();
                    for (key, item) in fields {
                        visits(1);
                        bytes = bytes
                            .checked_add(key.len().checked_mul(4).unwrap())
                            .and_then(|b| {
                                b.checked_add(g10_clone_forecast(item, depth + 1, visits))
                            })
                            .unwrap();
                    }
                }
                _ => {}
            }
            bytes
        }

        fn g10_command_forecast(value: &JsonValue, depth: usize, visits: &impl Fn(usize)) -> usize {
            assert!(depth <= 64);
            visits(1);
            let mut bytes = 128usize;
            match value {
                JsonValue::String(text) => {
                    bytes = bytes
                        .checked_add(text.units().len().checked_mul(8).unwrap())
                        .unwrap()
                }
                JsonValue::Number(number) => {
                    bytes = bytes
                        .checked_add(number.lexeme.len().checked_mul(4).unwrap())
                        .unwrap()
                }
                JsonValue::Array(items) => {
                    bytes = bytes.checked_add(512).unwrap();
                    for item in items {
                        bytes = bytes
                            .checked_add(g10_command_forecast(item, depth + 1, visits))
                            .unwrap();
                    }
                }
                JsonValue::Object(fields) => {
                    bytes = bytes.checked_add(512).unwrap();
                    for (key, item) in fields {
                        visits(1);
                        bytes = bytes
                            .checked_add(key.units().len().checked_mul(8).unwrap())
                            .and_then(|n| {
                                n.checked_add(g10_command_forecast(item, depth + 1, visits))
                            })
                            .unwrap();
                    }
                }
                _ => {}
            }
            bytes
        }

        // Only the two comparison scratch buffers use this borrowed serializer.
        // Their strict parsed values are compared in SourceCommandInputV1 below;
        // scratch insertion order is not a persisted/canonical byte contract.
        fn g10_scratch(value: &Value, charge: &impl Fn(usize), visits: &impl Fn(usize)) -> Vec<u8> {
            fn text_work(text: &str, visits: &impl Fn(usize)) {
                // Both passes: six escaped bytes per UTF-8 byte plus quotes.
                visits(
                    text.len()
                        .checked_mul(6)
                        .and_then(|n| n.checked_add(2))
                        .and_then(|n| n.checked_mul(2))
                        .unwrap(),
                );
            }
            fn prewalk(value: &Value, depth: usize, visits: &impl Fn(usize)) {
                assert!(depth <= 64);
                // Prewalk plus BOTH serializer node visits and punctuation.
                visits(35);
                match value {
                    Value::String(text) => text_work(text, visits),
                    Value::Number(number) => {
                        // Workspace enables arbitrary_precision; borrow its
                        // actual lexical storage rather than formatting a copy.
                        visits(number.as_str().len().checked_mul(2).unwrap());
                    }
                    Value::Array(items) => {
                        for item in items {
                            prewalk(item, depth + 1, visits);
                        }
                    }
                    Value::Object(fields) => {
                        for (key, item) in fields {
                            visits(7); // prewalk/key visits plus colon/comma
                            text_work(key, visits);
                            prewalk(item, depth + 1, visits);
                        }
                    }
                    _ => {}
                }
            }
            struct Counter {
                len: usize,
            }
            impl std::io::Write for Counter {
                fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                    let next = self
                        .len
                        .checked_add(bytes.len())
                        .filter(|n| *n < 131072)
                        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
                    self.len = next;
                    Ok(bytes.len())
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            struct Fixed<'a> {
                output: &'a mut [u8],
                at: usize,
            }
            impl std::io::Write for Fixed<'_> {
                fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                    let end = self
                        .at
                        .checked_add(bytes.len())
                        .filter(|n| *n <= self.output.len())
                        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
                    self.output[self.at..end].copy_from_slice(bytes);
                    self.at = end;
                    Ok(bytes.len())
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            // Same cumulative parent, no credit for avoided clones/sorting.
            charge(4096);
            assert!(
                std::mem::size_of::<Counter>()
                    + std::mem::size_of::<Fixed<'_>>()
                    + std::mem::size_of::<serde_json::Serializer<Counter>>()
                    + std::mem::size_of::<serde_json::Serializer<Fixed<'_>>>()
                    <= 4096
            );
            prewalk(value, 0, visits);
            let mut counter = Counter { len: 0 };
            serde_json::to_writer(&mut counter, value).unwrap();
            let total = counter.len.checked_add(1).unwrap();
            assert!(total <= 131072);
            charge(total); // BEFORE the exact-length zeroed buffer allocation
            let mut raw = vec![0u8; total];
            {
                let mut writer = Fixed {
                    output: &mut raw[..counter.len],
                    at: 0,
                };
                serde_json::to_writer(&mut writer, value).unwrap();
                assert_eq!(writer.at, counter.len);
            }
            raw[counter.len] = b'\n';
            assert_eq!(raw.len(), total);
            assert_eq!(raw.capacity(), total);
            raw
        }

        let g10_kd = serde_json::json!({
            "schema_version": "tos_source_declared_knower_v1",
            "record_id": "tos.knower.synthetic.g10.school-a",
            "record_version": 1,
            "kind": "SourceDeclaredKnower",
            "label": "Synthetic school A perspective"
        });
        let g10_rd = serde_json::json!({
            "schema_version": "tos_source_trs_definition_v1",
            "record_id": "tos.trs-definition.synthetic.g10.civil-year",
            "record_version": 1,
            "trs": {
                "id": "tos.trs.synthetic.held-list-civil-year-v1",
                "version": "1"
            },
            "calendar_algorithm": "GregorianProleptic",
            "year_numbering": "astronomical",
            "rules_version": "synthetic-gregorian-civil-year-r1",
            "unit": "civil-year",
            "origin": "astronomical-year-number"
        });
        let g10_event = serde_json::json!({
            "schema_version": "tos_historical_record_v1",
            "record_type": "historical-event",
            "record_id": "tos.historical-event.synthetic.g10-source",
            "record_version": 1,
            "preferred_label": "Explicit synthetic G10 source context",
            "identity_status": "provisional",
            "source_refs": [
                "ToS/research-packets/foundation-laboratory-2026-07/SYNTHETIC_G10_KNOWLEDGE.md"
            ],
            "external_identifiers": [],
            "same_as_posture": "no_equivalence_claim",
            "visibility": "public_metadata_only"
        });
        let g10_event_path = "ToS/source-witnesses/history/synthetic-g10/historical-event.json";
        let g10_evidence =
            "ToS/research-packets/foundation-laboratory-2026-07/SYNTHETIC_G10_KNOWLEDGE.md";
        let g10_evidence_raw="Explicit NEW synthetic attributed knowledge source: declarations A/B answer only their exact knower/target/cutoff/TRS predicate. No historical or ingestion attestation.\n".as_bytes();
        let g10_parse = |raw: &[u8]| {
            assert!(raw.len() <= 1_048_576);
            // Match the parser's mandatory recursive-slot charge before value().
            // This increases LOCAL available state, paid in full from the SAME
            // cumulative parent; variable value storage can still refuse.
            let slots = std::mem::size_of::<JsonValue>()
                .checked_add(std::mem::size_of::<JsonString>())
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<
                        std::collections::HashMap<Vec<u16>, usize>,
                    >())
                })
                .and_then(|n| {
                    std::mem::size_of::<Vec<JsonValue>>()
                        .checked_mul(2)
                        .and_then(|vectors| n.checked_add(vectors))
                })
                .and_then(|n| n.checked_mul(65))
                .unwrap();
            let state = raw
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(4096))
                .and_then(|n| n.checked_add(slots))
                .unwrap();
            g10_charge(state);
            let document = tos_foundation::parse_json_with_state_budget(
                raw,
                JsonMode::PublishedStrict,
                JsonLimits {
                    max_bytes: raw.len().max(1),
                    max_depth: 64,
                    max_visits: g10_visit_limit
                        .get()
                        .checked_sub(g10_used_visits.get())
                        .unwrap(),
                    max_integer_digits: 4096,
                },
                state,
            )
            .unwrap();
            g10_visit(document.visits());
            document
        };
        // Compute definition/Event refs through the real metadata subject kernel,
        // independently of the planning template's analysis-only SHA strings.
        let g10_kraw = canonical_json(&g10_kd);
        let g10_rraw = canonical_json(&g10_rd);
        let g10_eraw = canonical_json(&g10_event);
        assert!(g10_kraw.len() <= 1024 && g10_rraw.len() <= 1024 && g10_eraw.len() <= 1024);
        let g10_kdoc = g10_parse(&g10_kraw);
        let g10_rdoc = g10_parse(&g10_rraw);
        let g10_edoc = g10_parse(&g10_eraw);
        let g10_kref = tos_command::source_forms::metadata_subject(g10_kdoc.root()).unwrap();
        let g10_rref = tos_command::source_forms::metadata_subject(g10_rdoc.root()).unwrap();
        let g10_eref = tos_command::source_forms::metadata_subject(g10_edoc.root()).unwrap();
        let g10_kref_value: Value = serde_json::from_slice(&bytes(&g10_kref)).unwrap();
        let g10_rref_value: Value = serde_json::from_slice(&bytes(&g10_rref)).unwrap();
        let g10_eref_value: Value = serde_json::from_slice(&bytes(&g10_eref)).unwrap();
        // Separate macro expansions retain the same two ordered values and
        // two-slot array allocation within the prepaid G10 literal workspace.
        let g10_declaration_a = serde_json::json!({
            "schema_version": "tos_source_temporal_claim_v1",
            "claim_id": "tos.claim.synthetic.g10.declaration-a",
            "claim_type": "relation",
            "claim_version": 1,
            "assertion_layer": "scholarly_report",
            "subject_ref": "tos.historical-event.synthetic.g10-source",
            "predicate": "historical_dating",
            "object": {
                "kind": "date-assertion",
                "role": "historical-time",
                "calendar": "proleptic-gregorian",
                "year_numbering": "astronomical",
                "certainty": "exact",
                "value": "1945-04-02",
                "source_wording": {
                    "language": "en",
                    "text": "Independent synthetic full-date statement; not a knowledge time."
                }
            },
            "maker": {
                "maker_type": "model",
                "agent_ref": "model:codex"
            },
            "evidence_refs": [
                "ToS/research-packets/foundation-laboratory-2026-07/SYNTHETIC_G10_KNOWLEDGE.md"
            ],
            "provenance_event_ref": "tos.event.source-creation.synthetic-g10-declarations",
            "epistemic_status": "reported",
            "review_status": "unreviewed",
            "visibility": "public_metadata_only",
            "qualifiers": {
                "statement": "Synthetic source declaration; no historical verification or admission.",
                "statement_language": "en",
                "statement_script": "Latn",
                "attributed_knowledge_by_cutoff": {
                    "profile_ref": "tos_attributed_knowledge_by_cutoff_assertion_v1",
                    "knower_ref": {
                        "id": "tos.knower.synthetic.g10.school-a",
                        "version": 1,
                        "digest": "sha256:fa46cbad6a62e79793163af073870d79149f8be35304e941a835730e7070a6c5"
                    },
                    "about_ref": {
                        "id": "tos.claim.synthetic.supported-native-calendar",
                        "version": 1,
                        "digest": "${ACTUAL_TARGET_1_CANONICAL_DIGEST}"
                    },
                    "cutoff": {
                        "value": "1800",
                        "trs": {
                            "id": "tos.trs-definition.synthetic.g10.civil-year",
                            "version": 1,
                            "digest": "sha256:4371f15a306b930eb32158b96673761a667ddd0b44dc0cbb13e9af851873bd6e"
                        }
                    },
                    "answer": true,
                    "basis": {
                        "source": {
                            "id": "tos.historical-event.synthetic.g10-source",
                            "version": 1,
                            "digest": "sha256:495a2db608b04ac95abb975613df9d850242b32f4cab1f8e6fce12bea9c9d03a"
                        },
                        "source_locator": "ToS/source-witnesses/history/synthetic-g10/historical-event.json",
                        "raw_evidence": "Explicit synthetic answer to only the named attributed knowledge predicate."
                    },
                    "declared_ingest": {
                        "lexical": "2019-01-01T00:00:00Z",
                        "scope": "SourceDeclaredNotOwnerTransactionTime"
                    },
                    "knower_definition": {
                        "reference": {
                            "id": "tos.knower.synthetic.g10.school-a",
                            "version": 1,
                            "digest": "sha256:fa46cbad6a62e79793163af073870d79149f8be35304e941a835730e7070a6c5"
                        },
                        "definition": {
                            "schema_version": "tos_source_declared_knower_v1",
                            "record_id": "tos.knower.synthetic.g10.school-a",
                            "record_version": 1,
                            "kind": "SourceDeclaredKnower",
                            "label": "Synthetic school A perspective"
                        },
                        "scope": "InlineSourceDefinitionNotAuthorization"
                    },
                    "trs_definition": {
                        "reference": {
                            "id": "tos.trs-definition.synthetic.g10.civil-year",
                            "version": 1,
                            "digest": "sha256:4371f15a306b930eb32158b96673761a667ddd0b44dc0cbb13e9af851873bd6e"
                        },
                        "definition": {
                            "schema_version": "tos_source_trs_definition_v1",
                            "record_id": "tos.trs-definition.synthetic.g10.civil-year",
                            "record_version": 1,
                            "trs": {
                                "id": "tos.trs.synthetic.held-list-civil-year-v1",
                                "version": "1"
                            },
                            "calendar_algorithm": "GregorianProleptic",
                            "year_numbering": "astronomical",
                            "rules_version": "synthetic-gregorian-civil-year-r1",
                            "unit": "civil-year",
                            "origin": "astronomical-year-number"
                        },
                        "scope": "InlineSourceDefinitionNotAuthorization"
                    }
                }
            }
        });
        let g10_declaration_b = serde_json::json!({
            "schema_version": "tos_source_temporal_claim_v1",
            "claim_id": "tos.claim.synthetic.g10.declaration-b",
            "claim_type": "relation",
            "claim_version": 1,
            "assertion_layer": "scholarly_report",
            "subject_ref": "tos.historical-event.synthetic.g10-source",
            "predicate": "historical_dating",
            "object": {
                "kind": "date-assertion",
                "role": "historical-time",
                "calendar": "proleptic-gregorian",
                "year_numbering": "astronomical",
                "certainty": "exact",
                "value": "1945-04-02",
                "source_wording": {
                    "language": "en",
                    "text": "Independent synthetic full-date statement; not a knowledge time."
                }
            },
            "maker": {
                "maker_type": "model",
                "agent_ref": "model:codex"
            },
            "evidence_refs": [
                "ToS/research-packets/foundation-laboratory-2026-07/SYNTHETIC_G10_KNOWLEDGE.md"
            ],
            "provenance_event_ref": "tos.event.source-creation.synthetic-g10-declarations",
            "epistemic_status": "reported",
            "review_status": "unreviewed",
            "visibility": "public_metadata_only",
            "qualifiers": {
                "statement": "Synthetic source declaration; no historical verification or admission.",
                "statement_language": "en",
                "statement_script": "Latn",
                "attributed_knowledge_by_cutoff": {
                    "profile_ref": "tos_attributed_knowledge_by_cutoff_assertion_v1",
                    "knower_ref": {
                        "id": "tos.knower.synthetic.g10.school-a",
                        "version": 1,
                        "digest": "sha256:fa46cbad6a62e79793163af073870d79149f8be35304e941a835730e7070a6c5"
                    },
                    "about_ref": {
                        "id": "tos.claim.synthetic.supported-native-calendar-b",
                        "version": 1,
                        "digest": "${ACTUAL_TARGET_2_CANONICAL_DIGEST}"
                    },
                    "cutoff": {
                        "value": "1800",
                        "trs": {
                            "id": "tos.trs-definition.synthetic.g10.civil-year",
                            "version": 1,
                            "digest": "sha256:4371f15a306b930eb32158b96673761a667ddd0b44dc0cbb13e9af851873bd6e"
                        }
                    },
                    "answer": false,
                    "basis": {
                        "source": {
                            "id": "tos.historical-event.synthetic.g10-source",
                            "version": 1,
                            "digest": "sha256:495a2db608b04ac95abb975613df9d850242b32f4cab1f8e6fce12bea9c9d03a"
                        },
                        "source_locator": "ToS/source-witnesses/history/synthetic-g10/historical-event.json",
                        "raw_evidence": "Explicit synthetic answer to only the named attributed knowledge predicate."
                    },
                    "declared_ingest": {
                        "lexical": "2026-01-01T00:00:00Z",
                        "scope": "SourceDeclaredNotOwnerTransactionTime"
                    },
                    "knower_definition": {
                        "reference": {
                            "id": "tos.knower.synthetic.g10.school-a",
                            "version": 1,
                            "digest": "sha256:fa46cbad6a62e79793163af073870d79149f8be35304e941a835730e7070a6c5"
                        },
                        "definition": {
                            "schema_version": "tos_source_declared_knower_v1",
                            "record_id": "tos.knower.synthetic.g10.school-a",
                            "record_version": 1,
                            "kind": "SourceDeclaredKnower",
                            "label": "Synthetic school A perspective"
                        },
                        "scope": "InlineSourceDefinitionNotAuthorization"
                    },
                    "trs_definition": {
                        "reference": {
                            "id": "tos.trs-definition.synthetic.g10.civil-year",
                            "version": 1,
                            "digest": "sha256:4371f15a306b930eb32158b96673761a667ddd0b44dc0cbb13e9af851873bd6e"
                        },
                        "definition": {
                            "schema_version": "tos_source_trs_definition_v1",
                            "record_id": "tos.trs-definition.synthetic.g10.civil-year",
                            "record_version": 1,
                            "trs": {
                                "id": "tos.trs.synthetic.held-list-civil-year-v1",
                                "version": "1"
                            },
                            "calendar_algorithm": "GregorianProleptic",
                            "year_numbering": "astronomical",
                            "rules_version": "synthetic-gregorian-civil-year-r1",
                            "unit": "civil-year",
                            "origin": "astronomical-year-number"
                        },
                        "scope": "InlineSourceDefinitionNotAuthorization"
                    }
                }
            }
        });
        let mut g10_declarations = Value::Array(vec![g10_declaration_a, g10_declaration_b]);
        for (i, target) in [&member_ref, &second_member_ref].into_iter().enumerate() {
            let d = &mut g10_declarations[i]["qualifiers"]["attributed_knowledge_by_cutoff"];
            d["about_ref"] = target.clone();
            d["knower_ref"] = g10_kref_value.clone();
            d["cutoff"]["trs"] = g10_rref_value.clone();
            d["basis"]["source"] = g10_eref_value.clone();
            d["knower_definition"]["reference"] = g10_kref_value.clone();
            d["trs_definition"]["reference"] = g10_rref_value.clone();
            assert_eq!(d["knower_definition"]["definition"], g10_kd);
            assert_eq!(d["trs_definition"]["definition"], g10_rd);
            assert!(canonical_json(&g10_declarations[i]).len() <= 4096);
        }
        for (path, raw) in [
            (g10_event_path, g10_eraw.as_slice()),
            (g10_evidence, g10_evidence_raw),
        ] {
            let path = isolated.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, raw).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let g10_home = "ToS/source-witnesses/relations/synthetic-g10-declarations";
        let g10_source_path = format!("{g10_home}/source-claims.jsonl");
        let g10_owner = isolated.path().join("g10-declarations-owner.json");
        let g10_owner_value = serde_json::json!({
            "schema_version": "tos_local_claim_create_owner_v2",
            "uid": "${ACTUAL_UID}",
            "principal_id": "model:codex",
            "maker_type": "model",
            "source_root": "${ACTUAL_ISOLATED_ROOT}",
            "source_path": "ToS/source-witnesses/relations/synthetic-g10-declarations/source-claims.jsonl",
            "authority_ref": "synthetic-test-only:g10-declaration-creation",
            "expires_at": "2099-01-01T00:00:00Z",
            "provenance_event_id": "tos.event.source-creation.synthetic-g10-declarations",
            "allowed_operations": [
                "claims.create"
            ],
            "allowed_claim_ids": [
                "tos.claim.synthetic.g10.declaration-a",
                "tos.claim.synthetic.g10.declaration-b"
            ],
            "allowed_subject_refs": [
                "tos.historical-event.synthetic.g10-source"
            ],
            "allowed_object_refs": [],
            "allowed_object_values": [
                {
                    "kind": "date-assertion",
                    "role": "historical-time",
                    "calendar": "proleptic-gregorian",
                    "year_numbering": "astronomical",
                    "certainty": "exact",
                    "value": "1945-04-02",
                    "source_wording": {
                        "language": "en",
                        "text": "Independent synthetic full-date statement; not a knowledge time."
                    }
                }
            ],
            "allowed_predicates": [
                "historical_dating"
            ],
            "allowed_evidence_refs": [
                "ToS/research-packets/foundation-laboratory-2026-07/SYNTHETIC_G10_KNOWLEDGE.md"
            ]
        });
        let mut g10_owner_value = g10_owner_value;
        g10_owner_value["uid"] = serde_json::json!(uid);
        g10_owner_value["source_root"] = serde_json::json!(isolated.path());
        let g10_owner_raw = canonical_json(&g10_owner_value);
        assert!(g10_owner_raw.len() <= 8192);
        fs::write(&g10_owner, &g10_owner_raw).unwrap();
        fs::set_permissions(&g10_owner, fs::Permissions::from_mode(0o600)).unwrap();
        let g10_bd_files = authored_text_files(isolated.path());
        assert!(
            g10_bd_files.len() <= 512
                && g10_bd_files.values().map(Vec::len).sum::<usize>() <= 2 * 1024 * 1024
        );
        let g10_bd = super::validation_cut_cases::write_cut_store(&g10_bd_files, &store);
        assert_ne!(g10_bd, bp);
        // The new capture input is standalone, never another ancestor of Sp.
        let g10_bd_manifest: Value = serde_json::from_slice(
            &fs::read(
                store
                    .join("revisions")
                    .join(g10_bd.0.to_hex())
                    .join("snapshot.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(g10_bd_manifest["base_revision"].is_null());
        g10_charge(g10_clone_forecast(&invocation, 0, &g10_visit));
        let mut g10_invocation = invocation.clone();
        g10_invocation["owner_config"] = serde_json::json!(g10_owner);
        g10_invocation["source_revision"] = serde_json::json!(g10_bd.0.to_prefixed());
        g10_invocation["original_source_revision"] = serde_json::json!(g10_bd.0.to_prefixed());
        let g10_invocation_path = isolated.path().join("g10-declarations-invocation.json");
        let g10_invocation_raw = canonical_json(&g10_invocation);
        assert!(g10_invocation_raw.len() <= 8192);
        fs::write(&g10_invocation_path, &g10_invocation_raw).unwrap();
        fs::set_permissions(&g10_invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut g10_create = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","claims":g10_declarations});
        let g10_preview = alignment_native_cli(
            &repository,
            &g10_owner,
            &g10_invocation_path,
            &g10_create,
            deadline,
        );
        assert_eq!(g10_preview["grants_admission"], false);
        g10_create["operation"] = serde_json::json!("claims.create");
        g10_create["command_id"] = serde_json::json!("synthetic:g10-declarations-create");
        g10_charge(g10_clone_forecast(
            &g10_preview["result"]["owner_configuration"],
            0,
            &g10_visit,
        ));
        g10_create["expected_configuration"] = g10_preview["result"]["owner_configuration"].clone();
        g10_create["expected_revision"] = Value::Null;
        g10_charge(g10_clone_forecast(
            &g10_preview["result"]["expected_dependencies"],
            0,
            &g10_visit,
        ));
        g10_create["expected_dependencies"] =
            g10_preview["result"]["expected_dependencies"].clone();
        g10_charge(g10_clone_forecast(
            &g10_preview["result"]["source_bindings"],
            0,
            &g10_visit,
        ));
        g10_create["expected_inputs"] = g10_preview["result"]["source_bindings"].clone();
        let mut g10_created = alignment_native_cli(
            &repository,
            &g10_owner,
            &g10_invocation_path,
            &g10_create,
            deadline,
        );
        assert_eq!(g10_created["result"]["replayed"], false);
        assert_eq!(g10_created["result"]["receipt"]["grants_admission"], false);
        let mut g10_package = BTreeMap::new();
        let mut g10_package_capacity = 0usize;
        for entry in fs::read_dir(isolated.path().join(g10_home)).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            assert!(g10_package.len() < 5);
            let len = usize::try_from(entry.metadata().unwrap().len()).unwrap();
            assert!(len <= 131072);
            g10_package_capacity = g10_package_capacity
                .checked_add(len.checked_mul(2).unwrap())
                .unwrap();
            assert!(g10_package_capacity <= 262144);
            g10_charge(len.checked_mul(2).and_then(|n| n.checked_add(8)).unwrap());
            let raw = fs::read(entry.path()).unwrap();
            assert_eq!(raw.len(), len);
            assert!(raw.capacity() <= len * 2 + 8);
            g10_package.insert(entry.file_name().into_string().unwrap(), raw);
        }
        assert_eq!(
            g10_package
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "source-claims.jsonl",
                "source-create-request.json",
                "source-create-environment.json",
                "source-create-provenance.jsonl",
                "source-create-receipt.json"
            ])
        );
        g10_charge(g10_clone_forecast(
            &g10_created["result"]["receipt"],
            0,
            &g10_visit,
        ));
        let g10_receipt_document = g10_parse(&g10_package["source-create-receipt.json"]);
        let g10_receipt_expected_raw =
            g10_scratch(&g10_created["result"]["receipt"], &g10_charge, &g10_visit);
        let g10_receipt_expected = g10_parse(&g10_receipt_expected_raw);
        g10_charge(
            g10_package["source-create-receipt.json"]
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(4096))
                .unwrap(),
        );
        assert_eq!(
            bytes(g10_receipt_document.root()),
            bytes(g10_receipt_expected.root())
        );
        // Move ONLY the present, fully compared native receipt after the above
        // replay/admission checks. The old clone forecast stays debited.
        let receipt_slot = g10_created
            .get_mut("result")
            .unwrap()
            .get_mut("receipt")
            .unwrap();
        assert!(receipt_slot.is_object());
        let g10_receipt = receipt_slot.take();
        g10_charge(g10_clone_forecast(&g10_create, 0, &g10_visit));
        let g10_request_document = g10_parse(&g10_package["source-create-request.json"]);
        let g10_request_raw = g10_scratch(&g10_create, &g10_charge, &g10_visit);
        let g10_request_expected = g10_parse(&g10_request_raw);
        g10_charge(
            g10_request_raw
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(4096))
                .unwrap(),
        );
        assert_eq!(
            bytes(g10_request_document.root()),
            bytes(g10_request_expected.root())
        );
        drop(g10_receipt_document);
        drop(g10_receipt_expected);
        drop(g10_request_document);
        drop(g10_request_expected);
        let g10_stream = &g10_package["source-claims.jsonl"];
        assert!(g10_stream.len() <= 131072 && g10_stream.last() == Some(&b'\n'));
        let g10_stream_sha = Digest256::of_bytes(g10_stream);
        let g10_rows = g10_stream.split(|b| *b == b'\n').collect::<Vec<_>>();
        assert_eq!(g10_rows.len(), 3);
        assert!(g10_rows[2].is_empty());
        assert_eq!(g10_receipt["claims"].as_array().unwrap().len(), 2);
        let g10_drefs = [
            exact_ref(&g10_receipt["claims"][0]),
            exact_ref(&g10_receipt["claims"][1]),
        ];
        for i in 0..2 {
            assert!(g10_rows[i].len() <= 4096);
            let doc = g10_parse(g10_rows[i]);
            assert_eq!(
                bytes(doc.root()),
                bytes(&g10_parse(&canonical_json(&g10_create["claims"][i])).into_root())
            );
            assert_eq!(
                bytes(&tos_command::source_forms::metadata_subject(doc.root()).unwrap()),
                bytes(&g10_parse(&canonical_json(&g10_receipt["claims"][i])).into_root())
            );
        }
        let all_ids = [
            member_ref["id"].as_str().unwrap(),
            second_member_ref["id"].as_str().unwrap(),
            g10_drefs[0].id.as_str(),
            g10_drefs[1].id.as_str(),
        ];
        assert_eq!(all_ids.into_iter().collect::<BTreeSet<_>>().len(), 4);
        assert_eq!(
            fs::read(isolated.path().join(&member_path)).unwrap(),
            member_raw
        );
        assert_eq!(
            Digest256::of_bytes(&fs::read(isolated.path().join(&g10_source_path)).unwrap()),
            g10_stream_sha
        );
        // Reopen no old carrier: the complete FOUR actual packets are frozen below.
        let g10_authored = authored_text_files(isolated.path());
        let claim_streams = g10_authored
            .keys()
            .filter(|p| p.ends_with("/source-claims.jsonl"))
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            claim_streams,
            BTreeSet::from([member_path.as_str(), g10_source_path.as_str()])
        );
        assert!(
            !g10_authored
                .keys()
                .any(|p| p.ends_with("historical-claims.jsonl"))
        );
        drop(g10_bd_files);
        drop(g10_authored);
        drop(g10_invocation);
        drop(g10_created);
        drop(g10_preview);
        let extra_bytes = g10_used_bytes.get().saturating_sub(1_048_576);
        let extra_visits = g10_used_visits.get().saturating_sub(8192);
        b_fixture_retained_bytes = b_fixture_retained_bytes.checked_add(extra_bytes).unwrap();
        b_fixture_retained_visits = b_fixture_retained_visits.checked_add(extra_visits).unwrap();
        g10_applied_bytes.set(extra_bytes);
        g10_applied_visits.set(extra_visits);
        assert!(
            b_fixture_retained_bytes
                .checked_add(G10_FIXED_BYTES)
                .unwrap()
                < 16 * 1024 * 1024
        );
        assert!(
            b_fixture_retained_visits
                .checked_add(G10_FIXED_VISITS)
                .unwrap()
                < 1_000_000
        );
        eprintln!(
            "G10 phase=declarations-created native_delta=2 package_files=5 rows=2 target_bytes_unchanged=true"
        );
        let actual = authored_text_files(isolated.path());
        let revision = super::validation_cut_cases::write_cut_store(&actual, &store); // standalone Sp
        assert_ne!(revision, bp);
        let manifest: Value = serde_json::from_slice(
            &fs::read(
                store
                    .join("revisions")
                    .join(revision.0.to_hex())
                    .join("snapshot.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(manifest["base_revision"].is_null());
        files.retain(|path, _| !path.starts_with("ToS/"));
        files.extend(actual);
        let cut = open_cut(&store, revision, deadline, &cancellation);
        eprintln!(
            "temporal positive phase=actual-Sp-model Sp={}",
            revision.0.to_prefixed()
        );
        let (_inventory_capture, _inventory_reader, selected) = {
            assert_eq!(cut.current().revision(), revision);
            let selected_bytes = cut
                .current()
                .members()
                .try_fold(0u64, |total, member| total.checked_add(member.size_bytes))
                .expect("positive Sp manifest byte sum overflow");
            assert!(selected_bytes <= 2 * 1024 * 1024);
            let git_fixture = tempfile::tempdir().unwrap();
            let git_root = git_fixture.path();
            let git = |args: &[&str]| {
                assert!(
                    Instant::now() < deadline
                        && !cancellation.load(std::sync::atomic::Ordering::SeqCst)
                );
                let mut command = std::process::Command::new("git");
                command
                    .arg("-c")
                    .arg("core.hooksPath=/dev/null")
                    .arg("-C")
                    .arg(git_root)
                    .args(args);
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("GIT_") {
                        command.env_remove(key);
                    }
                }
                let output = command.output().unwrap();
                assert!(
                    output.status.success(),
                    "bounded S inventory Git operation failed"
                );
                assert!(
                    Instant::now() < deadline
                        && !cancellation.load(std::sync::atomic::Ordering::SeqCst)
                );
                output.stdout
            };
            git(&["init", "-q"]);
            let mut actual_s = BTreeMap::new();
            let mut stream = cut.stream(revision).unwrap();
            let membership = stream.expectation();
            while let Some(entry) = stream.next_member(deadline, &cancellation).unwrap() {
                assert_eq!(entry.revision, revision);
                assert!(
                    actual_s
                        .insert(entry.path.as_str().to_owned(), entry.raw)
                        .is_none()
                );
            }
            assert_eq!(stream.coverage(), Some(membership));
            // Narrow actual recipe admission before the extra Git/capture
            // copies. Product reader/writer limits and whole-case caps stay put.
            assert!(actual_s.values().map(Vec::len).sum::<usize>() <= 2 * 1024 * 1024);
            assert_eq!(actual_s.get(&member_path), Some(&member_raw));
            let mut inventory = files
                .iter()
                .filter(|(path, _)| !path.starts_with("ToS/"))
                .map(|(p, b)| (p.clone(), b.clone()))
                .collect::<BTreeMap<_, _>>();
            inventory.extend(actual_s);
            assert!(
                inventory.len() <= 512
                    && inventory.values().map(Vec::len).sum::<usize>() <= 8 * 1024 * 1024
            );
            for (name, raw) in &inventory {
                let target = git_root.join(name);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(&target, raw).unwrap();
                fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).unwrap();
            }
            // Force is Git's explicit exact-fixture membership, NOT a resource
            // or admission force. Retained JSONL package files must be tracked.
            git(&["add", "-f", "--", "."]);
            git(&[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "Exact native-created S inventory",
            ]);
            let commit = String::from_utf8(git(&["rev-parse", "HEAD^{commit}"])).unwrap();
            // The convenience wrapper starts its own 120s clock. Reuse its
            // exact existing owner operations with this SAME family deadline.
            let capture_home = tempfile::tempdir().unwrap();
            let capture_path = capture_home.path().join("software-capture");
            let restored_path = capture_home.path().join("software-restored");
            let selection = super::source_cut_cases::capture_software_archive(
                git_root,
                commit.trim(),
                &["ToS", "rust", "access"],
                &capture_path,
                deadline,
                &cancellation,
            );
            super::source_cut_cases::restore_software_archive(
                &capture_path,
                &restored_path,
                &selection,
                deadline,
                &cancellation,
            );
            let capture = super::source_cut_cases::SoftwareCaptureFixture {
                temporary: capture_home,
                capture: capture_path,
                restored: restored_path,
                selection,
            };
            let inventory_reader = tos_source_store::SoftwareCaptureReader::open(
                &capture.capture,
                &capture.restored,
                capture.selection.clone(),
                tos_source_store::ReadLimits {
                    max_manifest_bytes: 2 * 1024 * 1024,
                    max_manifest_entries: 512,
                    max_selected_object_bytes: 16 * 1024 * 1024,
                    json: JsonLimits::default(),
                },
                deadline,
                &cancellation,
            )
            .unwrap();
            let model = super::compiler_source_cases::native_selected_model_at_existing_cut(
                &cut,
                revision,
                &inventory_reader,
                temporary
                    .path()
                    .join("supported-positive-S-model/knowledge.sqlite3"),
                deadline,
                &cancellation,
            );
            assert_eq!(
                model
                    .open()
                    .unwrap()
                    .corpus_original_receipt()
                    .unwrap()
                    .origin
                    .native_producer
                    .as_ref()
                    .unwrap()
                    .source_revision,
                revision.0.to_hex()
            );
            (capture, inventory_reader, model)
        };
        let snapshot_name = "tos.snapshot.synthetic-supported-native-calendar";
        let frozen = serde_json::json!({"schema_version":"tos_frozen_chronology_v1",
            "source_snapshot":{"id":snapshot_name,"revision":revision.0.to_prefixed()},
            "system_as_of":{"value":"2026-10-19T05:00:00Z","trs":null},"attributed_knowledge":null,"source_publication_as_of":null,
            "selection_mode":"ExplicitExactMembers","members":[member_ref,second_member_ref],"policies":{
                "evidence":inline_policy("Evidence","ExactPinnedMemberEvidence-v1"),
                "selection":inline_policy("Selection","ExplicitExactMembers-v1")}});
        let relative = "ToS/source-witnesses/chronologies/synthetic-supported-calendar/chronology-version.json";
        let identity = "tos.chronology-version.synthetic-supported-native-calendar";
        let record = serde_json::json!({"schema_version":"tos_chronology_version_record_v1","record_type":"chronology-version",
            "record_id":identity,"record_version":1,"preferred_label":"Synthetic supported native calendar chronology",
            "variant_labels":[],"identity_status":"provisional","source_refs":[evidence],"external_identifiers":[],
            "same_as_posture":"no_equivalence_claim","visibility":"public_metadata_only","semantic_content":frozen});
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
        let owner = isolated.path().join("supported-CV-create-owner.json");
        let config = serde_json::json!({"schema_version":"tos_local_profile_create_owner_v1","uid":uid,
            "principal_id":"software:test-fixture","maker_type":"software","source_root":isolated.path(),
            "source_path":relative,"record_id":identity,"profile_type_id":"tos.entity.chronology-version",
            "authority_ref":"synthetic-test-only:supported-calendar-CV-creation","allowed_form_ids":["tos.form.creation.fixture-name"],
            "allowed_operations":["source.create"],"expires_at":"2099-01-01T00:00:00Z",
            "provenance_event_id":"tos.event.synthetic-supported-native-calendar-cv-create"});
        let config_raw = canonical_json(&config);
        fs::write(&owner, &config_raw).unwrap();
        fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
        let creation_fs =
            CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation)
                .unwrap();
        let mut request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","record":record,
            "forms":[{"form_id":"tos.form.creation.fixture-name","field_id":"metadata.preferred-name"}]});
        let mut context = cut_context(&files, config_raw, canonical_json(&request), revision);
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
        assert_prepared_file_refs(&preview, prepared.files());
        request["operation"] = serde_json::json!("source.create");
        request["command_id"] = serde_json::json!("synthetic:supported-calendar-CV-create");
        request["expected_configuration"] =
            serde_json::from_slice(&bytes(preview.object_get("owner_configuration").unwrap()))
                .unwrap();
        request["expected_dependencies"] =
            serde_json::from_slice(&bytes(preview.object_get("expected_dependencies").unwrap()))
                .unwrap();
        request["expected_source"] = Value::Null;
        request["expected_revision"] = Value::Null;
        context.request_raw = canonical_json(&request);
        drop(prepared);
        worker.finish(deadline, &cancellation).unwrap();
        let mut operation_worker =
            super::command_form_cases::schemas_with_image(&cut, &image, deadline, &cancellation);
        let (executed, published, result) = execute_isolated_creation_from_captures(
            &creation_fs,
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
            result.object_get("grants_admission"),
            Some(&JsonValue::Bool(false))
        );
        let home = isolated.path().join(relative).parent().unwrap().to_owned();
        assert_eq!(
            executed
                .prepared()
                .files()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "chronology-version.json",
                "chronology-version.human-forms.json",
                "source-create-request.json",
                "source-create-environment.json",
                "source-create-provenance.jsonl",
                "source-create-receipt.json"
            ])
        );
        assert_prepared_file_refs(
            &executed.prepared().preview().unwrap(),
            executed.prepared().files(),
        );
        for (name, raw) in executed.prepared().files() {
            assert_eq!(fs::read(home.join(name)).unwrap(), *raw);
        }
        assert_eq!(fs::read_dir(&home).unwrap().count(), 6);
        let cv_raw = fs::read(isolated.path().join(relative)).unwrap();
        let cv_record: Value = serde_json::from_slice(&cv_raw).unwrap();
        assert_eq!(cv_record, record);
        let cv_receipt: Value =
            serde_json::from_slice(&fs::read(home.join("source-create-receipt.json")).unwrap())
                .unwrap();
        assert_eq!(
            bytes(&parse(&canonical_json(&cv_receipt))),
            bytes(result.object_get("receipt").unwrap())
        );
        let v1_ref = cv_receipt["source"].clone();
        assert_eq!(
            bytes(&parse(&canonical_json(&v1_ref))),
            bytes(&tos_command::source_forms::metadata_subject(&parse(&cv_raw)).unwrap())
        );
        // execute_isolated_creation_from_captures already finishes its worker.
        drop(executed);
        // Real post-CV epoch and catalog before freezing authored Tp. Derived
        // catalog JSONL stays in separate protected read custody, not the cut.
        let control_path = isolated
            .path()
            .join("ToS/source-witnesses/.metadata-publication.json");
        let read_control = || {
            let metadata = match fs::symlink_metadata(&control_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
                Err(error) => panic!("positive actual CONTROL metadata: {error}"),
            };
            assert!(metadata.is_file() && metadata.len() <= 8192);
            let raw = fs::read(&control_path).expect("positive actual CONTROL read");
            assert_eq!(raw.len() as u64, metadata.len());
            Some(raw)
        };
        let control_before = read_control();
        let epoch = tos_source_store::MetadataPublicationEpoch::select(
            control_before.as_ref().map(|raw| parse(raw)),
        )
        .unwrap();
        super::compiler_source_cases::publish_native_catalog_fixture_with_selected_schemas(
            &repository,
            isolated.path(),
            deadline,
            &selected_schema_files,
        );
        assert_eq!(read_control(), control_before);
        let catalog_manifest_path = isolated
            .path()
            .join("ToS/source-witnesses/catalog/catalog.manifest.json");
        assert!(fs::metadata(&catalog_manifest_path).unwrap().len() <= 1_048_576);
        let catalog_manifest: Value =
            serde_json::from_slice(&fs::read(&catalog_manifest_path).unwrap()).unwrap();
        match epoch.token() {
            Some(token) => assert_eq!(
                catalog_manifest["selected_metadata_publication"]["token"],
                token
            ),
            None => assert!(
                catalog_manifest
                    .get("selected_metadata_publication")
                    .is_none()
            ),
        }
        let current_authored = authored_text_files(isolated.path());
        assert_eq!(
            current_authored.get("ToS/source-witnesses/.metadata-publication.json"),
            control_before.as_ref()
        );
        assert!(
            !current_authored
                .keys()
                .any(|p| p.starts_with("ToS/source-witnesses/catalog/") && !p.ends_with(".md"))
        );
        let tp = super::command_form_cases::successor(&current_authored, &store, revision);
        let current_cut = open_cut(&store, tp, deadline, &cancellation);
        let mut current_files = files
            .iter()
            .filter(|(p, _)| !p.starts_with("ToS/"))
            .map(|(p, r)| (p.clone(), r.clone()))
            .collect::<BTreeMap<_, _>>();
        current_files.extend(current_authored);
        let revision_path = isolated.path().join("supported-CV-read-owner.json");
        let revision_config = serde_json::json!({"schema_version":"tos_local_profile_revision_owner_v1","uid":uid,"principal_id":"software:test-fixture",
            "source_root":isolated.path(),"source_path":relative,"record_id":identity,"profile_type_id":"tos.entity.chronology-version",
            "authority_ref":"synthetic-test-only:supported-calendar-current-public-read","allowed_form_ids":["tos.form.creation.fixture-name"],
            "allowed_operations":["record.revise"],"allowed_fields":["semantic_content"],"expires_at":"2099-01-01T00:00:00Z"});
        let revision_config_raw = canonical_json(&revision_config);
        fs::write(&revision_path, &revision_config_raw).unwrap();
        fs::set_permissions(&revision_path, fs::Permissions::from_mode(0o600)).unwrap();
        let revision_fs =
            CreationFilesystem::select_isolated(&isolated, &revision_path, deadline, &cancellation)
                .unwrap();
        assert!(
            current_files.len() <= 2048
                && current_files.values().map(Vec::len).sum::<usize>() <= 33_554_432
        );
        let mut current_context = cut_context(
            &current_files,
            revision_config_raw.clone(),
            canonical_json(
                &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
            ),
            tp,
        );
        current_context.effective_uid = u64::from(uid);
        let snapshot = SourceSnapshot {
            reference: SourceSnapshotRef {
                id: StableId::parse(snapshot_name).unwrap(),
                revision,
            },
            current_cutoff: Cutoff {
                value: parse(b"1"),
                trs: None,
            },
        };
        let mut cold = selected.open().unwrap();
        let bound = tos_query::bind_verified_knowledge(
            &cold,
            &selected.vocabulary,
            &selected.descriptor_bytes,
        )
        .unwrap();
        assert_eq!(
            bound.require_source_revision().unwrap(),
            revision.0.to_hex()
        );
        // Finalized graph snapshot is an ACTUAL locator source, not owner
        // authority. The adapter independently verifies the full owner packet.
        assert!(selected.graph_input_bytes.len() <= 8 * 1024 * 1024);
        let graph: Value = serde_json::from_slice(&selected.graph_input_bytes).unwrap();
        assert_eq!(graph["source_revision"], revision.0.to_hex());
        let selected_graph = bound
            .source_for_adapter("reified-bibliographic-claims-v1")
            .unwrap();
        let mut matched = graph["nodes"].as_array().unwrap().iter().filter(|node| {
            node["source_graph"].as_str() == Some(selected_graph)
                && node["kind_id"] == "claim"
                && node["type_id"] == "tos.entity.claim"
                && node["attributes"]["source_claim"]["claim_id"] == member["claim_id"]
        });
        let node = matched.next().unwrap();
        assert!(matched.next().is_none());
        assert_eq!(
            bytes(&parse(&canonical_json(&node["attributes"]["source_claim"]))),
            bytes(&parse(member_row_raw))
        );
        let node_id = node["id"].as_str().unwrap();
        assert!(!node_id.is_empty() && node_id.len() <= 1024);
        Digest256::from_hex(node["content_revision"].as_str().unwrap()).unwrap();
        let locator = parse(&canonical_json(
            &serde_json::json!({"node_id":node_id,"content_revision":node["content_revision"]}),
        ));
        let mut matched_b = graph["nodes"].as_array().unwrap().iter().filter(|node| {
            node["source_graph"].as_str() == Some(selected_graph)
                && node["kind_id"] == "claim"
                && node["type_id"] == "tos.entity.claim"
                && node["attributes"]["source_claim"]["claim_id"] == second_member["claim_id"]
        });
        let node_b = matched_b.next().unwrap();
        assert!(matched_b.next().is_none());
        assert_eq!(
            bytes(&parse(&canonical_json(
                &node_b["attributes"]["source_claim"]
            ))),
            bytes(second_owner_document.root())
        );
        let node_b_id = node_b["id"].as_str().unwrap();
        assert!(!node_b_id.is_empty() && node_b_id.len() <= 1024);
        Digest256::from_hex(node_b["content_revision"].as_str().unwrap()).unwrap();
        let second_locator_raw = canonical_json(&serde_json::json!({
            "node_id":node_b_id,"content_revision":node_b["content_revision"]}));
        assert!(second_locator_raw.len() <= 4096 && second_locator_raw.capacity() <= 8192);
        let second_locator = tos_foundation::parse_json_with_state_budget(
            &second_locator_raw,
            JsonMode::PublishedStrict,
            JsonLimits {
                max_bytes: 4096,
                max_depth: 8,
                max_visits: 16,
                max_integer_digits: 20,
            },
            65_536, // included in the prepaid fixed 1MiB authoring/locator overlap
        )
        .unwrap()
        .into_root();

        assert_ne!(
            locator.object_get("node_id"),
            second_locator.object_get("node_id")
        );
        assert_eq!(
            second_locator.object_get("node_id").unwrap().as_str(),
            Some(node_b_id)
        );
        let temporal_request = parse(&canonical_json(
            &serde_json::json!({"schema_version":"tos_temporal_comparison_request_v1","source_revision":revision.0.to_hex(),
            "left":serde_json::from_slice::<Value>(&bytes(&locator)).unwrap(),"right":serde_json::from_slice::<Value>(&bytes(&locator)).unwrap()}),
        ));
        let temporal_controls = Arc::new(Controls::default());
        let temporal_held = Arc::new(AtomicUsize::new(0));
        let mut temporal_authority = TemporalAuthority::new(
            &bound,
            &tos_access::KnowledgeRequest::Temporal(temporal_request),
            temporal_held.clone(),
            temporal_controls.clone(),
        );
        let temporal_scope =
            tos_query::InspectCurrentAuthority::disclosure_scope(&temporal_authority);
        assert_eq!(
            temporal_scope.operation_id,
            "tos.knowledge.temporal.compare"
        );
        assert_eq!(
            temporal_scope.intended_use,
            tos_query::TEMPORAL_INTENDED_USE
        );
        let mut load_worker = super::command_form_cases::schemas_with_image(
            &current_cut,
            &image,
            deadline,
            &cancellation,
        );
        let pair_owner = load_chronology_from_owner(
            &revision_fs,
            &current_context,
            &current_cut,
            &exact_ref(&v1_ref),
            &context,
            &cut,
            &snapshot,
            &software,
            &components,
            &mut load_worker,
            ChronologyLimits::default(),
            deadline,
            &cancellation,
        )
        .unwrap();
        assert_eq!(
            pair_owner.bound().chronology().member_claim_revisions,
            vec![exact_ref(&member_ref), exact_ref(&second_member_ref)]
        );
        let pair_member = exact_ref(&member_ref);
        eprintln!(
            "temporal positive phase=held-selfpair Bp={} Sp={} Tp={}",
            bp.0.to_prefixed(),
            revision.0.to_prefixed(),
            tp.0.to_prefixed()
        );
        with_loaded_chronology_envelope_comparison(
            pair_owner,
            &pair_member,
            &pair_member,
            &locator,
            &locator,
            &mut cold,
            &bound,
            &mut temporal_authority,
            HeldClaimComparisonBudget {
                query: tos_query::InspectBudget {
                    max_open_vm_steps: selected.cold_limits().max_vm_steps,
                    max_read_vm_steps: 20_000_000,
                    max_matches: 16,
                    max_rows: 1000,
                    max_field_bytes: 16_384,
                    max_payload_bytes: 1_048_576,
                    max_decoded_bytes: 8 * 1024 * 1024,
                    max_response_bytes: 2 * 1024 * 1024,
                    json: JsonLimits::default(),
                },
                max_extra_owned_bytes: (16 * 1024 * 1024usize)
                    .checked_sub(b_fixture_retained_bytes)
                    .unwrap(),
                max_extra_json_visits: 1_000_000usize
                    .checked_sub(b_fixture_retained_visits)
                    .unwrap(),
                max_verification_owned_bytes: 4 * 1024 * 1024,
                max_verification_json_visits: 250_000,
            },
            deadline,
            &cancellation,
            |view| {
                assert!(temporal_held.load(Ordering::SeqCst) > 0);
                view.with_current_bytes(|raw| {
                    assert!(raw.len() <= 2 * 1024 * 1024);
                    let output: Value = serde_json::from_slice(raw).unwrap();
                    assert_eq!(output.as_object().unwrap().len(), 11);
                    assert_eq!(
                        output["schema_version"],
                        "tos_stored_claim_envelope_comparison_v1"
                    );
                    assert_eq!(
                        output["scope"],
                        "OwnerMemberVerifiedSourceEnvelopeComparison"
                    );
                    assert_eq!(output["a3_assessment"], "NotEstablished");
                    assert_eq!(
                        output["policy_scope"],
                        "SourceSelectionProvenanceNotAuthorization"
                    );
                    assert_eq!(output["source_revision"], revision.0.to_hex());
                    for (key, expected) in [
                        ("chronology_ref", &v1_ref),
                        ("left_member_ref", &member_ref),
                        ("right_member_ref", &member_ref),
                        (
                            "evidence_policy_ref",
                            &frozen["policies"]["evidence"]["reference"],
                        ),
                        (
                            "selection_policy_ref",
                            &frozen["policies"]["selection"]["reference"],
                        ),
                    ] {
                        assert_eq!(
                            bytes(&parse(&canonical_json(&output[key]))),
                            bytes(&parse(&canonical_json(expected)))
                        );
                    }
                    let native = &output["native_comparison"];
                    assert_eq!(
                        native["schema_version"],
                        "tos_temporal_comparison_result_v1"
                    );
                    assert_eq!(native["source_revision"], revision.0.to_hex());
                    assert_eq!(native["comparison"]["status"], "comparable");
                    assert_eq!(native["comparison"]["relation"], "equal");
                    assert_eq!(native["comparison"]["reasons"], serde_json::json!([]));
                    assert_eq!(
                        native["comparison"]["basis"],
                        "normalized-source-date-envelopes"
                    );
                    for side in ["left", "right"] {
                        let old = &native[side]["claim"]["attributes"]["source_claim"];
                        assert_eq!(member["object"]["calendar"], "proleptic-gregorian");
                        assert_eq!(
                            native[side]["normalized_time"]["calendar"],
                            member["object"]["calendar"]
                        );
                        assert_eq!(
                            bytes(&parse(&canonical_json(old))),
                            bytes(&parse(&canonical_json(&member)))
                        );
                        assert_eq!(
                            bytes(&parse(&canonical_json(
                                &native[side]["normalized_time"]["raw"]
                            ))),
                            bytes(&parse(&canonical_json(&member["object"])))
                        );
                        assert_eq!(
                            bytes(&parse(&canonical_json(&native["request"][side]))),
                            bytes(&locator)
                        );
                    }
                    assert_eq!(
                        fs::read(isolated.path().join(&member_path)).unwrap(),
                        member_raw
                    );
                    for side in ["left", "right"] {
                        assert_eq!(native[side]["normalized_time"]["precision"], "day");
                        assert_eq!(native[side]["normalized_time"]["certainty"], "exact");
                        assert_eq!(
                            native[side]["normalized_time"]["issues"],
                            serde_json::json!([])
                        );
                    }
                    assert_eq!(
                        native["left"]["normalized_time"],
                        native["right"]["normalized_time"]
                    );
                    let mut expected_sources = BTreeSet::new();
                    for side in ["left", "right"] {
                        for carrier in ["claim", "value"] {
                            for source in native[side][carrier]["source_refs"].as_array().unwrap() {
                                expected_sources.insert(source.as_str().unwrap());
                            }
                        }
                    }
                    assert!(!expected_sources.is_empty());
                    assert_eq!(
                        native["source_refs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|source| source.as_str().unwrap())
                            .collect::<Vec<_>>(),
                        expected_sources.into_iter().collect::<Vec<_>>()
                    );
                    assert_eq!(native["authority_boundary"]["performs_assessment"], false);
                    for private in [
                        "owner_evidence",
                        "configuration",
                        "principal",
                        "creation_receipt",
                    ] {
                        assert!(output.get(private).is_none());
                    }
                    Ok(())
                })?;
                temporal_controls.revoked.store(true, Ordering::SeqCst);
                assert!(
                    matches!(view.recheck(), Err(HeldClaimComparisonError::Query(error))
                            if error.code == tos_query::search_v2::SearchV2ErrorCode::StalePolicy)
                );
                temporal_controls.revoked.store(false, Ordering::SeqCst);
                view.recheck()?;
                temporal_controls.cancelled.store(true, Ordering::SeqCst);
                assert!(
                    matches!(view.recheck(), Err(HeldClaimComparisonError::Query(error))
                            if error.code == tos_query::search_v2::SearchV2ErrorCode::Cancelled)
                );
                temporal_controls.cancelled.store(false, Ordering::SeqCst);
                view.recheck()?;
                let mut revoked_owner = revision_config.clone();
                revoked_owner["allowed_operations"] = serde_json::json!([]);
                fs::write(&revision_path, canonical_json(&revoked_owner)).unwrap();
                assert!(view.recheck().is_err());
                fs::write(&revision_path, &revision_config_raw).unwrap();
                view.recheck()?;
                assert!(temporal_held.load(Ordering::SeqCst) > 0);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(temporal_held.load(Ordering::SeqCst), 0);
        assert!(!temporal_controls.revoked.load(Ordering::SeqCst));
        assert!(!temporal_controls.cancelled.load(Ordering::SeqCst));
        assert_eq!(fs::read(&revision_path).unwrap(), revision_config_raw);
        load_worker.finish(deadline, &cancellation).unwrap();
        assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), cv_raw);
        drop(load_worker);
        drop(temporal_authority);

        // BEGIN seven-context held-list fixture: NEW.
        {
            use super::selected_metadata_authority::Authority as ListAuthority;
            use tos_command::temporal_chronology_list::{
                ListEnvelopeAliases, ListManifestAliases, StoredChronologyListBudget,
                StoredChronologyListError, StoredChronologyListView,
                with_loaded_chronology_manifest_envelope, with_loaded_chronology_metadata_list,
            };
            use tos_foundation::query_context::QueryContext;
            use tos_foundation::temporal::TemporalBudget;
            use tos_foundation::{
                canonical_bytes_v1_with_state_budget_and_visits, emit_value_preserved_json,
                parse_json_with_state_budget,
            };
            let fixture_json = JsonLimits {
                max_bytes: 8 * 1024 * 1024,
                max_depth: 64,
                max_visits: 1_000_000,
                max_integer_digits: 4096,
            };
            let small_json = JsonLimits {
                max_bytes: 65_536,
                max_depth: 64,
                max_visits: 8192,
                max_integer_digits: 4096,
            };
            let pure_json = JsonLimits {
                max_bytes: 16_384,
                max_depth: 64,
                max_visits: 512,
                max_integer_digits: 4096,
            };
            assert!(
                member_row_raw.len() <= 65_536 && member_ref["id"].as_str().unwrap().len() <= 1024
            );
            let frozen_record = first_owner_document.root();
            let frozen_subject = frozen_record
                .object_get("subject_ref")
                .unwrap()
                .as_str()
                .unwrap();
            let frozen_predicate = frozen_record
                .object_get("predicate")
                .unwrap()
                .as_str()
                .unwrap();
            assert!(frozen_subject.len() <= 1024 && frozen_predicate.len() <= 1024);
            let negative_subject = "tos.fixture.held-list.absent-subject";
            assert!(selected.graph_input_bytes.len() <= 8 * 1024 * 1024);
            assert_eq!(
                bound.require_source_revision().unwrap(),
                revision.0.to_hex()
            );
            let selected_graph = bound
                .source_for_adapter("reified-bibliographic-claims-v1")
                .unwrap();
            // The same model/bound passed cold admission, including actual scope rows/roots.
            let candidate_count = {
                let mut scopes = selected.expectation.source_scopes.iter().filter(|scope| {
                    scope.source_graph == selected_graph
                        && scope.adapter_profile == "reified-bibliographic-claims-v1"
                });
                let scope = scopes.next().expect("selected Claim scope missing");
                assert!(scopes.next().is_none(), "selected Claim scope ambiguous");
                let count =
                    usize::try_from(scope.node_count).expect("selected candidate count overflow");
                assert!(count > 0 && count.checked_add(1).unwrap() <= 166);
                count
            };
            let event_path =
                "ToS/source-witnesses/history/synthetic-supported-calendar/historical-event.json";
            let event_raw = files.get(event_path).unwrap();
            assert!(event_raw.len() <= 2048);
            let event_document = parse_json_with_state_budget(
                event_raw,
                JsonMode::PublishedStrict,
                small_json,
                1_048_576,
            )
            .unwrap();
            let event_ref_value: Value = serde_json::from_slice(&bytes(
                &tos_command::source_forms::metadata_subject(event_document.root()).unwrap(),
            ))
            .unwrap();
            let actual_event_ref = exact_ref(&event_ref_value);
            assert_eq!(
                event_document
                    .root()
                    .object_get("record_id")
                    .unwrap()
                    .as_str(),
                Some(actual_event_ref.id.as_str())
            );
            assert_eq!(
                event_document
                    .root()
                    .object_get("record_version")
                    .unwrap()
                    .as_u64(),
                Some(actual_event_ref.version.get())
            );
            assert!(actual_event_ref.id.as_str().len() <= 1024);
            let actual_member = exact_ref(&member_ref);
            let actual_second_member = exact_ref(&second_member_ref);
            assert_ne!(actual_member.id, actual_second_member.id);
            let actual_members = [&actual_member, &actual_second_member];
            let frozen_records = [frozen_record, second_owner_document.root()];
            let member_refs = [&member_ref, &second_member_ref];
            // Additional bounded model-packet/callback workspace is debited
            // before allocation and stays charged through every later call.
            b_fixture_retained_bytes = b_fixture_retained_bytes.checked_add(262_144).unwrap();
            b_fixture_retained_visits = b_fixture_retained_visits.checked_add(65_536).unwrap();
            assert!(
                b_fixture_retained_bytes < 16 * 1024 * 1024
                    && b_fixture_retained_visits < 1_000_000
            );
            let model_b_packet_raw = canonical_json(
                &node_b["attributes"]["source_claim"]["qualifiers"]["temporal_claim_carrier"],
            );
            assert!(model_b_packet_raw.len() <= 4096 && model_b_packet_raw.capacity() <= 8192);
            let model_b_state = model_b_packet_raw
                .len()
                .checked_mul(128)
                .and_then(|n| n.checked_add(4096))
                .unwrap();
            assert!(model_b_state <= 1_048_576);
            b_fixture_retained_bytes = b_fixture_retained_bytes.checked_add(model_b_state).unwrap();
            b_fixture_retained_visits = b_fixture_retained_visits.checked_add(8192).unwrap();
            assert!(
                b_fixture_retained_bytes < 16 * 1024 * 1024
                    && b_fixture_retained_visits < 1_000_000
            );
            let model_b_packet = parse_json_with_state_budget(
                &model_b_packet_raw,
                JsonMode::PublishedStrict,
                small_json,
                model_b_state,
            )
            .unwrap();
            let event_wire = serde_json::json!({"id":actual_event_ref.id.as_str(),
        "version":actual_event_ref.version.get(),"digest":actual_event_ref.digest.to_hex()});
            let member_wire = serde_json::json!({"id":actual_member.id.as_str(),
        "version":actual_member.version.get(),"digest":actual_member.digest.to_hex()});
            let rules_raw = canonical_json(&serde_json::json!({"temporal_systems":{
        "tos.trs.synthetic.held-list-civil-year-v1":{"calendar_algorithm":"GregorianProleptic",
        "year_numbering":"astronomical","rules_version":"synthetic-gregorian-civil-year-r1","trs_version":"1"}}}));
            assert!(rules_raw.len() <= 65_536);
            let rules_document = parse_json_with_state_budget(
                &rules_raw,
                JsonMode::PublishedStrict,
                small_json,
                1_048_576,
            )
            .unwrap();
            let rules = rules_document.root();
            // Only an actual returned cursor survives a call; bounded bytes, not a lease.
            let mut returned_cursor: Option<Vec<u8>> = None;
            // The actual selected carrier supplies this preserving-packet
            // digest; raw owner bytes/order are not a digest-domain oracle.
            let mut actual_selected_packet_digests: [Option<Digest256>; 2] = [None; 2];
            for case in ["world_match", "world_cursor", "world_disjoint"] {
                assert!(Instant::now() < deadline);
                let early = matches!(case, "tagged_context" | "world_cursor");
                let has_world = matches!(
                    case,
                    "missing_carrier" | "world_match" | "world_cursor" | "world_disjoint"
                );
                let year = match case {
                    "world_cursor" => "1946",
                    "world_disjoint" => "1947",
                    _ => "1945",
                };
                let mut query_context = QueryContext::default();
                if case == "tagged_context" {
                    query_context.attributed_knowledge = Some(None);
                }

                if has_world {
                    let persisted_assertion =
                        &member["qualifiers"]["temporal_claim_carrier"]["roles"][0]["assertion"];
                    let assertion_native = parse(&canonical_json(persisted_assertion));
                    assert!(
                        emit_value_preserved_json(
                            &assertion_native,
                            JsonLimits {
                                max_bytes: 4096,
                                ..pure_json
                            }
                        )
                        .unwrap()
                        .len()
                            <= 4096
                    );
                    let query_evidence = persisted_assertion.clone();
                    assert!(member_path.len() <= 1024);
                    let query_locator = format!(
                        "{member_path}#line=1/qualifiers/temporal_claim_carrier/roles/0/assertion"
                    );
                    let raw = canonical_json(
                        &serde_json::json!({"profile_ref":"tos_world_civil_year_window_filter_v1",
                "assertion_scope":"SupplementalCarrierOnly","assertion_selection":"ExactlyOnePerPredicate",
                "non_total":"RefusePage","predicate":"Possible","coverage_rule":"GregorianCivilYearHalfOpenV1",
                "about_ref":event_wire,"position":{"type":"CalendarPosition","lexical":year,"resolution":"year",
                    "trs":{"id":"tos.trs.synthetic.held-list-civil-year-v1","version":"1"},
                    "calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical","rules_version":"synthetic-gregorian-civil-year-r1"},
                "basis":{"source":member_wire,"source_locator":query_locator,"raw_evidence":query_evidence}}),
                    );
                    assert!(raw.len() <= 65_536);
                    let doc = parse_json_with_state_budget(
                        &raw,
                        JsonMode::PublishedStrict,
                        small_json,
                        1_048_576,
                    )
                    .unwrap();
                    query_context.world_valid_time = Some(doc.into_root());

                    // No extra codec/evaluator call is hidden in preflight. These are
                    // bounded emit measurements of actual arguments and persisted data.
                    let filter_bytes = emit_value_preserved_json(
                        query_context.world_valid_time.as_ref().unwrap(),
                        pure_json,
                    )
                    .unwrap()
                    .len();
                    let rules_bytes = emit_value_preserved_json(rules, pure_json).unwrap().len();
                    for (member_index, record) in frozen_records.iter().enumerate() {
                        // The actual owner ref has the SAME ID/version but a
                        // prefixed digest and LF. Its measured size conservatively
                        // bounds the native bare-digest claim_ref; no digest-domain
                        // equality is asserted. This covers JSON escaping too.
                        let reference_bytes = canonical_json(member_refs[member_index]).len();
                        assert!(reference_bytes <= 4096);
                        let packet = record
                            .object_get("qualifiers")
                            .unwrap()
                            .object_get("temporal_claim_carrier")
                            .unwrap();
                        let packet_bytes =
                            emit_value_preserved_json(packet, pure_json).unwrap().len();
                        emit_value_preserved_json(
                            record.object_get("qualifiers").unwrap(),
                            pure_json,
                        )
                        .unwrap();
                        // Matches the separately reviewed native reserve:
                        // initial F+R+Ref+P, pre-copy F+P+Ref+2048.
                        // assertion+basis are disjoint subtrees of the ONE packet;
                        // final framing/digests/keys are <2048 bytes (no raw-ID shortcut).
                        let expression_bound = filter_bytes
                            .checked_mul(2)
                            .unwrap()
                            .checked_add(rules_bytes)
                            .unwrap()
                            .checked_add(reference_bytes.checked_mul(2).unwrap())
                            .unwrap()
                            .checked_add(packet_bytes.checked_mul(2).unwrap())
                            .unwrap()
                            .checked_add(2048)
                            .unwrap();
                        assert!(
                            expression_bound <= 9830,
                            "N2 per-pure byte admission must fit without reset"
                        );
                    }
                }
                let negative = case == "missing_carrier";
                let page_size = if negative { 1 } else { 3 };
                let scan_limit = if negative {
                    candidate_count.checked_add(1).unwrap()
                } else {
                    candidate_count
                };
                assert!(page_size <= 16 && scan_limit <= 166);
                let request_raw = canonical_json(
                    &serde_json::json!({"schema_version":"tos_stored_chronology_metadata_list_request_v1",
            "filter":if negative {serde_json::json!({"subject_ref":negative_subject})}
                else {serde_json::json!({"subject_ref":frozen_subject,"predicate":frozen_predicate})},
            "page_size":page_size,"scan_limit":scan_limit,"after":null}),
                );
                assert!(request_raw.len() <= 65_536);
                let mut request = parse_json_with_state_budget(
                    &request_raw,
                    JsonMode::PublishedStrict,
                    small_json,
                    1_048_576,
                )
                .unwrap()
                .into_root();
                if matches!(case, "empty_suffix" | "tagged_context" | "world_cursor") {
                    let raw = returned_cursor.as_ref().unwrap();
                    assert!(raw.len() <= 65_536 && raw.capacity() <= 131_080);
                    let cursor = parse_json_with_state_budget(
                        raw,
                        JsonMode::PublishedStrict,
                        small_json,
                        1_048_576,
                    )
                    .unwrap()
                    .into_root();
                    let JsonValue::Object(ref mut fields) = request else {
                        panic!("request object")
                    };
                    fields
                        .iter_mut()
                        .find(|(key, _)| key.as_str() == Some("after"))
                        .unwrap()
                        .1 = cursor;
                }
                assert!(Instant::now() < deadline);
                let mut list_worker = super::command_form_cases::schemas_with_image(
                    &current_cut,
                    &image,
                    deadline,
                    &cancellation,
                );
                let loaded_list = load_chronology_from_owner(
                    &revision_fs,
                    &current_context,
                    &current_cut,
                    &exact_ref(&v1_ref),
                    &context,
                    &cut,
                    &snapshot,
                    &software,
                    &components,
                    &mut list_worker,
                    ChronologyLimits::default(),
                    deadline,
                    &cancellation,
                )
                .unwrap();
                let selected_members = &loaded_list.bound().chronology().member_claim_revisions;
                assert_eq!(selected_members.len(), 2);
                assert_eq!(&selected_members[0], &actual_member);
                assert_eq!(&selected_members[1], &actual_second_member);
                let selected_cutoff = &loaded_list.bound().chronology().selection.system_as_of;
                assert_eq!(selected_cutoff.value.as_str(), Some("2026-10-19T05:00:00Z"));
                assert!(selected_cutoff.trs.is_none());
                assert!(
                    loaded_list
                        .bound()
                        .chronology()
                        .selection
                        .attributed_knowledge
                        .is_none()
                );
                assert!(
                    loaded_list
                        .bound()
                        .chronology()
                        .selection
                        .source_publication_as_of
                        .is_none()
                );
                assert_eq!(snapshot.current_cutoff.value.as_u64(), Some(1));
                if case == "tagged_context" {
                    assert!(
                        loaded_list
                            .bound()
                            .chronology()
                            .selection
                            .attributed_knowledge
                            .is_none()
                    );
                }
                held_list_fixture_counts[0] += 1;
                let inspect_controls = Arc::new(Controls::default());
                let list_controls = Arc::new(Controls::default());
                let inspect_held = Arc::new(AtomicUsize::new(0));
                let list_held = Arc::new(AtomicUsize::new(0));
                let mut inspect_authority = ListAuthority::for_scope(
                    &bound,
                    "tos.knowledge.node.inspect",
                    "read_only_public_knowledge_inspect_v1",
                    inspect_held.clone(),
                    inspect_controls.clone(),
                );
                let inner_list_authority = ListAuthority::for_scope(
                    &bound,
                    "tos.knowledge.temporal.list_claims",
                    "read_only_public_knowledge_claim_list_v1",
                    list_held.clone(),
                    list_controls.clone(),
                );
                let observed_counts = std::cell::Cell::new([0usize; 4]);
                let mut list_authority = ListCandidateObserver {
                    inner: inner_list_authority,
                    graph: selected_graph,
                    subject: frozen_subject,
                    predicate: frozen_predicate,
                    negative_subject,
                    counts: &observed_counts,
                };
                let mut callback_called = false;
                let list_budget = StoredChronologyListBudget {
                    query: tos_query::InspectBudget {
                        max_open_vm_steps: selected.cold_limits().max_vm_steps,
                        max_read_vm_steps: 20_000_000,
                        max_matches: 16,
                        max_rows: 1000,
                        max_field_bytes: 16_384,
                        max_payload_bytes: 1_048_576,
                        max_decoded_bytes: 8 * 1024 * 1024,
                        max_response_bytes: 2 * 1024 * 1024,
                        json: JsonLimits {
                            max_bytes: 1_048_576,
                            max_depth: 64,
                            max_visits: 1_000_000,
                            max_integer_digits: 4096,
                        },
                    },
                    temporal: TemporalBudget {
                        max_expression_bytes: 49_152,
                        max_members: 24_576,
                        max_pairings: 12_288,
                    },
                    max_members: 2,
                    max_extra_owned_bytes: (16 * 1024 * 1024usize)
                        .checked_sub(b_fixture_retained_bytes)
                        .unwrap(),
                    max_extra_json_visits: 1_000_000usize
                        .checked_sub(b_fixture_retained_visits)
                        .unwrap(),
                    max_verification_owned_bytes: 4 * 1024 * 1024,
                    max_verification_json_visits: 250_000,
                };
                let consume = {
                    |view: &mut StoredChronologyListView<'_>| {
                        callback_called = true;
                        assert!(!early);
                        let observed = observed_counts.get();
                        if case == "empty_suffix" {
                            assert_eq!(observed, [0; 4]);
                        } else {
                            assert_eq!(observed[0], candidate_count);
                            assert!(observed[1] >= 1 && observed[1] <= observed[0]);
                            assert_eq!(
                                observed[2], 2,
                                "filtered uniqueness includes nonmember Claims"
                            );
                            assert_eq!(
                                observed[3], 0,
                                "negative subject absent from all authenticated selected Claims"
                            );
                        }
                        assert_eq!(inspect_held.load(Ordering::SeqCst), 2);
                        assert_eq!(list_held.load(Ordering::SeqCst), 1);
                        view.with_current_bytes(|raw| {
                            assert!(raw.len() <= 2 * 1024 * 1024);
                            let public_doc = parse_json_with_state_budget(
                                raw,
                                JsonMode::PublishedStrict,
                                JsonLimits {
                                    max_bytes: 2 * 1024 * 1024,
                                    ..fixture_json
                                },
                                4 * 1024 * 1024,
                            )
                            .unwrap();
                            let public = public_doc.root();
                            assert_eq!(
                                public.object_get("schema_version").unwrap().as_str(),
                                Some("tos_stored_chronology_metadata_list_result_v1")
                            );
                            assert_eq!(
                                public.object_get("source_revision").unwrap().as_str(),
                                Some(revision.0.to_prefixed().as_str())
                            );
                            assert_eq!(
                                bytes(public.object_get("chronology_ref").unwrap()),
                                bytes(&parse(&canonical_json(&v1_ref)))
                            );
                            assert_eq!(
                                public
                                    .object_get("selected_model_receipt_id")
                                    .unwrap()
                                    .as_str(),
                                Some(bound.owner_receipt_id())
                            );
                            assert_eq!(
                                public.object_get("binding_scope").unwrap().as_str(),
                                Some("OwnerStoredChronologyExactSelectedModelMembers")
                            );
                            assert_eq!(
                                public.object_get("policy_scope").unwrap().as_str(),
                                Some("InlineSourceDefinitionNotAuthorization")
                            );
                            assert_eq!(
                                public.object_get("a3_scope").unwrap().as_str(),
                                Some("NotEstablished")
                            );
                            assert_eq!(
                                public
                                    .object_get("legacy_object_assessment")
                                    .unwrap()
                                    .as_str(),
                                Some("NotExamined")
                            );
                            assert_eq!(
                                public
                                    .object_get("event_extent_inferred")
                                    .unwrap()
                                    .as_bool(),
                                Some(false)
                            );
                            for key in [
                                "owner_evidence",
                                "configuration",
                                "principal",
                                "creation_receipt",
                                "provider_response",
                                "projection",
                            ] {
                                assert!(public.object_get(key).is_none());
                            }
                            let effective_selection = public
                                .object_get("effective_context")
                                .unwrap()
                                .object_get("selection")
                                .unwrap();
                            let effective_cutoff =
                                effective_selection.object_get("system_as_of").unwrap();
                            assert_eq!(
                                effective_cutoff.object_get("value").unwrap().as_str(),
                                Some("2026-10-19T05:00:00Z")
                            );
                            assert!(effective_cutoff.object_get("trs").unwrap().is_null());
                            assert!(
                                effective_selection
                                    .object_get("attributed_knowledge")
                                    .unwrap()
                                    .is_null()
                            );
                            assert!(
                                effective_selection
                                    .object_get("source_publication_as_of")
                                    .unwrap()
                                    .is_null()
                            );
                            if negative {
                                assert_eq!(public.as_object().unwrap().len(), 17);
                                assert_eq!(
                                    public.object_get("outcome").unwrap().as_str(),
                                    Some("NonTotalRefusal")
                                );
                                for key in [
                                    "rows",
                                    "next",
                                    "cursor_scope",
                                    "examined_candidates",
                                    "exhausted",
                                ] {
                                    assert!(public.object_get(key).is_none());
                                }
                                let refusals =
                                    public.object_get("refusals").unwrap().as_array().unwrap();
                                assert_eq!(refusals.len(), 1);
                                assert_eq!(
                                    refusals[0].object_get("domain").unwrap().as_str(),
                                    Some("WorldWindow")
                                );
                                assert_eq!(
                                    refusals[0].object_get("reason").unwrap().as_str(),
                                    Some("MissingTemporalCarrier")
                                );
                                assert_eq!(
                                    bytes(refusals[0].object_get("frozen_member_ref").unwrap()),
                                    bytes(&parse(&canonical_json(&member_ref)))
                                );
                            } else {
                                assert_eq!(public.as_object().unwrap().len(), 22);
                                assert_eq!(
                                    public.object_get("outcome").unwrap().as_str(),
                                    Some("Page")
                                );
                                let empty = matches!(case, "empty_suffix" | "world_disjoint");
                                let rows = public.object_get("rows").unwrap().as_array().unwrap();
                                assert_eq!(rows.len(), if empty { 0 } else { 1 });
                                assert_eq!(
                                    public.object_get("examined_candidates").unwrap().as_u64(),
                                    Some(if case == "empty_suffix" {
                                        0
                                    } else {
                                        candidate_count as u64
                                    })
                                );
                                assert_eq!(
                                    public.object_get("exhausted").unwrap().as_bool(),
                                    Some(case == "empty_suffix")
                                );
                                let next = public.object_get("next").unwrap();
                                if case == "empty_suffix" {
                                    assert!(next.is_null());
                                } else {
                                    assert!(!next.is_null());
                                }
                                for row in rows {
                                    assert_eq!(row.as_object().unwrap().len(), 6);
                                    assert_eq!(
                                        bytes(row.object_get("frozen_member_ref").unwrap()),
                                        bytes(&parse(&canonical_json(&member_ref)))
                                    );
                                    assert_eq!(
                                        row.object_get("provider_claim_digest_profile")
                                            .unwrap()
                                            .as_str(),
                                        Some("SourceRecordDigestV1")
                                    );
                                    for key in ["node_id", "content_revision"] {
                                        assert_eq!(
                                            row.object_get(key).unwrap().as_str(),
                                            locator.object_get(key).unwrap().as_str()
                                        );
                                    }
                                    let source = row
                                        .object_get("carrier")
                                        .unwrap()
                                        .object_get("attributes")
                                        .unwrap()
                                        .object_get("source_claim")
                                        .unwrap();
                                    assert_eq!(bytes(source), bytes(frozen_record));
                                    assert_eq!(bytes(source), bytes(&parse(member_row_raw)));
                                    let packet = source
                                        .object_get("qualifiers")
                                        .unwrap()
                                        .object_get("temporal_claim_carrier")
                                        .unwrap();
                                    let raw_packet =
                                        emit_value_preserved_json(packet, pure_json).unwrap();
                                    assert!(actual_selected_packet_digests[0].is_none());
                                    actual_selected_packet_digests[0] =
                                        Some(Digest256::of_bytes(&raw_packet));
                                    let provider_ref =
                                        row.object_get("provider_claim_ref").unwrap();
                                    assert_eq!(
                                        provider_ref.object_get("id").unwrap().as_str(),
                                        Some(actual_member.id.as_str())
                                    );
                                    assert_eq!(
                                        provider_ref.object_get("version").unwrap().as_u64(),
                                        Some(actual_member.version.get())
                                    );
                                    let source_digest = Digest256::of_bytes(
                                        &canonical_bytes_v1(
                                            source,
                                            CanonicalProfile::SourceRecordDigestV1,
                                            small_json,
                                        )
                                        .unwrap(),
                                    );
                                    assert_eq!(
                                        Digest256::from_prefixed(
                                            provider_ref
                                                .object_get("digest")
                                                .unwrap()
                                                .as_str()
                                                .unwrap()
                                        )
                                        .unwrap(),
                                        source_digest
                                    );
                                    // This is NOT owner-domain digest equality/inequality.
                                    assert_eq!(
                                        source
                                            .object_get("object")
                                            .unwrap()
                                            .object_get("calendar")
                                            .unwrap()
                                            .as_str(),
                                        Some("proleptic-gregorian")
                                    );
                                }
                                let assessments = public
                                    .object_get("world_assessments")
                                    .unwrap()
                                    .as_array()
                                    .unwrap();
                                assert_eq!(assessments.len(), if has_world { 2 } else { 0 });
                                if has_world {
                                    for (member_index, assessed) in assessments.iter().enumerate() {
                                        let actual_member = actual_members[member_index];
                                        let frozen_record = frozen_records[member_index];
                                        let assessment = assessed.object_get("result").unwrap();
                                        assert_eq!(
                                            bytes(
                                                assessed.object_get("frozen_member_ref").unwrap()
                                            ),
                                            bytes(&parse(&canonical_json(
                                                member_refs[member_index]
                                            )))
                                        );
                                        let pure_ref = assessment.object_get("claim_ref").unwrap();
                                        assert_eq!(
                                            pure_ref.object_get("id").unwrap().as_str(),
                                            Some(actual_member.id.as_str())
                                        );
                                        assert_eq!(
                                            pure_ref.object_get("version").unwrap().as_u64(),
                                            Some(actual_member.version.get())
                                        );
                                        assert_eq!(
                                            Digest256::from_hex(
                                                pure_ref
                                                    .object_get("digest")
                                                    .unwrap()
                                                    .as_str()
                                                    .unwrap()
                                            )
                                            .unwrap(),
                                            actual_member.digest
                                        );
                                        assert_eq!(
                                            bytes(
                                                assessment.object_get("preserved_filter").unwrap()
                                            ),
                                            bytes(query_context.world_valid_time.as_ref().unwrap())
                                        );
                                        let filter_raw = emit_value_preserved_json(
                                            query_context.world_valid_time.as_ref().unwrap(),
                                            pure_json,
                                        )
                                        .unwrap();
                                        assert_eq!(
                                            Digest256::from_prefixed(
                                                assessment
                                                    .object_get("query_emitted_json_sha256")
                                                    .unwrap()
                                                    .as_str()
                                                    .unwrap()
                                            )
                                            .unwrap(),
                                            Digest256::of_bytes(&filter_raw)
                                        );
                                        assert_eq!(
                                            assessment.object_get("outcome").unwrap().as_str(),
                                            Some(if case == "world_match" && member_index == 0 {
                                                "Match"
                                            } else {
                                                "NoMatch"
                                            })
                                        );
                                        assert_eq!(
                                            assessment
                                                .object_get("classification")
                                                .unwrap()
                                                .as_str(),
                                            Some(if case == "world_match" && member_index == 0 {
                                                "All"
                                            } else {
                                                "Disjoint"
                                            })
                                        );
                                        assert_eq!(
                                            assessment
                                                .object_get("event_extent_inferred")
                                                .unwrap()
                                                .as_bool(),
                                            Some(false)
                                        );
                                        assert_eq!(
                                            public.object_get("rules_scope").unwrap().as_str(),
                                            Some(
                                                "ExplicitCallerRulesSourceMatchedNotPolicyAdmission"
                                            )
                                        );
                                        let packet = frozen_record
                                            .object_get("qualifiers")
                                            .unwrap()
                                            .object_get("temporal_claim_carrier")
                                            .unwrap();

                                        let packet_digest = Digest256::from_prefixed(
                                            assessment
                                                .object_get("source_packet_emitted_json_sha256")
                                                .unwrap()
                                                .as_str()
                                                .unwrap(),
                                        )
                                        .unwrap();
                                        if member_index == 1 {
                                            // Same genuine selected graph packet, not owner/raw-file bytes.
                                            let emitted = emit_value_preserved_json(
                                                model_b_packet.root(),
                                                pure_json,
                                            )
                                            .unwrap();
                                            assert_eq!(
                                                packet_digest,
                                                Digest256::of_bytes(&emitted)
                                            );
                                        }
                                        if let Some(previous) =
                                            actual_selected_packet_digests[member_index]
                                        {
                                            assert_eq!(packet_digest, previous);
                                        } else {
                                            actual_selected_packet_digests[member_index] =
                                                Some(packet_digest);
                                        }
                                        assert_eq!(
                                            assessment
                                                .object_get("source_assertion")
                                                .unwrap()
                                                .object_get("position")
                                                .unwrap()
                                                .object_get("lexical")
                                                .unwrap()
                                                .as_str(),
                                            Some(["1945", "1946"][member_index])
                                        );
                                        let expected_assertion =
                                            packet.object_get("roles").unwrap().as_array().unwrap()
                                                [0]
                                            .object_get("assertion")
                                            .unwrap();
                                        assert_eq!(
                                            bytes(
                                                assessment.object_get("source_assertion").unwrap()
                                            ),
                                            bytes(expected_assertion)
                                        );
                                        let basis = assessment.object_get("source_basis").unwrap();
                                        let source_ref = basis.object_get("source").unwrap();
                                        assert_eq!(
                                            source_ref.object_get("id").unwrap().as_str(),
                                            Some(actual_event_ref.id.as_str())
                                        );
                                        assert_eq!(
                                            source_ref.object_get("version").unwrap().as_u64(),
                                            Some(actual_event_ref.version.get())
                                        );
                                        assert_eq!(
                                            Digest256::from_hex(
                                                source_ref
                                                    .object_get("digest")
                                                    .unwrap()
                                                    .as_str()
                                                    .unwrap()
                                            )
                                            .unwrap(),
                                            actual_event_ref.digest
                                        );
                                        assert_eq!(
                                            basis
                                                .object_get("raw_evidence")
                                                .unwrap()
                                                .object_get("raw_event_utf8")
                                                .unwrap()
                                                .as_str(),
                                            Some(std::str::from_utf8(event_raw).unwrap())
                                        );
                                        assert_eq!(
                                            basis
                                                .object_get("raw_evidence")
                                                .unwrap()
                                                .object_get("raw_event_sha256")
                                                .unwrap()
                                                .as_str(),
                                            Some(
                                                Digest256::of_bytes(event_raw)
                                                    .to_prefixed()
                                                    .as_str()
                                            )
                                        );
                                    }
                                } else {
                                    assert!(public.object_get("world_filter").unwrap().is_null());
                                    assert!(public.object_get("rules_scope").unwrap().is_null());
                                }
                                if matches!(case, "metadata" | "world_match") {
                                    assert!(returned_cursor.is_none());
                                    let (cursor, _) =
                                        canonical_bytes_v1_with_state_budget_and_visits(
                                            next,
                                            CanonicalProfile::SourceCommandInputV1,
                                            small_json,
                                            1_048_576,
                                        )
                                        .unwrap();
                                    assert!(cursor.len() <= 65_536 && cursor.capacity() <= 131_080);
                                    returned_cursor = Some(cursor);
                                }
                            }
                            Ok(())
                        })?;
                        for controls in [&inspect_controls, &list_controls] {
                            controls.revoked.store(true, Ordering::SeqCst);
                            assert!(
                                matches!(view.recheck(),Err(StoredChronologyListError::Query(error)) if error.code==tos_query::search_v2::SearchV2ErrorCode::StalePolicy)
                            );
                            controls.revoked.store(false, Ordering::SeqCst);
                            view.recheck()?;
                            controls.cancelled.store(true, Ordering::SeqCst);
                            assert!(
                                matches!(view.recheck(),Err(StoredChronologyListError::Query(error)) if error.code==tos_query::search_v2::SearchV2ErrorCode::Cancelled)
                            );
                            controls.cancelled.store(false, Ordering::SeqCst);
                            view.recheck()?;
                        }
                        // This temporary TEST owner drift changes no source or cut.
                        assert!(revision_config_raw.len() <= 65_536);
                        let mut drift = revision_config.clone();
                        drift["allowed_operations"] = serde_json::json!([]);
                        fs::write(&revision_path, canonical_json(&drift)).unwrap();
                        assert!(view.recheck().is_err());
                        fs::write(&revision_path, &revision_config_raw).unwrap();
                        view.recheck()?;
                        assert_eq!(inspect_held.load(Ordering::SeqCst), 2);
                        assert_eq!(list_held.load(Ordering::SeqCst), 1);
                        Ok(())
                    }
                };
                let result = if case == "world_match" {
                    // Exact original59 full declared input, excluding oracle expected/
                    // instructions. Actual stored source supplies every binding.
                    assert_eq!(
                        Digest256::of_bytes(include_bytes!(
                            "../foundation-wave-v1/oracles/temporal-acceptance-vectors.json"
                        )),
                        Digest256::from_hex(
                            "fda08fd958914a3605afab12e7b78012a3f5534ecfdffe2ab31cb2cb3a26eecd"
                        )
                        .unwrap()
                    );
                    let original_input = br#"{"claim_world_valid_times":{"claim-A@r1":"1945","claim-B@r1":"1946"},"operation":"list_claims","request_envelope":{"source_snapshot_ref":{"revision_fence":"source-rev-42","snapshot_id":"S1"},"temporal":{"chronology_version":"CV1","world_valid_time":{"position":"1945","trs_ref":"TRS:GregorianProleptic:fixture-v1"}}},"snapshot_manifest":{"chronology_version_id":"CV1","frozen_knowledge_selection":{"member_claim_revisions":["claim-A@r1","claim-B@r1"],"source_revision_fence":"source-rev-42","source_snapshot_ref":{"snapshot_id":"S1"},"system_state":{"cutoff":"2026-10-19T05:00:00Z","type":"SystemAsOf"}}},"source_profile":"ToS-Temporal-Core-Contract-v1","subject":"event-A"}"#;
                    assert_eq!(original_input.len(), 674);
                    assert_eq!(
                        Digest256::of_bytes(original_input),
                        Digest256::from_hex(
                            "db09b07dcdc7dab3d59051fd3eafe14e742b3fc918d90b32b6e571e5f29f60be"
                        )
                        .unwrap()
                    );
                    // Fixed source-checked input gets a smaller existing-parser
                    // state window, fully prepaid before parsing, not a peak guess.
                    let original_parse_bytes = original_input
                        .len()
                        .checked_mul(128)
                        .and_then(|n| n.checked_add(4096))
                        .unwrap();
                    assert!(original_parse_bytes <= 1_048_576);
                    // Prepay the new caller-input parse/state and fixed typed
                    // ref/hash/header copies from the SAME parent allowance.
                    let mut envelope_budget = list_budget;
                    envelope_budget.max_extra_owned_bytes = envelope_budget
                        .max_extra_owned_bytes
                        .checked_sub(
                            original_parse_bytes
                                .checked_add(std::mem::size_of::<tos_foundation::JsonDocument>())
                                .and_then(|n| n.checked_add(12_288))
                                .unwrap(),
                        )
                        .unwrap();
                    envelope_budget.max_extra_json_visits = envelope_budget
                        .max_extra_json_visits
                        .checked_sub(small_json.max_visits)
                        .unwrap();
                    let original = parse_json_with_state_budget(
                        original_input,
                        JsonMode::PublishedStrict,
                        small_json,
                        original_parse_bytes,
                    )
                    .unwrap();
                    assert!(v1_ref["id"].as_str().unwrap().len() <= 1024);
                    let chronology_ref = exact_ref(&v1_ref);
                    // These are explicit synthetic aliases to actual supplied
                    // source identities. They are not name, TRS or rights admission.
                    let aliases = ListManifestAliases {
                        envelope: ListEnvelopeAliases {
                            snapshot: ("S1", &snapshot.reference),
                            revision_fence: ("source-rev-42", revision),
                            chronology: ("CV1", &chronology_ref),
                            world_about: ("event-A", &actual_event_ref),
                            world_trs: (
                                "TRS:GregorianProleptic:fixture-v1",
                                "tos.trs.synthetic.held-list-civil-year-v1",
                                "1",
                            ),
                        },
                        members: [
                            ("claim-A@r1", &actual_member),
                            ("claim-B@r1", &actual_second_member),
                        ],
                    };
                    // Borrow the existing filter so the unchanged callback can
                    // compare it too; no duplicate fixture-owned world clone.
                    // The entry prepays its retained typed context clone.
                    let world = query_context.world_valid_time.as_ref().unwrap();
                    with_loaded_chronology_manifest_envelope(
                        loaded_list,
                        &mut cold,
                        &bound,
                        &mut inspect_authority,
                        &mut list_authority,
                        &snapshot,
                        original.root(),
                        &aliases,
                        &request,
                        world,
                        rules,
                        envelope_budget,
                        deadline,
                        &cancellation,
                        consume,
                    )
                } else if case == "world_disjoint" {
                    use tos_command::temporal_chronology_list::{
                        ListManifestEnvelopeAliases, ListManifestSliceAliases,
                        with_loaded_chronology_manifest_slice_envelope,
                    };
                    // Full genuine N2, synthetic1947 query. Fixed-two original59
                    // world_match and the direct valid1946 cursor case stay intact.
                    let mut general_budget = list_budget;
                    general_budget.max_extra_owned_bytes = general_budget
                        .max_extra_owned_bytes
                        .checked_sub(1_048_576)
                        .unwrap();
                    general_budget.max_extra_json_visits = general_budget
                        .max_extra_json_visits
                        .checked_sub(8192)
                        .unwrap();
                    let input_raw = canonical_json(&serde_json::json!({
                        "operation":"list_claims","source_profile":"ToS-Temporal-Core-Contract-v1","subject":"event-A",
                        "request_envelope":{"source_snapshot_ref":{"snapshot_id":"S1","revision_fence":"source-rev-42"},
                            "temporal":{"chronology_version":"CV1","world_valid_time":{"position":"1947","trs_ref":"TRS:GregorianProleptic:fixture-v1"}}},
                        "snapshot_manifest":{"chronology_version_id":"CV1","frozen_knowledge_selection":{
                            "system_state":{"type":"SystemAsOf","cutoff":"2026-10-19T05:00:00Z"},
                            "source_snapshot_ref":{"snapshot_id":"S1"},"source_revision_fence":"source-rev-42",
                            "member_claim_revisions":["claim-A@r1","claim-B@r1"]}},
                        "claim_world_valid_times":{"claim-A@r1":"1945","claim-B@r1":"1946"}}));
                    assert!(input_raw.len() <= 2048 && input_raw.capacity() <= 65_536);
                    let input_state = input_raw
                        .len()
                        .checked_mul(128)
                        .and_then(|n| n.checked_add(4096))
                        .unwrap();
                    assert!(input_state <= 266_240);
                    let input_document = parse_json_with_state_budget(
                        &input_raw,
                        JsonMode::PublishedStrict,
                        small_json,
                        input_state,
                    )
                    .unwrap();
                    let chronology_ref = exact_ref(&v1_ref);
                    let aliases = ListManifestSliceAliases {
                        envelope: ListManifestEnvelopeAliases::World(ListEnvelopeAliases {
                            snapshot: ("S1", &snapshot.reference),
                            revision_fence: ("source-rev-42", revision),
                            chronology: ("CV1", &chronology_ref),
                            world_about: ("event-A", &actual_event_ref),
                            world_trs: (
                                "TRS:GregorianProleptic:fixture-v1",
                                "tos.trs.synthetic.held-list-civil-year-v1",
                                "1",
                            ),
                        }),
                        members: &[
                            ("claim-A@r1", &actual_member),
                            ("claim-B@r1", &actual_second_member),
                        ],
                    };
                    with_loaded_chronology_manifest_slice_envelope(
                        loaded_list,
                        &mut cold,
                        &bound,
                        &mut inspect_authority,
                        &mut list_authority,
                        &snapshot,
                        input_document.root(),
                        &aliases,
                        &request,
                        query_context.world_valid_time.as_ref(),
                        Some(rules),
                        general_budget,
                        deadline,
                        &cancellation,
                        consume,
                    )
                } else {
                    with_loaded_chronology_metadata_list(
                        loaded_list,
                        &mut cold,
                        &bound,
                        &mut inspect_authority,
                        &mut list_authority,
                        &snapshot,
                        &query_context,
                        if has_world { Some(rules) } else { None },
                        &request,
                        list_budget,
                        deadline,
                        &cancellation,
                        consume,
                    )
                };
                if early {
                    assert!(matches!(
                        result,
                        Err(StoredChronologyListError::InvalidRequest(_))
                    ));
                    assert!(!callback_called);
                    assert_eq!(observed_counts.get(), [0; 4]);
                    held_list_fixture_counts[2] += 1;
                } else {
                    result.unwrap();
                    assert!(callback_called);
                    held_list_fixture_counts[1] += 3;
                }
                assert_eq!(inspect_held.load(Ordering::SeqCst), 0);
                assert_eq!(list_held.load(Ordering::SeqCst), 0);
                assert!(
                    !inspect_controls.revoked.load(Ordering::SeqCst)
                        && !inspect_controls.cancelled.load(Ordering::SeqCst)
                );
                assert!(
                    !list_controls.revoked.load(Ordering::SeqCst)
                        && !list_controls.cancelled.load(Ordering::SeqCst)
                );
                assert_eq!(fs::read(&revision_path).unwrap(), revision_config_raw);
                assert_eq!(
                    fs::read(isolated.path().join(&member_path)).unwrap(),
                    member_raw
                );
                assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), cv_raw);
                drop(inspect_authority);
                drop(list_authority);
                list_worker.finish(deadline, &cancellation).unwrap();
                drop(list_worker);
                assert_eq!(
                    bound.require_source_revision().unwrap(),
                    revision.0.to_hex()
                );
                eprintln!(
                    "held list fixture source=NEW case={} phase=complete early_cursor_refusal={}",
                    case, early
                );
            }
            assert!(
                returned_cursor
                    .as_ref()
                    .is_some_and(|raw| raw.len() <= 65_536)
            );
            // ONE extra genuine owner for an honest entry-level prefix Resource.
            // All earlier NEW leases/workers are dropped. No model/writer/cut added.
            {
                use tos_command::temporal_chronology_list::{
                    ListManifestEnvelopeAliases, ListManifestSliceAliases,
                    with_loaded_chronology_manifest_slice_envelope,
                };
                let case = "fixed_header_resource";
                let mut prepared_budget = StoredChronologyListBudget {
                    query: tos_query::InspectBudget {
                        max_open_vm_steps: selected.cold_limits().max_vm_steps,
                        max_read_vm_steps: 20_000_000,
                        max_matches: 16,
                        max_rows: 1000,
                        max_field_bytes: 16_384,
                        max_payload_bytes: 1_048_576,
                        max_decoded_bytes: 8 * 1024 * 1024,
                        max_response_bytes: 2 * 1024 * 1024,
                        json: JsonLimits {
                            max_bytes: 1_048_576,
                            max_depth: 64,
                            max_visits: 1_000_000,
                            max_integer_digits: 4096,
                        },
                    },
                    temporal: TemporalBudget {
                        max_expression_bytes: 49_152,
                        max_members: 24_576,
                        max_pairings: 12_288,
                    },
                    max_members: 2,
                    max_extra_owned_bytes: (16 * 1024 * 1024usize)
                        .checked_sub(b_fixture_retained_bytes)
                        .and_then(|n| n.checked_sub(1_048_576))
                        .unwrap(),
                    max_extra_json_visits: 1_000_000usize
                        .checked_sub(b_fixture_retained_visits)
                        .and_then(|n| n.checked_sub(8192))
                        .unwrap(),
                    max_verification_owned_bytes: 4 * 1024 * 1024,
                    max_verification_json_visits: 250_000,
                };
                assert!(
                    prepared_budget.max_extra_owned_bytes
                        > prepared_budget.max_verification_owned_bytes + 1
                );
                assert!(
                    prepared_budget.max_extra_json_visits
                        > prepared_budget.max_verification_json_visits
                );
                // The 1MiB/8192 caller window above is already deducted from
                // ACTUAL B remaining allowance before any of these allocations.
                let input_raw = br#"{"claim_world_valid_times":{"claim-A@r1":"1945","claim-B@r1":"1946"},"operation":"list_claims","request_envelope":{"source_snapshot_ref":{"revision_fence":"source-rev-42","snapshot_id":"S1"},"temporal":{"chronology_version":"CV1","world_valid_time":{"position":"1945","trs_ref":"TRS:GregorianProleptic:fixture-v1"}}},"snapshot_manifest":{"chronology_version_id":"CV1","frozen_knowledge_selection":{"member_claim_revisions":["claim-A@r1","claim-B@r1"],"source_revision_fence":"source-rev-42","source_snapshot_ref":{"snapshot_id":"S1"},"system_state":{"cutoff":"2026-10-19T05:00:00Z","type":"SystemAsOf"}}},"source_profile":"ToS-Temporal-Core-Contract-v1","subject":"event-A"}"#;
                assert_eq!(input_raw.len(), 674);
                let input_state = input_raw
                    .len()
                    .checked_mul(128)
                    .and_then(|n| n.checked_add(4096))
                    .unwrap();
                assert!(input_state <= 266_240);
                let input_document = parse_json_with_state_budget(
                    input_raw,
                    JsonMode::PublishedStrict,
                    small_json,
                    input_state,
                )
                .unwrap();
                let persisted_assertion =
                    &member["qualifiers"]["temporal_claim_carrier"]["roles"][0]["assertion"];
                let query_locator = format!(
                    "{member_path}#line=1/qualifiers/temporal_claim_carrier/roles/0/assertion"
                );
                let world_raw = canonical_json(&serde_json::json!({
                    "profile_ref":"tos_world_civil_year_window_filter_v1","assertion_scope":"SupplementalCarrierOnly",
                    "assertion_selection":"ExactlyOnePerPredicate","non_total":"RefusePage","predicate":"Possible",
                    "coverage_rule":"GregorianCivilYearHalfOpenV1","about_ref":event_wire,
                    "position":{"type":"CalendarPosition","lexical":"1945","resolution":"year",
                        "trs":{"id":"tos.trs.synthetic.held-list-civil-year-v1","version":"1"},
                        "calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical","rules_version":"synthetic-gregorian-civil-year-r1"},
                    "basis":{"source":member_wire,"source_locator":query_locator,"raw_evidence":persisted_assertion}}));
                assert!(world_raw.len() <= 2048 && world_raw.capacity() <= 65_536);
                let world_state = world_raw
                    .len()
                    .checked_mul(128)
                    .and_then(|n| n.checked_add(4096))
                    .unwrap();
                assert!(world_state <= 266_240);
                let world_document = parse_json_with_state_budget(
                    &world_raw,
                    JsonMode::PublishedStrict,
                    small_json,
                    world_state,
                )
                .unwrap();
                let request_raw = canonical_json(&serde_json::json!({
                    "schema_version":"tos_stored_chronology_metadata_list_request_v1",
                    "filter":{"subject_ref":frozen_subject,"predicate":frozen_predicate},
                    "page_size":3,"scan_limit":candidate_count,"after":null}));
                assert!(request_raw.len() <= 2048 && request_raw.capacity() <= 65_536);
                let request_state = request_raw
                    .len()
                    .checked_mul(128)
                    .and_then(|n| n.checked_add(4096))
                    .unwrap();
                assert!(request_state <= 266_240);
                assert!(
                    input_state
                        .checked_add(world_state)
                        .and_then(|n| n.checked_add(request_state))
                        .and_then(|n| n.checked_add(3 * 65_536 + 65_536))
                        .unwrap()
                        <= 1_048_576
                );
                let request_document = parse_json_with_state_budget(
                    &request_raw,
                    JsonMode::PublishedStrict,
                    small_json,
                    request_state,
                )
                .unwrap();
                let chronology_ref = exact_ref(&v1_ref);
                let aliases = ListManifestSliceAliases {
                    envelope: ListManifestEnvelopeAliases::World(ListEnvelopeAliases {
                        snapshot: ("S1", &snapshot.reference),
                        revision_fence: ("source-rev-42", revision),
                        chronology: ("CV1", &chronology_ref),
                        world_about: ("event-A", &actual_event_ref),
                        world_trs: (
                            "TRS:GregorianProleptic:fixture-v1",
                            "tos.trs.synthetic.held-list-civil-year-v1",
                            "1",
                        ),
                    }),
                    members: &[
                        ("claim-A@r1", &actual_member),
                        ("claim-B@r1", &actual_second_member),
                    ],
                };
                let mut list_worker = super::command_form_cases::schemas_with_image(
                    &current_cut,
                    &image,
                    deadline,
                    &cancellation,
                );
                let loaded_list = load_chronology_from_owner(
                    &revision_fs,
                    &current_context,
                    &current_cut,
                    &exact_ref(&v1_ref),
                    &context,
                    &cut,
                    &snapshot,
                    &software,
                    &components,
                    &mut list_worker,
                    ChronologyLimits::default(),
                    deadline,
                    &cancellation,
                )
                .unwrap();
                let selected_members = &loaded_list.bound().chronology().member_claim_revisions;
                assert_eq!(selected_members.len(), 2);
                assert_eq!(&selected_members[0], &actual_member);
                assert_eq!(&selected_members[1], &actual_second_member);
                let selected_cutoff = &loaded_list.bound().chronology().selection.system_as_of;
                assert_eq!(selected_cutoff.value.as_str(), Some("2026-10-19T05:00:00Z"));
                assert!(selected_cutoff.trs.is_none());
                assert!(
                    loaded_list
                        .bound()
                        .chronology()
                        .selection
                        .attributed_knowledge
                        .is_none()
                );
                assert!(
                    loaded_list
                        .bound()
                        .chronology()
                        .selection
                        .source_publication_as_of
                        .is_none()
                );
                assert_eq!(snapshot.current_cutoff.value.as_u64(), Some(1));
                if case == "tagged_context" {
                    assert!(
                        loaded_list
                            .bound()
                            .chronology()
                            .selection
                            .attributed_knowledge
                            .is_none()
                    );
                }
                held_list_fixture_counts[0] += 1;
                let inspect_controls = Arc::new(Controls::default());
                let list_controls = Arc::new(Controls::default());
                let inspect_held = Arc::new(AtomicUsize::new(0));
                let list_held = Arc::new(AtomicUsize::new(0));
                let mut inspect_authority = ListAuthority::for_scope(
                    &bound,
                    "tos.knowledge.node.inspect",
                    "read_only_public_knowledge_inspect_v1",
                    inspect_held.clone(),
                    inspect_controls.clone(),
                );
                let inner_list_authority = ListAuthority::for_scope(
                    &bound,
                    "tos.knowledge.temporal.list_claims",
                    "read_only_public_knowledge_claim_list_v1",
                    list_held.clone(),
                    list_controls.clone(),
                );
                let observed_counts = std::cell::Cell::new([0usize; 4]);
                let mut list_authority = ListCandidateObserver {
                    inner: inner_list_authority,
                    graph: selected_graph,
                    subject: frozen_subject,
                    predicate: frozen_predicate,
                    negative_subject,
                    counts: &observed_counts,
                };
                // All non-byte quotas remain the same valid N2 operation. Only
                // the prefix's available owned byte is intentionally insufficient.
                assert_eq!(prepared_budget.max_members, 2);
                assert_eq!(prepared_budget.temporal.max_expression_bytes / 5, 9830);
                assert_eq!(prepared_budget.temporal.max_members / 5, 4915);
                assert_eq!(prepared_budget.temporal.max_pairings / 5, 2457);
                assert!(candidate_count.checked_mul(2).unwrap() <= 333);
                prepared_budget.max_extra_owned_bytes = prepared_budget
                    .max_verification_owned_bytes
                    .checked_add(1)
                    .unwrap();
                let mut callback_called = false;
                let result = with_loaded_chronology_manifest_slice_envelope(
                    loaded_list,
                    &mut cold,
                    &bound,
                    &mut inspect_authority,
                    &mut list_authority,
                    &snapshot,
                    input_document.root(),
                    &aliases,
                    request_document.root(),
                    Some(world_document.root()),
                    Some(rules),
                    prepared_budget,
                    deadline,
                    &cancellation,
                    |_| {
                        callback_called = true;
                        Ok(())
                    },
                );
                assert!(matches!(
                    result,
                    Err(StoredChronologyListError::Resource(_))
                ));
                assert!(!callback_called);
                assert_eq!(observed_counts.get(), [0; 4]);
                assert_eq!(inspect_held.load(Ordering::SeqCst), 0);
                assert_eq!(list_held.load(Ordering::SeqCst), 0);
                for controls in [&inspect_controls, &list_controls] {
                    assert!(!controls.revoked.load(Ordering::SeqCst));
                    assert!(!controls.cancelled.load(Ordering::SeqCst));
                }
                assert_eq!(fs::read(&revision_path).unwrap(), revision_config_raw);
                assert_eq!(
                    fs::read(isolated.path().join(&member_path)).unwrap(),
                    member_raw
                );
                assert_eq!(fs::read(isolated.path().join(relative)).unwrap(), cv_raw);
                drop(inspect_authority);
                drop(list_authority);
                list_worker.finish(deadline, &cancellation).unwrap();
                drop(list_worker);
                general_fixed_header_resource_count += 1;
                eprintln!(
                    "general manifest source=NEW case=fixed_header_resource phase=complete queried=0 acquired=0"
                );
            }
        }
        // G10 attributed CV and finite Boolean consumer, only after every old
        // B/general holder and schema worker has ended and owner state restored.
        {
            g10_byte_limit.set(
                (16 * 1024 * 1024usize)
                    .checked_sub(b_fixture_retained_bytes)
                    .and_then(|n| n.checked_sub(G10_FIXED_BYTES))
                    .and_then(|n| n.checked_add(1_048_576))
                    .and_then(|n| n.checked_add(g10_applied_bytes.get()))
                    .unwrap(),
            );
            g10_visit_limit.set(
                1_000_000usize
                    .checked_sub(b_fixture_retained_visits)
                    .and_then(|n| n.checked_sub(G10_FIXED_VISITS))
                    .and_then(|n| n.checked_add(8192))
                    .and_then(|n| n.checked_add(g10_applied_visits.get()))
                    .unwrap(),
            );
            assert!(g10_used_bytes.get() <= g10_byte_limit.get());
            assert!(g10_used_visits.get() <= g10_visit_limit.get());
            let saved_b_cv_path = relative;
            let saved_b_cv_raw = &cv_raw;
            let saved_b_ref = &v1_ref;
            use super::selected_metadata_authority::Authority as ListAuthority;
            use tos_command::temporal_chronology_list::{
                AttributedCutoffBindings, StoredChronologyListBudget, StoredChronologyListError,
                with_loaded_chronology_attributed_knowledge_by_cutoff,
            };
            use tos_foundation::temporal::TemporalBudget;
            // Fixed-stack optional diagnostic owner; no payload/config/ID data.
            struct G10BudgetRecorder {
                slot: std::cell::Cell<&'static str>,
                starts: std::cell::Cell<u8>,
                pending_sql: std::cell::Cell<bool>,
                first: std::cell::Cell<Option<(&'static str, &'static str, [Option<u64>; 4])>>,
            }
            impl G10BudgetRecorder {
                fn observe(
                    &self,
                    phase: &'static str,
                    site: &'static str,
                    counts: [Option<u64>; 4],
                ) {
                    if phase == "begin" {
                        self.slot.set(site);
                        self.pending_sql.set(false);
                        // Finite four lookups plus one list; no per-row events.
                        self.starts.set(self.starts.get().saturating_add(1));
                    } else if phase == "sql_interrupt_observed" {
                        // An observed SQL interruption is pending provenance, not
                        // a first fault. The existing final read-VM check wins.
                        self.pending_sql.set(true);
                    } else if matches!(phase, "fault" | "fallback") && self.first.get().is_none() {
                        let site = if phase == "fallback" && self.pending_sql.get() {
                            "sql_interrupt_unclassified"
                        } else {
                            site
                        };
                        self.first.set(Some((self.slot.get(), site, counts)));
                    }
                }
                fn write_best_effort(&self) {
                    let (slot, site, counts) =
                        self.first
                            .get()
                            .unwrap_or(("not_started", "not_recorded", [None; 4]));
                    // Closed labels <=40B, slot<=11B, four optional u64 values
                    // <=104B total, fixed framing <128B: one line <512B.
                    // No owned format String or panicking stderr unwrap.
                    let mut stderr = std::io::stderr().lock();
                    let _ = std::io::Write::write_fmt(
                        &mut stderr,
                        format_args!(
                            "G10 inspect budget slot={} site={} counts={:?} starts={}\n",
                            slot,
                            site,
                            counts,
                            self.starts.get()
                        ),
                    );
                }
            }
            struct G10BudgetAuthority<'a, A> {
                inner: A,
                recorder: &'a G10BudgetRecorder,
            }
            impl<'hold, A: tos_query::InspectCurrentAuthority<'hold>>
                tos_query::InspectCurrentAuthority<'hold> for G10BudgetAuthority<'_, A>
            {
                fn observe_inspect_budget(
                    &self,
                    phase: &'static str,
                    site: &'static str,
                    counts: [Option<u64>; 4],
                ) {
                    self.recorder.observe(phase, site, counts);
                    self.inner.observe_inspect_budget(phase, site, counts);
                }
                fn disclosure_metadata_state_upper_bound(
                    &self,
                ) -> Result<usize, tos_query::search_v2::SearchV2Error> {
                    self.inner.disclosure_metadata_state_upper_bound()
                }
                fn abort_probe(&self) -> Option<std::sync::Arc<dyn tos_query::AbortProbe>> {
                    self.inner.abort_probe()
                }
                fn policy_binding(&self) -> tos_query::search_v2::CurrentPolicyBinding {
                    self.inner.policy_binding()
                }
                fn disclosure_scope(&self) -> tos_query::IndexedDisclosureScope {
                    self.inner.disclosure_scope()
                }
                fn check_selected(&mut self) -> Result<(), tos_query::search_v2::SearchV2Error> {
                    self.inner.check_selected()
                }
                fn authorize_current(
                    &mut self,
                    carrier: &tos_query::InspectedCarrier,
                ) -> Result<(), tos_query::search_v2::SearchV2Error> {
                    self.inner.authorize_current(carrier)
                }
                fn acquire_disclosure(
                    &mut self,
                    scope: &tos_query::IndexedDisclosureScope,
                    observed: &[tos_query::ObservedInspectCarrier],
                ) -> Result<
                    Box<dyn tos_query::InspectDisclosureLease + 'hold>,
                    tos_query::search_v2::SearchV2Error,
                > {
                    self.inner.acquire_disclosure(scope, observed)
                }
                // All unrelated original/source grant methods remain default deny.
            }
            struct G10CandidateObserver<'a> {
                inner: super::selected_metadata_authority::Authority,
                graph: &'a str,
                subject: &'a str,
                predicate: &'a str,
                negative_subject: &'a str,
                counts: &'a std::cell::Cell<[usize; 4]>,
            }
            impl<'hold> tos_query::InspectCurrentAuthority<'hold> for G10CandidateObserver<'_> {
                fn observe_inspect_budget(
                    &self,
                    phase: &'static str,
                    site: &'static str,
                    counts: [Option<u64>; 4],
                ) {
                    tos_query::InspectCurrentAuthority::observe_inspect_budget(
                        &self.inner,
                        phase,
                        site,
                        counts,
                    );
                }
                fn disclosure_metadata_state_upper_bound(
                    &self,
                ) -> Result<usize, tos_query::search_v2::SearchV2Error> {
                    tos_query::InspectCurrentAuthority::disclosure_metadata_state_upper_bound(
                        &self.inner,
                    )
                }
                fn abort_probe(&self) -> Option<std::sync::Arc<dyn tos_query::AbortProbe>> {
                    tos_query::InspectCurrentAuthority::abort_probe(&self.inner)
                }
                fn policy_binding(&self) -> tos_query::search_v2::CurrentPolicyBinding {
                    tos_query::InspectCurrentAuthority::policy_binding(&self.inner)
                }
                fn disclosure_scope(&self) -> tos_query::IndexedDisclosureScope {
                    tos_query::InspectCurrentAuthority::disclosure_scope(&self.inner)
                }
                fn check_selected(&mut self) -> Result<(), tos_query::search_v2::SearchV2Error> {
                    tos_query::InspectCurrentAuthority::check_selected(&mut self.inner)
                }
                fn authorize_current(
                    &mut self,
                    carrier: &tos_query::InspectedCarrier,
                ) -> Result<(), tos_query::search_v2::SearchV2Error> {
                    tos_query::InspectCurrentAuthority::authorize_current(
                        &mut self.inner,
                        carrier,
                    )?;
                    fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
                        value
                            .as_object()?
                            .iter()
                            .find_map(|(key, value)| (key.as_str() == Some(name)).then_some(value))
                    }
                    let payload = &carrier.payload;
                    assert_eq!(
                        field(payload, "source_graph").and_then(JsonValue::as_str),
                        Some(self.graph)
                    );
                    let mut counts = self.counts.get();
                    counts[0] = counts[0]
                        .checked_add(1)
                        .expect("candidate counter overflow");
                    if field(payload, "kind_id").and_then(JsonValue::as_str) == Some("claim") {
                        counts[1] = counts[1].checked_add(1).expect("Claim counter overflow");
                        let source =
                            field(field(payload, "attributes").unwrap(), "source_claim").unwrap();
                        let subject = field(source, "subject_ref")
                            .and_then(JsonValue::as_str)
                            .unwrap();
                        let predicate = field(source, "predicate")
                            .and_then(JsonValue::as_str)
                            .unwrap();
                        if predicate == self.predicate {
                            counts[2] =
                                counts[2].checked_add(1).expect("matching counter overflow");
                        }
                        if subject == self.negative_subject {
                            counts[3] =
                                counts[3].checked_add(1).expect("negative counter overflow");
                        }
                    }
                    self.counts.set(counts);
                    Ok(())
                }
                fn acquire_disclosure(
                    &mut self,
                    scope: &tos_query::IndexedDisclosureScope,
                    observed: &[tos_query::ObservedInspectCarrier],
                ) -> Result<
                    Box<dyn tos_query::InspectDisclosureLease + 'hold>,
                    tos_query::search_v2::SearchV2Error,
                > {
                    tos_query::InspectCurrentAuthority::acquire_disclosure(
                        &mut self.inner,
                        scope,
                        observed,
                    )
                }
            }

            // Full-authored map is needed by this EXISTING writer/cut mechanism.
            // It is a separate delegated source context, <=existing32MiB; no model.
            let frozen = serde_json::json!({"schema_version":"tos_frozen_chronology_v1",
            "source_snapshot":{"id":snapshot_name,"revision":revision.0.to_prefixed()},
            "system_as_of":{"value":"2026-10-19T05:00:00Z","trs":null},"attributed_knowledge":{"knower":g10_kref_value["id"],"cutoff":{"value":"1800","trs":g10_rref_value}},"source_publication_as_of":null,
            "selection_mode":"ExplicitExactMembers","members":[member_ref,second_member_ref,g10_receipt["claims"][0],g10_receipt["claims"][1]],"policies":{
                "evidence":inline_policy("Evidence","ExactPinnedMemberEvidence-v1"),
                "selection":inline_policy("Selection","ExplicitExactMembers-v1")}});
            let relative =
                "ToS/source-witnesses/chronologies/synthetic-g10/chronology-version.json";
            let identity = "tos.chronology-version.synthetic-g10";
            let record = serde_json::json!({"schema_version":"tos_chronology_version_record_v1","record_type":"chronology-version",
            "record_id":identity,"record_version":1,"preferred_label":"Synthetic G10 attributed source custody",
            "variant_labels":[],"identity_status":"provisional","source_refs":[g10_evidence],"external_identifiers":[],
            "same_as_posture":"no_equivalence_claim","visibility":"public_metadata_only","semantic_content":frozen});
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
            let owner = isolated.path().join("g10-CV-create-owner.json");
            let config = serde_json::json!({"schema_version":"tos_local_profile_create_owner_v1","uid":uid,
            "principal_id":"software:test-fixture","maker_type":"software","source_root":isolated.path(),
            "source_path":relative,"record_id":identity,"profile_type_id":"tos.entity.chronology-version",
            "authority_ref":"synthetic-test-only:g10-CV-creation","allowed_form_ids":["tos.form.creation.g10-cv-name"],
            "allowed_operations":["source.create"],"expires_at":"2099-01-01T00:00:00Z",
            "provenance_event_id":"tos.event.synthetic-g10-cv-create"});
            let config_raw = canonical_json(&config);
            assert!(config_raw.len() <= 8192);
            fs::write(&owner, &config_raw).unwrap();
            fs::set_permissions(&owner, fs::Permissions::from_mode(0o600)).unwrap();
            let creation_fs =
                CreationFilesystem::select_isolated(&isolated, &owner, deadline, &cancellation)
                    .unwrap();
            let mut request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"prepare-create","record":record,
            "forms":[{"form_id":"tos.form.creation.g10-cv-name","field_id":"metadata.preferred-name"}]});
            let mut g10_creation_context =
                cut_context(&current_files, config_raw, canonical_json(&request), tp);
            g10_creation_context.effective_uid = u64::from(uid);
            let mut worker = super::command_form_cases::schemas_with_image(
                &current_cut,
                &image,
                deadline,
                &cancellation,
            );
            let prepared = prepare_source_creation_from_captures(
                &g10_creation_context,
                &current_cut,
                &software,
                &components,
                &mut worker,
                deadline,
                &cancellation,
            )
            .unwrap();
            let preview = prepared.preview().unwrap();
            assert_prepared_file_refs(&preview, prepared.files());
            request["operation"] = serde_json::json!("source.create");
            request["command_id"] = serde_json::json!("synthetic:g10-CV-create");
            g10_charge(g10_command_forecast(
                preview.object_get("owner_configuration").unwrap(),
                0,
                &g10_visit,
            ));
            request["expected_configuration"] =
                serde_json::from_slice(&bytes(preview.object_get("owner_configuration").unwrap()))
                    .unwrap();
            g10_charge(g10_command_forecast(
                preview.object_get("expected_dependencies").unwrap(),
                0,
                &g10_visit,
            ));
            request["expected_dependencies"] = serde_json::from_slice(&bytes(
                preview.object_get("expected_dependencies").unwrap(),
            ))
            .unwrap();
            request["expected_source"] = Value::Null;
            request["expected_revision"] = Value::Null;
            g10_creation_context.request_raw = canonical_json(&request);
            drop(prepared);
            worker.finish(deadline, &cancellation).unwrap();
            drop(worker);
            let mut operation_worker = super::command_form_cases::schemas_with_image(
                &current_cut,
                &image,
                deadline,
                &cancellation,
            );
            let (executed, published, result) = execute_isolated_creation_from_captures(
                &creation_fs,
                &g10_creation_context,
                &current_cut,
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
                result.object_get("grants_admission"),
                Some(&JsonValue::Bool(false))
            );
            let home = isolated.path().join(relative).parent().unwrap().to_owned();
            assert_eq!(
                executed
                    .prepared()
                    .files()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "chronology-version.json",
                    "chronology-version.human-forms.json",
                    "source-create-request.json",
                    "source-create-environment.json",
                    "source-create-provenance.jsonl",
                    "source-create-receipt.json"
                ])
            );
            assert_prepared_file_refs(
                &executed.prepared().preview().unwrap(),
                executed.prepared().files(),
            );
            for (name, raw) in executed.prepared().files() {
                assert!(raw.len() <= 65_536);
                g10_charge(
                    raw.len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(8))
                        .unwrap(),
                );
                assert_eq!(fs::read(home.join(name)).unwrap(), *raw);
            }
            assert_eq!(fs::read_dir(&home).unwrap().count(), 6);
            assert!(fs::metadata(isolated.path().join(relative)).unwrap().len() <= 16384);
            g10_charge(32_768);
            let cv_raw = fs::read(isolated.path().join(relative)).unwrap();
            let cv_document = g10_parse(&cv_raw);
            g10_charge(g10_clone_forecast(&record, 0, &g10_visit));
            let cv_record: Value = serde_json::from_slice(&cv_raw).unwrap();
            assert_eq!(
                bytes(cv_document.root()),
                bytes(&g10_parse(&canonical_json(&record)).into_root())
            );
            assert_eq!(cv_record, record);
            let receipt_len = executed
                .prepared()
                .files()
                .get("source-create-receipt.json")
                .unwrap()
                .len();
            g10_charge(
                receipt_len
                    .checked_mul(8)
                    .and_then(|n| n.checked_add(4096))
                    .unwrap(),
            );
            let physical_receipt = fs::read(home.join("source-create-receipt.json")).unwrap();
            let receipt_document = g10_parse(&physical_receipt);
            g10_charge(g10_command_forecast(
                result.object_get("receipt").unwrap(),
                0,
                &g10_visit,
            ));
            assert_eq!(
                bytes(receipt_document.root()),
                bytes(result.object_get("receipt").unwrap())
            );
            let cv_receipt: Value = serde_json::from_slice(&physical_receipt).unwrap();
            drop(receipt_document);
            drop(physical_receipt);

            let v1_ref = cv_receipt["source"].clone();
            assert_eq!(
                bytes(g10_parse(&canonical_json(&v1_ref)).root()),
                bytes(&tos_command::source_forms::metadata_subject(cv_document.root()).unwrap())
            );
            drop(cv_document);
            // execute_isolated_creation_from_captures already finishes its worker.
            drop(executed);
            drop(operation_worker);
            drop(g10_creation_context);
            drop(creation_fs);
            drop(preview);
            drop(published);
            drop(result);
            // Real post-CV epoch and catalog before freezing authored Tp. Derived
            // catalog JSONL stays in separate protected read custody, not the cut.
            let control_path = isolated
                .path()
                .join("ToS/source-witnesses/.metadata-publication.json");
            let read_control = || {
                let metadata = match fs::symlink_metadata(&control_path) {
                    Ok(metadata) => metadata,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
                    Err(error) => panic!("positive actual CONTROL metadata: {error}"),
                };
                assert!(metadata.is_file() && metadata.len() <= 8192);
                g10_charge(
                    usize::try_from(metadata.len())
                        .unwrap()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(8))
                        .unwrap(),
                );
                let raw = fs::read(&control_path).expect("positive actual CONTROL read");
                assert_eq!(raw.len() as u64, metadata.len());
                Some(raw)
            };
            let control_before = read_control();
            let epoch = tos_source_store::MetadataPublicationEpoch::select(
                control_before
                    .as_ref()
                    .map(|raw| g10_parse(raw).into_root()),
            )
            .unwrap();
            super::compiler_source_cases::publish_native_catalog_fixture_with_selected_schemas(
                &repository,
                isolated.path(),
                deadline,
                &selected_schema_files,
            );
            assert_eq!(read_control(), control_before);
            let catalog_manifest_path = isolated
                .path()
                .join("ToS/source-witnesses/catalog/catalog.manifest.json");
            assert!(fs::metadata(&catalog_manifest_path).unwrap().len() <= 1_048_576);
            let catalog_len =
                usize::try_from(fs::metadata(&catalog_manifest_path).unwrap().len()).unwrap();
            g10_charge(
                catalog_len
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(8))
                    .unwrap(),
            );
            let catalog_raw = fs::read(&catalog_manifest_path).unwrap();
            assert_eq!(catalog_raw.len(), catalog_len);
            let catalog_document = g10_parse(&catalog_raw);
            match epoch.token() {
                Some(token) => assert_eq!(
                    catalog_document
                        .root()
                        .object_get("selected_metadata_publication")
                        .unwrap()
                        .object_get("token")
                        .unwrap()
                        .as_str(),
                    Some(token)
                ),
                None => assert!(
                    catalog_document
                        .root()
                        .object_get("selected_metadata_publication")
                        .is_none()
                ),
            }
            drop(catalog_document);
            drop(catalog_raw);
            let current_authored = authored_text_files(isolated.path());
            assert_eq!(
                current_authored.get("ToS/source-witnesses/.metadata-publication.json"),
                control_before.as_ref()
            );
            assert!(
                !current_authored
                    .keys()
                    .any(|p| p.starts_with("ToS/source-witnesses/catalog/") && !p.ends_with(".md"))
            );
            let g10_tp2 = super::command_form_cases::successor(&current_authored, &store, tp);
            let current_cut = open_cut(&store, g10_tp2, deadline, &cancellation);
            let mut current_files = files
                .iter()
                .filter(|(p, _)| !p.starts_with("ToS/"))
                .map(|(p, r)| (p.clone(), r.clone()))
                .collect::<BTreeMap<_, _>>();
            current_files.extend(current_authored);
            let revision_path = isolated.path().join("g10-CV-read-owner.json");
            let revision_config = serde_json::json!({"schema_version":"tos_local_profile_revision_owner_v1","uid":uid,"principal_id":"software:test-fixture",
            "source_root":isolated.path(),"source_path":relative,"record_id":identity,"profile_type_id":"tos.entity.chronology-version",
            "authority_ref":"synthetic-test-only:g10-current-public-read","allowed_form_ids":["tos.form.creation.g10-cv-name"],
            "allowed_operations":["record.revise"],"allowed_fields":["semantic_content"],"expires_at":"2099-01-01T00:00:00Z"});
            let revision_config_raw = canonical_json(&revision_config);
            assert!(revision_config_raw.len() <= 8192);
            fs::write(&revision_path, &revision_config_raw).unwrap();
            fs::set_permissions(&revision_path, fs::Permissions::from_mode(0o600)).unwrap();
            let revision_fs = CreationFilesystem::select_isolated(
                &isolated,
                &revision_path,
                deadline,
                &cancellation,
            )
            .unwrap();
            assert!(
                current_files.len() <= 2048
                    && current_files.values().map(Vec::len).sum::<usize>() <= 33_554_432
            );
            let mut current_context = cut_context(
                &current_files,
                revision_config_raw.clone(),
                canonical_json(
                    &serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"describe"}),
                ),
                g10_tp2,
            );
            current_context.effective_uid = u64::from(uid);
            let g10_snapshot = SourceSnapshot {
                reference: SourceSnapshotRef {
                    id: StableId::parse(snapshot_name).unwrap(),
                    revision,
                },
                current_cutoff: Cutoff {
                    value: JsonValue::String(JsonString::from_utf8("2026-10-19T05:00:00Z")),
                    trs: None,
                },
            };

            assert_eq!(g10_snapshot.reference, snapshot.reference);
            assert_eq!(
                g10_snapshot.current_cutoff.value.as_str(),
                Some("2026-10-19T05:00:00Z")
            );
            assert!(g10_snapshot.current_cutoff.trs.is_none());
            // Same actual cold-admitted model/scope, ALL nodes including non-Claims.
            let graph = bound
                .source_for_adapter("reified-bibliographic-claims-v1")
                .unwrap();
            let mut scopes = selected.expectation.source_scopes.iter().filter(|scope| {
                scope.source_graph == graph
                    && scope.adapter_profile == "reified-bibliographic-claims-v1"
            });
            let scope = scopes.next().expect("G10 selected scope missing");
            assert!(scopes.next().is_none());
            let candidate_count = usize::try_from(scope.node_count).unwrap();
            assert!((1..=43).contains(&candidate_count));
            // Original44 subprotocol bytes, no expected/oracle fields or manifest
            // fiction. Four custody members remain visible, only two are targets.
            let original_raw=br#"{"context":{},"input":{"request_envelope":{"source_snapshot_ref":{"resolved_system_cutoff":"2026-10-19T05:00:00Z","revision_fence":"source-rev-42","snapshot_id":"S1"},"temporal":{"attributed_knowledge_as_of":{"cutoff":"1800","knower":"school-A","trs_ref":"TRS:GregorianProleptic:fixture-v1"},"system_state":"Current"}},"source_claims":[{"id":"c1","school_A_knew_by_1800":true,"system_ingested_at":"2019-01-01T00:00:00Z"},{"id":"c2","school_A_knew_by_1800":false,"system_ingested_at":"2026-01-01T00:00:00Z"}]},"operation":"resolve_temporal_query_context","source_profile":"ToS-Temporal-Core-Contract-v1"}"#;
            assert_eq!(
                Digest256::of_bytes(include_bytes!(
                    "../foundation-wave-v1/oracles/temporal-acceptance-vectors.json"
                ))
                .to_hex(),
                "fda08fd958914a3605afab12e7b78012a3f5534ecfdffe2ab31cb2cb3a26eecd"
            );
            assert_eq!(original_raw.len(), 603);
            assert_eq!(
                Digest256::of_bytes(original_raw).to_hex(),
                "8465256db2da5a5f3e45be27a519e83207d88ecad59cea87233bfa550be63d57"
            );
            assert!(original_raw.len() <= 2048);
            g10_charge(65_536);
            let original_document = tos_foundation::parse_json_with_state_budget(
                original_raw,
                JsonMode::PublishedStrict,
                JsonLimits {
                    max_bytes: 2048,
                    max_depth: 16,
                    max_visits: g10_visit_limit
                        .get()
                        .checked_sub(g10_used_visits.get())
                        .unwrap()
                        .min(512),
                    max_integer_digits: 4096,
                },
                65536,
            )
            .unwrap();
            g10_visit(original_document.visits());
            let page_raw = canonical_json(
                &serde_json::json!({"schema_version":"tos_stored_chronology_metadata_list_request_v1","filter":{"predicate":"historical_dating"},"page_size":5,"scan_limit":candidate_count,"after":null}),
            );
            assert!(page_raw.len() <= 2048);
            let page_document = g10_parse(&page_raw);
            let target_a = exact_ref(&member_ref);
            let target_b = exact_ref(&second_member_ref);
            let bindings = AttributedCutoffBindings {
                snapshot: ("S1", &g10_snapshot.reference),
                revision_fence: ("source-rev-42", revision),
                knower: ("school-A", g10_kdoc.root()),
                trs: ("TRS:GregorianProleptic:fixture-v1", g10_rdoc.root()),
                targets: [
                    ("c1", &target_a, &g10_drefs[0]),
                    ("c2", &target_b, &g10_drefs[1]),
                ],
                answer_field: "school_A_knew_by_1800",
            };
            let mut g10_worker = super::command_form_cases::schemas_with_image(
                &current_cut,
                &image,
                deadline,
                &cancellation,
            );
            let g10_loaded = load_chronology_from_owner(
                &revision_fs,
                &current_context,
                &current_cut,
                &exact_ref(&v1_ref),
                &context,
                &cut,
                &g10_snapshot,
                &software,
                &components,
                &mut g10_worker,
                ChronologyLimits::default(),
                deadline,
                &cancellation,
            )
            .unwrap();
            assert_eq!(
                g10_loaded.bound().chronology().member_claim_revisions,
                vec![
                    target_a.clone(),
                    target_b.clone(),
                    g10_drefs[0].clone(),
                    g10_drefs[1].clone()
                ]
            );
            assert_eq!(
                g10_loaded.bound().chronology().selection.system_as_of,
                g10_snapshot.current_cutoff
            );
            let inspect_controls = Arc::new(Controls::default());
            let list_controls = Arc::new(Controls::default());
            let inspect_held = Arc::new(AtomicUsize::new(0));
            let list_held = Arc::new(AtomicUsize::new(0));
            let inspect_authority = ListAuthority::for_scope(
                &bound,
                "tos.knowledge.node.inspect",
                "read_only_public_knowledge_inspect_v1",
                inspect_held.clone(),
                inspect_controls.clone(),
            );
            let inner = ListAuthority::for_scope(
                &bound,
                "tos.knowledge.temporal.list_claims",
                "read_only_public_knowledge_claim_list_v1",
                list_held.clone(),
                list_controls.clone(),
            );
            let observed = std::cell::Cell::new([0usize; 4]);
            let list_authority = G10CandidateObserver {
                inner,
                graph,
                subject: "",
                predicate: "historical_dating",
                negative_subject: "tos.historical-event.synthetic.absent-G10",
                counts: &observed,
            };
            for authority in [&inspect_authority, &list_authority.inner] {
                assert!(
                    tos_query::InspectCurrentAuthority::disclosure_metadata_state_upper_bound(
                        authority
                    )
                    .unwrap()
                        <= 4096
                );
            }
            // Additional diagnostic workspace/work: SAME nonrefundable parent,
            // before wrapper construction, excess application and callee budget.
            g10_charge(4096);
            g10_visit(256);
            assert!(
                std::mem::size_of::<G10BudgetRecorder>()
                    + std::mem::size_of::<G10BudgetAuthority<'_, ListAuthority>>()
                    + std::mem::size_of::<G10BudgetAuthority<'_, G10CandidateObserver<'_>>>()
                    + std::mem::size_of::<Option<(&'static str, [Option<u64>; 4])>>()
                    + std::mem::size_of::<std::fmt::Arguments<'_>>()
                    // Transient disjoint authority/error borrows and match headers;
                    // the original moved SQL error is neither cloned nor retained.
                    + 64
                    <= 4096
            );
            let budget_recorder = G10BudgetRecorder {
                slot: std::cell::Cell::new("not_started"),
                starts: std::cell::Cell::new(0),
                pending_sql: std::cell::Cell::new(false),
                first: std::cell::Cell::new(None),
            };
            let mut inspect_authority = G10BudgetAuthority {
                inner: inspect_authority,
                recorder: &budget_recorder,
            };
            let mut list_authority = G10BudgetAuthority {
                inner: list_authority,
                recorder: &budget_recorder,
            };
            g10_charge(262_144); // callback parse/serde/canonical overlap
            g10_visit(1024 + 8192); // callback field checks + parser ceiling, no refund
            let extra_bytes = g10_used_bytes.get().saturating_sub(1_048_576);
            let extra_visits = g10_used_visits.get().saturating_sub(8192);
            b_fixture_retained_bytes = b_fixture_retained_bytes
                .checked_add(extra_bytes.checked_sub(g10_applied_bytes.get()).unwrap())
                .unwrap();
            b_fixture_retained_visits = b_fixture_retained_visits
                .checked_add(extra_visits.checked_sub(g10_applied_visits.get()).unwrap())
                .unwrap();
            g10_applied_bytes.set(extra_bytes);
            g10_applied_visits.set(extra_visits);
            let budget = StoredChronologyListBudget {
                query: tos_query::InspectBudget {
                    max_open_vm_steps: selected.cold_limits().max_vm_steps,
                    max_read_vm_steps: 20_000_000,
                    max_matches: 16,
                    max_rows: 1000,
                    max_field_bytes: 16384,
                    max_payload_bytes: 1048576,
                    max_decoded_bytes: 8 * 1024 * 1024,
                    max_response_bytes: 327680,
                    json: JsonLimits {
                        max_bytes: 1048576,
                        max_depth: 64,
                        max_visits: 81920,
                        max_integer_digits: 4096,
                    },
                },
                temporal: TemporalBudget {
                    max_expression_bytes: 49152,
                    max_members: 24576,
                    max_pairings: 12288,
                },
                max_members: 4,
                max_extra_owned_bytes: (16 * 1024 * 1024usize)
                    .checked_sub(b_fixture_retained_bytes)
                    .unwrap(),
                max_extra_json_visits: 1_000_000usize
                    .checked_sub(b_fixture_retained_visits)
                    .unwrap(),
                max_verification_owned_bytes: 4 * 1024 * 1024,
                max_verification_json_visits: 250000,
            };
            // Attributed response redistribution retains all four old capacity
            // forecasts; admit up to 524288 extra list capacity plus fixed work
            // headers from this SAME parent, never from refunded earlier holds.
            assert!(
                budget.max_extra_owned_bytes
                    > (4 * 1024 * 1024usize + 1048576 + 5 * 131080 + 5 * 4096 + 69632)
                        .checked_add(524288)
                        .and_then(|n| n.checked_add(std::mem::size_of::<tos_query::InspectBudget>()))
                        .and_then(|n| {
                            n.checked_add(16usize.checked_mul(std::mem::size_of::<usize>())?)
                        })
                        .unwrap()
            );
            assert!(budget.max_extra_json_visits > 250000 + 32768);
            assert_eq!(budget.query.max_response_bytes / 5, 65536);
            assert_eq!(budget.query.json.max_visits / 5, 16384);
            let mut called = false;
            let g10_result = with_loaded_chronology_attributed_knowledge_by_cutoff(
                g10_loaded,
                &mut cold,
                &bound,
                &mut inspect_authority,
                &mut list_authority,
                &g10_snapshot,
                original_document.root(),
                &bindings,
                page_document.root(),
                budget,
                deadline,
                &cancellation,
                |view| {
                    called = true;
                    assert_eq!(inspect_held.load(Ordering::SeqCst), 4);
                    assert_eq!(list_held.load(Ordering::SeqCst), 1);
                    assert_eq!(observed.get(), [candidate_count, 4, 4, 0]);
                    view.with_current_bytes(|raw|{
                assert!(raw.len()<=65536);
                let document=tos_foundation::parse_json_with_state_budget(raw,JsonMode::PublishedStrict,JsonLimits{max_bytes:65536,max_depth:64,max_visits:8192,max_integer_digits:4096},131072).unwrap();
                assert!(document.visits()<=8192);
                let value:Value=serde_json::from_slice(raw).unwrap();
                assert_eq!(value.as_object().unwrap().keys().map(String::as_str).collect::<BTreeSet<_>>(),BTreeSet::from(["schema_version","scope","outcome","source_revision","selected_model_receipt_id","chronology_ref","custody_members","effective_context","original_request","selected_targets","excluded_targets","declaration_bindings","declared_ingest","refusals"]));
                assert_eq!(value["schema_version"],"tos_stored_attributed_knowledge_by_cutoff_result_v1");
                assert_eq!(value["source_revision"],revision.0.to_prefixed());assert_eq!(value["selected_model_receipt_id"],bound.owner_receipt_id());assert_eq!(value["chronology_ref"],v1_ref);
                assert_eq!(value["outcome"],"Resolved");assert_eq!(value["scope"],"SupplementalSourceDeclarationsOnly");
                assert_eq!(value["original_request"],serde_json::from_slice::<Value>(original_raw).unwrap());
                assert_eq!(value["custody_members"],serde_json::json!([member_ref,second_member_ref,g10_receipt["claims"][0],g10_receipt["claims"][1]]));
                assert_eq!(value["selected_targets"].as_array().unwrap().len(),1);assert_eq!(value["selected_targets"][0]["target_ref"],member_ref);assert_eq!(value["selected_targets"][0]["declaration_ref"],g10_receipt["claims"][0]);
                assert_eq!(value["excluded_targets"].as_array().unwrap().len(),1);assert_eq!(value["excluded_targets"][0]["target_ref"],second_member_ref);assert_eq!(value["excluded_targets"][0]["declaration_ref"],g10_receipt["claims"][1]);
                assert_eq!(value["refusals"],serde_json::json!([]));
                assert!(value.get("next").is_none()&&value.get("exhausted").is_none());
                for i in 0..2 {
                    assert_eq!(value["declaration_bindings"][i]["basis"],g10_create["claims"][i]["qualifiers"]["attributed_knowledge_by_cutoff"]["basis"]);
                    assert_eq!(value["declaration_bindings"][i]["knower_ref"],g10_kref_value);
                    assert_eq!(value["declaration_bindings"][i]["cutoff"]["trs"],g10_rref_value);
                    assert_eq!(value["declaration_bindings"][i]["answer"],serde_json::json!(i==0));
                    assert_eq!(value["declared_ingest"][i]["lexical"],if i==0{"2019-01-01T00:00:00Z"}else{"2026-01-01T00:00:00Z"});
                    assert_eq!(value["declared_ingest"][i]["scope"],"SourceDeclaredNotOwnerTransactionTime");
                }
                assert_eq!(value["effective_context"]["selection"]["system_as_of"]["value"],"2026-10-19T05:00:00Z");
                assert_eq!(value["effective_context"]["selection"]["attributed_knowledge"]["knower"],g10_kref_value["id"]);
                assert_eq!(value["effective_context"]["selection"]["attributed_knowledge"]["cutoff"],serde_json::json!({"value":"1800","trs":g10_rref_value}));
                assert_eq!(value["effective_context"]["selection"]["source_snapshot"]["id"],snapshot_name);
                assert_eq!(value["effective_context"]["selection"]["source_snapshot"]["revision"],revision.0.to_prefixed());
                assert!(value["effective_context"]["selection"]["system_as_of"]["trs"].is_null());
                assert!(value["effective_context"]["selection"]["source_publication_as_of"].is_null());
                assert_eq!(document.root().as_object().unwrap().len(),14); Ok(())
            })?;
                    for controls in [&inspect_controls, &list_controls] {
                        controls.revoked.store(true, Ordering::SeqCst);
                        assert!(
                            matches!(view.recheck(),Err(StoredChronologyListError::Query(error))if error.code==tos_query::search_v2::SearchV2ErrorCode::StalePolicy)
                        );
                        controls.revoked.store(false, Ordering::SeqCst);
                        view.recheck()?;
                        controls.cancelled.store(true, Ordering::SeqCst);
                        assert!(
                            matches!(view.recheck(),Err(StoredChronologyListError::Query(error))if error.code==tos_query::search_v2::SearchV2ErrorCode::Cancelled)
                        );
                        controls.cancelled.store(false, Ordering::SeqCst);
                        view.recheck()?;
                    }
                    let mut drift = revision_config.clone();
                    drift["allowed_operations"] = serde_json::json!([]);
                    fs::write(&revision_path, canonical_json(&drift)).unwrap();
                    assert!(view.recheck().is_err());
                    fs::write(&revision_path, &revision_config_raw).unwrap();
                    view.recheck()?;
                    Ok(())
                },
            );
            if matches!(&g10_result, Err(StoredChronologyListError::Query(reason))
                if reason.code == tos_query::search_v2::SearchV2ErrorCode::BudgetExceeded)
            {
                budget_recorder.write_best_effort();
            }
            g10_result.unwrap();
            assert!(called);
            assert_eq!(inspect_held.load(Ordering::SeqCst), 0);
            assert_eq!(list_held.load(Ordering::SeqCst), 0);
            for controls in [&inspect_controls, &list_controls] {
                assert!(
                    !controls.revoked.load(Ordering::SeqCst)
                        && !controls.cancelled.load(Ordering::SeqCst)
                );
            }
            assert_eq!(fs::read(&revision_path).unwrap(), revision_config_raw);
            assert_eq!(
                fs::read(isolated.path().join(&member_path)).unwrap(),
                member_raw
            );
            assert_eq!(
                Digest256::of_bytes(&fs::read(isolated.path().join(&g10_source_path)).unwrap()),
                g10_stream_sha
            );
            assert_eq!(
                &fs::read(isolated.path().join(saved_b_cv_path)).unwrap(),
                saved_b_cv_raw
            );
            assert_eq!(
                &serde_json::from_slice::<Value>(
                    &fs::read(
                        isolated
                            .path()
                            .join(saved_b_cv_path)
                            .parent()
                            .unwrap()
                            .join("source-create-receipt.json")
                    )
                    .unwrap()
                )
                .unwrap()["source"],
                saved_b_ref
            );
            assert_eq!(
                fs::read(isolated.path().join(g10_event_path)).unwrap(),
                g10_eraw
            );
            drop(inspect_authority);
            drop(list_authority);
            g10_worker.finish(deadline, &cancellation).unwrap();
            drop(g10_worker);
            held_list_fixture_counts[0] += 1;
            held_list_fixture_counts[1] += 5;
            eprintln!(
                "G10 phase=complete targets=2 witnesses=2 actual_custody=4 lookups=4 lists=1 held_leases=5 boolean_checks=2 resolver=1 world_delta=0"
            );
        }
        // END seven-context held-list fixture: NEW.
        assert_eq!(held_list_fixture_counts, [10, 19, 2]);
        assert_eq!(general_fixed_header_resource_count, 1);
        eprintln!(
            "held list fixture phase=complete fresh_loaders=10 delegated_queries=19 acquired_query_leases=19 early_cursor_refusals=2 fixed_header_resource_refusals=1 max_pure_calls=14 max_held_query_leases=5"
        );
        eprintln!(
            "temporal positive phase=complete native-calls=2 primary-records=3 claim-files=5 claim-rows=2 CV-files=6 model=Sp selfpair=1"
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
    read_record_revision_publication(ctx, &[&transaction.transaction_id]).unwrap_or_else(|error| {
        panic!(
            "retained transaction {} committed={committed}: {error:?}; retained files={:?}",
            transaction.transaction_id,
            ctx.files
                .iter()
                .filter(|file| file
                    .path
                    .as_str()
                    .starts_with("ToS/source-witnesses/.metadata-"))
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>()
        )
    })
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
        .find(|f| f.path.as_str() == "rust/crates/tos-command/src/source_sign_native.rs")
        .unwrap()
        .raw
        .push(b' ');
    assert!(matches!(
        run_with_cut(&changed_software, None, true),
        Err(SourceCommandError::Conflict(_))
    ));
}

#[test]
fn native_initial_creation_cli_preserves_native_contract_and_cold_retained_receipt() {
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
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_forms_publication.rs",
        "rust/crates/tos-compiler/src/source_witness_catalog.rs",
        "rust/crates/tos-command/src/source_private_profile.rs",
        "rust/crates/tos-command/src/source_private_claim.rs",
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-command/src/source_private_owner_store.rs",
        "rust/crates/tos-command/src/source_corpus_index_projection.rs",
        "rust/crates/tos-compiler/src/source_bibliographic.rs",
        "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
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

    let inputs: std::collections::BTreeSet<&str> = inputs
        .into_iter()
        .chain(
            tos_command::source_claims::CLAIM_GROUNDING_RULE_INPUTS
                .iter()
                .copied(),
        )
        .chain(
            tos_command::source_claims::CLAIM_REVISION_RULE_INPUTS
                .iter()
                .copied(),
        )
        .collect();

    let mut fixture_bytes = 0u64;
    for name in &inputs {
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
        "creation CLI F={} E={} C={} W={} native_processes=6 workers<=6",
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
        let actual =
            alignment_native_cli(&repository, &owner, &invocation_path, &request, deadline);
        assert_eq!(
            actual["schema_version"],
            "tos_local_native_source_result_v1"
        );
        assert_eq!(actual["grants_admission"], false);
        let result = &actual["result"];
        assert!(result.is_object(), "native creation response is an object");
        assert_eq!(
            result["schema_version"],
            "tos_local_historical_create_result_v1"
        );
        assert_eq!(result["grants_admission"], false);
        assert_eq!(result["record_id"], record["record_id"]);
        assert_eq!(
            result["supported_operations"],
            serde_json::json!(["historical.create"])
        );
        match request["operation"].as_str().unwrap() {
            "describe" => assert_eq!(result["target_exists"], false),
            "prepare" => assert!(result["prepared_source"].is_object()),
            "prepare-create" => {
                assert!(result["prepared_source"].is_object());
                assert!(result["expected_dependencies"].is_string());
                assert!(result["prepared_files"].is_object());
                preview = result.clone();
            }
            operation => panic!("unexpected native creation request {operation}"),
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

#[test]
fn sign_promotion_native_entry_v3_assesses_prepares_commits_reads_and_replays() {
    use std::os::unix::fs::PermissionsExt;

    let repository = super::validation_cut_cases::repository()
        .canonicalize()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(900);
    let cancelled = AtomicBool::new(false);
    let temporary = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let fixture = super::native_public_assessment_fixture::native_public_sign_fixture(
        &repository,
        temporary.path(),
    )
    .unwrap();
    let public_root = fixture.public_root.clone();
    let content_path = public_root.join(
        "ToS/source-witnesses/works/synthetic-native-binding/technical-markup/synthetic-binding/public-synthetic-content.txt",
    );
    assert_eq!(fs::read(content_path).unwrap(), fixture.source_text);
    let claim_path = public_root.join(&fixture.source_path);
    let claim: Value = serde_json::from_slice(&fs::read(claim_path).unwrap()).unwrap();
    assert_eq!(claim["claim_id"], fixture.candidate_id);
    assert_eq!(
        claim["object"]["members"],
        serde_json::json!(fixture.occurrence_ids)
    );
    let authored = super::command_text_cases::authored_text_files(&public_root);
    let mut captured = authored.clone();
    let software_sources = [
        "rust/crates/tos-command/src/lib.rs",
        "rust/crates/tos-command/src/source_native_cli.rs",
        "rust/crates/tos-command/src/source_native_private_cli.rs",
        "rust/crates/tos-command/src/source_native_private_assessment_cli.rs",
        "rust/crates/tos-command/src/source_assessment_journal.rs",
        "rust/crates/tos-command/src/source_private_assessment_sources.rs",
        "rust/crates/tos-command/src/source_private_assessment_layers.rs",
        "rust/crates/tos-command/src/source_private_claim.rs",
        "rust/crates/tos-command/src/source_text_owner.rs",
        "rust/crates/tos-command/src/source_creation.rs",
        "rust/crates/tos-command/src/source_creation_store.rs",
        "rust/crates/tos-command/src/source_serialization.rs",
        "rust/crates/tos-command/src/source_native_creation_cli.rs",
        "rust/crates/tos-command/src/source_creation_cli_selection.rs",
        "rust/crates/tos-command/src/source_sign.rs",
        "rust/crates/tos-command/src/source_claims.rs",
        "rust/crates/tos-command/src/source_revisions.rs",
        "rust/crates/tos-command/src/source_forms.rs",
        "rust/crates/tos-command/src/source_forms_publication.rs",
        "rust/crates/tos-command/src/source_private_profile.rs",
        "rust/crates/tos-command/src/source_private_owner_store.rs",
        "rust/crates/tos-command/src/source_corpus_index_projection.rs",
        "rust/crates/tos-compiler/src/source_witness_catalog.rs",
        "rust/crates/tos-compiler/src/source_bibliographic.rs",
        "rust/crates/tos-compiler/src/source_bibliographic_render.rs",
        "rust/crates/tos-validation/src/assessment.rs",
    ];
    for reference in software_sources
        .into_iter()
        .chain(
            tos_command::source_claims::CLAIM_GROUNDING_RULE_INPUTS
                .iter()
                .copied(),
        )
        .chain(
            tos_command::source_claims::CLAIM_REVISION_RULE_INPUTS
                .iter()
                .copied(),
        )
    {
        let path = repository.join(reference);
        assert!(
            path.is_file() && fs::metadata(&path).unwrap().len() <= 2_097_152,
            "selected Sign/assessment source is a bounded regular file: {reference}"
        );
        captured
            .entry(reference.to_owned())
            .or_insert_with(|| fs::read(path).unwrap());
    }
    assert!(
        !authored.is_empty()
            && captured.len() <= 2048
            && captured.values().map(Vec::len).sum::<usize>() <= 33_554_432
    );
    let (capture, _software, components) =
        super::command_record_cases::captured_components(&captured, deadline, &cancelled);
    let store = temporary.path().join("sign-native-entry-cut");
    let initial_revision = super::validation_cut_cases::write_cut_store(&authored, &store);
    let images = [
        std::env::current_exe().unwrap(),
        PathBuf::from(
            std::env::var_os("TOS_NATIVE_OWNER_COMMAND_PATH").expect("native CLI required"),
        ),
        PathBuf::from(std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("native worker required")),
    ];
    let image_digests: Vec<_> = images
        .iter()
        .map(|path| super::command_text_cases::alignment_image_digest(path))
        .collect();
    let sign_owner = fixture.sign_owner_config_path.clone();
    let assessment_owner = fixture.assessment_owner_config_path.clone();
    let assessment_context = fixture.assessment_owner_context_path.clone();
    let invocation_path = temporary.path().join("sign-native-entry-invocation.json");
    let mut invocation = serde_json::json!({
        "schema_version":"tos_local_native_source_invocation_v1",
        "owner_config":sign_owner,
        "owner_context":null,
        "native_executable":images[1],
        "native_executable_sha256":image_digests[1].to_prefixed(),
        "corpus_store":store,
        "source_revision":initial_revision.0.to_prefixed(),
        "original_source_revision":initial_revision.0.to_prefixed(),
        "software_capture":capture.capture,
        "software_restored_root":capture.restored,
        "software_selection":{
            "source_git_commit":capture.selection.source_git_commit,
            "source_git_tree":capture.selection.source_git_tree,
            "capture_manifest_sha256":capture.selection.capture_manifest_sha256.to_prefixed()
        },
        "software_components":components.members().map(|member|member.path.as_str()).collect::<Vec<_>>(),
        "schema_worker":{"absolute_path":images[2],"sha256":image_digests[2].to_prefixed()},
        "assessment_schema_worker":{"absolute_path":images[2],"sha256":image_digests[2].to_prefixed()},
        "budgets":{
            "max_revisions":4,"max_members":2048,"max_total_bytes":33554432,
            "max_member_bytes":8388608,"max_schema_receipts":128,
            "max_schema_receipt_bytes":262144,"worker_cpu_seconds":3,
            "worker_address_space_bytes":1073741824
        }
    });
    let invoke =
        |invocation: &mut Value, owner: &Path, context: Option<&Path>, request: &Value| -> Value {
            invocation["owner_config"] = Value::from(owner.to_str().unwrap());
            invocation["owner_context"] =
                context.map_or(Value::Null, |path| Value::from(path.to_str().unwrap()));
            fs::write(&invocation_path, canonical_json(invocation)).unwrap();
            fs::set_permissions(&invocation_path, fs::Permissions::from_mode(0o600)).unwrap();
            let (status, raw, errors) = super::command_text_cases::native_owner_cli_observation(
                &repository,
                owner,
                &invocation_path,
                request,
                deadline,
            );
            assert!(
                status.success(),
                "native Sign entry operation {}: {}",
                request["operation"],
                String::from_utf8_lossy(&errors)
            );
            serde_json::from_slice(&raw).unwrap()
        };

    let sign_describe = serde_json::json!({
        "schema_version":"tos_local_source_command_v1","operation":"describe"
    });
    let refused = invoke(&mut invocation, &sign_owner, None, &sign_describe);
    assert_eq!(
        refused["schema_version"],
        "tos_local_native_source_result_v1"
    );
    assert_eq!(refused["grants_admission"], false);
    assert_eq!(refused["result"]["promotion"]["eligible"], false);
    assert_eq!(
        refused["result"]["promotion"]["grants_issuance_authority"],
        false
    );

    let assessment_describe = serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1",
        "operation":"describe","subject_id":fixture.candidate_id
    });
    let assessment_before = invoke(
        &mut invocation,
        &assessment_owner,
        Some(&assessment_context),
        &assessment_describe,
    );
    assert_eq!(
        assessment_before["schema_version"],
        "tos_local_assessment_result_v1"
    );
    let assessment_subject = assessment_before["result"]["command_context"]["subject"].clone();
    assert_eq!(assessment_subject["id"], fixture.candidate_id);
    assert_eq!(
        assessment_before["result"]["command_context"]["grants_authority"],
        false
    );
    let assessment_append = serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1",
        "operation":"append",
        "subject_id":fixture.candidate_id,
        "expected_subject":assessment_subject,
        "expected_snapshot":assessment_before["owner_snapshot"],
        "expected_revision":assessment_before["result"]["revision"],
        "command_id":"fixture-sign-flame-assessment-append",
        "assessments":[fixture.assessment.clone()]
    });
    let assessed = invoke(
        &mut invocation,
        &assessment_owner,
        Some(&assessment_context),
        &assessment_append,
    );
    assert_eq!(assessed["result"]["current_admission"]["can_use"], true);
    let assessment_read = serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1",
        "operation":"inspect",
        "subject_id":fixture.candidate_id,
        "expected_subject":assessment_subject,
        "expected_snapshot":assessed["owner_snapshot"]
    });
    let admission = invoke(
        &mut invocation,
        &assessment_owner,
        Some(&assessment_context),
        &assessment_read,
    );
    assert_eq!(admission["result"]["current_admission"]["can_use"], true);
    assert_eq!(
        admission["result"]["current_admission"]["use"],
        "sign-promotion"
    );

    let ready = invoke(&mut invocation, &sign_owner, None, &sign_describe);
    let promotion = &ready["result"]["promotion"];
    assert_eq!(promotion["eligible"], true);
    assert_eq!(promotion["grants_issuance_authority"], false);
    let basis = promotion["basis"].clone();
    assert_eq!(basis["candidate"]["id"], fixture.candidate_id);
    assert_eq!(basis["use"], "sign-promotion");
    assert_eq!(basis["grants_current_use"], false);
    assert!(!basis["assessment_refs"].as_array().unwrap().is_empty());
    let mut record = fixture.sign_record.clone();
    record["promotion_basis"] = basis;
    let prepared_source = invoke(
        &mut invocation,
        &sign_owner,
        None,
        &serde_json::json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"prepare","record":record
        }),
    );
    assert_eq!(prepared_source["grants_admission"], false);
    assert!(prepared_source["result"]["prepared_source"].is_object());

    let prepared = invoke(
        &mut invocation,
        &sign_owner,
        None,
        &serde_json::json!({
            "schema_version":"tos_local_source_command_v1",
            "operation":"prepare-create","record":record,
            "forms":fixture.forms.clone()
        }),
    );
    assert_eq!(prepared["grants_admission"], false);
    assert!(prepared["result"]["prepared_files"].is_object());
    assert_eq!(
        prepared["result"]["prepared_source"],
        prepared_source["result"]["prepared_source"]
    );
    let command = serde_json::json!({
        "schema_version":"tos_local_source_command_v1",
        "operation":"sign.promote",
        "command_id":"fixture-sign-flame-promote-once",
        "record":record,
        "forms":fixture.forms,
        "expected_configuration":prepared["result"]["owner_configuration"],
        "expected_dependencies":prepared["result"]["expected_dependencies"],
        "expected_source":null,
        "expected_revision":null
    });
    assert_eq!(command.as_object().unwrap().len(), 9);
    let created = invoke(&mut invocation, &sign_owner, None, &command);
    assert_eq!(created["result"]["replayed"], false);
    assert_eq!(created["grants_admission"], false);

    let config: Value = serde_json::from_slice(&fs::read(&sign_owner).unwrap()).unwrap();
    let source_path = public_root.join(config["source_path"].as_str().unwrap());
    assert!(source_path.is_file());
    let home = source_path.parent().unwrap();
    let sign_bytes = fs::read(&source_path).unwrap();
    let sign_read: Value = serde_json::from_slice(&sign_bytes).unwrap();
    assert_eq!(sign_read, record);
    assert_eq!(sign_read["identity_status"], "provisional");
    assert_eq!(sign_read["same_as_posture"], "no_equivalence_claim");
    assert_eq!(sign_read["visibility"], "public_metadata_only");
    let forms_path = home.join("sign.human-forms.json");
    let forms_bytes = fs::read(&forms_path).unwrap();
    let receipt_path = home.join("source-create-receipt.json");
    let receipt_bytes = fs::read(&receipt_path).unwrap();
    let receipt: Value = serde_json::from_slice(&receipt_bytes).unwrap();
    assert_eq!(receipt, created["result"]["receipt"]);
    let history_path = home.join("source-create-provenance.jsonl");
    let history_before = fs::read(&history_path).unwrap();
    assert_eq!(
        std::str::from_utf8(&history_before)
            .unwrap()
            .lines()
            .count(),
        1
    );
    let provenance: Value = serde_json::from_slice(&history_before).unwrap();
    assert_eq!(provenance["event_id"], config["provenance_event_id"]);
    assert_eq!(
        provenance["method"]["procedure"]["name"],
        "native-source-metadata-serialization"
    );
    assert_eq!(
        provenance["review_and_authority"]["human_review_status"],
        "not_performed"
    );
    assert_eq!(
        provenance["review_and_authority"]["promotion_authorized"],
        false
    );
    assert_eq!(
        provenance["rights_and_visibility"]["publication_authorized"],
        false
    );

    let mut current_files = super::command_text_cases::authored_text_files(&public_root);
    current_files.remove("ToS/source-witnesses/.historical-create.writer.lock");
    let successor = super::validation_cut_cases::write_cut_store_on_base(
        &current_files,
        &store,
        Some(initial_revision),
    );
    invocation["source_revision"] = Value::from(successor.0.to_prefixed());
    let replayed = invoke(&mut invocation, &sign_owner, None, &command);
    assert_eq!(replayed["result"]["replayed"], true);
    assert_eq!(replayed["result"]["receipt"], created["result"]["receipt"]);
    assert_eq!(fs::read(&source_path).unwrap(), sign_bytes);
    assert_eq!(fs::read(&forms_path).unwrap(), forms_bytes);
    assert_eq!(fs::read(&receipt_path).unwrap(), receipt_bytes);
    assert_eq!(fs::read(&history_path).unwrap(), history_before);

    // S01 is a separate source-bound review, after the successful Sign flow.
    // Reuse the existing fixture's eligible principal/scope/evidence unchanged.
    let claim_path = public_root.join(&fixture.source_path);
    let original_claim_bytes = authored.get(&fixture.source_path).unwrap();
    let original_claim_digest = Digest256::of_bytes(original_claim_bytes);
    assert_eq!(fs::read(&claim_path).unwrap(), *original_claim_bytes);
    let assessment_config: Value =
        serde_json::from_slice(&fs::read(&assessment_owner).unwrap()).unwrap();
    let journal_home = PathBuf::from(assessment_config["journal_directory"].as_str().unwrap())
        .join(Digest256::of_bytes(fixture.candidate_id.as_bytes()).to_hex());
    let read_batch = |revision: &Value| -> (Vec<u8>, Value) {
        let digest = revision.as_str().unwrap();
        assert!(Digest256::from_hex(digest).is_ok());
        let path = journal_home.join(format!("{digest}.json"));
        let metadata = path.symlink_metadata().unwrap();
        assert!(metadata.is_file() && metadata.len() <= 1_048_576);
        let raw = fs::read(&path).unwrap();
        let parsed = parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .into_root();
        let canonical = canonical_bytes_v1(
            &parsed,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default(),
        )
        .unwrap();
        assert_eq!(Digest256::of_bytes(&canonical).to_hex(), digest);
        let batch = serde_json::from_slice(&raw).unwrap();
        (raw, batch)
    };
    let (first_batch_bytes, first_batch) = read_batch(&assessed["result"]["revision"]);
    assert_eq!(first_batch, assessed["result"]["receipt"]);
    assert_eq!(first_batch["events"][0]["assessment"], fixture.assessment);

    let before_dispute = invoke(
        &mut invocation,
        &assessment_owner,
        Some(&assessment_context),
        &assessment_describe,
    );
    let current_subject = before_dispute["result"]["command_context"]["subject"].clone();
    assert_eq!(current_subject, assessment_subject);
    assert_eq!(
        before_dispute["result"]["revision"],
        assessed["result"]["revision"]
    );
    assert_eq!(before_dispute["result"]["batch_count"], 1);
    let mut dispute = fixture.assessment.clone();
    dispute["assessment_id"] = Value::from("tos.review.fixture.flame-continuity-source-dispute");
    dispute["decision"] = Value::from("dispute");
    dispute["rationale"] = Value::from(
        "Fixture-local counterreading: two literal flame occurrences do not settle continuity under change. This disputes the interpretation without asserting falsity or withdrawing the original evidence.",
    );
    assert_eq!(dispute["supersedes"], serde_json::json!([]));
    let dispute_request = serde_json::json!({
        "schema_version":"tos_local_assessment_command_v1","operation":"append",
        "subject_id":current_subject["id"],"expected_subject":current_subject,
        "expected_snapshot":before_dispute["owner_snapshot"],
        "expected_revision":before_dispute["result"]["revision"],
        "command_id":"fixture-sign-claim-dispute","assessments":[dispute.clone()]
    });
    let disputed = invoke(
        &mut invocation,
        &assessment_owner,
        Some(&assessment_context),
        &dispute_request,
    );
    assert_eq!(disputed["result"]["replayed"], false);
    assert_ne!(
        disputed["result"]["revision"],
        before_dispute["result"]["revision"]
    );
    assert_eq!(
        disputed["result"]["receipt"]["previous_revision"],
        before_dispute["result"]["revision"]
    );
    assert_eq!(
        disputed["result"]["current_admission"]["status"],
        "disputed"
    );
    assert_eq!(disputed["result"]["current_admission"]["can_use"], false);
    let cold_disputed = invoke(
        &mut invocation,
        &assessment_owner,
        Some(&assessment_context),
        &serde_json::json!({"schema_version":"tos_local_assessment_command_v1",
            "operation":"inspect","subject_id":current_subject["id"],
            "expected_subject":current_subject,"expected_snapshot":disputed["owner_snapshot"]}),
    );
    assert_eq!(
        cold_disputed["result"]["revision"],
        disputed["result"]["revision"]
    );
    assert_eq!(cold_disputed["result"]["batch_count"], 2);
    let current_admission = &cold_disputed["result"]["current_admission"];
    assert_eq!(current_admission["subject"], current_subject);
    assert_eq!(current_admission["status"], "disputed");
    assert_ne!(current_admission["status"], "rejected");
    assert_ne!(current_admission["status"], "withdrawn");
    assert_eq!(current_admission["can_use"], false);
    let active = current_admission["assessment_refs"].as_array().unwrap();
    for event in [&fixture.assessment, &dispute] {
        assert!(
            active
                .iter()
                .any(|reference| reference["id"] == event["assessment_id"])
        );
    }
    let (second_batch_bytes, second_batch) = read_batch(&disputed["result"]["revision"]);
    assert!(!second_batch_bytes.is_empty());
    assert_eq!(second_batch, disputed["result"]["receipt"]);
    assert_eq!(second_batch["events"][0]["assessment"], dispute);
    assert_ne!(
        second_batch["events"][0]["assessment"]["decision"],
        "reject"
    );
    assert_ne!(
        second_batch["events"][0]["assessment"]["decision"],
        "withdraw"
    );
    assert_eq!(
        read_batch(&assessed["result"]["revision"]).0,
        first_batch_bytes,
        "the original assessment batch must remain byte-for-byte retained"
    );
    assert_eq!(
        fs::read(journal_home.join("head")).unwrap(),
        format!("{}\n", disputed["result"]["revision"].as_str().unwrap()).as_bytes()
    );
    let claim_after = fs::read(&claim_path).unwrap();
    assert_eq!(claim_after, *original_claim_bytes);
    assert_eq!(Digest256::of_bytes(&claim_after), original_claim_digest);
    assert!(Instant::now() < deadline);
}
