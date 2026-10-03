use super::*;

use rusqlite::{TransactionBehavior, params, types::ValueRef};
use std::sync::Arc;
use tos_source_store::{PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteConnection};

#[path = "source_cut_receipt_codec.rs"]
pub(super) mod receipt_codec;
use receipt_codec::{
    decode_diagnostic, decode_receipt, diagnostic_row_encoded_upper_bound, encode_diagnostic,
    encode_receipt, encoded_scalar_receipt_len,
};

const RECEIPT_SPOOL_PAGE_SIZE: u64 = 4096;
const RECEIPT_SPOOL_SCHEMA_VERSION: u8 = 1;
const RECEIPT_SPOOL_LEGACY: u8 = 1;
const RECEIPT_SPOOL_DIAGNOSTICS_V2: u8 = 2;
const RECEIPT_SPOOL_DIGEST_DOMAIN: &[u8] = b"tos-cut-schema-observation-spool-v1\0";

/// Caller-owned logical ceilings for the opt-in source-cut receipt spool.
/// SQLite inode, logical-byte, cache, auxiliary-file, I/O and deadline limits
/// remain in the supplied `PinnedSqliteAuxScope` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutSchemaReceiptSpoolLimits {
    pub max_observations: usize,
    pub max_total_encoded_bytes: usize,
    pub max_page_rows: usize,
    pub max_page_bytes: usize,
    pub max_database_pages: u64,
    pub max_cache_pages: u64,
}

#[derive(Debug, Clone)]
pub struct CutSchemaReceiptPage {
    rows: Vec<(u64, CutSchemaReceipt)>,
    next_after_ordinal: Option<u64>,
    encoded_bytes: usize,
}

impl CutSchemaReceiptPage {
    pub(super) fn from_rows(
        rows: Vec<(u64, CutSchemaReceipt)>,
        next_after_ordinal: Option<u64>,
        encoded_bytes: usize,
    ) -> Self {
        Self {
            rows,
            next_after_ordinal,
            encoded_bytes,
        }
    }

    pub fn rows(&self) -> &[(u64, CutSchemaReceipt)] {
        &self.rows
    }
    pub fn into_rows(self) -> Vec<(u64, CutSchemaReceipt)> {
        self.rows
    }
    pub fn next_after_ordinal(&self) -> Option<u64> {
        self.next_after_ordinal
    }
    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
}

#[derive(Debug, Clone)]
pub struct CutSchemaDiagnosticPage {
    rows: Vec<(u64, CutSchemaDiagnostic)>,
    next_after_ordinal: Option<u64>,
    encoded_bytes: usize,
}

impl CutSchemaDiagnosticPage {
    pub fn rows(&self) -> &[(u64, CutSchemaDiagnostic)] {
        &self.rows
    }
    pub fn into_rows(self) -> Vec<(u64, CutSchemaDiagnostic)> {
        self.rows
    }
    pub fn next_after_ordinal(&self) -> Option<u64> {
        self.next_after_ordinal
    }
    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
}

/// VAL-local execution summary. Counts distinguish legacy execution receipts
/// from authenticated diagnostics-v2 terminals; none of these fields claim
/// selected source membership or a complete source traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutSchemaReceiptSpoolSummary {
    pub execution_binding: CutExecutionBinding,
    pub legacy_receipt_count: u64,
    pub legacy_receipt_encoded_bytes: u64,
    pub diagnostics_v2_terminal_count: u64,
    pub diagnostics_v2_status_counts: [u64; 5],
    pub diagnostics_v2_cost: Option<CutSchemaDiagnosticsCumulativeCost>,
    pub ordered_observation_sha256: Digest256,
    pub worker_finished: bool,
}

/// Opt-in paged executor. Its SQLite scope and connection are retained until
/// worker finalization; it deliberately has no infallible receipt-slice API.
pub struct CutWorkerSchemaExecutorSpooling {
    inner: CutWorkerSchemaExecutor,
    scope: Option<PinnedSqliteAuxScope>,
    connection: Option<PinnedSqliteConnection>,
    limits: CutSchemaReceiptSpoolLimits,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    next_ordinal: u64,
    total_encoded_bytes: u64,
    legacy_receipt_count: u64,
    legacy_receipt_encoded_bytes: u64,
    diagnostics_v2_terminal_count: u64,
    diagnostics_v2_status_counts: [u64; 5],
    status_refused: bool,
    ordered_hasher: Digest256Hasher,
    finished_summary: Option<CutSchemaReceiptSpoolSummary>,
    reserved_observations: usize,
    reserved_encoded_bytes: usize,
    failed: bool,
    worker_finished: bool,
    custody_closed: bool,
}

