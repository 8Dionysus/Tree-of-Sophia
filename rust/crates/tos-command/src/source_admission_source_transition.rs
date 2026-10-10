//! Build a bounded V2 successor proposal from a complete selected-source
//! census and one exact authenticated V2 base.
//!
//! This is a proposal producer only. It derives additions and replacements
//! from the held filesystem cut, refuses deletions until an owning retirement
//! event is supplied, and leaves Native validation plus the ordinary V2
//! publisher as the only admission path.

use super::source_admission::{
    AdmissionBatch, AdmissionLimits, AdmissionWorkBudget, SourceUpdate, active, invalid,
};
use super::source_admission_segment_v2::SourceRevisionRootsV2;
use super::source_admission_source_census::{
    SourceCensusLimits, SourceCensusScan, SourceCensusSummary, SourceCensusWorkKind,
    census_selected_to_scratch, working_state_upper_bound,
};
use super::source_admission_v2_reader::{V2MemberTupleObservation, V2ReadSession};
use super::source_admission_v2_seen_pack::configure_db;
use super::source_foundation_admission::NativeSourceValidator;
use rusqlite::Row;
use std::{
    collections::BTreeMap,
    fs::File,
    io,
    mem::size_of,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, SourceRevision};
use tos_source_store::{
    CorpusCurrentSelection, PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteConnection,
    PinnedSqliteIoBudget,
};

const CENSUS_CONTROL_SQL_WORK_UNITS: u64 = 6;
const BATCH_BUILDER_CONTROL_WORK_UNITS: u64 = 16;
const BATCH_BUILDER_PER_MEMBER_WORK_UNITS: u64 = 3;

/// A finite partition carved from the selected invocation's existing source
/// state, member, byte, SQLite-cache, and local work limits. It creates no
/// new operation grant. The caller reserves `max_state_slice_bytes` on the
/// original Native source ledger before preparation. `max_changed_members`
/// is a caller-selected subset of the original member ceiling; callers
/// should choose it from their existing profile rather than treating this
/// finite in-memory producer as a global streaming solution.
#[derive(Clone, Copy)]
pub(crate) struct SourceTransitionProfile {
    pub(crate) census: SourceCensusLimits,
    pub(crate) admission: AdmissionLimits,
    pub(crate) max_state_slice_bytes: usize,
    pub(crate) census_state_bytes: usize,
    pub(crate) changed_rows_state_bytes: usize,
    pub(crate) batch_builder_state_bytes: usize,
    pub(crate) retained_fence_state_bytes: usize,
    /// Full external live-state envelope supplied to each authenticated base
    /// member seek. It includes the merge map, retained roots/current rows,
    /// transition fence/work state, and the held AUX cache/native allowance.
    pub(crate) cursor_caller_state_bytes: usize,
    pub(crate) sqlite_cache_bytes: usize,
    pub(crate) sqlite_native_overhead_bytes: usize,
    pub(crate) max_changed_members: usize,
    /// Local bounded units for the two censuses and ordered base merge. Native
    /// currently has no shared mutable work ledger; this is not represented
    /// as a global invocation-work debit.
    pub(crate) max_work_units: u64,
}

