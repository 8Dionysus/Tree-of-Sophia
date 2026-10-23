//! Retained matrix: D01 arithmetic/direction/range; D02 compatibility;
//! D03 coarse/nonmetric/calendar-policy; wire preservation/numeric admission.
use tos_foundation::temporal::difference::{
    DifferenceKind, DifferenceOutcome, NegativeDurationPolicy, difference, execute_difference,
};
use tos_foundation::temporal::{
    Duration, OrderingCapability, Position, Resolution, TemporalBudget, TrsRef,
};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};

fn coordinate(value: i64) -> Position {
    Position::Coordinate {
        value,
        unit: "seconds".into(),
        origin: "O".into(),
        trs: TrsRef {
            id: "X".into(),
            version: "rules1".into(),
        },
    }
}
fn elapsed(policy: Option<NegativeDurationPolicy>) -> DifferenceKind {
    DifferenceKind::Elapsed {
        negative_policy: policy,
    }
}
fn parse(s: &str) -> JsonValue {
    parse_json(
        s.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .root()
    .clone()
}
fn field<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}

#[test]
fn d01_exact_signed_arithmetic_zero_and_full_endpoint_range() {
    use NegativeDurationPolicy::*;
    for (begin, end, policy, expected) in [
        (4, 10, None, "6"),
        (4, 4, None, "0"),
        (10, 4, Some(Signed), "-6"),
        (i64::MIN, i64::MAX, None, "18446744073709551615"),
        (i64::MAX, i64::MIN, Some(Signed), "-18446744073709551615"),
    ] {
        let (a, b) = (coordinate(end), coordinate(begin));
        let original = (a.clone(), b.clone());
        assert_eq!(
            difference(&a, &b, &elapsed(policy)),
            DifferenceOutcome::Resolved(Duration::Elapsed {
                magnitude: expected.into(),
                unit: "seconds".into(),
                scale: TrsRef {
                    id: "X".into(),
                    version: "rules1".into()
                }
            })
        );
        assert_eq!((a, b), original);
    }
    assert_eq!(
        difference(&coordinate(4), &coordinate(10), &elapsed(None)),
        DifferenceOutcome::PolicyRequired("NegativeDurationPolicyRequired")
    );
    assert_eq!(
        difference(&coordinate(4), &coordinate(10), &elapsed(Some(Reject))),
        DifferenceOutcome::Reversed
    );
}

#[test]
fn d02_d03_incompatible_or_coarse_values_never_become_elapsed_numbers() {
    let base = coordinate(4);
    for dimension in ["id", "version", "unit", "origin"] {
        let mut end = coordinate(10);
        if let Position::Coordinate {
            trs, unit, origin, ..
        } = &mut end
        {
            match dimension {
                "id" => trs.id = "Y".into(),
                "version" => trs.version = "rules2".into(),
                "unit" => *unit = "minutes".into(),
                _ => *origin = "other-origin".into(),
            }
        }
        assert!(
            matches!(
                difference(&end, &base, &elapsed(None)),
                DifferenceOutcome::Incomparable(_)
            ),
            "{dimension}"
        );
    }
    let t = TrsRef {
        id: "calendar-X".into(),
        version: "rules1".into(),
    };
    for (lexical, resolution) in [("1344", Resolution::Year), ("1344-02", Resolution::Month)] {
        let date = Position::Calendar {
            lexical: lexical.into(),
            resolution,
            trs: Some(t.clone()),
            era: None,
        };
        assert_eq!(
            difference(&date, &date, &elapsed(None)),
            DifferenceOutcome::Unresolved("ElapsedScaleRequired")
        );
        assert_eq!(
            difference(
                &date,
                &date,
                &DifferenceKind::Calendar {
                    unit: "month".into(),
                    arithmetic_policy_ref: None
                }
            ),
            DifferenceOutcome::PolicyRequired("CalendarPolicyRequired")
        );
        assert_eq!(
            difference(
                &date,
                &date,
                &DifferenceKind::Calendar {
                    unit: "month".into(),
                    arithmetic_policy_ref: Some("calendar-policy-v1".into())
                }
            ),
            DifferenceOutcome::Unsupported("CalendarDifferenceProfileNotImplemented")
        );
    }
    let ordinal = Position::Ordinal {
        node: "rank".into(),
        rank: Some(4),
        trs: t,
        ordering: OrderingCapability::Total,
    };
    assert_eq!(
        difference(&ordinal, &ordinal, &elapsed(None)),
        DifferenceOutcome::Unsupported("NonMetricScale")
    );
    let mut missing = coordinate(10);
    if let Position::Coordinate { trs, .. } = &mut missing {
        trs.version.clear();
    }
    assert_eq!(
        difference(&missing, &base, &elapsed(None)),
        DifferenceOutcome::Unresolved("UnknownTRS")
    );
}

