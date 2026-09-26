// Exercises source mechanics with the maintained Claim fixture, independent of
// production admission. The real schema worker remains owned by OPS integration.
#[path = "../src/source_claims.rs"]
mod source_claims;
#[path = "../src/source_command.rs"]
mod source_command;
#[path = "../src/source_forms.rs"]
mod source_forms;
use source_claims::{advance_claim, replace_claim_row};
use source_command::{canonical, field, integer, object, parse, string, text};

const SOURCE: &[u8] = include_bytes!("fixtures/source_forms_shadow/claim_v1/source.json");

#[test]
fn successor_preserves_full_qualifier_context_and_sibling_bytes() {
    let source = parse(SOURCE).unwrap();
    let patch = object(vec![(
        "qualifiers",
        object(vec![("statement", string("Revised source wording."))]),
    )]);
    let revised = advance_claim(&source, &patch, None).unwrap();
    assert_eq!(
        integer(&revised, "claim_version").unwrap(),
        integer(&source, "claim_version").unwrap() + 1
    );
    for (key, value) in field(&source, "qualifiers").unwrap().as_object().unwrap() {
        if key.as_str() != Some("statement") {
            assert_eq!(
                field(
                    field(&revised, "qualifiers").unwrap(),
                    key.as_str().unwrap()
                )
                .unwrap(),
                value
            );
        }
    }
    let mut original = b" \r\n".to_vec();
    original.extend(canonical(&source).unwrap());
    original.extend(b"\r\n\t\n");
    let replacement = replace_claim_row(&original, &revised).unwrap();
    let mut expected = b" \r\n".to_vec();
    expected.extend(canonical(&revised).unwrap());
    expected.extend(b"\r\n\t\n");
    assert_eq!(replacement, expected);
    assert_eq!(
        text(&source, "claim_id").unwrap(),
        text(&revised, "claim_id").unwrap()
    );
}
#[test]
fn immutable_claim_fields_and_endpoint_kind_are_refused() {
    let source = parse(SOURCE).unwrap();
    for key in [
        "claim_id",
        "subject_ref",
        "maker",
        "review_status",
        "claim_version",
    ] {
        assert!(advance_claim(&source, &object(vec![(key, string("changed"))]), None).is_err());
    }
    assert!(advance_claim(&source, &object(vec![("object", object(vec![]))]), None).is_err());
    assert!(advance_claim(&source, &object(vec![]), None).is_err());
}
#[test]
fn layer_transition_requires_exact_predecessor_and_keeps_qualifiers() {
    let source = parse(SOURCE).unwrap();
    let before = text(&source, "assertion_layer").unwrap();
    let transition = object(vec![
        ("from", string(before)),
        ("to", string("scholarly_report")),
    ]);
    if before != "scholarly_report" {
        let revised = advance_claim(
            &source,
            &object(vec![("assertion_layer", string("scholarly_report"))]),
            Some(&transition),
        )
        .unwrap();
        assert_eq!(
            field(&source, "qualifiers").unwrap(),
            field(&revised, "qualifiers").unwrap()
        );
    }
    let bad = object(vec![
        ("from", string("unrelated_layer")),
        ("to", string("scholarly_report")),
    ]);
    assert!(
        advance_claim(
            &source,
            &object(vec![("assertion_layer", string("scholarly_report"))]),
            Some(&bad)
        )
        .is_err()
    );
}
#[test]
fn repeated_id_is_not_silently_replaced() {
    let source = parse(SOURCE).unwrap();
    let mut raw = canonical(&source).unwrap();
    raw.push(b'\n');
    raw.extend(canonical(&source).unwrap());
    assert!(replace_claim_row(&raw, &source).is_err());
}
