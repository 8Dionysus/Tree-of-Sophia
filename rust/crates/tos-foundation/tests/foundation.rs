use tos_foundation::{
    CanonicalProfile, CodePointSpan, Digest256, Digest256Hasher, FoundationErrorCode,
    JsonEmissionProfile, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    LogicalRecordRefV1, RelativePath, StableId, UnicodeProfile, canonical_bytes_v1,
    canonical_count_v1, emit_json_profile, emit_preserved_json, parse_json,
    python_casefold_unicode16_v1, python_lower_unicode16_v1, python_strip_unicode16_v1,
};

#[test]
fn exact_hash_stream_and_lexical_types() {
    let mut stream = Digest256Hasher::new();
    stream.update(b"a");
    stream.update(b"bc");
    let digest = stream.finalize();
    assert_eq!(digest, Digest256::of_bytes(b"abc"));
    assert_eq!(
        digest.to_hex(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(Digest256::from_hex(&digest.to_hex()).unwrap(), digest);
    assert!(Digest256::from_hex(&digest.to_hex().to_uppercase()).is_err());
    assert_eq!(
        StableId::parse("tos.new-family.subject-é")
            .unwrap()
            .as_str(),
        "tos.new-family.subject-é"
    );
    assert_eq!(
        RelativePath::parse("records/café.json").unwrap().as_str(),
        "records/café.json"
    );
    for path in [
        "/absolute",
        "a//b",
        "a/../b",
        "a/.git/b",
        "a\\b",
        "a/\u{0001}",
    ] {
        assert_eq!(
            RelativePath::parse(path).unwrap_err().code,
            FoundationErrorCode::UnsafePath
        );
    }
}

#[test]
fn request_duplicate_keeps_first_position_and_last_number_context() {
    let raw = br#"{"a":1,"b":2,"\u0061":3.0}"#;
    let doc = parse_json(raw, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
    let object = doc.root().as_object().unwrap();
    assert_eq!(
        object
            .iter()
            .map(|(key, _)| key.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    match doc.root().object_get("a").unwrap() {
        JsonValue::Number(number) => {
            assert_eq!(number.kind, JsonNumberKind::Float);
            assert_eq!(number.lexeme, "3.0");
        }
        _ => panic!("expected number"),
    }
    assert_eq!(
        emit_preserved_json(&doc, JsonLimits::default()).unwrap(),
        br#"{"a":3.0,"b":2}"#
    );
    assert_eq!(
        parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap_err()
            .code,
        FoundationErrorCode::DuplicateMember
    );
}

#[test]
fn canonical_profile_sorts_and_distinguishes_surrogate_boundary() {
    let doc = parse_json(
        "{\"b\":2,\"a\":\"é\"}".as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        canonical_bytes_v1(
            doc.root(),
            CanonicalProfile::CorpusSnapshotV1,
            JsonLimits::default()
        )
        .unwrap(),
        "{\"a\":\"é\",\"b\":2}\n".as_bytes()
    );
    assert_eq!(
        canonical_count_v1(
            doc.root(),
            CanonicalProfile::CorpusSnapshotV1,
            JsonLimits::default()
        )
        .unwrap(),
        "{\"a\":\"é\",\"b\":2}\n".as_bytes().len()
    );
    let surrogate = parse_json(
        br#""\ud800""#,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        emit_preserved_json(&surrogate, JsonLimits::default()).unwrap(),
        br#""\ud800""#
    );
    assert_eq!(
        canonical_bytes_v1(
            surrogate.root(),
            CanonicalProfile::CorpusSnapshotV1,
            JsonLimits::default()
        )
        .unwrap_err()
        .code,
        FoundationErrorCode::InvalidUnicodeScalar
    );
    assert_eq!(
        canonical_count_v1(
            surrogate.root(),
            CanonicalProfile::CorpusSnapshotV1,
            JsonLimits::default()
        )
        .unwrap_err()
        .code,
        FoundationErrorCode::InvalidUnicodeScalar
    );
    let float = parse_json(br#"1.25"#, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(
        canonical_bytes_v1(
            float.root(),
            CanonicalProfile::CorpusSnapshotV1,
            JsonLimits::default()
        )
        .unwrap(),
        b"1.25\n"
    );
}

#[test]
fn code_point_offsets_bind_exact_utf8_bytes() {
    let text = "AéΩ";
    let digest = Digest256::of_bytes(text.as_bytes());
    let span = CodePointSpan::new(1, 3, digest, "source-exact-v1").unwrap();
    let bytes = span.byte_span_in(text).unwrap();
    assert_eq!((bytes.start, bytes.end), (1, 5));
    assert!(span.byte_span_in("AéO").is_err());
}

#[test]
fn canonical_output_rejects_unparsed_values_with_invalid_shape() {
    let limits = JsonLimits::default();
    let number = JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: "01".to_owned(),
    });
    assert_eq!(
        canonical_bytes_v1(&number, CanonicalProfile::CorpusSnapshotV1, limits)
            .unwrap_err()
            .code,
        FoundationErrorCode::InvalidNumber
    );
    assert_eq!(
        canonical_count_v1(&number, CanonicalProfile::CorpusSnapshotV1, limits)
            .unwrap_err()
            .code,
        FoundationErrorCode::InvalidNumber
    );
    let duplicate = JsonValue::Object(vec![
        (JsonString::from_utf8("a"), JsonValue::Null),
        (JsonString::from_utf8("a"), JsonValue::Bool(true)),
    ]);
    assert_eq!(
        canonical_bytes_v1(&duplicate, CanonicalProfile::CorpusSnapshotV1, limits)
            .unwrap_err()
            .code,
        FoundationErrorCode::DuplicateMember
    );
    assert_eq!(
        canonical_count_v1(&duplicate, CanonicalProfile::CorpusSnapshotV1, limits)
            .unwrap_err()
            .code,
        FoundationErrorCode::DuplicateMember
    );
}

#[test]
fn writer_enforces_byte_limit_before_expanding_escapes() {
    let document = parse_json(
        br#""\u0000\u0000\u0000""#,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    let limits = JsonLimits::new(12, 64, 100, 100).unwrap();
    assert_eq!(
        emit_preserved_json(&document, limits).unwrap_err().code,
        FoundationErrorCode::BudgetExceeded
    );
    assert_eq!(
        canonical_bytes_v1(document.root(), CanonicalProfile::CorpusSnapshotV1, limits)
            .unwrap_err()
            .code,
        FoundationErrorCode::BudgetExceeded
    );
    assert_eq!(
        canonical_count_v1(document.root(), CanonicalProfile::CorpusSnapshotV1, limits)
            .unwrap_err()
            .code,
        FoundationErrorCode::BudgetExceeded
    );
    let nested = JsonValue::Array(vec![JsonValue::Array(vec![JsonValue::Null])]);
    for limit in [
        JsonLimits::new(100, 1, 100, 100).unwrap(),
        JsonLimits::new(100, 64, 2, 100).unwrap(),
    ] {
        assert_eq!(
            canonical_count_v1(&nested, CanonicalProfile::SourceCommandInputV1, limit)
                .unwrap_err()
                .code,
            canonical_bytes_v1(&nested, CanonicalProfile::SourceCommandInputV1, limit)
                .unwrap_err()
                .code
        );
    }
    assert_eq!(
        JsonLimits::new(100, usize::MAX, 100, 100).unwrap_err().code,
        FoundationErrorCode::BudgetExceeded
    );
}

#[test]
fn named_profiles_keep_their_distinct_exact_bytes() {
    let limits = JsonLimits::default();
    let parsed = tos_foundation::parse_json_profile(
        br#"{"b":-0,"a":900719925474099312345678901234567890}"#,
        "tos_published_json_v1",
        limits,
    )
    .unwrap();
    assert_eq!(parsed.root().object_get("a").unwrap().as_u64(), None);
    let no_lf = b"{\"a\":900719925474099312345678901234567890,\"b\":0}";
    for profile in [
        CanonicalProfile::SourceRecordDigestV1,
        CanonicalProfile::SourceCommandInputV1,
    ] {
        assert_eq!(
            canonical_bytes_v1(parsed.root(), profile, limits).unwrap(),
            no_lf
        );
        assert_eq!(
            canonical_count_v1(parsed.root(), profile, limits).unwrap(),
            no_lf.len()
        );
    }
    let mut snapshot = no_lf.to_vec();
    snapshot.push(b'\n');
    assert_eq!(
        canonical_bytes_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, limits).unwrap(),
        snapshot
    );
    assert_eq!(
        canonical_count_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, limits).unwrap(),
        snapshot.len()
    );
    let exact = JsonLimits::new(no_lf.len(), 64, 100, 100).unwrap();
    assert_eq!(
        canonical_count_v1(parsed.root(), CanonicalProfile::SourceCommandInputV1, exact).unwrap(),
        no_lf.len()
    );
    assert_eq!(
        canonical_count_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, exact)
            .unwrap_err()
            .code,
        canonical_bytes_v1(parsed.root(), CanonicalProfile::CorpusSnapshotV1, exact)
            .unwrap_err()
            .code
    );
    assert_eq!(
        CanonicalProfile::from_profile("unknown").unwrap_err().code,
        FoundationErrorCode::UnsupportedFormat
    );
    assert_eq!(
        tos_foundation::JsonMode::from_profile("unknown")
            .unwrap_err()
            .code,
        FoundationErrorCode::UnsupportedFormat
    );
    assert_eq!(
        tos_foundation::canonical_raw_bytes_v1(
            br#"{"command_id":"same","command_id":"changed"}"#,
            CanonicalProfile::SourceCommandInputV1,
            limits,
        )
        .unwrap_err()
        .code,
        FoundationErrorCode::DuplicateMember
    );
}

#[test]
fn python_float_layout_boundaries() {
    let limits = JsonLimits::default();
    for (raw, expected) in [
        ("-0.0", "-0.0"),
        ("1.0", "1.0"),
        ("1e-4", "0.0001"),
        ("1e-5", "1e-05"),
        ("1e15", "1000000000000000.0"),
        ("1e16", "1e+16"),
        ("1e20", "1e+20"),
        ("1.234e-7", "1.234e-07"),
        ("5e-324", "5e-324"),
        ("1.7976931348623157e308", "1.7976931348623157e+308"),
        ("1e-400", "0.0"),
    ] {
        let parsed = parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits).unwrap();
        assert_eq!(
            canonical_bytes_v1(
                parsed.root(),
                CanonicalProfile::SourceRecordDigestV1,
                limits
            )
            .unwrap(),
            expected.as_bytes(),
            "{raw}"
        );
        assert_eq!(
            canonical_count_v1(
                parsed.root(),
                CanonicalProfile::SourceRecordDigestV1,
                limits
            )
            .unwrap(),
            expected.len(),
            "{raw}"
        );
    }
}

