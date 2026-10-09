//! Bounded Python-compatible value operations over the retained native JSON
//! representation.  This is mechanical value semantics only; source authority
//! and transport policy remain with their callers.

use crate::{
    FoundationError, FoundationErrorCode as Code, JsonLimits, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, Result, emit_python_compact_json, python_lower_unicode16_v1,
    python_printable_unicode16_v1,
};

fn budget() -> FoundationError {
    FoundationError::new(Code::BudgetExceeded, "Python value work budget exceeded")
}

fn invalid(detail: &'static str) -> FoundationError {
    FoundationError::new(Code::InvalidJson, detail)
}

fn normalized_integer(value: &str) -> String {
    let (negative, digits) = value
        .strip_prefix('-')
        .map_or((false, value), |digits| (true, digits));
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        "0".to_owned()
    } else if negative {
        format!("-{digits}")
    } else {
        digits.to_owned()
    }
}

fn number_integer_text(value: &JsonValue) -> Option<String> {
    let JsonValue::Number(number) = value else {
        return None;
    };
    match number.kind {
        JsonNumberKind::Int => Some(normalized_integer(&number.lexeme)),
        JsonNumberKind::Float => {
            let value = number.lexeme.parse::<f64>().ok()?;
            (value.is_finite() && value.fract() == 0.0)
                .then(|| normalized_integer(&format!("{value:.0}")))
        }
    }
}

fn number_float(value: &JsonValue) -> Option<f64> {
    match value {
        JsonValue::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        JsonValue::Number(number) => number.lexeme.parse::<f64>().ok(),
        _ => None,
    }
}

struct VisitBudget(usize);

impl VisitBudget {
    fn visit(&mut self) -> Result<()> {
        self.0 = self.0.checked_sub(1).ok_or_else(budget)?;
        Ok(())
    }
}

fn equal(left: &JsonValue, right: &JsonValue, visits: &mut VisitBudget) -> Result<bool> {
    visits.visit()?;
    if let (Some(a), Some(b)) = (number_integer_text(left), number_integer_text(right)) {
        return Ok(a == b);
    }
    if let (Some(integer), Some(float)) = (number_integer_text(left), number_float(right)) {
        return Ok(float.is_finite()
            && float.fract() == 0.0
            && integer == normalized_integer(&format!("{float:.0}")));
    }
    if let (Some(float), Some(integer)) = (number_float(left), number_integer_text(right)) {
        return Ok(float.is_finite()
            && float.fract() == 0.0
            && normalized_integer(&format!("{float:.0}")) == integer);
    }
    match (left, right) {
        (JsonValue::Number(a), JsonValue::Number(b)) => {
            Ok(a.lexeme.parse::<f64>().ok() == b.lexeme.parse::<f64>().ok())
        }
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                if !equal(a, b, visits)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            let mut left_entries: Vec<_> = a.iter().collect();
            let mut right_entries: Vec<_> = b.iter().collect();
            left_entries.sort_by(|(left, _), (right, _)| left.units().cmp(right.units()));
            right_entries.sort_by(|(left, _), (right, _)| left.units().cmp(right.units()));
            for ((left_key, value), (right_key, other)) in
                left_entries.into_iter().zip(right_entries)
            {
                if left_key != right_key {
                    return Ok(false);
                }
                if !equal(value, other, visits)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(left == right),
    }
}

fn unicode_code_points(units: &[u16]) -> Vec<u32> {
    let mut points = Vec::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        let first = units[index];
        if (0xD800..=0xDBFF).contains(&first)
            && units
                .get(index + 1)
                .is_some_and(|second| (0xDC00..=0xDFFF).contains(second))
        {
            let second = units[index + 1];
            points.push(0x10000 + (((first as u32 - 0xD800) << 10) | (second as u32 - 0xDC00)));
            index += 2;
        } else {
            // A lone surrogate is a Python string code point of its own.
            points.push(first as u32);
            index += 1;
        }
    }
    points
}

