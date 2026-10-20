//! Bounded TEI date capture, not a general XML processor. Original bytes and
//! every supplied reference remain available even outside this codec's subset.
use super::*;
use std::collections::BTreeMap;

fn preserved(mut result: JsonValue, input: &JsonValue, context: &JsonValue) -> JsonValue {
    put(
        &mut result,
        "preserved_raw",
        field(input, "xml_fragment").clone(),
    );
    put(&mut result, "preserved_input", input.clone());
    put(&mut result, "preserved_context", context.clone());
    put(
        &mut result,
        "parser_version",
        text("tos-tei-date-subset-v1"),
    );
    put(
        &mut result,
        "tei_version",
        field(input, "tei_version").clone(),
    );
    put(&mut result, "precision_loss", JsonValue::Array(vec![]));
    if w3c_profile_selected(input) {
        put(
            &mut result,
            "normalized_profile",
            text("TEI-P5-4.12.0:att.datable.w3c"),
        );
        put(&mut result, "normalized_calendar", text("Gregorian"));
        put(
            &mut result,
            "calendar_rules_version",
            text("tei-w3c-gregorian-no-year-zero-v1"),
        );
        put(&mut result, "year_numbering", text("W3C-no-year-zero"));
    }
    result
}

fn w3c_profile_selected(input: &JsonValue) -> bool {
    string(input, "tei_version") == Some("P5-4.12.0")
        && (input.object_get("normalized_profile").is_none()
            || matches!(
                string(input, "normalized_profile"),
                Some("att.datable.w3c" | "TEI-P5-4.12.0:att.datable.w3c")
            ))
}

#[derive(Debug)]
struct Element {
    name: String,
    attrs: BTreeMap<String, String>,
}
fn name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_:.-".contains(&b)
}
fn decode(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        let end = rest.find(';')?;
        let entity = &rest[..end];
        let c = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let n = if let Some(x) = entity.strip_prefix("#x") {
                    u32::from_str_radix(x, 16).ok()?
                } else {
                    entity.strip_prefix('#')?.parse().ok()?
                };
                char::from_u32(n)?
            }
        };
        if c == '\0' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
            return None;
        }
        out.push(c);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}
