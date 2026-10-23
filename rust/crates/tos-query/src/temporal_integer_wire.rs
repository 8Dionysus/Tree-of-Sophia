//! Explicit client integer boundary over existing selected read operations.
//! Native selection, authorization and source digests remain unchanged. The
//! retained disclosure lease covers the transformed response through delivery.
use crate::{
    knowledge_binding::BoundCmpKnowledge,
    knowledge_inspect::{
        DisclosableInspect, InspectBudget, InspectCurrentAuthority, execute_selected_carrier_packet,
    },
    knowledge_temporal::{execute_selected_claim_list, execute_selected_claim_projection},
    search_v2::{SearchV2Error, SearchV2ErrorCode},
    source_read_projection::{object, text},
    temporal_comparison::{
        CLAIM_LIST_INTENDED_USE, CLAIM_LIST_OPERATION, CLAIM_PROJECTION_INTENDED_USE,
        CLAIM_PROJECTION_OPERATION,
    },
};
use tos_compiler::VerifiedKnowledgeModel;
use tos_foundation::temporal::{
    TemporalBudget, TemporalError,
    integer_wire::{
        IntegerWireSafety, check_integer_wire_value, encode_integer_wire_response,
        parse_integer_wire_request, refuse_integer_wire_value,
    },
};
use tos_foundation::{
    JsonMode, JsonValue, emit_value_preserved_json, parse_json_with_state_budget,
};

fn wire_budget_error() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "integer transport retained workspace exceeds limit",
    }
}
// This is a separate logical workspace ceiling using the caller's decoded
// ceiling; the native Reader still charges transferred rows independently.
// It is neither allocator/RSS accounting nor a claim that decoded transfers
// and retained transport state are the same quantity.
fn admit_wire_state(
    value: &JsonValue,
    budget: InspectBudget,
    extra: usize,
    operation_bytes: usize,
) -> Result<(), SearchV2Error> {
    fn string_bytes(value: &tos_foundation::JsonString) -> Result<usize, SearchV2Error> {
        if let Some(text) = value.as_str() {
            text.bytes().try_fold(2usize, |count, byte| {
                let emitted = match byte {
                    b'"' | b'\\' | 8 | 9 | 10 | 12 | 13 => 2,
                    0..=31 => 6,
                    _ => 1,
                };
                count.checked_add(emitted).ok_or_else(wire_budget_error)
            })
        } else {
            // Preserved lone UTF16 surrogates use six-byte JSON escapes.
            value
                .units()
                .len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(2))
                .ok_or_else(wire_budget_error)
        }
    }
    fn bounded(
        value: &JsonValue,
        depth: usize,
        visits: &mut usize,
        budget: InspectBudget,
    ) -> Result<usize, SearchV2Error> {
        *visits = visits.checked_add(1).ok_or_else(wire_budget_error)?;
        if depth > budget.json.max_depth.min(64) || *visits > budget.json.max_visits {
            return Err(wire_budget_error());
        }
        let add = |a: usize, b: usize| a.checked_add(b).ok_or_else(wire_budget_error);
        match value {
            JsonValue::Null => Ok(4),
            JsonValue::Bool(true) => Ok(4),
            JsonValue::Bool(false) => Ok(5),
            JsonValue::Number(number) => Ok(number.lexeme.len()),
            JsonValue::String(text) => string_bytes(text),
            JsonValue::Array(values) => {
                let mut bytes = add(2, values.len().saturating_sub(1))?;
                for child in values {
                    bytes = add(bytes, bounded(child, depth + 1, visits, budget)?)?;
                }
                Ok(bytes)
            }
            JsonValue::Object(values) => {
                let mut bytes = add(2, values.len().saturating_sub(1))?;
                for (key, child) in values {
                    bytes = add(bytes, add(string_bytes(key)?, 1)?)?;
                    bytes = add(bytes, bounded(child, depth + 1, visits, budget)?)?;
                }
                Ok(bytes)
            }
        }
    }
    let emitted = bounded(value, 0, &mut 0, budget)?;
    let owned = value
        .retained_storage_bytes()
        .map_err(|_| wire_budget_error())?
        .checked_add(std::mem::size_of::<JsonValue>())
        .ok_or_else(wire_budget_error)?;
    // Separate tree storage from emitted bytes rather than multiplying every
    // container slot by a string-escape bound. Four trees cover source/encoded
    // and cursor clones. Sixteen emitted buffers conservatively cover raw Vec,
    // UTF8+UTF16 fallback strings (including Vec growth), quoting, and emitters.
    // Admission precedes all these allocations; allocator/RSS is separate.
    let peak = owned
        .checked_mul(4)
        .and_then(|n| emitted.checked_mul(16).and_then(|raw| n.checked_add(raw)))
        .and_then(|n| {
            operation_bytes
                .checked_mul(8)
                .and_then(|header| n.checked_add(header))
        })
        .and_then(|n| n.checked_add(8192))
        .and_then(|n| n.checked_add(extra))
        .ok_or_else(wire_budget_error)?;
    if u64::try_from(peak).map_or(true, |n| n > budget.max_decoded_bytes) {
        return Err(wire_budget_error());
    }
    Ok(())
}