fn contains_code_points(haystack: &[u16], needle: &[u16]) -> bool {
    let haystack = unicode_code_points(haystack);
    let needle = unicode_code_points(needle);
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    let mut prefix = vec![0usize; needle.len()];
    let mut matched = 0;
    for index in 1..needle.len() {
        while matched > 0 && needle[index] != needle[matched] {
            matched = prefix[matched - 1];
        }
        if needle[index] == needle[matched] {
            matched += 1;
        }
        prefix[index] = matched;
    }
    matched = 0;
    for unit in haystack {
        while matched > 0 && unit != needle[matched] {
            matched = prefix[matched - 1];
        }
        if unit == needle[matched] {
            matched += 1;
            if matched == needle.len() {
                return true;
            }
        }
    }
    false
}

/// Python truthiness for strict JSON values, with booleans handled as booleans.
pub fn python_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(value) => *value,
        JsonValue::Number(number) => number.lexeme.parse::<f64>().is_ok_and(|value| value != 0.0),
        JsonValue::String(value) => !value.units().is_empty(),
        JsonValue::Array(value) => !value.is_empty(),
        JsonValue::Object(value) => !value.is_empty(),
    }
}

/// Python recursive equality, including bool/int equivalence and exact
/// integer-to-integral-float comparison. Object insertion order is ignored.
pub fn python_equals(left: &JsonValue, right: &JsonValue, max_visits: usize) -> Result<bool> {
    if max_visits == 0 {
        return Err(budget());
    }
    equal(left, right, &mut VisitBudget(max_visits))
}

