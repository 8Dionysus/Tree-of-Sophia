// Intentionally path-imported until CMD/OPS review chooses an internal route.
// No public `tos_command` source writer is exposed by this differential test.
#[path = "../src/source_forms_shadow.rs"]
mod source_forms_shadow;

use source_forms_shadow::{
    ShadowError, WorkFormsInput, apply_or_replay, run_claim_command, run_work_command,
};
use tos_foundation::{CanonicalProfile, JsonLimits, JsonMode, canonical_digest_v1, parse_json};

const SOURCE: &[u8] = include_bytes!("fixtures/source_forms_shadow/source.initial.json");
const INITIAL: &[u8] = include_bytes!("fixtures/source_forms_shadow/form-set.initial.json");
const PUBLISHED: &[u8] = include_bytes!("fixtures/source_forms_shadow/form-set.published.json");
const CONFIG: &[u8] = include_bytes!("fixtures/source_forms_shadow/owner.synthetic.json");
const REQUEST: &[u8] = include_bytes!("fixtures/source_forms_shadow/apply.request.json");
const RESPONSE: &[u8] = include_bytes!("fixtures/source_forms_shadow/apply.response.json");
const DESCRIBE_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/describe.response.json");
const PREPARE_PREFERRED_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/prepare.preferred.response.json");
const PREPARE_RUSSIAN_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/prepare.russian.response.json");
const REPLAY_RESPONSE: &[u8] = include_bytes!("fixtures/source_forms_shadow/replay.response.json");
const LIPSIUS_SOURCE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/source.json");
const LIPSIUS_INITIAL: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/initial.json");
const LIPSIUS_PUBLISHED: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/published.json");
const LIPSIUS_CONFIG: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/owner.json");
const LIPSIUS_DESCRIBE_REQUEST: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/describe_request.json");
const LIPSIUS_DESCRIBE_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/describe.json");
const LIPSIUS_PREPARE_REQUEST: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/prepare_request.json");
const LIPSIUS_PREPARE_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/prepare.json");
const LIPSIUS_CREATE_REQUEST: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/create_request.json");
const LIPSIUS_CREATE_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/create.json");
const LIPSIUS_APPLY_REQUEST: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/apply_request.json");
const LIPSIUS_APPLY_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/apply.json");
const LIPSIUS_REPLAY_RESPONSE: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/de_constantia/replay.json");
const INSTANT: &str = "2026-01-01T12:34:56+00:00";

const CLAIM_SOURCE: &[u8] = include_bytes!("fixtures/source_forms_shadow/claim_v1/source.json");
const CLAIM_INITIAL: &[u8] = include_bytes!("fixtures/source_forms_shadow/claim_v1/initial.json");
const CLAIM_V1_OWNER: &[u8] = include_bytes!("fixtures/source_forms_shadow/claim_v1/owner.json");
const CLAIM_V2_OWNER: &[u8] = include_bytes!("fixtures/source_forms_shadow/claim_v2/owner.json");
const CLAIM_V1_APPLY: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/claim_v1/apply_request.json");
const CLAIM_V2_APPLY: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/claim_v2/apply_request.json");
const CLAIM_V1_PUBLISHED: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/claim_v1/published.json");
const CLAIM_V2_PUBLISHED: &[u8] =
    include_bytes!("fixtures/source_forms_shadow/claim_v2/published.json");