#[test]
fn wire_n3_inputs_remain_immutable_and_admission_is_explicit() {
    let raw = r#"{"beginning":{"type":"CoordinatePosition","value":4,"axis":{"trs":{"id":"X","version":"rules1"},"unit":"seconds","origin":"O"}},"end":{"type":"CoordinatePosition","value":10,"axis":{"trs":{"id":"X","version":"rules1"},"unit":"seconds","origin":"O"}},"duration_kind":"Elapsed"}"#;
    let input = parse(raw);
    let original = input.clone();
    let result = execute_difference(&input, &parse("{}"), TemporalBudget::default()).unwrap();
    assert_eq!(field(&result, "status").as_str(), Some("Resolved"));
    assert_eq!(
        field(field(&result, "duration"), "magnitude").as_str(),
        Some("6")
    );
    assert_eq!(
        field(field(&result, "duration"), "unit").as_str(),
        Some("seconds")
    );
    assert_eq!(field(&result, "preserved_input"), &original);
    assert_eq!(input, original);
    for (value, reason) in [
        ("9223372036854775808", "CoordinateOutOfRange"),
        ("4.0", "ExactIntegerCoordinateRequired"),
    ] {
        let input = parse(&raw.replace("\"value\":4", &format!("\"value\":{value}")));
        let result = execute_difference(&input, &parse("{}"), TemporalBudget::default()).unwrap();
        assert_eq!(field(&result, "status").as_str(), Some("Unsupported"));
        assert_eq!(field(&result, "reason").as_str(), Some(reason));
        assert!(result.object_get("duration").is_none());
    }
    let coarse = parse(
        r#"{"beginning":{"type":"CalendarPosition","lexical":"1344","resolution":"year","trs_ref":{"id":"calendar-X","version":"v1"}},"end":{"type":"CalendarPosition","lexical":"1345-02","resolution":"month","trs_ref":{"id":"calendar-X","version":"v1"}},"duration_kind":"Elapsed"}"#,
    );
    let result = execute_difference(&coarse, &parse("{}"), TemporalBudget::default()).unwrap();
    assert_eq!(
        field(&result, "reason").as_str(),
        Some("ElapsedScaleRequired")
    );
    assert_eq!(field(&result, "preserved_input"), &coarse);
    assert!(result.object_get("duration").is_none());
    for modifier in [
        r#""qualifier":"Approximate""#,
        r#""resolution":"year""#,
        r#""uncertainty":{"width":null}"#,
    ] {
        let nonexact = parse(&raw.replace("\"value\":4", &format!("\"value\":4,{modifier}")));
        let result =
            execute_difference(&nonexact, &parse("{}"), TemporalBudget::default()).unwrap();
        assert_eq!(field(&result, "reason").as_str(), Some("NonExactPosition"));
    }
    assert!(
        execute_difference(
            &input,
            &parse("{}"),
            TemporalBudget {
                max_pairings: 0,
                ..TemporalBudget::default()
            }
        )
        .is_err()
    );
    let conflicting = parse(&raw.replace("\"value\":4", "\"value\":4,\"unit\":\"minutes\""));
    let result = execute_difference(
        &conflicting,
        &parse(r#"{"comparison_axis":"X"}"#),
        TemporalBudget::default(),
    )
    .unwrap();
    assert_eq!(
        field(&result, "reason").as_str(),
        Some("ConflictingAxisDeclaration")
    );

    // N5 Safe wire rows share this existing admission/preservation family.
    // Native i64/i128 tests above remain a distinct contract, not JS evidence.
    use tos_foundation::temporal::integer_wire::*;
    let budget = TemporalBudget::default();
    for (json, expected) in [
        ("0", IntegerWireSafety::Safe),
        ("9007199254740991", IntegerWireSafety::Safe),
        ("-9007199254740991", IntegerWireSafety::Safe),
        (
            "9007199254740992",
            IntegerWireSafety::Unsupported("ExactIntegerOutOfWireRange"),
        ),
        (
            "9007199254740993",
            IntegerWireSafety::Unsupported("ExactIntegerOutOfWireRange"),
        ),
        (
            "-9007199254740992",
            IntegerWireSafety::Unsupported("ExactIntegerOutOfWireRange"),
        ),
        (
            "-9007199254740993",
            IntegerWireSafety::Unsupported("ExactIntegerOutOfWireRange"),
        ),
        (
            "1.0",
            IntegerWireSafety::Unsupported("JsonFloatNotSupported"),
        ),
        (
            "1e0",
            IntegerWireSafety::Unsupported("JsonFloatNotSupported"),
        ),
        (
            "-0",
            IntegerWireSafety::Unsupported("NonCanonicalJsonInteger"),
        ),
        (r#""9007199254740993""#, IntegerWireSafety::Safe),
    ] {
        let v = parse(json);
        let original = v.clone();
        assert_eq!(
            check_integer_wire_value(&v, budget).unwrap(),
            expected,
            "{json}"
        );
        let encoded = encode_integer_wire_response("difference", &v, budget).unwrap();
        assert_eq!(
            check_integer_wire_value(&encoded, budget).unwrap(),
            IntegerWireSafety::Safe
        );
        match expected {
            IntegerWireSafety::Safe => {
                assert_eq!(field(&encoded, "outcome").as_str(), Some("Encoded"));
                assert_eq!(field(&encoded, "payload"), &original);
            }
            IntegerWireSafety::Unsupported(reason) => {
                assert_eq!(field(&encoded, "outcome").as_str(), Some("Unsupported"));
                assert_eq!(field(&encoded, "reason").as_str(), Some(reason));
                assert_eq!(
                    field(&encoded, "representation").as_str(),
                    Some("OpaqueRawJson")
                );
                let raw = field(&encoded, "raw_json").as_str().unwrap();
                assert_eq!(parse(raw), original);
                assert_eq!(
                    field(&encoded, "emitted_json_sha256").as_str(),
                    Some(
                        tos_foundation::Digest256::of_bytes(raw.as_bytes())
                            .to_prefixed()
                            .as_str()
                    )
                );
                for absent in ["payload", "next", "cursor", "raw_source_bytes_digest"] {
                    assert!(encoded.object_get(absent).is_none());
                }
            }
        }
        assert_eq!(v, original);
    }
    // Whole-value output coverage: unsafe numbers may hide in refusal echoes,
    // source rows, refs or arbitrary raw metadata; none may leak to JS as number.
    for v in [
        parse(r#"{"status":"Unsupported","preserved_input":{"value":9007199254740993}}"#),
        parse(r#"{"outcome":"Refused","source_temporal_packet":{"opaque":9007199254740993}}"#),
        parse(
            r#"{"rows":[{"claim_ref":{"version":9007199254740993},"carrier":{"raw":9007199254740992}}],"next":{"source_order":9007199254740993}}"#,
        ),
        parse(r#"{"raw_decimal":0.100000000000000005,"raw_negative_zero":-0}"#),
    ] {
        let encoded = encode_integer_wire_response("selected-query", &v, budget).unwrap();
        assert_eq!(field(&encoded, "outcome").as_str(), Some("Unsupported"));
        assert_eq!(parse(field(&encoded, "raw_json").as_str().unwrap()), v);
        assert_eq!(
            check_integer_wire_value(&encoded, budget).unwrap(),
            IntegerWireSafety::Safe
        );
        assert!(encoded.object_get("payload").is_none());
        assert!(encoded.object_get("next").is_none());
    }
    for (magnitude, safe) in [
        ("0", true),
        ("-1", true),
        ("18014398509481982", true),
        ("170141183460469231731687303715884105727", true),
        ("-170141183460469231731687303715884105728", true),
        ("170141183460469231731687303715884105728", false),
        ("01", false),
        ("-0", false),
        ("+1", false),
        ("1.0", false),
        ("1e3", false),
        ("9999999999999999999999999999999999999999", false),
    ] {
        let v = JsonValue::String(tos_foundation::JsonString::from_utf8(magnitude));
        assert_eq!(
            validate_elapsed_magnitude_string(&v) == IntegerWireSafety::Safe,
            safe,
            "{magnitude}"
        );
    }
    assert_eq!(
        validate_elapsed_magnitude_string(&parse("6")),
        IntegerWireSafety::Unsupported("ElapsedMagnitudeStringRequired")
    );
    let safe_endpoints = parse(
        &raw.replace(r#""value":4"#, r#""value":-9007199254740991"#)
            .replace(r#""value":10"#, r#""value":9007199254740991"#),
    );
    let native = execute_difference(&safe_endpoints, &parse("{}"), budget).unwrap();
    let encoded = encode_integer_wire_response("difference", &native, budget).unwrap();
    assert_eq!(field(&encoded, "outcome").as_str(), Some("Encoded"));
    assert_eq!(
        field(field(field(&encoded, "payload"), "duration"), "magnitude").as_str(),
        Some("18014398509481982")
    );
    assert_eq!(
        field(&encoded, "elapsed_magnitude_profile").as_str(),
        Some("CanonicalDecimalI128StringV1")
    );
    // Known elapsed String validation does not silently normalize malformed data.
    let bad_elapsed = parse(r#"{"duration":{"kind":"Elapsed","magnitude":"01"}}"#);
    assert_eq!(
        field(
            &encode_integer_wire_response("difference", &bad_elapsed, budget).unwrap(),
            "reason"
        )
        .as_str(),
        Some("NonCanonicalElapsedMagnitude")
    );
    let request = parse(
        r#"{"schema_version":"tos_exact_integer_wire_v1","integer_encoding":"SafeJsonNumberV1","operation":"difference","payload":{"input":{},"context":{}}}"#,
    );
    let borrowed = parse_integer_wire_request(&request, budget).unwrap();
    assert_eq!(borrowed.integer_encoding, SAFE_INTEGER_ENCODING);
    assert_eq!(borrowed.operation, "difference");
    assert!(std::ptr::eq(borrowed.payload, field(&request, "payload")));
    // An explicit Safe-only helper refusal does not exercise Decimal dispatch;
    // the scoped Decimal difference owner path is exercised below.
    let unsupported = parse(
        r#"{"schema_version":"tos_exact_integer_wire_v1","integer_encoding":"CanonicalDecimalStringV1","operation":"difference","payload":{"input":{"value":"9007199254740993"},"context":{}}}"#,
    );
    let borrowed = parse_integer_wire_request(&unsupported, budget).unwrap();
    assert_ne!(borrowed.integer_encoding, SAFE_INTEGER_ENCODING);
    let refused = refuse_integer_wire_value(
        borrowed.operation,
        "UnsupportedIntegerEncoding",
        &unsupported,
        budget,
    )
    .unwrap();
    assert_eq!(field(&refused, "outcome").as_str(), Some("Unsupported"));
    assert_eq!(
        parse(field(&refused, "raw_json").as_str().unwrap()),
        unsupported
    );
    for invalid in [
        r#"{"schema_version":"tos_exact_integer_wire_v1","operation":"difference","payload":{}}"#,
        r#"{"schema_version":"tos_exact_integer_wire_v1","integer_encoding":"SafeJsonNumberV1","operation":"difference","payload":{},"extra":true}"#,
    ] {
        assert!(matches!(
            parse_integer_wire_request(&parse(invalid), budget),
            Err(tos_foundation::temporal::TemporalError::MalformedRequest(_))
        ));
    }
    let oversized_fallback = parse(&format!(
        r#"{{"unsafe":9007199254740993,"quoted":"{}"}}"#,
        r#"\""#.repeat(100)
    ));
    assert!(matches!(
        encode_integer_wire_response(
            "difference",
            &oversized_fallback,
            tos_foundation::temporal::TemporalBudget {
                max_expression_bytes: 512,
                ..budget
            }
        ),
        Err(tos_foundation::temporal::TemporalError::ResourceExhausted(
            _
        ))
    ));

    // CanonicalDecimalStringV1 is opt-in only for the frozen difference slice.
    // Exercise the real exported wrapper after the I/C routing/header seam.
    fn replace_at(value: &mut JsonValue, path: &[&str], replacement: JsonValue) {
        if path.is_empty() {
            *value = replacement;
            return;
        }
        let JsonValue::Object(members) = value else {
            panic!("object fixture");
        };
        if path.len() == 1 {
            if let Some((_, old)) = members
                .iter_mut()
                .find(|(k, _)| k.as_str() == Some(path[0]))
            {
                *old = replacement;
            } else {
                members.push((tos_foundation::JsonString::from_utf8(path[0]), replacement));
            }
        } else {
            let child = &mut members
                .iter_mut()
                .find(|(k, _)| k.as_str() == Some(path[0]))
                .unwrap()
                .1;
            replace_at(child, &path[1..], replacement);
        }
    }
    fn text_value(s: &str) -> JsonValue {
        JsonValue::String(tos_foundation::JsonString::from_utf8(s))
    }
    fn decimal_request(beginning: &str, end: &str, policy: Option<&str>) -> JsonValue {
        let mut q = parse(
            r#"{"schema_version":"tos_exact_integer_wire_v1","integer_encoding":"CanonicalDecimalStringV1","operation":"difference","payload":{"input":{"beginning":{"type":"CoordinatePosition","value":"4","axis":{"trs":{"id":"X","version":"rules1"},"unit":"seconds","origin":"O"}},"end":{"type":"CoordinatePosition","value":"10","axis":{"trs":{"id":"X","version":"rules1"},"unit":"seconds","origin":"O"}},"duration_kind":"Elapsed"},"context":{}}}"#,
        );
        replace_at(
            &mut q,
            &["payload", "input", "beginning", "value"],
            text_value(beginning),
        );
        replace_at(
            &mut q,
            &["payload", "input", "end", "value"],
            text_value(end),
        );
        if let Some(policy) = policy {
            replace_at(
                &mut q,
                &["payload", "input", "negative_policy"],
                text_value(policy),
            );
        }
        q
    }
    let run_decimal =
        |q: &JsonValue| tos_foundation::temporal::execute_temporal_integer_wire(q, budget).unwrap();
    for (begin, end, policy, status, magnitude, reason) in [
        (
            "9007199254740992",
            "9007199254740993",
            None,
            "Resolved",
            Some("1"),
            None,
        ),
        (
            "-9007199254740993",
            "-9007199254740992",
            None,
            "Resolved",
            Some("1"),
            None,
        ),
        (
            "-9223372036854775808",
            "9223372036854775807",
            None,
            "Resolved",
            Some("18446744073709551615"),
            None,
        ),
        (
            "9223372036854775807",
            "-9223372036854775808",
            Some("Signed"),
            "Resolved",
            Some("-18446744073709551615"),
            None,
        ),
        ("0", "0", None, "Resolved", Some("0"), None),
        (
            "10",
            "4",
            None,
            "PolicyRequired",
            None,
            Some("NegativeDurationPolicyRequired"),
        ),
        ("10", "4", Some("Signed"), "Resolved", Some("-6"), None),
        (
            "10",
            "4",
            Some("Reject"),
            "Reversed",
            None,
            Some("EndBeforeBeginning"),
        ),
    ] {
        let q = decimal_request(begin, end, policy);
        let original = q.clone();
        let result = run_decimal(&q);
        assert_eq!(
            field(&result, "integer_encoding").as_str(),
            Some("CanonicalDecimalStringV1")
        );
        assert_eq!(
            field(&result, "supported_scope").as_str(),
            Some("DifferenceCoordinateI64V1")
        );
        assert_eq!(field(&result, "outcome").as_str(), Some("Encoded"));
        let payload = field(&result, "payload");
        assert_eq!(field(payload, "status").as_str(), Some(status));
        assert_eq!(
            field(field(payload, "duration"), "magnitude").as_str(),
            magnitude
        );
        if let Some(reason) = reason {
            assert_eq!(field(payload, "reason").as_str(), Some(reason));
        }
        assert_eq!(
            field(payload, "preserved_input"),
            field(field(&q, "payload"), "input")
        );
        assert_eq!(
            check_integer_wire_value(&result, budget).unwrap(),
            IntegerWireSafety::Safe
        );
        assert_eq!(q, original);
    }
    let base = decimal_request("9007199254740992", "9007199254740993", None);
    for (path, replacement, reason) in [
        (
            &["payload", "input", "end", "axis", "trs", "id"][..],
            "Y",
            "NoCrosswalk",
        ),
        (
            &["payload", "input", "end", "axis", "trs", "version"][..],
            "rules2",
            "NoCrosswalk",
        ),
        (
            &["payload", "input", "end", "axis", "unit"][..],
            "minutes",
            "IncompatibleUnits",
        ),
        (
            &["payload", "input", "end", "axis", "origin"][..],
            "other",
            "IncompatibleOrigins",
        ),
    ] {
        let mut q = base.clone();
        replace_at(&mut q, path, text_value(replacement));
        let result = run_decimal(&q);
        let payload = field(&result, "payload");
        assert_eq!(field(payload, "status").as_str(), Some("Incomparable"));
        assert_eq!(field(payload, "reason").as_str(), Some(reason));
        assert_eq!(
            field(payload, "preserved_input"),
            field(field(&q, "payload"), "input")
        );
        assert_eq!(
            check_integer_wire_value(&result, budget).unwrap(),
            IntegerWireSafety::Safe
        );
    }
    let mut rejected: Vec<(JsonValue, &str)> = Vec::new();
    for (bad, reason) in [
        ("+1", "NonCanonicalCoordinateString"),
        ("-0", "NonCanonicalCoordinateString"),
        ("01", "NonCanonicalCoordinateString"),
        (" 1", "NonCanonicalCoordinateString"),
        ("1 ", "NonCanonicalCoordinateString"),
        ("1.0", "NonCanonicalCoordinateString"),
        ("1e3", "NonCanonicalCoordinateString"),
        ("١", "NonCanonicalCoordinateString"),
        ("", "NonCanonicalCoordinateString"),
        ("-", "NonCanonicalCoordinateString"),
        ("9223372036854775808", "CoordinateOutOfRange"),
        ("-9223372036854775809", "CoordinateOutOfRange"),
        (
            "999999999999999999999999999999999999999",
            "CoordinateOutOfRange",
        ),
        (
            "9999999999999999999999999999999999999999",
            "DecimalCoordinateDigitLimit",
        ),
    ] {
        rejected.push((decimal_request(bad, "10", None), reason));
    }
    for (json, reason) in [
        ("4", "DecimalCoordinateStringRequired"),
        ("true", "DecimalCoordinateStringRequired"),
        ("null", "DecimalCoordinateStringRequired"),
        ("4.0", "JsonFloatNotSupported"),
        ("9007199254740993", "ExactIntegerOutOfWireRange"),
    ] {
        let mut q = base.clone();
        replace_at(
            &mut q,
            &["payload", "input", "beginning", "value"],
            parse(json),
        );
        rejected.push((q, reason));
    }
    for (path, value, reason) in [
        (
            &["payload", "input", "beginning", "type"][..],
            text_value("CalendarPosition"),
            "DecimalDifferenceProfileNotSupported",
        ),
        (
            &["payload", "input", "beginning", "resolution"][..],
            text_value("year"),
            "DecimalDifferenceProfileNotSupported",
        ),
        (
            &["payload", "input", "beginning", "uncertainty"][..],
            JsonValue::Null,
            "DecimalDifferenceProfileNotSupported",
        ),
        (
            &["payload", "input", "duration_kind"][..],
            text_value("Calendar"),
            "DecimalDifferenceProfileNotSupported",
        ),
        (
            &["payload", "context"][..],
            parse(r#"{"system_as_of":"Current"}"#),
            "DecimalDifferenceContextNotSupported",
        ),
        (
            &["payload", "context"][..],
            parse(r#"{"record_version":"9007199254740993"}"#),
            "DecimalDifferenceContextNotSupported",
        ),
        (
            &["payload", "input", "opaque"][..],
            parse(r#"{"value":"9007199254740993"}"#),
            "DecimalDifferenceProfileNotSupported",
        ),
    ] {
        let mut q = base.clone();
        replace_at(&mut q, path, value);
        rejected.push((q, reason));
    }
    for (q, reason) in rejected {
        let original = q.clone();
        let result = run_decimal(&q);
        assert_eq!(field(&result, "outcome").as_str(), Some("Unsupported"));
        assert_eq!(
            field(&result, "integer_encoding").as_str(),
            Some("CanonicalDecimalStringV1")
        );
        assert_eq!(
            field(&result, "supported_scope").as_str(),
            Some("DifferenceCoordinateI64V1")
        );
        assert_eq!(field(&result, "reason").as_str(), Some(reason));
        assert_eq!(
            field(&result, "representation").as_str(),
            Some("OpaqueRawJson")
        );
        let raw = field(&result, "raw_json").as_str().unwrap();
        assert_eq!(
            parse(raw),
            original,
            "refusal must preserve the WIRE request, never an internal numeric echo"
        );
        assert_eq!(
            field(&result, "emitted_json_sha256").as_str(),
            Some(
                tos_foundation::Digest256::of_bytes(raw.as_bytes())
                    .to_prefixed()
                    .as_str()
            )
        );
        assert_eq!(
            check_integer_wire_value(&result, budget).unwrap(),
            IntegerWireSafety::Safe
        );
        for absent in ["payload", "next", "cursor", "raw_source_bytes_digest"] {
            assert!(result.object_get(absent).is_none());
        }
        assert_eq!(q, original);
    }
    let mut malformed = base.clone();
    replace_at(&mut malformed, &["payload", "context"], JsonValue::Null);
    assert!(matches!(
        tos_foundation::temporal::execute_temporal_integer_wire(&malformed, budget),
        Err(tos_foundation::temporal::TemporalError::MalformedRequest(_))
    ));
    let mut other = base.clone();
    replace_at(&mut other, &["operation"], text_value("relate_extents"));
    let result = run_decimal(&other);
    assert_eq!(field(&result, "outcome").as_str(), Some("Unsupported"));
    assert!(result.object_get("supported_scope").is_none());
    let mut safe = base.clone();
    replace_at(
        &mut safe,
        &["integer_encoding"],
        text_value("SafeJsonNumberV1"),
    );
    let result = run_decimal(&safe);
    assert_eq!(
        field(field(&result, "payload"), "status").as_str(),
        Some("Unsupported"),
        "Safe/native coordinate ABI must not silently acquire decimal-string decoding"
    );
    assert!(result.object_get("supported_scope").is_none());
    assert!(matches!(
        tos_foundation::temporal::execute_temporal_integer_wire(
            &base,
            TemporalBudget {
                max_pairings: 0,
                ..budget
            }
        ),
        Err(tos_foundation::temporal::TemporalError::ResourceExhausted(
            _
        ))
    ));
}