/// Python membership for JSON strings, arrays and objects. Nested arrays and
/// objects remain unhashable as dictionary needles.
pub fn python_member(needle: &JsonValue, haystack: &JsonValue, max_visits: usize) -> Result<bool> {
    if max_visits == 0 {
        return Err(budget());
    }
    match haystack {
        JsonValue::String(value) => {
            let JsonValue::String(needle) = needle else {
                return Err(invalid("native string membership requires a string"));
            };
            Ok(contains_code_points(value.units(), needle.units()))
        }
        JsonValue::Array(values) => {
            let mut visits = VisitBudget(max_visits);
            for value in values {
                if equal(needle, value, &mut visits)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        JsonValue::Object(values) => {
            if matches!(needle, JsonValue::Array(_) | JsonValue::Object(_)) {
                return Err(invalid("unhashable native dictionary membership"));
            }
            let JsonValue::String(needle) = needle else {
                return Ok(false);
            };
            Ok(values.iter().any(|(key, _)| key == needle))
        }
        _ => Err(invalid("native membership requires a container")),
    }
}

struct TextBuilder {
    value: String,
    units: usize,
    maximum: usize,
}

impl TextBuilder {
    fn new(maximum: usize) -> Result<Self> {
        if maximum == 0 {
            return Err(budget());
        }
        Ok(Self {
            value: String::new(),
            units: 0,
            maximum,
        })
    }

    fn push(&mut self, value: &str) -> Result<()> {
        let units = value.encode_utf16().count();
        self.units = self
            .units
            .checked_add(units)
            .filter(|total| *total <= self.maximum)
            .ok_or_else(budget)?;
        self.value.push_str(value);
        Ok(())
    }

    fn finish(self) -> String {
        self.value
    }
}

fn quoted_python_string(value: &JsonString, out: &mut TextBuilder) -> Result<()> {
    let has_single = value.units().contains(&(b'\'' as u16));
    let has_double = value.units().contains(&(b'"' as u16));
    let quote = if has_single && !has_double { '"' } else { '\'' };
    out.push(&quote.to_string())?;
    let units = value.units();
    let mut index = 0;
    while index < units.len() {
        let first = units[index];
        if (0xD800..=0xDBFF).contains(&first) {
            if let Some(second) = units
                .get(index + 1)
                .copied()
                .filter(|v| (0xDC00..=0xDFFF).contains(v))
            {
                let point = 0x10000 + (((first as u32 - 0xD800) << 10) | (second as u32 - 0xDC00));
                let ch = char::from_u32(point).ok_or_else(|| invalid("invalid Unicode scalar"))?;
                if ch == quote || ch == '\\' {
                    out.push("\\")?;
                    out.push(&ch.to_string())?;
                } else if ch == '\n' {
                    out.push("\\n")?;
                } else if ch == '\r' {
                    out.push("\\r")?;
                } else if ch == '\t' {
                    out.push("\\t")?;
                } else if python_printable_unicode16_v1(ch) {
                    out.push(&ch.to_string())?;
                } else {
                    out.push(&format!("\\U{point:08x}"))?;
                }
                index += 2;
                continue;
            }
            out.push(&format!("\\u{first:04x}"))?;
            index += 1;
            continue;
        }
        if (0xDC00..=0xDFFF).contains(&first) {
            out.push(&format!("\\u{first:04x}"))?;
            index += 1;
            continue;
        }
        let ch = char::from_u32(first as u32).ok_or_else(|| invalid("invalid Unicode scalar"))?;
        if ch == quote || ch == '\\' {
            out.push("\\")?;
            out.push(&ch.to_string())?;
        } else if ch == '\n' {
            out.push("\\n")?;
        } else if ch == '\r' {
            out.push("\\r")?;
        } else if ch == '\t' {
            out.push("\\t")?;
        } else if python_printable_unicode16_v1(ch) {
            out.push(&ch.to_string())?;
        } else if (first as u32) <= 0xff {
            out.push(&format!("\\x{first:02x}"))?;
        } else {
            out.push(&format!("\\u{first:04x}"))?;
        }
        index += 1;
    }
    out.push(&quote.to_string())
}

fn json_scalar(value: &JsonValue) -> Result<String> {
    let limits = JsonLimits::new(8 * 1024 * 1024, 4, 16, 4300)?;
    String::from_utf8(emit_python_compact_json(value, limits)?)
        .map_err(|_| invalid("Python JSON scalar is not UTF-8"))
}

fn render_python(
    value: &JsonValue,
    top: bool,
    mode: TextMode,
    out: &mut TextBuilder,
) -> Result<()> {
    match value {
        JsonValue::Null => out.push("None"),
        JsonValue::Bool(value) => out.push(if *value { "True" } else { "False" }),
        JsonValue::Number(number) if number.kind == JsonNumberKind::Int => {
            out.push(&normalized_integer(&number.lexeme))
        }
        JsonValue::Number(_) => out.push(&json_scalar(value)?),
        JsonValue::String(value) if top && mode == TextMode::String => {
            let text = value.as_str().ok_or_else(|| {
                FoundationError::new(
                    Code::InvalidUnicodeScalar,
                    "Python string contains a lone surrogate",
                )
            })?;
            out.push(text)
        }
        JsonValue::String(value) => quoted_python_string(value, out),
        JsonValue::Array(values) => {
            out.push("[")?;
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(", ")?;
                }
                render_python(value, false, mode, out)?;
            }
            out.push("]")
        }
        JsonValue::Object(values) => {
            out.push("{")?;
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    out.push(", ")?;
                }
                quoted_python_string(key, out)?;
                out.push(": ")?;
                render_python(value, false, mode, out)?;
            }
            out.push("}")
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum TextMode {
    String,
    Repr,
}

/// Python `str()` over the strict JSON scalar/container model.
pub fn python_string(value: &JsonValue, max_utf16_units: usize) -> Result<String> {
    let mut out = TextBuilder::new(max_utf16_units)?;
    render_python(value, true, TextMode::String, &mut out)?;
    Ok(out.finish())
}

/// Python `repr()` over the strict JSON scalar/container model.
pub fn python_repr(value: &JsonValue, max_utf16_units: usize) -> Result<String> {
    let mut out = TextBuilder::new(max_utf16_units)?;
    render_python(value, true, TextMode::Repr, &mut out)?;
    Ok(out.finish())
}

fn render_searchable(value: &JsonValue, out: &mut TextBuilder) -> Result<()> {
    match value {
        JsonValue::Null => out.push("null"),
        JsonValue::Bool(value) => out.push(if *value { "true" } else { "false" }),
        JsonValue::Number(_) => out.push(&json_scalar(value)?),
        JsonValue::String(value) => {
            if value.as_str().is_none() {
                return Err(FoundationError::new(
                    Code::InvalidUnicodeScalar,
                    "search document contains a lone surrogate",
                ));
            }
            out.push(&json_scalar(&value_as_json(value))?)
        }
        JsonValue::Array(values) => {
            out.push("[")?;
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(", ")?;
                }
                render_searchable(value, out)?;
            }
            out.push("]")
        }
        JsonValue::Object(values) => {
            let mut entries: Vec<_> = values.iter().collect();
            if entries.iter().any(|(key, _)| key.as_str().is_none()) {
                return Err(FoundationError::new(
                    Code::InvalidUnicodeScalar,
                    "search document contains a lone surrogate key",
                ));
            }
            entries.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
            out.push("{")?;
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index > 0 {
                    out.push(", ")?;
                }
                out.push(&json_scalar(&JsonValue::String(key.clone()))?)?;
                out.push(": ")?;
                render_searchable(value, out)?;
            }
            out.push("}")
        }
    }
}

