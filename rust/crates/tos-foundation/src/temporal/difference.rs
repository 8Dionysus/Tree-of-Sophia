//! Pure exact-coordinate difference; no calendar/zone conversion or writes.
//! I64 endpoints are subtracted in i128 and emitted as an exact decimal string.
use super::{
    Duration, Position, Resolution, TemporalBudget, TemporalError, TrsRef, field, obj, put, reason,
    string, text,
};
use crate::{JsonNumberKind, JsonValue};

const RULE: &str = "tos-exact-coordinate-difference-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NegativeDurationPolicy {
    Signed,
    Reject,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DifferenceKind {
    Elapsed {
        negative_policy: Option<NegativeDurationPolicy>,
    },
    Calendar {
        unit: String,
        arithmetic_policy_ref: Option<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DifferenceOutcome {
    Resolved(Duration),
    Incomparable(&'static str),
    Unresolved(&'static str),
    Unsupported(&'static str),
    PolicyRequired(&'static str),
    Reversed,
}
fn trs(p: &Position) -> Option<&TrsRef> {
    match p {
        Position::Calendar { trs, .. } => trs.as_ref(),
        Position::Coordinate { trs, .. } | Position::Ordinal { trs, .. } => Some(trs),
    }
}
fn known(t: &TrsRef) -> bool {
    !t.id.trim().is_empty() && !t.version.trim().is_empty()
}

/// Difference is signed only when explicitly requested for reversed operands.
/// No unit conversion, origin shift, abs(), endpoint swap or clock default.
pub fn difference(
    end: &Position,
    beginning: &Position,
    kind: &DifferenceKind,
) -> DifferenceOutcome {
    use DifferenceOutcome::*;
    let (Some(et), Some(bt)) = (trs(end), trs(beginning)) else {
        return Unresolved("UnknownTRS");
    };
    if !known(et) || !known(bt) {
        return Unresolved("UnknownTRS");
    }
    if et != bt {
        return Incomparable("NoCrosswalk");
    }
    if matches!(end, Position::Ordinal { .. }) || matches!(beginning, Position::Ordinal { .. }) {
        return Unsupported("NonMetricScale");
    }
    if let DifferenceKind::Calendar {
        unit,
        arithmetic_policy_ref,
    } = kind
    {
        if unit.trim().is_empty() {
            return Unresolved("CalendarUnitRequired");
        }
        if arithmetic_policy_ref
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        {
            return PolicyRequired("CalendarPolicyRequired");
        }
        return Unsupported("CalendarDifferenceProfileNotImplemented");
    }
    let (
        Position::Coordinate {
            value: e,
            unit: eu,
            origin: eo,
            ..
        },
        Position::Coordinate {
            value: b,
            unit: bu,
            origin: bo,
            ..
        },
    ) = (end, beginning)
    else {
        return Unresolved("ElapsedScaleRequired");
    };
    if eu.trim().is_empty() || bu.trim().is_empty() {
        return Unresolved("UnitRequired");
    }
    if eo.trim().is_empty() || bo.trim().is_empty() {
        return Unresolved("OriginRequired");
    }
    if eu != bu {
        return Incomparable("IncompatibleUnits");
    }
    if eo != bo {
        return Incomparable("IncompatibleOrigins");
    }
    // Full endpoint-domain difference fits in i128: magnitude <= 2^64 - 1.
    let delta = i128::from(*e) - i128::from(*b);
    if delta < 0 {
        match kind {
            DifferenceKind::Elapsed {
                negative_policy: None,
            } => return PolicyRequired("NegativeDurationPolicyRequired"),
            DifferenceKind::Elapsed {
                negative_policy: Some(NegativeDurationPolicy::Reject),
            } => return Reversed,
            _ => {}
        }
    }
    Resolved(Duration::Elapsed {
        magnitude: delta.to_string(),
        unit: eu.clone(),
        scale: et.clone(),
    })
}

fn reference(value: &JsonValue) -> Option<TrsRef> {
    let id = string(value, "id")?;
    let version = string(value, "version")?;
    let t = TrsRef {
        id: id.into(),
        version: version.into(),
    };
    known(&t).then_some(t)
}
fn position(value: &JsonValue) -> Result<Position, DifferenceOutcome> {
    use DifferenceOutcome::*;
    if value.as_object().is_none() {
        return Err(Unresolved("MissingPosition"));
    }
    if value
        .object_get("qualifier")
        .is_some_and(|q| q.as_str() != Some("None"))
    {
        return Err(Unresolved("NonExactPosition"));
    }
    match string(value, "type") {
        Some("CoordinatePosition") => {
            if value
                .object_get("resolution")
                .is_some_and(|r| r.as_str() != Some("exact"))
                || value
                    .object_get("uncertainty")
                    .is_some_and(|u| !u.is_null())
            {
                return Err(Unresolved("NonExactPosition"));
            }
            let axis = field(value, "axis");
            let t = reference(field(axis, "trs")).ok_or(Unresolved("UnknownTRS"))?;
            let unit = string(axis, "unit")
                .filter(|s| !s.trim().is_empty())
                .ok_or(Unresolved("UnitRequired"))?;
            let origin = string(axis, "origin")
                .filter(|s| !s.trim().is_empty())
                .ok_or(Unresolved("OriginRequired"))?;
            for key in ["unit", "origin"] {
                if let Some(declared) = value.object_get(key) {
                    if declared.as_str() != string(axis, key) {
                        return Err(Incomparable("ConflictingAxisDeclaration"));
                    }
                }
            }
            if let Some(declared) = value.object_get("trs_ref") {
                if reference(declared).as_ref() != Some(&t) {
                    return Err(Incomparable("ConflictingAxisDeclaration"));
                }
            }
            let JsonValue::Number(n) = field(value, "value") else {
                return Err(Unsupported("ExactIntegerCoordinateRequired"));
            };
            if n.kind != JsonNumberKind::Int {
                return Err(Unsupported("ExactIntegerCoordinateRequired"));
            }
            let value = n
                .lexeme
                .parse::<i64>()
                .map_err(|_| Unsupported("CoordinateOutOfRange"))?;
            Ok(Position::Coordinate {
                value,
                unit: unit.into(),
                origin: origin.into(),
                trs: t,
            })
        }
        Some("CalendarPosition" | "CalendarPositionBucket") => {
            let resolution = match string(value, "resolution") {
                Some("year") => Resolution::Year,
                Some("month") => Resolution::Month,
                Some("day") => Resolution::Day,
                _ => Resolution::Unknown,
            };
            Ok(Position::Calendar {
                lexical: string(value, "lexical")
                    .or_else(|| string(value, "position"))
                    .unwrap_or("")
                    .into(),
                resolution,
                trs: reference(field(value, "trs_ref")),
                era: string(value, "era_ref").map(str::to_owned),
            })
        }
        Some(
            "Unknown" | "UnknownBoundary" | "Unspecified" | "NoBoundDeclared" | "Unbounded"
            | "NotApplicable",
        ) => Err(Unresolved("UnknownBoundary")),
        _ => Err(Unsupported("PositionKindNotImplemented")),
    }
}
fn envelope(outcome: DifferenceOutcome) -> JsonValue {
    match outcome {
        DifferenceOutcome::Resolved(Duration::Elapsed {
            magnitude,
            unit,
            scale,
        }) => obj(vec![
            ("status", text("Resolved")),
            (
                "duration",
                obj(vec![
                    ("kind", text("Elapsed")),
                    ("magnitude", text(&magnitude)),
                    ("unit", text(&unit)),
                    (
                        "timescale_ref",
                        obj(vec![
                            ("id", text(&scale.id)),
                            ("version", text(&scale.version)),
                        ]),
                    ),
                ]),
            ),
        ]),
        DifferenceOutcome::Resolved(_) => reason("Unsupported", "DurationKindNotImplemented"),
        DifferenceOutcome::Incomparable(r) => reason("Incomparable", r),
        DifferenceOutcome::Unresolved(r) => reason("Unresolved", r),
        DifferenceOutcome::Unsupported(r) => reason("Unsupported", r),
        DifferenceOutcome::PolicyRequired(r) => reason("PolicyRequired", r),
        DifferenceOutcome::Reversed => reason("Reversed", "EndBeforeBeginning"),
    }
}

/// Wire adapter with immutable input echo. Context cannot replace operand axes.
/// No unbounded computation/allocation: only two fixed-width coordinates.
pub fn execute_difference(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    if input.as_object().is_none() || context.as_object().is_none() {
        return Err(TemporalError::MalformedRequest(
            "difference input/context must be objects",
        ));
    }
    let finish = |o| {
        let mut r = envelope(o);
        put(&mut r, "rule_version", text(RULE));
        put(&mut r, "preserved_input", input.clone());
        r
    };
    if budget.max_pairings == 0 {
        return Err(TemporalError::ResourceExhausted(
            "difference pair budget exhausted",
        ));
    }
    let end = match position(field(input, "end")) {
        Ok(v) => v,
        Err(e) => return Ok(finish(e)),
    };
    let beginning = match position(field(input, "beginning")) {
        Ok(v) => v,
        Err(e) => return Ok(finish(e)),
    };
    let kind = match string(input, "duration_kind") {
        Some("Elapsed") => {
            let p = match string(input, "negative_policy") {
                None => None,
                Some("Signed") => Some(NegativeDurationPolicy::Signed),
                Some("Reject") => Some(NegativeDurationPolicy::Reject),
                Some(_) => {
                    return Err(TemporalError::MalformedRequest(
                        "unknown negative duration policy",
                    ));
                }
            };
            DifferenceKind::Elapsed { negative_policy: p }
        }
        Some("Calendar") => DifferenceKind::Calendar {
            unit: string(input, "calendar_unit").unwrap_or("").into(),
            arithmetic_policy_ref: string(input, "arithmetic_policy_ref").map(str::to_owned),
        },
        _ => {
            return Err(TemporalError::MalformedRequest(
                "duration_kind must be Elapsed or Calendar",
            ));
        }
    };
    Ok(finish(difference(&end, &beginning, &kind)))
}
