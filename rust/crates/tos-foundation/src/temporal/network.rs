//! Bounded exact non-disjunctive Allen network consistency.
//!
//! This solver uses only endpoint order, not calendars or elapsed durations.
//! Every interval is proper. A witness assigns every endpoint and is checked
//! independently against every derived edge before it is returned.
use crate::temporal::TemporalError;
use crate::{JsonNumber, JsonNumberKind, JsonString, JsonValue};
use std::collections::{BTreeMap, BTreeSet};

const PROFILE: &str = "ExactComparableEndpointIntervalsV1";
const RULE_VERSION: &str = "tos-exact-allen-endpoint-network-v1";
// Safety ceilings are disclosed as bounds, not claims of solved consistency.
const MAX_NODES: usize = 256;
const MAX_CONSTRAINTS: usize = 4096;
const MAX_STEPS: usize = 2_000_000;

#[derive(Clone, Debug)]
struct Edge {
    from: usize,
    to: usize,
    strict: bool,
    source: Option<usize>,
}
#[derive(Clone, Copy)]
struct Budget {
    nodes: usize,
    constraints: usize,
    steps: usize,
}

fn text(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
fn num(n: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn obj(p: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        p.into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn field<'a>(v: &'a JsonValue, k: &str) -> &'a JsonValue {
    v.object_get(k).unwrap_or(&JsonValue::Null)
}
fn string<'a>(v: &'a JsonValue, k: &str) -> Option<&'a str> {
    field(v, k).as_str()
}
fn outcome(status: &str, reason: &str, started: bool, steps: usize) -> JsonValue {
    obj(vec![
        ("status", text(status)),
        ("reason", text(reason)),
        ("solver_started", JsonValue::Bool(started)),
        ("solver_steps", num(steps)),
        ("rule_version", text(RULE_VERSION)),
    ])
}
fn budget(input: &JsonValue) -> Result<Budget, TemporalError> {
    let b = field(input, "resource_budget");
    let get = |k| {
        field(b, k)
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(TemporalError::MalformedRequest(
                "explicit unsigned network budgets required",
            ))
    };
    Ok(Budget {
        nodes: get("max_nodes")?,
        constraints: get("max_constraints")?,
        steps: get("max_solver_steps")?,
    })
}
fn endpoint(index: usize, nodes: &[String]) -> JsonValue {
    obj(vec![
        ("node", text(&nodes[index / 2])),
        (
            "endpoint",
            text(if index % 2 == 0 { "start" } else { "end" }),
        ),
    ])
}
fn edge_value(edge: &Edge, nodes: &[String]) -> JsonValue {
    obj(vec![
        ("from", endpoint(edge.from, nodes)),
        ("to", endpoint(edge.to, nodes)),
        (
            "relation",
            text(if edge.strict { "Less" } else { "LessOrEqual" }),
        ),
        (
            "source_constraint_index",
            edge.source.map_or(JsonValue::Null, num),
        ),
    ])
}
fn bound_value(b: Budget) -> JsonValue {
    obj(vec![
        ("max_nodes", num(b.nodes.min(MAX_NODES))),
        ("max_constraints", num(b.constraints.min(MAX_CONSTRAINTS))),
        ("max_solver_steps", num(b.steps.min(MAX_STEPS))),
    ])
}

fn resource_limit(b: Budget, started: bool, steps: usize) -> JsonValue {
    let mut r = outcome("Undetermined", "ResourceLimit", started, steps);
    if let JsonValue::Object(v) = &mut r {
        v.push((JsonString::from_utf8("budget"), bound_value(b)));
    }
    r
}

