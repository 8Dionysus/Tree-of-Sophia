// Intentionally path-imported until CMD/OPS review chooses an internal route.
// No public `tos_command` source writer is exposed by this differential test.
#[path = "../src/source_forms_shadow.rs"]
mod source_forms_shadow;

use source_forms_shadow::{apply_or_replay, ShadowError, WorkFormsInput};
use tos_foundation::{canonical_digest_v1, parse_json, CanonicalProfile, JsonLimits, JsonMode};

const SOURCE: &[u8] = include_bytes!("fixtures/source_forms_shadow/source.initial.json");
const INITIAL: &[u8] = include_bytes!("fixtures/source_forms_shadow/form-set.initial.json");
const PUBLISHED: &[u8] = include_bytes!("fixtures/source_forms_shadow/form-set.published.json");
const CONFIG: &[u8] = include_bytes!("fixtures/source_forms_shadow/owner.synthetic.json");
const REQUEST: &[u8] = include_bytes!("fixtures/source_forms_shadow/apply.request.json");
const RESPONSE: &[u8] = include_bytes!("fixtures/source_forms_shadow/apply.response.json");
const INSTANT: &str = "2026-01-01T12:34:56+00:00";

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
    }
}

fn edit(raw: &[u8], old: &str, new: &str) -> Vec<u8> {
    let value = String::from_utf8(raw.to_vec()).unwrap();
    assert!(value.contains(old), "fixture edit target missing: {old}");
    value.replacen(old, new, 1).into_bytes()
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