fn value_as_json(value: &JsonString) -> JsonValue {
    JsonValue::String(value.clone())
}

/// Python `json.dumps(value, ensure_ascii=False, sort_keys=True)` searchable
/// text with its default separators and the pinned Python lowercase operation.
pub fn python_searchable_text(value: &JsonValue, max_utf16_units: usize) -> Result<String> {
    let mut out = TextBuilder::new(max_utf16_units)?;
    render_searchable(value, &mut out)?;
    let text = out.finish();
    python_lower_unicode16_v1(
        &text,
        max_utf16_units,
        max_utf16_units.saturating_mul(2),
        max_utf16_units.saturating_mul(4),
    )
}

/// Lowercase using the pinned Python/Unicode16 primitive.
pub fn python_lower(value: &str, max_code_points: usize) -> Result<String> {
    if value.encode_utf16().count() > max_code_points {
        return Err(budget());
    }
    python_lower_unicode16_v1(
        value,
        max_code_points,
        max_code_points.saturating_mul(2),
        max_code_points.saturating_mul(4),
    )
}

/// Lower an exact UTF-16 string while retaining any legacy unpaired surrogate
/// units unchanged. Scalar runs are delegated to the pinned Unicode owner.
pub fn python_lower_json_string(value: &JsonString, max_units: usize) -> Result<JsonString> {
    if let Some(text) = value.as_str() {
        return Ok(JsonString::from_utf8(&python_lower(text, max_units)?));
    }
    if value.units().len() > max_units {
        return Err(budget());
    }
    let mut output = Vec::with_capacity(value.units().len());
    let mut run = Vec::new();
    let flush = |run: &mut Vec<u16>, output: &mut Vec<u16>| -> Result<()> {
        if !run.is_empty() {
            let text =
                String::from_utf16(run).map_err(|_| invalid("invalid Unicode scalar run"))?;
            output.extend(python_lower(&text, max_units)?.encode_utf16());
            run.clear();
        }
        Ok(())
    };
    let mut index = 0;
    while index < value.units().len() {
        let unit = value.units()[index];
        if (0xD800..=0xDBFF).contains(&unit) {
            if value
                .units()
                .get(index + 1)
                .is_some_and(|next| (0xDC00..=0xDFFF).contains(next))
            {
                run.push(unit);
                run.push(value.units()[index + 1]);
                index += 2;
                continue;
            }
            flush(&mut run, &mut output)?;
            output.push(unit);
        } else if (0xDC00..=0xDFFF).contains(&unit) {
            flush(&mut run, &mut output)?;
            output.push(unit);
        } else {
            run.push(unit);
        }
        index += 1;
    }
    flush(&mut run, &mut output)?;
    if output.len() > max_units.saturating_mul(2) {
        return Err(budget());
    }
    Ok(JsonString::from_utf16(&output))
}

/// Format one finite `f64` with the same Python repr selected by the foundation
/// JSON writer. The returned spelling remains numeric source text for callers.
pub fn python_float_text(value: f64) -> Result<String> {
    if !value.is_finite() {
        return Err(FoundationError::new(
            Code::NonfiniteFloat,
            "nonfinite Python float",
        ));
    }
    let number = JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Float,
        lexeme: value.to_string(),
    });
    json_scalar(&number)
}