impl CutWorkerSchemaExecutorSpooling {
    pub(super) fn new(
        inner: CutWorkerSchemaExecutor,
        workspace_dir: std::fs::File,
        request: PinnedSqliteAuxRequest,
        limits: CutSchemaReceiptSpoolLimits,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, ItemRefusal> {
        if limits.max_observations == 0
            || limits.max_observations == usize::MAX
            || limits.max_total_encoded_bytes == 0
            || limits.max_total_encoded_bytes == usize::MAX
            || limits.max_page_rows == 0
            || limits.max_page_rows > limits.max_observations
            || limits.max_page_bytes == 0
            || limits.max_page_bytes > limits.max_total_encoded_bytes
            || limits.max_database_pages == 0
            || limits.max_database_pages == u64::MAX
            || limits.max_cache_pages == 0
            || limits.max_cache_pages > limits.max_database_pages
            || inner.revision.is_none()
            || inner.scalar_check_count != 0
            || !inner.receipts.is_empty()
            || inner.diagnostic_executions != 0
            || !inner.pending_diagnostics.is_empty()
            || inner.finished
            || inner.protocol_started
            || limits.max_observations > inner.limits.max_receipts
            || limits.max_observations as u64 > inner.prepared.operation_budget().max_total_units
            || limits.max_observations as u64 > inner.prepared.operation_budget().max_chunks
            || deadline <= Instant::now()
            || cancelled.load(Ordering::Acquire)
            || request.deadline != deadline
            || !Arc::ptr_eq(&request.cancelled, &cancelled)
            || limits
                .max_database_pages
                .checked_mul(RECEIPT_SPOOL_PAGE_SIZE)
                .is_none_or(|bytes| bytes > request.limits.main_logical_bytes)
            || limits
                .max_cache_pages
                .checked_mul(RECEIPT_SPOOL_PAGE_SIZE)
                .is_none_or(|bytes| bytes > request.limits.main_logical_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        let mut scope = PinnedSqliteAuxScope::new(workspace_dir, request).map_err(|_| {
            ItemRefusal::Source("cut schema receipt spool scope unavailable".into())
        })?;
        let mut connection = scope
            .open_connection()
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool unavailable".into()))?;
        initialize_spool_database(&mut connection, limits)?;
        check(deadline, &cancelled)?;
        let mut ordered_hasher = Digest256Hasher::new();
        ordered_hasher.update(RECEIPT_SPOOL_DIGEST_DOMAIN);
        Ok(Self {
            inner,
            scope: Some(scope),
            connection: Some(connection),
            limits,
            deadline,
            cancelled,
            next_ordinal: 0,
            total_encoded_bytes: 0,
            legacy_receipt_count: 0,
            legacy_receipt_encoded_bytes: 0,
            diagnostics_v2_terminal_count: 0,
            diagnostics_v2_status_counts: [0; 5],
            status_refused: false,
            ordered_hasher,
            finished_summary: None,
            reserved_observations: 0,
            reserved_encoded_bytes: 0,
            failed: false,
            worker_finished: false,
            custody_closed: false,
        })
    }

    pub fn execution_binding(&self) -> CutExecutionBinding {
        self.inner.execution_binding()
    }

    pub fn legacy_receipt_count(&self) -> u64 {
        self.legacy_receipt_count
    }

    pub fn diagnostics_v2_terminal_count(&self) -> u64 {
        self.diagnostics_v2_terminal_count
    }

    /// Take the one complete authenticated non-verdict terminal that caused a
    /// spool check refusal. The refusal blocks further checks; callers may
    /// still explicitly finish, verify EOF, and read bounded persisted pages.
    pub fn take_diagnostics_v2_status_refusal(&mut self) -> Option<CutSchemaDiagnostic> {
        self.inner.take_spooled_diagnostics_v2_status_refusal()
    }

    pub fn set_operation_budget(&mut self, budget: BatchStreamBudget) -> Result<(), ItemRefusal> {
        self.guard_configurable()?;
        self.inner.set_operation_budget(budget)
    }

    pub fn set_shared_schema_worker_quota(
        &mut self,
        quota: SharedSchemaWorkerQuota,
    ) -> Result<(), ItemRefusal> {
        self.guard_configurable()?;
        self.inner.set_shared_schema_worker_quota(quota)
    }

    pub fn enable_diagnostics_v2(
        &mut self,
        limits: CutSchemaDiagnosticsLimits,
    ) -> Result<(), ItemRefusal> {
        self.guard_configurable()?;
        self.inner.enable_diagnostics_v2(limits)
    }

    pub fn set_diagnostics_v2_legacy_raw_instance_limit(
        &mut self,
        max_instance_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        self.guard_configurable()?;
        self.inner
            .set_diagnostics_v2_legacy_raw_instance_limit(max_instance_bytes)
    }

    pub fn set_diagnostics_v2_legacy_selected_limits(
        &mut self,
        limits: LegacySelectedDiagnosticsLimits,
    ) -> Result<(), ItemRefusal> {
        self.guard_configurable()?;
        self.inner.set_diagnostics_v2_legacy_selected_limits(limits)
    }

    pub fn set_diagnostics_v2_controller_state_cap(
        &mut self,
        cap_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        self.guard_configurable()?;
        self.inner
            .set_diagnostics_v2_controller_state_cap(cap_bytes)
    }

    pub fn diagnostics_v2_cumulative_cost(
        &self,
    ) -> Result<CutSchemaDiagnosticsCumulativeCost, ItemRefusal> {
        self.inner.diagnostics_v2_cumulative_cost()
    }

    pub fn take_schema_diagnostic_rejection(&mut self) -> Option<CutSchemaDiagnostic> {
        self.inner.take_schema_diagnostic_rejection()
    }

    pub fn read_receipts_after(
        &mut self,
        after_ordinal: Option<u64>,
        max_rows: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaReceiptPage, ItemRefusal> {
        if self.inner.diagnostics_v2.is_some() {
            return Err(ItemRefusal::Unsupported(
                "legacy receipt pages are unavailable for diagnostics-v2 observations".into(),
            ));
        }
        self.read_page::<CutSchemaReceipt>(
            after_ordinal,
            max_rows,
            max_bytes,
            deadline,
            cancelled,
            RECEIPT_SPOOL_LEGACY,
            decode_receipt,
        )
        .map(
            |(rows, next_after_ordinal, encoded_bytes)| CutSchemaReceiptPage {
                rows,
                next_after_ordinal,
                encoded_bytes,
            },
        )
    }

    pub fn read_diagnostics_after(
        &mut self,
        after_ordinal: Option<u64>,
        max_rows: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaDiagnosticPage, ItemRefusal> {
        if self.inner.diagnostics_v2.is_none() {
            return Err(ItemRefusal::Unsupported(
                "diagnostics-v2 pages are unavailable for legacy receipts".into(),
            ));
        }
        self.read_page::<CutSchemaDiagnostic>(
            after_ordinal,
            max_rows,
            max_bytes,
            deadline,
            cancelled,
            RECEIPT_SPOOL_DIAGNOSTICS_V2,
            decode_diagnostic,
        )
        .map(
            |(rows, next_after_ordinal, encoded_bytes)| CutSchemaDiagnosticPage {
                rows,
                next_after_ordinal,
                encoded_bytes,
            },
        )
    }

    pub fn finish_spooled(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaReceiptSpoolSummary, ItemRefusal> {
        if self.worker_finished {
            return self.summary_spooled(deadline, cancelled);
        }
        self.check_live(deadline, cancelled)?;
        if self.failed || self.reserved_observations != 0 {
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt spool history is incomplete".into(),
            ));
        }
        if let Err(error) = self.inner.finish(self.deadline.min(deadline), cancelled) {
            self.mark_storage_failure();
            return Err(error);
        }
        self.verify_database_census(deadline, cancelled)?;
        self.check_live(deadline, cancelled)?;
        let diagnostics_v2_cost = if self.inner.diagnostic_execution_count() != 0 {
            Some(self.inner.diagnostics_v2_cumulative_cost()?)
        } else {
            None
        };
        let summary = CutSchemaReceiptSpoolSummary {
            execution_binding: self.inner.execution_binding(),
            legacy_receipt_count: self.legacy_receipt_count,
            legacy_receipt_encoded_bytes: self.legacy_receipt_encoded_bytes,
            diagnostics_v2_terminal_count: self.diagnostics_v2_terminal_count,
            diagnostics_v2_status_counts: self.diagnostics_v2_status_counts,
            diagnostics_v2_cost,
            ordered_observation_sha256: self.ordered_hasher.clone().finalize(),
            worker_finished: true,
        };
        self.worker_finished = true;
        self.finished_summary = Some(summary.clone());
        self.check_live(deadline, cancelled)?;
        Ok(summary)
    }

    /// Return the fixed-size authenticated summary captured by the single
    /// worker EOF. This is available after the trait `finish` path consumed
    /// that same EOF; it never repeats worker I/O.
    pub fn summary_spooled(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaReceiptSpoolSummary, ItemRefusal> {
        self.check_live(deadline, cancelled)?;
        let summary = self.finished_summary.clone().ok_or_else(|| {
            ItemRefusal::Unsupported("cut schema spool has no completed worker summary".into())
        })?;
        self.check_live(deadline, cancelled)?;
        Ok(summary)
    }

    /// Release the caller-owned SQLite auxiliary inode after bounded readers
    /// have consumed any required final pages. EOF is reported by
    /// `finish_spooled`; custody remains live until this explicit close.
    pub fn close_spooled(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if !self.worker_finished || self.failed || self.custody_closed {
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt spool is not ready to close".into(),
            ));
        }
        self.check_live(deadline, cancelled)?;
        let connection = self
            .connection
            .take()
            .ok_or_else(|| ItemRefusal::Unsupported("cut schema spool connection absent".into()))?;
        connection.close().map_err(|(connection, _)| {
            drop(connection);
            self.mark_storage_failure();
            ItemRefusal::Source("cut schema receipt spool close failed".into())
        })?;
        self.scope.take();
        self.custody_closed = true;
        self.check_deadline_cancelled(deadline, cancelled)
    }

    fn guard_configurable(&self) -> Result<(), ItemRefusal> {
        if self.failed || self.worker_finished || self.custody_closed || self.next_ordinal != 0 {
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt spool operation already started".into(),
            ));
        }
        self.check_execution_live(self.deadline, &self.cancelled)
    }

