use sha2::{Digest, Sha256};

use crate::error::{FoundationError, FoundationErrorCode, Result};

/// Exact SHA-256 of the bytes supplied by the caller. The digest is not a read grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Digest256([u8; 32]);

impl Digest256 {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Digest256Hasher::new();
        hasher.update(bytes);
        hasher.finalize()
    }

    pub fn from_hex(value: &str) -> Result<Self> {
        let raw = value.as_bytes();
        if raw.len() != 64 {
            return Err(FoundationError::new(FoundationErrorCode::InvalidDigest, "expected 64 lowercase hex digits"));
        }
        let mut result = [0u8; 32];
        for (i, pair) in raw.chunks_exact(2).enumerate() {
            result[i] = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
        }
        Ok(Self(result))
    }

    pub fn from_prefixed(value: &str) -> Result<Self> {
        let bare = value.strip_prefix("sha256:").ok_or_else(|| {
            FoundationError::new(FoundationErrorCode::InvalidDigest, "expected sha256: prefix")
        })?;
        Self::from_hex(bare)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }

    pub fn to_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(64);
        for byte in self.0 {
            text.push(HEX[(byte >> 4) as usize] as char);
            text.push(HEX[(byte & 15) as usize] as char);
        }
        text
    }

    pub fn to_prefixed(self) -> String { format!("sha256:{}", self.to_hex()) }
}

fn hex_digit(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(FoundationError::new(FoundationErrorCode::InvalidDigest, "expected lowercase hex")),
    }
}

#[derive(Clone, Default)]
pub struct Digest256Hasher(Sha256);

impl Digest256Hasher {
    pub fn new() -> Self { Self(Sha256::new()) }
    pub fn update(&mut self, bytes: &[u8]) { self.0.update(bytes); }
    pub fn finalize(self) -> Digest256 { Digest256(self.0.finalize().into()) }
}
