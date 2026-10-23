//! Synthetic structural vectors only; no real source, grant or completeness issuer.
use tos_foundation::{
    BindingError, CompositionLink, DependencyComparison, JsonLimits, JsonMode,
    JsonString, JsonValue, OperationBindingLimits, ValueDisposition,
    check_composition, check_operation_binding, compare_dependency_bindings, parse_json,
};

fn limits() -> OperationBindingLimits {
    OperationBindingLimits { max_envelope_bytes: 1_000_000, max_nodes: 1_000_000,
        max_depth: 64, max_refs: 1_000 }
}
fn json(s: &str) -> JsonValue {
    parse_json(s.as_bytes(), JsonMode::PublishedStrict, JsonLimits::default()).unwrap().into_root()
}
fn string(s: &str) -> JsonValue { JsonValue::String(JsonString::from_utf8(s)) }
fn get<'a>(v: &'a JsonValue, key: &str) -> &'a JsonValue { v.object_get(key).unwrap() }
fn get_mut<'a>(v: &'a mut JsonValue, key: &str) -> &'a mut JsonValue {
    let JsonValue::Object(fields) = v else { panic!("fixture object") };
    &mut fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)).unwrap().1
}
fn put(v: &mut JsonValue, key: &str, value: JsonValue) { *get_mut(v, key) = value; }
fn reference(id: &str) -> JsonValue {
    json(&format!(r#"{{"id":"{id}","version":1,"digest":"sha256:{}"}}"#, "0".repeat(64)))
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(fields.into_iter().map(|(k, v)| (JsonString::from_utf8(k), v)).collect())
}
fn produced() -> JsonValue {
    object(vec![("kind", string("Produced")), ("output_ref", reference("fixture.output")),
        ("output_profile_ref", reference("fixture.profile"))])
}
fn fixture() -> JsonValue {
    object(vec![
        ("schema_version", string("tos_operation_binding_v1")), ("run_id", string("fixture.run")),
        ("operator_ref", reference("fixture.operator")), ("context_ref", reference("fixture.context")),
        ("selection_ref", reference("fixture.selection")),
        ("inputs", JsonValue::Array(vec![object(vec![("slot", string("in")),
            ("profile_ref", reference("fixture.profile")), ("record_ref", reference("fixture.input"))])])),
        ("dependency_manifest", object(vec![("manifest_ref", reference("fixture.manifest")),
            ("observable_ref", reference("fixture.observable")), ("coverage", string("Incomplete")),
            ("entries", JsonValue::Array(vec![object(vec![("slot", string("source")),
                ("role", string("Source")), ("ref", reference("fixture.input"))])]))])),
        ("value", produced()), ("execution", string("Completed")),
        ("applicability", object(vec![("kind", string("ApplicabilityUnresolved")),
            ("reason", string("synthetic; no source applicability assessment"))])),
        ("assessment", object(vec![("kind", string("NotRelevant"))])),
        ("certificates", JsonValue::Array(vec![])), ("warrant_refs", JsonValue::Array(vec![])),
        ("authority_boundary", string("StructuralBindingOnlyNoAdmissionOrProofVerification")),
    ])
}
fn partial() -> JsonValue {
    let mut value = produced(); put(&mut value, "kind", string("Partial"));
    let JsonValue::Object(fields) = &mut value else { unreachable!() };
    fields.push((JsonString::from_utf8("partial_schema_ref"), reference("fixture.partial-schema")));
    fields.push((JsonString::from_utf8("coverage_ref"), reference("fixture.partial-coverage")));
    value
}
fn certificate(e: &JsonValue) -> JsonValue {
    object(vec![("certificate_ref", reference("fixture.certificate")),
        ("binding_status", string("UnverifiedReference")), ("bound_run_id", get(e, "run_id").clone()),
        ("bound_operator_ref", get(e, "operator_ref").clone()), ("bound_input_refs", get(e, "inputs").clone()),
        ("bound_context_ref", get(e, "context_ref").clone()),
        ("bound_selection_ref", get(e, "selection_ref").clone()),
        ("bound_dependency_manifest_ref", get(get(e, "dependency_manifest"), "manifest_ref").clone()),
        ("bound_output_ref", get(get(e, "value"), "output_ref").clone())])
}
fn right_input(e: &mut JsonValue) -> &mut JsonValue {
    let JsonValue::Array(rows) = get_mut(e, "inputs") else { unreachable!() }; &mut rows[0]
}

#[test]
fn operation_binding_conformance() {
    // Risk 1: result/execution pairing and required partial schema.
    for (value, execution, expected) in [
        (produced(), "Completed", None),
        (produced(), "ResourceLimit", Some(BindingError::IncompatibleDisposition)),
        (partial(), "Running", None),
        (partial(), "Completed", None),
        (partial(), "NotStarted", Some(BindingError::IncompatibleDisposition)),
        (object(vec![("kind", string("NoCompleteValue")), ("reason", string("blocked"))]),
            "Completed", Some(BindingError::IncompatibleDisposition)),
        (object(vec![("kind", string("NoCompleteValue")), ("reason", string("blocked"))]),
            "BlockedByUpstream", None),
        (object(vec![("kind", string("Undefined")), ("reason", string("outside domain")),
            ("domain_evidence_ref", reference("fixture.domain"))]), "Completed", None),
    ] {
        let mut e = fixture(); put(&mut e, "value", value); put(&mut e, "execution", string(execution));
        match expected {
            Some(error) => assert_eq!(check_operation_binding(&e, limits()).unwrap_err(), error),
            None => { check_operation_binding(&e, limits()).unwrap(); }
        }
    }
    let mut missing = fixture(); let mut p = partial();
    let JsonValue::Object(fields) = &mut p else { unreachable!() };
    fields.retain(|(k, _)| k.as_str() != Some("partial_schema_ref")); put(&mut missing, "value", p);
    assert!(matches!(check_operation_binding(&missing, limits()), Err(BindingError::Malformed(_))));

    // Risk 2: exact intermediate/profile/context/selection, including ID labels
    // retained while only version/digest changes. Right run is not executed.
    let left = fixture(); let l = check_operation_binding(&left, limits()).unwrap();
    assert_eq!(l.value_disposition(), ValueDisposition::Produced);
    assert!(std::ptr::eq(l.as_json(), &left));
    let mut right = fixture(); put(right_input(&mut right), "record_ref", reference("fixture.output"));
    let r = check_operation_binding(&right, limits()).unwrap();
    check_composition(&l, &r, &CompositionLink::exact("in"), limits()).unwrap();
    for axis in ["version", "digest", "profile", "context", "selection"] {
        let mut wrong = right.clone();
        match axis {
            "version" => put(get_mut(right_input(&mut wrong), "record_ref"), "version", json("2")),
            "digest" => put(get_mut(right_input(&mut wrong), "record_ref"), "digest", string(&format!("sha256:{}", "1".repeat(64)))),
            "profile" => put(right_input(&mut wrong), "profile_ref", reference("fixture.other-profile")),
            "context" => put(&mut wrong, "context_ref", reference("fixture.other-context")),
            _ => put(&mut wrong, "selection_ref", reference("fixture.other-selection")),
        }
        let w = check_operation_binding(&wrong, limits()).unwrap();
        assert_eq!(check_composition(&l, &w, &CompositionLink::exact("in"), limits()), Err(BindingError::IncompatibleBoundary), "{axis}");
    }

    // Risk 3: equal self-declared complete pins are still unresolved. A changed
    // value-affecting pin is Changed; run/certificate IDs alone are not pins.
    let mut new = fixture(); put(&mut new, "run_id", string("fixture.new-run"));
    for coverage in ["Incomplete", "DeclaredCompleteForObservable"] {
        put(get_mut(&mut new, "dependency_manifest"), "coverage", string(coverage));
        let n = check_operation_binding(&new, limits()).unwrap();
        assert_eq!(compare_dependency_bindings(&l, &n, None, None, limits()).unwrap(), DependencyComparison::Unresolved);
    }
    put(get_mut(right_input(&mut new), "record_ref"), "version", json("2"));
    let n = check_operation_binding(&new, limits()).unwrap();
    assert_eq!(compare_dependency_bindings(&l, &n, None, None, limits()).unwrap(), DependencyComparison::Changed);
    let valid_certificate = certificate(&left);
    let mut certified = left.clone(); put(&mut certified, "certificates", JsonValue::Array(vec![valid_certificate.clone()]));
    check_operation_binding(&certified, limits()).unwrap();
    for axis in ["bound_run_id", "bound_operator_ref", "bound_context_ref", "bound_selection_ref", "bound_dependency_manifest_ref", "bound_output_ref"] {
        let mut c = valid_certificate.clone();
        let wrong = if axis == "bound_run_id" { string("fixture.foreign-run") }
            else if axis == "bound_output_ref" { JsonValue::Null } else { reference("fixture.foreign-ref") };
        put(&mut c, axis, wrong); put(&mut certified, "certificates", JsonValue::Array(vec![c]));
        assert_eq!(check_operation_binding(&certified, limits()).unwrap_err(), BindingError::CertificateBoundaryMismatch, "{axis}");
    }

    // Native construction can bypass the JSON parser: duplicate keys must still
    // fail, and direct helpers must not inherit a previous caller's budget.
    let mut duplicate = fixture(); let JsonValue::Object(fields) = &mut duplicate else { unreachable!() };
    fields.push((JsonString::from_utf8("run_id"), string("duplicate")));
    assert_eq!(check_operation_binding(&duplicate, limits()).unwrap_err(), BindingError::Malformed("duplicate object key"));
    for (axis, cap) in (0..4).flat_map(|axis| [0, 1].map(|cap| (axis, cap))) {
        let mut small = limits();
        match axis { 0 => small.max_envelope_bytes = cap, 1 => small.max_nodes = cap,
            2 => small.max_depth = cap, _ => small.max_refs = cap }
        assert_eq!(check_operation_binding(&left, small).unwrap_err(), BindingError::ResourceLimit);
        assert_eq!(check_composition(&l, &r, &CompositionLink::exact("in"), small), Err(BindingError::ResourceLimit));
        assert_eq!(compare_dependency_bindings(&l, &r, None, None, small), Err(BindingError::ResourceLimit));
    }
}
