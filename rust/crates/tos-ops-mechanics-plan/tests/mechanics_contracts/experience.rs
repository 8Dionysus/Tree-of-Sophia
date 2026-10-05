//! Native schema subjects with the retained package-local mutation assertions.
use jsonschema::{Draft, Validator};
use serde_json::{Value, json};

fn validator(schema: &Value) -> Validator {
    jsonschema::options()
        .with_draft(Draft::Draft202012)
        .offline()
        .build(schema)
        .unwrap()
}
fn wrong(value: &Value) -> Value {
    match value {
        Value::Bool(_) => json!("not-a-boolean"),
        Value::Number(n) if n.is_i64() || n.is_u64() => json!("not-an-integer"),
        Value::Number(_) => json!("not-a-number"),
        Value::String(_) => json!(12345),
        Value::Array(_) => json!({"not": "an array"}),
        Value::Object(_) => json!("not-an-object"),
        Value::Null => json!("not-null"),
    }
}
fn escape(value: &Value, suffix: &str) -> Value {
    match value {
        Value::Bool(value) => json!(!value),
        Value::Number(value) if value.is_i64() => {
            json!(value.as_i64().unwrap().checked_add(1).unwrap())
        }
        Value::Number(value) if value.is_u64() => {
            json!(value.as_u64().unwrap().checked_add(1).unwrap())
        }
        Value::Number(value) => json!(value.as_f64().unwrap() + 1.0),
        Value::String(value) => json!(format!("{value}{suffix}")),
        _ => json!(suffix),
    }
}
fn pointer(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}
fn walk<'a>(value: &'a Value, path: String, found: &mut Vec<(String, &'a Value)>) {
    found.push((path.clone(), value));
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                walk(child, pointer(&path, key), found);
            }
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                walk(child, pointer(&path, &index.to_string()), found);
            }
        }
        _ => (),
    }
}
fn mutation(check: &Validator, example: &Value, path: &str, value: Value, label: &str) {
    let mut changed = example.clone();
    *changed.pointer_mut(path).unwrap() = value;
    assert!(
        !check.is_valid(&changed),
        "{label}: {path} unexpectedly validated"
    );
}
fn effective<'a>(schema: &'a Value, example: &Value, variants: bool) -> &'a Value {
    if variants {
        if let Some(choices) = schema.get("oneOf").and_then(Value::as_array) {
            for choice in choices {
                if choice.is_object() && validator(choice).is_valid(example) {
                    return choice;
                }
            }
        }
    }
    schema
}
fn constraints<'a>(
    schema: &'a Value,
    example: &'a Value,
    path: String,
    variants: bool,
    required: &mut Vec<String>,
    constrained: &mut Vec<(String, &'a Value)>,
) {
    let schema = effective(schema, example, variants);
    constrained.push((path.clone(), schema));
    if schema["type"] == "object" {
        if let Some(object) = example.as_object() {
            if let Some(keys) = schema.get("required").and_then(Value::as_array) {
                for key in keys {
                    let key = key.as_str().unwrap();
                    if object.contains_key(key) {
                        required.push(pointer(&path, key));
                    }
                }
            }
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (key, child) in properties {
                    if let Some(value) = object.get(key) {
                        constraints(
                            child,
                            value,
                            pointer(&path, key),
                            variants,
                            required,
                            constrained,
                        );
                    }
                }
            }
        }
    } else if schema["type"] == "array" {
        if let Some(value) = example.as_array().and_then(|values| values.first()) {
            constraints(
                &schema["items"],
                value,
                pointer(&path, "0"),
                variants,
                required,
                constrained,
            );
        }
    }
}
fn remove(value: &mut Value, path: &str) {
    let (parent, leaf) = path.rsplit_once('/').unwrap();
    let key = leaf.replace("~1", "/").replace("~0", "~");
    let value = value.pointer_mut(parent).unwrap();
    if let Some(object) = value.as_object_mut() {
        assert!(object.remove(&key).is_some());
    } else {
        value
            .as_array_mut()
            .unwrap()
            .remove(key.parse::<usize>().unwrap());
    }
}

fn schema_for_path<'a>(schema: &'a Value, example: &Value, path: &str) -> &'a Value {
    let mut schema = schema;
    let mut value = example;
    for segment in path.strip_prefix('/').unwrap().split('/') {
        let key = segment.replace("~1", "/").replace("~0", "~");
        schema = effective(schema, value, true);
        if value.is_object() {
            schema = &schema["properties"][&key];
            value = &value[&key];
        } else {
            schema = &schema["items"];
            value = &value[key.parse::<usize>().unwrap()];
            schema = effective(schema, value, true);
        }
    }
    effective(schema, value, true)
}

