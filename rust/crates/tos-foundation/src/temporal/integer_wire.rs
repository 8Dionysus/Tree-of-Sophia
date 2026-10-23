//! Explicit opt-in exact-integer transport. Legacy native/request ABIs do not
//! acquire browser precision guarantees from this adapter. No f64 or semantic
//! saturation; arbitrary source strings are never interpreted as numbers.
use super::*;
use crate::{Digest256, JsonLimits};

pub const INTEGER_WIRE_SCHEMA: &str = "tos_exact_integer_wire_v1";
pub const SAFE_INTEGER_ENCODING: &str = "SafeJsonNumberV1";
pub const ELAPSED_MAGNITUDE_PROFILE: &str = "CanonicalDecimalI128StringV1";
pub const MAX_SAFE_JSON_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerWireSafety {
    Safe,
    Unsupported(&'static str),
}
/// Borrowed outer framing only. Callers MUST refuse an encoding other than
/// SAFE_INTEGER_ENCODING before invoking a generic native operation. The closed
/// decimal difference adapter separately admits its two typed coordinate paths.
#[derive(Clone, Copy, Debug)]
pub struct IntegerWireRequest<'a> {
    pub integer_encoding: &'a str,
    pub operation: &'a str,
    pub payload: &'a JsonValue,
}
fn limits(b: TemporalBudget) -> JsonLimits {
    JsonLimits {
        max_bytes: b.max_expression_bytes,
        max_depth: 64,
        max_visits: b.max_members,
        max_integer_digits: b.max_expression_bytes,
    }
}
fn preserved(v: &JsonValue, b: TemporalBudget) -> Result<Vec<u8>, TemporalError> {
    crate::emit_value_preserved_json(v, limits(b)).map_err(|e| {
        if e.code == crate::FoundationErrorCode::BudgetExceeded {
            TemporalError::ResourceExhausted("integer wire byte/member/depth limit")
        } else {
            TemporalError::MalformedRequest("invalid integer wire JSON")
        }
    })
}
pub fn parse_integer_wire_request(
    v: &JsonValue,
    b: TemporalBudget,
) -> Result<IntegerWireRequest<'_>, TemporalError> {
    preserved(v, b)?;
    let members = v.as_object().ok_or(TemporalError::MalformedRequest(
        "integer wire outer object required",
    ))?;
    let keys = ["schema_version", "integer_encoding", "operation", "payload"];
    if members.len() != keys.len()
        || members.iter().enumerate().any(|(i, (k, _))| {
            !keys.iter().any(|s| k.as_str() == Some(*s))
                || members[..i].iter().any(|(other, _)| other == k)
        })
    {
        return Err(TemporalError::MalformedRequest(
            "integer wire exact outer keys required",
        ));
    }
    if string(v, "schema_version") != Some(INTEGER_WIRE_SCHEMA) {
        return Err(TemporalError::MalformedRequest(
            "unsupported integer wire schema",
        ));
    }
    let word = |key| {
        string(v, key)
            .filter(|s| !s.trim().is_empty())
            .ok_or(TemporalError::MalformedRequest(
                "integer wire nonempty selector required",
            ))
    };
    let payload = field(v, "payload");
    if payload.as_object().is_none() {
        return Err(TemporalError::MalformedRequest(
            "integer wire payload object required",
        ));
    }
    Ok(IntegerWireRequest {
        integer_encoding: word("integer_encoding")?,
        operation: word("operation")?,
        payload,
    })
}
fn canonical_integer(s: &str) -> bool {
    if s == "0" {
        return true;
    }
    let digits = s.strip_prefix('-').unwrap_or(s).as_bytes();
    digits.first().is_some_and(|c| matches!(*c, b'1'..=b'9'))
        && digits.iter().all(u8::is_ascii_digit)
}
/// Only for an explicitly typed native Elapsed magnitude, not source strings.
/// The String stays a String; parsing validates grammar/range, never retypes it.
pub fn validate_elapsed_magnitude_string(v: &JsonValue) -> IntegerWireSafety {
    let Some(s) = v.as_str() else {
        return IntegerWireSafety::Unsupported("ElapsedMagnitudeStringRequired");
    };
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.len() > 39 || !canonical_integer(s) || s.parse::<i128>().is_err() {
        IntegerWireSafety::Unsupported("NonCanonicalElapsedMagnitude")
    } else {
        IntegerWireSafety::Safe
    }
}
#[derive(Default)]
struct Scan {
    nodes: usize,
    depth: usize,
    reason: Option<&'static str>,
}
fn scan(v: &JsonValue, depth: usize, s: &mut Scan) {
    s.nodes += 1;
    s.depth = s.depth.max(depth);
    match v {
        JsonValue::Number(n) => {
            let reason = if n.kind != JsonNumberKind::Int {
                Some("JsonFloatNotSupported")
            } else if !canonical_integer(&n.lexeme) {
                Some("NonCanonicalJsonInteger")
            } else if n.lexeme.len() > 17
                || n.lexeme
                    .parse::<i64>()
                    .ok()
                    .is_none_or(|n| !(-MAX_SAFE_JSON_INTEGER..=MAX_SAFE_JSON_INTEGER).contains(&n))
            {
                Some("ExactIntegerOutOfWireRange")
            } else {
                None
            };
            if s.reason.is_none() {
                s.reason = reason;
            }
        }
        JsonValue::Array(a) => {
            for v in a {
                scan(v, depth + 1, s);
            }
        }
        JsonValue::Object(o) => {
            for (_, v) in o {
                scan(v, depth + 1, s);
            }
        }
        _ => {}
    }
}
/// The complete JSON value is admitted before walking its numeric leaves.
/// Every number (including unknown raw evidence/echoes) must be canonical Safe
/// Int. Strings remain opaque; no blind decimal-string conversion occurs.
pub fn check_integer_wire_value(
    v: &JsonValue,
    b: TemporalBudget,
) -> Result<IntegerWireSafety, TemporalError> {
    preserved(v, b)?;
    let mut s = Scan::default();
    scan(v, 0, &mut s);
    Ok(s.reason
        .map_or(IntegerWireSafety::Safe, IntegerWireSafety::Unsupported))
}
#[derive(Clone, Copy)]
enum OutputProfile {
    Safe,
    DecimalDifference,
}
fn header(
    operation: &str,
    profile: OutputProfile,
    b: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    if operation.trim().is_empty() {
        return Err(TemporalError::MalformedRequest(
            "integer wire operation required",
        ));
    }
    if operation.len() > b.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted(
            "integer wire operation limit",
        ));
    }
    let mut result = obj(vec![
        ("schema_version", text("tos_exact_integer_wire_result_v1")),
        (
            "integer_encoding",
            text(match profile {
                OutputProfile::Safe => SAFE_INTEGER_ENCODING,
                OutputProfile::DecimalDifference => "CanonicalDecimalStringV1",
            }),
        ),
        ("operation", text(operation)),
        ("elapsed_magnitude_profile", text(ELAPSED_MAGNITUDE_PROFILE)),
    ]);
    if matches!(profile, OutputProfile::DecimalDifference) {
        put(
            &mut result,
            "supported_scope",
            text("DifferenceCoordinateI64V1"),
        );
    }
    Ok(result)
}
fn admit_replacement(
    header_bytes: usize,
    placeholder_bytes: usize,
    replacement_bytes: usize,
    b: TemporalBudget,
) -> Result<(), TemporalError> {
    let size = header_bytes
        .checked_sub(placeholder_bytes)
        .and_then(|n| n.checked_add(replacement_bytes));
    if size.is_none_or(|n| n > b.max_expression_bytes) {
        Err(TemporalError::ResourceExhausted(
            "integer wire envelope limit",
        ))
    } else {
        Ok(())
    }
}
/// Explicit refusal for unsupported encoding/profile or unsafe output. No
/// payload, success cursor or unsafe numeric echo escapes beside raw_json.
/// emitted_json_sha256 hashes the emitted PreservedCompact UTF8 contents, NOT
/// original source bytes or any canonical source-record digest.
pub fn refuse_integer_wire_value(
    operation: &str,
    reason: &str,
    value: &JsonValue,
    b: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    refuse_profile_value(operation, reason, value, OutputProfile::Safe, b)
}
fn refuse_profile_value(
    operation: &str,
    reason: &str,
    value: &JsonValue,
    profile: OutputProfile,
    b: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let raw = preserved(value, b)?;
    if reason.len() > b.max_expression_bytes {
        return Err(TemporalError::ResourceExhausted(
            "integer wire reason limit",
        ));
    }
    let mut result = header(operation, profile, b)?;
    put(&mut result, "outcome", text("Unsupported"));
    put(&mut result, "reason", text(reason));
    put(&mut result, "representation", text("OpaqueRawJson"));
    put(
        &mut result,
        "preservation_profile",
        text("tos_json_value_lexemes_v1"),
    );
    put(
        &mut result,
        "emitted_json_sha256",
        text(&Digest256::of_bytes(&raw).to_prefixed()),
    );
    put(&mut result, "raw_json", JsonValue::Null);
    let header_bytes = preserved(&result, b)?.len();
    // Compact JSON has no literal controls. Quoting its UTF8 text adds two
    // quotes and one escape byte for each quote/backslash, without f64/parse.
    let quoted = raw
        .len()
        .checked_add(2)
        .and_then(|n| n.checked_add(raw.iter().filter(|c| **c == b'"' || **c == b'\\').count()))
        .ok_or(TemporalError::ResourceExhausted(
            "integer wire raw escaping limit",
        ))?;
    admit_replacement(header_bytes, 4, quoted, b)?;
    let raw = String::from_utf8(raw)
        .map_err(|_| TemporalError::MalformedRequest("integer wire emitted UTF8 invalid"))?;
    put(&mut result, "raw_json", text(&raw));
    preserved(&result, b)?;
    Ok(result)
}
/// Encode a whole operation/page/refusal result. Encoded describes transport,
/// not truth/admission or successful query semantics (those stay in payload).
/// Native difference Elapsed magnitude keeps its canonical bounded String even
/// above binary64's safe Int range. Generic coordinates/versions stay unchanged.
pub fn encode_integer_wire_response(
    operation: &str,
    value: &JsonValue,
    b: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    encode_profile_response(operation, value, OutputProfile::Safe, b)
}
fn encode_profile_response(
    operation: &str,
    value: &JsonValue,
    profile: OutputProfile,
    b: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    let raw = preserved(value, b)?;
    let mut s = Scan::default();
    scan(value, 0, &mut s);
    if let Some(reason) = s.reason {
        drop(raw);
        return refuse_profile_value(operation, reason, value, profile, b);
    }
    if operation == "difference" && string(field(value, "duration"), "kind") == Some("Elapsed") {
        if let IntegerWireSafety::Unsupported(reason) =
            validate_elapsed_magnitude_string(field(field(value, "duration"), "magnitude"))
        {
            drop(raw);
            return refuse_profile_value(operation, reason, value, profile, b);
        }
    }
    let mut result = header(operation, profile, b)?;
    put(&mut result, "outcome", text("Encoded"));
    put(&mut result, "representation", text("StructuredJson"));
    put(&mut result, "payload", JsonValue::Null);
    let bytes = preserved(&result, b)?.len();
    admit_replacement(bytes, 4, raw.len(), b)?;
    let mut head = Scan::default();
    scan(&result, 0, &mut head);
    if head
        .nodes
        .checked_sub(1)
        .and_then(|n| n.checked_add(s.nodes))
        .is_none_or(|n| n > b.max_members)
        || s.depth.checked_add(1).is_none_or(|n| n > 64)
    {
        return Err(TemporalError::ResourceExhausted(
            "integer wire envelope structure limit",
        ));
    }
    put(&mut result, "payload", value.clone());
    preserved(&result, b)?;
    Ok(result)
}

/// Closed difference-only profile; callers cannot supply arbitrary header claims.
pub(crate) fn encode_decimal_difference_response(
    value: &JsonValue,
    b: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    encode_profile_response("difference", value, OutputProfile::DecimalDifference, b)
}
pub(crate) fn refuse_decimal_difference_value(
    reason: &str,
    value: &JsonValue,
    b: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    refuse_profile_value(
        "difference",
        reason,
        value,
        OutputProfile::DecimalDifference,
        b,
    )
}
