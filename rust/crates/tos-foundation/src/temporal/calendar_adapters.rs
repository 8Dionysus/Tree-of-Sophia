//! Explicit, bounded adapters. Mapping tables and wall-time offsets are supplied
//! data, never registries inferred from labels. No IANA or historical-calendar
//! conformance is implied by a synthetic table.
use super::*;

fn finish(mut r: JsonValue, input: &JsonValue, context: &JsonValue, rule: &str) -> JsonValue {
    put(&mut r, "preserved_input", input.clone());
    put(&mut r, "preserved_context", context.clone());
    put(&mut r, "adapter_version", text(rule));
    if r.object_get("precision_loss").is_none() {
        put(&mut r, "precision_loss", JsonValue::Array(vec![]));
    }
    r
}
fn reference(v: &JsonValue) -> bool {
    v.as_str().is_some_and(|s| !s.is_empty())
        || (string(v, "id").is_some_and(|s| !s.is_empty())
            && string(v, "version").is_some_and(|s| !s.is_empty()))
}
// Claim source identity has a different wire from a TRS. Keep the shared TRS
// helper unchanged; numeric-version/digest Claims are strict exact references.
fn sidecar_claim_reference(v: &JsonValue) -> bool {
    if v.as_str().is_some() {
        return reference(v);
    }
    if field(v, "version").as_str().is_some() {
        // This stays a legacy opaque reference, including its extra metadata;
        // it is never promoted to the numeric-version exact owner wire.
        return reference(v);
    }
    v.as_object().is_some_and(|fields| fields.len() == 3)
        && ["id", "version", "digest"]
            .iter()
            .all(|key| v.object_get(key).is_some())
        && string(v, "id").is_some_and(|id| crate::StableId::parse(id).is_ok())
        && field(v, "version")
            .as_u64()
            .is_some_and(|version| crate::RecordVersion::new(version).is_ok())
        && string(v, "digest").is_some_and(|digest| crate::Digest256::from_prefixed(digest).is_ok())
}
fn budget_check(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<(), TemporalError> {
    // Public direct adapters share the complete aggregate admission used by
    // top-level dispatch, before any preserved-input/context clones.
    if input.as_object().is_none() || context.as_object().is_none() {
        return Err(TemporalError::MalformedRequest(
            "input and context must be objects",
        ));
    }
    super::admit_transport(input, context, budget)
}
pub fn add_duration(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    budget_check(input, context, budget)?;
    add_duration_admitted(input, context, budget)
}

// Parent dispatch has already admitted both roots; keep one semantic body.
pub(super) fn add_duration_admitted(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let done = |r| Ok(finish(r, input, context, "tos-calendar-month-v1"));
    let anchor = field(input, "anchor");
    if anchor.is_null() {
        return done(reason("Unresolved", "MissingAnchor"));
    }
    let duration = field(input, "duration");
    if duration.as_object().is_none() {
        return Err(TemporalError::MalformedRequest("duration object required"));
    }
    if string(duration, "kind") != Some("Calendar") || string(duration, "unit") != Some("month") {
        return done(reason("Unsupported", "UnsupportedDurationKindOrUnit"));
    }
    let n = integer(field(duration, "value")).ok_or(TemporalError::MalformedRequest(
        "integer duration value required",
    ))?;
    let policy = field(input, "arithmetic_policy");
    if policy.is_null() {
        return done(reason("PolicyRequired", "CalendarPolicyRequired"));
    }
    if string(policy, "calendar") != Some("GregorianProleptic") {
        return done(reason("Unsupported", "ExplicitCalendarRulesRequired"));
    }
    if !reference(field(policy, "policy_ref")) {
        return done(reason("PolicyRequired", "PolicyVersionRequired"));
    }
    if !reference(field(anchor, "trs_ref")) {
        return done(reason("Unresolved", "UnknownTRS"));
    }
    if string(anchor, "resolution") != Some("day") {
        return done(reason("Unsupported", "DayAnchorRequired"));
    }
    let raw = string(anchor, "position")
        .ok_or(TemporalError::MalformedRequest("anchor position required"))?;
    if raw.len() > budget.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted("expression byte limit"));
    }
    let Some((calendar::Civil(y, m, d), "day")) = calendar::parse_civil(raw) else {
        return done(reason("Unresolved", "UnsupportedAnchorLexicalForm"));
    };
    // Astronomical year zero is outside this historical adapter's first cut.
    if y < 1 {
        return done(reason("OutOfCoverage", "EraNumberingRequired"));
    }
    let Some(total) = y
        .checked_mul(12)
        .and_then(|v| v.checked_add(i64::from(m) - 1))
        .and_then(|v| v.checked_add(n))
    else {
        return done(reason("OutOfCoverage", "CalendarOverflow"));
    };
    let year = total.div_euclid(12);
    let month = (total.rem_euclid(12) + 1) as u8;
    if !(1..=9999).contains(&year) {
        return done(reason("OutOfCoverage", "CalendarYearOutsideSupportedRange"));
    }
    let last = calendar::days_in_month(year, month).unwrap();
    let day = match string(policy, "month_end") {
        Some("ClampToLastValidDay") => d.min(last),
        Some("RejectInvalidDay") if d <= last => d,
        Some("RejectInvalidDay") => return done(reason("Unresolved", "InvalidDayUnderPolicy")),
        _ => return done(reason("PolicyRequired", "CalendarPolicyRequired")),
    };
    let value = obj(vec![
        (
            "position",
            text(&calendar::lexical(calendar::Civil(year, month, day))),
        ),
        ("resolution", text("day")),
        ("trs_ref", field(anchor, "trs_ref").clone()),
    ]);
    done(obj(vec![
        ("status", text("Resolved")),
        ("value", value),
        ("policy_ref", field(policy, "policy_ref").clone()),
        (
            "calendar_rules_version",
            text("proleptic-gregorian-civil-v1"),
        ),
        ("month_end_adjusted", JsonValue::Bool(day != d)),
        ("elapsed_duration_inferred", JsonValue::Bool(false)),
    ]))
}