impl SourceTransitionProfile {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_original_limits(
        original_state_slice_bytes: usize,
        mut census: SourceCensusLimits,
        admission: AdmissionLimits,
        max_changed_members: usize,
        sqlite_cache_bytes: usize,
        sqlite_native_overhead_bytes: usize,
        selected_work_units: u64,
    ) -> io::Result<Self> {
        let admission = admission.validate()?;
        let max_files = usize::try_from(census.max_files)
            .map_err(|_| invalid("source transition member limit exceeds address space"))?;
        if original_state_slice_bytes == 0
            || original_state_slice_bytes == usize::MAX
            || max_changed_members == 0
            || max_changed_members > max_files
            || max_changed_members > admission.max_members
            || census.max_files == 0
            || census.max_files == u64::MAX
            || census.max_files > i64::MAX as u64
            || census.max_files > admission.max_members as u64
            || census.max_directories == 0
            || census.max_directories == u64::MAX
            || census.max_directories > i64::MAX as u64
            || census.max_entries == 0
            || census.max_entries == u64::MAX
            || census.max_entries > i64::MAX as u64
            || census.max_path_bytes > admission.max_batch_bytes
            || census.max_path_bytes == usize::MAX
            || census.max_depth == usize::MAX
            || i64::try_from(census.max_depth).is_err()
            || census.max_member_bytes == 0
            || census.max_member_bytes == u64::MAX
            || census.max_member_bytes > i64::MAX as u64
            || census.max_member_bytes > admission.max_member_bytes
            || census.max_source_bytes == 0
            || census.max_source_bytes == u64::MAX
            || census.max_source_bytes > i64::MAX as u64
            || census.max_source_bytes > admission.max_source_bytes
            || sqlite_cache_bytes < 1024
            || sqlite_cache_bytes / 1024 > i32::MAX as usize
            || sqlite_native_overhead_bytes == 0
            || sqlite_native_overhead_bytes == usize::MAX
            || selected_work_units == 0
            || selected_work_units == u64::MAX
        {
            return Err(invalid(
                "source transition limits exceed the selected original profile",
            ));
        }
        census.state_slice_bytes = working_state_upper_bound(census.max_path_bytes)?;
        let changed_rows_state_bytes =
            changed_rows_state_upper_bound(max_changed_members, census.max_path_bytes)?;
        let batch_builder_state_bytes =
            batch_builder_state_upper_bound(max_changed_members, census.max_path_bytes)?;
        let retained_fence_state_bytes = retained_fence_state_upper_bound()?;
        let cursor_caller_state_bytes = cursor_caller_state_upper_bound(
            changed_rows_state_bytes,
            retained_fence_state_bytes,
            sqlite_cache_bytes,
            sqlite_native_overhead_bytes,
        )?;
        let required_work = minimum_work_units(census, max_changed_members)?;
        if required_work > selected_work_units {
            return Err(invalid(
                "source transition work bound exceeds the selected original ceiling",
            ));
        }
        let profile = Self {
            census,
            admission,
            max_state_slice_bytes: original_state_slice_bytes,
            census_state_bytes: census.state_slice_bytes,
            changed_rows_state_bytes,
            batch_builder_state_bytes,
            retained_fence_state_bytes,
            cursor_caller_state_bytes,
            sqlite_cache_bytes,
            sqlite_native_overhead_bytes,
            max_changed_members,
            max_work_units: selected_work_units,
        };
        profile.check_state_partition()?;
        Ok(profile)
    }