fn recursive_contract(schema: &Value, example: &Value, installation: bool) -> [usize; 7] {
    let check = validator(schema);
    assert!(
        check.is_valid(example),
        "authored example: {:?}",
        check.iter_errors(example).collect::<Vec<_>>()
    );
    let suffix = if installation {
        "__experience_installation_service_office_not_allowed__"
    } else {
        "__experience_governance_boundary_not_allowed__"
    };
    let mut fields = Vec::new();
    walk(example, String::new(), &mut fields);
    let mut counts = [0usize; 7];
    for (path, value) in &fields {
        if let Some(object) = value.as_object() {
            let mut unknown = object.clone();
            unknown.insert("contract_escape".into(), json!("loose-field"));
            mutation(
                &check,
                example,
                path,
                Value::Object(unknown),
                "unknown field",
            );
            counts[0] += 1;
        }
        if !path.is_empty() {
            mutation(&check, example, path, wrong(value), "wrong type");
            counts[1] += 1;
        }
        if let Some(array) = value.as_array() {
            let mut replacement = array.clone();
            if installation {
                if replacement.is_empty() {
                    replacement.push(json!({"not":"a valid array item"}));
                } else {
                    replacement[0] = wrong(&replacement[0]);
                }
            } else {
                replacement = vec![array.first().map(wrong).unwrap_or(json!(12345))];
            }
            mutation(&check, example, path, json!(replacement), "bad array item");
            if !installation && (array.is_empty() || array[0].is_string()) {
                mutation(&check, example, path, json!([""]), "empty array string");
            }
            counts[2] += 1;
        }
        if installation && value.is_string() {
            mutation(&check, example, path, json!(""), "empty string");
            counts[3] += 1;
        }
        if !installation && value.is_number() {
            mutation(&check, example, path, json!(-1), "negative number");
        }
        if installation && value.is_number() {
            // Unlike required/const enumeration (which selects the first
            // array item), the original numeric test visits every value.
            let field = schema_for_path(schema, example, path);
            for (key, delta) in [("minimum", -1.0), ("maximum", 1.0)] {
                if let Some(bound) = field.get(key).and_then(Value::as_f64) {
                    mutation(&check, example, path, json!(bound + delta), key);
                }
            }
        }
    }
    let mut required = Vec::new();
    let mut constrained = Vec::new();
    constraints(
        schema,
        example,
        String::new(),
        installation,
        &mut required,
        &mut constrained,
    );
    for path in required {
        let mut changed = example.clone();
        remove(&mut changed, &path);
        assert!(!check.is_valid(&changed), "missing required {path}");
        counts[4] += 1;
    }
    for (path, field) in constrained {
        if path.is_empty() {
            continue;
        }
        let value = example.pointer(&path).unwrap();
        for (index, keyword) in [(5, "const"), (6, "enum")] {
            if field.get(keyword).is_some() {
                let replacement = if installation || keyword == "const" {
                    escape(value, suffix)
                } else {
                    json!(suffix)
                };
                mutation(&check, example, &path, replacement, keyword);
                counts[index] += 1;
            }
        }
    }
    // Counts are accumulated across each family's authored pairs below: an
    // individual contract need not contain every kind of constraint.
    counts
}

fn candidate_contract(schema: &Value, example: &Value, arrays: &mut usize) {
    let check = validator(schema);
    assert!(check.is_valid(example));
    let suffix = "__experience_candidate_adoption_write_guard_not_allowed__";
    let mut changed = example.clone();
    changed
        .as_object_mut()
        .unwrap()
        .insert("contract_escape".into(), json!(true));
    assert!(!check.is_valid(&changed));
    const GUARDS: &[&str] = &[
        "authority_required",
        "derived_only",
        "direct_tos_write",
        "direct_write",
        "direct_write_allowed",
        "direct_write_blocked",
        "dossier_allowed",
        "drill_required",
        "kag_may_force_uptake",
        "kag_may_propose",
        "lineage_indexed",
        "meaning_authority",
        "release_required",
        "required_trial",
        "requires_eval_verdict",
        "requires_owner_consent",
        "rollback_required",
        "scar_required",
        "source_theft",
        "submit_only",
    ];
    for section in ["", "/refs", "/payload"] {
        if let Some(object) = example.pointer(section).and_then(Value::as_object) {
            if !section.is_empty() {
                let mut unknown = object.clone();
                unknown.insert(
                    "contract_escape".into(),
                    json!(if section == "/refs" {
                        "loose-ref"
                    } else {
                        "loose-payload"
                    }),
                );
                mutation(
                    &check,
                    example,
                    section,
                    json!(unknown),
                    "unknown section field",
                );
            }
            for (key, value) in object {
                let path = pointer(section, key);
                if value.is_array() {
                    *arrays += 1;
                    mutation(
                        &check,
                        example,
                        &path,
                        json!([12345]),
                        "non-string array item",
                    );
                    mutation(&check, example, &path, json!([""]), "empty array string");
                }
                if section != "/payload" {
                    continue;
                }
                mutation(&check, example, &path, wrong(value), "wrong payload type");
                if let Some(value) = value.as_bool().filter(|_| GUARDS.contains(&key.as_str())) {
                    mutation(&check, example, &path, json!(!value), "inverted guardrail");
                }
                if value.is_number() {
                    mutation(&check, example, &path, json!(-1), "negative payload number");
                    if key == "retention_cycles" {
                        mutation(&check, example, &path, json!(0), "zero retention");
                    }
                    if key == "value" || key.contains("rate") || key.contains("threshold") {
                        mutation(&check, example, &path, json!(1.5), "ratio above one");
                    }
                }
                if schema
                    .pointer(&pointer("/properties/payload/properties", key))
                    .and_then(|s| s.get("enum"))
                    .is_some()
                {
                    mutation(&check, example, &path, json!(suffix), "payload enum escape");
                }
            }
        }
    }
}

