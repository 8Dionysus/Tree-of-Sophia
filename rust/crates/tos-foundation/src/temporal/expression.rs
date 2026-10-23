use super::*;

/// The original lexical form is immutable within the returned value. Parsing
/// does not supply a TRS, probability, tolerance, or normalization authority.
pub fn capture_expression(
    raw: &str,
    profile: ExpressionProfile,
    locale: Option<String>,
    budget: TemporalBudget,
) -> TemporalExpression {
    let mut e = TemporalExpression {
        raw: raw.into(),
        profile,
        locale,
        parser_version: "tos-edtf-subset-v1",
        state: ParseState::Unparsed,
        syntax: JsonValue::Null,
        diagnostics: vec![],
    };
    if raw.len() > budget.max_expression_bytes {
        e.diagnostics.push("ResourceLimit".into());
        return e;
    }
    let level = match &e.profile {
        ExpressionProfile::Edtf { level, version } if version == "2019-02-04" && *level <= 2 => {
            *level
        }
        _ => {
            e.diagnostics.push("UnsupportedProfileLevel".into());
            return e;
        }
    };
    match parse(raw, level, budget) {
        Ok(v) => {
            e.state = ParseState::Complete;
            e.syntax = v;
        }
        Err(r) => {
            e.diagnostics.push(r.into());
        }
    }
    e
}
pub fn serialize_expression(expression: &TemporalExpression) -> &str {
    &expression.raw
}

fn atom(raw: &str) -> Option<JsonValue> {
    let parts = raw.split('-').collect::<Vec<_>>();
    if parts.is_empty()
        || parts.len() > 3
        || parts[0].len() != 4
        || !parts[0].bytes().all(|x| x.is_ascii_digit())
    {
        return None;
    }
    let year = parts[0].parse::<i64>().ok()?;
    for p in &parts[1..] {
        if p.len() != 2 || !p.bytes().all(|x| x.is_ascii_digit()) {
            return None;
        }
    }
    if parts.len() > 1 && !(1..=12).contains(&parts[1].parse::<i64>().ok()?) {
        return None;
    }
    // Lexical EDTF dates use its declared Gregorian date syntax. This check
    // does not assign a source claim's TRS or an instant.
    if parts.len() == 3 {
        let month = parts[1].parse::<u8>().ok()?;
        let day = parts[2].parse::<u8>().ok()?;
        if day == 0 || day > super::calendar::days_in_month(year, month)? {
            return None;
        }
    }
    let resolution = match parts.len() {
        1 => "year",
        2 => "month",
        _ => "day",
    };
    Some(obj(vec![
        ("position", text(raw)),
        ("resolution", text(resolution)),
    ]))
}
fn endpoint(raw: &str, level: u8) -> Result<JsonValue, &'static str> {
    if raw == ".." {
        return if level >= 1 {
            Ok(obj(vec![
                ("type", text("NoBoundDeclared")),
                ("lexical_marker", text("..")),
                ("interpretation", text("NoneOrOtherReason")),
            ]))
        } else {
            Err("UnsupportedProfileLevel")
        };
    }
    if raw.is_empty() {
        return if level >= 1 {
            Ok(obj(vec![("type", text("Unknown"))]))
        } else {
            Err("UnsupportedProfileLevel")
        };
    }
    atom(raw).ok_or("MalformedExpression")
}
fn parse(raw: &str, level: u8, budget: TemporalBudget) -> Result<JsonValue, &'static str> {
    if raw.starts_with('[') || raw.starts_with('{') {
        if level < 2 {
            return Err("UnsupportedProfileLevel");
        }
        let (closing, kind) = if raw.starts_with('[') {
            (']', "ExactlyOneOf")
        } else {
            ('}', "AllOf")
        };
        if !raw.ends_with(closing) {
            return Err("MalformedExpression");
        }
        let mut members = vec![];
        let mut expanded = vec![];
        let mut had_range = false;
        for token in raw[1..raw.len() - 1].split(',') {
            let token = token.trim();
            if members.len() >= budget.max_members {
                return Err("ResourceLimit");
            }
            if let Some((a, b)) = token.split_once("..") {
                let lo = year(a)?;
                let hi = year(b)?;
                if hi < lo {
                    return Err("ContradictoryBounds");
                }
                let width = (hi - lo + 1) as usize;
                if width > budget.max_members.saturating_sub(expanded.len()) {
                    return Err("ResourceLimit");
                }
                members.push(obj(vec![(
                    "inclusive_year_range",
                    JsonValue::Array(vec![number(lo), number(hi)]),
                )]));
                expanded.extend((lo..=hi).map(number));
                had_range = true;
            } else {
                let n = year(token)?;
                if expanded.len() >= budget.max_members {
                    return Err("ResourceLimit");
                }
                members.push(number(n));
                expanded.push(number(n));
            }
        }
        if members.is_empty() {
            return Err("MalformedExpression");
        }
        let mut v = obj(vec![
            ("type", text(kind)),
            ("members", JsonValue::Array(members)),
        ]);
        if had_range {
            put(&mut v, "expanded_years", JsonValue::Array(expanded));
        }
        return Ok(v);
    }
    if let Some((start, end)) = raw.split_once('/') {
        if end.contains('/') {
            return Err("MalformedExpression");
        }
        return Ok(obj(vec![
            ("type", text("IntervalExpression")),
            ("start", endpoint(start, level)?),
            ("end", endpoint(end, level)?),
            ("whole_interval_precision", JsonValue::Null),
        ]));
    }
    let (base, qualifier) = match raw.as_bytes().last() {
        Some(b'?') => (&raw[..raw.len() - 1], "Uncertain"),
        Some(b'~') => (&raw[..raw.len() - 1], "Approximate"),
        Some(b'%') => (&raw[..raw.len() - 1], "UncertainAndApproximate"),
        _ => (raw, "None"),
    };
    if qualifier != "None" {
        if level < 1 {
            return Err("UnsupportedProfileLevel");
        }
        let a = atom(base).ok_or("MalformedExpression")?;
        let mut p = obj(vec![("resolution", field(&a, "resolution").clone())]);
        if base.len() == 4 {
            put(&mut p, "year", number(year(base)?));
        } else {
            put(&mut p, "lexical", text(base));
        }
        return Ok(obj(vec![
            ("position", p),
            ("qualifier", text(qualifier)),
            ("raw", text(raw)),
        ]));
    }
    if raw.contains('X') || raw.contains('S') || raw.contains('Y') {
        return Err("UnsupportedEdtfFeature");
    }
    atom(raw).ok_or("MalformedExpression")
}
fn year(s: &str) -> Result<i64, &'static str> {
    if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err("UnsupportedEdtfFeature");
    }
    s.parse().map_err(|_| "MalformedExpression")
}
