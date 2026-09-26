//! Read-only current-format ToS corpus snapshots.
//!
//! This crate verifies mechanical exactness. Source meaning, rights, admission,
//! publication and current-use authority remain with their owners.

mod cut;
mod error;
mod limits;
mod manifest;
mod object;
mod software;
mod secure_open;

pub use cut::{
    CorpusCutReader, CutReadLimits, RetiredSourceMemberV1, SourceMemberStreamV1, SourceMemberV1,
    SourceMembershipV1, SourcePresenceV1,
};
pub use error::{Result, StoreError, StoreErrorCode};
pub use limits::ReadLimits;
pub use software::{SoftwareCaptureReader, SoftwareCaptureSelectionV1, SOFTWARE_COMPANION_PROFILE_V1};
pub use manifest::{
    CorpusDescriptor, CorpusReader, MemberMetadata, RetirementMetadata, Selector, Snapshot,
};
