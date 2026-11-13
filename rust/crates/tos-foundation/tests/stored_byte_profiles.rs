use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, canonical_bytes_v1,
    emit_json_profile, emit_ordered_compact_json_v1,
    emit_ordered_compact_json_v1_with_state_budget_and_visits_and_check,
    emit_sorted_indent2_json_v1_with_state_budget, parse_json,
};

// Frozen from the historical serializer recipes documented at commit
// 91004cc2cce4d52690d1077140875c980bf8268a in
// tests/conformance/rust/{README.md,history-v1/generate_oracle.py};
// The combined input was captured with CPython 3.14.8 from those recipes.
// Historical form-set source: 60e96eceb0903d74e287217fe5e8c1a891f4ab74,
// mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py.
#[test]
fn stored_profiles_have_fixed_bytes() {
    let limits = JsonLimits::default();
    let input = r#"{"z":[-0,-0.0,18446744073709551616,{"Ω":"é\n\"\\\u0001","a":1e-5}],"a":true}"#;
    let value = parse_json(input.as_bytes(), JsonMode::PublishedStrict, limits).unwrap();
    let ordered = r#"{"z":[0,-0.0,18446744073709551616,{"Ω":"é\n\"\\\u0001","a":1e-05}],"a":true}"#;
    let sorted = r#"{"a":true,"z":[0,-0.0,18446744073709551616,{"a":1e-05,"Ω":"é\n\"\\\u0001"}]}"#;
    let ordered_indent = concat!(
        "{\n  \"z\": [\n    0,\n    -0.0,\n    18446744073709551616,\n    {\n",
        "      \"Ω\": \"é\\n\\\"\\\\\\u0001\",\n      \"a\": 1e-05\n    }\n  ],\n  \"a\": true\n}"
    );
    let sorted_indent = concat!(
        "{\n  \"a\": true,\n  \"z\": [\n    0,\n    -0.0,\n    18446744073709551616,\n    {\n",
        "      \"a\": 1e-05,\n      \"Ω\": \"é\\n\\\"\\\\\\u0001\"\n    }\n  ]\n}"
    );
    let root = value.root();
    assert_eq!(
        emit_ordered_compact_json_v1(root, limits).unwrap(),
        ordered.as_bytes()
    );
    let mut check = || Ok(());
    let mut admit = |_, _| Ok(());
    assert_eq!(
        emit_ordered_compact_json_v1_with_state_budget_and_visits_and_check(
            root, limits, 1_000_000, &mut check, &mut admit
        )
        .unwrap()
        .0,
        ordered.as_bytes()
    );
    for profile in [
        CanonicalProfile::SourceRecordDigestV1,
        CanonicalProfile::SourceCommandInputV1,
    ] {
        assert_eq!(
            canonical_bytes_v1(root, profile, limits).unwrap(),
            sorted.as_bytes()
        );
    }
    assert_eq!(
        canonical_bytes_v1(root, CanonicalProfile::CorpusSnapshotV1, limits).unwrap(),
        format!("{sorted}\n").as_bytes()
    );
    assert_eq!(
        emit_sorted_indent2_json_v1_with_state_budget(
            root, limits, 1_000_000, &mut check, &mut admit
        )
        .unwrap()
        .0,
        sorted_indent.as_bytes()
    );
    for profile in [
        JsonEmissionProfile::SourceFormSetPublishedV1,
        JsonEmissionProfile::SourceWitnessCatalogPublishedV3,
    ] {
        assert_eq!(
            emit_json_profile(root, profile, limits).unwrap().bytes,
            format!("{ordered_indent}\n").as_bytes()
        );
    }
    assert_eq!(
        emit_json_profile(
            root,
            JsonEmissionProfile::SourceFoundationLabReportV1,
            limits
        )
        .unwrap()
        .bytes,
        format!("{sorted_indent}\n").as_bytes()
    );
}

#[test]
fn retained_canonical_digest_vector_is_unchanged() {
    // tests/conformance/rust/canonical-profiles-v1.jsonl at 91004cc2cce4d52690d1077140875c980bf8268a
    let limits = JsonLimits::default();
    let doc = parse_json(br#"{"n":-0.0}"#, JsonMode::PublishedStrict, limits).unwrap();
    let bytes = canonical_bytes_v1(doc.root(), CanonicalProfile::CorpusSnapshotV1, limits).unwrap();
    assert_eq!(bytes, b"{\"n\":-0.0}\n");
    assert_eq!(
        Digest256::of_bytes(&bytes).to_hex(),
        "63ac50753737f63908c4ff9b25d30491d0307332682eb0dc25247c04e06c1d55"
    );
}
