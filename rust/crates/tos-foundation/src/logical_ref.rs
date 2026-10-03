//! Versioned pure logical identity framing. Physical placement and authority
//! remain with the storage and command owners.

use crate::digest::Digest256;
use crate::error::{FoundationError, FoundationErrorCode as Code, Result};

const MAGIC: &[u8; 4] = b"TOSL";
const VERSION: u16 = 1;
const MAX_SMALL: usize = 255;
const MAX_LARGE: usize = 4096;
const MAX_FRAME: usize = 4 + 2 + (2 + MAX_SMALL) * 3 + (4 + MAX_LARGE) * 2 + 32 + 8;

/// Opaque exact owner identity plus content binding. Bytes are never parsed as
/// UTF-8, a path, a ToS kind, or a grant by this codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalRecordRefV1 {
    domain: Vec<u8>,
    profile_id: Vec<u8>,
    profile_version: Vec<u8>,
    subject: Vec<u8>,
    revision_token: Vec<u8>,
    content_sha256: Digest256,
    content_length: u64,
}

impl LogicalRecordRefV1 {
    pub const PROFILE: &'static str = "tos_logical_record_ref_v1";

    pub fn new(
        domain: &[u8],
        profile_id: &[u8],
        profile_version: &[u8],
        subject: &[u8],
        revision_token: &[u8],
        content_sha256: Digest256,
        content_length: u64,
    ) -> Result<Self> {
        for field in [domain, profile_id, profile_version] {
            check_field(field, MAX_SMALL)?;
        }
        for field in [subject, revision_token] {
            check_field(field, MAX_LARGE)?;
        }
        Ok(Self {
            domain: domain.to_vec(),
            profile_id: profile_id.to_vec(),
            profile_version: profile_version.to_vec(),
            subject: subject.to_vec(),
            revision_token: revision_token.to_vec(),
            content_sha256,
            content_length,
        })
    }

    pub fn domain(&self) -> &[u8] {
        &self.domain
    }
    pub fn profile_id(&self) -> &[u8] {
        &self.profile_id
    }
    pub fn profile_version(&self) -> &[u8] {
        &self.profile_version
    }
    pub fn subject(&self) -> &[u8] {
        &self.subject
    }
    pub fn revision_token(&self) -> &[u8] {
        &self.revision_token
    }
    pub fn content_sha256(&self) -> Digest256 {
        self.content_sha256
    }
    pub fn content_length(&self) -> u64 {
        self.content_length
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            4 + 2
                + 2
                + self.domain.len()
                + 2
                + self.profile_id.len()
                + 2
                + self.profile_version.len()
                + 4
                + self.subject.len()
                + 4
                + self.revision_token.len()
                + 32
                + 8,
        );
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        for field in [&self.domain, &self.profile_id, &self.profile_version] {
            bytes.extend_from_slice(&(field.len() as u16).to_le_bytes());
            bytes.extend_from_slice(field);
        }
        for field in [&self.subject, &self.revision_token] {
            bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
            bytes.extend_from_slice(field);
        }
        bytes.extend_from_slice(self.content_sha256.as_bytes());
        bytes.extend_from_slice(&self.content_length.to_le_bytes());
        bytes
    }

    pub fn digest(&self) -> Digest256 {
        Digest256::of_bytes(&self.encode())
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_FRAME {
            return Err(FoundationError::new(
                Code::BudgetExceeded,
                "logical reference frame exceeds v1 limit",
            ));
        }
        let mut cursor = Cursor { bytes, position: 0 };
        if cursor.take(4)? != MAGIC {
            return Err(FoundationError::new(
                Code::InvalidFrame,
                "logical reference magic differs",
            ));
        }
        let version = u16::from_le_bytes(cursor.take(2)?.try_into().expect("two bytes"));
        if version != VERSION {
            return Err(FoundationError::new(
                Code::UnsupportedFormat,
                "unknown logical reference version",
            ));
        }
        let domain = cursor.small_field()?;
        let profile_id = cursor.small_field()?;
        let profile_version = cursor.small_field()?;
        let subject = cursor.large_field()?;
        let revision_token = cursor.large_field()?;
        let content_sha256 =
            Digest256::from_bytes(cursor.take(32)?.try_into().expect("digest bytes"));
        let content_length = u64::from_le_bytes(cursor.take(8)?.try_into().expect("eight bytes"));
        if cursor.position != bytes.len() {
            return Err(FoundationError::new(
                Code::InvalidFrame,
                "trailing logical reference bytes",
            ));
        }
        Self::new(
            domain,
            profile_id,
            profile_version,
            subject,
            revision_token,
            content_sha256,
            content_length,
        )
    }
}

fn check_field(field: &[u8], max: usize) -> Result<()> {
    if field.is_empty() || field.len() > max {
        return Err(FoundationError::new(
            Code::InvalidFrame,
            "logical reference field length is out of range",
        ));
    }
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(len).ok_or_else(|| {
            FoundationError::new(Code::InvalidFrame, "logical reference length overflow")
        })?;
        let field = self.bytes.get(self.position..end).ok_or_else(|| {
            FoundationError::new(Code::InvalidFrame, "truncated logical reference frame")
        })?;
        self.position = end;
        Ok(field)
    }

    fn small_field(&mut self) -> Result<&'a [u8]> {
        let len = u16::from_le_bytes(self.take(2)?.try_into().expect("two bytes")) as usize;
        if len == 0 || len > MAX_SMALL {
            return Err(FoundationError::new(
                Code::InvalidFrame,
                "logical reference small field length is out of range",
            ));
        }
        self.take(len)
    }

    fn large_field(&mut self) -> Result<&'a [u8]> {
        let len = u32::from_le_bytes(self.take(4)?.try_into().expect("four bytes")) as usize;
        if len == 0 || len > MAX_LARGE {
            return Err(FoundationError::new(
                Code::InvalidFrame,
                "logical reference large field length is out of range",
            ));
        }
        self.take(len)
    }
}