macro_rules! pair {
    ($root:expr, $schema:literal, $example:literal) => {{
        (
            serde_json::from_slice::<Value>(&std::fs::read($root.join($schema)).unwrap()).unwrap(),
            serde_json::from_slice::<Value>(&std::fs::read($root.join($example)).unwrap()).unwrap(),
        )
    }};
}

pub(super) fn experience_candidate_all_retained_schema_mutations(root: &std::path::Path) {
    let pairs = [
        pair!(
            root,
            "mechanics/experience/parts/candidate-review/schemas/aoa_experience_candidate_dossier_v1.json",
            "mechanics/experience/parts/candidate-review/examples/aoa_experience_candidate_dossier.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/adoption-boundary/schemas/tos_adoption_boundary_dossier_v1.json",
            "mechanics/experience/parts/adoption-boundary/examples/tos_adoption_boundary_dossier.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/candidate-review/schemas/tos_intake_boundary_decision_v1.json",
            "mechanics/experience/parts/candidate-review/examples/tos_intake_boundary_decision.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/write-guards/schemas/tos_no_direct_write_guard_v1.json",
            "mechanics/experience/parts/write-guards/examples/tos_no_direct_write_guard.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/adoption-boundary/schemas/tos_no_runtime_adoption_guard_v1.json",
            "mechanics/experience/parts/adoption-boundary/examples/tos_no_runtime_adoption_guard.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/pattern-review/schemas/tos_pattern_review_note_v1.json",
            "mechanics/experience/parts/pattern-review/examples/tos_pattern_review_note.example.json"
        ),
    ];
    assert!(!pairs.is_empty());
    let mut arrays = 0;
    for (schema, example) in pairs {
        candidate_contract(&schema, &example, &mut arrays);
    }
    assert!(arrays > 0);
}

pub(super) fn experience_governance_all_retained_schema_mutations(root: &std::path::Path) {
    let pairs = [
        pair!(
            root,
            "mechanics/experience/parts/governance-boundary/schemas/tos_governance_review_note_v1.json",
            "mechanics/experience/parts/governance-boundary/examples/tos_governance_review_note.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/governance-boundary/schemas/tos_governance_dossier_boundary_v1.json",
            "mechanics/experience/parts/governance-boundary/examples/tos_governance_dossier_boundary_v1.example.json"
        ),
    ];
    assert!(!pairs.is_empty());
    let mut total = [0usize; 7];
    for (schema, example) in pairs {
        let counts = recursive_contract(&schema, &example, false);
        for (slot, count) in total.iter_mut().zip(counts) {
            *slot += count;
        }
    }
    for index in [0, 1, 2, 4, 5, 6] {
        assert!(
            total[index] > 0,
            "mutation family {index} must be exercised"
        );
    }
}

pub(super) fn experience_installation_all_retained_schema_mutations(root: &std::path::Path) {
    let pairs = [
        pair!(
            root,
            "mechanics/experience/parts/installation-boundary/schemas/tos_installation_dossier_boundary_v1.json",
            "mechanics/experience/parts/installation-boundary/examples/tos_installation_dossier_boundary_v1.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/service-office-boundary/schemas/tos_no_runtime_office_write_guard_v1.json",
            "mechanics/experience/parts/service-office-boundary/examples/tos_no_runtime_office_write_guard_v1.example.json"
        ),
        pair!(
            root,
            "mechanics/experience/parts/service-office-boundary/schemas/tos_service_dossier_boundary_v1.json",
            "mechanics/experience/parts/service-office-boundary/examples/tos_service_dossier_boundary_v1.example.json"
        ),
    ];
    assert!(!pairs.is_empty());
    let mut total = [0usize; 7];
    for (schema, example) in pairs {
        let counts = recursive_contract(&schema, &example, true);
        for (slot, count) in total.iter_mut().zip(counts) {
            *slot += count;
        }
    }
    for index in [0, 1, 2, 3, 4, 5, 6] {
        assert!(
            total[index] > 0,
            "mutation family {index} must be exercised"
        );
    }
}
