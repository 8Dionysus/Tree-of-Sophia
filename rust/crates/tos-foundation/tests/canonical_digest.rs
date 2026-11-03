use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, jcs_bytes_v1, jcs_digest_v1,
    parse_json, verify_canonical_digest_v1,
};

fn value(raw: &str) -> JsonValue {
    parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}

#[test]
fn tagged_jcs_record_round_trips_with_exact_number_strings() {
    let raw = r#"{"large_integer":"9007199254740993","negative_zero":"-0.0"}"#;
    let record = value(raw);
    let digest = jcs_digest_v1(&record, JsonLimits::default()).unwrap();
    assert_eq!(digest.profile, "jcs1");
    assert!(
        verify_canonical_digest_v1(
            &value(raw),
            Some(digest.profile),
            CanonicalProfile::SourceCommandInputV1,
            digest.digest,
            JsonLimits::default(),
        )
        .unwrap()
    );
    assert!(
        jcs_digest_v1(
            &value(r#"{"large_integer":9007199254740993,"negative_zero":-0.0}"#),
            JsonLimits::default(),
        )
        .is_err()
    );
    assert!(jcs_digest_v1(&value("9007199254740994"), JsonLimits::default()).is_err());
    let exact_large_decimal = value("9007199254740994.0");
    assert!(jcs_bytes_v1(&exact_large_decimal, JsonLimits::default()).is_ok());
    assert!(jcs_digest_v1(&exact_large_decimal, JsonLimits::default()).is_err());
    assert!(jcs_digest_v1(&value("9007199254740992"), JsonLimits::default()).is_ok());
    // An exactly representable binary64 integer can still exceed the ToS
    // admission limit. Verification applies the same policy as writing.
    assert!(
        verify_canonical_digest_v1(
            &value("9007199254740994"),
            Some("jcs1"),
            CanonicalProfile::SourceCommandInputV1,
            Digest256::of_bytes(b"9007199254740994"),
            JsonLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn untagged_historical_profile_and_mismatched_tag() {
    let record = value(r#"{"n":1.0}"#);
    let historical = Digest256::of_bytes(b"{\"n\":1.0}\n");
    assert!(
        verify_canonical_digest_v1(
            &record,
            None,
            CanonicalProfile::CorpusSnapshotV1,
            historical,
            JsonLimits::default(),
        )
        .unwrap()
    );
    assert!(
        verify_canonical_digest_v1(
            &record,
            Some(CanonicalProfile::CorpusSnapshotV1.as_str()),
            CanonicalProfile::CorpusSnapshotV1,
            historical,
            JsonLimits::default(),
        )
        .unwrap()
    );
    assert!(
        !verify_canonical_digest_v1(
            &record,
            Some("jcs1"),
            CanonicalProfile::CorpusSnapshotV1,
            historical,
            JsonLimits::default(),
        )
        .unwrap()
    );
    assert!(
        verify_canonical_digest_v1(
            &record,
            Some("unsupported"),
            CanonicalProfile::CorpusSnapshotV1,
            historical,
            JsonLimits::default(),
        )
        .is_err()
    );
    assert!(
        verify_canonical_digest_v1(
            &record,
            Some(CanonicalProfile::SourceCommandInputV1.as_str()),
            CanonicalProfile::CorpusSnapshotV1,
            historical,
            JsonLimits::default(),
        )
        .is_err()
    );
}
