//! CMD-owned construction and publication preparation for immutable V2 source roots.
//!
//! These roots are derived from a completed native candidate. Their logical
//! membership and physical tree commitments remain separate from the V1
//! manifest digest and from NativeAdmissionComplete.
use super::source_admission::{AdmissionWorkBudget, active};
use std::cell::{Cell, RefCell};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;
use std::{io, mem::size_of};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_segment_store::{
    AuthenticatedTreeDeltaV1, AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1,
    AuthenticatedTreeIoLedgerV1, AuthenticatedTreeLimitsV1, AuthenticatedTreeWorkV1, SegmentLimits,
    SegmentStore,
};
use tos_source_store::native_v2::objects::{
    MAX_PACKED_OBJECT_FRAMES_V2, PackedObjectLimitsV2, PackedObjectWriterV2,
};
use tos_source_store::{
    CorpusCurrentSelection, CorpusPointerFormat, SourceMembershipV1, SourceMembershipV2,
    StreamedCorpusCutReaderV1, StreamedRevisionV1,
};

use tos_source_store::native_v2::roots::*;

/// IO and incremental persistent-allocation adapter for one completed native
/// V2 writer. The complete reservation is selected before this object can
/// touch the physical store; each new pack is additionally precharged by the
/// SegmentStore install path before it is staged.
pub(crate) struct NativeV2TreeIo {
    io: tos_source_store::PinnedSqliteIoBudget,
    custody: Arc<tos_source_store::PinnedSqliteSpaceReservation>,
    max_allocated_bytes: u64,
    allocation_unit_bytes: u64,
    max_working_state_bytes: usize,
    reserved: AtomicU64,
    actual: AtomicU64,
}

impl NativeV2TreeIo {
    pub(crate) fn new(
        io: tos_source_store::PinnedSqliteIoBudget,
        custody: Arc<tos_source_store::PinnedSqliteSpaceReservation>,
        max_allocated_bytes: u64,
        allocation_unit_bytes: u64,
        max_working_state_bytes: usize,
    ) -> io::Result<Arc<Self>> {
        if max_allocated_bytes == 0
            || max_allocated_bytes == u64::MAX
            || allocation_unit_bytes == 0
            || allocation_unit_bytes == u64::MAX
            || max_working_state_bytes == 0
            || max_working_state_bytes == usize::MAX
        {
            return Err(invalid("V2 allocation accountant profile is invalid"));
        }
        Ok(Arc::new(Self {
            io,
            custody,
            max_allocated_bytes,
            allocation_unit_bytes,
            max_working_state_bytes,
            reserved: AtomicU64::new(0),
            actual: AtomicU64::new(0),
        }))
    }

    pub(crate) fn from_budget(
        budget: &super::source_foundation_admission::NativeSegmentV2Budget,
    ) -> Arc<Self> {
        Arc::clone(&budget.allocation_accountant)
    }

    pub(crate) fn reserve_file_allocation(&self, bytes: u64) -> io::Result<u64> {
        let upper = allocation_upper_bound(bytes, self.allocation_unit_bytes)?;
        if !self.reserve_allocated_bytes(upper) {
            return Err(invalid("V2 persistent allocation precharge refused"));
        }
        Ok(upper)
    }

    pub(crate) fn reconcile_file_allocation(&self, reserved: u64, actual: u64) -> io::Result<()> {
        if !self.reconcile_allocated_bytes(reserved, actual) {
            return Err(invalid("V2 persistent allocation reconciliation refused"));
        }
        Ok(())
    }

    pub(crate) fn release_file_allocation(&self, reserved: u64) -> io::Result<()> {
        let result = self
            .reserved
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(reserved)
            });
        result
            .map(|_| ())
            .map_err(|_| invalid("V2 persistent allocation precharge regressed"))
    }

    /// Release an exactly removed temporary inode after its earlier physical
    /// reconciliation. Both the outstanding precharge and observed allocation
    /// are reversed; ambiguous callers must keep the charge instead.
    pub(crate) fn release_temporary_file_allocation(
        &self,
        reserved: u64,
        actual_removed: u64,
    ) -> io::Result<()> {
        let previous_actual = self
            .actual
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(actual_removed)
            })
            .map_err(|_| invalid("V2 temporary allocation actual regressed"))?;
        let next_actual = previous_actual - actual_removed;
        if let Err(error) = self.custody.update_actual_allocated(next_actual) {
            let _ = self
                .actual
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_add(actual_removed)
                });
            let _ = self.custody.update_actual_allocated(previous_actual);
            return Err(io::Error::new(io::ErrorKind::InvalidData, error));
        }
        if self
            .reserved
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(reserved)
            })
            .is_err()
        {
            let restored = self
                .actual
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_add(actual_removed)
                })
                .is_ok();
            if restored {
                let _ = self.custody.update_actual_allocated(previous_actual);
            }
            return Err(invalid("V2 temporary allocation precharge regressed"));
        }
        Ok(())
    }

    pub(crate) fn selected_allocation_unit_bytes(&self) -> u64 {
        self.allocation_unit_bytes
    }

    pub(crate) fn max_working_state_bytes(&self) -> usize {
        self.max_working_state_bytes
    }

    pub(crate) fn actual_allocated_bytes(&self) -> u64 {
        self.actual.load(Ordering::Acquire)
    }

    pub(crate) fn io_budget(&self) -> &tos_source_store::PinnedSqliteIoBudget {
        &self.io
    }

    pub(crate) fn custody_reservation(
        &self,
    ) -> Arc<tos_source_store::PinnedSqliteSpaceReservation> {
        Arc::clone(&self.custody)
    }
}

impl AuthenticatedTreeIoLedgerV1 for NativeV2TreeIo {
    fn charge_read(&self, bytes: u64) -> bool {
        self.io.charge_read(bytes).is_ok()
    }
    fn charge_read_upper_bound(&self, bytes: u64) -> bool {
        self.io.charge_read_upper_bound(bytes).is_ok()
    }
    fn record_read_returned(&self, bytes: u64) -> bool {
        self.io.record_read_returned(bytes).is_ok()
    }
    fn charge_write(&self, bytes: u64) -> bool {
        self.io.charge_write(bytes).is_ok()
    }
    fn record_write_returned(&self, bytes: u64) -> bool {
        self.io.record_write_returned(bytes).is_ok()
    }
    fn reserve_allocated_bytes(&self, bytes: u64) -> bool {
        let Ok(next) = self
            .reserved
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(bytes)
                    .filter(|sum| *sum <= self.max_allocated_bytes)
            })
        else {
            return false;
        };
        let _ = next;
        true
    }
    fn reconcile_allocated_bytes(&self, reserved: u64, actual: u64) -> bool {
        let Ok(previous) =
            self.actual
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_add(actual)
                })
        else {
            return false;
        };
        let next = previous + actual;
        let within_file_precharge = actual <= reserved;
        let within_total_cap = next <= self.max_allocated_bytes;
        self.custody.update_actual_allocated(next).is_ok()
            && within_file_precharge
            && within_total_cap
    }
    fn allocation_unit_bytes(&self) -> u64 {
        self.allocation_unit_bytes
    }
}

fn allocation_upper_bound(bytes: u64, unit: u64) -> io::Result<u64> {
    if unit == 0 || unit == u64::MAX {
        return Err(invalid("V2 allocation quantum is invalid"));
    }
    bytes
        .checked_add(unit - 1)
        .and_then(|n| n.checked_div(unit))
        .and_then(|n| n.checked_mul(unit))
        .and_then(|n| n.checked_add(unit))
        .ok_or_else(|| invalid("V2 allocation precharge overflow"))
}

fn writer_context_state_bytes(
    candidate: &super::source_admission_spooled_candidate::SpoolCandidate<'_>,
    index: &super::source_admission_spooled_index::IndexView<'_>,
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
    rootset_state: usize,
) -> io::Result<usize> {
    let (base_rust, base_cache) = candidate.borrowed_base_declared_retained_state_bytes()?;
    let candidate_state = candidate.own_retained_state_upper_bound_bytes()?;
    let index_state = index.declared_retained_state_bytes()?;
    let index_row_workspace = index.writer_row_state_limit();
    let cursor_paths = profile
        .tree_limits
        .max_key_bytes
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(3 * size_of::<RelativePath>()))
        .ok_or_else(|| invalid("V2 writer cursor state overflow"))?;
    candidate_state
        .checked_add(base_rust)
        .and_then(|bytes| bytes.checked_add(base_cache))
        .and_then(|bytes| bytes.checked_add(index_state))
        .and_then(|bytes| bytes.checked_add(index_row_workspace))
        .and_then(|bytes| bytes.checked_add(rootset_state))
        .and_then(|bytes| bytes.checked_add(size_of::<NativeV2TreeIo>()))
        .and_then(|bytes| bytes.checked_add(size_of::<AuthenticatedTreeWorkV1>()))
        .and_then(|bytes| bytes.checked_add(2 * size_of::<usize>()))
        .and_then(|bytes| bytes.checked_add(profile.max_working_state_bytes / 8))
        // At most three path cursors coexist in the dependency walk. The
        // source-row allowance above covers the bounded SQL result workspace.
        .and_then(|bytes| bytes.checked_add(65_536))
        .and_then(|bytes| bytes.checked_add(cursor_paths))
        .ok_or_else(|| invalid("V2 writer retained context state overflow"))
}

fn selected_segment_limits(
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
) -> io::Result<SegmentLimits> {
    Ok(SegmentLimits {
        max_segment_bytes: profile.max_allocated_bytes,
        max_frame_bytes: profile.max_frame_bytes,
        max_frames: u32::try_from(profile.tree_limits.max_nodes.min(u32::MAX as u64))
            .map_err(|_| invalid("V2 segment frame limit exceeds range"))?
            .max(1),
        max_journal_bytes: profile
            .max_working_state_bytes
            .min(4 * 1024 * 1024)
            .max(128),
    })
}

fn packed_object_limits(
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
    segment: &SegmentStore,
    used: AuthenticatedTreeWorkV1,
    rows_used: u64,
    max_objects: u64,
    caller_live_state_bytes: usize,
    shared_work: &AdmissionWorkBudget,
    cumulative_tree_work: bool,
) -> io::Result<PackedObjectLimitsV2> {
    if caller_live_state_bytes >= profile.max_working_state_bytes {
        return Err(invalid(
            "packed object caller state exceeds selected profile",
        ));
    }
    let mut tree_limits = if cumulative_tree_work {
        profile.tree_limits
    } else {
        remaining_tree_limits(profile.tree_limits, used)?
    };
    let rows_remaining = profile
        .tree_limits
        .max_rows
        .checked_sub(rows_used)
        .ok_or_else(|| invalid("packed object cumulative row profile regressed"))?;
    if !cumulative_tree_work {
        // The tree API requires a positive finite row cap even for an empty
        // root; `max_objects` and the source iterator still prevent any row.
        tree_limits.max_rows = rows_remaining.max(1);
    }
    let max_work_units = shared_work.remaining()?;
    if max_work_units == 0 {
        return Err(invalid("packed object shared work ceiling exhausted"));
    }
    Ok(PackedObjectLimitsV2 {
        segment_limits: selected_segment_limits(profile)?,
        tree_limits,
        max_working_state_bytes: profile.max_working_state_bytes,
        caller_live_state_bytes,
        max_work_units,
        max_objects,
        max_delta_rows: rows_remaining,
        max_pack_frames: selected_segment_limits(profile)?
            .max_frames
            .min(MAX_PACKED_OBJECT_FRAMES_V2),
    })
}

pub(crate) struct BuiltInitialRootSetV2 {
    pub(crate) roots: SourceRootSetV2,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha256: Digest256,
    pub(crate) tree_io: Arc<NativeV2TreeIo>,
    pub(crate) segment_store: SegmentStore,
    pub(crate) work: AuthenticatedTreeWorkV1,
}

pub(crate) struct BuiltSuccessorRootSetV2 {
    pub(crate) expected_base: SourceRevision,
    pub(crate) expected_previous_rootset_sha256: Option<Digest256>,
    pub(crate) expected_selection: CorpusCurrentSelection,
    pub(crate) roots: SourceRootSetV2,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha256: Digest256,
    pub(crate) source_record: Vec<u8>,
    pub(crate) tree_io: Arc<NativeV2TreeIo>,
    pub(crate) segment_store: SegmentStore,
    pub(crate) work: AuthenticatedTreeWorkV1,
}

