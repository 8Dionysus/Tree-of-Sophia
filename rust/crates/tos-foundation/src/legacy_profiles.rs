//! Frozen canonical encodings retained only to verify historical digests.
//!
//! New records use JCS. Keep these bytes independent from the live JSON writer;
//! remove this module when tracked records and operator stores no longer use the
//! corresponding profile.
use std::collections::HashSet;

use crate::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode as Code, JsonLimits,
    JsonMode, JsonNumber, JsonNumberKind, JsonValue, Result, parse_json,
};

pub(crate) fn verify(
    value: &JsonValue,
    profile: CanonicalProfile,
    expected: Digest256,
    limits: JsonLimits,
) -> Result<bool> {
    Ok(Digest256::of_bytes(&encode(value, profile, limits)?) == expected)
}

/// Compare a bounded raw JSON fragment with its frozen historical encoding.
/// The caller supplies the already parsed fragment and an output limit that
/// covers both the fragment and its canonical form.
pub fn verify_raw_fragment(
    value: &JsonValue,
    profile: CanonicalProfile,
    raw: &[u8],
    limits: JsonLimits,
) -> Result<bool> {
    validate_limits(limits)?;
    if raw.len() > limits.max_bytes {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON byte budget exceeded",
        ));
    }
    Ok(encode(value, profile, limits)?.as_slice() == raw)
}

fn encode(value: &JsonValue, profile: CanonicalProfile, limits: JsonLimits) -> Result<Vec<u8>> {
    validate_limits(limits)?;
    let mut bytes = Vec::new();
    let mut visits = 0;
    write_value(value, 0, &mut visits, &mut bytes, limits)?;
    if profile == CanonicalProfile::CorpusSnapshotV1 {
        append(&mut bytes, b"\n", limits)?;
    }
    Ok(bytes)
}

fn validate_limits(limits: JsonLimits) -> Result<()> {
    if limits.max_bytes == 0
        || limits.max_depth == 0
        || limits.max_visits == 0
        || limits.max_integer_digits == 0
    {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON limits must be positive",
        ));
    }
    if limits.max_depth > 128 {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON depth limit exceeds safe maximum",
        ));
    }
    Ok(())
}

fn append(out: &mut Vec<u8>, bytes: &[u8], limits: JsonLimits) -> Result<()> {
    let next = out
        .len()
        .checked_add(bytes.len())
        .filter(|next| *next <= limits.max_bytes)
        .ok_or_else(|| {
            FoundationError::new(Code::BudgetExceeded, "JSON output byte budget exceeded")
        })?;
    out.try_reserve(bytes.len()).map_err(|_| {
        FoundationError::new(Code::BudgetExceeded, "legacy verifier output allocation")
    })?;
    out.extend_from_slice(bytes);
    debug_assert_eq!(out.len(), next);
    Ok(())
}

fn write_value(
    value: &JsonValue,
    depth: usize,
    visits: &mut usize,
    out: &mut Vec<u8>,
    limits: JsonLimits,
) -> Result<()> {
    if depth > limits.max_depth || *visits >= limits.max_visits {
        return Err(FoundationError::new(
            Code::BudgetExceeded,
            "JSON output structural budget exceeded",
        ));
    }
    *visits += 1;
    match value {
        JsonValue::Null => append(out, b"null", limits),
        JsonValue::Bool(true) => append(out, b"true", limits),
        JsonValue::Bool(false) => append(out, b"false", limits),
        JsonValue::Number(number) => write_number(number, out, limits),
        JsonValue::String(value) => write_string(value, out, limits),
        JsonValue::Array(items) => {
            append(out, b"[", limits)?;
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    append(out, b",", limits)?;
                }
                write_value(item, depth + 1, visits, out, limits)?;
            }
            append(out, b"]", limits)
        }
        JsonValue::Object(entries) => {
            if entries.len() > limits.max_visits.saturating_sub(*visits) {
                return Err(FoundationError::new(
                    Code::BudgetExceeded,
                    "JSON output structural budget exceeded",
                ));
            }
            let mut ordered = Vec::new();
            ordered.try_reserve_exact(entries.len()).map_err(|_| {
                FoundationError::new(Code::BudgetExceeded, "legacy verifier sort allocation")
            })?;
            let mut seen = HashSet::new();
            for (key, item) in entries {
                if !seen.insert(key.units()) {
                    return Err(FoundationError::new(
                        Code::DuplicateMember,
                        "duplicate decoded JSON member",
                    ));
                }
                ordered.push((key, item));
            }
            ordered.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
            append(out, b"{", limits)?;
            for (index, (key, item)) in ordered.into_iter().enumerate() {
                if index != 0 {
                    append(out, b",", limits)?;
                }
                write_string(key, out, limits)?;
                append(out, b":", limits)?;
                write_value(item, depth + 1, visits, out, limits)?;
            }
            append(out, b"}", limits)
        }
    }
}