pub fn convert(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    budget_check(input, context, budget)?;
    convert_admitted(input, context, budget)
}

// Parent dispatch has already admitted both roots; keep one semantic body.
pub(super) fn convert_admitted(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let done = |r| Ok(finish(r, input, context, "tos-explicit-crosswalk-v1"));
    let p = field(input, "position");
    if !reference(field(p, "trs_ref")) || !reference(field(input, "target_trs")) {
        return done(reason("Unresolved", "UnknownTRS"));
    }
    if !reference(field(input, "crosswalk_version")) {
        return done(reason("NoCrosswalk", "CrosswalkVersionRequired"));
    }
    let map = field(input, "fixture_mapping");
    let Some(entries) = map.as_object() else {
        return done(reason("NoCrosswalk", "CrosswalkDataRequired"));
    };
    if entries.len() > budget.max_members {
        return Err(TemporalError::ResourceExhausted("mapping member limit"));
    }
    let mut coordinates = std::collections::BTreeSet::new();
    for (coordinate, _) in entries {
        if !coordinates.insert(coordinate.units()) {
            return Err(TemporalError::MalformedRequest(
                "duplicate crosswalk coordinate",
            ));
        }
    }
    let v = field(p, "value");
    let key = if let Some(s) = v.as_str() {
        s.to_owned()
    } else if let Some(n) = integer(v) {
        n.to_string()
    } else {
        return done(reason("Unsupported", "UnsupportedCrosswalkCoordinate"));
    };
    let Some(mapped) = map.object_get(&key) else {
        return done(reason("OutOfCoverage", "CrosswalkEntryAbsent"));
    };
    // A table row may be exact, a finite set, a partial record or explicitly
    // unsupported. Preserve precision-loss declarations instead of laundering
    // an approximate projection into Unique.
    let mut result = if mapped.is_null() {
        reason("Unsupported", "CrosswalkEntryUnsupported")
    } else if let Some(values) = mapped.as_array() {
        if values.len() > budget.max_members {
            return Err(TemporalError::ResourceExhausted("candidate member limit"));
        }
        if values.is_empty() {
            reason("OutOfCoverage", "EmptyCrosswalkEntry")
        } else {
            // A finite candidate set contains coordinates, not row-status
            // declarations. Validate the whole set before cloning any value.
            for value in values {
                if integer(value).is_none() && value.as_str().is_none() {
                    return Err(TemporalError::MalformedRequest(
                        "invalid mapping coordinate",
                    ));
                }
            }
            let candidates = values
                .iter()
                .map(|v| {
                    obj(vec![
                        ("value", v.clone()),
                        ("trs_ref", field(input, "target_trs").clone()),
                    ])
                })
                .collect();
            obj(vec![
                ("status", text("Ambiguous")),
                ("candidates", JsonValue::Array(candidates)),
            ])
        }
    } else if mapped.as_object().is_some() {
        match string(mapped, "status") {
            Some("Partial" | "Unsupported" | "PrecisionLoss") => mapped.clone(),
            _ => return done(reason("Unsupported", "UnsupportedCrosswalkEntryShape")),
        }
    } else if integer(mapped).is_some() || mapped.as_str().is_some() {
        obj(vec![
            ("status", text("Unique")),
            (
                "position",
                obj(vec![
                    ("value", mapped.clone()),
                    ("trs_ref", field(input, "target_trs").clone()),
                ]),
            ),
        ])
    } else {
        return Err(TemporalError::MalformedRequest(
            "invalid mapping coordinate",
        ));
    };
    put(
        &mut result,
        "crosswalk_version",
        field(input, "crosswalk_version").clone(),
    );
    put(&mut result, "source_trs", field(p, "trs_ref").clone());
    put(
        &mut result,
        "rules_source",
        text("supplied-mapping-not-calendar-conformance"),
    );
    if string(&result, "status") == Some("PrecisionLoss") {
        let losses = field(&result, "precision_loss").clone();
        let mut r = finish(result, input, context, "tos-explicit-crosswalk-v1");
        put(
            &mut r,
            "precision_loss",
            if losses.is_null() {
                JsonValue::Array(vec![text("DeclaredByCrosswalk")])
            } else {
                losses
            },
        );
        return Ok(r);
    }
    done(result)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Wall {
    civil: calendar::Civil,
    h: u8,
    m: u8,
    s: u8,
}
fn wall(raw: &str) -> Option<Wall> {
    let b = raw.as_bytes();
    if b.len() != 19 || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    if !b[11..13]
        .iter()
        .chain(b[14..16].iter())
        .chain(b[17..19].iter())
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let (civil, "day") = calendar::parse_civil(&raw[..10])? else {
        return None;
    };
    if civil.0 < 1 {
        return None;
    }
    let h = raw[11..13].parse().ok()?;
    let m = raw[14..16].parse().ok()?;
    let s = raw[17..19].parse().ok()?;
    if h > 23 || m > 59 || s > 59 {
        return None;
    }
    Some(Wall { civil, h, m, s })
}
fn offset(raw: &str) -> Option<i32> {
    let b = raw.as_bytes();
    if b.len() != 6
        || !matches!(b[0], b'+' | b'-')
        || b[3] != b':'
        || !b[1..3]
            .iter()
            .chain(b[4..6].iter())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let h: i32 = raw[1..3].parse().ok()?;
    let m: i32 = raw[4..6].parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some((h * 60 + m) * if b[0] == b'-' { -1 } else { 1 })
}
fn shift(w: Wall, offset: i32) -> Option<String> {
    let minute = i32::from(w.h) * 60 + i32::from(w.m) - offset;
    let mut c = w.civil;
    for _ in 0..minute.div_euclid(1440).unsigned_abs() {
        if minute < 0 {
            c = if c.2 > 1 {
                calendar::Civil(c.0, c.1, c.2 - 1)
            } else if c.1 > 1 {
                calendar::Civil(c.0, c.1 - 1, calendar::days_in_month(c.0, c.1 - 1)?)
            } else {
                calendar::Civil(c.0 - 1, 12, 31)
            };
        } else {
            c = calendar::next(c, "day")?;
        }
    }
    if !(1..=9999).contains(&c.0) {
        return None;
    }
    let minute = minute.rem_euclid(1440);
    Some(format!(
        "{}T{:02}:{:02}:{:02}Z",
        calendar::lexical(c),
        minute / 60,
        minute % 60,
        w.s
    ))
}
pub fn resolve_position(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    budget_check(input, context, budget)?;
    resolve_position_admitted(input, context, budget)
}

// Parent dispatch has already admitted both roots; keep one semantic body.
pub(super) fn resolve_position_admitted(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let done = |r| {
        Ok(finish(
            r,
            input,
            context,
            "tos-explicit-local-wall-fixture-v1",
        ))
    };
    if !reference(field(input, "zone_ref")) {
        return done(reason("Unresolved", "ZoneRequired"));
    }
    let f = field(input, "transition_fixture");
    if f.as_object().is_none() {
        return done(reason("Unsupported", "TransitionRulesRequired"));
    }
    if !reference(field(f, "fixture_id")) {
        return done(reason("Unresolved", "TransitionVersionRequired"));
    }
    if let (Some(input_calendar), Some(fixture_calendar)) =
        (string(input, "calendar"), string(f, "calendar"))
        && input_calendar != fixture_calendar
    {
        return done(reason("Unresolved", "CalendarDeclarationConflict"));
    }
    let algorithm = string(input, "calendar").or_else(|| string(f, "calendar"));
    if algorithm != Some("GregorianProleptic") {
        return done(reason("Unresolved", "ExplicitCalendarRulesRequired"));
    }
    let raw = string(input, "local_date_time")
        .ok_or(TemporalError::MalformedRequest("local_date_time required"))?;
    if raw.len() > budget.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted("expression byte limit"));
    }
    let Some(w) = wall(raw) else {
        return done(reason("Unsupported", "UnsupportedLocalWallLexicalForm"));
    };
    let Some(window) = field(f, "validity_window").as_array() else {
        return Err(TemporalError::MalformedRequest("validity_window required"));
    };
    if window.len() != 2 {
        return Err(TemporalError::MalformedRequest(
            "window must have two bounds",
        ));
    }
    let start = window[0]
        .as_str()
        .and_then(wall)
        .ok_or(TemporalError::MalformedRequest("invalid window start"))?;
    let end = window[1]
        .as_str()
        .and_then(wall)
        .ok_or(TemporalError::MalformedRequest("invalid window end"))?;
    if start >= end {
        return Err(TemporalError::MalformedRequest("window must increase"));
    }
    if w < start || w >= end {
        return done(reason("OutOfCoverage", "TransitionWindowExceeded"));
    }
    let Some(offsets) = field(f, "offsets_for_local_wall_time").as_array() else {
        return Err(TemporalError::MalformedRequest(
            "offset candidates required",
        ));
    };
    if offsets.len() > budget.max_members {
        return Err(TemporalError::ResourceExhausted("offset member limit"));
    }
    let mut candidates = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for o in offsets {
        let os = o
            .as_str()
            .ok_or(TemporalError::MalformedRequest("offset must be string"))?;
        let offset = offset(os).ok_or(TemporalError::MalformedRequest("invalid offset"))?;
        if !seen.insert(offset) {
            continue;
        }
        let Some(utc) = shift(w, offset) else {
            return done(reason("OutOfCoverage", "UtcOutsideSupportedRange"));
        };
        candidates.push(obj(vec![("offset", text(os)), ("instant_utc", text(&utc))]));
    }
    let selector = field(input, "fold_selector");
    let mut result = if candidates.is_empty() {
        reason("Unresolved", "NonexistentLocalTime")
    } else if !selector.is_null() {
        let Some(selected) = selector.as_str() else {
            return Err(TemporalError::MalformedRequest(
                "fold_selector must name an offset",
            ));
        };
        let Some(c) = candidates
            .iter()
            .find(|c| string(c, "offset") == Some(selected))
        else {
            return done(reason("Unresolved", "InvalidFoldSelector"));
        };
        obj(vec![("status", text("Unique")), ("value", c.clone())])
    } else if candidates.len() == 1 {
        obj(vec![
            ("status", text("Unique")),
            ("value", candidates[0].clone()),
        ])
    } else {
        obj(vec![
            ("status", text("Ambiguous")),
            ("candidates", JsonValue::Array(candidates)),
        ])
    };
    put(
        &mut result,
        "rules_source",
        text("source-profile-fixture-not-IANA"),
    );
    put(
        &mut result,
        "transition_version",
        field(f, "fixture_id").clone(),
    );
    put(
        &mut result,
        "calendar_rules_version",
        text("proleptic-gregorian-civil-v1"),
    );
    done(result)
}

