//! Typed reports over retained source fields. These values grant no identity,
//! access, rights, review, or state-machine authority.
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, String>;
trait PythonStrip {
    fn pystrip(&self) -> &str;
}
impl PythonStrip for str {
    fn pystrip(&self) -> &str {
        tos_foundation::python_strip_unicode16_v1(self, self.len())
            .expect("input-derived scalar bound")
    }
}
fn decimal(text: &str) -> Result<u64> {
    if text.is_empty() {
        return Err("empty decimal".into());
    }
    text.chars().try_fold(0u64, |n, c| {
        tos_foundation::python_decimal_value_unicode16_v1(c)
            .and_then(|d| n.checked_mul(10)?.checked_add(u64::from(d)))
            .ok_or_else(|| "invalid decimal".into())
    })
}

pub fn fold(value: &str) -> Result<String> {
    let limit = value.len().checked_mul(3).ok_or("casefold size overflow")?;
    tos_foundation::python_casefold_unicode16_v1(value, value.len(), limit, limit)
        .map_err(|e| e.to_string())
}
fn re(pattern: &str) -> Result<Regex> {
    Regex::new(pattern).map_err(|e| e.to_string())
}
fn result(value: Value, status: &str, unparsed: Vec<Value>, issues: Vec<Value>) -> Value {
    json!({"value":value,"normalization_status":status,"unparsed_fragments":unparsed,"issues":issues})
}
fn status(known: bool, unknown: bool) -> &'static str {
    if unknown {
        if known {
            "partial"
        } else {
            "reported_unparsed"
        }
    } else {
        "normalized"
    }
}
pub fn validate_profile(profile: &Value) -> Result<()> {
    if profile["version"] != 1 {
        return Err("unsupported registry normalization profile".into());
    }
    for kind in ["registry", "gaps"] {
        let mapping = profile[kind].as_object().ok_or("missing field mapping")?;
        for (source, rule) in mapping {
            if source.is_empty()
                || rule["target"].as_str().is_none_or(str::is_empty)
                || rule["value_type"].as_str().is_none_or(str::is_empty)
            {
                return Err(format!("incomplete field mapping: {kind}.{source}"));
            }
        }
    }
    Ok(())
}
fn split(text: &str) -> Vec<&str> {
    let mut stack = Vec::new();
    let mut output = Vec::new();
    let mut start = 0;
    for (at, c) in text.char_indices() {
        if let Some(close) = match c {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '（' => Some('）'),
            _ => None,
        } {
            stack.push(close);
        } else if stack.last() == Some(&c) {
            stack.pop();
        } else if stack.is_empty() && matches!(c, ';' | '|' | '\n' | '\r') {
            output.push(&text[start..at]);
            start = at + c.len_utf8();
        }
    }
    output.push(&text[start..]);
    output
}
fn lexical(text: &str, vocabulary: &Value, multi: bool) -> Result<Value> {
    let vocabulary = vocabulary.as_object().ok_or("missing lexical vocabulary")?;
    let mut known = Vec::new();
    let mut unknown = Vec::new();
    for fragment in if multi { split(text) } else { vec![text] } {
        let fragment = fragment.pystrip();
        if fragment.is_empty() {
            continue;
        }
        match vocabulary.get(&fold(fragment)?) {
            Some(value) if !value.is_null() => known.push(value.clone()),
            _ => unknown.push(json!(fragment)),
        }
    }
    let state = status(!known.is_empty(), !unknown.is_empty());
    let value = if multi {
        Value::Array(known)
    } else {
        known.into_iter().next().unwrap_or(Value::Null)
    };
    Ok(result(value, state, unknown, vec![]))
}
fn qualified(text: &str, vocabulary: &Value, key: &str) -> Result<Value> {
    let vocabulary = vocabulary
        .as_object()
        .ok_or("missing qualified vocabulary")?;
    let qualifier = re(r"^(.+?)\s*(\([^\n]+\)|[—–]\s*.+)$")?;
    let mut terms = Vec::new();
    let mut unknown = Vec::new();
    for fragment in split(text) {
        let fragment = fragment.pystrip();
        if fragment.is_empty() {
            continue;
        }
        let commas = fragment.split(',').map(str::trim).collect::<Vec<_>>();
        let exact = commas.iter().map(|p| fold(p)).collect::<Result<Vec<_>>>()?;
        let parts = if commas.len() > 1 && exact.iter().all(|p| vocabulary.contains_key(p)) {
            commas
        } else {
            vec![fragment]
        };
        for part in parts {
            let mut value = vocabulary.get(&fold(part)?);
            let mut qualifiers = Vec::new();
            if value.is_none_or(Value::is_null) {
                if let Some(c) = qualifier.captures(part) {
                    value = vocabulary.get(&fold(c[1].pystrip())?);
                    if value.is_some_and(|v| !v.is_null()) {
                        qualifiers.push(json!(c[2].pystrip()));
                    }
                }
            }
            match value.filter(|v| !v.is_null()) {
                None => unknown.push(json!(part)),
                Some(value) => {
                    let mut facets = if let Some(v) = value.as_object() {
                        v.clone()
                    } else {
                        Map::from_iter([(key.into(), value.clone())])
                    };
                    let mut all = facets
                        .get("qualifiers")
                        .map(|v| v.as_array().cloned().ok_or("invalid vocabulary qualifiers"))
                        .transpose()?
                        .unwrap_or_default();
                    all.extend(qualifiers);
                    facets.insert("reported_fragment".into(), json!(part));
                    facets.insert("qualifiers".into(), json!(all));
                    terms.push(Value::Object(facets));
                }
            }
        }
    }
    let state = status(!terms.is_empty(), !unknown.is_empty());
    Ok(result(json!(terms), state, unknown, vec![]))
}
fn valid_url_host(url: &str) -> bool {
    let Some((_, rest)) = url.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    if host.starts_with('[') {
        let Some(end) = host.find(']') else {
            return false;
        };
        let inside = &host[1..end];
        return inside.parse::<std::net::Ipv6Addr>().is_ok()
            || re(r"^v[0-9A-Fa-f]+\..+$").is_ok_and(|r| r.is_match(inside));
    }
    !host.contains(['[', ']']) && !host.split(':').next().unwrap_or("").is_empty()
}
pub fn links(text: &str) -> Result<Value> {
    let pattern = re(r#"(?i)https?://[^\s<>"“”]+"#)?;
    let separators = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n' | ';' | '|' | ',');
    let mut links = Vec::new();
    let mut unknown = Vec::new();
    let mut previous = 0;
    for found in pattern.find_iter(text) {
        let prefix = text[previous..found.start()].trim_matches(separators);
        if !prefix.is_empty() {
            unknown.push(json!(prefix));
        }
        let token = found.as_str();
        let mut begins = vec![0];
        for (i, c) in token.char_indices() {
            if c == ';'
                && (token[i + 1..]
                    .get(..7)
                    .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
                    || token[i + 1..]
                        .get(..8)
                        .is_some_and(|s| s.eq_ignore_ascii_case("https://")))
            {
                begins.push(i + 1);
            }
        }
        for (i, start) in begins.iter().copied().enumerate() {
            let end = begins.get(i + 1).map_or(token.len(), |n| n - 1);
            let piece = &token[start..end];
            let mut url = piece.trim_end_matches([';', '|', ',']);
            while url.ends_with(')') && url.matches(')').count() > url.matches('(').count() {
                url = &url[..url.len() - 1];
            }
            if valid_url_host(url) {
                let offset = text[..found.start() + start].chars().count();
                links.push(json!({"url":url,"source_span":[offset,offset+url.chars().count()]}));
            } else {
                unknown.push(json!(piece));
            }
        }
        previous = found.end();
    }
    let suffix = text[previous..].trim_matches(separators);
    if !suffix.is_empty() {
        unknown.push(json!(suffix));
    }
    let state = status(!links.is_empty(), !unknown.is_empty());
    Ok(result(json!(links), state, unknown, vec![]))
}
pub fn valid_day(year: i32, month: u32, day: u32) -> bool {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    (1..=9999).contains(&year) && day > 0 && day <= days
}
fn checked_date(text: &str, datetime: bool) -> Result<Value> {
    if datetime {
        let Some((day, time)) = text.split_once('T') else {
            return Err("invalid Excel datetime".into());
        };
        return Ok(result(
            json!({"value":if time=="00:00:00"{day}else{text},"precision":if time=="00:00:00"{"day"}else{"datetime"}}),
            "normalized",
            vec![],
            vec![],
        ));
    }
    if let Some(c) = re(r"^(\d{4})-(\d{2})-(\d{2})$")?.captures(text) {
        if c[1]
            .parse()
            .ok()
            .zip(c[2].parse().ok())
            .zip(c[3].parse().ok())
            .is_some_and(|((y, m), d)| valid_day(y, m, d))
        {
            return Ok(result(
                json!({"value":text,"precision":"day"}),
                "normalized",
                vec![],
                vec![],
            ));
        }
        return Ok(result(
            Value::Null,
            "reported_unparsed",
            vec![json!(text)],
            vec![json!("invalid_calendar_date")],
        ));
    }
    Ok(result(
        Value::Null,
        "reported_unparsed",
        vec![json!(text)],
        vec![json!("date_precision_unresolved")],
    ))
}
fn roman(text: &str) -> Result<Option<u64>> {
    if !re(r"^X{0,3}(IX|IV|V?I{0,3})$")?.is_match(text) {
        return Ok(None);
    }
    let mut total = 0i64;
    let mut previous = 0;
    for c in text.chars().rev() {
        let n = match c {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            _ => return Ok(None),
        };
        total += if n < previous { -n } else { n };
        previous = n;
    }
    Ok((total > 0).then_some(total as u64))
}
fn era(text: Option<&str>) -> &'static str {
    match text {
        None => "unspecified",
        Some(v)
            if v.eq_ignore_ascii_case("BC")
                || v.eq_ignore_ascii_case("BCE")
                || fold(v).is_ok_and(|v| v.starts_with("до")) =>
        {
            "BCE"
        }
        _ => "CE",
    }
}
fn temporal(text: &str) -> Result<Value> {
    let mut statement = json!({"reported_statement":text,"expressions":[]});
    if ["не установлена", "unknown", "not established", "undated"].contains(&fold(text)?.as_str())
    {
        statement["reported_date_status"] = json!("not_established");
        return Ok(result(statement, "normalized", vec![], vec![]));
    }
    if re(r"^\d{1,4}$")?.is_match(text) {
        if let Ok(n @ 1..=9999) = decimal(text) {
            statement["expressions"] =
                json!([{"year":n,"precision":"year","era":"unspecified","scope":"unspecified"}]);
            return Ok(result(statement, "normalized", vec![], vec![]));
        }
    }
    let approx = r"(?P<approx>c\.?\s*|ca\.?\s*|ок\.?\s*)?";
    let eras = r"BCE|BC|CE|AD|до\s+н\.\s*э\.|н\.\s*э\.";
    let year = re(&format!(
        r"(?i)^{approx}(?P<start>\d{{1,4}})(?:\s*[–—-]\s*(?P<end>\d{{1,4}}))?(?:\s*(?P<era>{eras})|\s*гг?\.)?$"
    ))?;
    if let Some(c) = year.captures(text) {
        let start = decimal(&c["start"])?;
        let end = decimal(c.name("end").map_or(&c["start"], |v| v.as_str()))?;
        if start > 0 && end > 0 {
            statement["expressions"] = json!([{"year_start":start,"year_end":end,"precision":"year","era":era(c.name("era").map(|v|v.as_str())),"approximate":c.name("approx").is_some(),"scope":"unspecified"}]);
            return Ok(result(statement, "normalized", vec![], vec![]));
        }
    }
    let millennium = re(&format!(
        r"(?i)^{approx}(?P<number>first|second|third|\d{{1,2}}(?:st|nd|rd|th)?)\s+millennium(?:\s*(?P<era>{eras}))?$"
    ))?;
    if let Some(c) = millennium.captures(text) {
        let ordinal = fold(&c["number"])?;
        let number = match ordinal.as_str() {
            "first" => 1,
            "second" => 2,
            "third" => 3,
            _ => decimal(
                &ordinal
                    .chars()
                    .take_while(|c| tos_foundation::python_decimal_unicode16_v1(*c))
                    .collect::<String>(),
            )?,
        };
        if number > 0 {
            statement["expressions"] = json!([{"millennium":number,"precision":"millennium","era":era(c.name("era").map(|v|v.as_str())),"approximate":c.name("approx").is_some(),"scope":"unspecified"}]);
            return Ok(result(statement, "normalized", vec![], vec![]));
        }
    }
    let century = re(&format!(
        r"(?i)^{approx}(?:(?P<start_part>early|mid|middle|late)\s+)?(?P<start>\d{{1,2}}|[IVX]+)(?:st|nd|rd|th)?(?:\s*[–—-]\s*(?:(?P<end_part>early|mid|middle|late)\s+)?(?P<end>\d{{1,2}}|[IVX]+)(?:st|nd|rd|th)?)?\s*(?:centur(?:y|ies)|c\.|в\.|вв\.)(?:\s*(?P<era>{eras}))?$"
    ))?;
    if let Some(c) = century.captures(text) {
        let number = |v: &str| -> Result<Option<u64>> {
            if v.chars().all(tos_foundation::python_decimal_unicode16_v1) {
                Ok(decimal(v).ok().filter(|n| *n > 0))
            } else {
                roman(&v.to_uppercase())
            }
        };
        if let (Some(start), Some(end)) = (
            number(&c["start"])?,
            number(c.name("end").map_or(&c["start"], |v| v.as_str()))?,
        ) {
            let mut expression = json!({"century_start":start,"century_end":end,"precision":"century","era":era(c.name("era").map(|v|v.as_str())),"approximate":c.name("approx").is_some(),"scope":"unspecified"});
            for key in ["start_part", "end_part"] {
                if let Some(value) = c.name(key) {
                    expression[key] = json!(fold(value.as_str())?);
                }
            }
            statement["expressions"] = json!([expression]);
            return Ok(result(statement, "normalized", vec![], vec![]));
        }
    }
    Ok(result(
        statement,
        "reported_unparsed",
        vec![json!(text)],
        vec![],
    ))
}
pub fn normalize(
    kind: &str,
    raw: &[(String, Value)],
    profile: &Value,
    dates: &BTreeSet<String>,
) -> Result<Value> {
    if !["registry", "gaps"].contains(&kind) {
        return Err("unsupported registry record kind".into());
    }
    let mapping = profile[kind].as_object().ok_or("missing registry kind")?;
    if raw.iter().any(|(k, _)| !mapping.contains_key(k)) {
        return Err("unmapped registry fields".into());
    }
    let vocabulary = &profile["vocabulary"];
    let mut fields = Vec::new();
    let mut issues = Vec::new();
    for (source, raw) in raw {
        let rule = &mapping[source];
        let typ = rule["value_type"].as_str().ok_or("missing value type")?;
        let text = match raw {
            Value::Null => String::new(),
            Value::String(v) => {
                if dates.contains(source) {
                    v.replacen('T', " ", 1)
                } else {
                    v.pystrip().to_owned()
                }
            }
            Value::Bool(v) => if *v { "True" } else { "False" }.into(),
            Value::Number(_) => super::string(raw)?,
            _ => return Err("unsupported raw field type".into()),
        };
        let normalized = if raw.is_null() || raw.is_string() && text.is_empty() {
            result(Value::Null, "empty", vec![], vec![])
        } else {
            match typ {
                "date" => checked_date(
                    if dates.contains(source) {
                        raw.as_str().ok_or("Excel date representation")?
                    } else {
                        &text
                    },
                    dates.contains(source),
                )?,
                "temporal_statement" => temporal(&text)?,
                "access_modes" | "use_tags" => lexical(&text, &vocabulary[typ], true)?,
                "object_kind" | "relevance" | "coverage" | "confidence" | "priority" => {
                    lexical(&text, &vocabulary[typ], false)?
                }
                "languages" => qualified(&text, &vocabulary[typ], "language_code")?,
                "formats" => qualified(&text, &vocabulary[typ], "format")?,
                "links" => links(raw.as_str().unwrap_or(&text))?,
                "reported_status" | "record_references" => {
                    let fragments = if typ == "reported_status" {
                        split(&text)
                    } else {
                        text.split([',', ';', '|', '\n', '\r']).collect()
                    };
                    let pattern = if typ == "reported_status" {
                        r"[A-Za-z][A-Za-z0-9_-]*"
                    } else {
                        profile["record_reference_pattern"]
                            .as_str()
                            .unwrap_or(r"[A-Za-z0-9][A-Za-z0-9._-]*")
                    };
                    let pattern = re(&format!(r"\A(?:{pattern})\z"))?;
                    let mut known = Vec::new();
                    let mut unknown = Vec::new();
                    for part in fragments
                        .into_iter()
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                    {
                        if pattern.is_match(part) {
                            known.push(json!(part))
                        } else {
                            unknown.push(json!(part))
                        }
                    }
                    let state = status(!known.is_empty(), !unknown.is_empty());
                    let value = if typ == "reported_status" {
                        json!({"reported_statement":text,"labels":known})
                    } else {
                        json!(known)
                    };
                    result(value, state, unknown, vec![])
                }
                "text" => result(json!(text), "normalized", vec![], vec![]),
                _ => return Err(format!("unsupported value type: {typ}")),
            }
        };
        for issue in normalized["issues"]
            .as_array()
            .ok_or("normalization issues")?
        {
            issues.push(json!({"source_field":source,"issue":issue}));
        }
        let mut field = json!({"source_field":source,"target":rule["target"],"value_type":typ});
        field
            .as_object_mut()
            .unwrap()
            .extend(normalized.as_object().unwrap().clone());
        fields.push(field);
    }
    Ok(json!({"reported_fields":fields,"issues":issues}))
}
