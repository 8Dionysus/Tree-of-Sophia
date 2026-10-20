//! Independent exact-axis and lexical preservation acceptance.
use tos_foundation::temporal::*;
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};

fn execute(raw: &str) -> JsonValue {
    let doc = parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    let context = JsonValue::Object(vec![]);
    execute_temporal_operation(
        "relate_extents",
        doc.root(),
        &context,
        TemporalBudget::default(),
    )
    .unwrap()
}

fn axis() -> Axis {
    Axis {
        trs: TrsRef {
            id: "arbitrary-comparable-axis".into(),
            version: "revision-17".into(),
        },
        unit: "fixture-tick".into(),
        origin: "explicit-origin".into(),
    }
}
fn interval(start: i64, end: i64, axis: Axis) -> ExactExtent {
    ExactExtent::Interval {
        axis,
        beginning: start,
        end,
        beginning_inclusivity: Inclusivity::Inclusive,
        end_inclusivity: Inclusivity::Exclusive,
    }
}

#[test]
fn all_thirteen_exact_allen_relations_and_shift_invariance() {
    let cases = [
        ([0, 1], [2, 3], Relation::Before),
        ([0, 1], [1, 2], Relation::Meets),
        ([0, 2], [1, 3], Relation::Overlaps),
        ([0, 1], [0, 2], Relation::Starts),
        ([1, 2], [0, 3], Relation::During),
        ([1, 2], [0, 2], Relation::Finishes),
        ([0, 1], [0, 1], Relation::Equals),
        ([2, 3], [0, 1], Relation::After),
        ([1, 2], [0, 1], Relation::MetBy),
        ([1, 3], [0, 2], Relation::OverlappedBy),
        ([0, 2], [0, 1], Relation::StartedBy),
        ([0, 3], [1, 2], Relation::Contains),
        ([0, 2], [1, 2], Relation::FinishedBy),
    ];
    for (left, right, expected) in cases {
        for shift in [-100_000_i64, 0, 237_901] {
            let a = interval(left[0] + shift, left[1] + shift, axis());
            let b = interval(right[0] + shift, right[1] + shift, axis());
            assert_eq!(
                relate_exact_extents(&a, &b),
                RelationOutcome::Entailed(expected)
            );
        }
    }
}

#[test]
fn full_i64_range_does_not_overflow_relation_or_coordinate_comparison() {
    let a = interval(i64::MIN, i64::MIN + 1, axis());
    let b = interval(i64::MAX - 1, i64::MAX, axis());
    assert_eq!(
        relate_exact_extents(&a, &b),
        RelationOutcome::Entailed(Relation::Before)
    );
    let position = |value| Position::Coordinate {
        value,
        unit: axis().unit,
        origin: axis().origin,
        trs: axis().trs,
    };
    assert_eq!(
        compare_positions(&position(i64::MIN), &position(i64::MAX)),
        PositionComparison::Less
    );
    assert_eq!(
        compare_positions(&position(i64::MAX), &position(i64::MIN)),
        PositionComparison::Greater
    );
}

#[test]
fn axis_id_version_unit_and_origin_are_all_required_for_comparison() {
    let left = interval(0, 1, axis());
    let mut axes = [axis(), axis(), axis(), axis()];
    axes[0].trs.id = "other-axis".into();
    axes[1].trs.version = "revision-18".into();
    axes[2].unit = "other-unit".into();
    axes[3].origin = "other-origin".into();
    for other in axes {
        let actual = relate_exact_extents(&left, &interval(2, 3, other));
        assert!(
            matches!(
                actual,
                RelationOutcome::Incomparable(_)
                    | RelationOutcome::Unresolved(_)
                    | RelationOutcome::Unsupported(_)
            ),
            "{actual:?}"
        );
    }
}

#[test]
fn unknown_extents_are_not_equal_and_reversed_intervals_are_not_valid() {
    assert!(matches!(
        relate_exact_extents(&ExactExtent::Unknown, &ExactExtent::Unknown),
        RelationOutcome::Unresolved(_)
    ));
    let actual = relate_exact_extents(&interval(3, 2, axis()), &interval(4, 5, axis()));
    assert!(
        matches!(actual, RelationOutcome::Contradicted(_)),
        "{actual:?}"
    );
}

