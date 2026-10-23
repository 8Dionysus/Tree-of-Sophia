//! CanonicalDecimalStringV1 is admitted ONLY for DifferenceCoordinateI64V1.
//! Two known coordinate leaves are projected to the existing native i64 ABI;
//! the original wire input replaces the internal numeric echo before emission.
//! This grants no general coordinate/version/query/CLI or source-write support.
use super::integer_wire::{
    IntegerWireSafety, check_integer_wire_value, encode_decimal_difference_response,
    parse_integer_wire_request, refuse_decimal_difference_value, validate_elapsed_magnitude_string,
};
use super::*;

fn shape(v: &JsonValue, allowed: &[&str], required: &[&str]) -> bool {
    let Some(members) = v.as_object() else {
        return false;
    };
    required.iter().all(|k| v.object_get(k).is_some())
        && members.iter().enumerate().all(|(i, (k, _))| {
            allowed.iter().any(|name| k.as_str() == Some(*name))
                && !members[..i].iter().any(|(other, _)| other == k)
        })
}
fn nonblank(v: &JsonValue, key: &str) -> bool {
    string(v, key).is_some_and(|s| !s.trim().is_empty())
}
fn coordinate(v: &JsonValue) -> Result<i64, &'static str> {
    const PROFILE: &str = "DecimalDifferenceProfileNotSupported";
    if !shape(
        v,
        &["type", "value", "axis", "resolution"],
        &["type", "value", "axis"],
    ) || string(v, "type") != Some("CoordinatePosition")
        || v.object_get("resolution")
            .is_some_and(|r| r.as_str() != Some("exact"))
    {
        return Err(PROFILE);
    }
    let axis = field(v, "axis");
    let trs = field(axis, "trs");
    if !shape(axis, &["trs", "unit", "origin"], &["trs", "unit", "origin"])
        || !shape(trs, &["id", "version"], &["id", "version"])
        || !nonblank(axis, "unit")
        || !nonblank(axis, "origin")
        || !nonblank(trs, "id")
        || !nonblank(trs, "version")
    {
        return Err(PROFILE);
    }
    let s = field(v, "value")
        .as_str()
        .ok_or("DecimalCoordinateStringRequired")?;
    let digits = s.strip_prefix('-').unwrap_or(s).as_bytes();
    if digits.len() > 39 {
        return Err("DecimalCoordinateDigitLimit");
    }
    if s != "0"
        && !(digits.first().is_some_and(|c| matches!(*c, b'1'..=b'9'))
            && digits.iter().all(u8::is_ascii_digit))
    {
        return Err("NonCanonicalCoordinateString");
    }
    s.parse::<i64>().map_err(|_| "CoordinateOutOfRange")
}
/// Pure scoped transport adapter. The outer owner routes only declared
/// CanonicalDecimalStringV1 + difference here; unsupported other operations or
/// encodings keep their own explicit refusal path. No implicit encoding default.
pub fn execute_decimal_difference_wire(
    request: &JsonValue,
    budget: TemporalBudget,
) -> Result<JsonValue, TemporalError> {
    // The borrowed parser admits full bytes/members/depth before any clone or
    // typed projection. The subsequent gate includes all original raw fields.
    let wire = parse_integer_wire_request(request, budget)?;
    if wire.integer_encoding != "CanonicalDecimalStringV1" || wire.operation != "difference" {
        return Err(TemporalError::MalformedRequest(
            "decimal difference scoped request required",
        ));
    }
    if let IntegerWireSafety::Unsupported(reason) = check_integer_wire_value(request, budget)? {
        return refuse_decimal_difference_value(reason, request, budget);
    }
    if !shape(wire.payload, &["input", "context"], &["input", "context"])
        || field(wire.payload, "input").as_object().is_none()
        || field(wire.payload, "context").as_object().is_none()
    {
        return Err(TemporalError::MalformedRequest(
            "decimal difference payload requires input/context objects",
        ));
    }
    let input = field(wire.payload, "input");
    let context = field(wire.payload, "context");
    if !context.as_object().expect("checked object").is_empty() {
        return refuse_decimal_difference_value(
            "DecimalDifferenceContextNotSupported",
            request,
            budget,
        );
    }
    if !shape(
        input,
        &["beginning", "end", "duration_kind", "negative_policy"],
        &["beginning", "end", "duration_kind"],
    ) || string(input, "duration_kind") != Some("Elapsed")
        || input
            .object_get("negative_policy")
            .is_some_and(|p| !matches!(p.as_str(), Some("Signed" | "Reject")))
    {
        return refuse_decimal_difference_value(
            "DecimalDifferenceProfileNotSupported",
            request,
            budget,
        );
    }
    // Only the two explicit value paths are decoded. IDs, rule versions,
    // wording and arbitrary strings are never searched/converted as numbers.
    let beginning = match coordinate(field(input, "beginning")) {
        Ok(v) => v,
        Err(reason) => return refuse_decimal_difference_value(reason, request, budget),
    };
    let end = match coordinate(field(input, "end")) {
        Ok(v) => v,
        Err(reason) => return refuse_decimal_difference_value(reason, request, budget),
    };
    let mut native = input.clone();
    for (key, value) in [("beginning", beginning), ("end", end)] {
        let JsonValue::Object(members) = &mut native else {
            unreachable!("validated input");
        };
        let position = &mut members
            .iter_mut()
            .find(|(name, _)| name.as_str() == Some(key))
            .expect("required coordinate")
            .1;
        put(position, "value", number(value));
    }
    // Existing operation owns exact i64->i128 subtraction and policy/statuses.
    // No source digest is computed over this temporary execution projection.
    let mut result = super::execute_temporal_operation("difference", &native, context, budget)?;
    drop(native);
    // Do this for EVERY native domain outcome, before ANY output gate/refusal.
    // The temporary numeric echo is not the user's source/wire representation.
    put(&mut result, "preserved_input", input.clone());
    if string(field(&result, "duration"), "kind") == Some("Elapsed") {
        if let IntegerWireSafety::Unsupported(reason) =
            validate_elapsed_magnitude_string(field(field(&result, "duration"), "magnitude"))
        {
            return refuse_decimal_difference_value(reason, &result, budget);
        }
    }
    encode_decimal_difference_response(&result, budget)
}