    fn check_state_partition(self) -> io::Result<()> {
        let minimum_cursor = cursor_caller_state_upper_bound(
            self.changed_rows_state_bytes,
            self.retained_fence_state_bytes,
            self.sqlite_cache_bytes,
            self.sqlite_native_overhead_bytes,
        )?;
        if self.census_state_bytes < working_state_upper_bound(self.census.max_path_bytes)?
            || self.changed_rows_state_bytes
                < changed_rows_state_upper_bound(
                    self.max_changed_members,
                    self.census.max_path_bytes,
                )?
            || self.batch_builder_state_bytes
                < batch_builder_state_upper_bound(
                    self.max_changed_members,
                    self.census.max_path_bytes,
                )?
            || self.retained_fence_state_bytes < retained_fence_state_upper_bound()?
            || self.cursor_caller_state_bytes < minimum_cursor
        {
            return Err(invalid("source transition state partition is incomplete"));
        }
        let shared_aux_state = self
            .retained_fence_state_bytes
            .checked_add(self.sqlite_cache_bytes)
            .and_then(|bytes| bytes.checked_add(self.sqlite_native_overhead_bytes))
            .ok_or_else(|| invalid("source transition shared state overflow"))?;
        let proposal_scan_peak = self
            .census_state_bytes
            .checked_add(root_result_state_upper_bound()?)
            .and_then(|bytes| bytes.checked_add(shared_aux_state))
            .ok_or_else(|| invalid("source transition proposal state overflow"))?;
        // The caller-state envelope is also the complete merge peak passed to
        // the reader before it allocates a row. The census buffer is no longer
        // live during this phase.
        let merge_peak = self.cursor_caller_state_bytes;
        // The proposal map remains live while the canonical batch visitor
        // builds its bounded transient nodes; these phases overlap.
        let batch_peak = self
            .cursor_caller_state_bytes
            .checked_add(self.batch_builder_state_bytes)
            .ok_or_else(|| invalid("source transition batch state overflow"))?;
        let terminal_scan_peak = self
            .census_state_bytes
            .checked_add(shared_aux_state)
            // The caller transfers this map into SpoolCandidate before the
            // terminal census. Keep its bounded row storage charged until
            // the fence is dropped, even though this module no longer owns it.
            .and_then(|bytes| bytes.checked_add(self.changed_rows_state_bytes))
            .ok_or_else(|| invalid("source transition terminal state overflow"))?;
        // These are overlapping phase peaks, not independent grants: census
        // buffers are dropped before the merge, while the update map remains
        // live through batch construction. Taking the maximum preserves that
        // lifetime and still charges the complete cursor caller envelope.
        let required = proposal_scan_peak
            .max(merge_peak)
            .max(batch_peak)
            .max(terminal_scan_peak);
        if required > self.max_state_slice_bytes {
            return Err(invalid(
                "source transition exceeds the selected original state slice",
            ));
        }
        Ok(())
    }
}

fn changed_rows_state_upper_bound(count: usize, max_path_bytes: usize) -> io::Result<usize> {
    let per_row = max_path_bytes
        .checked_mul(2)
        .and_then(|n| n.checked_add(size_of::<String>() + size_of::<SourceUpdate>() + 512))
        .ok_or_else(|| invalid("source transition update-map row state overflow"))?;
    let root_result = root_result_state_upper_bound()?;
    per_row
        .checked_mul(count)
        .and_then(|n| n.checked_add(root_result))
        .and_then(|n| n.checked_add(max_path_bytes.checked_mul(4)?))
        .and_then(|n| {
            n.checked_add(size_of::<ScratchMember>() + size_of::<V2MemberTupleObservation>() + 8192)
        })
        .ok_or_else(|| invalid("source transition update-map state overflow"))
}

fn root_result_state_upper_bound() -> io::Result<usize> {
    SourceRevisionRootsV2::retained_state_upper_bound_for_value(
        SourceRevisionRootsV2::MAX_ENCODED_BYTES,
    )
}

fn batch_builder_state_upper_bound(count: usize, max_path_bytes: usize) -> io::Result<usize> {
    let per_row = max_path_bytes
        .checked_mul(16)
        .and_then(|n| n.checked_add(2048))
        .ok_or_else(|| invalid("source transition batch row state overflow"))?;
    per_row
        .checked_mul(count)
        .and_then(|n| n.checked_add(8192))
        .ok_or_else(|| invalid("source transition batch state overflow"))
}

fn cursor_caller_state_upper_bound(
    changed_rows_state_bytes: usize,
    retained_fence_state_bytes: usize,
    sqlite_cache_bytes: usize,
    sqlite_native_overhead_bytes: usize,
) -> io::Result<usize> {
    changed_rows_state_bytes
        .checked_add(retained_fence_state_bytes)
        .and_then(|bytes| bytes.checked_add(sqlite_cache_bytes))
        .and_then(|bytes| bytes.checked_add(sqlite_native_overhead_bytes))
        .ok_or_else(|| invalid("source transition cursor state overflow"))
}

