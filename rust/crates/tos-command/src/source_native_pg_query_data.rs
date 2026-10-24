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

/// Payloads of the selected binary queries, not a private-writer promise.
/// Domain digest lengths come from the actual bpcharsend envelope under the
/// same row lock before the full row is fetched. char(64) is NOT byte64.
/// Null fields need no more payload. This is not a configurable source grant.
pub(super) enum RowShape {
    None,
    Audit,
    DomainLengths,
    Domain([usize; 9]),
    Job,
}
impl RowShape {
    pub(super) fn domain(definition_bytes: usize, contract_bytes: usize) -> Self {
        Self::Domain([definition_bytes, 1, 1, 8, 8, 8, 8, 8, contract_bytes])
    }
    fn fields(&self) -> &[usize] {
        match self {
            Self::None => &[],
            Self::Audit => &[8, 6],
            Self::DomainLengths => &[4, 4],
            Self::Domain(fields) => fields,
            Self::Job => &[8],
        }
    }
    fn decoded(&self) -> SourceCommandResult<(usize, usize)> {
        Ok(match self {
            Self::Audit => (6, 1),
            // The real caller copies definition twice and contract once.
            // Only the two expected Digest256::to_hex values are ASCII64.
            Self::Domain(fields) => (add(add(mul(2, fields[0])?, fields[8])?, 2 * 64)?, 5),
            Self::None | Self::DomainLengths | Self::Job => (0, 0),
        })
    }
}

/// Reserve the actual transaction/four Row carriers and five statement-shape
/// holders before BEGIN, plus the two decoded length scalars. Also reserve
/// all three possible transaction-control SQL inputs before cleanup can
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
        add(
            mul(4, size_of::<postgres::Row>())?,
            add(
                mul(5, size_of::<RowShape>())?,
                mul(2, size_of::<i32>() + size_of::<usize>())?,
            )?,
        )?,
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
    let (decoded_bytes, decoded_strings) = shape.decoded()?;
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
    fn digest_row_quote_uses_sender_bytes_before_hex_refusal() {
        let field = "\u{1f642}".repeat(64);
        assert_eq!(field.chars().count(), 64);
        assert_eq!(field.len(), 256);
        assert!(tos_foundation::Digest256::from_hex(&field).is_err());
        let shape = RowShape::domain(field.len(), field.len());
        let fields = shape.fields();
        assert_eq!(fields[0], field.len());
        assert_eq!(fields[8], field.len());
        let (copies, strings) = shape.decoded().unwrap();
        assert!(copies >= 3 * field.len() + 2 * 64);
        assert_eq!(strings, 5);
        // The actual sender envelope, not a character-count heuristic,
        // supplies these lengths. Never clamp a larger observation to 256.
        let converted = RowShape::domain(384, 384);
        assert_eq!(converted.fields()[0], 384);
        assert_eq!(converted.fields()[8], 384);
        assert_eq!(converted.decoded().unwrap(), (3 * 384 + 2 * 64, 5));
        assert!(RowShape::domain(usize::MAX, 1).decoded().is_err());
    }
}
