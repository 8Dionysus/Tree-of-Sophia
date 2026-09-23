//! Read-only current-format ToS corpus snapshots.
//!
//! This crate verifies mechanical exactness. Source meaning, rights, admission,
//! publication and current-use authority remain with their owners.

mod error;
mod limits;
mod manifest;
mod object;

pub use error::{Result, StoreError, StoreErrorCode};
pub use limits::ReadLimits;
pub use manifest::{
    CorpusDescriptor, CorpusReader, MemberMetadata, RetirementMetadata, Selector, Snapshot,
};
