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
        .filter(|(name, _)| !profile_cut || name.starts_with("ToS/"))
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
    if profile_cut {
        assert!(
            cut.current()
                .members()
                .all(|member| member.path.as_str().starts_with("ToS/"))
        );
        let (_capture, software, components) = captured_components(&files, deadline, &cancel);
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
    } else {
        prepare_record_revision(&bound, publication, &mut worker, deadline, &cancel)
    }
}
fn captured_components(
    files: &BTreeMap<String, Vec<u8>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> (
    super::source_cut_cases::SoftwareCaptureFixture,
    tos_source_store::SoftwareCaptureReader,
    tos_source_store::SoftwareComponentSelectionV1,
) {
    use tos_source_store::{ReadLimits, SoftwareCaptureReader};
    let repository = repository().canonicalize().unwrap();
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
    let anchor = parse(include_bytes!(
        "../../fixtures/native-text-binding/source-anchor-v2-abc/variant-b.anchor.json"
    ));
    let source_binding = layer.object_get("source_binding").unwrap().clone();
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
    let ctx = native_profile_context();
    assert!(
        !ctx.files
            .iter()
            .any(|f| f.path.as_str().ends_with(".txt") || f.path.as_str().contains("/payload/"))
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
