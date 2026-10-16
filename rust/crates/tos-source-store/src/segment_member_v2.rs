//! Mechanical MemberPath-to-ObjectDigest resolution over typed V2 roots.
//!
//! The selected rootset, current/history epoch, admission, rights, and native
//! publication fences remain with the caller. A successful lookup here does
//! not establish those authorities.

use std::{
    io::{Cursor, Write},
    mem::size_of,
};

use tos_foundation::RelativePath;

use crate::{
    error::{Result, StoreError, StoreErrorCode},
    segment_index_read_v2::{
        SegmentIndexReaderV2, SegmentLocatedValueV2, validate_object_digest_value,
    },
    segment_locator::SegmentIndexKeySpaceV1,
    segment_member::{
        SEGMENT_MEMBER_VALUE_BYTES_V1, SegmentMemberMetadataV1, decode_segment_member_value,
    },
};

/// Paired typed readers for a caller-selected rootset. Role and shared request
/// identity are checked here; rootset selection and epoch admission are not.
pub struct SegmentMemberReaderV2 {
    members: SegmentIndexReaderV2,
    objects: SegmentIndexReaderV2,
}

impl SegmentMemberReaderV2 {
    pub fn new(
        members: SegmentIndexReaderV2,
        objects: SegmentIndexReaderV2,
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
                "V2 member roots do not share roles and request identity",
            ));
        }
        let required = Self::workspace_upper_bound()?;
        if max_workspace_bytes == 0
            || max_workspace_bytes == usize::MAX
            || max_workspace_bytes < required
        {
            return Err(refusal("V2 paired member workspace exceeds caller limit"));
        }
        members.check_request()?;
        objects.check_request()?;
        Ok(Self { members, objects })
    }

    /// Includes both actual V2 reader bounds, this pair, the exact 52-byte
    /// metadata buffer, and overlapping decode/value/cursor state. Caller path,
    /// sink, retained outputs, and native proof remain separately budgeted.
    pub fn workspace_upper_bound() -> Result<usize> {
        let readers = SegmentIndexReaderV2::workspace_upper_bound()?
            .checked_mul(2)
            .ok_or_else(|| refusal("V2 paired reader workspace overflows"))?;
        [
            readers,
            size_of::<Self>(),
            SEGMENT_MEMBER_VALUE_BYTES_V1,
            size_of::<SegmentMemberMetadataV1>(),
            size_of::<SegmentLocatedValueV2>(),
            size_of::<Cursor<&'static mut [u8]>>(),
            4096,
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
        .ok_or_else(|| refusal("V2 paired member workspace overflows"))
    }

    /// Read and decode one exact MemberPath metadata row. Its logical and
    /// physical representations must both be the unchanged 52-byte value.
    pub fn member(&self, path: &RelativePath) -> Result<Option<SegmentMemberMetadataV1>> {
        self.read_member_metadata(path)
    }

    /// Resolve the separate ObjectDigest extent only after the exact 40-byte
    /// logical `(length, raw SHA-256)` tuple and raw extent descriptor agree
    /// with MemberPath. Keep `sink` private until this returns successfully.
    pub fn read_exact(
        &self,
        path: &RelativePath,
        sink: &mut impl Write,
    ) -> Result<Option<SegmentMemberMetadataV1>> {
        self.members.check_request()?;
        self.objects.check_request()?;
        let Some(metadata) = self.read_member_metadata(path)? else {
            return Ok(None);
        };
        let object_value = self
            .objects
            .get(metadata.raw_sha256.as_bytes())?
            .ok_or_else(|| corrupt("V2 member object digest is absent"))?;
        validate_object_digest_value(metadata.raw_sha256.as_bytes(), object_value)?;
        if object_value.value_extent.length != metadata.size_bytes
            || object_value.physical_value_sha256 != metadata.raw_sha256
        {
            return Err(corrupt("V2 member and object metadata differ"));
        }
        let returned = self.objects.read_located_value(object_value, sink)?;
        if returned != metadata.size_bytes {
            return Err(corrupt("V2 member raw length differs"));
        }
        self.members.check_request()?;
        self.objects.check_request()?;
        Ok(Some(metadata))
    }

    fn read_member_metadata(&self, path: &RelativePath) -> Result<Option<SegmentMemberMetadataV1>> {
        self.members.check_request()?;
        let Some(value) = self.members.get(path.as_str().as_bytes())? else {
            self.members.check_request()?;
            self.objects.check_request()?;
            return Ok(None);
        };
        if value.value_extent.length != SEGMENT_MEMBER_VALUE_BYTES_V1 as u64
            || value.logical_value_bytes != SEGMENT_MEMBER_VALUE_BYTES_V1 as u64
            || value.logical_value_sha256 != value.physical_value_sha256
        {
            return Err(corrupt(
                "V2 member metadata logical or physical shape differs",
            ));
        }
        let mut raw = [0u8; SEGMENT_MEMBER_VALUE_BYTES_V1];
        let mut sink = Cursor::new(raw.as_mut_slice());
        let returned = self.members.read_located_value(value, &mut sink)?;
        if returned != SEGMENT_MEMBER_VALUE_BYTES_V1 as u64
            || sink.position() != SEGMENT_MEMBER_VALUE_BYTES_V1 as u64
        {
            return Err(corrupt("V2 member metadata read length differs"));
        }
        let metadata = decode_segment_member_value(&raw)?;
        self.members.check_request()?;
        self.objects.check_request()?;
        Ok(Some(metadata))
    }
}

fn refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn corrupt(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::CorruptSelectedObject, detail)
}