#[test]
fn whole_form_set_profile_preserves_order_and_python_layout() {
    let limits = JsonLimits::default();
    let raw = "{\"z\":{\"empty\":{},\"items\":[1.0,{\"Ω\":\"é\"},[],null]},\"a\":18446744073709551616,\"growth_history\":[{\"command_id\":\"c\",\"request_digest\":\"sha256:x\"}]}";
    let parsed = parse_json(raw.as_bytes(), JsonMode::PublishedStrict, limits).unwrap();
    let expected = concat!(
        "{\n",
        "  \"z\": {\n",
        "    \"empty\": {},\n",
        "    \"items\": [\n",
        "      1.0,\n",
        "      {\n",
        "        \"Ω\": \"é\"\n",
        "      },\n",
        "      [],\n",
        "      null\n",
        "    ]\n",
        "  },\n",
        "  \"a\": 18446744073709551616,\n",
        "  \"growth_history\": [\n",
        "    {\n",
        "      \"command_id\": \"c\",\n",
        "      \"request_digest\": \"sha256:x\"\n",
        "    }\n",
        "  ]\n",
        "}\n",
    );
    let encoded = emit_json_profile(
        parsed.root(),
        JsonEmissionProfile::SourceFormSetPublishedV1,
        limits,
    )
    .unwrap();
    assert_eq!(encoded.bytes, expected.as_bytes());
    assert_eq!(
        encoded.sha256.to_hex(),
        "3fff42c509876255bd080702a4b8b411e3626cbe1cf8cd2474e7d56778f22f00"
    );
    let tiny = JsonLimits::new(32, 64, 300_000, 4_300).unwrap();
    assert_eq!(
        emit_json_profile(
            parsed.root(),
            JsonEmissionProfile::SourceFormSetPublishedV1,
            tiny
        )
        .unwrap_err()
        .code,
        FoundationErrorCode::BudgetExceeded
    );
    assert_eq!(
        JsonEmissionProfile::from_profile("unknown")
            .unwrap_err()
            .code,
        FoundationErrorCode::UnsupportedFormat
    );
}

