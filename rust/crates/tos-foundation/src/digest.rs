use sha2::{Digest, Sha256};

use crate::error::{FoundationError, FoundationErrorCode, Result};

/// Exact SHA-256 of the bytes supplied by the caller. The digest is not a read grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Digest256([u8; 32]);

impl Digest256 {
    /// Construct an exact digest value from an already decoded 32-byte field.
    /// The caller is responsible for verifying the bytes that the digest names.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Digest256Hasher::new();
        hasher.update(bytes);
        hasher.finalize()
    }

    pub fn from_hex(value: &str) -> Result<Self> {
        let raw = value.as_bytes();
        if raw.len() != 64 {
            return Err(FoundationError::new(
                FoundationErrorCode::InvalidDigest,
                "expected 64 lowercase hex digits",
            ));
        }
        let mut result = [0u8; 32];
        if !crate::decode_lower_hex_into(value, &mut result) {
            return Err(FoundationError::new(
                FoundationErrorCode::InvalidDigest,
                "expected lowercase hex",
            ));
        }
        Ok(Self(result))
    }

    pub fn from_prefixed(value: &str) -> Result<Self> {
        let bare = value.strip_prefix("sha256:").ok_or_else(|| {
            FoundationError::new(
                FoundationErrorCode::InvalidDigest,
                "expected sha256: prefix",
            )
        })?;
        Self::from_hex(bare)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        crate::encode_lower_hex(&self.0)
    }

    pub fn to_prefixed(self) -> String {
        format!("sha256:{}", self.to_hex())
    }
}

#[derive(Clone, Default)]
pub struct Digest256Hasher(Sha256);

impl Digest256Hasher {
    pub fn new() -> Self {
        Self(Sha256::new())
    }
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    pub fn finalize(self) -> Digest256 {
        Digest256(self.0.finalize().into())
    }
}