    fn check_live(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        if self.failed || self.custody_closed {
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt spool is unavailable".into(),
            ));
        }
        self.check_deadline_cancelled(deadline, cancelled)
    }

    fn check_execution_live(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.check_live(deadline, cancelled)?;
        if self.worker_finished {
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt worker is already finished".into(),
            ));
        }
        if self.status_refused {
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt spool stopped after a non-verdict terminal".into(),
            ));
        }
        Ok(())
    }

    fn check_deadline_cancelled(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        if self.cancelled.load(Ordering::Acquire) || cancelled.load(Ordering::Acquire) {
            return Err(ItemRefusal::Source(
                "cut schema receipt spool cancelled".into(),
            ));
        }
        check(self.deadline.min(deadline), cancelled)
    }

    fn verify_database_census(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        let result = (|| {
            let mut hash = Digest256Hasher::new();
            hash.update(RECEIPT_SPOOL_DIGEST_DOMAIN);
            let mut count = 0u64;
            let mut total_bytes = 0u64;
            let connection = self.connection.as_ref().ok_or_else(|| {
                ItemRefusal::Unsupported("cut schema spool connection absent".into())
            })?;
            let mut statement = connection
            .prepare("SELECT ordinal,kind,payload,encoded_bytes FROM cut_schema_observation ORDER BY ordinal")
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool census failed".into()))?;
            let mut rows = statement.query([]).map_err(|_| {
                ItemRefusal::Source("cut schema receipt spool census failed".into())
            })?;
            while let Some(row) = rows
                .next()
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool census failed".into()))?
            {
                self.check_live(deadline, cancelled)?;
                let ordinal: i64 = row.get(0).map_err(|_| {
                    ItemRefusal::Source("cut schema receipt spool census row invalid".into())
                })?;
                let kind: i64 = row.get(1).map_err(|_| {
                    ItemRefusal::Source("cut schema receipt spool census row invalid".into())
                })?;
                let payload = row.get_ref(2).map_err(|_| {
                    ItemRefusal::Source("cut schema receipt spool census row invalid".into())
                })?;
                let encoded_bytes: i64 = row.get(3).map_err(|_| {
                    ItemRefusal::Source("cut schema receipt spool census row invalid".into())
                })?;
                let ordinal = u64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?;
                let kind = u8::try_from(kind).map_err(|_| ItemRefusal::Budget)?;
                let payload = payload.as_blob().map_err(|_| {
                    ItemRefusal::Source("cut schema receipt spool census row invalid".into())
                })?;
                if ordinal != count
                    || !matches!(kind, RECEIPT_SPOOL_LEGACY | RECEIPT_SPOOL_DIAGNOSTICS_V2)
                    || usize::try_from(encoded_bytes).ok() != Some(payload.len())
                {
                    return Err(ItemRefusal::Source(
                        "cut schema receipt spool census mismatch".into(),
                    ));
                }
                hash.update(&ordinal.to_be_bytes());
                hash.update(&[kind]);
                hash.update(&(payload.len() as u64).to_be_bytes());
                hash.update(payload);
                total_bytes = total_bytes
                    .checked_add(payload.len() as u64)
                    .ok_or(ItemRefusal::Budget)?;
                count = count.checked_add(1).ok_or(ItemRefusal::Budget)?;
            }
            drop(rows);
            drop(statement);
            if count != self.next_ordinal
                || total_bytes != self.total_encoded_bytes
                || hash.finalize() != self.ordered_hasher.clone().finalize()
            {
                return Err(ItemRefusal::Source(
                    "cut schema receipt spool census mismatch".into(),
                ));
            }
            Ok(())
        })();
        if result.is_err() {
            self.mark_storage_failure();
        }
        result
    }

    fn reserve_observation(&mut self, encoded_upper_bound: usize) -> Result<u64, ItemRefusal> {
        let next_count = usize::try_from(self.next_ordinal)
            .ok()
            .and_then(|count| count.checked_add(self.reserved_observations))
            .and_then(|count| count.checked_add(1))
            .filter(|count| *count <= self.limits.max_observations)
            .ok_or(ItemRefusal::Budget)?;
        let next_bytes = self
            .reserved_encoded_bytes
            .checked_add(encoded_upper_bound)
            .and_then(|bytes| {
                usize::try_from(self.total_encoded_bytes)
                    .ok()?
                    .checked_add(bytes)
            })
            .filter(|bytes| *bytes <= self.limits.max_total_encoded_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let ordinal = self.next_ordinal;
        self.reserved_observations = self
            .reserved_observations
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.reserved_encoded_bytes = self
            .reserved_encoded_bytes
            .checked_add(encoded_upper_bound)
            .ok_or(ItemRefusal::Budget)?;
        let _ = (next_count, next_bytes);
        Ok(ordinal)
    }

    fn release_reservation(&mut self, encoded_upper_bound: usize) {
        self.reserved_observations = self.reserved_observations.saturating_sub(1);
        self.reserved_encoded_bytes = self
            .reserved_encoded_bytes
            .saturating_sub(encoded_upper_bound);
    }

    fn commit_legacy_observation(
        &mut self,
        ordinal: u64,
        payload: &[u8],
    ) -> Result<(), ItemRefusal> {
        self.commit_observation(ordinal, RECEIPT_SPOOL_LEGACY, payload)?;
        self.legacy_receipt_count = self
            .legacy_receipt_count
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.legacy_receipt_encoded_bytes = self
            .legacy_receipt_encoded_bytes
            .checked_add(u64::try_from(payload.len()).map_err(|_| ItemRefusal::Budget)?)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn commit_diagnostic_observation(
        &mut self,
        ordinal: u64,
        payload: &[u8],
        status: schema_diagnostics::Status,
    ) -> Result<(), ItemRefusal> {
        self.commit_observation(ordinal, RECEIPT_SPOOL_DIAGNOSTICS_V2, payload)?;
        let status_index = status as usize;
        self.diagnostics_v2_status_counts[status_index] = self.diagnostics_v2_status_counts
            [status_index]
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.diagnostics_v2_terminal_count = self
            .diagnostics_v2_terminal_count
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn commit_observation(
        &mut self,
        ordinal: u64,
        kind: u8,
        payload: &[u8],
    ) -> Result<(), ItemRefusal> {
        if ordinal != self.next_ordinal
            || self.reserved_observations != 1
            || payload.len() > self.reserved_encoded_bytes
        {
            self.mark_storage_failure();
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt spool ordinal changed".into(),
            ));
        }
        let payload_len = payload.len();
        let payload_len_u64 = u64::try_from(payload_len).map_err(|_| ItemRefusal::Budget)?;
        let next_total = self
            .total_encoded_bytes
            .checked_add(payload_len_u64)
            .filter(|bytes| {
                u64::try_from(self.limits.max_total_encoded_bytes)
                    .is_ok_and(|limit| *bytes <= limit)
            })
            .ok_or(ItemRefusal::Budget)?;
        let next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.ordered_hasher.update(&ordinal.to_be_bytes());
        self.ordered_hasher.update(&[kind]);
        self.ordered_hasher.update(&payload_len_u64.to_be_bytes());
        self.ordered_hasher.update(payload);
        self.reserved_observations = 0;
        self.reserved_encoded_bytes = 0;
        self.total_encoded_bytes = next_total;
        self.next_ordinal = next_ordinal;
        Ok(())
    }

    fn append(&mut self, ordinal: u64, kind: u8, payload: &[u8]) -> Result<(), ItemRefusal> {
        self.check_execution_live(self.deadline, &self.cancelled)?;
        let encoded_len = u64::try_from(payload.len()).map_err(|_| ItemRefusal::Budget)?;
        let ordinal_i64 = i64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?;
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| ItemRefusal::Unsupported("cut schema spool connection absent".into()))?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool append failed".into()))?;
        transaction
            .execute(
                "INSERT INTO cut_schema_observation(ordinal, kind, payload, encoded_bytes) VALUES (?1, ?2, ?3, ?4)",
                params![ordinal_i64, i64::from(kind), payload, encoded_len as i64],
            )
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool append failed".into()))?;
        transaction
            .commit()
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool append failed".into()))?;
        self.check_execution_live(self.deadline, &self.cancelled)?;
        Ok(())
    }

    fn check_diagnostics_v2(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        let encoded_upper_bound = diagnostic_row_encoded_upper_bound(path.len(), contract.len())?;
        let ordinal = self.reserve_observation(encoded_upper_bound)?;
        let result = self
            .inner
            .produce_authenticated_diagnostics_v2_terminal_for_spool(
                path,
                raw,
                contract,
                self.deadline.min(deadline),
                cancelled,
            );
        let mut diagnostic = match result {
            Ok(diagnostic) => diagnostic,
            Err(error) => {
                self.release_reservation(encoded_upper_bound);
                if self.inner.protocol_started || self.inner.diagnostics_v2_cost_unknown {
                    self.mark_storage_failure();
                }
                return Err(error);
            }
        };
        let status = diagnostic.status();
        if status != schema_diagnostics::Status::Valid
            && let Err(error) = self
                .inner
                .precharge_pending_diagnostic(&mut diagnostic.result)
        {
            self.release_reservation(encoded_upper_bound);
            self.mark_storage_failure();
            return Err(error);
        }
        // The authenticated diagnostic and this bounded encoded row coexist
        // until append completes. The diagnostics state ceiling accounts the
        // pending report (including its vector slot); the spool ceiling
        // independently accounts the persistent row and encoder buffer.
        let payload = match encode_diagnostic(&diagnostic) {
            Ok(payload) if payload.len() <= encoded_upper_bound => payload,
            _ => {
                self.release_reservation(encoded_upper_bound);
                self.mark_storage_failure();
                return Err(ItemRefusal::Source(
                    "cut schema diagnostics spool encoding failed".into(),
                ));
            }
        };
        if let Err(error) = self.append(ordinal, RECEIPT_SPOOL_DIAGNOSTICS_V2, &payload) {
            self.release_reservation(encoded_upper_bound);
            self.mark_storage_failure();
            return Err(error);
        }
        if let Err(error) = self.commit_diagnostic_observation(ordinal, &payload, status) {
            self.mark_storage_failure();
            return Err(error);
        }
        match status {
            schema_diagnostics::Status::Valid => Ok(true),
            schema_diagnostics::Status::Invalid => {
                // Keep the same move-only invalid report available through
                // the legacy immediate rejection drain. Its report state and
                // encoded-row limits are charged independently.
                if let Err(error) = self.inner.retain_pending_diagnostic(diagnostic.result) {
                    self.mark_storage_failure();
                    return Err(error);
                }
                Ok(false)
            }
            _ => {
                let refusal = diagnostic_status_refusal(&diagnostic.result.unit.report);
                self.status_refused = true;
                if let Err(error) = self.inner.retain_pending_diagnostic(diagnostic.result) {
                    self.mark_storage_failure();
                    return Err(error);
                }
                Err(refusal)
            }
        }
    }

    fn read_page<T>(
        &mut self,
        after_ordinal: Option<u64>,
        max_rows: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        kind: u8,
        decode: fn(&[u8]) -> Result<T, ItemRefusal>,
    ) -> Result<(Vec<(u64, T)>, Option<u64>, usize), ItemRefusal> {
        let result = self.read_page_inner(
            after_ordinal,
            max_rows,
            max_bytes,
            deadline,
            cancelled,
            kind,
            decode,
        );
        if matches!(&result, Err(ItemRefusal::Source(reason)) if !reason.contains("cancelled")) {
            self.mark_storage_failure();
        }
        result
    }

    fn read_page_inner<T>(
        &mut self,
        after_ordinal: Option<u64>,
        max_rows: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        kind: u8,
        decode: fn(&[u8]) -> Result<T, ItemRefusal>,
    ) -> Result<(Vec<(u64, T)>, Option<u64>, usize), ItemRefusal> {
        self.check_live(deadline, cancelled)?;
        if !self.worker_finished {
            return Err(ItemRefusal::Unsupported(
                "cut schema receipt pages require completed worker EOF".into(),
            ));
        }
        if max_rows == 0
            || max_rows > self.limits.max_page_rows
            || max_bytes == 0
            || max_bytes > self.limits.max_page_bytes
        {
            return Err(ItemRefusal::Budget);
        }
        let max_metadata_bytes = max_rows
            .checked_add(1)
            .and_then(|rows| rows.checked_mul(std::mem::size_of::<(u64, usize)>()))
            .ok_or(ItemRefusal::Budget)?;
        if max_metadata_bytes > max_bytes {
            return Err(ItemRefusal::Budget);
        }
        let after_i64 = match after_ordinal {
            Some(value) => i64::try_from(value).map_err(|_| ItemRefusal::Budget)?,
            None => -1,
        };
        let max_rows_i64 = i64::try_from(max_rows)
            .map_err(|_| ItemRefusal::Budget)?
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| ItemRefusal::Unsupported("cut schema spool connection absent".into()))?;
        let mut statement = connection
            .prepare(
                "SELECT ordinal, length(payload), encoded_bytes FROM cut_schema_observation WHERE kind = ?1 AND ordinal > ?2 ORDER BY ordinal LIMIT ?3",
            )
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool read failed".into()))?;
        let mut rows = statement
            .query(params![i64::from(kind), after_i64, max_rows_i64])
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool read failed".into()))?;
        let mut metadata = Vec::new();
        metadata
            .try_reserve_exact(
                max_rows.checked_add(1).ok_or(ItemRefusal::Budget)?.min(
                    self.limits
                        .max_page_rows
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?,
                ),
            )
            .map_err(|_| ItemRefusal::Budget)?;
        while let Some(row) = rows
            .next()
            .map_err(|_| ItemRefusal::Source("cut schema receipt spool read failed".into()))?
        {
            self.check_live(deadline, cancelled)?;
            let ordinal: i64 = row
                .get(0)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?;
            let encoded_bytes: i64 = row
                .get(1)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?;
            let recorded_bytes: i64 = row
                .get(2)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?;
            let ordinal = u64::try_from(ordinal)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?;
            let encoded_bytes = usize::try_from(encoded_bytes)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?;
            let recorded_bytes = usize::try_from(recorded_bytes)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?;
            if recorded_bytes != encoded_bytes {
                return Err(ItemRefusal::Source(
                    "cut schema receipt spool row changed".into(),
                ));
            }
            metadata.push((ordinal, encoded_bytes));
        }
        self.check_live(deadline, cancelled)?;
        drop(rows);
        drop(statement);
        let has_more = metadata.len() > max_rows;
        if has_more {
            metadata.pop();
        }
        let metadata_slots = max_rows
            .checked_add(1)
            .and_then(|rows| rows.checked_mul(std::mem::size_of::<(u64, usize)>()))
            .ok_or(ItemRefusal::Budget)?;
        let output_slots = metadata
            .len()
            .checked_mul(std::mem::size_of::<(u64, T)>())
            .ok_or(ItemRefusal::Budget)?;
        let mut planned_encoded = 0usize;
        let mut planned_dynamic = 0usize;
        for (_, encoded_bytes) in &metadata {
            planned_encoded = planned_encoded
                .checked_add(*encoded_bytes)
                .ok_or(ItemRefusal::Budget)?;
            // The binary decoders reserve each collection exactly. This
            // conservative factor covers nested String/Vec allocations while
            // retaining encoded frames and output slots during decode.
            planned_dynamic = planned_dynamic
                .checked_add(encoded_bytes.checked_mul(64).ok_or(ItemRefusal::Budget)?)
                .ok_or(ItemRefusal::Budget)?;
        }
        if metadata_slots
            .checked_add(output_slots)
            .and_then(|bytes| bytes.checked_add(planned_encoded))
            .and_then(|bytes| bytes.checked_add(planned_dynamic))
            .is_none_or(|bytes| bytes > max_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(metadata.len())
            .map_err(|_| ItemRefusal::Budget)?;
        let mut actual_encoded = 0usize;
        let mut last_ordinal = after_ordinal;
        for (ordinal, expected_bytes) in metadata {
            self.check_live(deadline, cancelled)?;
            let ordinal_i64 = i64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?;
            let connection = self.connection.as_ref().ok_or_else(|| {
                ItemRefusal::Unsupported("cut schema spool connection absent".into())
            })?;
            let mut statement = connection
                .prepare(
                    "SELECT payload, encoded_bytes FROM cut_schema_observation WHERE kind = ?1 AND ordinal = ?2",
                )
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool read failed".into()))?;
            let mut rows = statement
                .query(params![i64::from(kind), ordinal_i64])
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool read failed".into()))?;
            let value = rows
                .next()
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool read failed".into()))?
                .ok_or_else(|| {
                    ItemRefusal::Source("cut schema receipt spool row changed".into())
                })?;
            let payload = match value
                .get_ref(0)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?
            {
                ValueRef::Blob(payload) => payload,
                _ => {
                    return Err(ItemRefusal::Source(
                        "cut schema receipt spool row changed".into(),
                    ));
                }
            };
            let recorded_bytes: i64 = value
                .get(1)
                .map_err(|_| ItemRefusal::Source("cut schema receipt spool row invalid".into()))?;
            if payload.len() != expected_bytes
                || usize::try_from(recorded_bytes).ok() != Some(expected_bytes)
            {
                return Err(ItemRefusal::Source(
                    "cut schema receipt spool row changed".into(),
                ));
            }
            actual_encoded = actual_encoded
                .checked_add(payload.len())
                .ok_or(ItemRefusal::Budget)?;
            let value = match decode(&payload) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            drop(rows);
            drop(statement);
            output.push((ordinal, value));
            last_ordinal = Some(ordinal);
        }
        self.check_live(deadline, cancelled)?;
        let next_after = if has_more { last_ordinal } else { None };
        Ok((output, next_after, actual_encoded))
    }

    fn mark_storage_failure(&mut self) {
        self.failed = true;
        self.inner.diagnostics_v2_cost_unknown = self.inner.diagnostics_v2.is_some();
        self.inner.prepared.poison(ExecutorFailure::Protocol);
    }
}