#[test]
fn logical_record_reference_has_exact_bounded_binary_identity() {
    let content = Digest256::from_bytes(std::array::from_fn(|index| index as u8));
    let identity =
        LogicalRecordRefV1::new(b"lab", b"p", b"1", &[0xff, b'A'], &[b'r', 0], content, 5).unwrap();
    let expected_hex = concat!(
        "544f534c010003006c616201007001003102000000ff41020000007200",
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        "0500000000000000",
    );
    let expected = expected_hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    let encoded = identity.encode();
    assert_eq!(encoded, expected);
    assert_eq!(
        identity.digest().to_hex(),
        "d7ad3667d125d9bb42bf77374db117ed91c597e786f59d6ab2f12555b04d0197"
    );
    assert_eq!(LogicalRecordRefV1::decode(&encoded).unwrap(), identity);
    assert_eq!(identity.subject(), &[0xff, b'A']);
    for cut in 0..encoded.len() {
        assert!(
            LogicalRecordRefV1::decode(&encoded[..cut]).is_err(),
            "truncation {cut}"
        );
    }
    let mut extra = encoded.clone();
    extra.push(0);
    assert_eq!(
        LogicalRecordRefV1::decode(&extra).unwrap_err().code,
        FoundationErrorCode::InvalidFrame
    );
    let mut unknown = encoded.clone();
    unknown[4] = 2;
    assert_eq!(
        LogicalRecordRefV1::decode(&unknown).unwrap_err().code,
        FoundationErrorCode::UnsupportedFormat
    );
    assert_eq!(
        LogicalRecordRefV1::new(b"", b"p", b"1", b"s", b"r", content, 5)
            .unwrap_err()
            .code,
        FoundationErrorCode::InvalidFrame
    );
    assert_eq!(
        LogicalRecordRefV1::new(&[b'x'; 256], b"p", b"1", b"s", b"r", content, 5)
            .unwrap_err()
            .code,
        FoundationErrorCode::InvalidFrame
    );
}

