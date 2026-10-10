//! Bounded immutable segment bytes for a future ToS source coordinator.
//!
//! This native Linux crate provides no source admission, owner-version
//! assignment, rights grant, public byte route, compaction or garbage collector.
//! A receipt is trusted only while held as this crate's private constructed
//! handle or after cold recovery verifies its pinned journal and actual bytes.

#[cfg(not(target_os = "linux"))]
compile_error!("tos-segment-store currently supports Linux only");

mod authenticated_pack_set_v2;
mod authenticated_tree;
mod error;
mod format;
mod generation;
mod journal;
mod packed_leaf;
mod placement;
mod selected;
mod store;

pub use authenticated_pack_set_v2::AuthenticatedTreePackSetV2;
pub use authenticated_tree::{
    AuthenticatedTreeCoverageV1, AuthenticatedTreeDeltaV1, AuthenticatedTreeDescriptorV1,
    AuthenticatedTreeDescriptorV2, AuthenticatedTreeEntryV1, AuthenticatedTreeIoLedgerV1,
    AuthenticatedTreeLimitsV1, AuthenticatedTreeLocatorV2, AuthenticatedTreeNodeRefV1,
    AuthenticatedTreeRowStreamV1, AuthenticatedTreeRowStreamV2, AuthenticatedTreeWorkV1,
    decode_placement_tree_row, encode_placement_tree_row,
};
pub use error::{Result, SegmentError, SegmentErrorCode};
pub use format::{FrameCoordinate, SegmentLimits};
pub use generation::{
    GenerationShapeLimits, KeyComparatorV1, PackedPartitionRefV1, PartitionBoundsV1,
    PlacementGenerationRowV1, PlacementPartitionV1, describe_placement_partition,
    placement_catalog_shape_root,
};
pub use packed_leaf::PackedPlacementLeafV1;
pub use placement::PlacementV1;
pub use selected::{
    GenerationCatalogV1, GenerationCoverageV1, GenerationCutV1, GenerationDescriptorV1,
    GenerationNamespaceV1, GenerationReadLimits, GenerationRowStreamV1, InstalledGenerationV1,
};
pub use store::{
    AttemptRecovery, AuditedStoreRoot, ByteDurabilityReceipt, DurabilityClass, FrameInput,
    OwnerBinding, SegmentOperationLimitsV1, SegmentOperationWorkV1, SegmentStore,
    VerificationBudget, VerifiedSealGuard,
};
