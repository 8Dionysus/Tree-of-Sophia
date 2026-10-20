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