fn write_number(number: &JsonNumber, out: &mut Vec<u8>, limits: JsonLimits) -> Result<()> {
    // The existing strict parser remains the number grammar and integer-digit
    // authority. This module owns the historical output spelling itself.
    let document = parse_json(number.lexeme.as_bytes(), JsonMode::PublishedStrict, limits)?;
    if !matches!(document.root(), JsonValue::Number(parsed) if parsed == number) {
        return Err(FoundationError::new(
            Code::InvalidNumber,
            "number lexeme and kind disagree",
        ));
    }
    if number.kind == JsonNumberKind::Float {
        let value = number
            .lexeme
            .parse::<f64>()
            .map_err(|_| FoundationError::new(Code::InvalidNumber, "float lexeme is invalid"))?;
        append(out, python_float_text(value).as_bytes(), limits)
    } else if number.lexeme == "-0" {
        append(out, b"0", limits)
    } else {
        append(out, number.lexeme.as_bytes(), limits)
    }
}

fn write_string(value: &crate::JsonString, out: &mut Vec<u8>, limits: JsonLimits) -> Result<()> {
    let text = value.as_str().ok_or_else(|| {
        FoundationError::new(
            Code::InvalidUnicodeScalar,
            "canonical UTF-8 cannot encode a lone surrogate",
        )
    })?;
    append(out, b"\"", limits)?;
    for character in text.chars() {
        match character {
            '"' => append(out, b"\\\"", limits)?,
            '\\' => append(out, b"\\\\", limits)?,
            '\u{0008}' => append(out, b"\\b", limits)?,
            '\u{000c}' => append(out, b"\\f", limits)?,
            '\n' => append(out, b"\\n", limits)?,
            '\r' => append(out, b"\\r", limits)?,
            '\t' => append(out, b"\\t", limits)?,
            c if (c as u32) < 32 => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let byte = c as u8;
                append(
                    out,
                    &[
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[(byte >> 4) as usize],
                        HEX[(byte & 15) as usize],
                    ],
                    limits,
                )?;
            }
            c => {
                let mut encoded = [0u8; 4];
                append(out, c.encode_utf8(&mut encoded).as_bytes(), limits)?;
            }
        }
    }
    append(out, b"\"", limits)
}

