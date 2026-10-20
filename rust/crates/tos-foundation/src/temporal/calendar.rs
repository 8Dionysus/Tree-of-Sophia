use super::*;

pub(super) fn days_in_month(y: i64, m: u8) -> Option<u8> {
    Some(match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y.rem_euclid(4) == 0 && (y.rem_euclid(100) != 0 || y.rem_euclid(400) == 0) {
                29
            } else {
                28
            }
        }
        _ => return None,
    })
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct Civil(pub i64, pub u8, pub u8);
pub(super) fn parse_civil(s: &str) -> Option<(Civil, &'static str)> {
    let p = s.split('-').collect::<Vec<_>>();
    if p.is_empty() || p.len() > 3 || p[0].len() != 4 || !p[0].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let y = p[0].parse().ok()?;
    if p.iter()
        .skip(1)
        .any(|s| s.len() != 2 || !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let m = if p.len() > 1 { p[1].parse().ok()? } else { 1 };
    let d = if p.len() > 2 { p[2].parse().ok()? } else { 1 };
    if d == 0 || d > days_in_month(y, m)? {
        return None;
    }
    Some((
        Civil(y, m, d),
        match p.len() {
            1 => "year",
            2 => "month",
            _ => "day",
        },
    ))
}
pub(super) fn next(c: Civil, res: &str) -> Option<Civil> {
    let Civil(y, m, d) = c;
    match res {
        "year" => Some(Civil(y.checked_add(1)?, 1, 1)),
        "month" => Some(if m == 12 {
            Civil(y.checked_add(1)?, 1, 1)
        } else {
            Civil(y, m + 1, 1)
        }),
        "day" => Some(if d < days_in_month(y, m)? {
            Civil(y, m, d + 1)
        } else if m < 12 {
            Civil(y, m + 1, 1)
        } else {
            Civil(y.checked_add(1)?, 1, 1)
        }),
        _ => None,
    }
}
pub(super) fn lexical(c: Civil) -> String {
    format!("{:04}-{:02}-{:02}", c.0, c.1, c.2)
}

/// Resolve an opaque exact TRS key against caller-admitted structured rules.
/// Labels and prose never select a calendar algorithm or numbering convention.
pub(super) fn admitted_rules<'a>(
    trs: &str,
    context: &'a JsonValue,
) -> Result<&'a JsonValue, &'static str> {
    let entry = field(context, "temporal_systems")
        .object_get(trs)
        .ok_or("CalendarRulesRequired")?;
    if string(entry, "calendar_algorithm") != Some("GregorianProleptic") {
        return Err("UnsupportedCalendar");
    }
    if string(entry, "year_numbering") != Some("astronomical") {
        return Err("UnsupportedYearNumbering");
    }
    if string(entry, "rules_version").is_none_or(|s| s.trim().is_empty()) {
        return Err("RuleVersionRequired");
    }
    Ok(entry)
}
pub(super) fn declared_rules(
    input: &JsonValue,
    trs: &str,
    context: &JsonValue,
) -> Result<(), &'static str> {
    if let Some(entry) = field(context, "temporal_systems").object_get(trs) {
        admitted_rules(trs, context)?;
        for (key, rule_key) in [
            ("calendar", "calendar_algorithm"),
            ("calendar_algorithm", "calendar_algorithm"),
            ("year_numbering", "year_numbering"),
            ("rules_version", "rules_version"),
        ] {
            if let Some(declared) = input.object_get(key) {
                if declared != field(entry, rule_key) {
                    return Err("ConflictingCalendarDeclaration");
                }
            }
        }
        return Ok(());
    }
    match string(input, "calendar") {
        Some("GregorianProleptic") => Ok(()),
        Some(_) => Err("UnsupportedCalendar"),
        None => Err("CalendarRulesRequired"),
    }
}