impl CutSchemaExecutor for CutWorkerSchemaExecutorSpooling {
    fn selected_source_revision(&self) -> Option<SourceRevision> {
        Some(self.inner.source_revision())
    }

    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.check_execution_live(deadline, cancelled)?;
        if self.inner.diagnostics_v2.is_some() {
            return self.check_diagnostics_v2(path, raw, contract, deadline, cancelled);
        }
        let encoded_upper_bound = encoded_scalar_receipt_len(path, contract)?;
        let ordinal = self.reserve_observation(encoded_upper_bound)?;
        let result = self
            .inner
            .check(path, raw, contract, self.deadline.min(deadline), cancelled);
        let valid = match result {
            Ok(valid) => valid,
            Err(error) => {
                self.release_reservation(encoded_upper_bound);
                if self.inner.protocol_started || self.inner.diagnostics_v2_cost_unknown {
                    self.mark_storage_failure();
                }
                return Err(error);
            }
        };
        let Some(receipt) = self.inner.receipts.pop() else {
            self.release_reservation(encoded_upper_bound);
            self.mark_storage_failure();
            return Err(ItemRefusal::Unsupported(
                "authenticated scalar receipt was not retained".into(),
            ));
        };
        let payload = match encode_receipt(&receipt) {
            Ok(payload) if payload.len() == encoded_upper_bound => payload,
            _ => {
                self.release_reservation(encoded_upper_bound);
                self.mark_storage_failure();
                return Err(ItemRefusal::Source(
                    "cut schema receipt spool encoding failed".into(),
                ));
            }
        };
        if let Err(error) = self.append(ordinal, RECEIPT_SPOOL_LEGACY, &payload) {
            self.release_reservation(encoded_upper_bound);
            self.mark_storage_failure();
            return Err(error);
        }
        if let Err(error) = self.commit_legacy_observation(ordinal, &payload) {
            self.mark_storage_failure();
            return Err(error);
        }
        Ok(valid)
    }

    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        // The spool intentionally has no resident exact-reuse index. Preserve
        // owner results by doing the same authenticated scalar operation again;
        // this charges another worker call and emits another receipt.
        self.check(path, raw, contract, deadline, cancelled)
    }

    fn schema_input_cost(
        &self,
        path: &str,
        raw: &[u8],
        contract: &str,
        ordinal: u64,
    ) -> Result<CutSchemaInputCost, ItemRefusal> {
        self.inner.schema_input_cost(path, raw, contract, ordinal)
    }

    fn set_operation_budget(&mut self, budget: BatchStreamBudget) -> Result<(), ItemRefusal> {
        CutWorkerSchemaExecutorSpooling::set_operation_budget(self, budget)
    }

    fn set_shared_schema_worker_quota(
        &mut self,
        quota: SharedSchemaWorkerQuota,
    ) -> Result<(), ItemRefusal> {
        CutWorkerSchemaExecutorSpooling::set_shared_schema_worker_quota(self, quota)
    }

    fn finish(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        self.finish_spooled(deadline, cancelled).map(|_| ())
    }

    fn check_batch(
        &mut self,
        _checks: &[CutSchemaCheck],
        _budget: BatchBudget,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<Vec<bool>, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "spooled schema batch execution is unavailable".into(),
        ))
    }
}