fn relation_edges(relation: &str, a: usize, b: usize, source: usize, out: &mut Vec<Edge>) -> bool {
    let (a, b, relation) = match relation {
        "after" => (b, a, "before"),
        "metBy" => (b, a, "meets"),
        "overlappedBy" => (b, a, "overlaps"),
        "startedBy" => (b, a, "starts"),
        "contains" => (b, a, "during"),
        "finishedBy" => (b, a, "finishes"),
        _ => (a, b, relation),
    };
    let (s, e, t, u) = (a * 2, a * 2 + 1, b * 2, b * 2 + 1);
    let mut add = |x, y, strict| {
        out.push(Edge {
            from: x,
            to: y,
            strict,
            source: Some(source),
        })
    };
    match relation {
        "before" => add(e, t, true),
        "meets" => {
            add(e, t, false);
            add(t, e, false);
        }
        "overlaps" => {
            add(s, t, true);
            add(t, e, true);
            add(e, u, true);
        }
        "starts" => {
            add(s, t, false);
            add(t, s, false);
            add(e, u, true);
        }
        "during" => {
            add(t, s, true);
            add(e, u, true);
        }
        "finishes" => {
            add(t, s, true);
            add(e, u, false);
            add(u, e, false);
        }
        "equals" => {
            add(s, t, false);
            add(t, s, false);
            add(e, u, false);
            add(u, e, false);
        }
        _ => return false,
    }
    true
}

/// Declared exact common-axis profile only. No unsupported relation is silently
/// dropped. Unsupported profile is classified before solver resource evaluation.
/// Direct calls use the default aggregate temporal transport limits.
pub fn execute_network(input: &JsonValue, context: &JsonValue) -> Result<JsonValue, TemporalError> {
    if input.as_object().is_none() || context.as_object().is_none() {
        return Err(TemporalError::MalformedRequest(
            "network input/context must be objects",
        ));
    }
    super::admit_transport(input, context, super::TemporalBudget::default())?;
    execute_network_admitted(input, context)
}

