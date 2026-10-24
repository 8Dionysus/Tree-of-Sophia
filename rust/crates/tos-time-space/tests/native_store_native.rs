//! Real pinned source record/receipt, not a fabricated authority/profile fixture.
//! Altered inputs below are explicit negative controls and issue no source grants.
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_foundation::{
    CanonicalProfile, Digest256, ExactRecordRef, JsonLimits, JsonMode, RecordVersion, StableId,
    parse_json,
};
use tos_time_space::{
    Allen, CycleStep, DomainLimits, OrderOutcome, Premise, Rational, RelationImage, Span,
    TsCodomain, TsOutputValue, TsProfileBody, TsProjection,
    native_store::{
        NativeTsLimits, NativeTsSession, decode_domain_value, decode_profile_body,
        decode_result_body, encode_profile_body, encode_result_body, typed_projection,
    },
};
use tos_time_space::{
    DomainError,
    native_store::{NativeIdentityFields, check_native_ts_record},
};

const SOURCE: &[u8] = include_bytes!(
    "../../../../ToS/source-witnesses/history/friedrich-nietzsche/jenseits-1886-production/historical-process.json"
);

#[test]
fn native_source_record_pin_uses_owner_canonical_digest() {
    let limits = JsonLimits::new(16_384, 32, 10_000, 128).unwrap();
    let doc = parse_json(SOURCE, JsonMode::PublishedStrict, limits).unwrap();
    let exact = ExactRecordRef {
        id: StableId::parse("tos.historical-process.friedrich-nietzsche.jenseits-1886-production")
            .unwrap(),
        version: RecordVersion::new(1).unwrap(),
        digest: Digest256::from_prefixed(
            "sha256:8b36a058c5d3cd97554cf62e461e0eca31fe3437c5d8ae49365c0eb02eb48f87",
        )
        .unwrap(),
    };
    let checked = check_native_ts_record(
        &doc,
        &exact,
        NativeIdentityFields::Record,
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .unwrap();
    assert!(std::ptr::eq(checked.document(), &doc));
    let mut raw_pin = exact.clone();
    raw_pin.digest = Digest256::of_bytes(SOURCE);
    assert!(matches!(
        check_native_ts_record(
            &doc,
            &raw_pin,
            NativeIdentityFields::Record,
            CanonicalProfile::SourceRecordDigestV1,
            limits
        ),
        Err(DomainError::Incompatible(_))
    ));
    assert!(matches!(
        check_native_ts_record(
            &doc,
            &exact,
            NativeIdentityFields::Record,
            CanonicalProfile::CorpusSnapshotV1,
            limits
        ),
        Err(DomainError::Unsupported(_))
    ));
    let mut wrong_version = exact.clone();
    wrong_version.version = RecordVersion::new(2).unwrap();
    assert!(matches!(
        check_native_ts_record(
            &doc,
            &wrong_version,
            NativeIdentityFields::Record,
            CanonicalProfile::SourceRecordDigestV1,
            limits
        ),
        Err(DomainError::Incompatible(_))
    ));
    assert!(matches!(
        check_native_ts_record(
            &doc,
            &exact,
            NativeIdentityFields::Claim,
            CanonicalProfile::SourceRecordDigestV1,
            limits
        ),
        Err(DomainError::Incompatible(_))
    ));
    let relaxed = parse_json(SOURCE, JsonMode::RequestLastWins, limits).unwrap();
    assert!(matches!(
        check_native_ts_record(
            &relaxed,
            &exact,
            NativeIdentityFields::Record,
            CanonicalProfile::SourceRecordDigestV1,
            limits
        ),
        Err(DomainError::Invalid(_))
    ));
}

fn session(cancelled: &AtomicBool) -> NativeTsSession<'_> {
    NativeTsSession::new(
        NativeTsLimits {
            domain: DomainLimits {
                max_work: 1_000_000,
                max_requested_bytes: 4_000_000,
                max_items: 100_000,
            },
            max_source_files: 32,
            max_source_bytes: 1_000_000,
            canonical: JsonLimits::new(16_384, 32, 10_000, 128).unwrap(),
            canonical_state_bytes: 65_536,
            deadline: Instant::now() + Duration::from_secs(20),
        },
        cancelled,
    )
    .unwrap()
}
fn json(t: &str) -> tos_foundation::JsonValue {
    parse_json(
        t.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}
fn q(n: i128) -> Rational {
    Rational::integer(n).unwrap()
}

#[test]
fn remaining_parent_reservation_keeps_original_spent_prefix() {
    // Numerical ledger only: no native source/authority/profile fixture.
    let cancelled = AtomicBool::new(false);
    let mut original = session(&cancelled);
    let first = original.remaining_allowance().unwrap();
    original.work(7).unwrap();
    original.requested_bytes(11).unwrap();
    original.item().unwrap();
    original.source_read(1, 13).unwrap();
    let next = original.remaining_allowance().unwrap();
    assert_eq!(next.domain.max_work, first.domain.max_work - 20);
    assert_eq!(
        next.domain.max_requested_bytes,
        first.domain.max_requested_bytes - 11
    );
    assert_eq!(next.domain.max_items, first.domain.max_items - 1);
    assert_eq!(next.max_source_files, first.max_source_files - 1);
    assert_eq!(next.max_source_bytes, first.max_source_bytes - 13);
    assert_eq!(next.deadline, first.deadline);
    assert!(matches!(
        original.work(usize::MAX),
        Err(DomainError::ResourceLimit)
    ));
    let after_failure = original.remaining_allowance().unwrap();
    assert_eq!(after_failure.domain.max_work, next.domain.max_work);
    assert_eq!(original.usage().work, 20);
}

#[test]
fn native_five_payload_codecs_and_projection_are_distinct() {
    // Pure domain payloads; none is presented as a registered native profile,
    // selected source, source authority or complete historical world carrier.
    let cancelled = AtomicBool::new(false);
    let mut s = session(&cancelled);
    let cycle = || {
        vec![CycleStep {
            from: 0,
            to: 0,
            strict: true,
            premise: Premise::AnchorOrder {
                left_anchor: 0,
                right_anchor: 1,
            },
        }]
    };
    let mut cases = vec![
        TsOutputValue::SpanMeet(None),
        TsOutputValue::SpanMeet(Some(
            Span::new(
                Rational::new(-i128::MAX, 1).unwrap(),
                Rational::new(i128::MAX, 1).unwrap(),
                true,
                true,
            )
            .unwrap(),
        )),
        TsOutputValue::TopologyValue(true),
        TsOutputValue::TopologyValue(false),
        TsOutputValue::OrderSolution(OrderOutcome::Witness(vec![q(-1), q(1)])),
        TsOutputValue::OrderSolution(OrderOutcome::StrictCycle(cycle())),
        TsOutputValue::RelationImage(RelationImage::NoAdmissibleModel(cycle())),
        TsOutputValue::RelationImage(RelationImage::Exact {
            possible: vec![
                (Allen::Before, vec![q(0), q(1), q(2), q(3)]),
                (Allen::Meets, vec![q(0), q(1), q(1), q(2)]),
            ],
        }),
    ];
    for a in tos_time_space::ALLEN_RELATIONS {
        cases.push(TsOutputValue::AllenRelation(a));
    }
    for value in cases {
        let body = encode_result_body(&value, &mut s).unwrap();
        let (c, decoded) = decode_result_body(&body, &mut s).unwrap();
        assert_eq!(body, encode_result_body(&decoded, &mut s).unwrap());
        let profile = TsProfileBody {
            projection: TsProjection::DirectDomainValueV1,
            codomain: c,
        };
        let p = encode_profile_body(&profile, &mut s).unwrap();
        let p = decode_profile_body(&p, &mut s).unwrap();
        assert!(std::ptr::eq(
            typed_projection(&p, c, &decoded, &mut s).unwrap(),
            &decoded
        ));
    }
    // Whole-wrapper schema is NOT the native /body schema or typed identity.
    assert!(matches!(
        decode_profile_body(
            &json(
                r#"{"record_id":"synthetic-negative-wrapper","body":{"projection":"DirectDomainValueV1","codomain":"TopologyValue"}}"#
            ),
            &mut s
        ),
        Err(DomainError::Invalid(_))
    ));
    assert!(matches!(
        decode_profile_body(
            &json(r#"{"projection":"/body","codomain":"TopologyValue"}"#),
            &mut s
        ),
        Err(DomainError::Unsupported(_))
    ));
    let p = TsProfileBody {
        projection: TsProjection::DirectDomainValueV1,
        codomain: TsCodomain::TopologyValue,
    };
    assert!(matches!(
        typed_projection(
            &p,
            TsCodomain::SpanMeet,
            &TsOutputValue::TopologyValue(true),
            &mut s
        ),
        Err(DomainError::Incompatible(_))
    ));
    for raw in [
        r#"{"codomain":"SpanMeet","value":{"lower":{"n":"2","d":"2"},"upper":{"n":"2","d":"1"},"lower_closed":true,"upper_closed":true}}"#,
        r#"{"codomain":"OrderSolution","value":{"kind":"Witness","endpoints":[]}}"#,
        r#"{"codomain":"RelationImage","value":{"kind":"Exact","possible":[]}}"#,
        r#"{"codomain":"TopologyValue","value":true,"extra":null}"#,
    ] {
        assert!(matches!(
            decode_result_body(&json(raw), &mut s),
            Err(DomainError::Invalid(_))
        ));
    }
    let cancelled = AtomicBool::new(true);
    let limits = NativeTsLimits {
        domain: DomainLimits {
            max_work: 2,
            max_requested_bytes: 1,
            max_items: 1,
        },
        max_source_files: 1,
        max_source_bytes: 1,
        canonical: JsonLimits::default(),
        canonical_state_bytes: 1,
        deadline: Instant::now() + Duration::from_secs(20),
    };
    assert!(matches!(
        NativeTsSession::new(limits, &cancelled),
        Err(DomainError::ResourceLimit)
    ));
    let active = AtomicBool::new(false);
    let mut low = NativeTsSession::new(limits, &active).unwrap();
    decode_domain_value(TsCodomain::TopologyValue, &json("true"), &mut low).unwrap();
    decode_domain_value(TsCodomain::TopologyValue, &json("false"), &mut low).unwrap();
    assert!(matches!(
        decode_domain_value(TsCodomain::TopologyValue, &json("true"), &mut low),
        Err(DomainError::ResourceLimit)
    ));
}