impl CutSchemaReceiptRange for CutWorkerSchemaExecutorSpooling {
    fn execution_binding(&self) -> CutExecutionBinding {
        CutWorkerSchemaExecutorSpooling::execution_binding(self)
    }

    fn source_revision(&self) -> SourceRevision {
        self.inner.source_revision()
    }

    fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        self.inner.contract_digest(contract)
    }

    fn receipt_count(&self) -> usize {
        usize::try_from(self.legacy_receipt_count).unwrap_or(usize::MAX)
    }

    fn receipt_range_supported(&self) -> bool {
        self.inner.diagnostics_v2.is_none()
    }

    fn receipt_page_limits(&self) -> (usize, usize) {
        (self.limits.max_page_rows, self.limits.max_page_bytes)
    }

    fn operation_budget(&self) -> BatchStreamBudget {
        self.inner.operation_budget()
    }

    fn receipt_limit_bytes(&self) -> usize {
        self.inner.receipt_limit_bytes()
    }

    fn release_child(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.inner
            .release_child(self.deadline.min(deadline), cancelled)
    }

    fn read_receipts_after(
        &mut self,
        after_ordinal: Option<u64>,
        max_rows: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaReceiptPage, ItemRefusal> {
        CutWorkerSchemaExecutorSpooling::read_receipts_after(
            self,
            after_ordinal,
            max_rows,
            max_bytes,
            deadline,
            cancelled,
        )
    }
}