#[test]
fn lexical_roundtrip_preserves_unknown_qualifiers_and_alternatives() {
    for raw in [
        "1984?",
        "1984~",
        "1984%",
        "1985-04-12/..",
        "1985-04-12/",
        "[1667,1668,1670..1672]",
        "{1667,1668}",
        "unparsed ancient era",
        "  unknown language λ  ",
    ] {
        let expression = capture_expression(
            raw,
            ExpressionProfile::Edtf {
                level: 2,
                version: "2019-02-04".into(),
            },
            None,
            TemporalBudget::default(),
        );
        assert_eq!(serialize_expression(&expression), raw);
        assert_eq!(expression.locale, None);
    }
}

#[test]
fn arbitrary_axis_rename_preserves_exact_answers() {
    for id in ["renamed-902", "α unusual axis", "not-a-calendar"] {
        let mut renamed = axis();
        renamed.trs.id = id.into();
        let left = interval(17, 19, renamed.clone());
        let right = interval(31, 37, renamed);
        assert_eq!(
            relate_exact_extents(&left, &right),
            RelationOutcome::Entailed(Relation::Before)
        );
    }
}

#[test]
fn request_axis_cannot_overwrite_explicitly_incompatible_operand_axes() {
    let actual = execute(
        r#"{"comparison_axis":"request-axis","left":{"start":1,"end":2,"trs_ref":"explicit-axis-A"},"right":{"start":3,"end":4,"trs_ref":"explicit-axis-B"}}"#,
    );
    assert_ne!(
        actual.object_get("status").and_then(JsonValue::as_str),
        Some("Entailed"),
        "{actual:?}"
    );
    let structured = execute(
        r#"{"comparison_axis":{"trs":{"id":"request-axis","version":"17"},"unit":"tick","origin":"explicit-origin"},"left":{"start":1,"end":2,"trs_ref":"explicit-axis-A"},"right":{"start":3,"end":4,"trs_ref":"explicit-axis-B"}}"#,
    );
    assert_ne!(
        structured.object_get("status").and_then(JsonValue::as_str),
        Some("Entailed"),
        "{structured:?}"
    );
    let comparable = execute(
        r#"{"comparison_axis":{"trs":{"id":"arbitrary-renamed-axis","version":"17"},"unit":"tick","origin":"explicit-origin"},"left":{"entity_id":"renamed-117","label":"Δ","start":1,"end":2,"beginning_inclusivity":"Inclusive","end_inclusivity":"Exclusive"},"right":{"entity_id":"renamed-902","label":"λ","start":3,"end":4,"beginning_inclusivity":"Inclusive","end_inclusivity":"Exclusive"}}"#,
    );
    assert_eq!(
        comparable.object_get("status").and_then(JsonValue::as_str),
        Some("Entailed"),
        "{comparable:?}"
    );
    assert_eq!(
        comparable
            .object_get("relation")
            .and_then(JsonValue::as_str),
        Some("before")
    );
}

#[test]
fn unknown_calendar_id_and_explicit_utc_do_not_supply_calendar_rules() {
    let actual = execute(
        r#"{"left":{"type":"Instant","local":"1945-04-02T06:00:00","trs_ref":"UnknownCalendar:no-rules-v9","zone":"UTC"},"right":{"type":"Instant","local":"1945-04-02T13:00:00","trs_ref":"UnknownCalendar:no-rules-v9","zone":"UTC"}}"#,
    );
    assert_ne!(
        actual.object_get("status").and_then(JsonValue::as_str),
        Some("Entailed"),
        "{actual:?}"
    );
}

#[test]
fn unspecified_inclusivity_and_partial_ordinal_order_do_not_entail() {
    let a = interval(0, 1, axis());
    let b = ExactExtent::Interval {
        axis: axis(),
        beginning: 2,
        end: 3,
        beginning_inclusivity: Inclusivity::Unspecified,
        end_inclusivity: Inclusivity::Exclusive,
    };
    assert!(matches!(
        relate_exact_extents(&a, &b),
        RelationOutcome::Unresolved(_)
    ));
    let pos = |rank| Position::Ordinal {
        node: format!("unrelated-label-{rank}"),
        rank: Some(rank),
        trs: axis().trs,
        ordering: OrderingCapability::Partial,
    };
    assert!(matches!(
        compare_positions(&pos(1), &pos(2)),
        PositionComparison::Unresolved(_) | PositionComparison::Incomparable(_)
    ));
    let date = || Position::Calendar {
        lexical: "1344".into(),
        resolution: Resolution::Year,
        trs: None,
        era: None,
    };
    assert_eq!(
        compare_positions(&date(), &date()),
        PositionComparison::Incomparable("UnknownTRS")
    );
}

