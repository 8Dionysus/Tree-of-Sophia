use tos_foundation::{
    CanonicalProfile, CodePointSpan, ContractDescriptor, ContractKey, DescriptorRegistry, Digest256,
    Digest256Hasher, FoundationErrorCode, JsonLimits, JsonMode, JsonNumberKind, JsonValue,
    JsonNumber, JsonString, OperationDescriptor, OperationEffect, RelativePath, StableId, canonical_bytes_v1,
    emit_preserved_json, parse_json,
};

#[test]
fn exact_hash_stream_and_lexical_types() {
    let mut stream = Digest256Hasher::new();
    stream.update(b"a");
    stream.update(b"bc");
    let digest = stream.finalize();
    assert_eq!(digest, Digest256::of_bytes(b"abc"));
    assert_eq!(digest.to_hex(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(Digest256::from_hex(&digest.to_hex()).unwrap(), digest);
    assert!(Digest256::from_hex(&digest.to_hex().to_uppercase()).is_err());
    assert_eq!(StableId::parse("tos.new-family.subject-é").unwrap().as_str(), "tos.new-family.subject-é");
    assert_eq!(RelativePath::parse("records/café.json").unwrap().as_str(), "records/café.json");
    for path in ["/absolute", "a//b", "a/../b", "a/.git/b", "a\\b", "a/\u{0001}"] {
        assert_eq!(RelativePath::parse(path).unwrap_err().code, FoundationErrorCode::UnsafePath);
    }
}

#[test]
fn request_duplicate_keeps_first_position_and_last_number_context() {
    let raw = br#"{"a":1,"b":2,"\u0061":3.0}"#;
    let doc = parse_json(raw, JsonMode::RequestLastWins, JsonLimits::default()).unwrap();
    let object = doc.root().as_object().unwrap();
    assert_eq!(object.iter().map(|(key, _)| key.as_str().unwrap()).collect::<Vec<_>>(), ["a", "b"]);
    match doc.root().object_get("a").unwrap() {
        JsonValue::Number(number) => {
            assert_eq!(number.kind, JsonNumberKind::Float);
            assert_eq!(number.lexeme, "3.0");
        }
        _ => panic!("expected number"),
    }
    assert_eq!(emit_preserved_json(&doc, JsonLimits::default()).unwrap(), br#"{"a":3.0,"b":2}"#);
    assert_eq!(parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap_err().code,
               FoundationErrorCode::DuplicateMember);
}

#[test]
fn canonical_profile_sorts_and_distinguishes_surrogate_boundary() {
    let doc = parse_json("{\"b\":2,\"a\":\"é\"}".as_bytes(), JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(canonical_bytes_v1(doc.root(), CanonicalProfile::CorpusSnapshotV1, JsonLimits::default()).unwrap(),
               "{\"a\":\"é\",\"b\":2}\n".as_bytes());
    let surrogate = parse_json(br#""\ud800""#, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(emit_preserved_json(&surrogate, JsonLimits::default()).unwrap(), br#""\ud800""#);
    assert_eq!(canonical_bytes_v1(surrogate.root(), CanonicalProfile::CorpusSnapshotV1, JsonLimits::default()).unwrap_err().code,
               FoundationErrorCode::InvalidUnicodeScalar);
    let float = parse_json(br#"1.25"#, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(canonical_bytes_v1(float.root(), CanonicalProfile::CorpusSnapshotV1, JsonLimits::default()).unwrap_err().code,
               FoundationErrorCode::UnsupportedCanonicalNumber);
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
fn descriptors_are_extensible_but_not_authority() {
    let mut registry = DescriptorRegistry::new();
    let input = ContractKey::new("tos.source.new-kind", "v1").unwrap();
    let output = ContractKey::new("tos.access.new-kind-view", "v2").unwrap();
    for key in [input.clone(), output.clone()] {
        registry.register_contract(ContractDescriptor::new(key, "ToS/contracts/owner.json", Digest256::of_bytes(b"schema"), "tos_foundation_json_v1").unwrap()).unwrap();
    }
    registry.register_operation(OperationDescriptor::new("tos.new-kind.read", "v1", input, output, OperationEffect::Read).unwrap()).unwrap();
    assert!(registry.operation("tos.new-kind.read", "v1").is_some());
    assert!(registry.operation("tos.unknown", "v1").is_none());
}

#[test]
fn canonical_output_rejects_unparsed_values_with_invalid_shape() {
    let limits = JsonLimits::default();
    let number = JsonValue::Number(JsonNumber { kind: JsonNumberKind::Int, lexeme: "01".to_owned() });
    assert_eq!(canonical_bytes_v1(&number, CanonicalProfile::CorpusSnapshotV1, limits).unwrap_err().code,
               FoundationErrorCode::InvalidNumber);
    let duplicate = JsonValue::Object(vec![
        (JsonString::from_utf8("a"), JsonValue::Null),
        (JsonString::from_utf8("a"), JsonValue::Bool(true)),
    ]);
    assert_eq!(canonical_bytes_v1(&duplicate, CanonicalProfile::CorpusSnapshotV1, limits).unwrap_err().code,
               FoundationErrorCode::DuplicateMember);
}

#[test]
fn writer_enforces_byte_limit_before_expanding_escapes() {
    let document = parse_json(br#""\u0000\u0000\u0000""#, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let limits = JsonLimits::new(12, 64, 100, 100).unwrap();
    assert_eq!(emit_preserved_json(&document, limits).unwrap_err().code,
               FoundationErrorCode::BudgetExceeded);
    assert_eq!(canonical_bytes_v1(document.root(), CanonicalProfile::CorpusSnapshotV1, limits).unwrap_err().code,
               FoundationErrorCode::BudgetExceeded);
    assert_eq!(JsonLimits::new(100, usize::MAX, 100, 100).unwrap_err().code,
               FoundationErrorCode::BudgetExceeded);
}