/// Named seasons/eras are converted only through supplied, versioned boundary
/// rows. The table defines historical applicability, never a generic season.
pub fn derive_named_extent(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    budget_check(input, context, budget)?;
    derive_named_extent_admitted(input, context, budget)
}

// Parent dispatch has already admitted both roots; keep one semantic body.
pub(super) fn derive_named_extent_admitted(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let done = |r| {
        Ok(finish(
            r,
            input,
            context,
            "tos-named-calendar-boundaries-v1",
        ))
    };
    let p = field(input, "position");
    let rule = field(input, "season_boundary_rule");
    if rule.is_null() {
        let mut r = reason("Unsupported", "SeasonBoundaryRuleNotSupplied");
        put(&mut r, "preserved_position", p.clone());
        return done(r);
    }
    if !reference(field(rule, "version")) || !reference(field(p, "trs_ref")) {
        return done(reason("Unresolved", "VersionedCalendarRulesRequired"));
    }
    if !crate::query_context::same_json(field(rule, "trs_ref"), field(p, "trs_ref")) {
        return done(reason("Unresolved", "CalendarRuleTrsMismatch"));
    }
    let position_calendar = p
        .object_get("calendar")
        .map(|value| {
            value.as_str().ok_or(TemporalError::MalformedRequest(
                "position calendar must be a string",
            ))
        })
        .transpose()?;
    let rule_calendar = rule
        .object_get("calendar")
        .map(|value| {
            value.as_str().ok_or(TemporalError::MalformedRequest(
                "boundary rule calendar must be a string",
            ))
        })
        .transpose()?;
    if matches!((position_calendar, rule_calendar), (Some(a), Some(b)) if a != b) {
        return done(reason("Unresolved", "CalendarDeclarationConflict"));
    }
    let Some(rows) = field(rule, "boundaries").as_array() else {
        return Err(TemporalError::MalformedRequest("boundary rows required"));
    };
    if rows.len() > budget.max_members {
        return Err(TemporalError::ResourceExhausted("boundary member limit"));
    }
    let hits: Vec<_> = rows
        .iter()
        .filter(|r| {
            field(r, "year") == field(p, "year")
                && field(r, "season_code") == field(p, "season_code")
        })
        .collect();
    if hits.is_empty() {
        return done(reason("OutOfCoverage", "CalendarBoundaryEntryAbsent"));
    }
    if hits.len() > 1 {
        return done(reason("Ambiguous", "MultipleCalendarBoundaryEntries"));
    }
    let row = hits[0];
    if field(row, "year").is_null() || field(row, "season_code").is_null() {
        return Err(TemporalError::MalformedRequest(
            "named position fields required",
        ));
    }
    if string(rule, "calendar") != Some("GregorianProleptic") {
        return done(reason("Unsupported", "ExplicitCalendarRulesRequired"));
    }
    let start = string(row, "start").and_then(calendar::parse_civil);
    let end = string(row, "end_exclusive").and_then(calendar::parse_civil);
    if !matches!((start,end),(Some((a,"day")),Some((b,"day"))) if a<b && a.0>=1 && b.0<=9999) {
        return Err(TemporalError::MalformedRequest(
            "invalid calendar boundary row",
        ));
    }
    let mut value = row.clone();
    put(&mut value, "type", text("CalendarPositionBucket"));
    put(&mut value, "trs_ref", field(p, "trs_ref").clone());
    done(obj(vec![
        ("status", text("Resolved")),
        ("value", value),
        ("calendar_rules_version", field(rule, "version").clone()),
        (
            "rules_source",
            text("supplied-boundaries-not-historical-calendar-conformance"),
        ),
        ("event_extent_inferred", JsonValue::Bool(false)),
    ]))
}