#[test]
fn pinned_claim_v1_statement_describe_prepare_apply_replay() {
    let describe = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_INITIAL,
        CLAIM_V1_OWNER,
        include_bytes!("fixtures/source_forms_shadow/claim_v1/describe_request.json"),
    ))
    .unwrap();
    assert!(describe.proposed_form_set.is_none());
    assert_response(
        &describe.response,
        include_bytes!("fixtures/source_forms_shadow/claim_v1/describe.json"),
    );
    let prepared = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_INITIAL,
        CLAIM_V1_OWNER,
        include_bytes!(
            "fixtures/source_forms_shadow/claim_v1/prepare-claim-statement.request.json"
        ),
    ))
    .unwrap();
    assert_response(
        &prepared.response,
        include_bytes!(
            "fixtures/source_forms_shadow/claim_v1/prepare-claim-statement.response.json"
        ),
    );
    let applied = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_INITIAL,
        CLAIM_V1_OWNER,
        CLAIM_V1_APPLY,
    ))
    .unwrap();
    assert_published_bytes(applied.proposed_form_set.as_deref(), CLAIM_V1_PUBLISHED);
    assert_response(
        &applied.response,
        include_bytes!("fixtures/source_forms_shadow/claim_v1/apply.json"),
    );
    let replay = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_V1_PUBLISHED,
        CLAIM_V1_OWNER,
        CLAIM_V1_APPLY,
    ))
    .unwrap();
    assert!(replay.proposed_form_set.is_none());
    assert_response(
        &replay.response,
        include_bytes!("fixtures/source_forms_shadow/claim_v1/replay.json"),
    );
    let denied = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_INITIAL,
        CLAIM_V1_OWNER,
        include_bytes!("fixtures/source_forms_shadow/claim_v2/prepare-claim-name.request.json"),
    ));
    assert!(matches!(denied, Err(ShadowError::Denied(_))));
}

#[test]
fn pinned_claim_v2_display_batch_describe_prepare_apply_replay_and_revocation() {
    let describe = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_INITIAL,
        CLAIM_V2_OWNER,
        include_bytes!("fixtures/source_forms_shadow/claim_v2/describe_request.json"),
    ))
    .unwrap();
    assert_response(
        &describe.response,
        include_bytes!("fixtures/source_forms_shadow/claim_v2/describe.json"),
    );
    for (request, expected) in [
        (
            include_bytes!(
                "fixtures/source_forms_shadow/claim_v2/prepare-claim-statement.request.json"
            )
            .as_slice(),
            include_bytes!(
                "fixtures/source_forms_shadow/claim_v2/prepare-claim-statement.response.json"
            )
            .as_slice(),
        ),
        (
            include_bytes!("fixtures/source_forms_shadow/claim_v2/prepare-claim-name.request.json")
                .as_slice(),
            include_bytes!(
                "fixtures/source_forms_shadow/claim_v2/prepare-claim-name.response.json"
            )
            .as_slice(),
        ),
        (
            include_bytes!(
                "fixtures/source_forms_shadow/claim_v2/prepare-claim-caption.request.json"
            )
            .as_slice(),
            include_bytes!(
                "fixtures/source_forms_shadow/claim_v2/prepare-claim-caption.response.json"
            )
            .as_slice(),
        ),
        (
            include_bytes!(
                "fixtures/source_forms_shadow/claim_v2/prepare-claim-hover.request.json"
            )
            .as_slice(),
            include_bytes!(
                "fixtures/source_forms_shadow/claim_v2/prepare-claim-hover.response.json"
            )
            .as_slice(),
        ),
    ] {
        let prepared =
            run_claim_command(input(CLAIM_SOURCE, CLAIM_INITIAL, CLAIM_V2_OWNER, request)).unwrap();
        assert_response(&prepared.response, expected);
    }
    let applied = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_INITIAL,
        CLAIM_V2_OWNER,
        CLAIM_V2_APPLY,
    ))
    .unwrap();
    assert_published_bytes(applied.proposed_form_set.as_deref(), CLAIM_V2_PUBLISHED);
    assert_response(
        &applied.response,
        include_bytes!("fixtures/source_forms_shadow/claim_v2/apply.json"),
    );
    let replay = run_claim_command(input(
        CLAIM_SOURCE,
        CLAIM_V2_PUBLISHED,
        CLAIM_V2_OWNER,
        CLAIM_V2_APPLY,
    ))
    .unwrap();
    assert_response(
        &replay.response,
        include_bytes!("fixtures/source_forms_shadow/claim_v2/replay.json"),
    );
    let revoked = edit(CLAIM_V2_OWNER,
        "\"allowed_field_ids\": [\n    \"claim.statement\",\n    \"claim.name\",\n    \"claim.caption\",\n    \"claim.hover\"\n  ]",
        "\"allowed_field_ids\": [\"claim.statement\"]");
    assert!(matches!(
        run_claim_command(input(CLAIM_SOURCE, CLAIM_INITIAL, &revoked, CLAIM_V2_APPLY)),
        Err(ShadowError::Denied(_))
    ));
    assert!(matches!(
        run_claim_command(input(
            CLAIM_SOURCE,
            CLAIM_V2_PUBLISHED,
            &revoked,
            CLAIM_V2_APPLY
        )),
        Err(ShadowError::Denied(_))
    ));
}