fn mapped(error: TemporalError) -> SearchV2Error {
    SearchV2Error {
        code: match error {
            TemporalError::ResourceExhausted(_) => SearchV2ErrorCode::BudgetExceeded,
            TemporalError::MalformedRequest(_) => SearchV2ErrorCode::InvalidRequest,
        },
        message: "exact integer transport refused request or exceeded bounds",
    }
}
fn invalid() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::InvalidRequest,
        message: "integer transport operation or cursor profile mismatch",
    }
}
fn set(value: &mut JsonValue, key: &str, replacement: JsonValue) {
    if let JsonValue::Object(fields) = value {
        if let Some((_, value)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
            *value = replacement;
        }
    }
}
fn cursor(native: JsonValue, operation: &str) -> JsonValue {
    object(vec![
        ("schema_version", text("tos_integer_wire_cursor_v1")),
        ("integer_encoding", text("SafeJsonNumberV1")),
        ("operation", text(operation)),
        ("native_cursor", native),
    ])
}

/// `payload` is the unchanged native Claim list or projection request, except
/// list `after` must use the explicit transport cursor returned by this seam.
/// A cursor remains an untrusted suffix position, never a completeness proof.
pub fn execute_selected_claim_integer_wire<'hold, A: InspectCurrentAuthority<'hold> + ?Sized>(
    model: &mut VerifiedKnowledgeModel<'_>,
    bound: &BoundCmpKnowledge<'_>,
    authority: &mut A,
    request: &JsonValue,
    budget: InspectBudget,
) -> Result<DisclosableInspect<'hold>, SearchV2Error> {
    // Admission precedes even the framing codec's temporary preserved emitter.
    admit_wire_state(request, budget, 0, 0)?;
    let temporal_budget = TemporalBudget {
        max_expression_bytes: budget
            .json
            .max_bytes
            .min(budget.max_payload_bytes)
            .min(budget.max_response_bytes),
        max_members: budget.json.max_visits,
        max_pairings: TemporalBudget::default().max_pairings,
    };
    let wire = parse_integer_wire_request(request, temporal_budget).map_err(mapped)?;
    let intended = match wire.operation {
        CLAIM_LIST_OPERATION => CLAIM_LIST_INTENDED_USE,
        CLAIM_PROJECTION_OPERATION => CLAIM_PROJECTION_INTENDED_USE,
        _ => return Err(invalid()),
    };
    let refusal = if wire.integer_encoding != "SafeJsonNumberV1" {
        Some("IntegerEncodingNotImplemented")
    } else {
        match check_integer_wire_value(request, temporal_budget).map_err(mapped)? {
            IntegerWireSafety::Safe => None,
            IntegerWireSafety::Unsupported(reason) => Some(reason),
        }
    };
    if let Some(reason) = refusal {
        // Even a no-read rejection follows the selected operation's existing
        // current authorization and bounded response custody route.
        return execute_selected_carrier_packet(
            model,
            bound,
            authority,
            wire.operation,
            intended,
            budget,
            |_| {
                refuse_integer_wire_value(wire.operation, reason, request, temporal_budget)
                    .map_err(mapped)
            },
        );
    }
    let mut native = wire.payload.clone(); // whole request admitted above
    if wire.operation == CLAIM_LIST_OPERATION {
        let after = native.object_get("after").ok_or_else(invalid)?;
        if *after != JsonValue::Null {
            if !after.as_object().is_some_and(|fields| fields.len() == 4)
                || after
                    .object_get("schema_version")
                    .and_then(JsonValue::as_str)
                    != Some("tos_integer_wire_cursor_v1")
                || after
                    .object_get("integer_encoding")
                    .and_then(JsonValue::as_str)
                    != Some("SafeJsonNumberV1")
                || after.object_get("operation").and_then(JsonValue::as_str) != Some(wire.operation)
            {
                return Err(invalid());
            }
            let original = after
                .object_get("native_cursor")
                .ok_or_else(invalid)?
                .clone();
            set(&mut native, "after", original);
        }
    }
    let packet = match wire.operation {
        CLAIM_LIST_OPERATION => {
            execute_selected_claim_list(model, bound, authority, &native, budget)?
        }
        CLAIM_PROJECTION_OPERATION => {
            execute_selected_claim_projection(model, bound, authority, &native, budget)?
        }
        _ => unreachable!(),
    };
    drop(native);
    let (bytes, mut lease) = packet.into_parts();
    let mut limits = budget.json;
    limits.max_bytes = limits.max_bytes.min(budget.max_response_bytes);
    let workspace = usize::try_from(budget.max_decoded_bytes)
        .unwrap_or(usize::MAX)
        .checked_sub(bytes.capacity())
        .ok_or_else(wire_budget_error)?;
    let mut value =
        parse_json_with_state_budget(&bytes, JsonMode::PublishedStrict, limits, workspace)
            .map_err(|error| SearchV2Error {
                code: if error.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                    SearchV2ErrorCode::BudgetExceeded
                } else {
                    SearchV2ErrorCode::CorruptSelectedCarrier
                },
                message: "selected response parser refused malformed data or workspace limit",
            })?
            .into_root();
    admit_wire_state(&value, budget, bytes.capacity(), wire.operation.len())?;
    if wire.operation == CLAIM_LIST_OPERATION {
        if let Some(next) = value.object_get("next").filter(|v| **v != JsonValue::Null) {
            let wrapped = cursor(next.clone(), wire.operation);
            set(&mut value, "next", wrapped);
        }
    }
    // The codec's opaque fallback hashes these emitted selected-response
    // bytes; it never relabels that digest as original source-file fixity.
    let encoded =
        encode_integer_wire_response(wire.operation, &value, temporal_budget).map_err(mapped)?;
    drop(value);
    drop(bytes);
    let body = emit_value_preserved_json(&encoded, limits).map_err(|_| SearchV2Error {
        code: SearchV2ErrorCode::BudgetExceeded,
        message: "integer transport response exceeds limits",
    })?;
    lease.recheck()?;
    Ok(DisclosableInspect::from_controlled(body, lease))
}