fn python_float_text(value: f64) -> String {
    use std::fmt::Write as _;

    let mut buffer = ryu::Buffer::new();
    let shortest = buffer.format_finite(value.abs());
    let mut result = String::new();
    if value == 0.0 {
        result.push_str(if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        });
        return result;
    }
    let negative = value.is_sign_negative();
    let (mantissa, suffix) = match shortest.find('e') {
        Some(at) => (
            &shortest[..at],
            shortest[at + 1..]
                .parse::<i32>()
                .expect("finite f64 exponent"),
        ),
        None => (shortest, 0),
    };
    let decimal_position = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let digits = || mantissa.bytes().filter(|byte| *byte != b'.');
    let leading = digits().take_while(|byte| *byte == b'0').count();
    let total = digits().count();
    let trailing = digits().rev().take_while(|byte| *byte == b'0').count();
    let length = total - leading - trailing;
    let exponent = suffix + decimal_position - 1 - leading as i32;
    let write_digits = |out: &mut String, start: usize, end: usize| {
        for digit in digits().skip(leading + start).take(end - start) {
            out.push(digit as char);
        }
    };
    if negative {
        result.push('-');
    }
    if (-4..16).contains(&exponent) {
        let point = exponent + 1;
        if point <= 0 {
            result.push_str("0.");
            for _ in 0..-point {
                result.push('0');
            }
            write_digits(&mut result, 0, length);
        } else if point as usize >= length {
            write_digits(&mut result, 0, length);
            for _ in length..point as usize {
                result.push('0');
            }
            result.push_str(".0");
        } else {
            write_digits(&mut result, 0, point as usize);
            result.push('.');
            write_digits(&mut result, point as usize, length);
        }
    } else {
        write_digits(&mut result, 0, 1);
        if length > 1 {
            result.push('.');
            write_digits(&mut result, 1, length);
        }
        write!(
            result,
            "e{}{:02}",
            if exponent < 0 { '-' } else { '+' },
            exponent.unsigned_abs()
        )
        .expect("String formatting");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{encode, verify};
    use crate::{CanonicalProfile, Digest256, JsonLimits, JsonMode, parse_json};

    fn value(raw: &str) -> crate::JsonValue {
        parse_json(
            raw.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap()
        .into_root()
    }

    #[test]
    fn raw_fragment_verification_uses_the_frozen_profile_bytes() {
        let record = value(r#"{"b":2,"a":1}"#);
        let limits = JsonLimits::default();
        assert!(
            super::verify_raw_fragment(
                &record,
                CanonicalProfile::SourceRecordDigestV1,
                b"{\"a\":1,\"b\":2}",
                limits,
            )
            .unwrap()
        );
        assert!(
            !super::verify_raw_fragment(
                &record,
                CanonicalProfile::SourceRecordDigestV1,
                b"{\"b\":2,\"a\":1}",
                limits,
            )
            .unwrap()
        );
        assert!(
            !super::verify_raw_fragment(
                &record,
                CanonicalProfile::SourceRecordDigestV1,
                b"{\"a\":1,\"b\":2}\n",
                limits,
            )
            .unwrap()
        );
        assert!(
            super::verify_raw_fragment(
                &record,
                CanonicalProfile::CorpusSnapshotV1,
                b"{\"a\":1,\"b\":2}\n",
                limits,
            )
            .unwrap()
        );
    }

    #[test]
    fn historical_recorded_bytes_include_profile_line_feed_and_exact_integers() {
        let parsed = value(r#"{"b":-0,"a":900719925474099312345678901234567890}"#);
        let no_lf = b"{\"a\":900719925474099312345678901234567890,\"b\":0}";
        assert_eq!(
            encode(
                &parsed,
                CanonicalProfile::SourceRecordDigestV1,
                JsonLimits::default()
            )
            .unwrap(),
            no_lf
        );
        assert_eq!(
            encode(
                &parsed,
                CanonicalProfile::SourceCommandInputV1,
                JsonLimits::default()
            )
            .unwrap(),
            no_lf
        );
        let mut snapshot = no_lf.to_vec();
        snapshot.push(b'\n');
        assert_eq!(
            encode(
                &parsed,
                CanonicalProfile::CorpusSnapshotV1,
                JsonLimits::default()
            )
            .unwrap(),
            snapshot
        );
        assert!(
            verify(
                &value(r#"{"n":1.0}"#),
                CanonicalProfile::CorpusSnapshotV1,
                Digest256::of_bytes(b"{\"n\":1.0}\n"),
                JsonLimits::default()
            )
            .unwrap()
        );
    }

    #[test]
    fn historical_python_float_layout_vectors_remain_exact() {
        for (raw, expected) in [
            ("-0.0", "-0.0"),
            ("1.0", "1.0"),
            ("1e-4", "0.0001"),
            ("1e-5", "1e-05"),
            ("1e15", "1000000000000000.0"),
            ("1e16", "1e+16"),
            ("1e20", "1e+20"),
            ("1.234e-7", "1.234e-07"),
            ("5e-324", "5e-324"),
            ("1.7976931348623157e308", "1.7976931348623157e+308"),
            ("1e-400", "0.0"),
        ] {
            assert_eq!(
                encode(
                    &value(raw),
                    CanonicalProfile::SourceRecordDigestV1,
                    JsonLimits::default()
                )
                .unwrap(),
                expected.as_bytes(),
                "{raw}"
            );
        }
    }
}
