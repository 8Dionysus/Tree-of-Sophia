//! Closed schema operations required by the PG formal-role reader.
//!
//! This interface grants no role/current-use authority. Common will consume
//! it on the concrete, private-constructed PgPayloadHost; an arbitrary trait
//! implementation is not a native role issuer. PWE retains its executor and
//! cumulative state exclusively. No mutable worker, quota setter, replacement
//! Session, selectable cutoff or caller-supplied success witness is exposed.
use crate::DurableResult;
use tos_foundation::{CutPreparedSchemaExecutionBinding, Digest256};
use tos_validation::FormatProfile;

pub(crate) trait PgRoleSchemaAccess {
    /// Check a native reader's canonical current/retained instance against an
    /// ACTUAL prepared selected contract. False is a real schema negative;
    /// resource, source-epoch and executor failures remain their original Err.
    /// Schema success alone establishes neither origin nor role eligibility.
    ///
    /// Exact callsite: source_revisions::schema_at's check/check_reusing_scalar
    /// calls after native owner/history selection. The owner must perform
    /// original prospective input/copy/error payment and paid entry/exit checks
    /// on every Result, with the same private executor and Session. It may
    /// execute fresh rather than promise a scalar-reuse receipt.
    fn check_record_bytes(
        &mut self,
        source_path: &str,
        raw: &[u8],
        schema_path: &str,
    ) -> DurableResult<bool>;

    /// Point lookup on the ACTUAL executor's prepared resource table, not a
    /// digest inferred from a merely selected payload. Native metadata checks
    /// every declared dependency against this table. Missing means None, not
    /// permission to fetch/prepare another schema or silently use a default.
    fn selected_schema_contract_digest(
        &mut self,
        schema_path: &str,
    ) -> DurableResult<Option<Digest256>>;

    /// Paid read-only binding snapshot from that same executor. Common's
    /// RecordSchemaWorker adapter requires profile/binding correspondence;
    /// later closed checks still fence actual state and selected PG identity.
    /// This snapshot is not instance continuity or a caller factory token.
    fn schema_validation_binding(
        &mut self,
    ) -> DurableResult<(FormatProfile, CutPreparedSchemaExecutionBinding)>;
}
