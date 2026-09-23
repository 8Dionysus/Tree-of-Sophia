//! Bounded immutable segment bytes for a future ToS source coordinator.
//!
//! This native Linux crate provides no source admission, owner-version
//! assignment, rights grant, public byte route, compaction or garbage collector.
//! A receipt is trusted only while held as this crate's private constructed
//! handle or after cold recovery verifies its pinned journal and actual bytes.

#[cfg(not(target_os = "linux"))]
compile_error!("tos-segment-store currently supports Linux only");

mod error;
mod format;
mod journal;
mod store;

pub use error::{Result, SegmentError, SegmentErrorCode};
pub use format::{FrameCoordinate, SegmentLimits};
pub use store::{ByteDurabilityReceipt, DurabilityClass, FrameInput, OwnerBinding, SegmentStore};