// Finite grammar: one unprefixed <date> with plain text, followed by zero or
// more empty <precision/> elements. DTD, PI, namespaces and nested XML are
// explicitly outside this subset; they never reach an external resolver.
fn elements(raw: &str, max: usize) -> Option<Vec<Element>> {
    let b = raw.as_bytes();
    let mut i = 0;
    let mut open = false;
    let mut seen = false;
    let mut out = vec![];
    while i < b.len() {
        if b[i] != b'<' {
            let start = i;
            while i < b.len() && b[i] != b'<' {
                i += 1;
            }
            if !open && !raw[start..i].trim().is_empty() {
                return None;
            }
            decode(&raw[start..i])?;
            continue;
        }
        i += 1;
        if b.get(i) == Some(&b'/') {
            i += 1;
            let start = i;
            while i < b.len() && name_byte(b[i]) {
                i += 1;
            }
            if &raw[start..i] != "date" || !open {
                return None;
            }
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if b.get(i) != Some(&b'>') {
                return None;
            }
            i += 1;
            open = false;
            continue;
        }
        if open {
            return None;
        }
        let start = i;
        while i < b.len() && name_byte(b[i]) {
            i += 1;
        }
        let name = &raw[start..i];
        if !matches!(name, "date" | "precision")
            || (name == "date" && seen)
            || (name == "precision" && !seen)
        {
            return None;
        }
        let mut attrs = BTreeMap::new();
        loop {
            let before = i;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if matches!(b.get(i), Some(b'>') | Some(b'/')) {
                break;
            }
            if before == i {
                return None;
            }
            let start = i;
            while i < b.len() && name_byte(b[i]) {
                i += 1;
            }
            if i == start {
                return None;
            }
            let key = &raw[start..i];
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if b.get(i) != Some(&b'=') {
                return None;
            }
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            let q = *b.get(i)?;
            if !matches!(q, b'\'' | b'"') {
                return None;
            }
            i += 1;
            let start = i;
            while i < b.len() && b[i] != q {
                if b[i] == b'<' {
                    return None;
                }
                i += 1;
            }
            let val = decode(&raw[start..i])?;
            if attrs.insert(key.to_string(), val).is_some() {
                return None;
            }
            i += 1;
        }
        let empty = b.get(i) == Some(&b'/');
        if empty {
            i += 1;
        }
        if b.get(i) != Some(&b'>') {
            return None;
        }
        i += 1;
        if name == "precision" && !empty {
            return None;
        }
        if name == "date" {
            seen = true;
            open = !empty;
        }
        out.push(Element {
            name: name.into(),
            attrs,
        });
        if out.len() > max {
            return None;
        }
    }
    if open || !seen { None } else { Some(out) }
}
fn endpoint(s: &str, role: &str) -> Option<JsonValue> {
    // TEI P5 4.12.0 att.datable.w3c explicitly uses Gregorian and no year
    // zero. Negative year labels are retained; arithmetic uses astronomical
    // numbering only internally for the proleptic leap-day validity check.
    let negative = s.starts_with('-');
    let parts: Vec<_> = s.strip_prefix('-').unwrap_or(s).split('-').collect();
    if parts.is_empty()
        || parts.len() > 3
        || parts[0].len() != 4
        || !parts[0].bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let year: i64 = parts[0].parse().ok()?;
    if year == 0 {
        return None;
    }
    if parts
        .iter()
        .skip(1)
        .any(|p| p.len() != 2 || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let astronomical = if negative { 1 - year } else { year };
    if parts.len() > 1 {
        let month: u8 = parts[1].parse().ok()?;
        let last = calendar::days_in_month(astronomical, month)?;
        if parts.len() > 2 {
            let day: u8 = parts[2].parse().ok()?;
            if day == 0 || day > last {
                return None;
            }
        }
    }
    let res = match parts.len() {
        1 => "year",
        2 => "month",
        _ => "day",
    };
    let mut value = obj(vec![
        ("role", text(role)),
        ("position", text(s)),
        ("resolution", text(res)),
    ]);
    // TEI P5 dating attributes declare inclusive period/possibility bounds.
    // Scalar @when is a calendar position and has no interval-bound rule.
    if role != "when" {
        put(&mut value, "inclusivity", text("Inclusive"));
    }
    Some(value)
}

pub fn capture_tei(
    input: &JsonValue,
    context: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let raw = string(input, "xml_fragment")
        .ok_or(TemporalError::MalformedRequest("xml_fragment required"))?;
    if raw.len() > budget.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted("expression byte limit"));
    }
    let finish = |r| Ok(preserved(r, input, context));
    if string(input, "tei_version") != Some("P5-4.12.0") {
        return finish(reason("Unsupported", "UnsupportedTeiVersion"));
    }
    if !w3c_profile_selected(input) {
        return finish(reason("Unsupported", "UnsupportedTeiDatingProfile"));
    }
    let Some(es) = elements(raw, budget.max_members) else {
        return finish(reason("Unsupported", "OutsideBoundedTeiXmlSubset"));
    };
    let a = &es[0].attrs;
    let supported = [
        "xml:id",
        "xml:lang",
        "when",
        "notBefore",
        "notAfter",
        "from",
        "to",
        "cert",
        "calendar",
    ];
    if a.keys().any(|k| !supported.contains(&k.as_str())) {
        return finish(reason("Partial", "UnsupportedTeiAttribute"));
    }
    for key in ["when", "from", "to", "notBefore", "notAfter"] {
        if a.get(key).is_some_and(|s| {
            let year = s
                .strip_prefix('-')
                .unwrap_or(s)
                .split('-')
                .next()
                .unwrap_or("");
            year == "0000"
        }) {
            return finish(reason("Unparsed", "YearZeroNotPermittedByW3CProfile"));
        }
    }
    let has = |k: &str| a.contains_key(k);
    let invalid = (has("when")
        && ["from", "to", "notBefore", "notAfter"]
            .iter()
            .any(|k| has(k)))
        || (has("from") && has("notBefore"))
        || (has("to") && has("notAfter"));
    if invalid {
        return finish(reason("Unresolved", "IncompatibleTeiAttributes"));
    }
    let mut value = if let Some(s) = a.get("when") {
        let Some(mut p) = endpoint(s, "when") else {
            return finish(reason("Partial", "UnsupportedDateLexicalForm"));
        };
        put(&mut p, "type", text("CalendarPosition"));
        put(&mut p, "calendar", text("Gregorian"));
        p
    } else {
        let actual = has("from") || has("to");
        let mut v = obj(vec![(
            "type",
            text(if actual {
                "ActualInterval"
            } else {
                "PossiblePositionBounds"
            }),
        )]);
        let mut any = false;
        for (attr, role, key) in [
            ("from", "from", "start"),
            ("to", "to", "end"),
            (
                "notBefore",
                "earliest_possible_date",
                if actual { "start" } else { "lower" },
            ),
            (
                "notAfter",
                "latest_possible_date",
                if actual { "end" } else { "upper" },
            ),
        ] {
            if let Some(s) = a.get(attr) {
                let Some(mut p) = endpoint(s, role) else {
                    return finish(reason("Partial", "UnsupportedDateLexicalForm"));
                };
                if actual && attr == "notBefore" {
                    put(&mut p, "start_bound_only", JsonValue::Bool(true));
                }
                if actual && attr == "notAfter" {
                    put(&mut v, "end_certainty", text("upper_bound_only"));
                }
                put(&mut v, key, p);
                any = true;
            }
        }
        if !any {
            return finish(reason("Partial", "NoTemporalAttribute"));
        }
        v
    };
    put(&mut value, "normalized_calendar", text("Gregorian"));
    put(
        &mut value,
        "source_calendar",
        a.get("calendar")
            .map(|s| text(s))
            .unwrap_or_else(|| field(input, "source_calendar").clone()),
    );
    if let Some(source_calendar) = a.get("calendar") {
        put(
            &mut value,
            "source_calendar_pointers",
            JsonValue::Array(source_calendar.split_whitespace().map(text).collect()),
        );
    }
    if let Some(id) = a.get("xml:id") {
        put(&mut value, "source_element_id", text(id));
    }
    if let Some(cert) = a.get("cert") {
        put(&mut value, "certainty", text(cert));
    }
    let mut assessed = std::collections::BTreeSet::new();
    for e in es.iter().skip(1) {
        if e.name != "precision"
            || e.attrs.keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "target" | "match" | "precision" | "cert" | "resp"
                )
            })
        {
            return finish(reason("Partial", "UnsupportedPrecisionAssessment"));
        }
        let Some(id) = a.get("xml:id") else {
            return finish(reason("Partial", "UnresolvedPrecisionTarget"));
        };
        if e.attrs.get("target") != Some(&format!("#{id}")) {
            return finish(reason("Partial", "UnresolvedPrecisionTarget"));
        }
        let Some(m) = e.attrs.get("match") else {
            return finish(reason("Partial", "PrecisionMatchRequired"));
        };
        if !assessed.insert(m) {
            return finish(reason("Partial", "MultiplePrecisionAssessments"));
        }
        let key = match m.as_str() {
            "@when" => "",
            "@from" => "start",
            "@to" => "end",
            "@notBefore" => {
                if has("from") || has("to") {
                    "start"
                } else {
                    "lower"
                }
            }
            "@notAfter" => {
                if has("from") || has("to") {
                    "end"
                } else {
                    "upper"
                }
            }
            _ => return finish(reason("Partial", "UnsupportedPrecisionMatch")),
        };
        if !has(m.trim_start_matches('@')) {
            return finish(reason("Partial", "PrecisionAttributeAbsent"));
        }
        let Some(p) = e.attrs.get("precision") else {
            return finish(reason("Partial", "PrecisionValueRequired"));
        };
        let mut bound = if key.is_empty() {
            value.clone()
        } else {
            field(&value, key).clone()
        };
        put(&mut bound, "precision", text(p));
        put(&mut bound, "match", text(m));
        if key.is_empty() {
            value = bound;
        } else {
            put(&mut value, key, bound);
        }
    }
    finish(obj(vec![("status", text("Complete")), ("value", value)]))
}
