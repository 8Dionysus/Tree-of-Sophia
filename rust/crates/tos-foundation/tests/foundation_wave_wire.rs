//! Independent pure-wire acceptance; persistence and auth are tested by their owners.
use tos_foundation::temporal::{TemporalBudget, TemporalError, execute_temporal_operation};
use tos_foundation::{JsonLimits, JsonMode, JsonString, JsonValue, parse_json};

fn parse(raw: &str) -> JsonValue {
    parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap()
    .into_root()
}
fn get<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue {
    v.object_get(key).unwrap()
}
fn text<'a>(v: &'a JsonValue, key: &str) -> Option<&'a str> {
    v.object_get(key).and_then(JsonValue::as_str)
}
fn put(v: &mut JsonValue, key: &str, value: JsonValue) {
    let JsonValue::Object(fields) = v else {
        panic!("object required")
    };
    if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *old = value;
    } else {
        fields.push((JsonString::from_utf8(key), value));
    }
}
fn call(op: &str, input: &JsonValue) -> Result<JsonValue, TemporalError> {
    execute_temporal_operation(op, input, &parse("{}"), TemporalBudget::default())
}
fn frozen_input() -> JsonValue {
    parse(
        r#"{"request_envelope":{"source_snapshot_ref":{"snapshot_id":"snap-X","revision_fence":"fence-17"},"temporal":{"chronology_version":"chronology-X"}},"snapshot_manifest":{"chronology_version_id":"chronology-X","frozen_knowledge_selection":{"system_state":{"type":"SystemAsOf","cutoff":"2026-10-19T05:00:00Z"},"attributed_knowledge_as_of":{"knower":"school-X","cutoff":"1800","trs_ref":"calendar@v1"},"source_publication_as_of":{"cutoff":"1900","trs_ref":"calendar@v1"},"source_snapshot_ref":{"snapshot_id":"snap-X"},"source_revision_fence":"fence-17","evidence_policy_version":"evidence-X","selection_policy_version":"selection-X","member_claim_revisions":["claim-X@r2"]}}}"#,
    )
}
fn request_mut(input: &mut JsonValue) -> &mut JsonValue {
    let JsonValue::Object(top) = input else {
        panic!()
    };
    let env = &mut top
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("request_envelope"))
        .unwrap()
        .1;
    let JsonValue::Object(env) = env else {
        panic!()
    };
    &mut env
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("temporal"))
        .unwrap()
        .1
}
fn snapshot_mut(input: &mut JsonValue) -> &mut JsonValue {
    let JsonValue::Object(top) = input else {
        panic!()
    };
    let env = &mut top
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("request_envelope"))
        .unwrap()
        .1;
    let JsonValue::Object(env) = env else {
        panic!()
    };
    &mut env
        .iter_mut()
        .find(|(k, _)| k.as_str() == Some("source_snapshot_ref"))
        .unwrap()
        .1
}

#[test]
fn missing_source_snapshot_or_fence_is_a_system_error() {
    for raw in [
        r#"{}"#,
        r#"{"request_envelope":{"source_snapshot_ref":{},"temporal":{"system_state":"Current"}}}"#,
        r#"{"request_envelope":{"source_snapshot_ref":{"snapshot_id":"X"},"temporal":{"system_state":"Current"}}}"#,
        r#"{"request_envelope":{"source_snapshot_ref":{"revision_fence":"r7"},"temporal":{"system_state":"Current"}}}"#,
    ] {
        assert!(matches!(
            call("resolve_temporal_query_context", &parse(raw)),
            Err(TemporalError::MalformedRequest(_))
        ));
    }
}

#[test]
fn current_freeze_preserves_symbolic_refs_and_omitted_trs_without_persisting() {
    let input = parse(
        r#"{"source_snapshot_ref":{"snapshot_id":"symbolic-X","revision_fence":"symbolic-fence","resolved_system_cutoff":"1037"},"selection_request":{"system_state":"Current"},"claim_revisions":["symbolic-claim@r7"],"evidence_policy_version":"evidence-3","selection_policy_version":"selection-11"}"#,
    );
    let actual = call("create_chronology_version", &input).unwrap();
    assert_eq!(text(&actual, "status"), Some("ChronologyVersionCreated"));
    assert_eq!(get(&actual, "persisted").as_bool(), Some(false));
    let selection = get(&actual, "frozen_knowledge_selection");
    assert_eq!(text(get(selection, "system_state"), "cutoff"), Some("1037"));
    assert!(
        get(selection, "system_state")
            .object_get("trs_ref")
            .is_none()
    );
    assert_eq!(
        text(get(selection, "source_snapshot_ref"), "snapshot_id"),
        Some("symbolic-X")
    );
    assert_eq!(
        text(selection, "source_revision_fence"),
        Some("symbolic-fence")
    );
    assert_eq!(
        get(selection, "member_claim_revisions"),
        get(&input, "claim_revisions")
    );
    assert_eq!(
        text(selection, "evidence_policy_version"),
        Some("evidence-3")
    );
    assert_eq!(
        text(selection, "selection_policy_version"),
        Some("selection-11")
    );
}

#[test]
fn snapshot_and_fence_are_independent_reference_dimensions() {
    for (field, value, expected) in [
        ("snapshot_id", "another-snapshot", "source_snapshot_ref"),
        ("revision_fence", "another-fence", "source_revision_fence"),
    ] {
        let mut input = frozen_input();
        put(
            snapshot_mut(&mut input),
            field,
            parse(&format!("\"{value}\"")),
        );
        let actual = call("resolve_temporal_query_context", &input).unwrap();
        assert_eq!(text(&actual, "status"), Some("ContextMismatch"));
        assert_eq!(
            get(&actual, "mismatched_dimensions"),
            &parse(&format!("[\"{expected}\"]"))
        );
    }
}

