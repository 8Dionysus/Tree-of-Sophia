//! Exact bounded document verification for indexed knowledge search.
//!
//! The selected payload is compact JSON, while the maintained search oracle
//! indexes Python's sorted, default-spaced JSON spelling after Unicode lower.

use tos_foundation::{
    CanonicalProfile, Digest256, FoundationErrorCode, JsonLimits, JsonMode, canonical_bytes_v1,
    parse_json, python_lower_unicode16_v1,
};

use crate::{QueryError, QueryErrorCode};

#[derive(Clone, Copy, Debug)]
pub struct SearchDocumentBudget {
    pub max_carrier_bytes: usize,
    pub max_document_bytes: usize,
    pub max_document_code_points: usize,
    pub json: JsonLimits,
}

/// A document whose selected length/digest were checked against the exact
/// Python v2 search spelling. This does not grant publication or source rights.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSearchDocument {
    pub lower: String,
    pub code_points: usize,
}

fn budget_error(message: &'static str) -> QueryError {
    QueryError::new(QueryErrorCode::BudgetExceeded, message)
}

fn carrier_error(message: &'static str) -> QueryError {
    QueryError::new(QueryErrorCode::CorruptSelectedCarrier, message)
}

fn map_foundation(error: tos_foundation::FoundationError) -> QueryError {
    if error.code == FoundationErrorCode::BudgetExceeded {
        budget_error("indexed search document work budget exceeded")
    } else {
        carrier_error("indexed search document is invalid")
    }
}

fn push(output: &mut Vec<u8>, byte: u8, limit: usize) -> Result<(), QueryError> {
    if output.len() >= limit {
        return Err(budget_error("indexed search document byte budget exceeded"));
    }
    output.push(byte);
    Ok(())
}

/// Insert Python's default `, ` and `: ` separators into a canonical compact
/// JSON document. Punctuation inside strings, including escaped quotes, is
/// unchanged. The foundation canonical writer already supplies Python's
/// sorted keys, non-ASCII scalar spelling and finite number rendering.
fn python_default_spaces(compact: &[u8], limit: usize) -> Result<String, QueryError> {
    let mut output = Vec::with_capacity(compact.len().min(limit));
    let mut in_string = false;
    let mut escaped = false;
    for &byte in compact {
        push(&mut output, byte, limit)?;
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b',' | b':' => push(&mut output, b' ', limit)?,
                _ => {}
            }
        }
    }
    String::from_utf8(output).map_err(|_| carrier_error("canonical search document is not UTF-8"))
}

pub fn verify_indexed_search_document(
    carrier: &[u8],
    expected_code_points: u64,
    expected_sha256: Digest256,
    budget: SearchDocumentBudget,
) -> Result<VerifiedSearchDocument, QueryError> {
    if budget.max_carrier_bytes == 0
        || budget.max_document_bytes == 0
        || budget.max_document_code_points == 0
    {
        return Err(budget_error("indexed search document budget absent"));
    }
    if carrier.len() > budget.max_carrier_bytes {
        return Err(budget_error("indexed search carrier byte budget exceeded"));
    }
    let mut parse_limits = budget.json;
    parse_limits.max_bytes = parse_limits.max_bytes.min(budget.max_carrier_bytes);
    let parsed =
        parse_json(carrier, JsonMode::PublishedStrict, parse_limits).map_err(map_foundation)?;
    if parsed.root().as_object().is_none() {
        return Err(carrier_error("indexed search carrier is not an object"));
    }
    let mut emit_limits = budget.json;
    emit_limits.max_bytes = budget.max_document_bytes;
    let compact = canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        emit_limits,
    )
    .map_err(map_foundation)?;
    let spaced = python_default_spaces(&compact, budget.max_document_bytes)?;
    let lower = python_lower_unicode16_v1(
        &spaced,
        budget.max_document_code_points,
        budget.max_document_code_points,
        budget.max_document_bytes,
    )
    .map_err(map_foundation)?;
    let code_points = lower.chars().count();
    if code_points as u64 != expected_code_points
        || Digest256::of_bytes(lower.as_bytes()) != expected_sha256
    {
        return Err(carrier_error(
            "indexed search document digest or length differs",
        ));
    }
    Ok(VerifiedSearchDocument { lower, code_points })
}