// Parent dispatch retains its caller-selected, already-admitted transport limits.
pub(super) fn execute_network_admitted(
    input: &JsonValue,
    _context: &JsonValue,
) -> Result<JsonValue, TemporalError> {
    if string(input, "profile") != Some(PROFILE)
        || string(input, "node_shape") != Some("finite-proper-interval")
        || string(input, "endpoint_domain") != Some("exact-strict-weak-order")
    {
        return Ok(outcome(
            "Unsupported",
            "UnsupportedConsistencyProfile",
            false,
            0,
        ));
    }
    let axis = field(input, "endpoint_axis");
    if axis.as_str().is_none_or(|s| s.trim().is_empty()) && axis.as_object().is_none() {
        return Ok(outcome(
            "Unsupported",
            "ComparableEndpointAxisRequired",
            false,
            0,
        ));
    }
    if axis.as_object().is_some() {
        let trs = field(axis, "trs");
        if [
            string(trs, "id"),
            string(trs, "version"),
            string(axis, "unit"),
            string(axis, "origin"),
        ]
        .iter()
        .any(|v| v.is_none_or(|s| s.trim().is_empty()))
        {
            return Ok(outcome(
                "Unsupported",
                "ComparableEndpointAxisRequired",
                false,
                0,
            ));
        }
    }
    let b = budget(input)?;
    let raw_nodes = field(input, "nodes")
        .as_array()
        .ok_or(TemporalError::MalformedRequest(
            "network nodes array required",
        ))?;
    let constraints =
        field(input, "constraints")
            .as_array()
            .ok_or(TemporalError::MalformedRequest(
                "network constraints array required",
            ))?;
    if raw_nodes.len() > b.nodes.min(MAX_NODES)
        || constraints.len() > b.constraints.min(MAX_CONSTRAINTS)
        || b.steps == 0
    {
        let mut r = outcome("Undetermined", "ResourceLimit", false, 0);
        if let JsonValue::Object(v) = &mut r {
            v.push((JsonString::from_utf8("budget"), bound_value(b)));
        }
        return Ok(r);
    }
    let mut nodes = vec![];
    let mut indexes = BTreeMap::new();
    for node in raw_nodes {
        let id =
            node.as_str()
                .or_else(|| string(node, "id"))
                .ok_or(TemporalError::MalformedRequest(
                    "network node must have exact id",
                ))?;
        if id.trim().is_empty() || indexes.contains_key(id) {
            return Err(TemporalError::MalformedRequest(
                "empty or duplicate network node",
            ));
        }
        if ["start", "end", "beginning", "extent"]
            .iter()
            .any(|key| node.object_get(key).is_some())
        {
            return Ok(outcome(
                "Unsupported",
                "FixedOrUnknownEndpointProfileNotImplemented",
                false,
                0,
            ));
        }
        for key in ["trs_version", "unit", "origin"] {
            if let Some(declared) = node.object_get(key) {
                let expected = if key == "trs_version" {
                    string(field(axis, "trs"), "version")
                } else {
                    string(axis, key)
                };
                if declared.as_str().is_none() || declared.as_str() != expected {
                    return Ok(outcome("Unsupported", "IncompatibleEndpointAxes", false, 0));
                }
            }
        }
        if let Some(declared) = node.object_get("axis") {
            if !crate::query_context::same_json(declared, axis) {
                return Ok(outcome("Unsupported", "IncompatibleEndpointAxes", false, 0));
            }
        }
        if let Some(declared) = node.object_get("trs_ref") {
            let expected = axis.as_str().or_else(|| string(field(axis, "trs"), "id"));
            if declared.as_str() != expected {
                return Ok(outcome("Unsupported", "IncompatibleEndpointAxes", false, 0));
            }
        }
        indexes.insert(id.to_owned(), nodes.len());
        nodes.push(id.to_owned());
    }
    // Validate the supported relation vocabulary before entering the solver.
    let supported = [
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
    ];
    for c in constraints {
        let r = string(c, "relation")
            .ok_or(TemporalError::MalformedRequest("network relation required"))?;
        if !supported.contains(&r) {
            return Ok(outcome(
                "Unsupported",
                "UnsupportedConsistencyProfile",
                false,
                0,
            ));
        }
        if string(c, "a").is_none_or(|s| !indexes.contains_key(s))
            || string(c, "b").is_none_or(|s| !indexes.contains_key(s))
        {
            return Err(TemporalError::MalformedRequest(
                "network constraint refers to unknown node",
            ));
        }
        if let Some(declared) = c.object_get("endpoint_axis") {
            if !crate::query_context::same_json(declared, axis) {
                return Ok(outcome("Unsupported", "IncompatibleEndpointAxes", false, 0));
            }
        }
    }
    let mut edges = vec![];
    for i in 0..nodes.len() {
        edges.push(Edge {
            from: 2 * i,
            to: 2 * i + 1,
            strict: true,
            source: None,
        });
    }
    for (i, c) in constraints.iter().enumerate() {
        let a = indexes[string(c, "a").expect("validated")];
        let other = indexes[string(c, "b").expect("validated")];
        if !relation_edges(
            string(c, "relation").expect("validated"),
            a,
            other,
            i,
            &mut edges,
        ) {
            return Ok(outcome(
                "Unsupported",
                "UnsupportedConsistencyProfile",
                false,
                0,
            ));
        }
    }
    solve(&nodes, &edges, axis, b)
}

