//! RFC 8785 encoding with ToS's lossless numeric admission policy.
use crate::{
    FoundationError, FoundationErrorCode as Code, JsonLimits, JsonMode, JsonNumber, JsonString,
    JsonValue, Result, parse_json,
};

pub const JCS_PROFILE: &str = "jcs-rfc8785-v1";

fn invalid(detail: &str) -> FoundationError {
    FoundationError::new(Code::UnsupportedCanonicalNumber, detail)
}

// Compare decimal values without rounding or constructing powers of ten.
fn decimal_parts(s: &str) -> Option<(bool, String, i128)> {
    let negative = s.starts_with('-');
    let s = s.strip_prefix('-').unwrap_or(s);
    let (mantissa, exponent) = s.split_once(['e', 'E']).unwrap_or((s, "0"));
    let decimals = mantissa.split_once('.').map_or(0, |(_, x)| x.len());
    let digits = mantissa.replace('.', "");
    let significant = digits.trim_start_matches('0').trim_end_matches('0');
    if significant.is_empty() {
        return Some((false, String::new(), 0));
    }
    let trailing = digits.len() - digits.trim_end_matches('0').len();
    // An exponent outside i128 cannot be offset by the length of a lexeme
    // held in memory, so it cannot match a finite JCS output.
    let power = exponent
        .parse::<i128>()
        .ok()?
        .checked_sub(decimals as i128)?
        .checked_add(trailing as i128)?;
    Some((negative, significant.to_owned(), power))
}

fn number(n: &JsonNumber, limits: JsonLimits) -> Result<String> {
    // Values can be built without the parser; validate their grammar and kind.
    let doc = parse_json(n.lexeme.as_bytes(), JsonMode::PublishedStrict, limits)?;
    if !matches!(doc.root(), JsonValue::Number(parsed) if parsed.kind == n.kind) {
        return Err(invalid("invalid number kind"));
    }
    let f: f64 = n
        .lexeme
        .parse()
        .map_err(|_| invalid("invalid binary64 number"))?;
    if !f.is_finite() {
        return Err(FoundationError::new(
            Code::NonfiniteFloat,
            "JCS requires a finite number",
        ));
    }
    if f == 0.0 && n.lexeme.starts_with('-') {
        return Err(invalid("negative zero is not admitted"));
    }
    let mut buffer = ryu_js::Buffer::new();
    let out = buffer.format_finite(f).to_owned();
    if decimal_parts(&n.lexeme).is_none_or(|input| Some(input) != decimal_parts(&out)) {
        return Err(invalid("JCS decimal spelling would change the exact value"));
    }
    Ok(out)
}

/// Recursively encode numbers that JCS cannot represent exactly as strings
/// containing their exact input decimal spelling. Callers write this returned
/// value and compute its `jcs1` digest as one operation. Other values, including
/// strings with retained lone surrogates, remain unchanged for that writer to
/// admit or refuse.
pub fn stringify_non_jcs_numbers_v1(value: &JsonValue, limits: JsonLimits) -> Result<JsonValue> {
    crate::emit_value_preserved_json(value, limits)?;

    fn normalize(value: &JsonValue, limits: JsonLimits) -> Result<JsonValue> {
        match value {
            JsonValue::Number(n) => {
                let parsed = parse_json(n.lexeme.as_bytes(), JsonMode::PublishedStrict, limits)?;
                if !matches!(parsed.root(), JsonValue::Number(parsed) if parsed.kind == n.kind) {
                    return Err(invalid("invalid number kind"));
                }
                match number(n, limits) {
                    Ok(_) if requires_jcs1_string(n) => {
                        Ok(JsonValue::String(JsonString::from_utf8(&n.lexeme)))
                    }
                    Ok(_) => Ok(JsonValue::Number(n.clone())),
                    Err(error)
                        if error.code == Code::UnsupportedCanonicalNumber
                            || (error.code == Code::NonfiniteFloat
                                && n.kind == crate::JsonNumberKind::Int) =>
                    {
                        Ok(JsonValue::String(JsonString::from_utf8(&n.lexeme)))
                    }
                    Err(error) => Err(error),
                }
            }
            JsonValue::Array(values) => values
                .iter()
                .map(|value| normalize(value, limits))
                .collect::<Result<Vec<_>>>()
                .map(JsonValue::Array),
            JsonValue::Object(entries) => entries
                .iter()
                .map(|(key, value)| Ok((key.clone(), normalize(value, limits)?)))
                .collect::<Result<Vec<_>>>()
                .map(JsonValue::Object),
            value => Ok(value.clone()),
        }
    }

    normalize(value, limits)
}

/// `jcs1` keeps the exact ToS integer range even when IEEE 754 can represent
/// a larger integer exactly.
pub(crate) fn requires_jcs1_string(n: &JsonNumber) -> bool {
    if n.kind == crate::JsonNumberKind::Int {
        let digits = n.lexeme.strip_prefix('-').unwrap_or(&n.lexeme);
        let digits = digits.trim_start_matches('0');
        let boundary = "9007199254740992";
        digits.len() > boundary.len() || (digits.len() == boundary.len() && digits > boundary)
    } else {
        n.lexeme
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value.abs() > 9_007_199_254_740_992.0)
    }
}