fn initialize_spool_database(
    connection: &mut PinnedSqliteConnection,
    limits: CutSchemaReceiptSpoolLimits,
) -> Result<(), ItemRefusal> {
    let max_pages = i64::try_from(limits.max_database_pages).map_err(|_| ItemRefusal::Budget)?;
    let cache_pages = i64::try_from(limits.max_cache_pages).map_err(|_| ItemRefusal::Budget)?;
    connection
        .pragma_update(None, "page_size", RECEIPT_SPOOL_PAGE_SIZE as i64)
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    connection
        .pragma_update(None, "journal_mode", "OFF")
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    connection
        .pragma_update(None, "synchronous", "OFF")
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    connection
        .pragma_update(None, "temp_store", "MEMORY")
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    connection
        .pragma_update(None, "mmap_size", 0i64)
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    connection
        .pragma_update(None, "cache_size", cache_pages)
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    connection
        .pragma_update(None, "max_page_count", max_pages)
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;

    // Confirm the effective connection settings before the first schema write.
    // The caller's page/cache ceilings remain the authority; SQLite may refuse
    // a requested setting or report an effective value above those ceilings.
    let actual_page_size: u64 = connection
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    let actual_max_pages: i64 = connection
        .query_row("PRAGMA max_page_count", [], |row| row.get(0))
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    let actual_cache_pages: i64 = connection
        .query_row("PRAGMA cache_size", [], |row| row.get(0))
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    let actual_temp_store: i64 = connection
        .query_row("PRAGMA temp_store", [], |row| row.get(0))
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    let actual_mmap_size: i64 = connection
        .query_row("PRAGMA mmap_size", [], |row| row.get(0))
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    let actual_synchronous: i64 = connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    let actual_journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    if actual_page_size != RECEIPT_SPOOL_PAGE_SIZE
        || actual_max_pages <= 0
        || actual_max_pages > max_pages
        || actual_cache_pages <= 0
        || actual_cache_pages > cache_pages
        || actual_temp_store != 2
        || actual_mmap_size != 0
        || actual_synchronous != 0
        || !actual_journal_mode.eq_ignore_ascii_case("off")
    {
        return Err(ItemRefusal::Source(
            "cut schema receipt spool limits could not be enforced".into(),
        ));
    }

    connection
        .execute_batch(
            "CREATE TABLE cut_schema_observation (ordinal INTEGER PRIMARY KEY, kind INTEGER NOT NULL, payload BLOB NOT NULL, encoded_bytes INTEGER NOT NULL); CREATE INDEX cut_schema_observation_kind_ordinal ON cut_schema_observation(kind, ordinal);",
        )
        .map_err(|_| ItemRefusal::Source("cut schema receipt spool setup failed".into()))?;
    Ok(())
}
