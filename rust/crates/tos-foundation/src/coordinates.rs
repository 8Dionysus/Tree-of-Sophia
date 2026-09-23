use crate::digest::Digest256;
use crate::error::{FoundationError, FoundationErrorCode, Result};

/// Half-open byte offsets in one exact byte representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteSpan { pub start: u64, pub end: u64 }

impl ByteSpan {
    pub fn new(start: u64, end: u64) -> Result<Self> {
        if start > end { return Err(FoundationError::new(FoundationErrorCode::InvalidCoordinate, "end precedes start")); }
        Ok(Self { start, end })
    }
}

/// Half-open Unicode code-point offsets in a named, digest-bound text representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodePointSpan {
    pub start: u64,
    pub end: u64,
    pub representation_digest: Digest256,
    pub normalization_id: String,
}

impl CodePointSpan {
    pub fn new(start: u64, end: u64, representation_digest: Digest256, normalization_id: &str) -> Result<Self> {
        if start > end || normalization_id.is_empty() || normalization_id.chars().any(char::is_control) {
            return Err(FoundationError::new(FoundationErrorCode::InvalidCoordinate, "invalid code-point span or normalization id"));
        }
        Ok(Self { start, end, representation_digest, normalization_id: normalization_id.to_owned() })
    }

    /// Map a code-point interval to UTF-8 byte offsets in the exact declared representation.
    /// No normalization or source assessment happens here.
    pub fn byte_span_in(&self, representation: &str) -> Result<ByteSpan> {
        if Digest256::of_bytes(representation.as_bytes()) != self.representation_digest {
            return Err(FoundationError::new(FoundationErrorCode::InvalidCoordinate, "representation digest differs"));
        }
        let total = representation.chars().count() as u64;
        if self.end > total {
            return Err(FoundationError::new(FoundationErrorCode::InvalidCoordinate, "code-point interval exceeds representation"));
        }
        let byte_at = |index: u64| -> u64 {
            if index == total { representation.len() as u64 }
            else { representation.char_indices().nth(index as usize).map_or(0, |(byte, _)| byte as u64) }
        };
        ByteSpan::new(byte_at(self.start), byte_at(self.end))
    }
}
