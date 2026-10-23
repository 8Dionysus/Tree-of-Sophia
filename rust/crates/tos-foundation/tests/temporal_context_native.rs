use tos_foundation::JsonValue;
use tos_foundation::temporal::{TemporalBudget, execute_temporal_operation};
fn parse(s: &str) -> JsonValue {
    tos_foundation::parse_json(
        s.as_bytes(),
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .unwrap()
    .root()
    .clone()
}
fn f<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn s<'a>(v: &'a JsonValue, k: &str) -> Option<&'a str> {
    f(v, k).as_str()
}
fn context(key: &str) -> JsonValue {
    parse(&format!(
        r#"{{"temporal_systems":{{"{key}":{{"calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical","rules_version":"rules-r17"}}}}}}"#
    ))
}
fn run(op: &str, input: &str, context: &JsonValue) -> JsonValue {
    execute_temporal_operation(op, &parse(input), context, TemporalBudget::default()).unwrap()
}

#[test]
fn arbitrary_admitted_keys_resolve_buckets_and_local_clocks_without_id_branches() {
    for key in ["sys-A", "changed-key-902", "opaque_λ"] {
        let c = context(key);
        let resolved = run(
            "resolve_position",
            &format!(r#"{{"expression":"2000-02","trs_ref":"{key}"}}"#),
            &c,
        );
        assert_eq!(s(&resolved, "status"), Some("Resolved"));
        assert_eq!(s(f(&resolved, "position"), "resolution"), Some("month"));

        let r = run(
            "derive_extent",
            &format!(r#"{{"expression":"2000-02","resolution":"month","trs_ref":"{key}"}}"#),
            &c,
        );
        assert_eq!(s(&r, "status"), Some("Resolved"));
        assert_eq!(s(f(&r, "value"), "end_exclusive"), Some("2000-03-01"));
        assert_eq!(s(&r, "trs_rules_version"), Some("rules-r17"));
        let r = run(
            "relate_extents",
            &format!(
                r#"{{"left":{{"type":"Instant","local":"1945-04-02T06:00:00","zone":"UTC","trs_ref":"{key}"}},"right":{{"type":"Instant","local":"1945-04-02T13:00:00","zone":"UTC","trs_ref":"{key}"}}}}"#
            ),
            &c,
        );
        assert_eq!(s(&r, "status"), Some("Entailed"));
        assert_eq!(s(&r, "relation"), Some("before"));
        assert_eq!(s(&r, "trs_rules_version"), Some("rules-r17"));
    }
}
#[test]
fn unknown_key_incomplete_rules_and_conflicting_calendar_never_override() {
    let input = r#"{"expression":"1945","resolution":"year","trs_ref":"sys-A"}"#;
    assert_ne!(
        s(&run("derive_extent", input, &context("sys-B")), "status"),
        Some("Resolved")
    );
    let missing_version = parse(
        r#"{"temporal_systems":{"sys-A":{"calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical"}}}"#,
    );
    assert_eq!(
        s(&run("derive_extent", input, &missing_version), "reason"),
        Some("RuleVersionRequired")
    );
    let conflicting =
        r#"{"expression":"1945","resolution":"year","trs_ref":"sys-A","calendar":"Julian"}"#;
    assert_eq!(
        s(
            &run("derive_extent", conflicting, &context("sys-A")),
            "reason"
        ),
        Some("ConflictingCalendarDeclaration")
    );
    let wrong_rules = parse(
        r#"{"temporal_systems":{"sys-A":{"calendar_algorithm":"Julian","year_numbering":"astronomical","rules_version":"v1"}}}"#,
    );
    assert_ne!(
        s(&run("derive_extent", input, &wrong_rules), "status"),
        Some("Resolved")
    );
}
#[test]
fn admitted_alternatives_require_independence_and_preserve_bucket_uncertainty() {
    let c = context("sys-A");
    let approximate = run(
        "relate_extents",
        r#"{"left":{"type":"ExactlyOneOf","positions":[1667],"trs_ref":"sys-A","qualifier":"Approximate"},"right":{"position":1669,"trs_ref":"sys-A"},"quantifier":"for_all_members","alternative_dependence":"Independent"}"#,
        &c,
    );
    assert_eq!(s(&approximate, "status"), Some("Unresolved"));
    let r = run(
        "relate_extents",
        r#"{"left":{"type":"ExactlyOneOf","positions":[1667,1668],"resolution":"year","trs_ref":"sys-A"},"right":{"position":1669,"resolution":"year","trs_ref":"sys-A"},"quantifier":"for_all_members","alternative_dependence":"Independent"}"#,
        &c,
    );
    assert_eq!(s(&r, "status"), Some("Entailed"));
    assert_eq!(s(&r, "relation"), Some("before"));
    let r = run(
        "relate_extents",
        r#"{"left":{"type":"ExactlyOneOf","positions":[1669],"resolution":"year","trs_ref":"sys-A"},"right":{"position":1669,"resolution":"year","trs_ref":"sys-A"},"quantifier":"for_all_members","alternative_dependence":"Independent"}"#,
        &c,
    );
    assert_eq!(s(&r, "status"), Some("Possible"));
    assert_eq!(f(&r, "relations").as_array().unwrap().len(), 3);

    // Same marginals can describe independent choices or shared worlds. This
    // bounded profile supports declared independent scalar and finite-set pairs.
    let base = r#""left":{"type":"ExactlyOneOf","positions":[0,10],"trs_ref":"r"},"right":{"position":11,"trs_ref":"r"},"comparison_profile":"OrdinalRanksV1","quantifier":"for_all_members""#;
    let cases = [
        (
            r#""alternative_dependence":"Independent""#,
            "Entailed",
            "",
            false,
        ),
        (
            r#""alternative_dependence":"Correlated","joint_worlds":[[0,1],[10,11]]"#,
            "Unsupported",
            "CorrelatedAlternativesNotImplemented",
            false,
        ),
        (
            r#""alternative_dependence":"Unknown""#,
            "Unresolved",
            "AlternativeDependenceUnknown",
            false,
        ),
        (
            r#""alternative_profile":"IndependentAlternativesV1""#,
            "Entailed",
            "",
            false,
        ),
        (
            r#""alternative_dependence":null,"alternative_profile":"IndependentAlternativesV1""#,
            "Unresolved",
            "AlternativeDependenceUnknown",
            false,
        ),
        (
            r#""alternative_dependence":7,"alternative_profile":"IndependentAlternativesV1""#,
            "Unresolved",
            "AlternativeDependenceUnknown",
            false,
        ),
        (
            r#""alternative_dependence":"Unknown","alternative_profile":"IndependentAlternativesV1""#,
            "Unresolved",
            "AlternativeDependenceUnknown",
            false,
        ),
        (
            r#""alternative_dependence":"unrecognized""#,
            "Unresolved",
            "AlternativeDependenceUnknown",
            false,
        ),
        (
            r#""alternative_dependence":"Correlated""#,
            "Undetermined",
            "ResourceLimit",
            true,
        ),
    ];
    let mut requests: Vec<(String, &str, &str, bool)> = cases
        .iter()
        .map(|(extra, status, reason, zero)| {
            (format!("{{{base},{extra}}}"), *status, *reason, *zero)
        })
        .collect();
    requests.push((
        format!("{{{base}}}"),
        "Unresolved",
        "AlternativeDependenceUnknown",
        false,
    ));
    for metadata in [
        r#""correlation_ref":"shared-k""#,
        r#""world_constraints":{"rule":"B=A+1"}"#,
        r#""dependency_refs":["shared-k"]"#,
        r#""joint_worlds":[[0,1],[10,11]]"#,
    ] {
        requests.push((
            format!(r#"{{{base},"alternative_dependence":"Independent",{metadata}}}"#),
            "Unsupported",
            "ConflictingAlternativeDependence",
            false,
        ));
        let nested = base.replace(
            r#""positions":[0,10]"#,
            &format!(r#""positions":[0,10],{metadata}"#),
        );
        requests.push((
            format!(r#"{{{nested},"alternative_dependence":"Independent"}}"#),
            "Unsupported",
            "ConflictingAlternativeDependence",
            false,
        ));
    }
    let two_sets = base.replace(
        r#""right":{"position":11,"trs_ref":"r"}"#,
        r#""right":{"type":"ExactlyOneOf","positions":[1,11],"trs_ref":"r"}"#,
    );
    for (dependence, status, reason) in [
        ("Independent", "Possible", ""),
        (
            "Correlated",
            "Unsupported",
            "CorrelatedAlternativesNotImplemented",
        ),
        ("Unknown", "Unresolved", "AlternativeDependenceUnknown"),
    ] {
        requests.push((
            format!(r#"{{{two_sets},"alternative_dependence":"{dependence}"}}"#),
            status,
            reason,
            false,
        ));
    }
    requests.push((
        format!(r#"{{{two_sets},"alternative_dependence":"Independent"}}"#),
        "Undetermined",
        "ResourceLimit",
        false,
    ));
    let disjoint = two_sets.replace("[1,11]", "[20,21]");
    requests.push((
        format!(r#"{{{disjoint},"alternative_dependence":"Independent"}}"#),
        "Entailed",
        "",
        false,
    ));
    let empty = two_sets.replace("[1,11]", "[]");
    requests.push((
        format!(r#"{{{empty},"alternative_dependence":"Independent"}}"#),
        "Unresolved",
        "EmptyPossibilities",
        false,
    ));
    // Neither a numeric member nor a rank profile erases supplied uncertainty
    // or resolution. Exercise the left set, scalar right, and right set.
    for modifier in [
        r#""uncertainty":{"width":null}"#,
        r#""uncertainty":null"#,
        r#""resolution":"year""#,
        r#""resolution":null"#,
    ] {
        for (wire, target) in [
            (base, r#""positions":[0,10]"#),
            (base, r#""position":11"#),
            (two_sets.as_str(), r#""positions":[1,11]"#),
        ] {
            let wire = wire.replace(target, &format!("{target},{modifier}"));
            requests.push((
                format!(r#"{{{wire},"alternative_dependence":"Independent"}}"#),
                "Unresolved",
                "NonExactPosition",
                false,
            ));
        }
    }
    let exact = base.replace(
        r#""positions":[0,10]"#,
        r#""positions":[0,10],"resolution":"exact""#,
    );
    requests.push((
        format!(r#"{{{exact},"alternative_dependence":"Independent"}}"#),
        "Entailed",
        "",
        false,
    ));
    for (wire, status, reason, zero) in requests {
        let input = parse(&wire);
        let original = input.clone();
        let mut budget = TemporalBudget::default();
        budget.max_pairings = if zero {
            0
        } else if reason == "ResourceLimit" {
            3
        } else {
            4
        };
        let result =
            execute_temporal_operation("relate_extents", &input, &parse("{}"), budget).unwrap();
        assert_eq!(s(&result, "status"), Some(status), "{wire}");
        if status == "Possible" {
            assert_eq!(
                f(&result, "relations"),
                &parse(r#"["before","after"]"#),
                "{wire}"
            );
            let expected_pairs = parse(
                r#"[
                {"left":0,"right":1,"relations":["before"]},
                {"left":0,"right":11,"relations":["before"]},
                {"left":10,"right":1,"relations":["after"]},
                {"left":10,"right":11,"relations":["before"]}
            ]"#,
            );
            assert_eq!(f(&result, "pair_relations"), &expected_pairs, "{wire}");
        } else if reason.is_empty() {
            assert_eq!(s(&result, "relation"), Some("before"), "{wire}");
        } else {
            assert_eq!(s(&result, "reason"), Some(reason), "{wire}");
        }
        assert_eq!(f(&result, "preserved_input"), &original, "{wire}");
        assert_eq!(input, original);
    }
}
#[test]
fn actual_reversed_buckets_are_contradictory_only_with_declared_rules() {
    let input = r#"{"type":"ActualInterval","start":{"position":"1350","resolution":"year","trs_ref":"sys-A"},"end":{"position":"1349","resolution":"year","trs_ref":"sys-A"}}"#;
    let r = run("derive_extent", input, &context("sys-A"));
    assert_eq!(s(&r, "status"), Some("ContradictoryBounds"));
    assert_eq!(f(&r, "source_input_preserved"), &JsonValue::Bool(true));
    assert_ne!(
        s(&run("derive_extent", input, &parse("{}")), "status"),
        Some("ContradictoryBounds")
    );
    let approximate = input.replace(
        "\"position\":\"1350\"",
        "\"position\":\"1350\",\"qualifier\":\"Approximate\"",
    );
    assert_eq!(
        s(
            &run("derive_extent", &approximate, &context("sys-A")),
            "status"
        ),
        Some("Unresolved")
    );
}
#[test]
fn named_boundary_rules_require_exact_axis_reference_and_cannot_override_explicit_bounds() {
    let ctx = parse(
        r#"{"comparison_axis_definition":{"reference":"axis-ref@r7","trs":{"id":"any-id","version":"v3"},"unit":"tick","origin":"origin","boundary_profile":"ExactHalfOpenIntervalsV1"}}"#,
    );
    let input = r#"{"comparison_axis":"axis-ref@r7","left":{"start":1,"end":2},"right":{"start":3,"end":4}}"#;
    let r = run("relate_extents", input, &ctx);
    assert_eq!(s(&r, "status"), Some("Entailed"));
    assert_eq!(s(&r, "relation"), Some("before"));
    let conflicting = r#"{"comparison_axis":"axis-ref@r7","left":{"start":1,"end":2,"end_inclusivity":"Inclusive"},"right":{"start":3,"end":4}}"#;
    assert_eq!(
        s(&run("relate_extents", conflicting, &ctx), "reason"),
        Some("ConflictingBoundaryProfile")
    );
    let conflict_profiles = r#"{"comparison_axis":"axis-ref@r7","boundary_profile":"ExactClosedIntervalsV1","left":{"start":1,"end":2},"right":{"start":3,"end":4}}"#;
    assert_eq!(
        s(&run("relate_extents", conflict_profiles, &ctx), "reason"),
        Some("ConflictingBoundaryProfile")
    );
    let unknown = input.replace("axis-ref@r7", "other-ref");
    assert_ne!(
        s(&run("relate_extents", &unknown, &ctx), "status"),
        Some("Entailed")
    );
}

#[test]
fn supplemental_coordinate_filters_are_total_only_for_unique_explicit_comparable_sources() {
    use tos_foundation::temporal::*;
    use tos_foundation::{Digest256, ExactRecordRef, RecordVersion, SourceRevision, StableId};
    fn reference(id: &str) -> String {
        format!(
            r#"{{"id":"{id}","version":3,"digest":"1111111111111111111111111111111111111111111111111111111111111111"}}"#
        )
    }
    fn coordinate(value: &str) -> String {
        format!(
            r#"{{"type":"CoordinatePosition","resolution":"exact","value":{value},"trs":{{"id":"axis-X","version":"rules-1"}},"unit":"tick","origin":"O"}}"#
        )
    }
    fn basis() -> String {
        format!(
            r#"{{"source":{},"source_locator":null,"raw_evidence":"synthetic explicit source assertion"}}"#,
            reference("evidence")
        )
    }
    fn role_binding(role: &str, value: &str) -> String {
        let knower = if role == "AttributedKnowledge" {
            format!(r#", "knower_ref":{}"#, reference("knower"))
        } else {
            String::new()
        };
        format!(
            r#"{{"role":"{role}","assertion":{{"profile_ref":"tos_role_coordinate_assertion_v1","about_ref":{}{knower},"position":{}}},"basis":{}}}"#,
            reference("external-record"),
            coordinate(value),
            basis()
        )
    }
    fn role_query(role: &str, cutoff: &str) -> String {
        let knower = if role == "AttributedKnowledge" {
            format!(r#", "knower_ref":{}"#, reference("knower"))
        } else {
            String::new()
        };
        format!(
            r#"{{"role":"{role}","about_ref":{}{knower},"operator":"LessOrEqual","cutoff":{},"basis":{}}}"#,
            reference("external-record"),
            coordinate(cutoff),
            basis()
        )
    }
    fn filter(roles: &str, world: &str) -> JsonValue {
        parse(&format!(
            r#"{{"profile_ref":"tos_claim_coordinate_filter_v1","assertion_scope":"SupplementalCarrierOnly","assertion_selection":"ExactlyOnePerPredicate","non_total":"RefusePage","roles":[{roles}],"world_valid_time":{world}}}"#
        ))
    }
    fn carrier(roles: &str, validity: &str) -> TemporalClaimCarrier {
        decode_claim_carrier(&parse(&format!(r#"{{"temporal_claim_carrier":{{"schema_version":"tos_temporal_claim_carrier_v1","roles":[{roles}]{validity}}}}}"#)),TemporalBudget::default()).unwrap().unwrap()
    }
    let claim_ref = ExactRecordRef {
        id: StableId::parse("unrelated-selected-claim").unwrap(),
        version: RecordVersion::new(17).unwrap(),
        digest: Digest256::from_hex(&"2".repeat(64)).unwrap(),
    };
    let revision = SourceRevision(Digest256::from_hex(&"3".repeat(64)).unwrap());
    let budget = TemporalBudget::default();
    let evaluate = |c: Option<&TemporalClaimCarrier>, q: &JsonValue| {
        evaluate_claim_coordinate_filter(c, q, &claim_ref, revision, budget).unwrap()
    };
    for role in [
        "AttributedKnowledge",
        "SourcePublication",
        "SystemTransaction",
    ] {
        let q = filter(&role_query(role, "10"), "null");
        validate_claim_coordinate_filter(&q, budget).unwrap();
        for (value, outcome) in [("4", "Match"), ("10", "Match"), ("11", "NoMatch")] {
            let c = carrier(&role_binding(role, value), "");
            let original = c.clone();
            let query_original = q.clone();
            let result = evaluate(Some(&c), &q);
            assert_eq!(s(&result, "outcome"), Some(outcome));
            assert_eq!(
                s(&result, "assertion_scope"),
                Some("SupplementalCarrierOnly")
            );
            assert_eq!(s(&result, "legacy_object_assessment"), Some("NotExamined"));
            assert_eq!(
                s(&result, "source_revision"),
                Some(revision.0.to_prefixed().as_str())
            );
            assert_eq!(
                s(f(&result, "claim_ref"), "id"),
                Some("unrelated-selected-claim")
            );
            let explanation = &f(&result, "comparisons").as_array().unwrap()[0];
            assert_eq!(
                f(explanation, "source_binding"),
                &parse(&role_binding(role, value))
            );
            assert_eq!(
                f(explanation, "query_predicate"),
                &parse(&role_query(role, "10"))
            );
            assert_eq!(f(&result, "preserved_filter"), &q);
            assert_eq!(c, original);
            assert_eq!(q, query_original);
        }
    }
    let source = role_binding("AttributedKnowledge", "4");
    let q = filter(&role_query("AttributedKnowledge", "10"), "null");
    let mut refusals: Vec<(TemporalClaimCarrier, &str)> = vec![
        (carrier("", ""), "MissingRoleAssertion"),
        (
            carrier(&format!("{source},{source}"), ""),
            "MultipleRoleAssertions",
        ),
        (
            carrier(&source.replace("external-record", "different-record"), ""),
            "MissingRoleAssertion",
        ),
        (
            carrier(
                &source.replace(r#""id":"knower""#, r#""id":"different-knower""#),
                "",
            ),
            "MissingRoleAssertion",
        ),
        (
            carrier(&source.replace("\"version\":3", "\"version\":4"), ""),
            "MissingRoleAssertion",
        ),
        (
            carrier(
                &source.replace("tos_role_coordinate_assertion_v1", "unknown-profile"),
                "",
            ),
            "UnsupportedRoleAssertionProfile",
        ),
        (
            carrier(&source.replace("\"value\":4", "\"value\":4.0"), ""),
            "ExactI64CoordinateRequired",
        ),
        (
            carrier(&source.replace("\"value\":4", "\"value\":null"), ""),
            "ExactI64CoordinateRequired",
        ),
        (
            carrier(
                &source.replace("\"resolution\":\"exact\"", "\"resolution\":\"year\""),
                "",
            ),
            "ExactCoordinateRequired",
        ),
        (
            carrier(
                &source.replace("\"value\":4", "\"value\":4,\"uncertainty\":null"),
                "",
            ),
            "UnsupportedProfileField",
        ),
    ];
    for (from, to, reason) in [
        ("axis-X", "axis-Y", "NoCrosswalk"),
        ("rules-1", "rules-2", "NoCrosswalk"),
        (
            "\"unit\":\"tick\"",
            "\"unit\":\"seconds\"",
            "IncompatibleScales",
        ),
        (
            "\"origin\":\"O\"",
            "\"origin\":\"other\"",
            "IncompatibleScales",
        ),
    ] {
        refusals.push((carrier(&source.replace(from, to), ""), reason));
    }
    for (c, reason) in refusals {
        let original = c.clone();
        let result = evaluate(Some(&c), &q);
        assert_eq!(s(&result, "outcome"), Some("Refused"));
        assert_eq!(s(&result, "reason"), Some(reason));
        assert_eq!(c, original);
    }
    assert_eq!(
        s(&evaluate(None, &q), "reason"),
        Some("MissingTemporalCarrier")
    );
    // A false first cutoff cannot hide a later missing/unsupported predicate.
    let combined = filter(
        &format!(
            "{},{}",
            role_query("AttributedKnowledge", "0"),
            role_query("SourcePublication", "10")
        ),
        "null",
    );
    assert_eq!(
        s(&evaluate(Some(&carrier(&source, "")), &combined), "outcome"),
        Some("Refused")
    );
    // The whole query is validated before an empty/missing source is examined.
    let bad = filter(
        &format!(
            "{},{}",
            role_query("AttributedKnowledge", "0"),
            role_query("SourcePublication", "4.0")
        ),
        "null",
    );
    for query in [
        bad,
        filter("", "null"),
        filter(
            &role_query("AttributedKnowledge", "10")
                .replace("\"about_ref\"", "\"missing_about_ref\""),
            "null",
        ),
        filter(
            &role_query("SourcePublication", "10").replace("LessOrEqual", "UnknownOperator"),
            "null",
        ),
    ] {
        assert!(matches!(
            validate_claim_coordinate_filter(&query, budget),
            Err(TemporalError::MalformedRequest(_))
        ));
        assert!(matches!(
            evaluate_claim_coordinate_filter(None, &query, &claim_ref, revision, budget),
            Err(TemporalError::MalformedRequest(_))
        ));
    }
    let world_query = |value: &str| {
        format!(
            r#"{{"profile_ref":"tos_world_coordinate_at_filter_v1","about_ref":{},"at":{},"basis":{}}}"#,
            reference("external-record"),
            coordinate(value),
            basis()
        )
    };
    let world_source = |beginning: &str, end: &str, left: &str, right: &str| {
        format!(
            r#"{{"role":"ActualInterval","assertion":{{"profile_ref":"tos_world_coordinate_extent_v1","about_ref":{},"extent":{{"type":"Interval","beginning":{{"position":{},"inclusivity":"{left}"}},"end":{{"position":{},"inclusivity":"{right}"}}}}}},"basis":{}}}"#,
            reference("external-record"),
            coordinate(beginning),
            coordinate(end),
            basis()
        )
    };
    for (left, right, at, expected) in [
        ("Inclusive", "Exclusive", "4", "Match"),
        ("Exclusive", "Inclusive", "4", "NoMatch"),
        ("Inclusive", "Inclusive", "10", "Match"),
        ("Inclusive", "Exclusive", "10", "NoMatch"),
        ("Exclusive", "Exclusive", "7", "Match"),
        ("Inclusive", "Inclusive", "11", "NoMatch"),
    ] {
        let c = carrier(&world_source("4", "10", left, right), "");
        assert_eq!(
            s(
                &evaluate(Some(&c), &filter("", &world_query(at))),
                "outcome"
            ),
            Some(expected)
        );
    }
    let world = world_source("4", "10", "Inclusive", "Exclusive");
    let wq = filter("", &world_query("7"));
    for (c, reason) in [
        (
            carrier(&world_source("10", "4", "Inclusive", "Exclusive"), ""),
            "NonProperWorldInterval",
        ),
        (
            carrier(&world_source("4", "4", "Inclusive", "Exclusive"), ""),
            "NonProperWorldInterval",
        ),
        (
            carrier(&world_source("4", "10", "Unspecified", "Exclusive"), ""),
            "ExplicitInclusivityRequired",
        ),
        (
            carrier(&format!("{world},{world}"), ""),
            "MultipleWorldAssertions",
        ),
        (
            carrier(
                &world,
                &format!(r#", "validity":{{"type":"Unknown","basis":{}}}"#, basis()),
            ),
            "PendingValidityReconciliation",
        ),
    ] {
        let result = evaluate(Some(&c), &wq);
        assert_eq!(s(&result, "outcome"), Some("Refused"));
        assert_eq!(s(&result, "reason"), Some(reason));
    }
    let instant = format!(
        r#"{{"role":"WorldOccurrence","assertion":{{"profile_ref":"tos_world_coordinate_extent_v1","about_ref":{},"extent":{{"type":"Instant","position":{}}}}},"basis":{}}}"#,
        reference("external-record"),
        coordinate("7"),
        basis()
    );
    let c = carrier(&instant, "");
    for (at, outcome) in [("7", "Match"), ("8", "NoMatch")] {
        assert_eq!(
            s(
                &evaluate(Some(&c), &filter("", &world_query(at))),
                "outcome"
            ),
            Some(outcome)
        );
    }
    let c = carrier(&source, "");
    for limits in [
        TemporalBudget {
            max_pairings: 0,
            ..budget
        },
        TemporalBudget {
            max_expression_bytes: 32,
            ..budget
        },
        TemporalBudget {
            max_members: 1,
            ..budget
        },
    ] {
        assert!(matches!(
            evaluate_claim_coordinate_filter(Some(&c), &q, &claim_ref, revision, limits),
            Err(TemporalError::ResourceExhausted(_))
        ));
    }
    // Possible civil-year occurrence windows: same family, no list/authority.
    // Explicitly user-approved Possible/source/calendar/TRS semantics.
    // Original oracle unchanged; no list or authorization grant.
    let year_position = |year: &str, key: &str| {
        format!(
            r#"{{"type":"CalendarPosition","lexical":"{year}","resolution":"year","trs":{{"id":"{key}","version":"axis-v7"}},"calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical","rules_version":"civil-r7"}}"#
        )
    };
    let year_query = |position: &str| {
        parse(&format!(
            r#"{{"profile_ref":"tos_world_civil_year_window_filter_v1","assertion_scope":"SupplementalCarrierOnly","assertion_selection":"ExactlyOnePerPredicate","non_total":"RefusePage","predicate":"Possible","coverage_rule":"GregorianCivilYearHalfOpenV1","about_ref":{},"position":{position},"basis":{}}}"#,
            reference("external-record"),
            basis()
        ))
    };
    let year_assertion = |position: &str| {
        format!(
            r#"{{"profile_ref":"tos_world_civil_year_occurrence_v1","category":"OccurrencePossibleTimeBucket","coverage_rule":"GregorianCivilYearHalfOpenV1","about_ref":{},"position":{position}}}"#,
            reference("external-record")
        )
    };
    let year_role = |assertion: &str| {
        format!(
            r#"{{"role":"WorldOccurrence","assertion":{assertion},"basis":{}}}"#,
            basis()
        )
    };
    let year_budget = TemporalBudget {
        max_members: 8192,
        ..budget
    };
    for key in ["opaque-year-A", "renamed-civil-axis"] {
        let rules = parse(&format!(
            r#"{{"temporal_systems":{{"{key}":{{"calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical","rules_version":"civil-r7","trs_version":"axis-v7"}}}}}}"#
        ));
        let q = year_query(&year_position("1945", key));
        validate_claim_world_window_filter(&q, &rules, year_budget).unwrap();
        for (year, outcome, classification) in [
            ("1945", "Match", "All"),
            ("1946", "NoMatch", "Disjoint"),
            ("1944", "NoMatch", "Disjoint"),
            ("0000", "NoMatch", "Disjoint"),
        ] {
            let c = carrier(&year_role(&year_assertion(&year_position(year, key))), "");
            let original = c.original_packet().clone();
            let r = evaluate_claim_world_window_filter(
                Some(&c),
                &q,
                &rules,
                &claim_ref,
                revision,
                year_budget,
            )
            .unwrap();
            assert_eq!(s(&r, "outcome"), Some(outcome));
            assert_eq!(s(&r, "classification"), Some(classification));
            assert_eq!(f(&r, "event_extent_inferred"), &JsonValue::Bool(false));
            assert_eq!(f(&r, "preserved_filter"), &q);
            assert_eq!(c.original_packet(), &original);
            assert_eq!(
                f(f(&r, "source_assertion"), "position"),
                &parse(&year_position(year, key))
            );
            assert_eq!(s(&r, "legacy_object_assessment"), Some("NotExamined"));
        }
        let a = year_assertion(&year_position("1945", key));
        let source_negatives = [
            a.replace("OccurrencePossibleTimeBucket","ActualDuration"),
            a.replace(r#""resolution":"year""#,r#""resolution":"Unknown""#),
            a.replace("axis-v7","axis-v8"),
            a.replace("civil-r7","civil-r8"),
            a.replace("1945","9999"),
            a.replace(r#""category":"OccurrencePossibleTimeBucket""#,r#""category":"OccurrencePossibleTimeBucket","alternative_dependence":"Independent""#),
            a.replace(r#""category":"OccurrencePossibleTimeBucket""#,r#""category":"OccurrencePossibleTimeBucket","correlation_ref":"shared-k""#),
            a.replace(r#""position":{"#,r#""position":{"uncertainty":null,"#),
            a.replace(r#""position":{"#,r#""position":{"admitted":true,"#),
        ];
        for bad in source_negatives {
            let c = carrier(&year_role(&bad), "");
            let r = evaluate_claim_world_window_filter(
                Some(&c),
                &q,
                &rules,
                &claim_ref,
                revision,
                year_budget,
            )
            .unwrap();
            assert_eq!(s(&r, "outcome"), Some("Refused"));
            assert!(r.object_get("classification").is_none());
        }
        let good_role = year_role(&a);
        let duplicate = carrier(&format!("{good_role},{good_role}"), "");
        let nonmatching = year_role(&year_assertion(&year_position("1946", key)));
        let mixed = carrier(
            &format!(
                "{nonmatching},{}",
                year_role(&a.replace("OccurrencePossibleTimeBucket", "ActualDuration"))
            ),
            "",
        );
        for c in [duplicate, mixed] {
            let r = evaluate_claim_world_window_filter(
                Some(&c),
                &q,
                &rules,
                &claim_ref,
                revision,
                year_budget,
            )
            .unwrap();
            assert_eq!(s(&r, "outcome"), Some("Refused"));
        }
        let typed_beginning=year_position("2020",key).replace(r#","calendar_algorithm":"GregorianProleptic","year_numbering":"astronomical","rules_version":"civil-r7""#, "");
        for validity in [
            format!(r#", "validity":{{"type":"Unknown","basis":{}}}"#, basis()),
            format!(
                r#", "validity":{{"type":"Interval","beginning":{typed_beginning},"end":{{"type":"Unknown"}},"basis":{}}}"#,
                basis()
            ),
            format!(
                r#", "validity":{{"type":"RightCensored","observed_through":{typed_beginning},"cutoff_basis":{}}}"#,
                basis()
            ),
        ] {
            let c = carrier(&good_role, &validity);
            let r = evaluate_claim_world_window_filter(
                Some(&c),
                &q,
                &rules,
                &claim_ref,
                revision,
                year_budget,
            )
            .unwrap();
            assert_eq!(s(&r, "reason"), Some("PendingValidityReconciliation"));
        }
        for bad_query in [
            year_query(&year_position("9999", key)),
            year_query(&year_position("1945", key).replace("axis-v7", "wrong")),
            year_query(&year_position("1945", key).replace("astronomical", "historical")),
            parse(
                &tos_foundation::emit_value_preserved_json(
                    &q,
                    tos_foundation::JsonLimits::default(),
                )
                .map(|b| String::from_utf8(b).unwrap())
                .unwrap()
                .replace("Possible", "Guaranteed"),
            ),
        ] {
            assert!(matches!(
                validate_claim_world_window_filter(&bad_query, &rules, year_budget),
                Err(TemporalError::MalformedRequest(_))
            ));
            assert!(matches!(
                evaluate_claim_world_window_filter(
                    None,
                    &bad_query,
                    &rules,
                    &claim_ref,
                    revision,
                    year_budget
                ),
                Err(TemporalError::MalformedRequest(_))
            ));
        }
        assert!(matches!(
            validate_claim_world_window_filter(&q, &parse("{}"), year_budget),
            Err(TemporalError::MalformedRequest(_))
        ));
        let c = carrier(&good_role, "");
        for limited in [
            TemporalBudget {
                max_pairings: 0,
                ..year_budget
            },
            TemporalBudget {
                max_members: 1,
                ..year_budget
            },
            TemporalBudget {
                max_expression_bytes: 32,
                ..year_budget
            },
        ] {
            assert!(matches!(
                evaluate_claim_world_window_filter(
                    Some(&c),
                    &q,
                    &rules,
                    &claim_ref,
                    revision,
                    limited
                ),
                Err(TemporalError::ResourceExhausted(_))
            ));
        }
    }
}
