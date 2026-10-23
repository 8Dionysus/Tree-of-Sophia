use tos_foundation::query_context_wire::{create_chronology_json, resolve_context_json};
use tos_foundation::temporal::codecs::capture_tei;
use tos_foundation::temporal::difference::execute_difference;
use tos_foundation::temporal::{TemporalBudget, TemporalError, execute_temporal_operation};
use tos_foundation::{JsonLimits, JsonMode, JsonString, JsonValue, parse_json};

type Direct = fn(&JsonValue, &JsonValue, TemporalBudget) -> Result<JsonValue, TemporalError>;

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

fn routes() -> [(&'static str, Direct, JsonValue, &'static str); 4] {
    [
        (
            "capture_expression",
            capture_tei,
            parse(
                r#"{"xml_fragment":"<date when='1962-10'/>","tei_version":"P5-4.12.0","calendar":"Gregorian"}"#,
            ),
            "Complete",
        ),
        (
            "difference",
            execute_difference,
            parse(
                r#"{"beginning":{"type":"CoordinatePosition","value":4,"axis":{"trs":{"id":"X","version":"rules1"},"unit":"seconds","origin":"O"}},"end":{"type":"CoordinatePosition","value":10,"axis":{"trs":{"id":"X","version":"rules1"},"unit":"seconds","origin":"O"}},"duration_kind":"Elapsed"}"#,
            ),
            "Resolved",
        ),
        (
            "resolve_temporal_query_context",
            resolve_context_json,
            parse(
                r#"{"request_envelope":{"source_snapshot_ref":{"snapshot_id":"snapshot-A","revision_fence":"revision-A","resolved_system_cutoff":7},"temporal":{"system_state":"Current","world_valid_time":"opaque-world-time"}}}"#,
            ),
            "Resolved",
        ),
        (
            "create_chronology_version",
            create_chronology_json,
            parse(
                r#"{"source_snapshot_ref":{"snapshot_id":"snapshot-A","revision_fence":"revision-A","resolved_system_cutoff":7},"selection_request":{"system_state":"Current"},"claim_revisions":[{"id":"claim-A","version":1,"digest":"digest-A"}],"evidence_policy_version":"evidence-A","selection_policy_version":"selection-A"}"#,
            ),
            "ChronologyVersionCreated",
        ),
    ]
}

fn put(value: &mut JsonValue, name: &str, item: JsonValue) {
    let JsonValue::Object(fields) = value else {
        panic!("object fixture required");
    };
    fields.push((JsonString::from_utf8(name), item));
}

fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

#[test]
fn direct_codecs_difference_and_context_bound_both_complete_roots() {
    for (operation, direct, input, _) in routes() {
        for in_context in [false, true] {
            let mut i = input.clone();
            let mut c = parse("{}");
            put(
                if in_context { &mut c } else { &mut i },
                "unused",
                text(&"x".repeat(2048)),
            );
            let budget = TemporalBudget {
                max_expression_bytes: 1024,
                ..TemporalBudget::default()
            };
            assert!(matches!(
                direct(&i, &c, budget),
                Err(TemporalError::ResourceExhausted(_))
            ));
            assert!(matches!(
                execute_temporal_operation(operation, &i, &c, budget),
                Err(TemporalError::ResourceExhausted(_))
            ));

            let mut i = input.clone();
            let mut c = parse("{}");
            put(
                if in_context { &mut c } else { &mut i },
                "unused",
                JsonValue::Array(vec![JsonValue::Null; 100]),
            );
            let budget = TemporalBudget {
                max_members: 64,
                ..TemporalBudget::default()
            };
            assert!(matches!(
                direct(&i, &c, budget),
                Err(TemporalError::ResourceExhausted(_))
            ));
            assert!(matches!(
                execute_temporal_operation(operation, &i, &c, budget),
                Err(TemporalError::ResourceExhausted(_))
            ));

            let mut deep = JsonValue::Null;
            for _ in 0..65 {
                deep = JsonValue::Array(vec![deep]);
            }
            let mut i = input.clone();
            let mut c = parse("{}");
            put(if in_context { &mut c } else { &mut i }, "unused", deep);
            assert!(matches!(
                direct(&i, &c, TemporalBudget::default()),
                Err(TemporalError::ResourceExhausted(_))
            ));
            assert!(matches!(
                execute_temporal_operation(operation, &i, &c, TemporalBudget::default()),
                Err(TemporalError::ResourceExhausted(_))
            ));
        }
    }
}

#[test]
fn direct_codecs_difference_and_context_reject_nested_native_duplicates() {
    for (operation, direct, input, _) in routes() {
        for in_context in [false, true] {
            let duplicate = JsonValue::Object(vec![
                (JsonString::from_utf8("same"), JsonValue::Null),
                (JsonString::from_utf8("same"), JsonValue::Bool(true)),
            ]);
            let mut i = input.clone();
            let mut c = parse("{}");
            put(
                if in_context { &mut c } else { &mut i },
                "unused",
                duplicate,
            );
            assert!(matches!(
                direct(&i, &c, TemporalBudget::default()),
                Err(TemporalError::MalformedRequest(_))
            ));
            assert!(matches!(
                execute_temporal_operation(operation, &i, &c, TemporalBudget::default()),
                Err(TemporalError::MalformedRequest(_))
            ));
        }
    }
}

#[test]
fn direct_codecs_difference_and_context_preserve_bounded_semantics() {
    for (operation, direct, input, expected_status) in routes() {
        let context = parse("{}");
        let before = (input.clone(), context.clone());
        let result = direct(&input, &context, TemporalBudget::default()).unwrap();
        assert_eq!(
            result.object_get("status").and_then(JsonValue::as_str),
            Some(expected_status)
        );
        let top =
            execute_temporal_operation(operation, &input, &context, TemporalBudget::default())
                .unwrap();
        for (key, value) in result.as_object().unwrap() {
            assert_eq!(top.object_get(key.as_str().unwrap()), Some(value));
        }
        assert_eq!((input, context), before);
    }
}
