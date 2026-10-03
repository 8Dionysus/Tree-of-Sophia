//! Held operation-local spill for the physical pack inventory used by native
//! V2 cold closure. The selected rootset and exact auxiliary SQLite scope are
//! fixed at construction; the segment-store callback alone can complete rows.

use rusqlite::{OptionalExtension, params, types::ValueRef};
use std::{
    cell::{Cell, RefCell},
    fs::File,
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tos_foundation::Digest256;
use tos_segment_store::{
    AuthenticatedTreePackSetV2, Result as SegmentResult, SegmentError, SegmentErrorCode,
};
use tos_source_store::{
    PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteConnection, PinnedSqliteIoBudget,
    PinnedSqliteSpaceBudget,
};

const DIGEST_BYTES: usize = 32;

#[derive(Clone, Copy, Eq, PartialEq)]
enum SpillPhase {
    Observing,
    Verifying,
    Failed,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum HistoryPhase {
    Loading,
    Sealed,
    Iterating,
    Complete,
    Failed,
}

pub(crate) struct V2ColdHistoryRow {
    pub revision: [u8; DIGEST_BYTES],
    pub base_revision: Option<[u8; DIGEST_BYTES]>,
    pub raw: Vec<u8>,
}

/// Finite in-memory terms for the SQLite-backed exact digest set.
///
/// The SQLite main-file logical/allocation limits remain on the held request.
/// `retained_operation_state_bytes` and `sqlite_native_overhead_bytes` are
/// caller-owned terms in the same operation state bill; this type does not
/// create a new state or storage grant.
#[derive(Clone, Copy, Debug)]
pub(crate) struct V2SeenPackSpillLimits {
    /// Existing cumulative tree-node ceiling; distinct observed packs cannot
    /// exceed the number of physical nodes traversed.
    pub max_tree_nodes: u64,
    /// Existing retained-history ceiling; the spill does not create a larger
    /// row grant.
    pub max_history_roots: u64,
    /// Source-owned maximum canonical history-row encoding.
    pub max_history_row_bytes: usize,
    pub cache_bytes: usize,
    pub max_operation_state_bytes: usize,
    pub retained_operation_state_bytes: usize,
    pub sqlite_native_overhead_bytes: usize,
}

/// Owner-supplied scratch for one side of a cold source/target verification.
/// The workspace FD, auxiliary request and finite cache/native overhead are
/// selected under the original operation's grants.
pub struct V2SeenPackSpillRequest {
    pub workspace: File,
    pub request: PinnedSqliteAuxRequest,
    pub cache_bytes: usize,
    pub sqlite_native_overhead_bytes: usize,
}

/// Independent held scratch requests for the source and fresh target. These
/// scopes never share verified rows, even when the copied rootset is equal.
pub struct V2SeenPackSpillRequests {
    pub source: V2SeenPackSpillRequest,
    pub target: V2SeenPackSpillRequest,
}

impl V2SeenPackSpillRequests {
    pub(crate) fn validate_for_operation(
        &self,
        io: &PinnedSqliteIoBudget,
        auxiliary_space: &PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
    ) -> std::io::Result<()> {
        let source_cap = validate_request(&self.source, io, auxiliary_space, deadline, cancelled)?;
        let target_cap = validate_request(&self.target, io, auxiliary_space, deadline, cancelled)?;
        let remaining = auxiliary_space.snapshot();
        let combined_cap = source_cap
            .checked_add(target_cap)
            .ok_or_else(|| invalid("V2 spill allocation ceiling overflow"))?;
        if !remaining.ledger_consistent
            || remaining.allocation_anomalies != 0
            || remaining.reserved_current_bytes > remaining.declared_available_bytes
            || combined_cap
                > remaining
                    .declared_available_bytes
                    .saturating_sub(remaining.reserved_current_bytes)
        {
            return Err(invalid(
                "V2 source and target spill ceilings exceed held auxiliary space",
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &V2SeenPackSpillRequest,
    io: &PinnedSqliteIoBudget,
    auxiliary_space: &PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> std::io::Result<u64> {
    let limits = request.request.limits;
    let allocation_ceiling = [
        limits.main_allocated_bytes,
        limits.temp_db_allocated_bytes,
        limits.main_journal_allocated_bytes,
        limits.temp_journal_allocated_bytes,
        limits.other_aux_aggregate_allocated_bytes,
    ]
    .into_iter()
    .try_fold(0u64, |total, bytes| total.checked_add(bytes))
    .ok_or_else(|| invalid("V2 spill auxiliary allocation ceiling overflow"))?;
    if !request.request.io_budget.shares_with(io)
        || !request.request.space_budget.shares_with(auxiliary_space)
        || request.request.deadline != deadline
        || !Arc::ptr_eq(&request.request.cancelled, cancelled)
        || allocation_ceiling == 0
        || request.cache_bytes == 0
        || request.cache_bytes == usize::MAX
        || request.sqlite_native_overhead_bytes == 0
        || request.sqlite_native_overhead_bytes == usize::MAX
    {
        return Err(invalid(
            "V2 spill request differs from the original operation envelope",
        ));
    }
    Ok(allocation_ceiling)
}

impl V2SeenPackSpillLimits {
    pub(crate) fn state_charge(self) -> std::io::Result<usize> {
        let cache_kib = self.cache_bytes / 1024;
        if self.max_tree_nodes == 0
            || self.max_tree_nodes == u64::MAX
            || self.max_history_roots == 0
            || self.max_history_roots == u64::MAX
            || self.max_history_row_bytes == 0
            || self.max_history_row_bytes == usize::MAX
            || cache_kib == 0
            || cache_kib > i32::MAX as usize
            || self.cache_bytes == usize::MAX
            || self.sqlite_native_overhead_bytes == 0
            || self.sqlite_native_overhead_bytes == usize::MAX
            || self.max_operation_state_bytes == 0
            || self.max_operation_state_bytes == usize::MAX
        {
            return Err(invalid("V2 pack spill profile is not finite"));
        }
        let charge = size_of::<V2SeenPackSpill>()
            .checked_add(self.cache_bytes)
            // Includes the ordered cursor, returned row, digest passed to the
            // physical verifier, and one conversion/callback scratch copy.
            .and_then(|bytes| bytes.checked_add(4 * DIGEST_BYTES))
            // One authenticated history row is resident while its compact
            // tuple is decoded and the corresponding roots are checked.
            .and_then(|bytes| bytes.checked_add(self.max_history_row_bytes.checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(self.sqlite_native_overhead_bytes))
            .ok_or_else(|| invalid("V2 pack spill state charge overflow"))?;
        if self
            .retained_operation_state_bytes
            .checked_add(charge)
            .is_none_or(|bytes| bytes > self.max_operation_state_bytes)
        {
            return Err(invalid(
                "V2 pack spill exceeds the held operation state bill",
            ));
        }
        Ok(charge)
    }
}

/// Creates a fresh spill for one held physical store verification. Callers
/// must open a distinct spill for source and fresh-target closure, even when
/// their copied selector and store metadata match.
pub(crate) struct V2SeenPackSpill {
    db: RefCell<PinnedSqliteConnection>,
    _scope: PinnedSqliteAuxScope,
    io: PinnedSqliteIoBudget,
    space: PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    physical_root: (u64, u64),
    store_id: [u8; 16],
    domain_digest: Digest256,
    closure_binding: Digest256,
    limits: V2SeenPackSpillLimits,
    entries: Cell<u64>,
    phase: Cell<SpillPhase>,
    binding_checked: Cell<bool>,
    history_entries: Cell<u64>,
    history_phase: Cell<HistoryPhase>,
}

impl V2SeenPackSpill {
    pub(crate) fn open(
        workspace: File,
        request: PinnedSqliteAuxRequest,
        physical_root: (u64, u64),
        store_id: [u8; 16],
        domain_digest: Digest256,
        closure_binding: Digest256,
        limits: V2SeenPackSpillLimits,
    ) -> std::io::Result<Arc<Self>> {
        limits.state_charge()?;
        let io = request.io_budget.clone();
        let space = request.space_budget.clone();
        let deadline = request.deadline;
        let cancelled = request.cancelled.clone();
        let mut scope = PinnedSqliteAuxScope::new(workspace, request)
            .map_err(|_| invalid("V2 pack spill auxiliary scope refused"))?;
        let db = scope
            .open_connection()
            .map_err(|_| invalid("V2 pack spill auxiliary connection refused"))?;
        configure_db(&db, limits.cache_bytes)?;
        db.execute_batch(
            "CREATE TABLE v2_cold_seen_pack(\
                 digest BLOB NOT NULL PRIMARY KEY CHECK(length(digest)=32),\
                 verified INTEGER NOT NULL CHECK(verified IN (0,1))\
             ) WITHOUT ROWID;\
             CREATE INDEX v2_cold_seen_pack_pending ON v2_cold_seen_pack(digest)\
                 WHERE verified=0;\
             CREATE TABLE v2_cold_history(\
                 revision BLOB NOT NULL PRIMARY KEY CHECK(length(revision)=32),\
                 base_revision BLOB CHECK(base_revision IS NULL OR length(base_revision)=32),\
                 raw BLOB NOT NULL CHECK(length(raw)>0),\
                 color INTEGER NOT NULL DEFAULT 0 CHECK(color IN (0,1,2)),\
                 walk_root BLOB CHECK(walk_root IS NULL OR length(walk_root)=32)\
             ) WITHOUT ROWID;\
             CREATE INDEX v2_cold_history_unvisited ON v2_cold_history(revision)\
                 WHERE color=0;\
             CREATE INDEX v2_cold_history_walk ON v2_cold_history(walk_root)\
                 WHERE color=1;",
        )
        .map_err(sql_invalid)?;
        let spill = Arc::new(Self {
            db: RefCell::new(db),
            _scope: scope,
            io,
            space,
            deadline,
            cancelled,
            physical_root,
            store_id,
            domain_digest,
            closure_binding,
            limits,
            entries: Cell::new(0),
            phase: Cell::new(SpillPhase::Observing),
            binding_checked: Cell::new(false),
            history_entries: Cell::new(0),
            history_phase: Cell::new(HistoryPhase::Loading),
        });
        spill
            .check_context()
            .map_err(|_| invalid("V2 pack spill request context is unavailable"))?;
        Ok(spill)
    }

    pub(crate) fn state_charge(&self) -> std::io::Result<usize> {
        self.limits.state_charge()
    }

    pub(crate) fn observe_history(
        &self,
        revision: [u8; DIGEST_BYTES],
        base_revision: Option<[u8; DIGEST_BYTES]>,
        raw: &[u8],
    ) -> std::io::Result<()> {
        let result = (|| {
            self.check_context()
                .map_err(|_| invalid("V2 cold history context is unavailable"))?;
            if self.history_phase.get() != HistoryPhase::Loading
                || raw.is_empty()
                || raw.len() > self.limits.max_history_row_bytes
            {
                return Err(invalid("V2 cold history row is outside its finite profile"));
            }
            let next_entries = self
                .history_entries
                .get()
                .checked_add(1)
                .filter(|count| *count <= self.limits.max_history_roots)
                .ok_or_else(|| invalid("V2 cold history row ceiling exceeded"))?;
            let base = base_revision.map(|value| value.to_vec());
            let changed = self
                .db
                .borrow()
                .execute(
                    "INSERT INTO v2_cold_history(revision,base_revision,raw) VALUES(?1,?2,?3)",
                    params![revision.as_slice(), base.as_deref(), raw],
                )
                .map_err(sql_invalid)?;
            if changed != 1 {
                return Err(invalid(
                    "V2 cold history insertion changed an unexpected row count",
                ));
            }
            self.history_entries.set(next_entries);
            self.check_context()
                .map_err(|_| invalid("V2 cold history context is unavailable"))
        })();
        if result.is_err() {
            self.history_phase.set(HistoryPhase::Failed);
            self.phase.set(SpillPhase::Failed);
        }
        result
    }

    /// Seal the authenticated stream after its true EOF, check all base links,
    /// and walk indexed color rows without a resident history vector or an
    /// in-memory DFS stack. Each row transitions through the colors at most
    /// once, so the number of row visits and updates stays linear in history.
    pub(crate) fn seal_history(
        &self,
        current_revision: [u8; DIGEST_BYTES],
        expected_entries: u64,
    ) -> std::io::Result<()> {
        let result = self.seal_history_inner(current_revision, expected_entries);
        if result.is_err() {
            self.history_phase.set(HistoryPhase::Failed);
            self.phase.set(SpillPhase::Failed);
        }
        result
    }

    fn seal_history_inner(
        &self,
        current_revision: [u8; DIGEST_BYTES],
        expected_entries: u64,
    ) -> std::io::Result<()> {
        self.check_context()
            .map_err(|_| invalid("V2 cold history context is unavailable"))?;
        if self.history_phase.get() != HistoryPhase::Loading
            || self.history_entries.get() != expected_entries
            || expected_entries == 0
            || expected_entries > self.limits.max_history_roots
        {
            return Err(invalid("V2 cold history EOF or row count differs"));
        }
        let db = self.db.borrow();
        let current_exists = db
            .query_row(
                "SELECT 1 FROM v2_cold_history WHERE revision=?1",
                params![current_revision.as_slice()],
                |_| Ok(()),
            )
            .optional()
            .map_err(sql_invalid)?
            .is_some();
        if !current_exists {
            return Err(invalid("V2 cold current revision is absent from history"));
        }
        let missing_base = db
            .query_row(
                "SELECT child.revision FROM v2_cold_history AS child \
                 WHERE child.base_revision IS NOT NULL \
                   AND NOT EXISTS (SELECT 1 FROM v2_cold_history AS parent \
                                   WHERE parent.revision=child.base_revision) LIMIT 1",
                [],
                digest_row,
            )
            .optional()
            .map_err(sql_invalid)?;
        drop(db);
        if missing_base.is_some() {
            return Err(invalid("V2 cold history base revision is absent"));
        }

        loop {
            self.check_context()
                .map_err(|_| invalid("V2 cold history context is unavailable"))?;
            let start = self
                .db
                .borrow()
                .query_row(
                    "SELECT revision FROM v2_cold_history WHERE color=0 ORDER BY revision LIMIT 1",
                    [],
                    digest_row,
                )
                .optional()
                .map_err(sql_invalid)?;
            let Some(start) = start else { break };
            let mut current = start;
            let mut hops = 0u64;
            loop {
                self.check_context()
                    .map_err(|_| invalid("V2 cold history context is unavailable"))?;
                let (base, color) = self
                    .db
                    .borrow()
                    .query_row(
                        "SELECT base_revision,color FROM v2_cold_history WHERE revision=?1",
                        params![current.as_slice()],
                        |row| {
                            let base = match row.get_ref(0)? {
                                ValueRef::Null => None,
                                ValueRef::Blob(raw) if raw.len() == DIGEST_BYTES => {
                                    let mut digest = [0; DIGEST_BYTES];
                                    digest.copy_from_slice(raw);
                                    Some(digest)
                                }
                                _ => return Err(rusqlite::Error::InvalidQuery),
                            };
                            Ok((base, row.get::<_, i64>(1)?))
                        },
                    )
                    .optional()
                    .map_err(sql_invalid)?
                    .ok_or_else(|| invalid("V2 cold history walk lost a revision"))?;
                match color {
                    0 => {
                        let changed = self
                            .db
                            .borrow()
                            .execute(
                                "UPDATE v2_cold_history SET color=1,walk_root=?1 \
                                 WHERE revision=?2 AND color=0",
                                params![start.as_slice(), current.as_slice()],
                            )
                            .map_err(sql_invalid)?;
                        if changed != 1 {
                            return Err(invalid("V2 cold history walk color changed unexpectedly"));
                        }
                        hops = hops
                            .checked_add(1)
                            .filter(|count| *count <= self.limits.max_history_roots)
                            .ok_or_else(|| invalid("V2 cold history cycle detected"))?;
                        match base {
                            Some(base) => current = base,
                            None => break,
                        }
                    }
                    1 => return Err(invalid("V2 cold history cycle detected")),
                    2 => break,
                    _ => return Err(invalid("V2 cold history walk color is invalid")),
                }
            }
            let changed = self
                .db
                .borrow()
                .execute(
                    "UPDATE v2_cold_history SET color=2,walk_root=NULL \
                     WHERE color=1 AND walk_root=?1",
                    params![start.as_slice()],
                )
                .map_err(sql_invalid)?;
            if changed == 0 {
                return Err(invalid("V2 cold history walk made no progress"));
            }
        }
        self.check_context()
            .map_err(|_| invalid("V2 cold history context is unavailable"))?;
        self.history_phase.set(HistoryPhase::Sealed);
        Ok(())
    }

    pub(crate) fn next_history(
        &self,
        after: Option<[u8; DIGEST_BYTES]>,
    ) -> std::io::Result<Option<V2ColdHistoryRow>> {
        let result = (|| {
            self.check_context()
                .map_err(|_| invalid("V2 cold history context is unavailable"))?;
            if !matches!(
                self.history_phase.get(),
                HistoryPhase::Sealed | HistoryPhase::Iterating
            ) {
                return Err(invalid("V2 cold history cursor is not sealed"));
            }
            let row = {
                let db = self.db.borrow();
                let query = match after {
                    Some(_) => {
                        "SELECT revision,base_revision,raw FROM v2_cold_history \
                                WHERE revision>?1 ORDER BY revision LIMIT 1"
                    }
                    None => {
                        "SELECT revision,base_revision,raw FROM v2_cold_history \
                             ORDER BY revision LIMIT 1"
                    }
                };
                let decode = |row: &rusqlite::Row<'_>| {
                    let revision = digest_row(row)?;
                    let base_revision = match row.get_ref(1)? {
                        ValueRef::Null => None,
                        ValueRef::Blob(raw) if raw.len() == DIGEST_BYTES => {
                            let mut digest = [0; DIGEST_BYTES];
                            digest.copy_from_slice(raw);
                            Some(digest)
                        }
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    };
                    let raw = match row.get_ref(2)? {
                        ValueRef::Blob(raw)
                            if !raw.is_empty()
                                && raw.len() <= self.limits.max_history_row_bytes =>
                        {
                            raw.to_vec()
                        }
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    };
                    Ok(V2ColdHistoryRow {
                        revision,
                        base_revision,
                        raw,
                    })
                };
                if let Some(after) = after {
                    db.query_row(query, params![after.as_slice()], decode)
                        .optional()
                } else {
                    db.query_row(query, [], decode).optional()
                }
                .map_err(sql_invalid)?
            };
            self.check_context()
                .map_err(|_| invalid("V2 cold history context is unavailable"))?;
            match row {
                Some(row) => {
                    self.history_phase.set(HistoryPhase::Iterating);
                    Ok(Some(row))
                }
                None => {
                    self.history_phase.set(HistoryPhase::Complete);
                    Ok(None)
                }
            }
        })();
        if result.is_err() {
            self.history_phase.set(HistoryPhase::Failed);
            self.phase.set(SpillPhase::Failed);
        }
        result
    }

    pub(crate) fn finish_history(&self, processed: u64) -> std::io::Result<()> {
        self.check_context()
            .map_err(|_| invalid("V2 cold history context is unavailable"))?;
        if self.history_phase.get() != HistoryPhase::Complete
            || processed != self.history_entries.get()
            || processed == 0
        {
            self.history_phase.set(HistoryPhase::Failed);
            self.phase.set(SpillPhase::Failed);
            return Err(invalid("V2 cold history ordered EOF differs"));
        }
        Ok(())
    }

    fn check_context(&self) -> SegmentResult<()> {
        if self.phase.get() == SpillPhase::Failed {
            return Err(segment_error(
                SegmentErrorCode::BudgetExceeded,
                "V2 pack spill is terminal after a prior refusal",
            ));
        }
        if Instant::now() >= self.deadline {
            return Err(segment_error(
                SegmentErrorCode::DeadlineExceeded,
                "V2 pack spill deadline exceeded",
            ));
        }
        if self.cancelled.load(Ordering::Acquire) {
            return Err(segment_error(
                SegmentErrorCode::Cancelled,
                "V2 pack spill cancelled",
            ));
        }
        if self.io.snapshot().failure.is_some() {
            return Err(segment_error(
                SegmentErrorCode::BudgetExceeded,
                "V2 pack spill shares a failed I/O ledger",
            ));
        }
        let space = self.space.snapshot();
        if !space.ledger_consistent || space.allocation_anomalies != 0 {
            return Err(segment_error(
                SegmentErrorCode::BudgetExceeded,
                "V2 pack spill shares a failed physical-space ledger",
            ));
        }
        Ok(())
    }

    fn observe_inner(&self, digest: Digest256) -> SegmentResult<()> {
        self.check_context()?;
        if !self.binding_checked.get() || self.phase.get() != SpillPhase::Observing {
            return Err(segment_error(
                SegmentErrorCode::InvalidReceipt,
                "V2 pack spill is not bound for observation",
            ));
        }
        let bytes = *digest.as_bytes();
        let existing = self
            .db
            .borrow()
            .query_row(
                "SELECT verified FROM v2_cold_seen_pack WHERE digest=?1",
                params![bytes.as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| segment_error(SegmentErrorCode::Io, "V2 pack spill lookup failed"))?;
        match existing {
            Some(0 | 1) => return self.check_context(),
            Some(_) => {
                return Err(segment_error(
                    SegmentErrorCode::InvalidFormat,
                    "V2 pack spill row state differs",
                ));
            }
            None => {}
        }
        let next_entries = self
            .entries
            .get()
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_tree_nodes)
            .ok_or_else(|| {
                segment_error(
                    SegmentErrorCode::BudgetExceeded,
                    "V2 pack spill distinct digest ceiling exceeded",
                )
            })?;
        let changed = self
            .db
            .borrow()
            .execute(
                "INSERT INTO v2_cold_seen_pack(digest,verified) VALUES(?1,0)",
                params![bytes.as_slice()],
            )
            .map_err(|_| segment_error(SegmentErrorCode::Io, "V2 pack spill insert failed"))?;
        if changed != 1 {
            return Err(segment_error(
                SegmentErrorCode::InvalidFormat,
                "V2 pack spill insertion changed an unexpected row count",
            ));
        }
        self.entries.set(next_entries);
        self.check_context()
    }

    fn next_pending(
        &self,
        after: Option<[u8; DIGEST_BYTES]>,
    ) -> SegmentResult<Option<[u8; DIGEST_BYTES]>> {
        self.check_context()?;
        let db = self.db.borrow();
        let query = match after {
            Some(_) => {
                "SELECT digest FROM v2_cold_seen_pack WHERE verified=0 AND digest>?1 ORDER BY digest LIMIT 1"
            }
            None => "SELECT digest FROM v2_cold_seen_pack WHERE verified=0 ORDER BY digest LIMIT 1",
        };
        let value = if let Some(after) = after {
            db.query_row(query, params![after.as_slice()], digest_row)
                .optional()
        } else {
            db.query_row(query, [], digest_row).optional()
        }
        .map_err(|_| segment_error(SegmentErrorCode::Io, "V2 pack spill ordered read failed"))?;
        drop(db);
        self.check_context()?;
        Ok(value)
    }

    fn mark_verified(&self, digest: [u8; DIGEST_BYTES]) -> SegmentResult<()> {
        self.check_context()?;
        if self.phase.get() != SpillPhase::Verifying {
            return Err(segment_error(
                SegmentErrorCode::InvalidReceipt,
                "V2 pack spill is not verifying pending rows",
            ));
        }
        let changed = self
            .db
            .borrow()
            .execute(
                "UPDATE v2_cold_seen_pack SET verified=1 WHERE digest=?1 AND verified=0",
                params![digest.as_slice()],
            )
            .map_err(|_| segment_error(SegmentErrorCode::Io, "V2 pack spill completion failed"))?;
        if changed != 1 {
            return Err(segment_error(
                SegmentErrorCode::InvalidFormat,
                "V2 pack spill pending row changed unexpectedly",
            ));
        }
        self.check_context()
    }
}

// SAFETY: this implementation holds one private auxiliary SQLite database
// whose request is fixed at construction. All digest writes are exact 32-byte
// primary-key inserts under a finite row cap. Pending rows are traversed by a
// binary-key cursor and changed to verified only after SegmentStore's supplied
// physical read, digest verification and packed-frame scan succeeds. Any error
// makes the instance terminal, and each source/target closure gets a new scope.
unsafe impl AuthenticatedTreePackSetV2 for V2SeenPackSpill {
    fn check_binding(
        &self,
        physical_root: (u64, u64),
        store_id: [u8; 16],
        domain_digest: Digest256,
        closure_binding: Digest256,
    ) -> SegmentResult<()> {
        self.check_context()?;
        if self.physical_root != physical_root
            || self.store_id != store_id
            || self.domain_digest != domain_digest
            || self.closure_binding != closure_binding
        {
            self.phase.set(SpillPhase::Failed);
            return Err(segment_error(
                SegmentErrorCode::InvalidReceipt,
                "V2 pack spill store or selected closure binding differs",
            ));
        }
        self.binding_checked.set(true);
        Ok(())
    }

    fn observe(&self, digest: Digest256) -> SegmentResult<()> {
        let result = self.observe_inner(digest);
        if result.is_err() {
            self.phase.set(SpillPhase::Failed);
        }
        result
    }

    fn verify_pending(
        &self,
        verify: &mut dyn FnMut(Digest256) -> SegmentResult<()>,
    ) -> SegmentResult<()> {
        let result = (|| {
            self.check_context()?;
            if !self.binding_checked.get() || self.phase.get() != SpillPhase::Observing {
                return Err(segment_error(
                    SegmentErrorCode::InvalidReceipt,
                    "V2 pack spill is not ready to verify pending rows",
                ));
            }
            self.phase.set(SpillPhase::Verifying);
            let mut after = None;
            while let Some(raw) = self.next_pending(after)? {
                if after.is_some_and(|previous| raw <= previous) {
                    return Err(segment_error(
                        SegmentErrorCode::InvalidFormat,
                        "V2 pack spill order did not advance",
                    ));
                }
                let digest = Digest256::from_bytes(raw);
                verify(digest)?;
                self.mark_verified(raw)?;
                after = Some(raw);
            }
            self.check_context()?;
            self.phase.set(SpillPhase::Observing);
            Ok(())
        })();
        if result.is_err() {
            self.phase.set(SpillPhase::Failed);
        }
        result
    }
}

fn configure_db(db: &PinnedSqliteConnection, cache_bytes: usize) -> std::io::Result<()> {
    let cache_kib = cache_bytes / 1024;
    if cache_kib == 0 || cache_kib > i32::MAX as usize {
        return Err(invalid("V2 pack spill cache is not representable"));
    }
    db.pragma_update(None, "page_size", 4096i64)
        .map_err(sql_invalid)?;
    db.pragma_update(None, "journal_mode", "OFF")
        .map_err(sql_invalid)?;
    db.pragma_update(None, "synchronous", 0i64)
        .map_err(sql_invalid)?;
    db.pragma_update(None, "temp_store", "FILE")
        .map_err(sql_invalid)?;
    db.pragma_update(None, "cache_spill", "ON")
        .map_err(sql_invalid)?;
    db.pragma_update(None, "mmap_size", 0i64)
        .map_err(sql_invalid)?;
    db.pragma_update(None, "cache_size", -(cache_kib as i64))
        .map_err(sql_invalid)?;
    let journal = db
        .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
        .map_err(sql_invalid)?;
    let synchronous = db
        .query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
        .map_err(sql_invalid)?;
    let temp_store = db
        .query_row("PRAGMA temp_store", [], |row| row.get::<_, i64>(0))
        .map_err(sql_invalid)?;
    let cache_spill = db
        .query_row("PRAGMA cache_spill", [], |row| row.get::<_, i64>(0))
        .map_err(sql_invalid)?;
    let mmap = db
        .query_row("PRAGMA mmap_size", [], |row| row.get::<_, i64>(0))
        .map_err(sql_invalid)?;
    let cache = db
        .query_row("PRAGMA cache_size", [], |row| row.get::<_, i64>(0))
        .map_err(sql_invalid)?;
    let page_size = db
        .query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
        .map_err(sql_invalid)?;
    if !journal.eq_ignore_ascii_case("off")
        || synchronous != 0
        || temp_store != 1
        || cache_spill == 0
        || mmap != 0
        || cache != -(cache_kib as i64)
        || page_size != 4096
    {
        return Err(invalid("V2 pack spill SQLite policy changed"));
    }
    Ok(())
}

fn digest_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<[u8; DIGEST_BYTES]> {
    match row.get_ref(0)? {
        ValueRef::Blob(raw) if raw.len() == DIGEST_BYTES => {
            let mut digest = [0; DIGEST_BYTES];
            digest.copy_from_slice(raw);
            Ok(digest)
        }
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn segment_error(code: SegmentErrorCode, detail: &'static str) -> SegmentError {
    SegmentError::new(code, detail)
}

fn invalid(detail: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, detail)
}

fn sql_invalid(_: rusqlite::Error) -> std::io::Error {
    invalid("V2 pack spill SQLite operation refused")
}
