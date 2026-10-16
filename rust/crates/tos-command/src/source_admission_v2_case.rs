//! Actual bounded V2 read -> cold image -> fresh restored read consumer.
//! The writer/Native owner supplies the earned revision and original budgets.
//! This consumer creates no admission token and does not synthesize history.
use super::source_admission::{AdmissionWorkBudget, active, invalid};
use super::source_admission_v2_backup_restore::{
    V2HeldImageRoots, V2HeldSourceRoot, V2HeldTargetRoot, V2ImageLimits, V2ImageReceipt,
    open_selected_target, transfer_image_with_cold_spill_at, verify_named_root,
};
use super::source_admission_v2_seen_pack::V2SeenPackSpillRequests;
use std::{
    fs::File,
    io,
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::native_v2::reader::{V2PointReadLimits, V2ReadSession};
use tos_source_store::{
    PinnedSqliteIoBudget, PinnedSqliteSpaceBudget, PinnedSqliteSpaceReservation,
};

#[derive(Clone, Copy)]
pub struct V2CaseLimits {
    pub point: V2PointReadLimits,
    pub image: V2ImageLimits,
    /// Original whole-case ceilings. The two point sessions and the image
    /// receive disjoint work slices; a fresh session does not reset these.
    pub max_total_tree_nodes: u64,
    pub max_total_tree_bytes: u64,
    pub max_state_bytes: usize,
}
impl V2CaseLimits {
    fn validate(self) -> io::Result<Self> {
        let nodes = self
            .point
            .tree
            .max_nodes
            .checked_mul(2)
            .and_then(|n| n.checked_add(self.image.tree.max_nodes))
            .ok_or_else(|| invalid("V2 case node slice overflow"))?;
        let bytes = self
            .point
            .tree
            .max_total_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(self.image.tree.max_total_bytes))
            .ok_or_else(|| invalid("V2 case byte slice overflow"))?;
        if self.max_total_tree_nodes == 0
            || self.max_total_tree_nodes == u64::MAX
            || self.max_total_tree_bytes == 0
            || self.max_total_tree_bytes == u64::MAX
            || nodes > self.max_total_tree_nodes
            || bytes > self.max_total_tree_bytes
            || self.max_state_bytes == 0
            || self.max_state_bytes == usize::MAX
            || self
                .point
                .max_state_bytes
                .checked_add(8192)
                .filter(|n| *n <= self.max_state_bytes)
                .is_none()
            || self
                .image
                .max_state_bytes
                .checked_add(8192)
                .filter(|n| *n <= self.max_state_bytes)
                .is_none()
        {
            return Err(invalid("V2 case whole resource slices differ"));
        }
        Ok(self)
    }

    fn validate_cold_spill(self) -> io::Result<Self> {
        let limits = self.validate()?;
        let overhead = cold_case_phase_overhead_bytes();
        if self
            .point
            .max_state_bytes
            .checked_add(overhead)
            .is_none_or(|bytes| bytes > self.max_state_bytes)
            || self
                .image
                .max_state_bytes
                .checked_add(overhead)
                .is_none_or(|bytes| bytes > self.max_state_bytes)
        {
            return Err(invalid("V2 cold spill binding exceeds whole-case state"));
        }
        Ok(limits)
    }
}

pub const fn cold_case_phase_overhead_bytes() -> usize {
    8192 + std::mem::size_of::<ColdCaseResources<'static>>()
        + std::mem::size_of::<super::source_admission_v2_backup_restore::V2ImageOutcome>()
        + std::mem::size_of::<V2CaseReceipt>()
        + std::mem::size_of::<V2CaseOutcome>()
}

pub struct V2CaseReceipt {
    pub image: V2ImageReceipt,
    pub observed_revision: SourceRevision,
    pub observed_path: RelativePath,
    pub observed_sha256: Digest256,
    pub observed_bytes: u64,
}
/// After any named restore write, success AND refusal return the same held
/// reservation. The owning invocation must retain it until terminal cleanup
/// or a real baseline handoff, even if the final restored read refuses.
pub struct V2CaseOutcome {
    pub result: io::Result<V2CaseReceipt>,
    pub custody: Arc<PinnedSqliteSpaceReservation>,
}

struct ColdCaseResources<'a> {
    auxiliary_space: &'a PinnedSqliteSpaceBudget,
    requests: V2SeenPackSpillRequests,
    held_roots: V2HeldImageRoots<'a>,
    shared_work: AdmissionWorkBudget,
}