#[test]
fn unicode16_lower_and_strip_are_versioned_and_bounded() {
    assert_eq!(
        UnicodeProfile::PythonNativeUnicodeV1.as_str(),
        "tos-python-native-unicode-v1"
    );
    assert_eq!(
        UnicodeProfile::PythonNativeUnicodeV1.ucd_version(),
        "16.0.0"
    );
    assert_eq!(
        UnicodeProfile::from_profile("unknown").unwrap_err().code,
        FoundationErrorCode::UnsupportedFormat
    );
    let lower = |input| python_lower_unicode16_v1(input, 256, 256, 1024).unwrap();
    assert_eq!(lower("ΣΟΣ"), "σος");
    assert_eq!(lower("İ"), "i\u{307}");
    assert_eq!(lower("ẞ Straße K 𐐀"), "ß straße k 𐐨");
    assert_eq!(
        python_strip_unicode16_v1("\u{85}\u{2003}  ΣΟΣ\u{3000}", 256).unwrap(),
        "ΣΟΣ"
    );
    assert_eq!(
        python_lower_unicode16_v1("İ", 1, 1, 8).unwrap_err().code,
        FoundationErrorCode::BudgetExceeded
    );
    assert_eq!(
        python_lower_unicode16_v1("İ", 1, 2, 2).unwrap_err().code,
        FoundationErrorCode::BudgetExceeded
    );
    assert_eq!(
        python_strip_unicode16_v1("ΣΟΣ", 2).unwrap_err().code,
        FoundationErrorCode::BudgetExceeded
    );
}

#[test]
fn unicode16_full_casefold_matches_python_default_and_bounds() {
    let fold = |input| python_casefold_unicode16_v1(input, 256, 768, 3072).unwrap();
    // Independent default-full mappings from Unicode 16 CaseFolding.txt C + F.
    assert_eq!(fold("ẞ Straße ﬃ İ I ı"), "ss strasse ffi i\u{307} i ı");
    assert_eq!(fold("ΣΟΣ Σοσ Σος ς"), "σοσ σοσ σοσ σ");
    assert_eq!(fold("µ K ſ 𐐀"), "μ k s 𐐨");
    // Cherokee folds to uppercase; a lowercase shortcut is observably wrong.
    assert_eq!(fold("Ꭰ ꭰ"), "Ꭰ Ꭰ");
    // Unicode 16 additions, independent of older host Unicode tables.
    assert_eq!(
        fold("\u{1c89}\u{a7cb}\u{10d50}"),
        "\u{1c8a}\u{264}\u{10d70}"
    );
    assert_eq!(fold("é e\u{301} 🙂"), "é e\u{301} 🙂");
    assert_eq!(fold(&fold("ẞ Σος ﬃ ꭰ")), fold("ẞ Σος ﬃ ꭰ"));
    assert_eq!(python_casefold_unicode16_v1("", 0, 0, 0).unwrap(), "");
    assert_eq!(python_casefold_unicode16_v1("ß", 1, 2, 2).unwrap(), "ss");
    assert_eq!(
        python_casefold_unicode16_v1("İ", 1, 2, 3).unwrap(),
        "i\u{307}"
    );
    for (text, input, points, bytes) in [
        ("ß", 0, 2, 2),
        ("ß", 1, 1, 2),
        ("ß", 1, 2, 1),
        ("İ", 1, 2, 2),
        ("🙂", 1, 1, 3),
        ("aaß", 3, 3, 4),
    ] {
        assert_eq!(
            python_casefold_unicode16_v1(text, input, points, bytes)
                .unwrap_err()
                .code,
            FoundationErrorCode::BudgetExceeded
        );
    }
}
