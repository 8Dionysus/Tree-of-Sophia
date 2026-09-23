//! Bounded immutable segment bytes for a future ToS source coordinator.
//!
//! This native Linux crate provides no source admission, owner-version
//! assignment, rights grant, public byte route, compaction or garbage collector.
//! A receipt is trusted only while held as this crate's private constructed
//! handle or after cold recovery verifies its pinned journal and actual bytes.

#[cfg(not(target_os = "linux"))]
compile_error!("tos-segment-store currently supports Linux only");

mod audit;
mod error;
mod format;
mod generation;
mod journal;
mod packed_leaf;
mod placement;
mod store;

pub use audit::{
    PlacementAuditLimits, PlacementAuditRow, PlacementComparison, compare_placement_streams,
};
pub use error::{Result, SegmentError, SegmentErrorCode};
pub use format::{FrameCoordinate, SegmentLimits};
pub use generation::{
    GenerationShapeLimits, GenerationStreamComparison, KeyComparatorV1, PackedPartitionRefV1,
    PartitionBoundsV1, PlacementGenerationRowV1, PlacementPartitionV1, compare_generation_streams,
    describe_placement_partition, placement_catalog_shape_root,
};
pub use packed_leaf::PackedPlacementLeafV1;
pub use placement::PlacementV1;
pub use store::{
    AttemptRecovery, ByteDurabilityReceipt, DurabilityClass, FrameInput, OwnerBinding,
    SegmentStore, VerificationBudget, VerifiedSealGuard,
};