fn string(s: &JsonString, out: &mut Vec<u8>) -> Result<()> {
    let s = s.as_str().ok_or_else(|| {
        FoundationError::new(Code::InvalidUnicodeScalar, "JCS rejects lone surrogates")
    })?;
    out.push(b'"');
    for c in s.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{8}' => out.extend_from_slice(b"\\b"),
            '\t' => out.extend_from_slice(b"\\t"),
            '\n' => out.extend_from_slice(b"\\n"),
            '\u{c}' => out.extend_from_slice(b"\\f"),
            '\r' => out.extend_from_slice(b"\\r"),
            c if c <= '\u{1f}' => out.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes()),
            c => {
                let mut bytes = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut bytes).as_bytes());
            }
        }
    }
    out.push(b'"');
    Ok(())
}

/// Encode without LF. `limits` bound input shape and produced bytes.
/// The input and JCS output must have the same exact decimal value. Negative
/// zero is refused; different spellings of the same decimal value are allowed.
pub fn jcs_bytes_v1(value: &JsonValue, limits: JsonLimits) -> Result<Vec<u8>> {
    // Reuse the existing bounded traversal for depth and visit validation. Its
    // output is discarded and never determines JCS bytes.
    crate::emit_value_preserved_json(value, limits)?;
    fn emit(v: &JsonValue, out: &mut Vec<u8>, limits: JsonLimits) -> Result<()> {
        match v {
            JsonValue::Null => out.extend_from_slice(b"null"),
            JsonValue::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
            JsonValue::Number(n) => out.extend_from_slice(number(n, limits)?.as_bytes()),
            JsonValue::String(s) => string(s, out)?,
            JsonValue::Array(a) => {
                out.push(b'[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    emit(x, out, limits)?;
                }
                out.push(b']');
            }
            JsonValue::Object(o) => {
                let mut entries: Vec<_> = o.iter().collect();
                entries.sort_by(|a, b| a.0.units().cmp(b.0.units()));
                for pair in entries.windows(2) {
                    if pair[0].0.units() == pair[1].0.units() {
                        return Err(FoundationError::new(
                            Code::DuplicateMember,
                            "duplicate JCS member",
                        ));
                    }
                }
                out.push(b'{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    string(k, out)?;
                    out.push(b':');
                    emit(v, out, limits)?;
                }
                out.push(b'}');
            }
        }
        if out.len() > limits.max_bytes {
            return Err(FoundationError::new(
                Code::BudgetExceeded,
                "JCS byte limit exceeded",
            ));
        }
        Ok(())
    }
    let mut out = Vec::new();
    emit(value, &mut out, limits)?;
    Ok(out)
}

/// Strict parse and encode, rejecting duplicate source members.
pub fn jcs_raw_bytes_v1(raw: &[u8], limits: JsonLimits) -> Result<Vec<u8>> {
    let doc = parse_json(raw, JsonMode::PublishedStrict, limits)?;
    jcs_bytes_v1(doc.root(), limits)
}

#[cfg(test)]
mod tests {
    use super::{jcs_bytes_v1, stringify_non_jcs_numbers_v1};
    use crate::{JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json};

    #[test]
    fn refused_numbers_round_trip_as_exact_decimal_strings() {
        let limits = JsonLimits::default();
        let raw = br#"{"object":{"extensions":{"large_integer":9007199254740993,"negative_zero":-0.0,"inexact_decimal":0.100000000000000005,"exact_integer":42,"safe_boundary":9007199254740992,"over_boundary_integer":9007199254740994,"over_boundary_decimal":9007199254740994.0}}}"#;
        let input = parse_json(raw, JsonMode::PublishedStrict, limits).unwrap();
        let normalized = stringify_non_jcs_numbers_v1(input.root(), limits).unwrap();
        let bytes = emit_value_preserved_json(&normalized, limits).unwrap();
        let round_trip = parse_json(&bytes, JsonMode::PublishedStrict, limits).unwrap();

        assert_eq!(round_trip.root(), &normalized);
        let extensions = round_trip
            .root()
            .object_get("object")
            .and_then(|value| value.object_get("extensions"))
            .unwrap();
        assert_eq!(
            extensions.object_get("large_integer").unwrap().as_str(),
            Some("9007199254740993")
        );
        assert_eq!(
            extensions.object_get("negative_zero").unwrap().as_str(),
            Some("-0.0")
        );
        assert_eq!(
            extensions.object_get("inexact_decimal").unwrap().as_str(),
            Some("0.100000000000000005")
        );
        assert_eq!(
            extensions.object_get("exact_integer").unwrap().as_u64(),
            Some(42)
        );
        assert_eq!(
            extensions.object_get("safe_boundary").unwrap().as_u64(),
            Some(9_007_199_254_740_992)
        );
        assert_eq!(
            extensions
                .object_get("over_boundary_integer")
                .unwrap()
                .as_str(),
            Some("9007199254740994")
        );
        assert_eq!(
            extensions
                .object_get("over_boundary_decimal")
                .unwrap()
                .as_str(),
            Some("9007199254740994.0")
        );
        assert!(jcs_bytes_v1(&normalized, limits).is_ok());
    }

    #[test]
    fn number_normalization_preserves_lone_surrogate_for_the_jcs_writer() {
        let limits = JsonLimits::default();
        let input = parse_json(
            br#"{"extension":"\ud800","large_integer":9007199254740994}"#,
            JsonMode::PublishedStrict,
            limits,
        )
        .unwrap();
        let normalized = stringify_non_jcs_numbers_v1(input.root(), limits).unwrap();
        let JsonValue::String(extension) = normalized.object_get("extension").unwrap() else {
            panic!("extension stays a string");
        };
        assert!(extension.has_lone_surrogate());
        assert_eq!(
            normalized.object_get("large_integer").unwrap().as_str(),
            Some("9007199254740994")
        );
        assert!(jcs_bytes_v1(&normalized, limits).is_err());
    }
}