/// Pure PostgreSQL daterange projection. No server, SQL execution or claim
/// lookup occurs here. A projection is a bucket index, not an assertion of an
/// event's exact extent. The complete supplied sidecar remains authoritative.
pub fn project_postgres_range(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    budget_check(input, context, budget)?;
    project_postgres_range_admitted(input, context, budget)
}

// Parent dispatch has already admitted both roots; keep one semantic body.
pub(super) fn project_postgres_range_admitted(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let done = |r| {
        Ok(finish(
            r,
            input,
            context,
            "tos-postgres-day-bucket-projection-v1",
        ))
    };
    if string(input, "postgres_type") != Some("daterange") {
        return done(reason("Unsupported", "UnsupportedPostgresRangeType"));
    }
    if string(input, "bound_form") != Some("[)") {
        return done(reason("Unsupported", "UnsupportedPostgresBoundForm"));
    }
    let extent = field(input, "source_extent");
    if string(extent, "type") != Some("CalendarDayBucket") {
        return done(reason("Unsupported", "ExplicitCalendarDayBucketRequired"));
    }
    if (extent.object_get("unit").is_some() && string(extent, "unit") != Some("day"))
        || (extent.object_get("resolution").is_some()
            && string(extent, "resolution") != Some("day"))
    {
        return done(reason("Unsupported", "UnsupportedProjectionUnit"));
    }
    let calendar = string(extent, "calendar").or_else(|| string(input, "calendar"));
    if let (Some(a), Some(b)) = (string(extent, "calendar"), string(input, "calendar")) {
        if a != b {
            return done(reason("Unresolved", "CalendarDeclarationConflict"));
        }
    }
    if calendar.is_some_and(|c| c != "GregorianProleptic") {
        return done(reason("Unsupported", "UnsupportedProjectionCalendar"));
    }
    let sidecar = field(input, "sidecar");
    let mut missing = vec![];
    if calendar.is_none() {
        missing.push(text("calendar:GregorianProleptic"));
    }
    if !reference(field(extent, "trs_ref")) {
        missing.push(text("source_extent.trs_ref"));
    }
    if !sidecar_claim_reference(field(input, "sidecar_claim_ref")) {
        missing.push(text("sidecar_claim_ref"));
    }
    for key in ["raw_claim", "trs_ref", "precision_assessment", "provenance"] {
        // An explicit null precision assessment is preserved as absence of an
        // assessment; an omitted field is missing data, never invented.
        if sidecar.object_get(key).is_none()
            || (key != "precision_assessment" && field(sidecar, key).is_null())
        {
            missing.push(text(&format!("sidecar.{key}")));
        }
    }
    if !missing.is_empty() {
        let mut r = reason("Unresolved", "ProjectionContextRequired");
        put(&mut r, "missing_requirements", JsonValue::Array(missing));
        return done(r);
    }
    if !crate::query_context::same_json(field(sidecar, "trs_ref"), field(extent, "trs_ref")) {
        return done(reason("Unresolved", "SidecarTrsMismatch"));
    }
    let start =
        string(extent, "start").ok_or(TemporalError::MalformedRequest("bucket start required"))?;
    let end = string(extent, "end_exclusive").ok_or(TemporalError::MalformedRequest(
        "bucket end_exclusive required",
    ))?;
    if start.len() > budget.max_expression_bytes || end.len() > budget.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted("expression byte limit"));
    }
    let Some((a, "day")) = calendar::parse_civil(start) else {
        return done(reason("Unsupported", "UnsupportedProjectionLexicalForm"));
    };
    let Some((b, "day")) = calendar::parse_civil(end) else {
        return done(reason("Unsupported", "UnsupportedProjectionLexicalForm"));
    };
    if a.0 < 1 || b.0 > 9999 {
        return done(reason(
            "OutOfCoverage",
            "ProjectionYearOutsideSupportedRange",
        ));
    }
    if a >= b {
        return done(reason("ContradictoryBounds", "NonIncreasingDayBucket"));
    }
    if calendar::next(a, "day") != Some(b) {
        return done(reason("Unsupported", "NotSingleDayBucket"));
    }
    let metadata = JsonValue::Array(
        ["raw_claim", "trs_ref", "precision_assessment", "provenance"]
            .iter()
            .map(|s| text(s))
            .collect(),
    );
    let loss_report = obj(vec![
        ("range_only_omits", metadata.clone()),
        (
            "range_only_warning",
            text("RangeDoesNotEncodeSourceSemantics"),
        ),
        ("sidecar_retains_all_supplied_fields", JsonValue::Bool(true)),
        ("projection_package_loss", JsonValue::Array(vec![])),
    ]);
    done(obj(vec![
        ("status", text("Projected")),
        ("postgres_type", text("daterange")),
        ("postgres_value", text(&format!("[{start},{end})"))),
        (
            "sidecar_claim_ref",
            field(input, "sidecar_claim_ref").clone(),
        ),
        ("sidecar", sidecar.clone()),
        ("sidecar_preserved", metadata),
        ("source_extent", extent.clone()),
        ("loss_warning", JsonValue::Null),
        ("loss_report", loss_report),
        (
            "calendar_rules_version",
            text("proleptic-gregorian-civil-v1"),
        ),
        ("projection_semantics", text("CalendarPositionBucketIndex")),
        ("event_extent_inferred", JsonValue::Bool(false)),
    ]))
}