#[test]
fn selector_object_order_is_semantically_irrelevant_but_values_and_knowers_are_not() {
    let mut input = frozen_input();
    put(
        request_mut(&mut input),
        "attributed_knowledge_as_of",
        parse(r#"{"trs_ref":"calendar@v1","cutoff":"1800","knower":"school-X"}"#),
    );
    put(
        request_mut(&mut input),
        "source_publication_as_of",
        parse(r#"{"trs_ref":"calendar@v1","cutoff":"1900"}"#),
    );
    assert_eq!(
        text(
            &call("resolve_temporal_query_context", &input).unwrap(),
            "status"
        ),
        Some("Resolved")
    );
    for changed in [
        r#"{"knower":"school-Y","cutoff":"1800","trs_ref":"calendar@v1"}"#,
        r#"{"knower":"school-X","cutoff":"1801","trs_ref":"calendar@v1"}"#,
    ] {
        put(
            request_mut(&mut input),
            "attributed_knowledge_as_of",
            parse(changed),
        );
        let actual = call("resolve_temporal_query_context", &input).unwrap();
        assert_eq!(
            get(&actual, "mismatched_dimensions"),
            &parse(r#"["attributed_knowledge_as_of"]"#)
        );
    }
}

#[test]
fn omitted_selectors_inherit_and_world_filter_does_not_change_frozen_members() {
    let mut input = frozen_input();
    let before = input.clone();
    put(
        request_mut(&mut input),
        "world_valid_time",
        parse(r#"{"position":"1945","trs_ref":"calendar@v1"}"#),
    );
    let actual = call("resolve_temporal_query_context", &input).unwrap();
    assert_eq!(
        get(&actual, "effective_knowledge_selection"),
        get(
            get(&before, "snapshot_manifest"),
            "frozen_knowledge_selection"
        )
    );
    assert_eq!(
        get(&actual, "world_valid_time"),
        &parse(r#"{"position":"1945","trs_ref":"calendar@v1"}"#)
    );
}

#[test]
fn attributed_selection_requires_the_named_knower() {
    let input = parse(
        r#"{"request_envelope":{"source_snapshot_ref":{"snapshot_id":"X","revision_fence":"r7","resolved_system_cutoff":"2026"},"temporal":{"system_state":"Current","attributed_knowledge_as_of":{"cutoff":"1800"}}}}"#,
    );
    assert!(matches!(
        call("resolve_temporal_query_context", &input),
        Err(TemporalError::MalformedRequest(_))
    ));
}

#[test]
fn system_cutoff_and_trs_object_order_preserves_exact_values() {
    use tos_foundation::query_context_wire::resolve_context_json;

    let mut input = frozen_input();
    let mut manifest = get(&input, "snapshot_manifest").clone();
    let mut frozen = get(&manifest, "frozen_knowledge_selection").clone();
    put(
        &mut frozen,
        "system_state",
        parse(
            r#"{"type":"SystemAsOf","cutoff":{"day":[1,2],"year":1886},"trs_ref":{"version":"rules-1","id":"calendar-A"}}"#,
        ),
    );
    put(&mut manifest, "frozen_knowledge_selection", frozen.clone());
    put(&mut input, "snapshot_manifest", manifest);
    for (state, matches) in [
        (
            parse(
                r#"{"type":"SystemAsOf","cutoff":{"year":1886,"day":[1,2]},"trs_ref":{"id":"calendar-A","version":"rules-1"}}"#,
            ),
            true,
        ),
        (
            parse(
                r#"{"type":"SystemAsOf","cutoff":{"year":1887,"day":[1,2]},"trs_ref":{"id":"calendar-A","version":"rules-1"}}"#,
            ),
            false,
        ),
        (
            parse(
                r#"{"type":"SystemAsOf","cutoff":{"year":1886,"day":[2,1]},"trs_ref":{"id":"calendar-A","version":"rules-1"}}"#,
            ),
            false,
        ),
        (
            parse(
                r#"{"type":"SystemAsOf","cutoff":{"year":1886,"day":[1,2]},"trs_ref":{"id":"calendar-A","version":"rules-2"}}"#,
            ),
            false,
        ),
        (
            parse(r#"{"type":"SystemAsOf","cutoff":{"year":1886,"day":[1,2]}}"#),
            true,
        ),
        (
            parse(
                r#"{"type":"SystemAsOf","cutoff":{"year":1886.0,"day":[1,2]},"trs_ref":{"id":"calendar-A","version":"rules-1"}}"#,
            ),
            false,
        ),
    ] {
        put(request_mut(&mut input), "system_state", state);
        let before = input.clone();
        for direct in [false, true] {
            let actual = if direct {
                resolve_context_json(&input, &parse("{}"), TemporalBudget::default()).unwrap()
            } else {
                call("resolve_temporal_query_context", &input).unwrap()
            };
            if matches {
                assert_eq!(text(&actual, "status"), Some("Resolved"));
                assert_eq!(get(&actual, "effective_knowledge_selection"), &frozen);
            } else {
                assert_eq!(text(&actual, "status"), Some("ContextMismatch"));
                assert_eq!(
                    get(&actual, "mismatched_dimensions"),
                    &parse(r#"["system_state"]"#)
                );
                assert_eq!(get(&actual, "result"), &JsonValue::Null);
            }
            assert_eq!(input, before);
        }
    }
}