fn solve(
    nodes: &[String],
    edges: &[Edge],
    axis: &JsonValue,
    b: Budget,
) -> Result<JsonValue, TemporalError> {
    let n = nodes.len() * 2;
    let limit = b.steps.min(MAX_STEPS);
    let mut values = vec![0usize; n];
    let mut pred = vec![None; n];
    let mut used = 0usize;
    for pass in 0..n {
        let mut changed = None;
        for (i, e) in edges.iter().enumerate() {
            if used >= limit {
                return Ok(resource_limit(b, used > 0, used));
            }
            used += 1;
            let proposed = values[e.from] + usize::from(e.strict);
            if proposed > values[e.to] {
                values[e.to] = proposed;
                pred[e.to] = Some(i);
                changed = Some(e.to);
            }
        }
        if changed.is_none() {
            return witness(nodes, edges, &values, axis, used, b);
        }
        if pass + 1 == n {
            let mut vertex = changed.expect("changed");
            for _ in 0..n {
                vertex = edges[pred[vertex].ok_or(TemporalError::MalformedRequest(
                    "network proof predecessor missing",
                ))?]
                .from;
            }
            let start = vertex;
            let mut cycle = vec![];
            loop {
                if used >= limit {
                    return Ok(resource_limit(b, true, used));
                }
                used += 1;
                let ei = pred[vertex].ok_or(TemporalError::MalformedRequest(
                    "network proof predecessor missing",
                ))?;
                cycle.push(ei);
                vertex = edges[ei].from;
                if vertex == start {
                    break;
                }
                if cycle.len() > n {
                    return Err(TemporalError::MalformedRequest(
                        "network proof cycle invalid",
                    ));
                }
            }
            cycle.reverse();
            // Proof verification is independent of relaxation values.
            let strict = cycle.iter().filter(|&&i| edges[i].strict).count();
            if strict == 0
                || cycle
                    .iter()
                    .enumerate()
                    .any(|(i, &ei)| edges[ei].to != edges[cycle[(i + 1) % cycle.len()]].from)
            {
                return Err(TemporalError::MalformedRequest(
                    "network positive cycle verification failed",
                ));
            }
            return Ok(obj(vec![
                ("status", text("InconsistentProof")),
                (
                    "proof",
                    obj(vec![
                        ("kind", text("StrictEndpointCycle")),
                        (
                            "cycle_edges",
                            JsonValue::Array(
                                cycle
                                    .iter()
                                    .map(|&i| edge_value(&edges[i], nodes))
                                    .collect(),
                            ),
                        ),
                        ("strict_edge_count", num(strict)),
                        ("verified", JsonValue::Bool(true)),
                    ]),
                ),
                ("endpoint_axis", axis.clone()),
                ("solver_started", JsonValue::Bool(true)),
                ("solver_steps", num(used)),
                ("rule_version", text(RULE_VERSION)),
            ]));
        }
    }
    // Empty network has a complete empty witness, not a missing solver result.
    witness(nodes, edges, &values, axis, used, b)
}
fn witness(
    nodes: &[String],
    edges: &[Edge],
    values: &[usize],
    axis: &JsonValue,
    mut used: usize,
    b: Budget,
) -> Result<JsonValue, TemporalError> {
    let limit = b.steps.min(MAX_STEPS);
    for e in edges {
        if used >= limit {
            return Ok(resource_limit(b, used > 0, used));
        }
        used += 1;
        if values[e.to] < values[e.from] + usize::from(e.strict) {
            return Err(TemporalError::MalformedRequest(
                "network witness verification failed",
            ));
        }
    }
    let endpoints = (0..values.len())
        .map(|i| {
            obj(vec![
                ("endpoint_ref", endpoint(i, nodes)),
                ("rank", num(values[i])),
            ])
        })
        .collect();
    let source_constraints = edges
        .iter()
        .filter_map(|e| e.source)
        .collect::<BTreeSet<_>>();
    Ok(obj(vec![
        ("status", text("ConsistentWitness")),
        (
            "witness",
            obj(vec![
                ("endpoints", JsonValue::Array(endpoints)),
                (
                    "checked_endpoint_constraints",
                    JsonValue::Array(edges.iter().map(|e| edge_value(e, nodes)).collect()),
                ),
                (
                    "checked_source_constraints",
                    JsonValue::Array(source_constraints.into_iter().map(num).collect()),
                ),
                ("verified", JsonValue::Bool(true)),
            ]),
        ),
        ("endpoint_axis", axis.clone()),
        ("solver_started", JsonValue::Bool(!edges.is_empty())),
        ("solver_steps", num(used)),
        ("rule_version", text(RULE_VERSION)),
    ]))
}