#[test]
fn pairwise_budget_exhaustion_returns_undetermined_not_entailed() {
    let raw = br#"{"left":{"type":"ExactlyOneOf","positions":[7,8],"trs_ref":"explicit-ordinal"},"right":{"position":9,"trs_ref":"explicit-ordinal"},"quantifier":"for_all_members"}"#;
    let doc = parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let actual = execute_temporal_operation(
        "relate_extents",
        doc.root(),
        &JsonValue::Object(vec![]),
        TemporalBudget {
            max_pairings: 0,
            ..TemporalBudget::default()
        },
    )
    .unwrap();
    assert_eq!(
        actual.object_get("status").and_then(JsonValue::as_str),
        Some("Undetermined")
    );
    assert_eq!(
        actual.object_get("reason").and_then(JsonValue::as_str),
        Some("ResourceLimit")
    );
}

#[test]
fn partial_crosswalk_keeps_declared_precision_loss() {
    let raw = br#"{"position":{"value":10,"trs_ref":"A@v1"},"target_trs":"B@v1","crosswalk_version":"rule@v1","fixture_mapping":{"10":{"status":"Partial","position":{"value":42},"precision_loss":["fractional-coordinate-lost"]}}}"#;
    let input = parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let actual = execute_temporal_operation(
        "convert",
        input.root(),
        &JsonValue::Object(vec![]),
        TemporalBudget::default(),
    )
    .unwrap();
    assert_eq!(
        actual.object_get("status").and_then(JsonValue::as_str),
        Some("Partial"),
        "{actual:?}"
    );
    let expected = input
        .root()
        .object_get("fixture_mapping")
        .unwrap()
        .object_get("10")
        .unwrap()
        .object_get("precision_loss")
        .unwrap();
    assert_eq!(
        actual.object_get("precision_loss"),
        Some(expected),
        "{actual:?}"
    );
}

#[test]
fn unknown_qualifier_width_never_becomes_an_exact_relation() {
    for qualifier in ["Uncertain", "Approximate", "UncertainAndApproximate"] {
        let raw = format!(
            r#"{{"comparison_axis":{{"trs":{{"id":"explicit-axis","version":"17"}},"unit":"tick","origin":"explicit-origin"}},"left":{{"type":"Instant","position":1,"qualifier":"{qualifier}"}},"right":{{"type":"Instant","position":3,"qualifier":"None"}}}}"#
        );
        let actual = execute(&raw);
        assert_ne!(
            actual.object_get("status").and_then(JsonValue::as_str),
            Some("Entailed"),
            "{actual:?}"
        );
    }
}