fn retained_fence_state_upper_bound() -> io::Result<usize> {
    size_of::<SourceTransitionPrepared<'static>>()
        .checked_add(size_of::<SourceTransitionFence<'static>>())
        .and_then(|n| n.checked_add(AdmissionWorkBudget::retained_allocation_upper_bound_bytes()))
        .and_then(|n| n.checked_add(8192))
        .ok_or_else(|| invalid("source transition retained fence state overflow"))
}

fn minimum_work_units(census: SourceCensusLimits, max_changed_members: usize) -> io::Result<u64> {
    // The census owner charges at most entries + 4*directories + 4*members,
    // plus bounded schema/savepoint/query/EOF units, per scan. The merge adds
    // one SQL-row and one authenticated-cursor attempt per member plus EOF.
    let per_scan = census
        .max_entries
        .checked_add(
            census
                .max_directories
                .checked_mul(4)
                .ok_or_else(|| invalid("source transition directory work overflow"))?,
        )
        .and_then(|n| n.checked_add(census.max_files.checked_mul(4)?))
        .and_then(|n| n.checked_add(4 + CENSUS_CONTROL_SQL_WORK_UNITS))
        .ok_or_else(|| invalid("source transition census work overflow"))?;
    let batch_builder = u64::try_from(max_changed_members)
        .map_err(|_| invalid("source transition batch work exceeds the work range"))?
        .checked_mul(BATCH_BUILDER_PER_MEMBER_WORK_UNITS)
        .and_then(|n| n.checked_add(BATCH_BUILDER_CONTROL_WORK_UNITS))
        .ok_or_else(|| invalid("source transition batch work overflow"))?;
    per_scan
        .checked_mul(2)
        .and_then(|n| n.checked_add(census.max_files.checked_mul(2)?))
        .and_then(|n| n.checked_add(64))
        .and_then(|n| n.checked_add(batch_builder))
        .ok_or_else(|| invalid("source transition total work overflow"))
}

struct ScratchMember {
    path: String,
    sha256: Digest256,
    size: u64,
    mode: u32,
}

fn read_scratch_member(row: &Row<'_>) -> io::Result<ScratchMember> {
    let path = row
        .get::<_, String>(0)
        .map_err(|_| invalid("source transition census path row"))?;
    let digest = row
        .get::<_, Vec<u8>>(1)
        .map_err(|_| invalid("source transition census digest row"))?;
    let digest: [u8; 32] = digest
        .try_into()
        .map_err(|_| invalid("source transition census digest width"))?;
    let size = row
        .get::<_, i64>(2)
        .map_err(|_| invalid("source transition census size row"))?;
    let mode = row
        .get::<_, i64>(3)
        .map_err(|_| invalid("source transition census mode row"))?;
    Ok(ScratchMember {
        path,
        sha256: Digest256::from_bytes(digest),
        size: u64::try_from(size).map_err(|_| invalid("source transition size range"))?,
        mode: u32::try_from(mode).map_err(|_| invalid("source transition mode range"))?,
    })
}

fn insert_update(
    updates: &mut BTreeMap<String, SourceUpdate>,
    member: &ScratchMember,
    profile: SourceTransitionProfile,
) -> io::Result<()> {
    if updates.len() >= profile.max_changed_members {
        return Err(invalid("source transition changed-member slice exhausted"));
    }
    updates.insert(
        member.path.clone(),
        SourceUpdate {
            sha256: member.sha256,
            size_bytes: member.size,
            mode: member.mode,
        },
    );
    Ok(())
}

fn derive_updates(
    db: &PinnedSqliteConnection,
    summary: SourceCensusSummary,
    base: &mut V2ReadSession,
    original_base: SourceRevision,
    roots: &SourceRevisionRootsV2,
    profile: SourceTransitionProfile,
    work: &AdmissionWorkBudget,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<BTreeMap<String, SourceUpdate>> {
    if roots.revision != original_base
        || roots
            .membership_v1
            .is_some_and(|membership| roots.member_count != membership.count)
        || roots.member_count != roots.members.entries
        || roots.source_bytes > profile.admission.max_source_bytes
        || roots.member_count > profile.census.max_files
    {
        return Err(invalid("source transition V2 base root counts differ"));
    }
    work.charge_many(4)?;
    let mut statement = db
        .prepare(
            "SELECT path,sha256,size,mode FROM source_member_census \
             WHERE scan_label=?1 ORDER BY path COLLATE BINARY",
        )
        .map_err(|_| invalid("source transition census ordered query prepare"))?;
    let mut rows = statement
        .query([SourceCensusScan::Proposal.label()])
        .map_err(|_| invalid("source transition census ordered query"))?;

    let mut actual: Option<ScratchMember> = None;
    let mut actual_eof = false;
    let mut authenticated: Option<V2MemberTupleObservation> = None;
    let mut base_eof = false;
    let mut base_after: Option<String> = None;
    let mut updates = BTreeMap::new();
    let mut base_members = 0u64;
    let mut base_bytes = 0u64;
    let mut actual_members = 0u64;
    let mut actual_bytes = 0u64;

    loop {
        active(deadline, cancel)?;
        if actual.is_none() && !actual_eof {
            work.charge(())?;
            actual = rows
                .next()
                .map_err(|_| invalid("source transition census row read"))?
                .map(read_scratch_member)
                .transpose()?;
            actual_eof = actual.is_none();
            if let Some(row) = &actual {
                actual_members = actual_members
                    .checked_add(1)
                    .filter(|n| *n <= profile.census.max_files)
                    .ok_or_else(|| invalid("source transition actual member count overflow"))?;
                actual_bytes = actual_bytes
                    .checked_add(row.size)
                    .filter(|n| *n <= profile.census.max_source_bytes)
                    .ok_or_else(|| invalid("source transition actual source bytes overflow"))?;
            }
        }
        if authenticated.is_none() && !base_eof {
            work.charge(())?;
            let after = base_after.as_ref().map(|path| path.as_bytes());
            authenticated = base
                .next_member_tuple_after(original_base, after, profile.cursor_caller_state_bytes)
                .map_err(invalid)?;
            base_eof = authenticated.is_none();
            if let Some(row) = &authenticated {
                base_members = base_members
                    .checked_add(1)
                    .filter(|n| *n <= roots.member_count)
                    .ok_or_else(|| invalid("source transition base member count overflow"))?;
                base_bytes = base_bytes
                    .checked_add(row.size_bytes)
                    .filter(|n| *n <= roots.source_bytes)
                    .ok_or_else(|| invalid("source transition base bytes overflow"))?;
            }
        }

        match (actual.as_ref(), authenticated.as_ref()) {
            (None, None) => break,
            (Some(_), None) => {
                let row = actual.as_ref().expect("actual row present");
                insert_update(&mut updates, row, profile)?;
                actual = None;
            }
            (None, Some(_)) => {
                return Err(invalid(
                    "source transition deletion requires authenticated retirement evidence",
                ));
            }
            (Some(current), Some(original)) => match current
                .path
                .as_bytes()
                .cmp(original.path.as_str().as_bytes())
            {
                std::cmp::Ordering::Less => {
                    insert_update(&mut updates, current, profile)?;
                    actual = None;
                }
                std::cmp::Ordering::Greater => {
                    return Err(invalid(
                        "source transition deletion requires authenticated retirement evidence",
                    ));
                }
                std::cmp::Ordering::Equal => {
                    if current.sha256 != original.sha256
                        || current.size != original.size_bytes
                        || current.mode != original.source_mode
                    {
                        insert_update(&mut updates, current, profile)?;
                    }
                    let original = authenticated
                        .take()
                        .ok_or_else(|| invalid("source transition base cursor disappeared"))?;
                    base_after = Some(original.path.as_str().to_owned());
                    actual = None;
                }
            },
        }
    }
    drop(rows);
    drop(statement);
    if actual_members != summary.member_count
        || actual_bytes != summary.source_bytes
        || base_members != roots.member_count
        || base_bytes != roots.source_bytes
        || updates.is_empty()
        || updates.len() > profile.admission.max_members
    {
        return Err(invalid(
            "source transition census/base closure totals differ",
        ));
    }
    Ok(updates)
}

pub(crate) struct SourceTransitionPrepared<'a> {
    batch: Option<AdmissionBatch>,
    fence: SourceTransitionFence<'a>,
}

impl SourceTransitionPrepared<'_> {
    pub(crate) fn take_batch(&mut self) -> io::Result<AdmissionBatch> {
        self.batch
            .take()
            .ok_or_else(|| invalid("source transition batch already consumed"))
    }

    pub(crate) fn retained_state_upper_bound_bytes(&self) -> usize {
        self.fence.profile.max_state_slice_bytes
    }

    pub(crate) fn work_units_used(&self) -> u64 {
        self.fence.work.used()
    }

    pub(crate) fn proposal_census(&self) -> SourceCensusSummary {
        self.fence.proposal
    }

    /// Keep the exact current selector captured by the held V2 session pinned
    /// across the accepted-publication lookup. This can be a later current
    /// revision than the historical base used to derive the canonical batch.
    pub(crate) fn verify_lookup_fence(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        if self.batch.is_some() {
            return Err(invalid(
                "source transition lookup fence follows exact batch derivation",
            ));
        }
        self.fence.verify_lookup_fence(base)
    }

    /// After exact accepted-publication lookup misses, allow a new candidate
    /// only when the current selector is still the original base. This never
    /// promotes a newer current revision into a replacement batch base.
    pub(crate) fn verify_original_base_after_lookup_miss(
        &mut self,
        base: &mut V2ReadSession,
    ) -> io::Result<()> {
        if self.batch.is_some() {
            return Err(invalid(
                "source transition miss fence follows exact batch derivation",
            ));
        }
        self.fence.verify_original_base_after_lookup_miss(base)
    }

    /// Re-census the complete selected tree and recheck the original V2
    /// current fence after Native validation, while this prepared guard and
    /// its original AUX/IO/work slices remain alive.
    pub(crate) fn verify_before_publish(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        if self.batch.is_some() {
            return Err(invalid(
                "source transition batch must enter Native validation before its terminal fence",
            ));
        }
        self.fence.verify_before_publish(base)
    }
}

