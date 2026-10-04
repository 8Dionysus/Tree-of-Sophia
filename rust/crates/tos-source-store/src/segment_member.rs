//! Mechanical member-to-object resolution for a selected V2 root pair.
//! Neither this decoder nor the public root DTOs establish native admission,
//! semantic completion, currentness, rights or historical selection.

use crate::{Result, SegmentIndexKeySpaceV1, SegmentIndexReaderV1, StoreError, StoreErrorCode};
use std::io::{Cursor, Write};
use tos_foundation::{Digest256, RelativePath};

pub const SEGMENT_MEMBER_VALUE_BYTES_V1: usize = 52;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentMemberMetadataV1 {
    pub size_bytes: u64,
    pub raw_sha256: Digest256,
    pub source_mode: u32,
}

/// Decode the exact native MemberPath value. Physical relocation is absent
/// from this value; the object-digest root separately selects its raw extent.
pub fn decode_segment_member_value(raw: &[u8]) -> Result<SegmentMemberMetadataV1> {
    if raw.len() != SEGMENT_MEMBER_VALUE_BYTES_V1 || &raw[..8] != b"TOSMEMV1" {
        return Err(corrupt("segment member metadata wire differs"));
    }
    let mut size = [0u8; 8];
    size.copy_from_slice(&raw[8..16]);
    let mut sha = [0u8; 32];
    sha.copy_from_slice(&raw[16..48]);
    let mut mode = [0u8; 4];
    mode.copy_from_slice(&raw[48..52]);
    let metadata = SegmentMemberMetadataV1 {
        size_bytes: u64::from_le_bytes(size),
        raw_sha256: Digest256::from_bytes(sha),
        source_mode: u32::from_le_bytes(mode),
    };
    if !matches!(metadata.source_mode, 0o600 | 0o644 | 0o755)
        || (metadata.size_bytes == 0 && metadata.raw_sha256 != Digest256::of_bytes(&[]))
    {
        return Err(corrupt(
            "segment member source mode or empty digest differs",
        ));
    }
    Ok(metadata)
}

pub struct SegmentMemberReaderV1 {
    members: SegmentIndexReaderV1,
    objects: SegmentIndexReaderV1,
}

impl SegmentMemberReaderV1 {
    /// Pair only mechanically role-correct roots on the SAME held request.
    /// A native admitted rootset/current-selector fence remains caller-owned.
    pub fn new(
        members: SegmentIndexReaderV1,
        objects: SegmentIndexReaderV1,
        max_workspace_bytes: usize,
    ) -> Result<Self> {
        members.check_request()?;
        objects.check_request()?;
        if members.keyspace() != SegmentIndexKeySpaceV1::MemberPath
            || objects.keyspace() != SegmentIndexKeySpaceV1::ObjectDigest
            || !members.shares_reader_request(&objects)
        {
            return Err(StoreError::new(
                StoreErrorCode::DescriptorMismatch,
                "segment member roots do not share roles and original request",
            ));
        }
        if max_workspace_bytes == usize::MAX || max_workspace_bytes < Self::workspace_upper_bound()?
        {
            return Err(StoreError::new(
                StoreErrorCode::BudgetExceeded,
                "segment member paired workspace exceeds caller limit",
            ));
        }
        Ok(Self { members, objects })
    }

    /// Conservative logical simultaneous reader state, not allocator/RSS or
    /// shared filesystem SPACE. Caller path/sink/native proof are additional.
    pub fn workspace_upper_bound() -> Result<usize> {
        SegmentIndexReaderV1::workspace_upper_bound()?
            .checked_mul(2)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .and_then(|n| n.checked_add(SEGMENT_MEMBER_VALUE_BYTES_V1))
            .ok_or_else(|| {
                StoreError::new(
                    StoreErrorCode::BudgetExceeded,
                    "segment member workspace overflow",
                )
            })
    }

    pub fn member(&self, path: &RelativePath) -> Result<Option<SegmentMemberMetadataV1>> {
        let Some(value) = self.members.get(path.as_str().as_bytes())? else {
            return Ok(None);
        };
        if value.value_extent.length != SEGMENT_MEMBER_VALUE_BYTES_V1 as u64 {
            return Err(corrupt("segment member metadata extent length differs"));
        }
        let mut raw = [0u8; SEGMENT_MEMBER_VALUE_BYTES_V1];
        let mut sink = Cursor::new(raw.as_mut_slice());
        self.members.read_located_value(value, &mut sink)?;
        if sink.position() != SEGMENT_MEMBER_VALUE_BYTES_V1 as u64 {
            return Err(corrupt("segment member metadata read length differs"));
        }
        let metadata = decode_segment_member_value(&raw)?;
        self.members.check_request()?;
        Ok(Some(metadata))
    }

    /// The caller must keep the sink private until success and preserve its
    /// selected admitted-root/currentness/rights fences. No virtual file or
    /// mode transformation is created; source_mode is returned exactly.
    pub fn read_exact(
        &self,
        path: &RelativePath,
        sink: &mut impl Write,
    ) -> Result<Option<SegmentMemberMetadataV1>> {
        let Some(metadata) = self.member(path)? else {
            return Ok(None);
        };
        let value = self
            .objects
            .get(metadata.raw_sha256.as_bytes())?
            .ok_or_else(|| corrupt("segment member object digest is absent"))?;
        if value.value_extent.length != metadata.size_bytes
            || value.value_sha256 != metadata.raw_sha256
        {
            return Err(corrupt("segment member and object metadata differ"));
        }
        let returned = self.objects.read_located_value(value, sink)?;
        if returned != metadata.size_bytes {
            return Err(corrupt("segment member raw length differs"));
        }
        Ok(Some(metadata))
    }
}

fn corrupt(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::CorruptSelectedObject, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value(size: u64, sha: Digest256, mode: u32) -> [u8; 52] {
        let mut raw = [0u8; 52];
        raw[..8].copy_from_slice(b"TOSMEMV1");
        raw[8..16].copy_from_slice(&size.to_le_bytes());
        raw[16..48].copy_from_slice(sha.as_bytes());
        raw[48..].copy_from_slice(&mode.to_le_bytes());
        raw
    }
    #[test]
    fn private_source_mode_and_empty_raw_digest_are_preserved() {
        let sha = Digest256::of_bytes(&[]);
        assert_eq!(
            decode_segment_member_value(&value(0, sha, 0o600)).unwrap(),
            SegmentMemberMetadataV1 {
                size_bytes: 0,
                raw_sha256: sha,
                source_mode: 0o600
            }
        );
    }
    #[test]
    fn malformed_width_mode_and_empty_digest_are_refused() {
        let raw = value(0, Digest256::of_bytes(b"nonempty"), 0o644);
        assert!(decode_segment_member_value(&raw).is_err());
        assert!(decode_segment_member_value(&raw[..51]).is_err());
        assert!(decode_segment_member_value(&value(1, Digest256::of_bytes(b"a"), 0o666)).is_err());
    }
}
