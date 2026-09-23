use std::num::NonZeroU64;

use crate::digest::Digest256;
use crate::error::{FoundationError, FoundationErrorCode, Result};

/// Opaque, spelling-preserving identity. Its owner descriptor supplies kind grammar.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StableId(String);

impl StableId {
    pub fn parse(value: &str) -> Result<Self> {
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(FoundationError::new(FoundationErrorCode::InvalidIdentifier, "empty or control-bearing identity"));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str { &self.0 }
    pub fn into_string(self) -> String { self.0 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RecordVersion(NonZeroU64);

impl RecordVersion {
    pub fn new(value: u64) -> Result<Self> {
        NonZeroU64::new(value).map(Self).ok_or_else(|| {
            FoundationError::new(FoundationErrorCode::InvalidVersion, "record version must be positive")
        })
    }
    pub const fn get(self) -> u64 { self.0.get() }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ExactRecordRef {
    pub id: StableId,
    pub version: RecordVersion,
    pub digest: Digest256,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SourceRevision(pub Digest256);