#[test]
fn alternative_dependence_is_preserved_without_invented_cartesian_worlds() {
    use tos_foundation::JsonString;
    fn parse(raw: &str) -> JsonValue {
        parse_json(
            raw.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .root()
        .clone()
    }
    fn set(value: &mut JsonValue, key: &str, data: JsonValue) {
        let JsonValue::Object(fields) = value else {
            panic!("object")
        };
        if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
            *old = data;
        } else {
            fields.push((JsonString::from_utf8(key), data));
        }
    }
    fn text(raw: &str) -> JsonValue {
        JsonValue::String(JsonString::from_utf8(raw))
    }
    fn run(input: &JsonValue, budget: TemporalBudget) -> JsonValue {
        let actual =
            execute_temporal_operation("relate_extents", input, &JsonValue::Object(vec![]), budget)
                .unwrap();
        assert_eq!(
            actual.object_get("preserved_input"),
            Some(input),
            "{actual:?}"
        );
        actual
    }
    fn status(value: &JsonValue) -> Option<&str> {
        value.object_get("status").and_then(JsonValue::as_str)
    }
    fn reason(value: &JsonValue) -> Option<&str> {
        value.object_get("reason").and_then(JsonValue::as_str)
    }
    // The shared worlds allow only before. Cartesian marginals add the false
    // shared-world pair (10,1). v1 may refuse instead of solving joint worlds.
    let worlds = [(0, 1), (10, 11)];
    assert!(worlds.iter().all(|(a, b)| a < b));
    assert!(
        worlds
            .iter()
            .any(|(a, _)| worlds.iter().any(|(_, b)| a > b))
    );
    for (axis, ids, reverse) in [
        ("opaque-axis@v7", ["source-A", "source-B"], false),
        ("renamed-axis-λ@v31", ["A", "a"], true),
    ] {
        let mut base = parse(
            r#"{"left":{"type":"ExactlyOneOf","positions":[0,10],"trs_ref":"axis"},"right":{"type":"ExactlyOneOf","positions":[1,11],"trs_ref":"axis"},"quantifier":"for_all_members","comparison_profile":"OrdinalRanksV1"}"#,
        );
        for (index, key) in ["left", "right"].iter().enumerate() {
            let JsonValue::Object(fields) = &mut base else {
                unreachable!()
            };
            let operand = &mut fields
                .iter_mut()
                .find(|(k, _)| k.as_str() == Some(key))
                .unwrap()
                .1;
            set(operand, "trs_ref", text(axis));
            set(operand, "id", text(ids[index]));
            set(operand, "label", text(ids[1 - index]));
            if reverse {
                let JsonValue::Object(fields) = operand else {
                    unreachable!()
                };
                let JsonValue::Array(values) = &mut fields
                    .iter_mut()
                    .find(|(k, _)| k.as_str() == Some("positions"))
                    .unwrap()
                    .1
                else {
                    unreachable!()
                };
                values.reverse();
            }
        }
        for (mode, expected_status, expected_reason) in [
            (Some("Independent"), "Possible", ""),
            (
                Some("Correlated"),
                "Unsupported",
                "CorrelatedAlternativesNotImplemented",
            ),
            (
                Some("Unknown"),
                "Unresolved",
                "AlternativeDependenceUnknown",
            ),
            (
                Some("invalid-mode"),
                "Unresolved",
                "AlternativeDependenceUnknown",
            ),
            (None, "Unresolved", "AlternativeDependenceUnknown"),
        ] {
            let mut input = base.clone();
            if let Some(mode) = mode {
                set(&mut input, "alternative_dependence", text(mode));
            }
            if mode == Some("Correlated") || mode == Some("Unknown") {
                set(&mut input, "joint_worlds", parse("[[0,1],[10,11]]"));
            }
            let actual = run(
                &input,
                TemporalBudget {
                    max_pairings: 4,
                    ..TemporalBudget::default()
                },
            );
            assert_eq!(status(&actual), Some(expected_status), "{actual:?}");
            if mode == Some("Independent") {
                use std::collections::BTreeSet;
                let pairs = actual
                    .object_get("pair_relations")
                    .and_then(JsonValue::as_array)
                    .unwrap();
                assert_eq!(pairs.len(), 4);
                let mut seen = BTreeSet::new();
                let mut union = BTreeSet::new();
                for pair in pairs {
                    let integer = |key: &str| {
                        let JsonValue::Number(n) = pair.object_get(key).unwrap() else {
                            panic!("exact coordinate")
                        };
                        n.lexeme.parse::<i64>().unwrap()
                    };
                    let (a, b) = (integer("left"), integer("right"));
                    assert!([0, 10].contains(&a) && [1, 11].contains(&b));
                    assert!(seen.insert((a, b)), "duplicate pair witness");
                    let expected = if a < b {
                        "before"
                    } else if a == b {
                        "equals"
                    } else {
                        "after"
                    };
                    let relations = pair
                        .object_get("relations")
                        .and_then(JsonValue::as_array)
                        .unwrap();
                    assert_eq!(relations.len(), 1);
                    assert_eq!(relations[0].as_str(), Some(expected));
                    union.insert(expected);
                }
                assert_eq!(union, BTreeSet::from(["before", "after"]));
                let reported = actual
                    .object_get("relations")
                    .and_then(JsonValue::as_array)
                    .unwrap();
                assert_eq!(reported.len(), 2);
                assert_eq!(
                    reported
                        .iter()
                        .map(|v| v.as_str().unwrap())
                        .collect::<BTreeSet<_>>(),
                    union
                );
                let limited = run(
                    &input,
                    TemporalBudget {
                        max_pairings: 3,
                        ..TemporalBudget::default()
                    },
                );
                assert_eq!(
                    (status(&limited), reason(&limited)),
                    (Some("Undetermined"), Some("ResourceLimit"))
                );
                assert!(
                    limited
                        .object_get("pair_relations")
                        .is_none_or(|v| v.as_array().is_some_and(|pairs| pairs.is_empty()))
                );
            } else {
                assert_eq!(reason(&actual), Some(expected_reason), "{actual:?}");
            }
        }
        for (right_positions, expected) in [("[11,12]", "Entailed"), ("[]", "Unresolved")] {
            let mut input = base.clone();
            set(&mut input, "alternative_dependence", text("Independent"));
            let JsonValue::Object(fields) = &mut input else {
                unreachable!()
            };
            let right = &mut fields
                .iter_mut()
                .find(|(k, _)| k.as_str() == Some("right"))
                .unwrap()
                .1;
            set(right, "positions", parse(right_positions));
            let actual = run(&input, TemporalBudget::default());
            assert_eq!(status(&actual), Some(expected), "{actual:?}");
            if expected == "Entailed" {
                assert_eq!(
                    actual.object_get("relation").and_then(JsonValue::as_str),
                    Some("before")
                );
                assert_eq!(
                    actual
                        .object_get("pair_relations")
                        .and_then(JsonValue::as_array)
                        .unwrap()
                        .len(),
                    4
                );
            }
        }
        // A supported scalar control proves the Independent route; the named
        // profile applies only to omission, never overrides explicit Unknown.
        let mut scalar = base.clone();
        set(&mut scalar, "right", parse(r#"{"position":11}"#));
        let JsonValue::Object(fields) = &mut scalar else {
            unreachable!()
        };
        let right = &mut fields
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some("right"))
            .unwrap()
            .1;
        set(right, "trs_ref", text(axis));
        for mode in [Some("Independent"), None, Some("Unknown")] {
            let mut input = scalar.clone();
            set(
                &mut input,
                "alternative_profile",
                text("IndependentAlternativesV1"),
            );
            if let Some(mode) = mode {
                set(&mut input, "alternative_dependence", text(mode));
            }
            let actual = run(&input, TemporalBudget::default());
            if mode == Some("Unknown") {
                assert_eq!(
                    (status(&actual), reason(&actual)),
                    (Some("Unresolved"), Some("AlternativeDependenceUnknown"))
                );
            } else {
                assert_eq!(status(&actual), Some("Entailed"));
                assert_eq!(
                    actual.object_get("relation").and_then(JsonValue::as_str),
                    Some("before")
                );
            }
        }
        for key in [
            "correlation_ref",
            "world_constraints",
            "dependency_refs",
            "joint_worlds",
        ] {
            for level in [None, Some("left"), Some("right")] {
                let mut input = scalar.clone();
                set(&mut input, "alternative_dependence", text("Independent"));
                let metadata = if key == "correlation_ref" {
                    text("opaque-dependency@revision7")
                } else {
                    parse("[[0,1],[10,11]]")
                };
                if let Some(level) = level {
                    let JsonValue::Object(fields) = &mut input else {
                        unreachable!()
                    };
                    let operand = &mut fields
                        .iter_mut()
                        .find(|(k, _)| k.as_str() == Some(level))
                        .unwrap()
                        .1;
                    set(operand, key, metadata);
                } else {
                    set(&mut input, key, metadata);
                }
                let actual = run(&input, TemporalBudget::default());
                assert!(
                    matches!(status(&actual), Some("Unsupported" | "Unresolved")),
                    "{actual:?}"
                );
            }
        }
        set(&mut base, "alternative_dependence", text("Correlated"));
        let actual = run(
            &base,
            TemporalBudget {
                max_pairings: 0,
                ..TemporalBudget::default()
            },
        );
        assert_eq!(
            (status(&actual), reason(&actual)),
            (Some("Undetermined"), Some("ResourceLimit"))
        );
    }
}
