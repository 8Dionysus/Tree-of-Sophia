//! Prospective logical data admission for the three existing PG freshness
//! queries, on the caller's actual OperationBudget. This is NOT a proof of
//! driver buffers/errors/network/kernel cost or complete PG/STO funding.
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_native_cli::private_assessment::OperationBudget;
use std::{mem::size_of, ops::Range, sync::atomic::AtomicBool, time::Instant};

fn exhausted() -> SourceCommandError {
    SourceCommandError::Unsupported("PG freshness query data reservation overflow")
}
fn add(a: usize, b: usize) -> SourceCommandResult<usize> {
    a.checked_add(b).ok_or_else(exhausted)
}
fn mul(a: usize, b: usize) -> SourceCommandResult<usize> {
    a.checked_mul(b).ok_or_else(exhausted)
}

// PostgreSQL char(64) counts characters, not encoded bytes. Even a malformed
// non-ASCII digest must be transferred before parse_hex can refuse it. A UTF-8
// scalar occupies at most four bytes; do not assume a private writer emitted
// lowercase ASCII merely from the column's char(64) declaration.
const CHAR64_UTF8_MAX_BYTES: usize = 64 * 4;

/// The real Row payload shapes from durable_schema.sql: bigint8, bool1,
/// char(64)<=256 UTF-8 bytes and normal/active maintenance_state (max6).
/// Null fields need no more payload. This is not a configurable source grant.
pub(super) enum RowShape {
    None,
    Audit,
    Domain,
    Job,
}
impl RowShape {
    fn fields(&self) -> &'static [usize] {
        match self {
            Self::None => &[],
            Self::Audit => &[8, 6],
            Self::Domain => &[
                CHAR64_UTF8_MAX_BYTES,
                1,
                1,
                8,
                8,
                8,
                8,
                8,
                CHAR64_UTF8_MAX_BYTES,
            ],
            Self::Job => &[8],
        }
    }
    fn decoded(&self) -> (usize, usize) {
        match self {
            // Actual get::<String> copies, plus two expected digest hex
            // Strings in the domain comparisons. No post-query measurement.
            Self::Audit => (6, 1),
            // source_definition_digest is copied twice, contract_digest once;
            // the two expected Digest256::to_hex values are always ASCII64.
            Self::Domain => (3 * CHAR64_UTF8_MAX_BYTES + 2 * 64, 5),
            Self::None | Self::Job => (0, 0),
        }
    }
}

/// Reserve actual inline transaction/three returned Row carriers before BEGIN,
/// and all three possible transaction-control SQL inputs before cleanup can
/// be needed. No claimed bound for foreign driver-owned error payloads.
pub(super) fn begin(
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    if deadline > budget.limits()?.deadline {
        return Err(SourceCommandError::Conflict(
            "PG query cutoff exceeds original operation",
        ));
    }
    let carriers = add(
        size_of::<postgres::Transaction<'_>>(),
        mul(3, size_of::<postgres::Row>())?,
    )?;
    let transaction_sql = add(
        add("START TRANSACTION".len(), "COMMIT".len())?,
        "ROLLBACK".len(),
    )?;
    budget.charge_work(transaction_sql)?;
    budget.charge_bytes(add(carriers, transaction_sql)?)
}

/// All debits happen BEFORE the actual statement call, even when that call
/// later errors or returns no row. Work/byte prefixes remain spent on refusal.
pub(super) fn statement(
    budget: &mut OperationBudget,
    sql: &'static str,
    parameters: &[usize],
    shape: RowShape,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let input = parameters.iter().try_fold(sql.len(), |n, p| add(n, *p))?;
    let fields = shape.fields();
    let payload = fields.iter().try_fold(0usize, |n, p| add(n, *p))?;
    // Protocol-v3 DataRow: message tag1 + length4 + field count2,
    // then one signed4-byte length and payload per field.
    let data_row = if fields.is_empty() {
        0
    } else {
        add(add(1 + 4 + 2, mul(fields.len(), 4)?)?, payload)?
    };
    // tokio-postgres Row keeps Vec<Option<Range<usize>>>. The pinned u8-independent
    // RawVec minimum4/doubling rule bounds old+new range backing by3*N+4 slots.
    let ranges = if fields.is_empty() {
        0
    } else {
        mul(
            add(mul(fields.len(), 3)?, 4)?,
            size_of::<Option<Range<usize>>>(),
        )?
    };
    let (decoded_bytes, decoded_strings) = shape.decoded();
    let holders = add(
        add(data_row, ranges)?,
        add(decoded_bytes, mul(decoded_strings, size_of::<String>())?)?,
    )?;
    budget.charge_work(add(
        add(input, data_row)?,
        add(decoded_bytes, fields.len())?,
    )?)?;
    budget.charge_bytes(add(input, holders)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_row_quote_covers_non_ascii_char64_before_hex_refusal() {
        let field = "\u{1f642}".repeat(64);
        assert_eq!(field.chars().count(), 64);
        assert_eq!(field.len(), 256);
        assert!(tos_foundation::Digest256::from_hex(&field).is_err());
        let fields = RowShape::Domain.fields();
        assert!(fields[0] >= field.len());
        assert!(fields[8] >= field.len());
        let (copies, strings) = RowShape::Domain.decoded();
        assert!(copies >= 3 * field.len() + 2 * 64);
        assert_eq!(strings, 5);
    }
}
