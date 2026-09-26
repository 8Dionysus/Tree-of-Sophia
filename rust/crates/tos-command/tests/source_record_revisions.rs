//! Whole executable record preparation on an existing bounded Work fixture.
//! Oracle hashes were obtained from maintained Python `_proposal`, with the
//! same fixture, principal, source-copy selector and authored correction.
use tos_command::source_command::{CommandContext, SourceCommandError, SourceFile};
use tos_command::source_revisions::{
    RetainedRevisionTransaction, RevisionPublication, RevisionTransactionStatus,
    prepare_record_revision,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonString, JsonValue, RelativePath,
    SourceRevision, canonical_bytes_v1, parse_json,
};

const SOURCE: &[u8] = include_bytes!("fixtures/source_forms_shadow/source.initial.json");
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
            files.push(file($path, include_bytes!(concat!("../../../../", $path))));
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
fn apply_request(ctx: &mut CommandContext, publication: Option<&RevisionPublication>) -> JsonValue {
    let prepared = prepare_record_revision(ctx, publication).unwrap();
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
    let prepared = prepare_record_revision(&ctx, None).unwrap();
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
    apply_proposed(&mut ctx, &prepared.changes);
    let replay = prepare_record_revision(&ctx, None).unwrap();
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
    let inspected = prepare_record_revision(&ctx, None).unwrap();
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
        prepare_record_revision(&ctx, None),
        Err(SourceCommandError::Conflict(_))
    ));
}
#[test]
fn current_account_expiry_scope_and_selected_exact_recovery_are_independent() {
    let mut ctx = context(true);
    let initial_publication = RevisionPublication::default();
    let request = apply_request(&mut ctx, Some(&initial_publication));
    let prepared = prepare_record_revision(&ctx, Some(&initial_publication)).unwrap();
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
    let mut publication = RevisionPublication {
        token: None,
        transactions: vec![transaction],
    };
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
    let rollback = prepare_record_revision(&ctx, Some(&publication)).unwrap();
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
        prepare_record_revision(&ctx, Some(&publication)),
        Err(SourceCommandError::Denied(_))
    ));
    ctx.effective_uid = 1000;
    ctx.recorded_at = "2031-01-01T00:00:00Z".into();
    assert!(matches!(
        prepare_record_revision(&ctx, Some(&publication)),
        Err(SourceCommandError::Denied(_))
    ));
    ctx.recorded_at = "2026-09-26T12:00:00Z".into();
    // Exact normal retry resumes the original proposal; committed replay must
    // independently reconstruct the retained publication and predecessor.
    ctx.request_raw = bytes(&request);
    let resume = prepare_record_revision(&ctx, Some(&publication)).unwrap();
    apply_proposed(&mut ctx, &resume.changes);
    publication.transactions[0].status = RevisionTransactionStatus::Committed;
    assert!(
        prepare_record_revision(&ctx, Some(&publication))
            .unwrap()
            .replayed
    );
    let mut revoked = config;
    set(&mut revoked, "allowed_operations", arr(&["record.recover"]));
    ctx.configuration_raw = bytes(&revoked);
    assert!(matches!(
        prepare_record_revision(&ctx, Some(&publication)),
        Err(SourceCommandError::Denied(_))
    ));
}
