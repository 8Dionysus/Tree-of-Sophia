//! Actual bounded V2 read -> cold image -> fresh restored read consumer.
//! The writer/Native owner supplies the earned revision and original budgets.
//! This consumer creates no admission token and does not synthesize history.
use super::source_admission::{active, invalid};
use super::source_admission_v2_backup_restore::{
    V2ImageLimits, V2ImageReceipt, transfer_image, transfer_image_with_cold_spill,
};
use super::source_admission_v2_reader::{V2PointReadLimits, V2ReadSession};
use super::source_admission_v2_seen_pack::V2SeenPackSpillRequests;
use std::{
    io,
    mem::size_of,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, RelativePath, SourceRevision};
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
        let paired_request_state = size_of::<ImageMode<'static>>();
        if self
            .point
            .max_state_bytes
            .checked_add(paired_request_state)
            .and_then(|bytes| bytes.checked_add(8192))
            .is_none_or(|bytes| bytes > self.max_state_bytes)
        {
            return Err(invalid(
                "V2 cold spill requests exceed whole-case point state",
            ));
        }
        Ok(limits)
    }
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

enum ImageMode<'a> {
    Compatibility,
    ColdSpill {
        auxiliary_space: &'a PinnedSqliteSpaceBudget,
        requests: V2SeenPackSpillRequests,
    },
}

pub fn read_backup_restore_case(
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
) -> io::Result<V2CaseOutcome> {
    let limits = limits.validate()?;
    read_backup_restore_case_inner(
        source,
        fresh_target,
        expected_revision,
        identity,
        expected_path,
        limits,
        original_io,
        persistent_space,
        deadline,
        cancel,
        ImageMode::Compatibility,
    )
}

/// Bounded image route using separately held auxiliary scratch for source and
/// fresh-target cold closure. The caller supplies both original requests.
pub fn read_backup_restore_case_with_cold_spill(
    source: &Path,
    fresh_target: &Path,
    expected_revision: SourceRevision,
    identity: &str,
    expected_path: &RelativePath,
    limits: V2CaseLimits,
    original_io: PinnedSqliteIoBudget,
    persistent_space: &PinnedSqliteSpaceBudget,
    original_auxiliary_space: &PinnedSqliteSpaceBudget,
    requests: V2SeenPackSpillRequests,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
) -> io::Result<V2CaseOutcome> {
    let limits = limits.validate_cold_spill()?;
    if original_auxiliary_space.shares_with(persistent_space) {
        return Err(invalid(
            "V2 cold spill auxiliary space must be separately held",
        ));
    }
    requests.validate_for_operation(&original_io, original_auxiliary_space, deadline, &cancel)?;
    let _profiles = limits.image.validate_cold_spill(&requests)?;
    read_backup_restore_case_inner(
        source,
        fresh_target,
        expected_revision,
        identity,
        expected_path,
        limits,
        original_io,
        persistent_space,
        deadline,
        cancel,
        ImageMode::ColdSpill {
            auxiliary_space: original_auxiliary_space,
            requests,
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
    image_mode: ImageMode<'_>,
) -> io::Result<V2CaseOutcome> {
    let limits = match &image_mode {
        ImageMode::Compatibility => limits.validate()?,
        ImageMode::ColdSpill { .. } => limits.validate_cold_spill()?,
    };
    active(deadline, &cancel)?;
    let (digest, bytes, mode) = {
        let mut session = V2ReadSession::open(
            source,
            limits.point,
            original_io.clone(),
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
    let image = match image_mode {
        ImageMode::Compatibility => transfer_image(
            source,
            fresh_target,
            limits.image,
            &original_io,
            persistent_space,
            deadline,
            &cancel,
        )?,
        ImageMode::ColdSpill {
            auxiliary_space,
            requests,
        } => transfer_image_with_cold_spill(
            source,
            fresh_target,
            limits.image,
            &original_io,
            persistent_space,
            auxiliary_space,
            requests,
            deadline,
            &cancel,
        )?,
    };
    let image_result = image.result;
    let custody = image.custody;
    let result = (|| {
        let image_receipt = image_result?;
        if image_receipt.selection.revision != expected_revision {
            return Err(invalid("V2 case source advanced before cold image"));
        }
        let mut restored = V2ReadSession::open(
            fresh_target,
            limits.point,
            original_io,
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
    Ok(V2CaseOutcome { result, custody })
}