struct SourceTransitionFence<'a> {
    root: &'a File,
    original_base: SourceRevision,
    selected_current: CorpusCurrentSelection,
    proposal: SourceCensusSummary,
    profile: SourceTransitionProfile,
    _scope: PinnedSqliteAuxScope,
    db: PinnedSqliteConnection,
    io: PinnedSqliteIoBudget,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    work: AdmissionWorkBudget,
}

impl SourceTransitionFence<'_> {
    fn verify_lookup_fence(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        active(self.deadline, &self.cancel)?;
        if base.selected_selection() != self.selected_current {
            return Err(invalid(
                "source transition selected current differs from its lookup fence",
            ));
        }
        base.verify_current_fence().map_err(invalid)
    }

    fn verify_original_base_after_lookup_miss(
        &mut self,
        base: &mut V2ReadSession,
    ) -> io::Result<()> {
        self.verify_lookup_fence(base)?;
        if self.selected_current.revision != self.original_base {
            return Err(invalid(
                "source transition accepted lookup missed for a stale original base",
            ));
        }
        Ok(())
    }

    fn verify_before_publish(&mut self, base: &mut V2ReadSession) -> io::Result<()> {
        active(self.deadline, &self.cancel)?;
        if base.selected_selection() != self.selected_current
            || self.selected_current.revision != self.original_base
        {
            return Err(invalid(
                "source transition original V2 base is no longer current",
            ));
        }
        let mut callback = |kind| self.work.charge(kind);
        let terminal = census_selected_to_scratch(
            self.root,
            &self.db,
            &self.io,
            SourceCensusScan::Terminal,
            self.profile.census,
            &mut callback,
            self.deadline,
            &self.cancel,
        )?;
        if terminal != self.proposal {
            return Err(invalid("source transition terminal census changed"));
        }
        base.verify_current_fence().map_err(invalid)
    }
}