pub(super) fn resolve(input: &JsonValue, context: &JsonValue) -> JsonValue {
    if string(input, "zone_ref").is_some() || string(input, "local_date_time").is_some() {
        return reason("Unsupported", "ZoneResolverNotImplemented");
    }
    let raw = string(input, "expression").unwrap_or("");
    let trs = field(input, "trs_ref");
    let trs = if trs.is_null() {
        field(context, "trs_ref")
    } else {
        trs
    };
    let mut result = if trs.is_null() {
        reason("Unresolved", "UnknownTRS")
    } else if let Some(reference) = trs.as_str() {
        match declared_rules(input, reference, context) {
            Err(e) => reason("Unresolved", e),
            Ok(()) => match parse_civil(raw) {
                Some((_, resolution)) => obj(vec![
                    ("status", text("Resolved")),
                    (
                        "position",
                        obj(vec![
                            ("type", text("CalendarPosition")),
                            ("lexical", text(raw)),
                            ("resolution", text(resolution)),
                            ("trs_ref", trs.clone()),
                        ]),
                    ),
                ]),
                None => reason("Unresolved", "MalformedExpression"),
            },
        }
    } else {
        reason("Unsupported", "ExplicitCalendarRulesRequired")
    };
    put(&mut result, "preserved_raw", text(raw));
    put(&mut result, "implicit_zone", JsonValue::Null);
    put(&mut result, "utc_conversion", JsonValue::Null);
    result
}
/// Minimal civil-date adapter. Choosing the algorithm requires an explicit
/// GregorianProleptic calendar declaration; the TRS identity is opaque data.
pub(super) fn derive(input: &JsonValue, context: &JsonValue) -> JsonValue {
    if !field(input, "season_boundary_rule").is_null() || !field(input, "position").is_null() {
        let p = field(input, "position");
        if p.object_get("season_code").is_some() {
            let mut r = reason(
                "Unsupported",
                if field(input, "season_boundary_rule").is_null() {
                    "SeasonBoundaryRuleNotSupplied"
                } else {
                    "UnsupportedSeasonBoundaryRule"
                },
            );
            put(&mut r, "preserved_position", p.clone());
            return r;
        }
    }
    if string(input, "type") == Some("ActualInterval") {
        let (a, b) = (field(input, "start"), field(input, "end"));
        let (Some(at), Some(bt)) = (string(a, "trs_ref"), string(b, "trs_ref")) else {
            return reason("Unresolved", "UnknownTRS");
        };
        if at != bt {
            return reason("Incomparable", "NoCrosswalk");
        }
        if [input, a, b].iter().any(|v| {
            v.object_get("qualifier")
                .is_some_and(|q| q.as_str() != Some("None"))
        }) {
            return reason("Unresolved", "NonExactPosition");
        }
        if let Err(e) = declared_rules(input, at, context) {
            return reason("Unsupported", e);
        }
        for p in [a, b] {
            if let Err(e) = declared_rules(p, at, context) {
                return reason("Unsupported", e);
            }
        }
        let (Some((start, start_res)), Some((end, end_res))) = (
            string(a, "position").and_then(parse_civil),
            string(b, "position").and_then(parse_civil),
        ) else {
            return reason("Unresolved", "MalformedExpression");
        };
        if string(a, "resolution") != Some(start_res) || string(b, "resolution") != Some(end_res) {
            return reason("Unresolved", "ResolutionMismatch");
        }
        if next(end, end_res).is_some_and(|upper| start >= upper) {
            return obj(vec![
                ("status", text("ContradictoryBounds")),
                ("normalized_extent", JsonValue::Null),
                ("source_input_preserved", JsonValue::Bool(true)),
                ("preserved_input", input.clone()),
            ]);
        }
        return reason("Unresolved", "ActualBoundaryPrecisionRequired");
    }
    let Some(raw) = string(input, "expression") else {
        return reason("Unresolved", "MissingPosition");
    };
    let trs = field(context, "trs_ref");
    let trs = if trs.is_null() {
        field(input, "trs_ref")
    } else {
        trs
    };
    if trs.as_str().is_none() {
        return reason("Unresolved", "UnknownTRS");
    }
    if let Err(e) = declared_rules(input, trs.as_str().expect("checked"), context) {
        return reason("Unsupported", e);
    }
    let Some((start, res)) = parse_civil(raw) else {
        return reason("Unresolved", "MalformedExpression");
    };
    if string(input, "resolution") != Some(res) {
        return reason("Unresolved", "ResolutionMismatch");
    }
    let Some(end) = next(start, res) else {
        return reason("Unsupported", "CalendarOutOfCoverage");
    };
    let mut value = obj(vec![
        ("type", text("CalendarPositionBucket")),
        ("start", text(&lexical(start))),
        ("end_exclusive", text(&lexical(end))),
        ("resolution", text(res)),
        ("trs_ref", trs.clone()),
    ]);
    if let Some(k) = string(input, "claim_kind") {
        put(&mut value, "claim_kind", text(k));
    }
    let mut result = obj(vec![
        ("status", text("Resolved")),
        ("value", value),
        ("preserved_raw", text(raw)),
        ("calendar_rules", text("proleptic-gregorian-civil-v1")),
        ("event_extent_inferred", JsonValue::Bool(false)),
    ]);
    if let Ok(rules) = admitted_rules(trs.as_str().expect("checked"), context) {
        put(
            &mut result,
            "trs_rules_version",
            field(rules, "rules_version").clone(),
        );
    }
    result
}