#[test]
fn pinned_claim_stream_requires_exactly_one_selected_claim() {
    let request = include_bytes!("fixtures/source_forms_shadow/claim_v1/describe_request.json");
    let absent = run_claim_command(input(b"", CLAIM_INITIAL, CLAIM_V1_OWNER, request));
    assert_eq!(
        absent.unwrap_err(),
        ShadowError::Invalid("delegated Claim is not exactly once")
    );
    let mut duplicate = CLAIM_SOURCE.to_vec();
    duplicate.extend_from_slice(CLAIM_SOURCE);
    let repeated = run_claim_command(input(&duplicate, CLAIM_INITIAL, CLAIM_V1_OWNER, request));
    assert_eq!(
        repeated.unwrap_err(),
        ShadowError::Invalid("delegated Claim is not exactly once")
    );
}

fn input<'a>(
    source: &'a [u8],
    set: &'a [u8],
    config: &'a [u8],
    request: &'a [u8],
) -> WorkFormsInput<'a> {
    WorkFormsInput {
        source_raw: source,
        form_set_raw: set,
        owner_config_raw: config,
        request_raw: request,
        recorded_at: INSTANT,
        effective_uid: 1000,
    }
}

fn edit(raw: &[u8], old: &str, new: &str) -> Vec<u8> {
    let value = String::from_utf8(raw.to_vec()).unwrap();
    assert!(value.contains(old), "fixture edit target missing: {old}");
    value.replacen(old, new, 1).into_bytes()
}