#[allow(clippy::too_many_arguments)]
/// Prepare one proposal from the supplied exact original V2 revision, which
/// may now be historical while a previously accepted batch is being looked
/// up. A lookup miss must call `verify_original_base_after_lookup_miss` before
/// candidate ingestion. Before calling, the owner must debit
/// `profile.max_state_slice_bytes` on the same Native spooled ledger
/// represented by `aux`; this method verifies that the spool and V2 IO
/// handles are the invocation's paired selections, but it does not reserve a
/// second state allowance.
pub(crate) fn prepare_source_transition<'a>(
    invocation: &NativeSourceValidator<'_>,
    base: &mut V2ReadSession,
    original_base: SourceRevision,
    root: &'a File,
    profile: SourceTransitionProfile,
    workspace: File,
    aux: PinnedSqliteAuxRequest,
    validator_sha256: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<SourceTransitionPrepared<'a>> {
    profile.check_state_partition()?;
    active(deadline, cancel)?;
    if aux.deadline != deadline || !std::ptr::eq(Arc::as_ptr(&aux.cancelled), cancel) {
        return Err(invalid("source transition original caller binding differs"));
    }
    invocation.verify_spooled_v2_io(&aux.io_budget, base.io_budget(), deadline, cancel)?;
    let selected_current = base.selected_selection();
    let retained_cancel = Arc::clone(&aux.cancelled);
    let spool_io = aux.io_budget.clone();
    let work = AdmissionWorkBudget::new(profile.max_work_units)?;
    work.charge_many(8)?;
    base.verify_current_fence().map_err(invalid)?;
    let roots = base
        .roots_for_revision(original_base)
        .map_err(invalid)?
        .ok_or_else(|| invalid("source transition exact original V2 base absent"))?;
    if roots.member_count > profile.census.max_files
        || roots.source_bytes > profile.census.max_source_bytes
    {
        return Err(invalid(
            "source transition base exceeds the selected census profile",
        ));
    }
    let mut scope = PinnedSqliteAuxScope::new(workspace, aux)
        .map_err(|_| invalid("source transition held census scratch selection refused"))?;
    let db = scope
        .open_connection()
        .map_err(|_| invalid("source transition held census scratch connection refused"))?;
    configure_db(&db, profile.sqlite_cache_bytes)?;
    let progress_deadline = deadline;
    let progress_cancel = Arc::clone(&retained_cancel);
    db.progress_handler(
        1000,
        Some(move || active(progress_deadline, &progress_cancel).is_err()),
    );
    let mut callback = |kind| work.charge(kind);
    let proposal = census_selected_to_scratch(
        root,
        &db,
        &spool_io,
        SourceCensusScan::Proposal,
        profile.census,
        &mut callback,
        deadline,
        cancel,
    )?;
    drop(callback);
    let updates = derive_updates(
        &db,
        proposal,
        base,
        original_base,
        &roots,
        profile,
        &work,
        deadline,
        cancel,
    )?;
    drop(roots);
    base.verify_current_fence().map_err(invalid)?;
    // Charge the full selected changed-member ceiling before the maintained
    // batch builder walks the update map, materializes canonical rows, and
    // visits them for the digest. Failed construction retains this spent
    // prefix; the work budget is not reset between phases.
    let batch_builder_work = u64::try_from(profile.max_changed_members)
        .map_err(|_| invalid("source transition batch work exceeds the work range"))?
        .checked_mul(BATCH_BUILDER_PER_MEMBER_WORK_UNITS)
        .and_then(|n| n.checked_add(BATCH_BUILDER_CONTROL_WORK_UNITS))
        .ok_or_else(|| invalid("source transition batch work overflow"))?;
    work.charge_many(batch_builder_work)?;
    let batch = AdmissionBatch::from_verified_rows(
        Some(original_base.0),
        validator_sha256,
        updates,
        root.try_clone()
            .map_err(|_| invalid("source transition held root clone refused"))?,
        profile.admission,
        profile.batch_builder_state_bytes,
        &spool_io,
        deadline,
        cancel,
    )?;
    Ok(SourceTransitionPrepared {
        batch: Some(batch),
        fence: SourceTransitionFence {
            root,
            original_base,
            selected_current,
            proposal,
            profile,
            _scope: scope,
            db,
            io: spool_io,
            deadline,
            cancel: retained_cancel,
            work,
        },
    })
}
