use super::*;
use std::cmp::Ordering;

fn known_trs(t: &TrsRef) -> bool {
    !t.id.trim().is_empty() && !t.version.trim().is_empty()
}
fn known_axis(a: &Axis) -> bool {
    known_trs(&a.trs) && !a.unit.trim().is_empty() && !a.origin.trim().is_empty()
}
pub fn compare_positions(a: &Position, b: &Position) -> PositionComparison {
    let ord = match (a, b) {
        (
            Position::Coordinate {
                value: a,
                unit: au,
                origin: ao,
                trs: at,
            },
            Position::Coordinate {
                value: b,
                unit: bu,
                origin: bo,
                trs: bt,
            },
        ) => {
            if !known_trs(at) || !known_trs(bt) {
                return PositionComparison::Incomparable("UnknownTRS");
            }
            if at != bt {
                return PositionComparison::Incomparable("NoCrosswalk");
            }
            if au.is_empty()
                || ao.is_empty()
                || bu.is_empty()
                || bo.is_empty()
                || au != bu
                || ao != bo
            {
                return PositionComparison::Incomparable("IncompatibleScales");
            }
            a.cmp(b)
        }
        (
            Position::Ordinal {
                rank: a,
                trs: at,
                ordering: ac,
                ..
            },
            Position::Ordinal {
                rank: b,
                trs: bt,
                ordering: bc,
                ..
            },
        ) => {
            if !known_trs(at) || !known_trs(bt) {
                return PositionComparison::Incomparable("UnknownTRS");
            }
            if at != bt {
                return PositionComparison::Incomparable("NoCrosswalk");
            }
            if !matches!(
                ac,
                OrderingCapability::Total | OrderingCapability::OrdinalOnly
            ) || ac != bc
            {
                return PositionComparison::Unresolved("PartialOrderRequiresConstraints");
            }
            let (Some(a), Some(b)) = (a, b) else {
                return PositionComparison::Unresolved("OrdinalRankUnknown");
            };
            a.cmp(b)
        }
        (Position::Calendar { trs: a, .. }, Position::Calendar { trs: b, .. }) => {
            if a.is_none() || b.is_none() {
                return PositionComparison::Incomparable("UnknownTRS");
            }
            return if a != b {
                PositionComparison::Incomparable("NoCrosswalk")
            } else {
                PositionComparison::Unresolved("CalendarRulesRequired")
            };
        }
        _ => return PositionComparison::Incomparable("IncompatiblePositionKinds"),
    };
    match ord {
        Ordering::Less => PositionComparison::Less,
        Ordering::Equal => PositionComparison::Equal,
        Ordering::Greater => PositionComparison::Greater,
    }
}
pub fn relate_exact_extents(a: &ExactExtent, b: &ExactExtent) -> RelationOutcome {
    use ExactExtent::*;
    match (a, b) {
        (Unknown, _) | (_, Unknown) => RelationOutcome::Unresolved("UnknownExtent"),
        (
            Instant {
                axis: a,
                position: x,
            },
            Instant {
                axis: b,
                position: y,
            },
        ) => {
            if !known_axis(a) || !known_axis(b) {
                return RelationOutcome::Incomparable("UnknownTRS");
            }
            if a != b {
                return RelationOutcome::Incomparable("IncompatibleAxes");
            }
            RelationOutcome::Entailed(match x.cmp(y) {
                Ordering::Less => Relation::Before,
                Ordering::Equal => Relation::Equals,
                Ordering::Greater => Relation::After,
            })
        }
        (
            Interval {
                axis: a,
                beginning: as_,
                end: ae,
                beginning_inclusivity: ai,
                end_inclusivity: aj,
            },
            Interval {
                axis: b,
                beginning: bs,
                end: be,
                beginning_inclusivity: bi,
                end_inclusivity: bj,
            },
        ) => {
            if !known_axis(a) || !known_axis(b) {
                return RelationOutcome::Incomparable("UnknownTRS");
            }
            if a != b {
                return RelationOutcome::Incomparable("IncompatibleAxes");
            }
            if as_ >= ae || bs >= be {
                return RelationOutcome::Contradicted("ProperIntervalRequired");
            }
            if [ai, aj, bi, bj].contains(&&Inclusivity::Unspecified) {
                return RelationOutcome::Unresolved("BoundaryInclusivityUnspecified");
            }
            // Allen topology is defined on the endpoint ordering. The
            // inclusivity contract is retained; no SQL [) normalization.
            RelationOutcome::Entailed(allen(*as_, *ae, *bs, *be))
        }
        _ => RelationOutcome::Unsupported("MixedPointIntervalProfile"),
    }
}
fn allen(a: i64, b: i64, c: i64, d: i64) -> Relation {
    if b < c {
        Relation::Before
    } else if a > d {
        Relation::After
    } else if b == c {
        Relation::Meets
    } else if a == d {
        Relation::MetBy
    } else if a == c && b == d {
        Relation::Equals
    } else if a == c && b < d {
        Relation::Starts
    } else if a == c {
        Relation::StartedBy
    } else if b == d && a > c {
        Relation::Finishes
    } else if b == d {
        Relation::FinishedBy
    } else if a > c && b < d {
        Relation::During
    } else if a < c && b > d {
        Relation::Contains
    } else if a < c {
        Relation::Overlaps
    } else {
        Relation::OverlappedBy
    }
}
fn compatible(a: &JsonValue, b: &JsonValue) -> Result<(), &'static str> {
    let at = string(a, "trs_ref");
    let bt = string(b, "trs_ref");
    if at.is_none_or(|s| s.trim().is_empty()) || bt.is_none_or(|s| s.trim().is_empty()) {
        return Err("UnknownTRS");
    }
    if at != bt {
        return Err("NoCrosswalk");
    }
    Ok(())
}
pub(super) fn compare_json(input: &JsonValue) -> JsonValue {
    let a = field(input, "left");
    let b = field(input, "right");
    if let Err(r) = compatible(a, b) {
        let mut out = reason("Incomparable", r);
        if r == "UnknownTRS" {
            put(
                &mut out,
                "preserved_lexical",
                JsonValue::Array(vec![
                    field(a, "lexical").clone(),
                    field(b, "lexical").clone(),
                ]),
            );
        }
        return out;
    }
    if [a, b].iter().any(|v| {
        v.object_get("qualifier")
            .is_some_and(|q| q.as_str() != Some("None"))
    }) {
        return reason("Unresolved", "NonExactPosition");
    }
    if string(a, "type") != Some("OrdinalPosition") || string(b, "type") != Some("OrdinalPosition")
    {
        return reason("Unsupported", "PositionComparisonProfileNotImplemented");
    }
    for operand in [a, b] {
        if matches!(string(operand, "ordering"), Some("Partial" | "Unknown"))
            || matches!(
                string(operand, "ordering_capability"),
                Some("Partial" | "Unknown")
            )
        {
            return reason("Unresolved", "PartialOrderRequiresConstraints");
        }
    }
    // The numeric OrdinalPosition profile uses explicit ranks. Arbitrary
    // named nodes require constraints and are never sorted by labels.
    let (Some(a), Some(b)) = (integer(field(a, "value")), integer(field(b, "value"))) else {
        return reason("Unresolved", "OrdinalRankUnknown");
    };
    obj(vec![
        (
            "status",
            text(match a.cmp(&b) {
                Ordering::Less => "Less",
                Ordering::Equal => "Equal",
                Ordering::Greater => "Greater",
            }),
        ),
        ("basis", text("same-explicit-ordinal-TRS")),
        ("absolute_conversion", text("NotRequested")),
    ])
}
fn axis_value(value: &JsonValue) -> Option<Axis> {
    let t = field(value, "trs");
    let a = Axis {
        trs: TrsRef {
            id: string(t, "id")?.into(),
            version: string(t, "version")?.into(),
        },
        unit: string(value, "unit")?.into(),
        origin: string(value, "origin")?.into(),
    };
    known_axis(&a).then_some(a)
}
fn explicit_axis(input: &JsonValue, context: &JsonValue) -> Option<Axis> {
    let v = field(input, "comparison_axis");
    if v.as_object().is_some() {
        return axis_value(v);
    }
    let reference = v.as_str()?;
    let definition = field(context, "comparison_axis_definition");
    if string(definition, "reference") != Some(reference) {
        return None;
    }
    axis_value(definition)
}
fn boundary_profile<'a>(
    input: &'a JsonValue,
    context: &'a JsonValue,
) -> Result<Option<&'a str>, &'static str> {
    let mut selected = None;
    let axis = field(input, "comparison_axis");
    let definition = field(context, "comparison_axis_definition");
    let context_profile = if axis
        .as_str()
        .is_some_and(|r| string(definition, "reference") == Some(r))
    {
        string(definition, "boundary_profile")
    } else {
        None
    };
    for profile in [
        string(input, "boundary_profile"),
        string(axis, "boundary_profile"),
        context_profile,
    ]
    .into_iter()
    .flatten()
    {
        if selected.is_some_and(|prior| prior != profile) {
            return Err("ConflictingBoundaryProfile");
        }
        selected = Some(profile);
    }
    Ok(selected)
}
fn operand_axis(v: &JsonValue, axis: &Axis) -> Result<(), &'static str> {
    if v.object_get("qualifier")
        .is_some_and(|q| q.as_str() != Some("None"))
    {
        return Err("NonExactPosition");
    }
    for key in ["trs_ref", "unit", "origin"] {
        if let Some(value) = v.object_get(key) {
            let expected = match key {
                "trs_ref" => axis.trs.id.as_str(),
                "unit" => axis.unit.as_str(),
                _ => axis.origin.as_str(),
            };
            if value.as_str() != Some(expected) {
                return Err("IncompatibleAxes");
            }
        }
    }
    if let Some(value) = v.object_get("axis") {
        if axis_value(value).as_ref() != Some(axis) {
            return Err("IncompatibleAxes");
        }
    }
    if let Some(value) = v.object_get("trs_version") {
        if value.as_str() != Some(axis.trs.version.as_str()) {
            return Err("IncompatibleAxes");
        }
    }
    Ok(())
}
fn exact(
    v: &JsonValue,
    axis: &Axis,
    profile: Result<Option<&str>, &'static str>,
) -> Result<ExactExtent, &'static str> {
    let profile = profile?;
    operand_axis(v, axis)?;
    if string(v, "type") == Some("Instant") {
        return Ok(ExactExtent::Instant {
            axis: axis.clone(),
            position: integer(field(v, "position")).ok_or("ExactEndpointProfileRequired")?,
        });
    }
    let v = if field(v, "extent").is_null() {
        v
    } else {
        field(v, "extent")
    };
    operand_axis(v, axis)?;
    let inclusivity = |key| {
        let rule = match profile {
            Some("ExactClosedIntervalsV1") => Some(Inclusivity::Inclusive),
            Some("ExactHalfOpenIntervalsV1") => Some(if key == "beginning_inclusivity" {
                Inclusivity::Inclusive
            } else {
                Inclusivity::Exclusive
            }),
            Some(_) => return Err("UnsupportedBoundaryProfile"),
            None => None,
        };
        let declared = match string(v, key) {
            Some("Inclusive") => Some(Inclusivity::Inclusive),
            Some("Exclusive") => Some(Inclusivity::Exclusive),
            Some("Unspecified") => Some(Inclusivity::Unspecified),
            None => None,
            Some(_) => return Err("InvalidBoundaryInclusivity"),
        };
        if let (Some(a), Some(b)) = (rule, declared) {
            if a != b {
                return Err("ConflictingBoundaryProfile");
            }
        }
        Ok(declared.or(rule).unwrap_or(Inclusivity::Unspecified))
    };
    Ok(ExactExtent::Interval {
        axis: axis.clone(),
        beginning: integer(field(v, "start")).ok_or("ExactEndpointProfileRequired")?,
        end: integer(field(v, "end")).ok_or("ExactEndpointProfileRequired")?,
        beginning_inclusivity: inclusivity("beginning_inclusivity")?,
        end_inclusivity: inclusivity("end_inclusivity")?,
    })
}
fn local_instant(s: &str) -> Option<(super::calendar::Civil, u32)> {
    let (d, t) = s.split_once('T')?;
    let (date, res) = super::calendar::parse_civil(d)?;
    if res != "day" {
        return None;
    }
    let p = t.split(':').collect::<Vec<_>>();
    if p.len() != 3
        || p.iter()
            .any(|p| p.len() != 2 || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let (h, m, sec) = (
        p[0].parse::<u32>().ok()?,
        p[1].parse::<u32>().ok()?,
        p[2].parse::<u32>().ok()?,
    );
    if h > 23 || m > 59 || sec > 59 {
        return None;
    }
    Some((date, h * 3600 + m * 60 + sec))
}
/// The finite alternative profile admits independent candidates only. A named
/// profile can supply the declaration when absent, never override explicit data.
#[derive(Clone, Copy)]
enum AlternativeDependence {
    Independent,
    Correlated,
    Unknown,
}
fn alternative_dependence(input: &JsonValue) -> AlternativeDependence {
    match input.object_get("alternative_dependence") {
        Some(v) => match v.as_str() {
            Some("Independent") => AlternativeDependence::Independent,
            Some("Correlated") => AlternativeDependence::Correlated,
            _ => AlternativeDependence::Unknown,
        },
        None if string(input, "alternative_profile") == Some("IndependentAlternativesV1") => {
            AlternativeDependence::Independent
        }
        None => AlternativeDependence::Unknown,
    }
}
fn nonempty_dependency(v: &JsonValue) -> bool {
    if v.is_null() {
        false
    } else if let Some(s) = v.as_str() {
        !s.trim().is_empty()
    } else if let Some(a) = v.as_array() {
        !a.is_empty()
    } else if let Some(o) = v.as_object() {
        !o.is_empty()
    } else {
        true
    }
}
pub(super) fn relate_json(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> JsonValue {
    let (a, b) = (field(input, "left"), field(input, "right"));
    let alternatives = string(input, "quantifier") == Some("for_all_members")
        || [a, b]
            .iter()
            .any(|v| string(v, "type") == Some("ExactlyOneOf"));
    if !alternatives {
        return relate_json_inner(input, context, budget);
    }
    let mut result = if budget.max_pairings == 0 {
        reason("Undetermined", "ResourceLimit")
    } else {
        match alternative_dependence(input) {
            AlternativeDependence::Correlated => {
                reason("Unsupported", "CorrelatedAlternativesNotImplemented")
            }
            AlternativeDependence::Unknown => reason("Unresolved", "AlternativeDependenceUnknown"),
            AlternativeDependence::Independent => {
                let contradictory = [input, a, b].iter().any(|v| {
                    [
                        "correlation_ref",
                        "world_constraints",
                        "dependency_refs",
                        "joint_worlds",
                    ]
                    .iter()
                    .any(|key| nonempty_dependency(field(v, key)))
                        || v.object_get("alternative_dependence")
                            .is_some_and(|d| d.as_str() != Some("Independent"))
                });
                if contradictory {
                    reason("Unsupported", "ConflictingAlternativeDependence")
                } else {
                    relate_json_inner(input, context, budget)
                }
            }
        }
    };
    // The dependency/world description is evidence, not an expansion request.
    put(&mut result, "preserved_input", input.clone());
    result
}
fn relate_json_inner(input: &JsonValue, context: &JsonValue, budget: TemporalBudget) -> JsonValue {
    let (a, b) = (field(input, "left"), field(input, "right"));
    if string(a, "type") == Some("UnknownExtent") || string(b, "type") == Some("UnknownExtent") {
        return reason("Unresolved", "UnknownExtent");
    }
    if !field(input, "base").is_null() {
        let Some(axis) = explicit_axis(input, context) else {
            return reason("Unresolved", "UnknownTRS");
        };
        let calc = |pair: &JsonValue| {
            let a = match exact(field(pair, "left"), &axis, boundary_profile(input, context)) {
                Ok(v) => v,
                Err(e) => return RelationOutcome::Incomparable(e),
            };
            let b = match exact(
                field(pair, "right"),
                &axis,
                boundary_profile(input, context),
            ) {
                Ok(v) => v,
                Err(e) => return RelationOutcome::Incomparable(e),
            };
            relate_exact_extents(&a, &b)
        };
        let base = calc(field(input, "base"));
        let renamed = calc(field(input, "renamed"));
        let mut r = relation_json(base.clone());
        if let (RelationOutcome::Entailed(a), RelationOutcome::Entailed(b)) = (base, renamed) {
            put(
                &mut r,
                "metamorphic_assertion",
                obj(vec![
                    (
                        "same_relation_after_entity_id_and_label_renaming",
                        JsonValue::Bool(a == b),
                    ),
                    ("base_relation", text(a.as_str())),
                    ("renamed_relation", text(b.as_str())),
                ]),
            );
        }
        return r;
    }
    if let Some(axis) = explicit_axis(input, context) {
        let a = match exact(a, &axis, boundary_profile(input, context)) {
            Ok(v) => v,
            Err(e) => return reason("Incomparable", e),
        };
        let b = match exact(b, &axis, boundary_profile(input, context)) {
            Ok(v) => v,
            Err(e) => return reason("Incomparable", e),
        };
        return relation_json(relate_exact_extents(&a, &b));
    }
    if let Err(r) = compatible(a, b) {
        return reason("Incomparable", r);
    }
    if [a, b].iter().any(|v| {
        v.object_get("qualifier")
            .is_some_and(|q| q.as_str() != Some("None"))
    }) {
        return reason("Unresolved", "NonExactPosition");
    }
    if string(a, "type") == Some("Instant") && string(b, "type") == Some("Instant") {
        let trs = string(a, "trs_ref").expect("compatible checked");
        if let Err(e) = super::calendar::declared_rules(input, trs, context) {
            return reason("Unresolved", e);
        }
        for operand in [a, b] {
            if super::calendar::admitted_rules(trs, context).is_ok() {
                if let Err(e) = super::calendar::declared_rules(operand, trs, context) {
                    return reason("Incomparable", e);
                }
            }
        }

        for operand in [a, b] {
            if let Some(c) = operand.object_get("calendar") {
                if c.as_str() != Some("GregorianProleptic") {
                    return reason("Incomparable", "IncompatibleCalendar");
                }
            }
        }
        if string(a, "zone").is_none() || string(a, "zone") != string(b, "zone") {
            return reason("Unresolved", "ZoneRulesRequired");
        }
        // Equal explicit fixed UTC offset is the supported lexical clock
        // profile. Named civil zones require the separate zone adapter.
        if string(a, "zone") != Some("UTC") {
            return reason("Unsupported", "ZoneResolverNotImplemented");
        }
        if let (Some(a), Some(b)) = (
            string(a, "local").and_then(local_instant),
            string(b, "local").and_then(local_instant),
        ) {
            let mut result = relation_json(RelationOutcome::Entailed(match a.cmp(&b) {
                Ordering::Less => Relation::Before,
                Ordering::Equal => Relation::Equals,
                Ordering::Greater => Relation::After,
            }));
            if let Ok(rules) = super::calendar::admitted_rules(trs, context) {
                put(
                    &mut result,
                    "trs_rules_version",
                    field(rules, "rules_version").clone(),
                );
            }
            return result;
        }
        return reason("Unresolved", "MalformedExpression");
    }
    if string(input, "boundary_policy") == Some("GregorianDayBucket") {
        for operand in [a, b] {
            if let Some(c) = operand.object_get("calendar") {
                if c.as_str() != Some("GregorianProleptic") {
                    return reason("Incomparable", "IncompatibleCalendar");
                }
            }
            if matches!(
                string(operand, "qualifier"),
                Some("Approximate" | "Uncertain" | "UncertainAndApproximate")
            ) {
                return reason("Unresolved", "NonExactPosition");
            }
        }
        let parse = |v: &JsonValue| -> Option<super::calendar::Civil> {
            if string(v, "resolution") != Some("day") {
                return None;
            }
            let (c, res) = super::calendar::parse_civil(string(v, "date")?)?;
            (res == "day").then_some(c)
        };
        if let (Some(a), Some(b)) = (parse(a), parse(b)) {
            let mut r = relation_json(RelationOutcome::Entailed(match a.cmp(&b) {
                Ordering::Less => Relation::Before,
                Ordering::Equal => Relation::Equals,
                Ordering::Greater => Relation::After,
            }));
            put(&mut r, "relation_kind", text("calendar-position-buckets"));
            return r;
        }
    }
    // Finite numeric possibilities are an explicitly declared ordinal-rank
    // profile; they are not calendar instants or elapsed durations.
    if string(input, "quantifier") == Some("for_all_members") {
        let Some(values) = field(a, "positions").as_array() else {
            return reason("Unsupported", "FiniteOrdinalPositionsRequired");
        };
        if string(a, "type") != Some("ExactlyOneOf") {
            return reason("Unsupported", "AlternativeCardinalityProfileRequired");
        }
        let two_sets = string(b, "type") == Some("ExactlyOneOf");
        let right_values: &[JsonValue] = if two_sets {
            let Some(values) = field(b, "positions").as_array() else {
                return reason("Unsupported", "FiniteOrdinalPositionsRequired");
            };
            values
        } else {
            std::slice::from_ref(field(b, "position"))
        };
        if values.is_empty() || right_values.is_empty() {
            return reason("Unresolved", "EmptyPossibilities");
        }
        if values
            .len()
            .checked_mul(right_values.len())
            .is_none_or(|count| count > budget.max_pairings)
        {
            return reason("Undetermined", "ResourceLimit");
        }
        let ordinal = string(input, "comparison_profile") == Some("OrdinalRanksV1");
        let trs = string(a, "trs_ref").expect("compatible checked");
        for operand in [a, b] {
            // OrdinalRanksV1 denotes exact ranks, never representatives of
            // coarse or uncertain positions. Width inference is not supported.
            if operand.object_get("uncertainty").is_some()
                || operand.object_get("resolution").is_some_and(|r| {
                    r.as_str().is_none() || (ordinal && r.as_str() != Some("exact"))
                })
            {
                return reason("Unresolved", "NonExactPosition");
            }
            if let Some(q) = operand.object_get("qualifier") {
                if q.as_str() != Some("None") {
                    return reason("Unresolved", "NonExactPosition");
                }
            }
        }
        if !ordinal {
            if super::calendar::admitted_rules(trs, context).is_err() {
                return reason("Unsupported", "ExplicitOrderedRankProfileRequired");
            }
            for p in [input, a, b] {
                if let Err(e) = super::calendar::declared_rules(p, trs, context) {
                    return reason("Incomparable", e);
                }
            }
        }

        let mut relations = vec![];
        let mut members = JsonValue::Object(vec![]);
        let mut pairs = vec![];
        for value in values {
            let Some(left) = integer(value) else {
                return reason("Unsupported", "OrdinalRankRequired");
            };
            for right_value in right_values {
                let Some(right) = integer(right_value) else {
                    return reason("Unsupported", "OrdinalRankRequired");
                };
                // The same pairwise rank comparison serves scalar and finite
                // independent-set inputs. Calendar buckets retain their width.
                let r = match left.cmp(&right) {
                    Ordering::Less => Relation::Before,
                    Ordering::Equal => Relation::Equals,
                    Ordering::Greater => Relation::After,
                };
                let pair_relations = if !ordinal && r == Relation::Equals {
                    vec![Relation::Before, Relation::Equals, Relation::After]
                } else {
                    vec![r]
                };
                for possible in &pair_relations {
                    if !relations.contains(possible) {
                        relations.push(*possible);
                    }
                }
                let encoded =
                    JsonValue::Array(pair_relations.iter().map(|r| text(r.as_str())).collect());
                if two_sets {
                    pairs.push(obj(vec![
                        ("left", value.clone()),
                        ("right", right_value.clone()),
                        ("relations", encoded),
                    ]));
                } else {
                    put(
                        &mut members,
                        &left.to_string(),
                        if pair_relations.len() == 1 {
                            text(r.as_str())
                        } else {
                            encoded
                        },
                    );
                }
            }
        }
        let mut r = relation_json(if relations.len() == 1 {
            RelationOutcome::Entailed(relations[0])
        } else {
            RelationOutcome::Possible(relations)
        });
        put(&mut r, "quantifier", text("for_all_members"));
        if two_sets {
            put(&mut r, "pair_relations", JsonValue::Array(pairs));
        } else {
            put(&mut r, "member_relations", members);
        }
        return r;
    }
    reason("Unsupported", "ExtentProfileNotImplemented")
}
pub(super) fn relate_claims_json(
    input: &JsonValue,
    _context: &JsonValue,
    _budget: TemporalBudget,
) -> JsonValue {
    let (a, b) = (field(input, "claim_a"), field(input, "claim_b"));
    let trs = string(input, "trs_ref");
    if trs.is_none_or(|s| s.trim().is_empty()) {
        return reason("Incomparable", "UnknownTRS");
    }
    for claim in [a, b] {
        if let Some(t) = claim.object_get("trs_ref") {
            if t.as_str() != trs {
                return reason("Incomparable", "NoCrosswalk");
            }
        }
    }
    // Independent instants merely known to the same nonzero date bucket.
    // This is not the relation between two actual year/day-long events.
    if string(a, "kind") == Some("occurrence-position")
        && string(b, "kind") == Some("occurrence-position")
        && string(a, "entity_extent") == Some("Instant")
        && string(b, "entity_extent") == Some("Instant")
        && field(a, "position") == field(b, "position")
        && field(a, "resolution") == field(b, "resolution")
        && matches!(string(a, "resolution"), Some("year" | "month" | "day"))
        && string(a, "position").is_some_and(|s| !s.is_empty())
    {
        let mut r = relation_json(RelationOutcome::Possible(vec![
            Relation::Before,
            Relation::Equals,
            Relation::After,
        ]));
        put(
            &mut r,
            "basis",
            text("independent-point-instants-anywhere-in-same-day-bucket"),
        );
        if !field(input, "source_assertion").is_null() {
            put(
                &mut r,
                "source_assertion",
                field(input, "source_assertion").clone(),
            );
            put(&mut r, "derivation_outcome", text("Possible"));
        }
        return r;
    }
    reason("Unsupported", "ClaimRelationProfileNotImplemented")
}
