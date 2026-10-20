//! Retained owner-local risks after dedup against foundation_wave_temporal.
//! Matrix: normalized lexical semantics -> first test; parser admission/limits ->
//! second; explicit primitive vs event/no fallback/JSON partial order -> third;
//! source confidence vs derived answer -> fourth. Independent R tests retain
//! Allen13/shift, full-i64, axis/version/unit/origin/rename, reversed/unknown
//! extents, raw roundtrip, request-axis override, and pairwise budget negatives.
use tos_foundation::JsonValue;
use tos_foundation::temporal::*;
fn parse(raw: &str) -> JsonValue {
    tos_foundation::parse_json(
        raw.as_bytes(),
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .unwrap()
    .root()
    .clone()
}
fn run(op: &str, input: &str) -> JsonValue {
    execute_temporal_operation(op, &parse(input), &parse("{}"), TemporalBudget::default()).unwrap()
}
fn f<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn s<'a>(v: &'a JsonValue, k: &str) -> Option<&'a str> {
    f(v, k).as_str()
}

#[test]
fn normalized_expression_semantics_preserve_boundaries_resolution_cardinality_and_qualifiers() {
    for (raw, expected) in [
        ("1985-04-12/..", "NoBoundDeclared"),
        ("1985-04-12/", "Unknown"),
    ] {
        let a = run(
            "capture_expression",
            &format!(r#"{{"raw":"{raw}","declared_level":1}}"#),
        );
        assert_eq!(s(f(f(&a, "value"), "end"), "type"), Some(expected));
    }
    let a = run(
        "capture_expression",
        r#"{"raw":"2004-02-01/2005-02","declared_level":0}"#,
    );
    assert_eq!(s(f(f(&a, "value"), "start"), "resolution"), Some("day"));
    assert_eq!(s(f(f(&a, "value"), "end"), "resolution"), Some("month"));
    for (raw, kind) in [
        ("[1667,1668,1670..1672]", "ExactlyOneOf"),
        ("{1667,1668}", "AllOf"),
    ] {
        let a = run(
            "capture_expression",
            &format!(r#"{{"raw":"{raw}","declared_level":2}}"#),
        );
        assert_eq!(s(f(&a, "value"), "type"), Some(kind));
        if kind == "ExactlyOneOf" {
            assert_eq!(
                f(f(&a, "value"), "expanded_years")
                    .as_array()
                    .unwrap()
                    .len(),
                5
            );
        }
    }
    for (raw, expected) in [
        ("1984?", "Uncertain"),
        ("1984~", "Approximate"),
        ("1984%", "UncertainAndApproximate"),
    ] {
        let e = capture_expression(
            raw,
            ExpressionProfile::Edtf {
                level: 1,
                version: "2019-02-04".into(),
            },
            None,
            TemporalBudget::default(),
        );
        assert_eq!(s(&e.syntax, "qualifier"), Some(expected));
        assert!(e.syntax.object_get("probability").is_none());
    }
}
#[test]
fn parser_profile_and_expansion_limits_preserve_evidence() {
    for (raw, level, max_members, diagnostic) in [
        ("[1000..9999]", 2, 2, "ResourceLimit"),
        ("1984?", 0, 2, "UnsupportedProfileLevel"),
    ] {
        let e = capture_expression(
            raw,
            ExpressionProfile::Edtf {
                level,
                version: "2019-02-04".into(),
            },
            None,
            TemporalBudget {
                max_members,
                ..TemporalBudget::default()
            },
        );
        assert_eq!(e.state, ParseState::Unparsed);
        assert_eq!(e.diagnostics, vec![diagnostic]);
        assert_eq!(serialize_expression(&e), raw);
    }
}
#[test]
fn explicit_primitives_keep_date_buckets_distinct_from_event_extents_and_unknown_context() {
    let r = execute_temporal_operation(
        "derive_extent",
        &parse(r#"{"expression":"2000-02","resolution":"month","calendar":"GregorianProleptic"}"#),
        &parse(r#"{"trs_ref":"arbitrary-calendar-id@v7"}"#),
        TemporalBudget::default(),
    )
    .unwrap();
    assert_eq!(s(f(&r, "value"), "start"), Some("2000-02-01"));
    assert_eq!(s(f(&r, "value"), "end_exclusive"), Some("2000-03-01"));
    assert_eq!(f(&r, "event_extent_inferred"), &JsonValue::Bool(false));
    let r = run(
        "relate_extents",
        r#"{"calendar":"GregorianProleptic","left":{"type":"Instant","trs_ref":"declared-clock@v3","zone":"UTC","local":"1945-04-02T06:00:00"},"right":{"type":"Instant","trs_ref":"declared-clock@v3","zone":"UTC","local":"1945-04-02T13:00:00"}}"#,
    );
    assert_eq!(s(&r, "status"), Some("Entailed"));
    assert_eq!(s(&r, "relation"), Some("before"));
    for (op, input, reason) in [
        (
            "resolve_position",
            r#"{"expression":"1945-04-02","calendar_ref":null,"era_ref":null,"zone_ref":null}"#,
            "UnknownTRS",
        ),
        (
            "derive_extent",
            r#"{"expression":"1945","resolution":"year"}"#,
            "UnknownTRS",
        ),
        (
            "compare_positions",
            r#"{"left":{"type":"OrdinalPosition","value":1,"trs_ref":"ordinal","ordering":"Partial"},"right":{"type":"OrdinalPosition","value":2,"trs_ref":"ordinal","ordering":"Partial"}}"#,
            "PartialOrderRequiresConstraints",
        ),
    ] {
        let r = run(op, input);
        assert_eq!(s(&r, "reason"), Some(reason));
    }
}
#[test]
fn source_affirmation_never_becomes_derived_same_instant_or_order() {
    let r = run(
        "relate_claims",
        r#"{"claim_a":{"kind":"occurrence-position","position":"1945-04-02","resolution":"day","entity_extent":"Instant"},"claim_b":{"kind":"occurrence-position","position":"1945-04-02","resolution":"day","entity_extent":"Instant"},"source_assertion":{"relation":"before","epistemic_status":"Affirmed"},"trs_ref":"explicit-trs@v1"}"#,
    );
    assert_eq!(s(&r, "status"), Some("Possible"));
    assert_eq!(f(&r, "relations"), &parse(r#"["before","equals","after"]"#));
    assert_eq!(
        s(f(&r, "source_assertion"), "epistemic_status"),
        Some("Affirmed")
    );
}

#[test]
fn supplemental_claim_carrier_codec_preserves_source_and_refuses_unsupported_forms() {
    fn packet(payload: &str) -> JsonValue {
        parse(&format!(
            r#"{{"statement":"retained wording","statement_language":null,"statement_script":null,"unrelated":{{"id":"Keep-λ","n":90071992547409931234567890}},"temporal_claim_carrier":{{"schema_version":"tos_temporal_claim_carrier_v1",{payload},"future":{{"unknown":null,"raw_decimal":"0.100000000000000005","opaque_float":0.100000000000000005,"negative_zero":-0,"integer":90071992547409931234567890}}}}}}"#
        ))
    }
    fn set(v: &mut JsonValue, key: &str, new: JsonValue) {
        let members = match v {
            JsonValue::Object(m) => m,
            _ => panic!("object fixture"),
        };
        if let Some((_, old)) = members.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
            *old = new;
        } else {
            members.push((tos_foundation::JsonString::from_utf8(key), new));
        }
    }
    fn remove(v: &mut JsonValue, key: &str) {
        if let JsonValue::Object(m) = v {
            m.retain(|(k, _)| k.as_str() != Some(key));
        }
    }
    fn mutate_packet(q: &mut JsonValue, change: impl FnOnce(&mut JsonValue)) {
        let JsonValue::Object(m) = q else {
            panic!("qualifier fixture")
        };
        change(
            &mut m
                .iter_mut()
                .find(|(k, _)| k.as_str() == Some("temporal_claim_carrier"))
                .unwrap()
                .1,
        );
    }
    let reference = r#"{"id":"tos.claim.source-λ","version":17,"digest":"1111111111111111111111111111111111111111111111111111111111111111"}"#;
    let basis = format!(
        r#"{{"source":{reference},"source_locator":"folio 2r","raw_evidence":{{"text":"exact 0.100000000000000005 / -0","bytes":{{"encoding":"base64","data":"LTA="}},"unknown":null}},"future_basis":"keep"}}"#
    );
    let coordinate = r#"{"type":"CoordinatePosition","value":10,"unit":"tick","origin":"O","trs":{"id":"opaque-r","version":"rules1"}}"#;
    let calendar = r#"{"type":"CalendarPosition","lexical":"1900","resolution":"year","trs":null}"#;
    let ordinal = r#"{"type":"OrdinalPosition","node":"phase-λ","rank":null,"trs":{"id":"phase-r","version":"v7"},"ordering":"Partial"}"#;
    let relative = format!(
        r#""relative_relations":[{{"subject_a":"tos.event.A","subject_b":"tos.event.B","relation":"before","source":{reference},"epistemic_status":"Affirmed","future_relation":null}}]"#
    );
    let unknown = format!(r#""validity":{{"type":"Unknown","basis":{basis}}}"#);
    let censored = |position: &str| {
        format!(
            r#""validity":{{"type":"RightCensored","observed_through":{position},"cutoff_basis":{basis}}}"#
        )
    };
    let lexical_beginning =
        r#"{"type":"CalendarPosition","lexical":"2020-01-01","resolution":"Unknown","trs":null}"#;
    let interval = |beginning: &str| {
        format!(
            r#""validity":{{"type":"Interval","beginning":{beginning},"end":{{"type":"Unknown","future":null}},"basis":{basis},"source_input":{{"type":"Interval","valid_from":"2020-01-01","valid_to":{{"type":"Unknown"}},"admitted":true,"resolved":true}}}}"#
        )
    };
    let role_tags = [
        ("WorldOccurrence", TemporalRole::WorldOccurrence),
        ("ActualInterval", TemporalRole::ActualInterval),
        ("Beginning", TemporalRole::Beginning),
        ("End", TemporalRole::End),
        ("SourceCreation", TemporalRole::SourceCreation),
        ("SourcePublication", TemporalRole::SourcePublication),
        ("AttributedKnowledge", TemporalRole::AttributedKnowledge),
        ("SystemTransaction", TemporalRole::SystemTransaction),
    ];
    let roles = format!(r#""roles":[{}]"#, role_tags.iter().map(|(tag,_)| format!(r#"{{"role":"{tag}","assertion":{{"raw":"unresolved source wording","unknown":null}},"basis":{basis}}}"#)).collect::<Vec<_>>().join(","));
    let recurrence = format!(
        r#""recurrence":{{"profile_ref":"opaque-pattern@v7","pattern":{{"raw":"R/../source-pattern","decimal":"0.100000000000000005","future":null}},"basis":{basis}}}"#
    );
    let combined = format!("{relative},{unknown},{roles},{recurrence}");
    let budget = TemporalBudget::default();
    for (name, payload) in [
        ("relative", relative.clone()),
        ("unknown", unknown.clone()),
        ("interval-lexical", interval(lexical_beginning)),
        ("interval-coordinate", interval(coordinate)),
        ("coordinate", censored(coordinate)),
        ("calendar", censored(calendar)),
        ("ordinal", censored(ordinal)),
        ("roles", roles.clone()),
        ("recurrence", recurrence.clone()),
        ("combined", combined),
    ] {
        let q = packet(&payload);
        let original = q.clone();
        let carrier = decode_claim_carrier(&q, budget).unwrap().unwrap();
        assert_eq!(
            carrier.authority(),
            CarrierAuthority::SupplementalSourceAssertions
        );
        assert_eq!(
            carrier.legacy_object_assessment(),
            LegacyObjectAssessment::NotExamined
        );
        assert_eq!(carrier.original_packet(), f(&q, "temporal_claim_carrier"));
        let encoded = encode_claim_carrier(&q, &carrier, budget).unwrap();
        assert_eq!(encoded, original, "{name}");
        assert_eq!(
            decode_claim_carrier(&encoded, budget).unwrap().as_ref(),
            Some(&carrier)
        );
        let destination = parse(r#"{"statement":"other wording","other":null}"#);
        let merged = encode_claim_carrier(&destination, &carrier, budget).unwrap();
        assert_eq!(s(&merged, "statement"), Some("other wording"));
        assert_eq!(
            f(&merged, "temporal_claim_carrier"),
            carrier.original_packet()
        );
        assert!(merged.object_get("other").is_some());
        assert!(
            f(&merged, "temporal_claim_carrier")
                .object_get("resolved")
                .is_none()
        );
        assert!(
            f(&merged, "temporal_claim_carrier")
                .object_get("admitted")
                .is_none()
        );
        // A full Claim's legacy object and IDs remain caller-owned and untouched.
        let mut claim = parse(
            r#"{"claim_id":"tos.claim.retained","object":{"kind":"unknown-date","source_wording":{"text":"legacy stays"}}}"#,
        );
        let legacy = f(&claim, "object").clone();
        set(&mut claim, "qualifiers", merged);
        assert_eq!(f(&claim, "object"), &legacy);
        assert_eq!(s(&claim, "claim_id"), Some("tos.claim.retained"));
        assert_eq!(q, original);
        if let Some(relations) = carrier.relative_relations() {
            assert_eq!(relations[0].relation, Relation::Before);
            assert_eq!(relations[0].epistemic_status, "Affirmed");
            assert_eq!(relations[0].source.version.get(), 17);
        }
        match (name, carrier.validity()) {
            (
                "interval-lexical",
                Some(CarrierValidity::Interval {
                    beginning:
                        Position::Calendar {
                            lexical,
                            resolution,
                            trs,
                            ..
                        },
                    end,
                    basis: b,
                }),
            ) => {
                assert_eq!(lexical, "2020-01-01");
                assert_eq!(*resolution, Resolution::Unknown);
                assert!(trs.is_none());
                assert_eq!(*end, Boundary::Unknown);
                assert_eq!(b.raw_evidence, f(&parse(&basis), "raw_evidence").clone());
            }
            (
                "interval-coordinate",
                Some(CarrierValidity::Interval {
                    beginning: Position::Coordinate { value, .. },
                    end,
                    ..
                }),
            ) => {
                assert_eq!(*value, 10);
                assert_eq!(*end, Boundary::Unknown);
            }
            ("unknown" | "combined", Some(CarrierValidity::Unknown { basis: b })) => {
                assert_eq!(b.source_locator.as_deref(), Some("folio 2r"))
            }
            (
                "coordinate",
                Some(CarrierValidity::RightCensored {
                    beginning,
                    observed_through: Position::Coordinate { value, .. },
                    cutoff_basis,
                }),
            ) => {
                assert!(beginning.is_none());
                assert_eq!(*value, 10);
                assert_eq!(
                    cutoff_basis.raw_evidence,
                    f(&parse(&basis), "raw_evidence").clone()
                );
                assert!(
                    f(carrier.original_packet(), "validity")
                        .object_get("beginning")
                        .is_none()
                );
            }
            (
                "calendar",
                Some(CarrierValidity::RightCensored {
                    observed_through: Position::Calendar { trs, .. },
                    ..
                }),
            ) => assert!(trs.is_none()),
            (
                "ordinal",
                Some(CarrierValidity::RightCensored {
                    observed_through: Position::Ordinal { rank, ordering, .. },
                    ..
                }),
            ) => {
                assert!(rank.is_none());
                assert_eq!(*ordering, OrderingCapability::Partial);
            }
            ("relative" | "roles" | "recurrence", None) => {}
            _ => panic!("typed validity mismatch {name}"),
        }
        if let Some(bindings) = carrier.roles() {
            assert_eq!(bindings.len(), 8);
            for (binding, (_, role)) in bindings.iter().zip(&role_tags) {
                assert_eq!(binding.role, *role);
                assert_eq!(
                    s(&binding.assertion, "raw"),
                    Some("unresolved source wording")
                );
            }
        }
        if let Some(pattern) = carrier.recurrence() {
            assert_eq!(pattern.profile_ref, "opaque-pattern@v7");
            assert_eq!(s(&pattern.pattern, "raw"), Some("R/../source-pattern"));
            assert!(pattern.pattern.object_get("occurrences").is_none());
        }
    }
    use ClaimCarrierError::{Malformed, Unsupported};
    let mut negatives = vec![
        (
            packet(r#""validity":{"type":"RightCensored"}"#),
            Malformed("observed_through"),
        ),
        (
            packet(&format!(
                r#""validity":{{"type":"RightCensored","observed_through":{coordinate}}}"#
            )),
            Malformed("cutoff_basis"),
        ),
        (
            packet(r#""validity":{"type":"Unknown"}"#),
            Malformed("basis"),
        ),
        (
            packet(&format!(
                r#""validity":{{"type":"RightCensored","observed_through":null,"cutoff_basis":{basis}}}"#
            )),
            Malformed("ObjectRequired"),
        ),
        (
            packet(&censored(r#"{"type":"Current"}"#)),
            Unsupported("UnsupportedPositionKind"),
        ),
        (
            packet(&format!(
                r#""validity":{{"type":"Unbounded","basis":{basis}}}"#
            )),
            Unsupported("UnsupportedValidityKind"),
        ),
        (
            packet(&format!(
                r#""roles":[{{"role":"HostTime","assertion":null,"basis":{basis}}}]"#
            )),
            Unsupported("UnsupportedTemporalRole"),
        ),
        (
            packet(&roles.replace("\"version\":17", "\"version\":0")),
            Malformed("InvalidRecordVersion"),
        ),
        (
            packet(&recurrence.replace("opaque-pattern@v7", "")),
            Malformed("NonemptyStringRequired"),
        ),
    ];
    // Same family: narrow end-aware capability, no implicit end conversion.
    // PublishedStrict rejects duplicate JSON before decode. An untrusted
    // JsonValue instead reaches decode's whole-value admission: the preserving
    // emitter rejects the duplicate before typed traversal, as InvalidCarrierJson.
    let mut q = packet(&interval(lexical_beginning));
    mutate_packet(&mut q, |p| {
        let mut v = f(p, "validity").clone();
        let mut end = f(&v, "end").clone();
        let JsonValue::Object(members) = &mut end else {
            panic!("end object fixture")
        };
        members.push((
            tos_foundation::JsonString::from_utf8("type"),
            parse(r#""Unknown""#),
        ));
        set(&mut v, "end", end);
        set(p, "validity", v);
    });
    negatives.push((q, Malformed("InvalidCarrierJson")));
    for (end, error) in [
        // JSON null reaches the typed end object guard.
        ("null".to_owned(), Malformed("ObjectRequired")),
    ]
    .into_iter()
    .chain(
        [
            "Known",
            "NoBoundDeclared",
            "Unbounded",
            "Unspecified",
            "NotApplicable",
            "RightCensored",
        ]
        .into_iter()
        .map(|tag| {
            (
                format!(r#"{{"type":"{tag}"}}"#),
                Unsupported("UnsupportedIntervalEndKind"),
            )
        }),
    )
    .chain(
        [
            "position",
            "inclusivity",
            "precision",
            "explicit_basis_ref",
            "source_profile",
            "lexical_marker",
            "observed_through",
            "cutoff_basis",
        ]
        .into_iter()
        .map(|key| {
            (
                format!(r#"{{"type":"Unknown","{key}":null}}"#),
                Malformed("ConflictingUnknownIntervalEnd"),
            )
        }),
    )
    .chain(["resolved", "admitted"].into_iter().map(|key| {
        (
            format!(r#"{{"type":"Unknown","{key}":true}}"#),
            Malformed("CarrierAuthorityFlagNotAllowed"),
        )
    })) {
        negatives.push((packet(&format!(r#""validity":{{"type":"Interval","beginning":{lexical_beginning},"end":{end},"basis":{basis}}}"#)), error));
    }
    for key in ["beginning", "end", "basis"] {
        let mut q = packet(&interval(lexical_beginning));
        mutate_packet(&mut q, |p| {
            let mut v = f(p, "validity").clone();
            remove(&mut v, key);
            set(p, "validity", v);
        });
        negatives.push((q, Malformed(key)));
    }
    for (key, value, error) in [
        ("beginning", JsonValue::Null, Malformed("ObjectRequired")),
        ("basis", JsonValue::Null, Malformed("ObjectRequired")),
        (
            "observed_through",
            JsonValue::Null,
            Malformed("ConflictingIntervalValidity"),
        ),
        (
            "cutoff_basis",
            JsonValue::Null,
            Malformed("ConflictingIntervalValidity"),
        ),
        (
            "resolved",
            JsonValue::Bool(true),
            Malformed("CarrierAuthorityFlagNotAllowed"),
        ),
        (
            "admitted",
            JsonValue::Bool(true),
            Malformed("CarrierAuthorityFlagNotAllowed"),
        ),
    ] {
        let mut q = packet(&interval(lexical_beginning));
        mutate_packet(&mut q, |p| {
            let mut v = f(p, "validity").clone();
            set(&mut v, key, value);
            set(p, "validity", v);
        });
        negatives.push((q, error));
    }
    for key in ["resolved", "admitted"] {
        let mut q = packet(&unknown);
        mutate_packet(&mut q, |p| set(p, key, JsonValue::Bool(true)));
        negatives.push((q, Malformed("CarrierAuthorityFlagNotAllowed")));
        for position in [
            coordinate.replace(r#""trs":{"#, &format!(r#""trs":{{"{key}":true,"#)),
            calendar.replace(
                r#""resolution":"year""#,
                &format!(r#""resolution":{{"type":"Fraction","digits":3,"{key}":true}}"#),
            ),
            calendar.replace(
                r#""resolution":"year""#,
                &format!(
                    r#""resolution":{{"type":"NamedUnit","unit":"source-unit","{key}":true}}"#
                ),
            ),
        ] {
            negatives.push((
                packet(&censored(&position)),
                Malformed("CarrierAuthorityFlagNotAllowed"),
            ));
        }
        for payload in [&unknown, &censored(coordinate)] {
            let mut q = packet(payload);
            mutate_packet(&mut q, |p| {
                let mut v = f(p, "validity").clone();
                set(&mut v, key, JsonValue::Bool(true));
                set(p, "validity", v);
            });
            negatives.push((q, Malformed("CarrierAuthorityFlagNotAllowed")));
        }
    }
    let mut q = packet(&unknown);
    mutate_packet(&mut q, |p| {
        set(
            p,
            "schema_version",
            parse(r#""tos_temporal_claim_carrier_v2""#),
        )
    });
    negatives.push((q, Unsupported("UnsupportedSchemaVersion")));
    let mut q = packet(&unknown);
    mutate_packet(&mut q, |p| {
        let mut v = f(p, "validity").clone();
        let mut b = f(&v, "basis").clone();
        remove(&mut b, "source_locator");
        set(&mut v, "basis", b);
        set(p, "validity", v);
    });
    negatives.push((q, Malformed("source_locator")));
    for (q, expected) in negatives {
        let original = q.clone();
        assert_eq!(decode_claim_carrier(&q, budget), Err(expected));
        assert_eq!(
            q, original,
            "refusal must retain complete unsupported input"
        );
    }
    assert_eq!(decode_claim_carrier(&parse("{}"), budget), Ok(None));
    assert!(matches!(
        decode_claim_carrier(&parse(r#"{"temporal_claim_carrier":null}"#), budget),
        Err(Malformed(_))
    ));
    for q in [packet(&unknown), packet(&interval(lexical_beginning))] {
        let carrier = decode_claim_carrier(&q, budget).unwrap().unwrap();
        for limited in [
            TemporalBudget {
                max_expression_bytes: 1,
                ..budget
            },
            TemporalBudget {
                max_members: 1,
                ..budget
            },
        ] {
            assert_eq!(
                decode_claim_carrier(&q, limited),
                Err(ClaimCarrierError::ResourceLimit)
            );
            assert_eq!(
                encode_claim_carrier(&parse("{}"), &carrier, limited),
                Err(ClaimCarrierError::ResourceLimit)
            );
        }
    }
}

#[test]
fn complete_transport_admission_precedes_dispatch_and_preserved_clones() {
    let empty = parse("{}");
    let large_value = JsonValue::String(tos_foundation::JsonString::from_utf8(&"x".repeat(128)));
    let unused = |value| {
        JsonValue::Object(vec![(
            tos_foundation::JsonString::from_utf8("unused"),
            value,
        )])
    };
    let large_input = unused(large_value.clone());
    let large_key = JsonValue::Object(vec![(
        tos_foundation::JsonString::from_utf8(&"k".repeat(128)),
        JsonValue::Null,
    )]);
    let large_number = unused(JsonValue::Number(tos_foundation::JsonNumber {
        kind: tos_foundation::JsonNumberKind::Int,
        lexeme: "9".repeat(128),
    }));
    let byte_limited = TemporalBudget {
        max_expression_bytes: 64,
        ..TemporalBudget::default()
    };
    for (op, input, context) in [
        ("difference", large_input.clone(), empty.clone()),
        ("convert", empty.clone(), large_input.clone()),
        ("difference", large_key, empty.clone()),
        ("difference", large_number, empty.clone()),
        // Missing raw would be MalformedRequest if dispatch ran first.
        ("capture_expression", large_input, empty.clone()),
    ] {
        let before = (input.clone(), context.clone());
        assert!(matches!(
            execute_temporal_operation(op, &input, &context, byte_limited),
            Err(TemporalError::ResourceExhausted(_))
        ));
        assert_eq!((input, context), before);
    }
    let many = unused(JsonValue::Array(vec![JsonValue::Null; 64]));
    assert!(matches!(
        execute_temporal_operation(
            "difference",
            &many,
            &empty,
            TemporalBudget {
                max_expression_bytes: 4096,
                max_members: 16,
                ..TemporalBudget::default()
            }
        ),
        Err(TemporalError::ResourceExhausted(_))
    ));
    // Each root alone has only two nodes, but together they require four.
    let two_nodes = parse(r#"{"x":[]}"#);
    assert!(matches!(
        execute_temporal_operation(
            "not-an-operation",
            &two_nodes,
            &two_nodes,
            TemporalBudget {
                max_members: 3,
                ..TemporalBudget::default()
            }
        ),
        Err(TemporalError::ResourceExhausted(_))
    ));
    // Each compact root is nine bytes; their aggregate is eighteen.
    let nine_bytes = parse(r#"{"x":"a"}"#);
    assert!(matches!(
        execute_temporal_operation(
            "not-an-operation",
            &nine_bytes,
            &nine_bytes,
            TemporalBudget {
                max_expression_bytes: 17,
                ..TemporalBudget::default()
            }
        ),
        Err(TemporalError::ResourceExhausted(_))
    ));
    assert_eq!(
        execute_temporal_operation(
            "capture_expression",
            &JsonValue::Null,
            &empty,
            TemporalBudget {
                max_expression_bytes: 0,
                max_members: 0,
                max_pairings: 0,
            }
        ),
        Err(TemporalError::MalformedRequest(
            "input and context must be objects"
        ))
    );
}

#[test]
fn complete_transport_uses_exact_preserved_byte_node_and_depth_boundaries() {
    let empty = parse("{}");
    // Independent compact spelling counts: BOTH roots include their braces.
    // Pairing limits remain operation-owned; the unknown branch needs none.
    for (input, bytes, members) in [
        (empty.clone(), 4, 2),
        (parse(r#"{"x":"\"\\\n😀"}"#), 20, 3),
        (parse(r#"{"x":"\u0001"}"#), 16, 3),
        (parse(r#"{"x":"😀"}"#), 14, 3),
    ] {
        let exact = TemporalBudget {
            max_expression_bytes: bytes,
            max_members: members,
            max_pairings: 0,
        };
        let value = execute_temporal_operation("not-an-operation", &input, &empty, exact).unwrap();
        assert_eq!(s(&value, "status"), Some("Unsupported"));
        for below in [
            TemporalBudget {
                max_expression_bytes: bytes - 1,
                ..exact
            },
            TemporalBudget {
                max_members: members - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                execute_temporal_operation("not-an-operation", &input, &empty, below),
                Err(TemporalError::ResourceExhausted(_))
            ));
        }
    }
    let nested = |arrays| {
        let mut value = JsonValue::Null;
        for _ in 0..arrays {
            value = JsonValue::Array(vec![value]);
        }
        JsonValue::Object(vec![(
            tos_foundation::JsonString::from_utf8("unused"),
            value,
        )])
    };
    // Object root is depth0; sixty-three arrays put the leaf at depth64.
    let depth64 = nested(63);
    assert!(
        execute_temporal_operation(
            "not-an-operation",
            &depth64,
            &empty,
            TemporalBudget::default()
        )
        .is_ok()
    );
    let depth65 = nested(64);
    assert!(matches!(
        execute_temporal_operation(
            "not-an-operation",
            &depth65,
            &empty,
            TemporalBudget::default()
        ),
        Err(TemporalError::ResourceExhausted(_))
    ));
}

#[test]
fn complete_transport_keeps_native_opaque_integer_float_and_unicode_values() {
    let input = parse(
        r#"{"position":{"type":"OrdinalPosition","value":10,"trs_ref":"source-v1"},"target_trs":"target-v2","crosswalk_version":"rule@v1","fixture_mapping":{"10":42},"opaque":9007199254740993,"fraction":1.25,"signed_zero":-0,"float_spelling":1e+2,"text":"\ud800"}"#,
    );
    let context = parse(
        r#"{"opaque":9007199254740993,"fraction":1.25,"signed_zero":-0,"float_spelling":1e+2,"text":"\ud800"}"#,
    );
    let budget = TemporalBudget::default();
    let direct = calendar_adapters::convert(&input, &context, budget).unwrap();
    let transported = execute_temporal_operation("convert", &input, &context, budget).unwrap();
    assert_eq!(s(&transported, "status"), Some("Unique"));
    for (key, value) in direct.as_object().unwrap() {
        assert_eq!(transported.object_get(key.as_str().unwrap()), Some(value));
    }
    assert_eq!(f(&transported, "preserved_input"), &input);
    assert_eq!(f(&transported, "preserved_context"), &context);
}
