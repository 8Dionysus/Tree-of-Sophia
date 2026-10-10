//! Native callback codec and retained form reconstruction over an independent
//! maintained Python packet. This does not issue source or command admission.
use serde_json::{Value, json};
use tos_command::source_forms_compiler::{
    materialize_compiler_forms, reconstruct_compiler_revision_forms,
};

fn value(raw: &[u8]) -> Value {
    serde_json::from_slice(raw).unwrap()
}

#[test]
fn native_callbacks_match_retained_python_forms_and_preserve_large_integer_context() {
    let mut source = value(include_bytes!(
        "fixtures/source_forms_shadow/claim_v1/source.json"
    ));
    let prior = value(include_bytes!(
        "fixtures/source_forms_shadow/claim_v1/initial.json"
    ));
    let published = value(include_bytes!(
        "fixtures/source_forms_shadow/claim_v1/published.json"
    ));
    let applied = value(include_bytes!(
        "fixtures/source_forms_shadow/claim_v1/apply.json"
    ));
    let owner = value(include_bytes!(
        "fixtures/source_forms_shadow/claim_v1/owner.json"
    ));
    let prepared = value(include_bytes!(
        "fixtures/source_forms_shadow/claim_v1/prepare-claim-statement.request.json"
    ));
    let selections = json!([{
        "form_id": prepared["form_id"], "field_id": prepared["field_id"]
    }]);
    let successor = reconstruct_compiler_revision_forms(
        &source,
        Some(&prior),
        owner["principal_id"].as_str().unwrap(),
        &selections,
        262_144,
    )
    .unwrap();
    assert_eq!(successor["forms"], published["forms"]);
    assert_eq!(successor["form_history"], published["form_history"]);
    let forms = materialize_compiler_forms(&source, &published, 262_144).unwrap();
    assert_eq!(Value::Array(forms), applied["materializations"]);
    assert!(
        reconstruct_compiler_revision_forms(
            &source,
            Some(&prior),
            owner["principal_id"].as_str().unwrap(),
            &selections,
            1,
        )
        .is_err()
    );

    // The whole Claim source is a mandatory context. Transporting it through
    // serde/foundation must preserve integers beyond the u64 boundary.
    let integer = "184467440737095516170";
    source["qualifiers"]["transport_integer"] = serde_json::from_str(integer).unwrap();
    assert_eq!(
        source["qualifiers"]["transport_integer"].to_string(),
        integer
    );
    let set = reconstruct_compiler_revision_forms(
        &source,
        None,
        owner["principal_id"].as_str().unwrap(),
        &selections,
        262_144,
    )
    .unwrap();
    let forms = materialize_compiler_forms(&source, &set, 262_144).unwrap();
    assert_eq!(forms.len(), 1);
    assert_eq!(
        forms[0]["context"][0]["value"]["qualifiers"]["transport_integer"].to_string(),
        integer
    );
    assert_eq!(forms[0]["admission"], Value::Null);
    assert_eq!(forms[0]["performs_semantic_assessment"], false);
}
