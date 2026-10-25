//! Query DATA only for one exact private Claim archive on the owner's actual
//! RR read session. No source permission, ledger, transaction or issuer is made
//! here. Foreign driver capacity/errors/network and semantic/native holder fit
//! remain separate obligations; DataRow length is not allocator capacity/RSS.
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_native_cli::private_assessment::OperationBudget;
use std::{mem::size_of, ops::Range};

pub(super) const HEADER_SQL: &str =
    "SELECT family,source_revision,package_digest,member_count,total_bytes
         FROM cmd2_private_package_archive
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND archive_ref=$4";
pub(super) const MEMBERS_SQL: &str = "SELECT member_name,member_bytes,member_digest
         FROM cmd2_private_package_archive_member
         WHERE domain=$1 AND owner_scope=$2 AND target_ref=$3 AND archive_ref=$4
         ORDER BY member_name COLLATE \"C\" LIMIT 66";

pub(super) enum ArchiveQuery {
    Header,
    Members,
}

// Same transaction/scope/key as the payload read. Complete aggregate metadata
// precedes payload transfer; declared total alone cannot bound corrupt rows.
pub(super) const ENVELOPE_SQL: &str = "SELECT a.family='claim',a.member_count,a.total_bytes,
         octet_length(convert_to(a.family,'UTF8')),
         octet_length(convert_to(a.source_revision,'UTF8')),
         octet_length(convert_to(a.package_digest,'UTF8')),
         s.members,s.raw_bytes,s.name_bytes,s.digest_bytes,s.max_name_bytes,s.max_digest_bytes
         FROM cmd2_private_package_archive a
         CROSS JOIN LATERAL (
           SELECT count(*) AS members,
             COALESCE(sum(octet_length(member_bytes)),0)::bigint AS raw_bytes,
             COALESCE(sum(octet_length(convert_to(member_name,'UTF8'))),0)::bigint AS name_bytes,
             COALESCE(sum(octet_length(convert_to(member_digest,'UTF8'))),0)::bigint AS digest_bytes,
             COALESCE(max(octet_length(convert_to(member_name,'UTF8'))),0) AS max_name_bytes,
             COALESCE(max(octet_length(convert_to(member_digest,'UTF8'))),0) AS max_digest_bytes
           FROM cmd2_private_package_archive_member
           WHERE domain=a.domain AND owner_scope=a.owner_scope
             AND target_ref=a.target_ref AND archive_ref=a.archive_ref
         ) s
         WHERE a.domain=$1 AND a.owner_scope=$2 AND a.target_ref=$3 AND a.archive_ref=$4";

fn overflow() -> SourceCommandError {
    SourceCommandError::Unsupported("private Claim archive query data overflow")
}
pub(super) fn add(a: usize, b: usize) -> SourceCommandResult<usize> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> SourceCommandResult<usize> {
    a.checked_mul(b).ok_or_else(overflow)
}

/// One actual statement's input, protocol-v3 DataRow payload/range elements
/// and decoded field contents. Counts are complete for this exact archive,
/// not top-N. Query errors/no-row exits retain the prospective debit.
pub(super) fn reserve_query(
    budget: &mut OperationBudget,
    sql: &'static str,
    parameter_bytes: usize,
    fields: usize,
    rows: usize,
    payload: usize,
    decoded_bytes: usize,
    decoded_strings: usize,
    decoded_vectors: usize,
) -> SourceCommandResult<()> {
    let input = add(sql.len(), parameter_bytes)?;
    let data = add(mul(rows, add(7, mul(fields, 4)?)?)?, payload)?;
    // Logical element storage, not a bound for private Row/Bytes allocation
    // capacity or the driver's collection/growth/foreign-control envelopes.
    let ranges = mul(mul(rows, fields)?, size_of::<Option<Range<usize>>>())?;
    let inline = add(
        mul(rows, size_of::<postgres::Row>())?,
        add(
            mul(decoded_strings, size_of::<String>())?,
            mul(decoded_vectors, size_of::<Vec<u8>>())?,
        )?,
    )?;
    budget.charge_work(add(
        add(input, data)?,
        add(decoded_bytes, mul(rows, fields)?)?,
    )?)?;
    budget.charge_bytes(add(
        add(input, data)?,
        add(ranges, add(decoded_bytes, inline)?)?,
    )?)
}