/// Python searchable rank values from ordered display fields. Strings inside
/// one display object retain source insertion order just as the Worker carrier.
pub fn python_search_rank_values(
    value: &JsonValue,
    fields: &[String],
    max_values: usize,
    max_utf16_units: usize,
) -> Result<Vec<JsonString>> {
    if max_values == 0 || max_utf16_units == 0 {
        return Err(budget());
    }
    let Some(display) = value.object_get("display") else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    let mut total_units = 0usize;
    for field in fields {
        let Some(item) = display.object_get(field) else {
            continue;
        };
        let mut selected = Vec::new();
        if let JsonValue::String(text) = item {
            selected.push(text.clone());
        } else if let Some(entries) = item.as_object() {
            for (_, candidate) in entries {
                if let JsonValue::String(text) = candidate {
                    selected.push(text.clone());
                }
            }
        }
        for text in selected {
            if result.len() >= max_values {
                return Err(budget());
            }
            total_units = total_units
                .checked_add(text.units().len())
                .filter(|total| *total <= max_utf16_units)
                .ok_or_else(budget)?;
            result.push(python_lower_json_string(&text, max_utf16_units)?);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JsonMode, parse_json};

    fn parse(raw: &str) -> JsonValue {
        parse_json(
            raw.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root()
    }

    #[test]
    fn python_value_rules_preserve_numeric_and_container_semantics() {
        let cases = [
            ("false", "0", true),
            ("0", "-0.0", true),
            ("true", "1.0", true),
            ("9007199254740993", "9007199254740992.0", false),
            ("[false,1.0]", "[0, true]", true),
            ("{\"a\":1,\"b\":2}", "{\"b\":2.0,\"a\":1}", true),
        ];
        for (left, right, expected) in cases {
            assert_eq!(
                python_equals(&parse(left), &parse(right), 32).unwrap(),
                expected
            );
        }
        assert!(python_truthy(&parse("[0]")));
        assert!(!python_truthy(&parse("0.0")));
        assert!(python_member(&parse("1.0"), &parse("[true,2]"), 32).unwrap());
    }

    #[test]
    fn python_text_and_searchable_match_python_default_spellings() {
        let value = parse("{\"z\":1.0,\"a\":[false,\"x\\n\"]}");
        assert_eq!(
            python_string(&value, 256).unwrap(),
            "{'z': 1.0, 'a': [False, 'x\\n']}"
        );
        assert_eq!(
            python_searchable_text(&value, 256).unwrap(),
            "{\"a\": [false, \"x\\n\"], \"z\": 1.0}"
        );
        assert_eq!(python_float_text(-0.0).unwrap(), "-0.0");
    }

    #[test]
    fn wtf16_membership_repr_lower_and_search_refusal_keep_their_boundaries() {
        let lone = parse("\"\\ud800\"");
        let haystack = parse("\"x\\ud800y\"");
        assert!(python_member(&lone, &haystack, 16).unwrap());
        assert!(!python_member(&parse("\"\\ud83d\""), &parse("\"😀\""), 16).unwrap());
        assert!(python_member(&parse("\"😀\""), &parse("\"x😀y\""), 16).unwrap());
        assert_eq!(python_repr(&lone, 16).unwrap(), "'\\ud800'");
        let lowered = match &lone {
            JsonValue::String(value) => python_lower_json_string(value, 16).unwrap(),
            _ => unreachable!(),
        };
        assert_eq!(lowered.units(), &[0xd800]);
        assert!(python_searchable_text(&lone, 16).is_err());
    }

    #[test]
    fn object_equality_ignores_insertion_order_with_a_bounded_sorted_walk() {
        let left = parse("{\"a\":1,\"b\":2,\"c\":3}");
        let right = parse("{\"c\":3.0,\"a\":true,\"b\":2}");
        assert!(python_equals(&left, &right, 8).unwrap());
        assert!(python_equals(&left, &right, 2).is_err());
    }
}