/// Build the first V2 rootset from an empty store using the real validated
/// candidate and maintained native index cursors. Existing V2 bases go through
/// the successor COW route; an existing V1 base goes through the explicit
/// full-history migration route.
pub(crate) fn build_initial_rootset_v2(
    store: &super::source_admission_store::AdmissionStore,
    candidate: &super::source_admission_spooled_candidate::SpoolCandidate<'_>,
    index: &super::source_admission_spooled_index::IndexView<'_>,
    revision: SourceRevision,
    manifest_sha256: Digest256,
    manifest_bytes: u64,
    batch_sha256: Digest256,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<BuiltInitialRootSetV2> {
    if candidate.base_revision().is_some() {
        return Err(invalid("V2 initial root builder requires an empty base"));
    }
    if store.has_v2_segments()? {
        return Err(invalid(
            "initial V2 writer refuses an existing or interrupted segment namespace",
        ));
    }
    let profile = index
        .segment_v2_budget()
        .ok_or_else(|| invalid("V2 root builder lacks the native completion profile"))?;
    let shared_work = candidate.admission_work_budget()?;
    index.verify_candidate()?;
    let tree_io = NativeV2TreeIo::from_budget(profile);
    if !store.has_v2_allocation_accountant(&tree_io) {
        return Err(invalid(
            "V2 writer and object-ingest allocation account are not shared",
        ));
    }
    store.retain_v2_store_custody(tree_io.custody_reservation());
    let segment_bytes = profile.max_allocated_bytes;
    if segment_bytes < 65_536 {
        return Err(invalid(
            "V2 persistent store profile is below metadata floor",
        ));
    }
    let segment_limits = selected_segment_limits(profile)?;
    let tree_ledger: Arc<dyn AuthenticatedTreeIoLedgerV1> = tree_io.clone();
    let segment = store.segment_store_v2_with_io(
        SOURCE_ADMISSION_V2_DOMAIN,
        segment_limits,
        tree_ledger,
        deadline,
        cancelled,
    )?;
    if segment.custody_domain() != SOURCE_ADMISSION_V2_DOMAIN {
        return Err(invalid("V2 segment store domain differs"));
    }
    let segment_live_state = segment_store_retained_state_bytes(&segment)?;
    let base_limits = profile.tree_limits;
    let mut used = AuthenticatedTreeWorkV1::default();
    let mut used_rows = 0u64;
    let writer_live_state = writer_context_state_bytes(
        candidate,
        index,
        profile,
        size_of::<BuiltInitialRootSetV2>(),
    )?;
    if writer_live_state > profile.max_working_state_bytes {
        return Err(invalid(
            "V2 initial writer context exceeds selected state profile",
        ));
    }

    let member_rows = {
        let mut after: Option<RelativePath> = None;
        let candidate = candidate;
        std::iter::from_fn(move || {
            match candidate
                .member_after_bounded(after.as_ref(), profile.max_working_state_bytes / 8)
            {
                Ok(Some(member)) => {
                    after = Some(member.path.clone());
                    let mut value = Vec::with_capacity(44);
                    value.extend_from_slice(member.sha256.as_bytes());
                    value.extend_from_slice(&member.size_bytes.to_be_bytes());
                    value.extend_from_slice(&member.mode.to_le_bytes());
                    Some(Ok(AuthenticatedTreeEntryV1 {
                        key: member.path.as_str().as_bytes().to_vec(),
                        value,
                    }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let members = build_full_tree_v2(
        &segment,
        MEMBERS_KIND,
        member_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state,
        &shared_work,
        deadline,
        cancelled,
    )?;
    let mut prior_roots_state = tree_retained_state_bytes(&members)?;

    let identity_rows = {
        let mut after: Option<String> = None;
        let index = index;
        std::iter::from_fn(move || match index.identities_after(after.as_deref()) {
            Ok(Some((id, path))) => {
                after = Some(id.clone());
                Some(Ok(AuthenticatedTreeEntryV1 {
                    key: id.into_bytes(),
                    value: path.as_str().as_bytes().to_vec(),
                }))
            }
            Ok(None) => None,
            Err(error) => Some(Err(tree_io_error(error))),
        })
    };
    let identities = build_full_tree_v2(
        &segment,
        IDENTITIES_KIND,
        identity_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(prior_roots_state)
            .ok_or_else(|| invalid("V2 identity roots state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    prior_roots_state = prior_roots_state
        .checked_add(tree_retained_state_bytes(&identities)?)
        .ok_or_else(|| invalid("V2 identity roots state overflow"))?;

    let identity_paths = build_index_identity_paths_v2(
        candidate,
        index,
        &segment,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(prior_roots_state)
            .ok_or_else(|| invalid("V2 reverse identity roots state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    prior_roots_state = prior_roots_state
        .checked_add(tree_retained_state_bytes(&identity_paths)?)
        .ok_or_else(|| invalid("V2 reverse identity roots state overflow"))?;

    let dependency_rows = {
        use super::source_admission_index::NativeDependencyDirectionV1::Forward;
        let mut after: Option<(RelativePath, RelativePath)> = None;
        let index = index;
        std::iter::from_fn(move || {
            match index.dependency_pair_after(Forward, after.as_ref().map(|(s, t)| (s, t))) {
                Ok(Some((source, target))) => {
                    after = Some((source.clone(), target.clone()));
                    let pair = match length_prefixed_pair(&source, &target) {
                        Ok(pair) => pair,
                        Err(error) => return Some(Err(tree_io_error(error))),
                    };
                    let key_len = source
                        .as_str()
                        .len()
                        .checked_add(1)
                        .and_then(|n| n.checked_add(target.as_str().len()));
                    let Some(key_len) = key_len else {
                        return Some(Err(tree_error("V2 dependency key length overflow")));
                    };
                    if key_len > profile.tree_limits.max_key_bytes {
                        return Some(Err(tree_error("V2 dependency key exceeds profile")));
                    }
                    // RelativePath excludes NUL, so this delimiter produces
                    // a unique bytewise source/target ordering and permits
                    // source-prefix seeks in later warm delta readers.
                    let mut key = Vec::new();
                    if key.try_reserve_exact(key_len).is_err() {
                        return Some(Err(tree_error("V2 dependency key allocation failed")));
                    }
                    key.extend_from_slice(source.as_str().as_bytes());
                    key.push(0);
                    key.extend_from_slice(target.as_str().as_bytes());
                    Some(Ok(AuthenticatedTreeEntryV1 { key, value: pair }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let dependencies = build_full_tree_v2(
        &segment,
        DEPENDENCIES_KIND,
        dependency_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(prior_roots_state)
            .ok_or_else(|| invalid("V2 dependency roots state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    prior_roots_state = prior_roots_state
        .checked_add(tree_retained_state_bytes(&dependencies)?)
        .ok_or_else(|| invalid("V2 dependency roots state overflow"))?;

    let retirement_rows = {
        let mut ordinal = 0u64;
        let count = candidate.retirement_count();
        let candidate = candidate;
        std::iter::from_fn(move || {
            if ordinal >= count {
                return None;
            }
            let row = candidate.retirement_at_bounded(ordinal, profile.max_working_state_bytes / 8);
            ordinal += 1;
            match row {
                Ok(Some(row)) => Some(encode_retirement_entry(ordinal - 1, row)),
                Ok(None) => Some(Err(tree_error("V2 retirement ordinal ended early"))),
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let retirements = build_full_tree_v2(
        &segment,
        RETIREMENTS_KIND,
        retirement_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(prior_roots_state)
            .ok_or_else(|| invalid("V2 retirement roots state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    let retirement_root_state = tree_retained_state_bytes(&retirements)?;
    let object_live_state = writer_live_state
        .checked_add(prior_roots_state)
        .and_then(|bytes| bytes.checked_add(retirement_root_state))
        .ok_or_else(|| invalid("V2 object root live-state overflow"))?;
    let object_limits = packed_object_limits(
        profile,
        &segment,
        used,
        used_rows,
        candidate.desired_object_count()?,
        object_live_state,
        &shared_work,
        false,
    )?;
    let object_work_budget = shared_work.clone();
    let mut object_debit_work = move || object_work_budget.charge_many(1).is_ok();
    let (objects, object_work) = PackedObjectWriterV2::build(
        &segment,
        candidate.packed_object_sources(),
        object_limits,
        tree_io.clone(),
        deadline,
        cancelled,
        &mut object_debit_work,
    )?;
    add_tree_work(&mut used, object_work.tree_work, base_limits)?;
    used_rows = used_rows
        .checked_add(objects.entries)
        .filter(|rows| *rows <= base_limits.max_rows)
        .ok_or_else(|| invalid("V2 packed object cumulative row profile exceeded"))?;

    let fence = index.fence();
    let (member_count, source_bytes) = candidate.membership_counts();
    if member_count != fence.membership.count || source_bytes != fence.source_bytes {
        return Err(invalid(
            "V2 member binding differs from completed candidate",
        ));
    }
    let current = SourceRevisionRootsV2 {
        revision,
        base_revision: None,
        validator_sha256: fence.validator_sha256,
        manifest_sha256,
        source_artifact: SourceRevisionArtifactV2::SnapshotV1 {
            sha256: manifest_sha256,
            bytes: manifest_bytes,
        },
        batch_sha256: Some(batch_sha256),
        membership_v1: Some(fence.membership),
        membership_v2: Some(SourceMembershipV2::from_members_tree(
            member_count,
            source_bytes,
            members.commitment,
        )),
        completion_proof: Some(index.completion_proof()?),
        source_bytes,
        member_count,
        identity_count: index.identity_count(),
        dependency_source_count: index.dependency_source_count(),
        dependency_count: index.dependency_count(),
        retirement_count: candidate.retirement_count(),
        members,
        identities,
        identity_paths: Some(identity_paths),
        dependencies,
        retirements,
        objects: Some(objects),
    };
    let current_retained = current.retained_state_bytes()?;
    let current_validate_state = writer_live_state
        .checked_add(segment_live_state)
        .and_then(|bytes| bytes.checked_add(current_retained))
        .ok_or_else(|| invalid("V2 initial current validation state overflow"))?;
    if current_validate_state > profile.max_working_state_bytes {
        return Err(invalid(
            "V2 initial current validation exceeds state profile",
        ));
    }
    current.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let current_row = current.encode_with_state_limit(
        profile.max_working_state_bytes,
        writer_live_state
            .checked_add(segment_live_state)
            .ok_or_else(|| invalid("V2 initial current-row state overflow"))?,
    )?;
    let current_row_len = current_row.len();
    let current_row_capacity = current_row.capacity();
    let history_entry = AuthenticatedTreeEntryV1 {
        key: revision.0.as_bytes().to_vec(),
        value: current_row,
    };
    let history_entry_state = size_of::<AuthenticatedTreeEntryV1>()
        .checked_add(history_entry.key.capacity())
        .and_then(|bytes| bytes.checked_add(history_entry.value.capacity()))
        .ok_or_else(|| invalid("V2 initial history entry state overflow"))?;
    let history_rows = [Ok(history_entry)];
    let history = build_full_tree_v2(
        &segment,
        HISTORY_KIND,
        history_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(current_retained)
            .and_then(|bytes| bytes.checked_add(history_entry_state))
            .ok_or_else(|| invalid("V2 initial history live state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    let roots = SourceRootSetV2 { current, history };
    let roots_retained_state = roots.retained_state_bytes()?;
    let roots_validate_state = writer_live_state
        .checked_add(segment_live_state)
        .and_then(|bytes| bytes.checked_add(roots_retained_state))
        .ok_or_else(|| invalid("V2 initial root validation state overflow"))?;
    if roots_validate_state > profile.max_working_state_bytes {
        return Err(invalid("V2 initial root validation exceeds state profile"));
    }
    roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let decode_workspace = decode_workspace_upper_bound(current_row_len)?;
    if decode_workspace > profile.max_working_state_bytes {
        return Err(invalid("V2 current history decode exceeds state profile"));
    }
    let history_row_result_state =
        SourceRevisionRootsV2::retained_state_upper_bound_for_value(ROOTSET_MAX_BYTES)?;
    let history_lookup_live_state = writer_live_state
        .checked_add(segment_live_state)
        .and_then(|bytes| bytes.checked_add(roots_retained_state))
        .and_then(|bytes| bytes.checked_add(current_row_capacity))
        .and_then(|bytes| bytes.checked_add(decode_workspace))
        .and_then(|bytes| bytes.checked_add(history_row_result_state))
        .ok_or_else(|| invalid("V2 initial history lookup state overflow"))?;
    if history_lookup_live_state > profile.max_working_state_bytes {
        return Err(invalid("V2 initial history lookup exceeds state profile"));
    }
    let work_budget = shared_work.clone();
    let mut debit_work = move || work_budget.charge_many(1).is_ok();
    let (current_row, history_read_work) = segment
        .lookup_authenticated_tree_v2_with_work_and_io_and_callback(
            &roots.history,
            revision.0.as_bytes(),
            remaining_tree_limits(base_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
            &mut debit_work,
        )
        .map_err(segment_io_error)?;
    add_tree_work(&mut used, history_read_work, base_limits)?;
    let current_row = current_row.ok_or_else(|| invalid("V2 current history row is absent"))?;
    if current_row.len() != current_row_len {
        return Err(invalid("V2 current history row length changed"));
    }
    roots.verify_current_history_row(revision.0.as_bytes(), &current_row, decode_workspace)?;
    drop(current_row);
    index.verify_candidate()?;
    let root_encode_live_state = writer_live_state
        .checked_add(segment_live_state)
        .ok_or_else(|| invalid("V2 initial rootset state overflow"))?;
    let bytes =
        roots.encode_with_state_limit(profile.max_working_state_bytes, root_encode_live_state)?;
    let sha256 = Digest256::of_bytes(&bytes);
    Ok(BuiltInitialRootSetV2 {
        roots,
        bytes,
        sha256,
        tree_io,
        segment_store: segment,
        work: used,
    })
}

/// Full, finite bridge from one exact streamed V1 revision into the V2 typed
/// history format. Every logical family is walked to EOF and rebuilt in the
/// same physical tree store; the actual retained `snapshot.json` digest is
/// retained as LegacyManifestV1 evidence.
fn build_legacy_revision_roots_v2(
    store: &super::source_admission_store::AdmissionStore,
    candidate: &super::source_admission_spooled_candidate::SpoolCandidate<'_>,
    reader: &StreamedCorpusCutReaderV1,
    metadata: StreamedRevisionV1,
    segment: &SegmentStore,
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
    tree_io: &Arc<NativeV2TreeIo>,
    used: &mut AuthenticatedTreeWorkV1,
    used_rows: &mut u64,
    shared_work: &AdmissionWorkBudget,
    writer_live_state: usize,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<SourceRevisionRootsV2> {
    let max_source_bytes = candidate.migration_source_byte_limit()?;
    let max_member_bytes = candidate.migration_member_byte_limit()?;
    if metadata.member_count > candidate.migration_member_count_limit()?
        || metadata.member_count != metadata.membership.count
        || metadata.identity_count > candidate.migration_history_identity_limit()?
        || metadata.dependency_source_count > metadata.dependency_count
    {
        return Err(invalid(
            "legacy V1 revision counts exceed migration profile",
        ));
    }
    let artifact_sha256 = store.legacy_snapshot_sha256(
        metadata.revision.0,
        candidate.v1_manifest_max_bytes()?,
        deadline,
        cancelled,
        tree_io.io_budget(),
    )?;
    let limits = profile.tree_limits;

    let observed_members = Cell::new(0u64);
    let observed_source_bytes = Cell::new(0u64);
    let membership_hash = RefCell::new(Digest256Hasher::new());
    membership_hash
        .borrow_mut()
        .update(b"tos-val-full-membership-v1\0");
    let observed_members_ref = &observed_members;
    let observed_source_bytes_ref = &observed_source_bytes;
    let membership_hash_ref = &membership_hash;
    let mut retained_families_state = 0usize;
    let members = {
        let mut after: Option<RelativePath> = None;
        let rows = std::iter::from_fn(move || {
            match reader.member_after(metadata.revision, after.as_ref()) {
                Ok(Some(member)) => {
                    let next_count = match observed_members_ref
                        .get()
                        .checked_add(1)
                        .filter(|count| *count <= metadata.member_count)
                    {
                        Some(count) => count,
                        None => return Some(Err(tree_error("legacy member count exceeded"))),
                    };
                    if member.size_bytes > max_member_bytes
                        || member.path.as_str().len() > limits.max_key_bytes
                        || 44 > limits.max_value_bytes
                    {
                        return Some(Err(tree_error("legacy member exceeds V2 profile")));
                    }
                    let next_bytes = match observed_source_bytes_ref
                        .get()
                        .checked_add(member.size_bytes)
                        .filter(|bytes| *bytes <= max_source_bytes)
                    {
                        Some(bytes) => bytes,
                        None => return Some(Err(tree_error("legacy source-byte bound exceeded"))),
                    };
                    crate::source_admission_spooled_index::feed_membership(
                        &mut membership_hash_ref.borrow_mut(),
                        &member.path,
                        member.size_bytes,
                        member.sha256,
                    );
                    observed_members_ref.set(next_count);
                    observed_source_bytes_ref.set(next_bytes);
                    after = Some(member.path.clone());
                    let mut value = Vec::new();
                    if value.try_reserve_exact(44).is_err() {
                        return Some(Err(tree_error("legacy member value allocation failed")));
                    }
                    value.extend_from_slice(member.sha256.as_bytes());
                    value.extend_from_slice(&member.size_bytes.to_be_bytes());
                    value.extend_from_slice(&member.mode.to_le_bytes());
                    Some(Ok(AuthenticatedTreeEntryV1 {
                        key: member.path.as_str().as_bytes().to_vec(),
                        value,
                    }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error,
                )))),
            }
        });
        build_full_tree_v2(
            segment,
            MEMBERS_KIND,
            rows,
            profile,
            tree_io,
            used,
            used_rows,
            writer_live_state
                .checked_add(retained_families_state)
                .ok_or_else(|| invalid("legacy member family state overflow"))?,
            shared_work,
            deadline,
            cancelled,
        )?
    };
    retained_families_state = tree_retained_state_bytes(&members)?;
    let membership = SourceMembershipV1 {
        count: observed_members.get(),
        digest: std::mem::replace(&mut *membership_hash.borrow_mut(), Digest256Hasher::new())
            .finalize(),
    };
    if observed_members.get() != metadata.member_count
        || observed_source_bytes.get() > max_source_bytes
        || membership != metadata.membership
    {
        return Err(invalid("legacy V1 member EOF or membership differs"));
    }

    let observed_identities = Cell::new(0u64);
    let observed_identities_ref = &observed_identities;
    let identities = {
        let mut after: Option<String> = None;
        let rows = std::iter::from_fn(move || {
            match reader.identity_after(metadata.revision, after.as_deref()) {
                Ok(Some((id, path))) => {
                    let next = match observed_identities_ref
                        .get()
                        .checked_add(1)
                        .filter(|count| *count <= metadata.identity_count)
                    {
                        Some(count) => count,
                        None => return Some(Err(tree_error("legacy identity count exceeded"))),
                    };
                    if id.len() > limits.max_key_bytes
                        || path.as_str().len() > limits.max_value_bytes
                    {
                        return Some(Err(tree_error("legacy identity exceeds V2 profile")));
                    }
                    observed_identities_ref.set(next);
                    after = Some(id.clone());
                    Some(Ok(AuthenticatedTreeEntryV1 {
                        key: id.into_bytes(),
                        value: path.as_str().as_bytes().to_vec(),
                    }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error,
                )))),
            }
        });
        build_full_tree_v2(
            segment,
            IDENTITIES_KIND,
            rows,
            profile,
            tree_io,
            used,
            used_rows,
            writer_live_state
                .checked_add(retained_families_state)
                .ok_or_else(|| invalid("legacy identity family state overflow"))?,
            shared_work,
            deadline,
            cancelled,
        )?
    };
    retained_families_state = retained_families_state
        .checked_add(tree_retained_state_bytes(&identities)?)
        .ok_or_else(|| invalid("legacy identity family state overflow"))?;
    if observed_identities.get() != metadata.identity_count {
        return Err(invalid("legacy V1 identity EOF differs"));
    }

    let identity_paths = build_streamed_identity_paths_v2(
        reader,
        metadata.revision,
        metadata.identity_count,
        segment,
        profile,
        tree_io,
        used,
        used_rows,
        writer_live_state
            .checked_add(retained_families_state)
            .ok_or_else(|| invalid("legacy reverse identity family state overflow"))?,
        shared_work,
        deadline,
        cancelled,
    )?;
    retained_families_state = retained_families_state
        .checked_add(tree_retained_state_bytes(&identity_paths)?)
        .ok_or_else(|| invalid("legacy reverse identity family state overflow"))?;

    let source_visits = Cell::new(0u64);
    let edge_visits = Cell::new(0u64);
    let nonempty_sources = Cell::new(0u64);
    let source_visits_ref = &source_visits;
    let edge_visits_ref = &edge_visits;
    let nonempty_sources_ref = &nonempty_sources;
    let dependencies = {
        let mut source_after: Option<RelativePath> = None;
        let mut current_source: Option<RelativePath> = None;
        let mut target_after: Option<RelativePath> = None;
        let rows = std::iter::from_fn(move || {
            loop {
                if let Some(source) = current_source.as_ref() {
                    match reader.dependency_after(metadata.revision, source, target_after.as_ref())
                    {
                        Ok(Some(target)) => {
                            let next_edges = match edge_visits_ref
                                .get()
                                .checked_add(1)
                                .filter(|count| *count <= metadata.dependency_count)
                            {
                                Some(count) => count,
                                None => {
                                    return Some(Err(tree_error(
                                        "legacy dependency count exceeded",
                                    )));
                                }
                            };
                            if target_after.is_none() {
                                nonempty_sources_ref
                                    .set(nonempty_sources_ref.get().saturating_add(1));
                            }
                            target_after = Some(target.clone());
                            edge_visits_ref.set(next_edges);
                            let key =
                                match dependency_tree_key(source, &target, limits.max_key_bytes) {
                                    Ok(key) => key,
                                    Err(error) => return Some(Err(tree_io_error(error))),
                                };
                            let value = match length_prefixed_pair(source, &target) {
                                Ok(value) if value.len() <= limits.max_value_bytes => value,
                                Ok(_) => {
                                    return Some(Err(tree_error(
                                        "legacy dependency value exceeds profile",
                                    )));
                                }
                                Err(error) => return Some(Err(tree_io_error(error))),
                            };
                            return Some(Ok(AuthenticatedTreeEntryV1 { key, value }));
                        }
                        Ok(None) => {
                            source_after = current_source.take();
                            target_after = None;
                        }
                        Err(error) => {
                            return Some(Err(tree_io_error(io::Error::new(
                                io::ErrorKind::InvalidData,
                                error,
                            ))));
                        }
                    }
                }
                match reader.dependency_source_after(metadata.revision, source_after.as_ref()) {
                    Ok(Some(source)) => {
                        let next = match source_visits_ref
                            .get()
                            .checked_add(1)
                            .filter(|count| *count <= metadata.dependency_source_count)
                        {
                            Some(count) => count,
                            None => {
                                return Some(Err(tree_error(
                                    "legacy dependency source count exceeded",
                                )));
                            }
                        };
                        source_visits_ref.set(next);
                        current_source = Some(source);
                    }
                    Ok(None) => return None,
                    Err(error) => {
                        return Some(Err(tree_io_error(io::Error::new(
                            io::ErrorKind::InvalidData,
                            error,
                        ))));
                    }
                }
            }
        });
        build_full_tree_v2(
            segment,
            DEPENDENCIES_KIND,
            rows,
            profile,
            tree_io,
            used,
            used_rows,
            writer_live_state
                .checked_add(retained_families_state)
                .ok_or_else(|| invalid("legacy dependency family state overflow"))?,
            shared_work,
            deadline,
            cancelled,
        )?
    };
    retained_families_state = retained_families_state
        .checked_add(tree_retained_state_bytes(&dependencies)?)
        .ok_or_else(|| invalid("legacy dependency family state overflow"))?;
    if source_visits.get() != metadata.dependency_source_count
        || nonempty_sources.get() != source_visits.get()
        || edge_visits.get() != metadata.dependency_count
    {
        return Err(invalid(
            "legacy V1 dependency EOF or empty-source representation differs",
        ));
    }

    let observed_retirements = Cell::new(0u64);
    let observed_retirements_ref = &observed_retirements;
    let retirements = {
        let mut ordinal = 0u64;
        let rows = std::iter::from_fn(move || {
            if ordinal == metadata.retirement_count {
                return None;
            }
            let current = ordinal;
            ordinal = match ordinal.checked_add(1) {
                Some(next) => next,
                None => return Some(Err(tree_error("legacy retirement ordinal overflow"))),
            };
            match reader.retirement_at(metadata.revision, current) {
                Ok(Some(row)) => {
                    observed_retirements_ref.set(ordinal);
                    Some(encode_retirement_entry(current, row))
                }
                Ok(None) => Some(Err(tree_error("legacy retirement ordinal ended early"))),
                Err(error) => Some(Err(tree_io_error(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error,
                )))),
            }
        });
        build_full_tree_v2(
            segment,
            RETIREMENTS_KIND,
            rows,
            profile,
            tree_io,
            used,
            used_rows,
            writer_live_state
                .checked_add(retained_families_state)
                .ok_or_else(|| invalid("legacy retirement family state overflow"))?,
            shared_work,
            deadline,
            cancelled,
        )?
    };
    if observed_retirements.get() != metadata.retirement_count
        || reader
            .retirement_at(metadata.revision, metadata.retirement_count)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .is_some()
    {
        return Err(invalid("legacy V1 retirement EOF differs"));
    }
    let roots = SourceRevisionRootsV2 {
        revision: metadata.revision,
        base_revision: metadata.base_revision,
        validator_sha256: metadata.validator_sha256,
        manifest_sha256: artifact_sha256,
        source_artifact: SourceRevisionArtifactV2::LegacyManifestV1 {
            sha256: artifact_sha256,
        },
        batch_sha256: None,
        membership_v1: Some(membership),
        membership_v2: Some(SourceMembershipV2::from_members_tree(
            observed_members.get(),
            observed_source_bytes.get(),
            members.commitment,
        )),
        completion_proof: None,
        source_bytes: observed_source_bytes.get(),
        member_count: observed_members.get(),
        identity_count: observed_identities.get(),
        dependency_source_count: source_visits.get(),
        dependency_count: edge_visits.get(),
        retirement_count: observed_retirements.get(),
        members,
        identities,
        dependencies,
        retirements,
        objects: None,
        identity_paths: Some(identity_paths),
    };
    let roots_retained = roots.retained_state_bytes()?;
    let post_build_live_state = writer_live_state
        .checked_add(segment_store_retained_state_bytes(segment)?)
        .and_then(|bytes| bytes.checked_add(roots_retained))
        .ok_or_else(|| invalid("legacy root retained state overflow"))?;
    if post_build_live_state > profile.max_working_state_bytes {
        return Err(invalid(
            "legacy root validation exceeds migration state profile",
        ));
    }
    roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    if roots.encode_state_upper_bound(post_build_live_state)? > profile.max_working_state_bytes {
        return Err(invalid(
            "legacy root encoding exceeds migration state profile",
        ));
    }
    let _ = candidate;
    Ok(roots)
}

fn build_full_tree_v2<I>(
    segment: &SegmentStore,
    kind: &[u8],
    rows: I,
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
    tree_io: &Arc<NativeV2TreeIo>,
    used: &mut AuthenticatedTreeWorkV1,
    used_rows: &mut u64,
    additional_live_state: usize,
    shared_work: &AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<AuthenticatedTreeDescriptorV2>
where
    I: IntoIterator<Item = tos_segment_store::Result<AuthenticatedTreeEntryV1>>,
{
    if additional_live_state > profile.max_working_state_bytes {
        return Err(invalid(
            "V2 migration tree live state exceeds source profile",
        ));
    }
    let mut limits = remaining_tree_limits(profile.tree_limits, *used)?;
    limits.max_rows = profile
        .tree_limits
        .max_rows
        .checked_sub(*used_rows)
        .filter(|rows| *rows > 0)
        .ok_or_else(|| invalid("V2 migration cumulative row profile exceeded"))?;
    let io_ledger: Arc<dyn AuthenticatedTreeIoLedgerV1> = tree_io.clone();
    let work_budget = shared_work.clone();
    let mut debit_work = move || work_budget.charge_many(1).is_ok();
    let (descriptor, work) = segment
        .build_authenticated_tree_v2_with_work_and_io_and_state_and_callback(
            kind,
            rows,
            limits,
            Some(io_ledger),
            profile.max_working_state_bytes,
            additional_live_state,
            deadline,
            cancelled,
            &mut debit_work,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    add_tree_work(used, work, profile.tree_limits)?;
    *used_rows = (*used_rows)
        .checked_add(descriptor.entries)
        .filter(|rows| *rows <= profile.tree_limits.max_rows)
        .ok_or_else(|| invalid("V2 migration cumulative row count exceeded"))?;
    Ok(descriptor)
}

/// Build the reverse identity index in path/id byte order without retaining
/// the corpus identity list. Source member paths and each path's IDs are
/// already exposed as bounded ordered cursors by the candidate and index.
fn build_index_identity_paths_v2(
    candidate: &super::source_admission_spooled_candidate::SpoolCandidate<'_>,
    index: &super::source_admission_spooled_index::IndexView<'_>,
    segment: &SegmentStore,
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
    tree_io: &Arc<NativeV2TreeIo>,
    used: &mut AuthenticatedTreeWorkV1,
    used_rows: &mut u64,
    additional_live_state: usize,
    shared_work: &AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<AuthenticatedTreeDescriptorV2> {
    let cursor_state = profile
        .tree_limits
        .max_key_bytes
        .checked_mul(4)
        .and_then(|bytes| {
            bytes.checked_add(
                4 * size_of::<Option<RelativePath>>()
                    + 4 * size_of::<Option<String>>()
                    + size_of::<Cell<u64>>(),
            )
        })
        .ok_or_else(|| invalid("reverse identity cursor state overflow"))?;
    let live_state = additional_live_state
        .checked_add(cursor_state)
        .ok_or_else(|| invalid("reverse identity live state overflow"))?;
    if live_state > profile.max_working_state_bytes {
        return Err(invalid("reverse identity cursor exceeds state profile"));
    }
    let expected = index.identity_count();
    let observed = Cell::new(0u64);
    let observed_rows = &observed;
    let row_allowance = profile.max_working_state_bytes / 8;
    let rows = {
        let mut after_path: Option<RelativePath> = None;
        let mut current_path: Option<RelativePath> = None;
        let mut after_id: Option<String> = None;
        std::iter::from_fn(move || {
            loop {
                if let Some(path) = current_path.as_ref() {
                    match index.identity_for_path_after(path, after_id.as_deref()) {
                        Ok(Some(id)) => {
                            after_id = Some(id.clone());
                            observed_rows.set(observed_rows.get().saturating_add(1));
                            let key = match identity_path_tree_key(
                                path,
                                &id,
                                profile.tree_limits.max_key_bytes,
                            ) {
                                Ok(key) => key,
                                Err(error) => return Some(Err(tree_io_error(error))),
                            };
                            return Some(Ok(AuthenticatedTreeEntryV1 {
                                key,
                                value: Vec::new(),
                            }));
                        }
                        Ok(None) => {
                            current_path = None;
                            after_id = None;
                        }
                        Err(error) => return Some(Err(tree_io_error(error))),
                    }
                    continue;
                }
                match candidate.member_after_bounded(after_path.as_ref(), row_allowance) {
                    Ok(Some(member)) => {
                        after_path = Some(member.path.clone());
                        current_path = Some(member.path);
                    }
                    Ok(None) => return None,
                    Err(error) => return Some(Err(tree_io_error(error))),
                }
            }
        })
    };
    let descriptor = build_full_tree_v2(
        segment,
        IDENTITY_PATHS_KIND,
        rows,
        profile,
        tree_io,
        used,
        used_rows,
        live_state,
        shared_work,
        deadline,
        cancelled,
    )?;
    if observed.get() != expected || descriptor.entries != expected {
        return Err(invalid("reverse identity cursor EOF or count differs"));
    }
    Ok(descriptor)
}

fn build_streamed_identity_paths_v2(
    reader: &StreamedCorpusCutReaderV1,
    revision: SourceRevision,
    expected: u64,
    segment: &SegmentStore,
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
    tree_io: &Arc<NativeV2TreeIo>,
    used: &mut AuthenticatedTreeWorkV1,
    used_rows: &mut u64,
    additional_live_state: usize,
    shared_work: &AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<AuthenticatedTreeDescriptorV2> {
    let cursor_state = profile
        .tree_limits
        .max_key_bytes
        .checked_mul(4)
        .and_then(|bytes| {
            bytes.checked_add(
                4 * size_of::<Option<RelativePath>>()
                    + 4 * size_of::<Option<String>>()
                    + size_of::<Cell<u64>>(),
            )
        })
        .ok_or_else(|| invalid("legacy reverse identity cursor state overflow"))?;
    let live_state = additional_live_state
        .checked_add(cursor_state)
        .ok_or_else(|| invalid("legacy reverse identity live state overflow"))?;
    if live_state > profile.max_working_state_bytes {
        return Err(invalid(
            "legacy reverse identity cursor exceeds state profile",
        ));
    }
    let limits = profile.tree_limits;
    let observed = Cell::new(0u64);
    let observed_rows = &observed;
    let rows = {
        let mut after_path: Option<RelativePath> = None;
        let mut current_path: Option<RelativePath> = None;
        let mut after_id: Option<String> = None;
        std::iter::from_fn(move || {
            loop {
                if let Some(path) = current_path.as_ref() {
                    match reader.identity_for_path_after(revision, path, after_id.as_deref()) {
                        Ok(Some(id)) => {
                            after_id = Some(id.clone());
                            let Some(next) = observed_rows
                                .get()
                                .checked_add(1)
                                .filter(|n| *n <= expected)
                            else {
                                return Some(Err(tree_error(
                                    "legacy reverse identity count exceeded",
                                )));
                            };
                            observed_rows.set(next);
                            let key = match identity_path_tree_key(path, &id, limits.max_key_bytes)
                            {
                                Ok(key) => key,
                                Err(error) => return Some(Err(tree_io_error(error))),
                            };
                            return Some(Ok(AuthenticatedTreeEntryV1 {
                                key,
                                value: Vec::new(),
                            }));
                        }
                        Ok(None) => {
                            current_path = None;
                            after_id = None;
                        }
                        Err(error) => {
                            return Some(Err(tree_io_error(io::Error::new(
                                io::ErrorKind::InvalidData,
                                error,
                            ))));
                        }
                    }
                    continue;
                }
                match reader.member_after(revision, after_path.as_ref()) {
                    Ok(Some(member)) => {
                        after_path = Some(member.path.clone());
                        current_path = Some(member.path);
                    }
                    Ok(None) => return None,
                    Err(error) => {
                        return Some(Err(tree_io_error(io::Error::new(
                            io::ErrorKind::InvalidData,
                            error,
                        ))));
                    }
                }
            }
        })
    };
    let descriptor = build_full_tree_v2(
        segment,
        IDENTITY_PATHS_KIND,
        rows,
        profile,
        tree_io,
        used,
        used_rows,
        live_state,
        shared_work,
        deadline,
        cancelled,
    )?;
    if observed.get() != expected || descriptor.entries != expected {
        return Err(invalid("legacy reverse identity EOF or count differs"));
    }
    Ok(descriptor)
}

/// Migrate a held, exact V1 cut into typed V2 roots. Unlike the V2-base COW
/// route this is intentionally a full rebuild: every retained V1 revision and
/// each of its four logical families is streamed and reauthenticated under
/// one cumulative work, row, IO and allocation profile.
pub(crate) fn build_v1_migration_rootset_v2(
    store: &super::source_admission_store::AdmissionStore,
    candidate: &super::source_admission_spooled_candidate::SpoolCandidate<'_>,
    index: &super::source_admission_spooled_index::IndexView<'_>,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<BuiltSuccessorRootSetV2> {
    if !candidate.matches_invocation(deadline, cancelled) {
        return Err(invalid("V1 migration invocation clock differs"));
    }
    let profile = index
        .segment_v2_budget()
        .ok_or_else(|| invalid("V1 migration lacks native V2 completion profile"))?;
    let shared_work = candidate.admission_work_budget()?;
    index.verify_candidate()?;
    let fence = candidate.fence()?;
    let expected_base = fence
        .base_revision
        .ok_or_else(|| invalid("V1 migration selected base is absent"))?;
    let base = candidate.v1_base_reader()?;
    if base.current_revision() != expected_base {
        return Err(invalid("V1 migration base reader binding differs"));
    }
    let revision_count = base.revision_count();
    let history_limit = candidate.migration_history_revision_limit()?;
    let history_identity_limit = candidate.migration_history_identity_limit()?;
    let total_history_count = revision_count
        .checked_add(1)
        .filter(|count| *count <= history_limit)
        .ok_or_else(|| invalid("V1 migration history count exceeds selected profile"))?;
    if revision_count == 0 {
        return Err(invalid("V1 migration selected cut has no current revision"));
    }

    let pointer_limits = candidate.migration_pointer_read_limits()?;
    let selected = store
        .current_selection(pointer_limits, deadline, cancelled, Some(&profile.io))?
        .ok_or_else(|| invalid("V1 migration current selector is absent"))?;
    if selected.format != CorpusPointerFormat::V1
        || selected.revision != expected_base
        || selected.rootset_sha256.is_some()
    {
        return Err(invalid("V1 migration selected pointer tuple differs"));
    }
    let current_v1 = base
        .revision_at(0)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .ok_or_else(|| invalid("V1 migration current metadata is absent"))?;
    if current_v1.revision != expected_base || selected.previous != current_v1.base_revision {
        return Err(invalid("V1 migration current pointer ancestry differs"));
    }

    // Preflight every retained row family and history identity visit before
    // the migration creates any V2 tree nodes. The selected limits are reused
    // unchanged for all old revisions, the new current roots and history.
    let mut expected_revision = Some(expected_base);
    let mut historical_identity_visits = 0u64;
    let mut expected_rows = total_history_count;
    for ordinal in 0..revision_count {
        let metadata = base
            .revision_at(ordinal)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .ok_or_else(|| invalid("V1 migration history metadata ended early"))?;
        if Some(metadata.revision) != expected_revision
            || metadata.member_count != metadata.membership.count
            || metadata.member_count > candidate.migration_member_count_limit()?
            || metadata.dependency_source_count > metadata.dependency_count
        {
            return Err(invalid("V1 migration ancestor metadata or count differs"));
        }
        expected_revision = metadata.base_revision;
        historical_identity_visits = historical_identity_visits
            .checked_add(metadata.identity_count)
            .filter(|count| *count <= history_identity_limit)
            .ok_or_else(|| invalid("V1 migration historical identity profile exceeded"))?;
        for count in [
            metadata.member_count,
            metadata.identity_count,
            metadata.dependency_count,
            metadata.retirement_count,
        ] {
            expected_rows = expected_rows
                .checked_add(count)
                .ok_or_else(|| invalid("V1 migration row preflight overflow"))?;
        }
    }
    if expected_revision.is_some() || historical_identity_visits > history_identity_limit {
        return Err(invalid("V1 migration history chain is incomplete"));
    }
    let (current_members, current_source_bytes) = candidate.membership_counts();
    let current_identity_count = index.identity_count();
    let current_dependency_sources = index.dependency_source_count();
    let current_dependency_count = index.dependency_count();
    let current_retirement_count = candidate.retirement_count();
    let current_object_count = candidate.desired_object_count()?;
    for count in [
        current_members,
        current_identity_count,
        current_dependency_count,
        current_retirement_count,
        current_object_count,
    ] {
        expected_rows = expected_rows
            .checked_add(count)
            .ok_or_else(|| invalid("V1 migration current row preflight overflow"))?;
    }
    if expected_rows > profile.tree_limits.max_rows
        || current_members != fence.membership.count
        || current_source_bytes != fence.source_bytes
        || current_dependency_sources > current_dependency_count
    {
        return Err(invalid(
            "V1 migration total rows or current counts exceed profile",
        ));
    }

    let tree_io = NativeV2TreeIo::from_budget(profile);
    if !store.has_v2_allocation_accountant(&profile.allocation_accountant) {
        return Err(invalid(
            "V1 migration store allocation or segment binding differs",
        ));
    }
    store.retain_v2_store_custody(tree_io.custody_reservation());
    let segment_bytes = profile.max_allocated_bytes;
    if segment_bytes < 65_536 {
        return Err(invalid(
            "V1 migration persistent profile is below metadata floor",
        ));
    }
    let segment_limits = selected_segment_limits(profile)?;
    let tree_ledger: Arc<dyn AuthenticatedTreeIoLedgerV1> = tree_io.clone();
    let segment = store.segment_store_v2_with_io(
        SOURCE_ADMISSION_V2_DOMAIN,
        segment_limits,
        tree_ledger,
        deadline,
        cancelled,
    )?;
    if segment.custody_domain() != SOURCE_ADMISSION_V2_DOMAIN {
        return Err(invalid("V1 migration segment store domain differs"));
    }
    let segment_live_state = segment_store_retained_state_bytes(&segment)?;

    let migration_cursor_state = size_of::<StreamedRevisionV1>()
        .checked_add(size_of::<Option<SourceRevision>>())
        .and_then(|bytes| bytes.checked_add(4 * size_of::<u64>()))
        .and_then(|bytes| bytes.checked_add(size_of::<RefCell<Digest256Hasher>>()))
        .and_then(|bytes| bytes.checked_add(8 * size_of::<Cell<u64>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
        .ok_or_else(|| invalid("V1 migration cursor state overflow"))?;
    let writer_live_state = writer_context_state_bytes(
        candidate,
        index,
        profile,
        size_of::<BuiltSuccessorRootSetV2>()
            .checked_add(migration_cursor_state)
            .ok_or_else(|| invalid("V1 migration writer state overflow"))?,
    )?;
    if writer_live_state > profile.max_working_state_bytes {
        return Err(invalid(
            "V1 migration writer state exceeds selected profile",
        ));
    }
    let mut used = AuthenticatedTreeWorkV1::default();
    let mut used_rows = 0u64;
    candidate.begin_v1_migration_history()?;

    for ordinal in 0..revision_count {
        active(deadline, cancelled)?;
        let metadata = base
            .revision_at(ordinal)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .ok_or_else(|| invalid("V1 migration history metadata ended during build"))?;
        let roots = build_legacy_revision_roots_v2(
            store,
            candidate,
            base,
            metadata,
            &segment,
            profile,
            &tree_io,
            &mut used,
            &mut used_rows,
            &shared_work,
            writer_live_state,
            deadline,
            cancelled,
        )?;
        let raw = roots.encode_with_state_limit(
            profile.max_working_state_bytes,
            writer_live_state
                .checked_add(segment_live_state)
                .ok_or_else(|| invalid("legacy root encoding state overflow"))?,
        )?;
        candidate.stage_v1_migration_history_row(metadata.revision.0, &raw)?;
    }

    let mut current_family_state = 0usize;
    let mut member_rows = {
        let mut after: Option<RelativePath> = None;
        std::iter::from_fn(move || {
            match candidate
                .member_after_bounded(after.as_ref(), profile.max_working_state_bytes / 8)
            {
                Ok(Some(member)) => {
                    after = Some(member.path.clone());
                    let mut value = Vec::new();
                    if value.try_reserve_exact(44).is_err() {
                        return Some(Err(tree_error(
                            "migration current member allocation failed",
                        )));
                    }
                    value.extend_from_slice(member.sha256.as_bytes());
                    value.extend_from_slice(&member.size_bytes.to_be_bytes());
                    value.extend_from_slice(&member.mode.to_le_bytes());
                    Some(Ok(AuthenticatedTreeEntryV1 {
                        key: member.path.as_str().as_bytes().to_vec(),
                        value,
                    }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let members = build_full_tree_v2(
        &segment,
        MEMBERS_KIND,
        &mut member_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(current_family_state)
            .ok_or_else(|| invalid("migration member family state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    current_family_state = tree_retained_state_bytes(&members)?;

    let identity_rows = {
        let mut after: Option<String> = None;
        std::iter::from_fn(move || match index.identities_after(after.as_deref()) {
            Ok(Some((id, path))) => {
                after = Some(id.clone());
                Some(Ok(AuthenticatedTreeEntryV1 {
                    key: id.into_bytes(),
                    value: path.as_str().as_bytes().to_vec(),
                }))
            }
            Ok(None) => None,
            Err(error) => Some(Err(tree_io_error(error))),
        })
    };
    let identities = build_full_tree_v2(
        &segment,
        IDENTITIES_KIND,
        identity_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(current_family_state)
            .ok_or_else(|| invalid("migration identity family state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    current_family_state = current_family_state
        .checked_add(tree_retained_state_bytes(&identities)?)
        .ok_or_else(|| invalid("migration identity family state overflow"))?;

    let identity_paths = build_index_identity_paths_v2(
        candidate,
        index,
        &segment,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(current_family_state)
            .ok_or_else(|| invalid("migration reverse identity family state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    current_family_state = current_family_state
        .checked_add(tree_retained_state_bytes(&identity_paths)?)
        .ok_or_else(|| invalid("migration reverse identity family state overflow"))?;

    let dependency_source_rows = Cell::new(0u64);
    let dependency_source_rows = &dependency_source_rows;
    let dependency_rows = {
        use super::source_admission_index::NativeDependencyDirectionV1::Forward;
        let mut after: Option<(RelativePath, RelativePath)> = None;
        let mut prior_source: Option<RelativePath> = None;
        std::iter::from_fn(move || {
            match index.dependency_pair_after(
                Forward,
                after.as_ref().map(|(source, target)| (source, target)),
            ) {
                Ok(Some((source, target))) => {
                    if prior_source.as_ref() != Some(&source) {
                        dependency_source_rows.set(dependency_source_rows.get().saturating_add(1));
                        prior_source = Some(source.clone());
                    }
                    after = Some((source.clone(), target.clone()));
                    let key = match dependency_tree_key(
                        &source,
                        &target,
                        profile.tree_limits.max_key_bytes,
                    ) {
                        Ok(key) => key,
                        Err(error) => return Some(Err(tree_io_error(error))),
                    };
                    let value = match length_prefixed_pair(&source, &target) {
                        Ok(value) if value.len() <= profile.tree_limits.max_value_bytes => value,
                        Ok(_) => {
                            return Some(Err(tree_error(
                                "migration dependency value exceeds profile",
                            )));
                        }
                        Err(error) => return Some(Err(tree_io_error(error))),
                    };
                    Some(Ok(AuthenticatedTreeEntryV1 { key, value }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let dependencies = build_full_tree_v2(
        &segment,
        DEPENDENCIES_KIND,
        dependency_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(current_family_state)
            .ok_or_else(|| invalid("migration dependency family state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    current_family_state = current_family_state
        .checked_add(tree_retained_state_bytes(&dependencies)?)
        .ok_or_else(|| invalid("migration dependency family state overflow"))?;
    if dependency_source_rows.get() != current_dependency_sources {
        return Err(invalid("migration current dependency source EOF differs"));
    }

    let retirement_rows = {
        let mut ordinal = 0u64;
        std::iter::from_fn(move || {
            if ordinal >= current_retirement_count {
                return None;
            }
            let current = ordinal;
            ordinal += 1;
            match candidate.retirement_at_bounded(current, profile.max_working_state_bytes / 8) {
                Ok(Some(row)) => Some(encode_retirement_entry(current, row)),
                Ok(None) => Some(Err(tree_error("migration current retirement ended early"))),
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let retirements = build_full_tree_v2(
        &segment,
        RETIREMENTS_KIND,
        retirement_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        writer_live_state
            .checked_add(current_family_state)
            .ok_or_else(|| invalid("migration retirement family state overflow"))?,
        &shared_work,
        deadline,
        cancelled,
    )?;
    current_family_state = current_family_state
        .checked_add(tree_retained_state_bytes(&retirements)?)
        .ok_or_else(|| invalid("migration retirement family state overflow"))?;
    if members.entries != current_members
        || identities.entries != current_identity_count
        || dependencies.entries != current_dependency_count
        || retirements.entries != current_retirement_count
    {
        return Err(invalid("migration current family EOF counts differ"));
    }
    if candidate
        .retirement_at_bounded(
            current_retirement_count,
            profile.max_working_state_bytes / 8,
        )?
        .is_some()
    {
        return Err(invalid("migration current retirement rows exceed count"));
    }

    let object_live_state = writer_live_state
        .checked_add(current_family_state)
        .ok_or_else(|| invalid("migration object root live-state overflow"))?;
    let object_limits = packed_object_limits(
        profile,
        &segment,
        used,
        used_rows,
        current_object_count,
        object_live_state,
        &shared_work,
        false,
    )?;
    let object_work_budget = shared_work.clone();
    let mut object_debit_work = move || object_work_budget.charge_many(1).is_ok();
    let (objects, object_work) = PackedObjectWriterV2::build(
        &segment,
        candidate.packed_object_sources(),
        object_limits,
        tree_io.clone(),
        deadline,
        cancelled,
        &mut object_debit_work,
    )?;
    add_tree_work(&mut used, object_work.tree_work, profile.tree_limits)?;
    used_rows = used_rows
        .checked_add(objects.entries)
        .filter(|rows| *rows <= profile.tree_limits.max_rows)
        .ok_or_else(|| invalid("migration packed object row profile exceeded"))?;
    current_family_state = current_family_state
        .checked_add(tree_retained_state_bytes(&objects)?)
        .ok_or_else(|| invalid("migration packed object family state overflow"))?;

    let placeholder = Digest256::of_bytes(&[]);
    let provisional = SourceRevisionRootsV2 {
        revision: expected_base,
        base_revision: Some(expected_base),
        validator_sha256: fence.validator_sha256,
        manifest_sha256: placeholder,
        source_artifact: SourceRevisionArtifactV2::CompactPackedV2 {
            sha256: placeholder,
            bytes: 1,
        },
        batch_sha256: Some(fence.batch_sha256),
        membership_v1: Some(fence.membership),
        membership_v2: Some(SourceMembershipV2::from_members_tree(
            current_members,
            current_source_bytes,
            members.commitment,
        )),
        completion_proof: Some(index.completion_proof()?),
        source_bytes: current_source_bytes,
        member_count: current_members,
        identity_count: current_identity_count,
        dependency_source_count: current_dependency_sources,
        dependency_count: current_dependency_count,
        retirement_count: current_retirement_count,
        members,
        identities,
        identity_paths: Some(identity_paths),
        dependencies,
        retirements,
        objects: Some(objects),
    };
    let provisional_retained_state = provisional.retained_state_bytes()?;
    let provisional_validate_state = writer_live_state
        .checked_add(segment_live_state)
        .and_then(|bytes| bytes.checked_add(provisional_retained_state))
        .ok_or_else(|| invalid("V1 migration provisional state overflow"))?;
    if provisional_validate_state > profile.max_working_state_bytes {
        return Err(invalid(
            "V1 migration provisional validation exceeds state profile",
        ));
    }
    provisional.validate_store_content_binding(segment.store_id(), segment.domain_digest())?;
    let commit_live_state = writer_live_state
        .checked_add(segment_live_state)
        .ok_or_else(|| invalid("V1 migration compact live-state overflow"))?;
    let (current, commit, source_record) = CompactCommitV2::seal_successor(
        provisional,
        fence.batch_sha256,
        profile.max_working_state_bytes,
        commit_live_state,
    )?;
    drop(commit);
    let current_retained_state = current.retained_state_bytes()?;
    let current_validate_state = writer_live_state
        .checked_add(segment_live_state)
        .and_then(|bytes| bytes.checked_add(source_record.capacity()))
        .and_then(|bytes| bytes.checked_add(current_retained_state))
        .ok_or_else(|| invalid("V1 migration current validation state overflow"))?;
    if current_validate_state > profile.max_working_state_bytes {
        return Err(invalid(
            "V1 migration current validation exceeds state profile",
        ));
    }
    current.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let artifact_sha256 = Digest256::of_bytes(&source_record);
    if current.source_artifact.sha256() != artifact_sha256
        || current.source_artifact.bytes() != Some(source_record.len() as u64)
    {
        return Err(invalid("V1 migration compact record binding differs"));
    }
    let current_row = current.encode_with_state_limit(
        profile.max_working_state_bytes,
        writer_live_state
            .checked_add(segment_live_state)
            .and_then(|bytes| bytes.checked_add(source_record.capacity()))
            .ok_or_else(|| invalid("V1 migration current row state overflow"))?,
    )?;
    let current_row_len = current_row.len();
    let current_row_capacity = current_row.capacity();
    let current_row_sha = Digest256::of_bytes(&current_row);
    candidate.stage_v1_migration_history_row(current.revision.0, &current_row)?;
    drop(current_row);

    let history_rows = {
        let mut after: Option<Digest256> = None;
        std::iter::from_fn(move || {
            match candidate.v1_migration_history_after(after, profile.max_working_state_bytes / 4) {
                Ok(Some((revision, raw))) => {
                    after = Some(revision);
                    Some(Ok(AuthenticatedTreeEntryV1 {
                        key: revision.as_bytes().to_vec(),
                        value: raw,
                    }))
                }
                Ok(None) => None,
                Err(error) => Some(Err(tree_io_error(error))),
            }
        })
    };
    let history_builder_live_state = writer_live_state
        .checked_add(current_retained_state)
        .and_then(|bytes| bytes.checked_add(source_record.capacity()))
        .and_then(|bytes| bytes.checked_add(profile.max_working_state_bytes / 4))
        .ok_or_else(|| invalid("V1 migration history state overflow"))?;
    let history = build_full_tree_v2(
        &segment,
        HISTORY_KIND,
        history_rows,
        profile,
        &tree_io,
        &mut used,
        &mut used_rows,
        history_builder_live_state,
        &shared_work,
        deadline,
        cancelled,
    )?;
    if history.entries != total_history_count {
        return Err(invalid("V1 migration history-tree EOF count differs"));
    }
    let roots = SourceRootSetV2 { current, history };
    let roots_retained_state = roots.retained_state_bytes()?;
    let root_live_state = roots_retained_state
        .checked_add(writer_live_state)
        .and_then(|bytes| bytes.checked_add(segment_live_state))
        .and_then(|bytes| bytes.checked_add(source_record.capacity()))
        .ok_or_else(|| invalid("V1 migration rootset retained state overflow"))?;
    if root_live_state > profile.max_working_state_bytes {
        return Err(invalid(
            "V1 migration root validation exceeds state profile",
        ));
    }
    roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let history_result_state =
        SourceRevisionRootsV2::retained_state_upper_bound_for_value(ROOTSET_MAX_BYTES)?;
    let history_lookup_live_state = root_live_state
        .checked_add(current_row_capacity)
        .and_then(|bytes| bytes.checked_add(history_result_state))
        .ok_or_else(|| invalid("V1 migration history lookup state overflow"))?;
    if history_lookup_live_state > profile.max_working_state_bytes {
        return Err(invalid("V1 migration history lookup exceeds state profile"));
    }
    let work_budget = shared_work.clone();
    let mut debit_work = move || work_budget.charge_many(1).is_ok();
    let (history_row, history_read_work) = segment
        .lookup_authenticated_tree_v2_with_work_and_io_and_callback(
            &roots.history,
            roots.current.revision.0.as_bytes(),
            remaining_tree_limits(profile.tree_limits, used)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
            &mut debit_work,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    add_tree_work(&mut used, history_read_work, profile.tree_limits)?;
    let history_row =
        history_row.ok_or_else(|| invalid("V1 migration current history row absent"))?;
    if history_row.len() != current_row_len || Digest256::of_bytes(&history_row) != current_row_sha
    {
        return Err(invalid("V1 migration current history row differs"));
    }
    drop(history_row);
    // The same retained selector is rechecked below after every streamed read.
    // The streamed V1 reader owns an immutable selected revision index.
    let selected_after =
        store.current_selection(pointer_limits, deadline, cancelled, Some(&profile.io))?;
    if selected_after != Some(selected) {
        return Err(invalid("V1 migration selector changed during full rebuild"));
    }
    index.verify_candidate()?;
    candidate.tick()?;
    let bytes = roots.encode_with_state_limit(profile.max_working_state_bytes, root_live_state)?;
    let sha256 = Digest256::of_bytes(&bytes);
    Ok(BuiltSuccessorRootSetV2 {
        expected_base,
        expected_previous_rootset_sha256: None,
        expected_selection: selected,
        roots,
        bytes,
        sha256,
        source_record,
        tree_io,
        segment_store: segment,
        work: used,
    })
}

/// Build one actual authenticated V2 successor from the selected base and the
/// already completed native candidate. The full V1 membership digest remains
/// the validator's audited value; physical COW work is limited to changed
/// member/identity/dependency/retirement rows and the one history append.
pub(crate) fn build_successor_rootset_v2(
    store: &super::source_admission_store::AdmissionStore,
    candidate: &super::source_admission_spooled_candidate::SpoolCandidate<'_>,
    index: &super::source_admission_spooled_index::IndexView<'_>,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<BuiltSuccessorRootSetV2> {
    if !candidate.matches_invocation(deadline, cancelled) {
        return Err(invalid("V2 successor invocation clock differs"));
    }
    let profile = index
        .segment_v2_budget()
        .ok_or_else(|| invalid("V2 successor lacks the native completion profile"))?;
    let shared_work = candidate.admission_work_budget()?;
    index.verify_candidate()?;
    let candidate_fence = candidate.fence()?;
    if candidate_fence != index.fence()
        || candidate_fence.base_revision.is_none()
        || !store.has_v2_allocation_accountant(&profile.allocation_accountant)
        || !store.has_v2_segments()?
    {
        return Err(invalid(
            "V2 successor candidate or held store binding differs",
        ));
    }

    let selected_base = candidate_fence
        .base_revision
        .ok_or_else(|| invalid("V2 successor base revision is absent"))?;
    let base_ref = candidate.v2_base_session()?;
    let base = base_ref.borrow();
    if base.selected_revision() != selected_base
        || !base.shares_io_budget(&profile.io)
        || base.current_roots().revision != selected_base
    {
        return Err(invalid("V2 successor selected base or IO ledger differs"));
    }
    base.verify_current_fence()?;
    let previous_rootset_sha256 = base.selected_rootset_sha256()?;
    let expected_selection = base.selected_selection();
    if expected_selection.format != CorpusPointerFormat::V2
        || expected_selection.revision != selected_base
        || expected_selection.rootset_sha256 != Some(previous_rootset_sha256)
    {
        return Err(invalid("V2 successor selected pointer tuple differs"));
    }
    let selected_roots = base.current_rootset();
    selected_roots.validate_store_binding(
        base.segment_store().store_id(),
        base.segment_store().domain_digest(),
    )?;
    if selected_roots.current.revision != selected_base {
        return Err(invalid("V2 successor base rootset revision differs"));
    }

    // The old session and the cloned four-root record overlap throughout the
    // writer. Check the exact retained source slice before cloning descriptors.
    let base_live_state = base.retained_live_state_bytes()?;
    let current_clone_state = selected_roots.current.retained_state_bytes()?;
    let cursor_state = profile
        .tree_limits
        .max_key_bytes
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(profile.tree_limits.max_value_bytes))
        .and_then(|bytes| bytes.checked_add(16 * 1024))
        .ok_or_else(|| invalid("V2 successor cursor state overflow"))?;
    let builder_state = size_of::<BuiltSuccessorRootSetV2>()
        .checked_add(size_of::<NativeV2TreeIo>())
        .and_then(|bytes| bytes.checked_add(size_of::<AuthenticatedTreeWorkV1>()))
        .and_then(|bytes| bytes.checked_add(cursor_state))
        .ok_or_else(|| invalid("V2 successor builder state overflow"))?;
    if base_live_state
        .checked_add(current_clone_state)
        .and_then(|bytes| bytes.checked_add(builder_state))
        .is_none_or(|bytes| bytes > profile.max_working_state_bytes)
    {
        return Err(invalid("V2 successor base and clone exceed state profile"));
    }

    let mut current = selected_roots.current.clone();
    let tree_io = NativeV2TreeIo::from_budget(profile);
    store.retain_v2_store_custody(tree_io.custody_reservation());
    let segment = base.segment_store().clone();
    if segment.custody_domain() != SOURCE_ADMISSION_V2_DOMAIN {
        return Err(invalid("V2 successor segment domain differs"));
    }
    current.validate_store_binding(segment.store_id(), segment.domain_digest())?;

    let mut work = base.accumulated_tree_work();
    let row_count = Cell::new(0u64);
    let row_count = &row_count;
    let row_limit = profile.tree_limits.max_rows;
    let row_allowance = candidate.v2_cursor_state_allowance(profile.max_working_state_bytes)?;
    let tree_live_state = base_live_state
        .checked_add(current.retained_state_bytes()?)
        .and_then(|bytes| bytes.checked_add(builder_state))
        .ok_or_else(|| invalid("V2 successor live-state overflow"))?;

    let members = {
        let mut after: Option<RelativePath> = None;
        let changes = std::iter::from_fn(move || {
            let path = match candidate.changed_source_after(after.as_ref(), row_allowance) {
                Ok(Some(path)) => path,
                Ok(None) => return None,
                Err(error) => return Some(Err(tree_io_error(error))),
            };
            after = Some(path.clone());
            let member = match candidate.member_bounded(&path, row_allowance) {
                Ok(member) => member,
                Err(error) => return Some(Err(tree_io_error(error))),
            };
            if member.as_ref().is_some_and(|member| member.path != path) {
                return Some(Err(tree_error("V2 changed member path differs")));
            }
            let value = match member {
                Some(member) => {
                    let mut value = Vec::new();
                    if value.try_reserve_exact(44).is_err() {
                        return Some(Err(tree_error("V2 member value allocation failed")));
                    }
                    value.extend_from_slice(member.sha256.as_bytes());
                    value.extend_from_slice(&member.size_bytes.to_be_bytes());
                    value.extend_from_slice(&member.mode.to_le_bytes());
                    Some(value)
                }
                None => None,
            };
            let mut key = Vec::new();
            if key.try_reserve_exact(path.as_str().len()).is_err() {
                return Some(Err(tree_error("V2 member key allocation failed")));
            }
            key.extend_from_slice(path.as_str().as_bytes());
            counted_delta(&row_count, row_limit, key, value)
        });
        apply_successor_delta(
            &segment,
            &current.members,
            changes,
            profile,
            &tree_io,
            &mut work,
            tree_live_state,
            &shared_work,
            deadline,
            cancelled,
        )?
    };
    current.members = members;

    // Stage identity deltas in two passes so a moved identity cannot be erased
    // by whichever changed path sorts later. The exact native index supplies
    // current rows; imported authenticated-base SQL supplies old ownership.
    candidate.clear_v2_delta_rows()?;
    let mut changed_after: Option<RelativePath> = None;
    while let Some(path) = candidate.changed_source_after(changed_after.as_ref(), row_allowance)? {
        changed_after = Some(path.clone());
        let mut after_id: Option<String> = None;
        while let Some(id) =
            candidate.v2_base_identity_for_path_after(&path, after_id.as_deref(), row_allowance)?
        {
            after_id = Some(id.clone());
            candidate.put_v2_identity_delta(&id, None)?;
        }
    }
    changed_after = None;
    while let Some(path) = candidate.changed_source_after(changed_after.as_ref(), row_allowance)? {
        changed_after = Some(path.clone());
        let mut after_id: Option<String> = None;
        while let Some(id) = index.identity_for_path_after(&path, after_id.as_deref())? {
            after_id = Some(id.clone());
            candidate.put_v2_identity_delta(&id, Some(&path))?;
        }
    }
    let identities = {
        let mut after: Option<String> = None;
        let changes = std::iter::from_fn(move || {
            let (id, path) =
                match candidate.v2_identity_delta_after(after.as_deref(), row_allowance) {
                    Ok(Some(row)) => row,
                    Ok(None) => return None,
                    Err(error) => return Some(Err(tree_io_error(error))),
                };
            after = Some(id.clone());
            let mut key = Vec::new();
            if key.try_reserve_exact(id.len()).is_err() {
                return Some(Err(tree_error("V2 identity key allocation failed")));
            }
            key.extend_from_slice(id.as_bytes());
            let value = match path {
                Some(path) => {
                    let mut value = Vec::new();
                    if value.try_reserve_exact(path.as_str().len()).is_err() {
                        return Some(Err(tree_error("V2 identity value allocation failed")));
                    }
                    value.extend_from_slice(path.as_str().as_bytes());
                    Some(value)
                }
                None => None,
            };
            counted_delta(&row_count, row_limit, key, value)
        });
        apply_successor_delta(
            &segment,
            &current.identities,
            changes,
            profile,
            &tree_io,
            &mut work,
            tree_live_state,
            &shared_work,
            deadline,
            cancelled,
        )?
    };
    current.identities = identities;

    // Maintain the path-keyed identity index by merging bounded old and new
    // identity cursors for each changed source path. An older root without
    // this secondary index gets one ordered full build from the current
    // candidate; no corpus-sized identity vector is retained.
    let identity_paths = if let Some(old_root) = current.identity_paths.as_ref() {
        let mut changed_after: Option<RelativePath> = None;
        let mut current_path: Option<RelativePath> = None;
        let mut old_after: Option<String> = None;
        let mut new_after: Option<String> = None;
        let mut old_id: Option<String> = None;
        let mut new_id: Option<String> = None;
        let mut old_loaded = false;
        let mut new_loaded = false;
        let mut old_done = false;
        let mut new_done = false;
        let changes = std::iter::from_fn(move || {
            loop {
                if current_path.is_none() {
                    let path = match candidate
                        .changed_source_after(changed_after.as_ref(), row_allowance)
                    {
                        Ok(Some(path)) => path,
                        Ok(None) => return None,
                        Err(error) => return Some(Err(tree_io_error(error))),
                    };
                    changed_after = Some(path.clone());
                    current_path = Some(path);
                    old_after = None;
                    new_after = None;
                    old_id = None;
                    new_id = None;
                    old_loaded = false;
                    new_loaded = false;
                    old_done = false;
                    new_done = false;
                }
                let Some(path) = current_path.as_ref() else {
                    return Some(Err(tree_error("reverse identity path cursor is absent")));
                };
                if !old_loaded && !old_done {
                    match candidate.v2_base_identity_for_path_after(
                        path,
                        old_after.as_deref(),
                        row_allowance,
                    ) {
                        Ok(Some(id)) => {
                            old_after = Some(id.clone());
                            old_id = Some(id);
                        }
                        Ok(None) => old_done = true,
                        Err(error) => return Some(Err(tree_io_error(error))),
                    }
                    old_loaded = true;
                }
                if !new_loaded && !new_done {
                    match index.identity_for_path_after(path, new_after.as_deref()) {
                        Ok(Some(id)) => {
                            new_after = Some(id.clone());
                            new_id = Some(id);
                        }
                        Ok(None) => new_done = true,
                        Err(error) => return Some(Err(tree_io_error(error))),
                    }
                    new_loaded = true;
                }
                if old_id.is_none() && new_id.is_none() && old_done && new_done {
                    current_path = None;
                    continue;
                }
                if old_id
                    .as_deref()
                    .zip(new_id.as_deref())
                    .is_some_and(|(a, b)| a == b)
                {
                    old_id = None;
                    new_id = None;
                    old_loaded = false;
                    new_loaded = false;
                    continue;
                }
                let (id, value, consume_old) = match (old_id.as_deref(), new_id.as_deref()) {
                    (Some(old), Some(new)) if old < new => (old, None, true),
                    (Some(_), Some(new)) => (new, Some(Vec::new()), false),
                    (Some(old), None) => (old, None, true),
                    (None, Some(new)) => (new, Some(Vec::new()), false),
                    (None, None) => {
                        return Some(Err(tree_error(
                            "reverse identity cursor stopped before path EOF",
                        )));
                    }
                };
                let key = match identity_path_tree_key(path, id, profile.tree_limits.max_key_bytes)
                {
                    Ok(key) => key,
                    Err(error) => return Some(Err(tree_io_error(error))),
                };
                if consume_old {
                    old_id = None;
                    old_loaded = false;
                } else {
                    new_id = None;
                    new_loaded = false;
                }
                return counted_delta(&row_count, row_limit, key, value);
            }
        });
        apply_successor_delta(
            &segment,
            old_root,
            changes,
            profile,
            &tree_io,
            &mut work,
            tree_live_state,
            &shared_work,
            deadline,
            cancelled,
        )?
    } else {
        let mut full_rows = row_count.get();
        build_index_identity_paths_v2(
            candidate,
            index,
            &segment,
            profile,
            &tree_io,
            &mut work,
            &mut full_rows,
            tree_live_state,
            &shared_work,
            deadline,
            cancelled,
        )?
    };
    current.identity_paths = Some(identity_paths);

    // Reverse-adjacency candidate rows are indexed by source/target. Remove
    // old outgoing edges first, then merge the native current edge rows; SQL
    // key conflict resolution handles an unchanged edge without global scans.
    changed_after = None;
    while let Some(source) =
        candidate.changed_source_after(changed_after.as_ref(), row_allowance)?
    {
        changed_after = Some(source.clone());
        let mut after_target: Option<RelativePath> = None;
        while let Some(target) = candidate.base_dependency_after(&source, after_target.as_ref())? {
            after_target = Some(target.clone());
            let key = dependency_tree_key(&source, &target, profile.tree_limits.max_key_bytes)?;
            candidate.put_v2_dependency_delta(&key, None)?;
        }
    }
    changed_after = None;
    while let Some(source) =
        candidate.changed_source_after(changed_after.as_ref(), row_allowance)?
    {
        changed_after = Some(source.clone());
        let mut after_target: Option<RelativePath> = None;
        while let Some(target) = index.dependency_after(&source, after_target.as_ref())? {
            after_target = Some(target.clone());
            let key = dependency_tree_key(&source, &target, profile.tree_limits.max_key_bytes)?;
            let value = length_prefixed_pair(&source, &target)?;
            candidate.put_v2_dependency_delta(&key, Some(&value))?;
        }
    }
    let dependencies = {
        let mut after: Option<Vec<u8>> = None;
        let changes = std::iter::from_fn(move || {
            let (key, value) =
                match candidate.v2_dependency_delta_after(after.as_deref(), row_allowance) {
                    Ok(Some(row)) => row,
                    Ok(None) => return None,
                    Err(error) => return Some(Err(tree_io_error(error))),
                };
            after = Some(key.clone());
            counted_delta(&row_count, row_limit, key, value)
        });
        apply_successor_delta(
            &segment,
            &current.dependencies,
            changes,
            profile,
            &tree_io,
            &mut work,
            tree_live_state,
            &shared_work,
            deadline,
            cancelled,
        )?
    };
    current.dependencies = dependencies;

    let new_retirement_count = candidate.new_retirement_count()?;
    if candidate
        .retirement_count()
        .checked_sub(current.retirement_count)
        != Some(new_retirement_count)
    {
        return Err(invalid(
            "V2 appended retirement range differs from selected base",
        ));
    }
    let retirement_start = current.retirement_count;
    let retirements = {
        let mut ordinal = 0u64;
        let changes = std::iter::from_fn(move || {
            if ordinal >= new_retirement_count {
                return None;
            }
            let index = ordinal;
            ordinal += 1;
            let row = match candidate.new_retirement_at_bounded(index, row_allowance) {
                Ok(Some(row)) => row,
                Ok(None) => return Some(Err(tree_error("V2 retirement row ended early"))),
                Err(error) => return Some(Err(tree_io_error(error))),
            };
            let key_ordinal = match retirement_start.checked_add(index) {
                Some(ordinal) => ordinal,
                None => return Some(Err(tree_error("V2 retirement ordinal overflow"))),
            };
            let entry = match encode_retirement_entry(key_ordinal, row) {
                Ok(entry) => entry,
                Err(error) => return Some(Err(error)),
            };
            counted_delta(&row_count, row_limit, entry.key, Some(entry.value))
        });
        apply_successor_delta(
            &segment,
            &current.retirements,
            changes,
            profile,
            &tree_io,
            &mut work,
            tree_live_state,
            &shared_work,
            deadline,
            cancelled,
        )?
    };
    current.retirements = retirements;

    let desired_object_count = candidate.desired_object_count()?;
    let object_limits = packed_object_limits(
        profile,
        &segment,
        work,
        row_count.get(),
        desired_object_count,
        tree_live_state,
        &shared_work,
        current.objects.is_some(),
    )?;
    let object_work_budget = shared_work.clone();
    let mut object_debit_work = move || object_work_budget.charge_many(1).is_ok();
    let (objects, object_work) = if let Some(old_objects) = current.objects.as_ref() {
        PackedObjectWriterV2::apply_delta(
            &segment,
            old_objects,
            candidate.packed_object_changes(),
            object_limits,
            tree_io.clone(),
            work,
            deadline,
            cancelled,
            &mut object_debit_work,
        )?
    } else {
        PackedObjectWriterV2::build(
            &segment,
            candidate.packed_object_sources(),
            object_limits,
            tree_io.clone(),
            deadline,
            cancelled,
            &mut object_debit_work,
        )?
    };
    if current.objects.is_some() {
        work = object_work.tree_work;
    } else {
        add_tree_work(&mut work, object_work.tree_work, profile.tree_limits)?;
    }
    row_count.set(
        row_count
            .get()
            .checked_add(object_work.object_rows)
            .filter(|rows| *rows <= row_limit)
            .ok_or_else(|| invalid("V2 packed object delta row profile exceeded"))?,
    );
    current.objects = Some(objects);

    let fence = index.fence();
    let (member_count, source_bytes) = candidate.membership_counts();
    let retirement_count = current
        .retirement_count
        .checked_add(new_retirement_count)
        .ok_or_else(|| invalid("V2 successor retirement count overflow"))?;
    if member_count != fence.membership.count
        || source_bytes != fence.source_bytes
        || retirement_count != candidate.retirement_count()
        || current.members.entries != member_count
    {
        return Err(invalid("V2 successor logical counts differ from candidate"));
    }
    current.revision = selected_base;
    current.base_revision = Some(selected_base);
    current.validator_sha256 = fence.validator_sha256;
    current.manifest_sha256 = Digest256::of_bytes(&[]);
    current.source_artifact = SourceRevisionArtifactV2::CompactCommitV2 {
        sha256: current.manifest_sha256,
        bytes: 1,
    };
    current.batch_sha256 = Some(candidate.batch_sha256());
    current.membership_v1 = Some(fence.membership);
    current.membership_v2 = Some(SourceMembershipV2::from_members_tree(
        member_count,
        source_bytes,
        current.members.commitment,
    ));
    current.completion_proof = Some(index.completion_proof()?);
    current.source_bytes = source_bytes;
    current.member_count = member_count;
    current.identity_count = index.identity_count();
    current.dependency_source_count = index.dependency_source_count();
    current.dependency_count = index.dependency_count();
    current.retirement_count = retirement_count;

    let commit_live_state = base_live_state
        .checked_add(builder_state)
        .ok_or_else(|| invalid("V2 compact commit live state overflow"))?;
    let (current, commit, source_record) = CompactCommitV2::seal_successor(
        current,
        candidate.batch_sha256(),
        profile.max_working_state_bytes,
        commit_live_state,
    )?;
    drop(commit);
    current.validate_store_binding(segment.store_id(), segment.domain_digest())?;
    let source_record_sha256 = Digest256::of_bytes(&source_record);
    if current.source_artifact.sha256() != source_record_sha256
        || current.source_artifact.bytes() != Some(source_record.len() as u64)
    {
        return Err(invalid("V2 compact source artifact binding differs"));
    }

    let row_live_state = base_live_state
        .checked_add(builder_state)
        .and_then(|bytes| bytes.checked_add(source_record.capacity()))
        .ok_or_else(|| invalid("V2 current history row live state overflow"))?;
    let current_row =
        current.encode_with_state_limit(profile.max_working_state_bytes, row_live_state)?;
    let expected_history_row_bytes = current_row.len();
    let expected_history_row_sha256 = Digest256::of_bytes(&current_row);
    let history_additional_live = row_live_state
        .checked_add(current.retained_state_bytes()?)
        .and_then(|bytes| bytes.checked_add(current_row.capacity()))
        .ok_or_else(|| invalid("V2 history append state overflow"))?;
    let mut current_row = Some(current_row);
    let revision = current.revision;
    let history_changes = std::iter::from_fn(move || {
        let value = current_row.take()?;
        counted_delta(
            &row_count,
            row_limit,
            revision.0.as_bytes().to_vec(),
            Some(value),
        )
    });
    let history = apply_successor_delta(
        &segment,
        &selected_roots.history,
        history_changes,
        profile,
        &tree_io,
        &mut work,
        history_additional_live,
        &shared_work,
        deadline,
        cancelled,
    )?;
    if history.entries
        != selected_roots
            .history
            .entries
            .checked_add(1)
            .ok_or_else(|| invalid("V2 history row count overflow"))?
    {
        return Err(invalid("V2 successor history append count differs"));
    }
    let roots = SourceRootSetV2 { current, history };
    roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;

    // The change bytes are moved into the COW iterator. A digest/length pair
    // remains as a finite equality witness for the authenticated history read.
    let roots_live_state = roots.retained_state_bytes()?;
    let history_read_node_state = profile
        .tree_limits
        .max_node_bytes
        .checked_mul(64)
        .and_then(|bytes| {
            profile
                .tree_limits
                .max_value_bytes
                .checked_mul(4)
                .and_then(|value| bytes.checked_add(value))
        })
        .ok_or_else(|| invalid("V2 history read scratch state overflow"))?;
    let history_read_extra = base_live_state
        .checked_add(builder_state)
        .and_then(|bytes| bytes.checked_add(source_record.capacity()))
        .and_then(|bytes| bytes.checked_add(roots_live_state))
        .and_then(|bytes| bytes.checked_add(history_read_node_state))
        .ok_or_else(|| invalid("V2 history read state overflow"))?;
    if history_read_extra > profile.max_working_state_bytes {
        return Err(invalid("V2 history read exceeds source state profile"));
    }
    let work_budget = shared_work.clone();
    let mut debit_work = move || work_budget.charge_many(1).is_ok();
    let (history_row, read_work) = segment
        .lookup_authenticated_tree_v2_with_work_and_io_and_callback(
            &roots.history,
            revision.0.as_bytes(),
            remaining_tree_limits(profile.tree_limits, work)?,
            Some(tree_io.clone()),
            deadline,
            cancelled,
            &mut debit_work,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    add_tree_work(&mut work, read_work, profile.tree_limits)?;
    let history_row = history_row.ok_or_else(|| invalid("V2 successor history row is absent"))?;
    if history_row.len() != expected_history_row_bytes
        || Digest256::of_bytes(&history_row) != expected_history_row_sha256
    {
        return Err(invalid("V2 successor current history row differs"));
    }
    drop(history_row);
    base.verify_current_fence()?;
    index.verify_candidate()?;

    let rootset_live_state = base_live_state
        .checked_add(builder_state)
        .and_then(|bytes| bytes.checked_add(source_record.capacity()))
        .ok_or_else(|| invalid("V2 rootset encoding state overflow"))?;
    let bytes =
        roots.encode_with_state_limit(profile.max_working_state_bytes, rootset_live_state)?;
    let sha256 = Digest256::of_bytes(&bytes);
    let result = BuiltSuccessorRootSetV2 {
        expected_base: selected_base,
        expected_previous_rootset_sha256: Some(previous_rootset_sha256),
        expected_selection,
        roots,
        bytes,
        sha256,
        source_record,
        tree_io,
        segment_store: segment,
        work,
    };
    Ok(result)
}

fn counted_delta(
    count: &Cell<u64>,
    maximum: u64,
    key: Vec<u8>,
    value: Option<Vec<u8>>,
) -> Option<tos_segment_store::Result<AuthenticatedTreeDeltaV1>> {
    let next = match count.get().checked_add(1) {
        Some(next) if next <= maximum => next,
        _ => return Some(Err(tree_error("V2 cumulative delta row limit exceeded"))),
    };
    count.set(next);
    Some(Ok(AuthenticatedTreeDeltaV1 { key, value }))
}

fn apply_successor_delta<I>(
    segment: &SegmentStore,
    old: &AuthenticatedTreeDescriptorV2,
    changes: I,
    profile: &super::source_foundation_admission::NativeSegmentV2Budget,
    tree_io: &Arc<NativeV2TreeIo>,
    work: &mut AuthenticatedTreeWorkV1,
    additional_live_state_bytes: usize,
    shared_work: &AdmissionWorkBudget,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> io::Result<AuthenticatedTreeDescriptorV2>
where
    I: IntoIterator<Item = tos_segment_store::Result<AuthenticatedTreeDeltaV1>>,
{
    if additional_live_state_bytes > profile.max_working_state_bytes {
        return Err(invalid("V2 COW live state exceeds source profile"));
    }
    let io_ledger: Arc<dyn AuthenticatedTreeIoLedgerV1> = tree_io.clone();
    let work_budget = shared_work.clone();
    let mut debit_work = move || work_budget.charge_many(1).is_ok();
    let (next, cumulative_work) = segment
        .apply_authenticated_tree_delta_v2_with_work_and_io_and_state_cumulative_and_callback(
            old,
            changes,
            profile.tree_limits,
            Some(io_ledger),
            *work,
            profile.max_working_state_bytes,
            additional_live_state_bytes,
            deadline,
            cancelled,
            &mut debit_work,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    *work = cumulative_work;
    Ok(next)
}

fn dependency_tree_key(
    source: &RelativePath,
    target: &RelativePath,
    maximum: usize,
) -> io::Result<Vec<u8>> {
    let length = source
        .as_str()
        .len()
        .checked_add(1)
        .and_then(|length| length.checked_add(target.as_str().len()))
        .filter(|length| *length <= maximum)
        .ok_or_else(|| invalid("V2 dependency key exceeds profile"))?;
    let mut key = Vec::new();
    key.try_reserve_exact(length)
        .map_err(|_| invalid("V2 dependency key allocation failed"))?;
    key.extend_from_slice(source.as_str().as_bytes());
    key.push(0);
    key.extend_from_slice(target.as_str().as_bytes());
    Ok(key)
}

fn identity_path_tree_key(path: &RelativePath, id: &str, maximum: usize) -> io::Result<Vec<u8>> {
    if id.is_empty() || id.as_bytes().contains(&0) {
        return Err(invalid("V2 reverse identity identifier is invalid"));
    }
    let length = path
        .as_str()
        .len()
        .checked_add(1)
        .and_then(|length| length.checked_add(id.len()))
        .filter(|length| *length <= maximum)
        .ok_or_else(|| invalid("V2 reverse identity key exceeds profile"))?;
    let mut key = Vec::new();
    key.try_reserve_exact(length)
        .map_err(|_| invalid("V2 reverse identity key allocation failed"))?;
    key.extend_from_slice(path.as_str().as_bytes());
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    Ok(key)
}

fn remaining_tree_limits(
    base: AuthenticatedTreeLimitsV1,
    used: AuthenticatedTreeWorkV1,
) -> io::Result<AuthenticatedTreeLimitsV1> {
    let mut remaining = base;
    let used_nodes = used
        .read_nodes
        .checked_add(used.written_nodes)
        .ok_or_else(|| invalid("V2 cumulative tree node overflow"))?;
    let used_bytes = used
        .read_bytes
        .checked_add(used.written_bytes)
        .ok_or_else(|| invalid("V2 cumulative tree byte overflow"))?;
    remaining.max_nodes = base
        .max_nodes
        .checked_sub(used_nodes)
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("V2 cumulative tree node profile exceeded"))?;
    remaining.max_total_bytes = base
        .max_total_bytes
        .checked_sub(used_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("V2 cumulative tree byte profile exceeded"))?;
    Ok(remaining)
}

fn add_tree_work(
    used: &mut AuthenticatedTreeWorkV1,
    next: AuthenticatedTreeWorkV1,
    limits: AuthenticatedTreeLimitsV1,
) -> io::Result<()> {
    used.read_nodes = used
        .read_nodes
        .checked_add(next.read_nodes)
        .ok_or_else(|| invalid("V2 cumulative tree node overflow"))?;
    used.written_nodes = used
        .written_nodes
        .checked_add(next.written_nodes)
        .ok_or_else(|| invalid("V2 cumulative tree node overflow"))?;
    used.read_bytes = used
        .read_bytes
        .checked_add(next.read_bytes)
        .ok_or_else(|| invalid("V2 cumulative tree byte overflow"))?;
    used.written_bytes = used
        .written_bytes
        .checked_add(next.written_bytes)
        .ok_or_else(|| invalid("V2 cumulative tree byte overflow"))?;
    used.allocated_bytes = used
        .allocated_bytes
        .checked_add(next.allocated_bytes)
        .ok_or_else(|| invalid("V2 cumulative allocation overflow"))?;
    if used
        .read_nodes
        .checked_add(used.written_nodes)
        .is_none_or(|n| n > limits.max_nodes)
        || used
            .read_bytes
            .checked_add(used.written_bytes)
            .is_none_or(|n| n > limits.max_total_bytes)
    {
        return Err(invalid("V2 cumulative tree work exceeded"));
    }
    Ok(())
}

fn length_prefixed_pair(source: &RelativePath, target: &RelativePath) -> io::Result<Vec<u8>> {
    let source_len = u32::try_from(source.as_str().len())
        .map_err(|_| invalid("V2 dependency source exceeds range"))?;
    let target_len = u32::try_from(target.as_str().len())
        .map_err(|_| invalid("V2 dependency target exceeds range"))?;
    let cap = 8usize
        .checked_add(source.as_str().len())
        .and_then(|n| n.checked_add(target.as_str().len()))
        .ok_or_else(|| invalid("V2 dependency pair size overflow"))?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(cap)
        .map_err(|_| invalid("V2 dependency pair allocation failed"))?;
    result.extend_from_slice(&source_len.to_be_bytes());
    result.extend_from_slice(source.as_str().as_bytes());
    result.extend_from_slice(&target_len.to_be_bytes());
    result.extend_from_slice(target.as_str().as_bytes());
    Ok(result)
}

fn encode_retirement_entry(
    ordinal: u64,
    row: tos_source_store::RetirementMetadata,
) -> tos_segment_store::Result<AuthenticatedTreeEntryV1> {
    let mut value = Vec::new();
    let path_len = u32::try_from(row.path.as_str().len())
        .map_err(|_| tree_error("V2 retirement path exceeds range"))?;
    let event_len = u32::try_from(row.event_ref.as_str().len())
        .map_err(|_| tree_error("V2 retirement event path exceeds range"))?;
    let capacity = 4usize
        .checked_add(row.path.as_str().len())
        .and_then(|n| n.checked_add(32 + 4))
        .and_then(|n| n.checked_add(row.event_ref.as_str().len()))
        .and_then(|n| n.checked_add(32 + 8))
        .ok_or_else(|| tree_error("V2 retirement tuple size overflow"))?;
    value
        .try_reserve_exact(capacity)
        .map_err(|_| tree_error("V2 retirement tuple allocation failed"))?;
    value.extend_from_slice(&path_len.to_be_bytes());
    value.extend_from_slice(row.path.as_str().as_bytes());
    value.extend_from_slice(row.sha256.as_bytes());
    value.extend_from_slice(&event_len.to_be_bytes());
    value.extend_from_slice(row.event_ref.as_str().as_bytes());
    value.extend_from_slice(row.event_sha256.as_bytes());
    value.extend_from_slice(&row.event_size_bytes.to_be_bytes());
    Ok(AuthenticatedTreeEntryV1 {
        key: ordinal.to_be_bytes().to_vec(),
        value,
    })
}

fn tree_error(message: &'static str) -> tos_segment_store::SegmentError {
    tos_segment_store::SegmentError::new(
        tos_segment_store::SegmentErrorCode::InvalidFormat,
        message,
    )
}

fn tree_io_error(error: io::Error) -> tos_segment_store::SegmentError {
    tos_segment_store::SegmentError::io("CMD source cursor failed while building V2 roots", error)
}

// The CMD owner exposes io::Result, retaining the complete segment refusal
// as its cause rather than converting it into a source-cursor failure.