#[allow(clippy::too_many_arguments)]
pub fn read_backup_restore_case_with_cold_spill_at(
    source_path: &Path,
    source_held: &File,
    source_identity: (u64, u64),
    artifact_root_held: &File,
    artifact_root_path: &Path,
    artifact_root_identity: (u64, u64),
    selected_target_path: &Path,
    selected_target_relative: &RelativePath,
    expected_revision: SourceRevision,
    identity: &str,
    expected_path: &RelativePath,
    limits: V2CaseLimits,
    original_io: PinnedSqliteIoBudget,
    persistent_space: &PinnedSqliteSpaceBudget,
    original_auxiliary_space: &PinnedSqliteSpaceBudget,
    requests: V2SeenPackSpillRequests,
    shared_work: AdmissionWorkBudget,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
) -> io::Result<V2CaseOutcome> {
    let limits = limits.validate_cold_spill()?;
    // Source, image and SQLite reserve their own live allocations. They may
    // share the original aggregate envelope: this keeps their combined usage
    // under the selected ceiling instead of minting an independent allowance.
    requests.validate_for_operation(&original_io, original_auxiliary_space, deadline, &cancel)?;
    let _profiles = limits.image.validate_cold_spill(&requests)?;
    verify_named_root(
        source_path,
        source_held,
        source_identity,
        &original_io,
        deadline,
        &cancel,
    )?;
    let target = V2HeldTargetRoot {
        artifact_path: artifact_root_path,
        artifact_held: artifact_root_held,
        artifact_identity: artifact_root_identity,
        store_path: selected_target_path,
        store_relative: selected_target_relative,
    };
    let target_root = open_selected_target(target, None, &original_io, deadline, &cancel)?;
    let target_metadata = target_root.metadata()?;
    let roots = V2HeldImageRoots {
        source: V2HeldSourceRoot {
            path: source_path,
            held: source_held,
            identity: source_identity,
        },
        target,
        target_store_identity: (target_metadata.dev(), target_metadata.ino()),
    };
    drop(target_root);
    read_backup_restore_case_inner(
        source_path,
        selected_target_path,
        expected_revision,
        identity,
        expected_path,
        limits,
        original_io,
        persistent_space,
        deadline,
        cancel,
        ColdCaseResources {
            auxiliary_space: original_auxiliary_space,
            requests,
            held_roots: roots,
            shared_work,
        },
    )
}

fn read_backup_restore_case_inner(
    source: &Path,
    fresh_target: &Path,
    expected_revision: SourceRevision,
    identity: &str,
    expected_path: &RelativePath,
    limits: V2CaseLimits,
    original_io: PinnedSqliteIoBudget,
    persistent_space: &PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    resources: ColdCaseResources<'_>,
) -> io::Result<V2CaseOutcome> {
    let limits = limits.validate_cold_spill()?;
    active(deadline, &cancel)?;
    let ColdCaseResources {
        auxiliary_space,
        requests,
        held_roots: roots,
        shared_work,
    } = resources;
    let (digest, bytes, mode) = {
        verify_named_root(
            roots.source.path,
            roots.source.held,
            roots.source.identity,
            &original_io,
            deadline,
            &cancel,
        )?;
        let mut session = V2ReadSession::open_at_named_with_work(
            source,
            roots.source.held,
            limits.point,
            original_io.clone(),
            shared_work.clone(),
            deadline,
            cancel.clone(),
        )?;
        if session.selected_revision() != expected_revision {
            return Err(invalid("V2 case writer revision differs"));
        }
        let observed = session
            .read_identity(expected_revision, identity)?
            .ok_or_else(|| invalid("V2 case genuine source identity absent"))?;
        if &observed.path != expected_path || observed.revision != expected_revision {
            return Err(invalid("V2 case source identity binding differs"));
        }
        let tuple = (observed.sha256, observed.size_bytes, observed.source_mode);
        session.verify_current_fence()?;
        tuple
    }; // Drop the actual source buffer/session before cold closure/copy.
    verify_named_root(
        roots.source.path,
        roots.source.held,
        roots.source.identity,
        &original_io,
        deadline,
        &cancel,
    )?;
    let image = transfer_image_with_cold_spill_at(
        source,
        roots.source.held,
        roots.source.identity,
        roots.target,
        roots.target_store_identity,
        limits.image,
        &original_io,
        persistent_space,
        auxiliary_space,
        requests,
        shared_work.clone(),
        deadline,
        &cancel,
    )?;
    let image_result = image.result;
    let custody = image.custody;
    let held_target_root = image.held_target_root;
    let result = (|| {
        let image_receipt = image_result?;
        if image_receipt.selection.revision != expected_revision {
            return Err(invalid("V2 case source advanced before cold image"));
        }
        let target_root = held_target_root
            .as_ref()
            .ok_or_else(|| invalid("V2 restored held target descriptor absent"))?;
        let _current_target = open_selected_target(
            roots.target,
            Some(roots.target_store_identity),
            &original_io,
            deadline,
            &cancel,
        )?;
        let mut restored = V2ReadSession::open_at_named_with_work(
            fresh_target,
            target_root,
            limits.point,
            original_io.clone(),
            shared_work.clone(),
            deadline,
            cancel.clone(),
        )?;
        if restored.selected_revision() != expected_revision {
            return Err(invalid("V2 case fresh restored revision differs"));
        }
        let observed = restored
            .read_identity(expected_revision, identity)?
            .ok_or_else(|| invalid("V2 case restored identity absent"))?;
        if &observed.path != expected_path
            || observed.revision != expected_revision
            || observed.sha256 != digest
            || observed.size_bytes != bytes
            || observed.source_mode != mode
        {
            return Err(invalid("V2 case real restored observation differs"));
        }
        restored.verify_current_fence()?;
        active(deadline, &cancel)?;
        Ok(V2CaseReceipt {
            image: image_receipt,
            observed_revision: expected_revision,
            observed_path: expected_path.clone(),
            observed_sha256: digest,
            observed_bytes: bytes,
        })
    })();
    let result = if held_target_root.is_some() {
        let source_fence = verify_named_root(
            roots.source.path,
            roots.source.held,
            roots.source.identity,
            &original_io,
            deadline,
            &cancel,
        );
        let target_fence = open_selected_target(
            roots.target,
            Some(roots.target_store_identity),
            &original_io,
            deadline,
            &cancel,
        )
        .map(|_| ());
        match result {
            Ok(receipt) => source_fence.and(target_fence).map(|_| receipt),
            Err(error) => Err(error),
        }
    } else {
        result
    };
    Ok(V2CaseOutcome { result, custody })
}
