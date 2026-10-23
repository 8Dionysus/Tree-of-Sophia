use tos_foundation::JsonValue;
use tos_foundation::temporal::network;
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
fn field<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn status(v: &JsonValue) -> &str {
    field(v, "status").as_str().unwrap()
}
fn request(nodes: &str, constraints: &str, steps: usize) -> JsonValue {
    parse(&format!(
        r#"{{"profile":"ExactComparableEndpointIntervalsV1","nodes":{nodes},"constraints":{constraints},"resource_budget":{{"max_nodes":256,"max_constraints":4096,"max_solver_steps":{steps}}},"endpoint_axis":"explicit-comparable-order@r7","node_shape":"finite-proper-interval","endpoint_domain":"exact-strict-weak-order"}}"#
    ))
}
fn run(v: &JsonValue) -> JsonValue {
    network::execute_network(v, &parse("{}")).unwrap()
}
fn endpoint(v: &JsonValue) -> (String, String) {
    (
        field(v, "node").as_str().unwrap().into(),
        field(v, "endpoint").as_str().unwrap().into(),
    )
}

#[test]
fn mixed_allen_cycle_has_checkable_positive_cycle_proof() {
    let r = run(&request(
        r#"["A","B","C"]"#,
        r#"[{"a":"A","relation":"starts","b":"B"},{"a":"B","relation":"before","b":"C"},{"a":"C","relation":"before","b":"A"}]"#,
        100,
    ));
    assert_eq!(status(&r), "InconsistentProof");
    let proof = field(&r, "proof");
    let edges = field(proof, "cycle_edges").as_array().unwrap();
    assert!(!edges.is_empty());
    assert!(
        edges
            .iter()
            .any(|e| field(e, "relation").as_str() == Some("Less"))
    );
    for (i, e) in edges.iter().enumerate() {
        assert_eq!(
            endpoint(field(e, "to")),
            endpoint(field(&edges[(i + 1) % edges.len()], "from"))
        );
    }
}
#[test]
fn complete_witness_satisfies_every_reported_endpoint_constraint() {
    let r = run(&request(
        r#"["A","B","C"]"#,
        r#"[{"a":"A","relation":"before","b":"B"},{"a":"B","relation":"before","b":"C"}]"#,
        100,
    ));
    assert_eq!(status(&r), "ConsistentWitness");
    let w = field(&r, "witness");
    let endpoints = field(w, "endpoints").as_array().unwrap();
    assert_eq!(endpoints.len(), 6);
    let values = endpoints
        .iter()
        .map(|e| {
            (
                endpoint(field(e, "endpoint_ref")),
                field(e, "rank").as_u64().unwrap(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for e in field(w, "checked_endpoint_constraints").as_array().unwrap() {
        let a = values[&endpoint(field(e, "from"))];
        let b = values[&endpoint(field(e, "to"))];
        if field(e, "relation").as_str() == Some("Less") {
            assert!(a < b);
        } else {
            assert!(a <= b);
        }
    }
    assert_eq!(values[&("A".into(), "start".into())], 0);
    assert_eq!(values[&("C".into(), "end".into())], 5);
}
#[test]
fn all_thirteen_relations_have_witnesses_and_their_negation_conflicts() {
    for relation in [
        "before",
        "meets",
        "overlaps",
        "starts",
        "during",
        "finishes",
        "equals",
        "after",
        "metBy",
        "overlappedBy",
        "startedBy",
        "contains",
        "finishedBy",
    ] {
        let constraint = format!(r#"[{{"a":"left","relation":"{relation}","b":"right"}}]"#);
        let actual = run(&request(r#"["left","right"]"#, &constraint, 100));
        assert_eq!(status(&actual), "ConsistentWitness", "{relation}");
        let endpoints = field(field(&actual, "witness"), "endpoints")
            .as_array()
            .unwrap();
        let get = |node: &str, side: &str| {
            endpoints
                .iter()
                .find(|v| endpoint(field(v, "endpoint_ref")) == (node.into(), side.into()))
                .and_then(|v| field(v, "rank").as_u64())
                .unwrap()
        };
        let (a, b, c, d) = (
            get("left", "start"),
            get("left", "end"),
            get("right", "start"),
            get("right", "end"),
        );
        let holds = match relation {
            "before" => b < c,
            "meets" => b == c,
            "overlaps" => a < c && c < b && b < d,
            "starts" => a == c && b < d,
            "during" => c < a && b < d,
            "finishes" => c < a && b == d,
            "equals" => a == c && b == d,
            "after" => a > d,
            "metBy" => a == d,
            "overlappedBy" => c < a && a < d && d < b,
            "startedBy" => a == c && b > d,
            "contains" => a < c && b > d,
            "finishedBy" => a < c && b == d,
            _ => false,
        };
        assert!(
            a < b && c < d && holds,
            "invalid {relation} witness: {a},{b},{c},{d}"
        );
        if relation != "equals" {
            let constraints = format!(
                r#"[{{"a":"left","relation":"{relation}","b":"right"}},{{"a":"left","relation":"equals","b":"right"}}]"#
            );
            assert_eq!(
                status(&run(&request(r#"["left","right"]"#, &constraints, 200))),
                "InconsistentProof",
                "{relation}"
            );
        }
    }
}
#[test]
fn budget_and_unsupported_are_distinct_and_never_consistent() {
    let v = request(
        r#"["A","B"]"#,
        r#"[{"a":"A","relation":"before","b":"B"}]"#,
        0,
    );
    let r = run(&v);
    assert_eq!(status(&r), "Undetermined");
    assert_eq!(field(&r, "reason").as_str(), Some("ResourceLimit"));
    assert_eq!(field(&r, "solver_started"), &JsonValue::Bool(false));
    let r = run(&parse(
        r#"{"profile":"UnsupportedMixedUnknownDisjunctionV1"}"#,
    ));
    assert_eq!(status(&r), "Unsupported");
    assert_eq!(field(&r, "solver_started"), &JsonValue::Bool(false));
    let r = run(&request(
        r#"["A","B"]"#,
        r#"[{"a":"A","relation":"Possible{before,meets}","b":"B"}]"#,
        100,
    ));
    assert_eq!(status(&r), "Unsupported");
    for steps in 1..5 {
        assert_eq!(
            status(&run(&request(
                r#"["A","B"]"#,
                r#"[{"a":"A","relation":"before","b":"B"}]"#,
                steps
            ))),
            "Undetermined"
        );
    }
}
#[test]
fn arbitrary_case_sensitive_and_separator_ids_are_preserved() {
    let r = run(&request(
        r#"["A","a","a.start/λ"]"#,
        r#"[{"a":"A","relation":"before","b":"a"},{"a":"a","relation":"before","b":"a.start/λ"}]"#,
        100,
    ));
    assert_eq!(status(&r), "ConsistentWitness");
    let endpoints = field(field(&r, "witness"), "endpoints").as_array().unwrap();
    let nodes = endpoints
        .iter()
        .map(|p| endpoint(field(p, "endpoint_ref")).0)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(nodes.len(), 3);
    assert!(nodes.contains("a.start/λ"));
}
#[test]
fn invalid_refs_duplicate_nodes_and_mixed_axis_do_not_solve() {
    assert!(network::execute_network(&request(r#"["A","A"]"#, "[]", 100), &parse("{}")).is_err());
    assert!(
        network::execute_network(
            &request(
                r#"["A"]"#,
                r#"[{"a":"A","relation":"before","b":"missing"}]"#,
                100
            ),
            &parse("{}")
        )
        .is_err()
    );
    let r = run(&request(
        r#"[{"id":"A","trs_ref":"other-axis"}]"#,
        "[]",
        100,
    ));
    assert_eq!(status(&r), "Unsupported");
}

fn set_field(value: &mut JsonValue, key: &str, item: JsonValue) {
    let JsonValue::Object(fields) = value else {
        panic!("object fixture required");
    };
    if let Some((_, old)) = fields
        .iter_mut()
        .find(|(name, _)| name.as_str() == Some(key))
    {
        *old = item;
    } else {
        fields.push((tos_foundation::JsonString::from_utf8(key), item));
    }
}

#[test]
fn direct_network_admits_whole_roots_before_cloning_identifiers() {
    use tos_foundation::temporal::{TemporalBudget, TemporalError, execute_temporal_operation};

    let base = request(r#"["A"]"#, "[]", 100);
    let budget = TemporalBudget::default();
    let oversized = budget.max_expression_bytes.checked_add(1).unwrap();
    for in_context in [false, true] {
        for value in [
            JsonValue::String(tos_foundation::JsonString::from_utf8(
                &"x".repeat(oversized),
            )),
            JsonValue::Array(vec![JsonValue::Null; budget.max_members]),
        ] {
            let mut input = base.clone();
            let mut context = parse("{}");
            set_field(
                if in_context { &mut context } else { &mut input },
                "unused",
                value,
            );
            assert!(matches!(
                network::execute_network(&input, &context),
                Err(TemporalError::ResourceExhausted(_))
            ));
            assert!(matches!(
                execute_temporal_operation("check_network_consistency", &input, &context, budget),
                Err(TemporalError::ResourceExhausted(_))
            ));
        }
        let mut input = base.clone();
        let mut context = parse("{}");
        set_field(
            if in_context { &mut context } else { &mut input },
            "unused",
            JsonValue::Object(vec![
                (
                    tos_foundation::JsonString::from_utf8("same"),
                    JsonValue::Null,
                ),
                (
                    tos_foundation::JsonString::from_utf8("same"),
                    JsonValue::Bool(true),
                ),
            ]),
        );
        assert!(matches!(
            network::execute_network(&input, &context),
            Err(TemporalError::MalformedRequest(_))
        ));
        assert!(matches!(
            execute_temporal_operation("check_network_consistency", &input, &context, budget),
            Err(TemporalError::MalformedRequest(_))
        ));
    }
    for key in ["nodes", "endpoint_axis"] {
        let mut input = base.clone();
        let value = JsonValue::String(tos_foundation::JsonString::from_utf8(
            &"x".repeat(oversized),
        ));
        set_field(
            &mut input,
            key,
            if key == "nodes" {
                JsonValue::Array(vec![value])
            } else {
                value
            },
        );
        assert!(matches!(
            network::execute_network(&input, &parse("{}")),
            Err(TemporalError::ResourceExhausted(_))
        ));
        assert!(matches!(
            execute_temporal_operation("check_network_consistency", &input, &parse("{}"), budget),
            Err(TemporalError::ResourceExhausted(_))
        ));
    }
}

#[test]
fn network_parent_keeps_its_explicit_transport_budget() {
    use tos_foundation::temporal::{TemporalBudget, TemporalError, execute_temporal_operation};

    let input = request(r#"["A"]"#, "[]", 100);
    let before = input.clone();
    let context = parse("{}");
    let budget = TemporalBudget::default();
    let direct = network::execute_network(&input, &context).unwrap();
    assert_eq!(status(&direct), "ConsistentWitness");
    let top =
        execute_temporal_operation("check_network_consistency", &input, &context, budget).unwrap();
    for (key, value) in direct.as_object().unwrap() {
        assert_eq!(top.object_get(key.as_str().unwrap()), Some(value));
    }
    assert_eq!(input, before);

    let mut larger = input.clone();
    set_field(
        &mut larger,
        "unused",
        JsonValue::String(tos_foundation::JsonString::from_utf8(
            &"x".repeat(budget.max_expression_bytes.checked_add(1).unwrap()),
        )),
    );
    let before = larger.clone();
    assert!(matches!(
        network::execute_network(&larger, &context),
        Err(TemporalError::ResourceExhausted(_))
    ));
    let explicit = TemporalBudget {
        max_expression_bytes: budget.max_expression_bytes.checked_mul(2).unwrap(),
        ..budget
    };
    let top = execute_temporal_operation("check_network_consistency", &larger, &context, explicit)
        .unwrap();
    assert_eq!(status(&top), "ConsistentWitness");
    for (key, value) in direct.as_object().unwrap() {
        assert_eq!(top.object_get(key.as_str().unwrap()), Some(value));
    }
    assert_eq!(larger, before);
}

#[test]
fn structured_network_axes_ignore_object_order_and_keep_complete_content() {
    let axis = parse(r#"{"trs":{"id":"opaque","version":"v1"},"unit":"day","origin":"zero"}"#);
    for (declared, matches) in [
        (r#"{"origin":"zero","unit":"day","trs":{"version":"v1","id":"opaque"}}"#, true),
        (r#"{"origin":"zero","unit":"day","trs":{"version":"v2","id":"opaque"}}"#, false),
        (r#"{"origin":"zero","unit":"hour","trs":{"version":"v1","id":"opaque"}}"#, false),
        (
            r#"{"origin":"zero","unit":"day","trs":{"version":"v1","id":"opaque"},"extra":null}"#,
            false,
        ),
    ] {
        for node_declaration in [true, false] {
            let declared = parse(declared);
            let mut input = request(
                r#"[{"id":"A"},{"id":"B"}]"#,
                r#"[{"a":"A","b":"B","relation":"before"}]"#,
                10000,
            );
            set_field(&mut input, "endpoint_axis", axis.clone());
            if node_declaration {
                let mut nodes = field(&input, "nodes").clone();
                if let JsonValue::Array(rows) = &mut nodes {
                    set_field(&mut rows[0], "axis", declared);
                }
                set_field(&mut input, "nodes", nodes);
            } else {
                let mut constraints = field(&input, "constraints").clone();
                if let JsonValue::Array(rows) = &mut constraints {
                    set_field(&mut rows[0], "endpoint_axis", declared);
                }
                set_field(&mut input, "constraints", constraints);
            }
            let before = input.clone();
            let direct = run(&input);
            let parent = tos_foundation::temporal::execute_temporal_operation(
                "check_network_consistency",
                &input,
                &parse("{}"),
                tos_foundation::temporal::TemporalBudget::default(),
            )
            .unwrap();
            for result in [&direct, &parent] {
                if matches {
                    assert_eq!(status(result), "ConsistentWitness");
                } else {
                    assert_eq!(status(result), "Unsupported");
                    assert_eq!(
                        field(result, "reason").as_str(),
                        Some("IncompatibleEndpointAxes")
                    );
                }
            }
            assert_eq!(input, before);
        }
    }
}
