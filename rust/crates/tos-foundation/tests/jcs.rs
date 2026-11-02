use tos_foundation::{
    Digest256, FoundationErrorCode as Code, JsonLimits, JsonNumber, JsonNumberKind, JsonString,
    JsonValue, jcs_bytes_v1, jcs_raw_bytes_v1,
};

fn encode(s: &str) -> Vec<u8> {
    jcs_raw_bytes_v1(s.as_bytes(), JsonLimits::default()).unwrap()
}

#[test]
fn exact_bytes_sorting_and_escaping() {
    let input = "{\"\\r\":\"\\u000f\\n\\\\\\\"\",\"דּ\":3,\"😀\":2,\"€\":1,\"1\":0}";
    let expected = "{\"\\r\":\"\\u000f\\n\\\\\\\"\",\"1\":0,\"€\":1,\"😀\":2,\"דּ\":3}";
    assert_eq!(encode(input), expected.as_bytes());
    assert_eq!(encode("{\"b\":2,\"a\":1}"), b"{\"a\":1,\"b\":2}");
    assert_eq!(
        Digest256::of_bytes(&encode("{\"b\":2,\"a\":1}")).to_hex(),
        "43258cff783fe7036d8a43033f830adfc60ec037382473548ac742b888292777"
    );
    assert_eq!(
        Digest256::of_bytes(&encode("null")).to_hex(),
        "74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b"
    );
}

#[test]
fn rejects_ambiguous_invalid_and_inexact() {
    for raw in [
        "{\"a\":1,\"a\":2}",
        "\"\\ud800\"",
        "9007199254740993",
        "1000000000000000128",
        "0.1000000000000000055511151231257827",
        "1e400",
        "-0",
        "-0.0",
        "-0e99",
        "1e-400",
        "NaN",
    ] {
        assert!(
            jcs_raw_bytes_v1(raw.as_bytes(), JsonLimits::default()).is_err(),
            "{raw}"
        );
    }
    assert_eq!(encode("9007199254740992"), b"9007199254740992");
    assert_eq!(encode("0.5"), b"0.5");
    let d = JsonString::from_utf8("x");
    let duplicate = JsonValue::Object(vec![(d.clone(), JsonValue::Null), (d, JsonValue::Null)]);
    assert_eq!(
        jcs_bytes_v1(&duplicate, JsonLimits::default())
            .unwrap_err()
            .code,
        Code::DuplicateMember
    );
    let lone = JsonValue::String(JsonString::from_utf16(&[0xd800]));
    assert!(jcs_bytes_v1(&lone, JsonLimits::default()).is_err());
    let invalid = JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Float,
        lexeme: "Infinity".into(),
    });
    assert!(jcs_bytes_v1(&invalid, JsonLimits::default()).is_err());
}

#[test]
fn decimal_admission_and_canonical_spelling() {
    for (input, expected) in [
        ("0.1", "0.1"),
        ("753.12", "753.12"),
        ("1.0", "1"),
        ("1e3", "1000"),
        ("1E-7", "1e-7"),
        ("0.30000000000000004", "0.30000000000000004"),
        ("123456789012345680000", "123456789012345680000"),
        ("1000000000000000000000", "1e+21"),
        ("0e999999999999999999999999", "0"),
        ("1.000000e+0003", "1000"),
    ] {
        assert_eq!(encode(input), expected.as_bytes(), "{input}");
    }
}

#[test]
fn rfc_8785_appendix_b_number_serialization() {
    // Exercise both the binary64 serializer and admission of its decimal output.
    let vectors = [
        (0x0000000000000000, "0"),
        (0x8000000000000000, "0"),
        (0x0000000000000001, "5e-324"),
        (0x8000000000000001, "-5e-324"),
        (0x7fefffffffffffff, "1.7976931348623157e+308"),
        (0xffefffffffffffff, "-1.7976931348623157e+308"),
        (0x4340000000000000, "9007199254740992"),
        (0xc340000000000000, "-9007199254740992"),
        (0x4430000000000000, "295147905179352830000"),
        (0x44b52d02c7e14af5, "9.999999999999997e+22"),
        (0x44b52d02c7e14af6, "1e+23"),
        (0x44b52d02c7e14af7, "1.0000000000000001e+23"),
        (0x444b1ae4d6e2ef4e, "999999999999999700000"),
        (0x444b1ae4d6e2ef4f, "999999999999999900000"),
        (0x444b1ae4d6e2ef50, "1e+21"),
        (0x3eb0c6f7a0b5ed8d, "0.000001"),
        (0x3eb0c6f7a0b5ed8c, "9.999999999999997e-7"),
        (0x41b3de4355555553, "333333333.3333332"),
        (0x41b3de4355555554, "333333333.33333325"),
        (0x41b3de4355555555, "333333333.3333333"),
        (0x41b3de4355555556, "333333333.3333334"),
        (0x41b3de4355555557, "333333333.33333343"),
        (0xbecbf647612f3696, "-0.0000033333333333333333"),
        (0x43143ff3c1cb0959, "1424953923781206.2"),
    ];
    for (bits, expected) in vectors {
        assert_eq!(
            ryu_js::Buffer::new().format_finite(f64::from_bits(bits)),
            expected
        );
        assert_eq!(encode(expected), expected.as_bytes(), "{bits:x}");
    }
}