fn assert_response(actual: &tos_foundation::JsonValue, expected_raw: &[u8]) {
    let expected = parse_json(
        expected_raw,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    let actual = canonical_digest_v1(
        actual,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    let expected = canonical_digest_v1(
        expected.root(),
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(actual, expected, "exact typed Work command response");
}

fn assert_published_bytes(actual: Option<&[u8]>, expected: &[u8]) {
    let actual = actual.expect("apply must propose a form set");
    if actual != expected {
        let at = actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
            .unwrap_or(actual.len().min(expected.len()));
        let start = at.saturating_sub(48);
        let actual_end = (at + 96).min(actual.len());
        let expected_end = (at + 96).min(expected.len());
        panic!(
            "published bytes differ at {at}; lengths actual={} expected={}; actual={:?}; expected={:?}",
            actual.len(),
            expected.len(),
            String::from_utf8_lossy(&actual[start..actual_end]),
            String::from_utf8_lossy(&expected[start..expected_end]),
        );
    }
}

#[test]
fn portable_work_profile_matches_jgb_describe_prepare_apply_replay() {
    let describe = run_work_command(input(
        SOURCE,
        INITIAL,
        CONFIG,
        br#"{"schema_version":"tos_local_source_command_v1","operation":"describe"}"#,
    ))
    .unwrap();
    assert!(describe.proposed_form_set.is_none());
    assert_response(&describe.response, DESCRIBE_RESPONSE);
    let prepared = run_work_command(input(SOURCE, INITIAL, CONFIG,
        br#"{"schema_version":"tos_local_source_command_v1","operation":"prepare","form_id":"tos.form.jenseits-von-gut-und-boese.name-original","field_id":"metadata.preferred-name"}"#)).unwrap();
    assert!(prepared.proposed_form_set.is_none());
    assert_response(&prepared.response, PREPARE_PREFERRED_RESPONSE);
    let created = run_work_command(input(SOURCE, INITIAL, CONFIG,
        br#"{"schema_version":"tos_local_source_command_v1","operation":"prepare","form_id":"tos.form.oracle.jgb-name-ru-copy","field_id":"metadata.variant-name:0"}"#)).unwrap();
    assert!(created.proposed_form_set.is_none());
    assert_response(&created.response, PREPARE_RUSSIAN_RESPONSE);
    let applied = run_work_command(input(SOURCE, INITIAL, CONFIG, REQUEST)).unwrap();
    assert_published_bytes(applied.proposed_form_set.as_deref(), PUBLISHED);
    assert_response(&applied.response, RESPONSE);
    let replay = run_work_command(input(SOURCE, PUBLISHED, CONFIG, REQUEST)).unwrap();
    assert!(replay.proposed_form_set.is_none());
    assert_response(&replay.response, REPLAY_RESPONSE);
}

#[test]
fn portable_work_profile_matches_independent_ru_cyrl_work() {
    for (request, expected) in [
        (LIPSIUS_DESCRIBE_REQUEST, LIPSIUS_DESCRIBE_RESPONSE),
        (LIPSIUS_PREPARE_REQUEST, LIPSIUS_PREPARE_RESPONSE),
        (LIPSIUS_CREATE_REQUEST, LIPSIUS_CREATE_RESPONSE),
    ] {
        let result = run_work_command(input(
            LIPSIUS_SOURCE,
            LIPSIUS_INITIAL,
            LIPSIUS_CONFIG,
            request,
        ))
        .unwrap();
        assert!(result.proposed_form_set.is_none());
        assert_response(&result.response, expected);
    }
    let applied = run_work_command(input(
        LIPSIUS_SOURCE,
        LIPSIUS_INITIAL,
        LIPSIUS_CONFIG,
        LIPSIUS_APPLY_REQUEST,
    ))
    .unwrap();
    assert_published_bytes(applied.proposed_form_set.as_deref(), LIPSIUS_PUBLISHED);
    assert_response(&applied.response, LIPSIUS_APPLY_RESPONSE);
    let replay = run_work_command(input(
        LIPSIUS_SOURCE,
        LIPSIUS_PUBLISHED,
        LIPSIUS_CONFIG,
        LIPSIUS_APPLY_REQUEST,
    ))
    .unwrap();
    assert!(replay.proposed_form_set.is_none());
    assert_response(&replay.response, LIPSIUS_REPLAY_RESPONSE);
}

#[test]
fn portable_work_profile_refuses_stale_scope_history_and_other_families() {
    let stale = edit(
        REQUEST,
        "sha256:7abcf8fe8b90667ac5ca8e2eb5affe51cc56283fb3ca4f7f26a2e03750f4e25b",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(matches!(
        run_work_command(input(SOURCE, INITIAL, CONFIG, &stale)),
        Err(ShadowError::Conflict(_))
    ));
    let impostor = edit(REQUEST, "agent:cmd2-forms-oracle", "impostor");
    assert!(matches!(
        run_work_command(input(SOURCE, INITIAL, CONFIG, &impostor)),
        Err(ShadowError::Denied(_))
    ));
    let forged = edit(REQUEST, "/identity_status", "/forged-context");
    assert!(matches!(
        run_work_command(input(SOURCE, INITIAL, CONFIG, &forged)),
        Err(ShadowError::Unsupported(_))
    ));
    let reused = edit(
        REQUEST,
        "sha256:7abcf8fe8b90667ac5ca8e2eb5affe51cc56283fb3ca4f7f26a2e03750f4e25b",
        "sha256:68eaceb9d4689d37d5814fc626a7ab67c4b14ed52c654f5569d3a71d041fcb74",
    );
    assert!(matches!(
        run_work_command(input(SOURCE, PUBLISHED, CONFIG, &reused)),
        Err(ShadowError::Conflict(_))
    ));
    let revoked = edit(
        CONFIG,
        "\"allowed_operations\": [\n    \"form.create\",\n    \"form.revise\"\n  ]",
        "\"allowed_operations\": []",
    );
    assert!(matches!(
        run_work_command(input(SOURCE, PUBLISHED, &revoked, REQUEST)),
        Err(ShadowError::Denied(_))
    ));
    let corrupt = edit(
        PUBLISHED,
        "\"prior_forms\": [",
        "\"prior_forms\": [] , \"forged\": [",
    );
    assert!(run_work_command(input(SOURCE, &corrupt, CONFIG, REQUEST)).is_err());
    let claim_source = edit(
        SOURCE,
        "\"record_type\": \"work\"",
        "\"record_type\": \"claim\"",
    );
    assert!(matches!(
        run_work_command(input(&claim_source, INITIAL, CONFIG, REQUEST)),
        Err(ShadowError::Unsupported(_))
    ));
    let mut wrong_uid = input(SOURCE, INITIAL, CONFIG, REQUEST);
    wrong_uid.effective_uid = 1001;
    assert!(matches!(
        run_work_command(wrong_uid),
        Err(ShadowError::Denied(_))
    ));
}

#[test]
fn independent_ru_cyrl_work_rejects_stale_scope_and_language_guard() {
    let stale = edit(
        LIPSIUS_APPLY_REQUEST,
        "sha256:29f1e0d7f8a73d565238753bcd90f01a6b883a605a45b799f5f9f411f7c6dfce",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(matches!(
        run_work_command(input(
            LIPSIUS_SOURCE,
            LIPSIUS_INITIAL,
            LIPSIUS_CONFIG,
            &stale
        )),
        Err(ShadowError::Conflict(_))
    ));
    let impostor = edit(
        LIPSIUS_APPLY_REQUEST,
        "agent:cmd2-de-constantia-oracle",
        "impostor",
    );
    assert!(matches!(
        run_work_command(input(
            LIPSIUS_SOURCE,
            LIPSIUS_INITIAL,
            LIPSIUS_CONFIG,
            &impostor
        )),
        Err(ShadowError::Denied(_))
    ));
    let wrong_guard = edit(
        LIPSIUS_APPLY_REQUEST,
        "/field_languages/notes",
        "/absent-language-guard",
    );
    assert!(matches!(
        run_work_command(input(
            LIPSIUS_SOURCE,
            LIPSIUS_INITIAL,
            LIPSIUS_CONFIG,
            &wrong_guard
        )),
        Err(ShadowError::Unsupported(_))
    ));
    let unknown_field = edit(
        LIPSIUS_PREPARE_REQUEST,
        "metadata.source-note",
        "metadata.unknown",
    );
    assert!(matches!(
        run_work_command(input(
            LIPSIUS_SOURCE,
            LIPSIUS_INITIAL,
            LIPSIUS_CONFIG,
            &unknown_field
        )),
        Err(ShadowError::Invalid(_))
    ));
}

#[test]
fn exact_python_two_change_candidate_and_replay() {
    let result = apply_or_replay(input(SOURCE, INITIAL, CONFIG, REQUEST)).unwrap();
    assert!(!result.replayed);
    assert_eq!(result.form_set_bytes, PUBLISHED);
    assert_eq!(
        result.revision.to_prefixed(),
        "sha256:68eaceb9d4689d37d5814fc626a7ab67c4b14ed52c654f5569d3a71d041fcb74"
    );
    assert_eq!(
        result.request_digest.to_prefixed(),
        "sha256:b1280cd1a73668bed88c003f8bbf5e03cd068b0a2b70672f94bed7ad312a44bf"
    );

    let oracle = parse_json(RESPONSE, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let oracle_receipt = oracle.root().object_get("receipt").unwrap();
    let actual_digest = canonical_digest_v1(
        &result.receipt,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    let oracle_digest = canonical_digest_v1(
        oracle_receipt,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        actual_digest, oracle_digest,
        "embedded receipt semantic fields"
    );

    let replay = apply_or_replay(input(SOURCE, &result.form_set_bytes, CONFIG, REQUEST)).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.form_set_bytes, result.form_set_bytes);
    assert_eq!(replay.revision, result.revision);
    assert_eq!(replay.request_digest, result.request_digest);
    assert_eq!(
        canonical_digest_v1(
            &replay.receipt,
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default()
        )
        .unwrap(),
        actual_digest
    );
}

#[test]
fn stale_scope_input_and_history_refuse_without_output() {
    let stale = edit(
        REQUEST,
        "sha256:7abcf8fe8b90667ac5ca8e2eb5affe51cc56283fb3ca4f7f26a2e03750f4e25b",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(matches!(
        apply_or_replay(input(SOURCE, INITIAL, CONFIG, &stale)),
        Err(ShadowError::Conflict(_))
    ));

    let impostor = edit(REQUEST, "agent:cmd2-forms-oracle", "impostor");
    assert!(matches!(
        apply_or_replay(input(SOURCE, INITIAL, CONFIG, &impostor)),
        Err(ShadowError::Denied(_))
    ));

    let duplicate_id = edit(
        REQUEST,
        "tos.form.oracle.jgb-name-ru-copy",
        "tos.form.jenseits-von-gut-und-boese.name-original",
    );
    assert!(matches!(
        apply_or_replay(input(SOURCE, INITIAL, CONFIG, &duplicate_id)),
        Err(ShadowError::Invalid(_))
    ));

    let changed_prepared_form = edit(REQUEST, "/identity_status", "/forged-context");
    assert!(matches!(
        apply_or_replay(input(SOURCE, INITIAL, CONFIG, &changed_prepared_form)),
        Err(ShadowError::Unsupported(_))
    ));

    let duplicate_key = b"{\"schema_version\":\"x\",\"schema_version\":\"y\"}";
    assert!(matches!(
        apply_or_replay(input(SOURCE, INITIAL, CONFIG, duplicate_key)),
        Err(ShadowError::Invalid(_))
    ));
    let oversize = vec![b' '; 1_048_577];
    assert!(matches!(
        apply_or_replay(input(SOURCE, INITIAL, CONFIG, &oversize)),
        Err(ShadowError::Invalid(_))
    ));

    let revoked = edit(
        CONFIG,
        "\"allowed_operations\": [\n    \"form.create\",\n    \"form.revise\"\n  ]",
        "\"allowed_operations\": []",
    );
    assert!(matches!(
        apply_or_replay(input(SOURCE, PUBLISHED, &revoked, REQUEST)),
        Err(ShadowError::Denied(_))
    ));

    let reused_id = edit(
        REQUEST,
        "sha256:7abcf8fe8b90667ac5ca8e2eb5affe51cc56283fb3ca4f7f26a2e03750f4e25b",
        "sha256:68eaceb9d4689d37d5814fc626a7ab67c4b14ed52c654f5569d3a71d041fcb74",
    );
    assert!(matches!(
        apply_or_replay(input(SOURCE, PUBLISHED, CONFIG, &reused_id)),
        Err(ShadowError::Conflict(_))
    ));

    let broken = edit(
        PUBLISHED,
        "\"prior_forms\": [",
        "\"prior_forms\": [] , \"forged\": [",
    );
    assert!(matches!(
        apply_or_replay(input(SOURCE, &broken, CONFIG, REQUEST)),
        Err(ShadowError::Unsupported(_))
    ));
}

#[test]
fn unknown_work_or_clock_cannot_become_a_general_writer() {
    let other_source = edit(SOURCE, "Jenseits von Gut und Böse", "Different Work");
    assert!(matches!(
        apply_or_replay(input(&other_source, INITIAL, CONFIG, REQUEST)),
        Err(ShadowError::Unsupported(_))
    ));
    let mut other_clock = input(SOURCE, INITIAL, CONFIG, REQUEST);
    other_clock.recorded_at = "2026-01-01T12:34:57+00:00";
    assert!(matches!(
        apply_or_replay(other_clock),
        Err(ShadowError::Unsupported(_))
    ));
}
