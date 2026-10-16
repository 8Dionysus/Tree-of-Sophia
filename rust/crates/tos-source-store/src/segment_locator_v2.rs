//! Bounded borrowed codec for physical and logical V2 segment-index pages.
//!
//! A checked page and its root DTO remain mechanical evidence. They do not
//! select or admit a root, prove initial whole-tree closure, or establish
//! freshness/history. Callers own typed-row checks, selected-root authority,
//! full closure before first publication, and parent-to-child summary/boundary
//! comparisons on later point reads.

use tos_foundation::{Digest256, Digest256Hasher};

use crate::{
    Result, SEGMENT_INDEX_KEY_MAX_BYTES_V1, SEGMENT_INDEX_PAGE_HEADER_BYTES_V1,
    SEGMENT_INDEX_PAGE_MAX_BYTES_V1, SegmentExtentV1, SegmentIndexKeySpaceV1,
    SegmentIndexPageKindV1, SegmentIndexPageLimitsV1, SegmentIndexSummaryV1, StoreError,
    StoreErrorCode,
};

pub const SEGMENT_INDEX_PAGE_HEADER_BYTES_V2: usize = SEGMENT_INDEX_PAGE_HEADER_BYTES_V1;
pub const SEGMENT_INDEX_PAGE_MAX_BYTES_V2: usize = SEGMENT_INDEX_PAGE_MAX_BYTES_V1;
pub const SEGMENT_INDEX_KEY_MAX_BYTES_V2: usize = SEGMENT_INDEX_KEY_MAX_BYTES_V1;

const PAGE_MAGIC: &[u8; 8] = b"TOSLPAGE";
const PAGE_VERSION: u16 = 2;
const LEAF: u8 = 0;
const INTERNAL: u8 = 1;
const DIGEST_BYTES: usize = 32;
const LOGICAL_DOMAIN: &[u8] = b"TOS_NATIVE_LOGICAL_NODE_V2\0";

/// Physical page and subtree limits plus finite aggregate logical limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexPageLimitsV2 {
    pub physical: SegmentIndexPageLimitsV1,
    pub max_logical_key_bytes: u64,
    pub max_logical_value_bytes: u64,
}

impl SegmentIndexPageLimitsV2 {
    fn validate(self) -> Result<Self> {
        let p = self.physical;
        if !(SEGMENT_INDEX_PAGE_HEADER_BYTES_V2..=SEGMENT_INDEX_PAGE_MAX_BYTES_V2)
            .contains(&p.max_page_bytes)
            || p.max_entries == u32::MAX
            || p.max_key_bytes == 0
            || p.max_key_bytes > SEGMENT_INDEX_KEY_MAX_BYTES_V2
            || p.max_segment_bytes == 0
            || p.max_segment_bytes == u64::MAX
            || p.max_row_count == u64::MAX
            || p.max_value_bytes == u64::MAX
            || p.max_encoded_page_bytes < SEGMENT_INDEX_PAGE_HEADER_BYTES_V2 as u64
            || p.max_encoded_page_bytes == u64::MAX
            || self.max_logical_key_bytes == u64::MAX
            || self.max_logical_value_bytes == u64::MAX
        {
            return Err(StoreError::new(
                StoreErrorCode::DescriptorMismatch,
                "segment-index V2 limits are invalid",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexLogicalSummaryV2 {
    pub row_count: u64,
    pub key_bytes: u64,
    pub value_bytes: u64,
    pub logical_sha256: Digest256,
}

/// Mechanical V2 root descriptor; construction does not select a root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexRootV2 {
    pub keyspace: SegmentIndexKeySpaceV1,
    pub root_extent: SegmentExtentV1,
    pub root_page_sha256: Digest256,
    pub physical_summary: SegmentIndexSummaryV1,
    pub logical_summary: SegmentIndexLogicalSummaryV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexLeafEntryV2<'a> {
    pub key: &'a [u8],
    pub value_extent: SegmentExtentV1,
    pub physical_value_sha256: Digest256,
    pub logical_value_bytes: u64,
    pub logical_value_sha256: Digest256,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexInternalEntryV2<'a> {
    pub lower_key: &'a [u8],
    pub upper_key: &'a [u8],
    pub child_extent: SegmentExtentV1,
    pub child_page_sha256: Digest256,
    pub physical_summary: SegmentIndexSummaryV1,
    pub logical_summary: SegmentIndexLogicalSummaryV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SegmentIndexEntryV2<'a> {
    Leaf(SegmentIndexLeafEntryV2<'a>),
    Internal(SegmentIndexInternalEntryV2<'a>),
}

/// A V2 page authenticated and decoded without materializing its entries.
#[derive(Clone, Copy, Debug)]
pub struct DecodedSegmentIndexPageV2<'a> {
    raw_page: &'a [u8],
    page_sha256: Digest256,
    keyspace: SegmentIndexKeySpaceV1,
    kind: SegmentIndexPageKindV1,
    entry_count: u32,
    physical_summary: SegmentIndexSummaryV1,
    logical_summary: SegmentIndexLogicalSummaryV2,
    first_key: Option<&'a [u8]>,
    last_key: Option<&'a [u8]>,
    limits: SegmentIndexPageLimitsV2,
}

/// Opaque sequential position in one checked V2 page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SegmentIndexEntryCursorV2 {
    byte_offset: usize,
    ordinal: u32,
    page_sha256: Digest256,
    page_len: usize,
}

impl<'a> DecodedSegmentIndexPageV2<'a> {
    pub const fn keyspace(&self) -> SegmentIndexKeySpaceV1 {
        self.keyspace
    }

    pub const fn kind(&self) -> SegmentIndexPageKindV1 {
        self.kind
    }

    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }

    pub const fn physical_summary(&self) -> SegmentIndexSummaryV1 {
        self.physical_summary
    }

    pub const fn logical_summary(&self) -> SegmentIndexLogicalSummaryV2 {
        self.logical_summary
    }

    pub const fn first_key(&self) -> Option<&'a [u8]> {
        self.first_key
    }

    pub const fn last_key(&self) -> Option<&'a [u8]> {
        self.last_key
    }

    pub(crate) fn initial_cursor(&self) -> SegmentIndexEntryCursorV2 {
        SegmentIndexEntryCursorV2 {
            byte_offset: SEGMENT_INDEX_PAGE_HEADER_BYTES_V2,
            ordinal: 0,
            page_sha256: self.page_sha256,
            page_len: self.raw_page.len(),
        }
    }

    pub(crate) fn next_entry(
        &self,
        cursor: &mut SegmentIndexEntryCursorV2,
    ) -> Result<Option<SegmentIndexEntryV2<'a>>> {
        self.validate_cursor(cursor)?;
        if cursor.ordinal == self.entry_count {
            return Ok(None);
        }
        let mut offset = cursor.byte_offset;
        let entry = decode_entry(self.raw_page, &mut offset, self.kind)?;
        let ordinal = cursor
            .ordinal
            .checked_add(1)
            .ok_or_else(|| cursor_error("segment-index V2 cursor ordinal overflows"))?;
        if offset <= cursor.byte_offset
            || offset > self.raw_page.len()
            || (ordinal == self.entry_count && offset != self.raw_page.len())
            || (ordinal < self.entry_count && offset == self.raw_page.len())
        {
            return Err(cursor_error(
                "segment-index V2 cursor is not at a checked entry boundary",
            ));
        }
        cursor.byte_offset = offset;
        cursor.ordinal = ordinal;
        Ok(Some(entry))
    }

    fn validate_cursor(&self, cursor: &SegmentIndexEntryCursorV2) -> Result<()> {
        if cursor.page_sha256 != self.page_sha256 || cursor.page_len != self.raw_page.len() {
            return Err(cursor_error(
                "segment-index V2 cursor belongs to a different page",
            ));
        }
        if cursor.ordinal > self.entry_count
            || cursor.byte_offset < SEGMENT_INDEX_PAGE_HEADER_BYTES_V2
            || cursor.byte_offset > self.raw_page.len()
            || (cursor.ordinal == 0 && cursor.byte_offset != SEGMENT_INDEX_PAGE_HEADER_BYTES_V2)
            || (cursor.ordinal == self.entry_count && cursor.byte_offset != self.raw_page.len())
            || (cursor.ordinal < self.entry_count && cursor.byte_offset == self.raw_page.len())
        {
            return Err(cursor_error(
                "segment-index V2 cursor offset or ordinal is invalid",
            ));
        }
        Ok(())
    }

    pub fn entry_at(&self, ordinal: u32) -> Result<Option<SegmentIndexEntryV2<'a>>> {
        if ordinal >= self.entry_count {
            return Ok(None);
        }
        let mut offset = SEGMENT_INDEX_PAGE_HEADER_BYTES_V2;
        for _ in 0..ordinal {
            let _ = decode_entry(self.raw_page, &mut offset, self.kind)?;
        }
        decode_entry(self.raw_page, &mut offset, self.kind).map(Some)
    }

    pub fn next_key_after(&self, key: &[u8]) -> Result<Option<&'a [u8]>> {
        self.validate_lookup_key(key)?;
        let mut offset = SEGMENT_INDEX_PAGE_HEADER_BYTES_V2;
        for _ in 0..self.entry_count {
            let entry = decode_entry(self.raw_page, &mut offset, self.kind)?;
            if entry_lower_key(entry) > key {
                return Ok(Some(entry_lower_key(entry)));
            }
        }
        Ok(None)
    }

    pub fn find_leaf(&self, key: &[u8]) -> Result<Option<SegmentIndexLeafEntryV2<'a>>> {
        self.validate_lookup_key(key)?;
        if self.kind != SegmentIndexPageKindV1::Leaf {
            return Ok(None);
        }
        let mut offset = SEGMENT_INDEX_PAGE_HEADER_BYTES_V2;
        for _ in 0..self.entry_count {
            let entry = decode_entry(self.raw_page, &mut offset, self.kind)?;
            let SegmentIndexEntryV2::Leaf(leaf) = entry else {
                return Err(corrupt("V2 leaf page contains an internal entry"));
            };
            match leaf.key.cmp(key) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => return Ok(Some(leaf)),
                std::cmp::Ordering::Greater => return Ok(None),
            }
        }
        Ok(None)
    }

    pub fn child_for_key(&self, key: &[u8]) -> Result<Option<SegmentIndexInternalEntryV2<'a>>> {
        self.validate_lookup_key(key)?;
        if self.kind != SegmentIndexPageKindV1::Internal {
            return Ok(None);
        }
        let mut offset = SEGMENT_INDEX_PAGE_HEADER_BYTES_V2;
        for _ in 0..self.entry_count {
            let entry = decode_entry(self.raw_page, &mut offset, self.kind)?;
            let SegmentIndexEntryV2::Internal(internal) = entry else {
                return Err(corrupt("V2 internal page contains a leaf entry"));
            };
            if key < internal.lower_key {
                return Ok(None);
            }
            if key <= internal.upper_key {
                return Ok(Some(internal));
            }
        }
        Ok(None)
    }

    fn validate_lookup_key(&self, key: &[u8]) -> Result<()> {
        if key.is_empty() || key.len() > self.limits.physical.max_key_bytes {
            return Err(budget("V2 lookup key is empty or exceeds its byte limit"));
        }
        if self.keyspace == SegmentIndexKeySpaceV1::ObjectDigest && key.len() != DIGEST_BYTES {
            return Err(corrupt("ObjectDigest lookup key must contain 32 bytes"));
        }
        Ok(())
    }
}

pub fn decode_segment_index_page_v2(
    raw_page: &[u8],
    expected_page_sha: Digest256,
    expected_keyspace: SegmentIndexKeySpaceV1,
    limits: SegmentIndexPageLimitsV2,
) -> Result<DecodedSegmentIndexPageV2<'_>> {
    let limits = limits.validate()?;
    if raw_page.len() > limits.physical.max_page_bytes {
        return Err(budget("V2 page exceeds its byte limit"));
    }
    if raw_page.len() < SEGMENT_INDEX_PAGE_HEADER_BYTES_V2 {
        return Err(corrupt("V2 page header is truncated"));
    }
    if Digest256::of_bytes(raw_page) != expected_page_sha {
        return Err(corrupt("V2 page digest does not match"));
    }
    if raw_page.get(..8) != Some(&PAGE_MAGIC[..]) {
        return Err(corrupt("V2 page magic is invalid"));
    }
    let version = read_u16_at(raw_page, 8)?;
    if version != PAGE_VERSION {
        return Err(StoreError::new(
            StoreErrorCode::UnsupportedFormat,
            "segment-index page version is not V2",
        ));
    }
    let kind = match raw_page[10] {
        LEAF => SegmentIndexPageKindV1::Leaf,
        INTERNAL => SegmentIndexPageKindV1::Internal,
        _ => {
            return Err(StoreError::new(
                StoreErrorCode::UnsupportedFormat,
                "segment-index V2 page kind is unsupported",
            ));
        }
    };
    let keyspace = keyspace_from_tag(raw_page[11]).ok_or_else(|| {
        StoreError::new(
            StoreErrorCode::UnsupportedFormat,
            "segment-index V2 keyspace is unsupported",
        )
    })?;
    if keyspace != expected_keyspace {
        return Err(corrupt("V2 page keyspace does not match"));
    }
    let entry_count = read_u32_at(raw_page, 12)?;
    if entry_count > limits.physical.max_entries {
        return Err(budget("V2 entry count exceeds its limit"));
    }
    if kind == SegmentIndexPageKindV1::Internal && entry_count == 0 {
        return Err(corrupt("V2 internal page must contain a child"));
    }

    let (physical_summary, mut logical_summary, first_key, last_key) =
        validate_page(raw_page, keyspace, kind, entry_count, limits)?;
    logical_summary.logical_sha256 =
        logical_digest(raw_page, keyspace, kind, entry_count, logical_summary)?;
    Ok(DecodedSegmentIndexPageV2 {
        raw_page,
        page_sha256: expected_page_sha,
        keyspace,
        kind,
        entry_count,
        physical_summary,
        logical_summary,
        first_key,
        last_key,
        limits,
    })
}

fn validate_page<'a>(
    raw_page: &'a [u8],
    keyspace: SegmentIndexKeySpaceV1,
    kind: SegmentIndexPageKindV1,
    entry_count: u32,
    limits: SegmentIndexPageLimitsV2,
) -> Result<(
    SegmentIndexSummaryV1,
    SegmentIndexLogicalSummaryV2,
    Option<&'a [u8]>,
    Option<&'a [u8]>,
)> {
    let p = limits.physical;
    let mut offset = SEGMENT_INDEX_PAGE_HEADER_BYTES_V2;
    let mut previous_key: Option<&[u8]> = None;
    let mut previous_upper: Option<&[u8]> = None;
    let mut first_key = None;
    let mut last_key = None;
    let mut physical = SegmentIndexSummaryV1 {
        row_count: 0,
        value_bytes: 0,
        encoded_page_bytes: 0,
    };
    let mut logical = SegmentIndexLogicalSummaryV2 {
        row_count: 0,
        key_bytes: 0,
        value_bytes: 0,
        logical_sha256: Digest256::from_bytes([0; 32]),
    };
    if kind == SegmentIndexPageKindV1::Leaf {
        physical.encoded_page_bytes = page_len_as_u64(raw_page)?;
    }
    enforce_physical_limits(physical, limits)?;
    enforce_logical_limits(logical, limits)?;

    for _ in 0..entry_count {
        let entry = decode_entry(raw_page, &mut offset, kind)?;
        match entry {
            SegmentIndexEntryV2::Leaf(leaf) => {
                validate_key(leaf.key, keyspace, limits)?;
                if previous_key.is_some_and(|previous| previous >= leaf.key) {
                    return Err(corrupt(
                        "V2 leaf keys are duplicated or not strictly ordered",
                    ));
                }
                previous_key = Some(leaf.key);
                first_key.get_or_insert(leaf.key);
                last_key = Some(leaf.key);
                validate_extent(leaf.value_extent, p.max_segment_bytes)?;
                if leaf.value_extent.length == 0
                    && leaf.physical_value_sha256 != Digest256::of_bytes(b"")
                {
                    return Err(corrupt("V2 empty leaf value digest is not SHA256(empty)"));
                }
                validate_leaf_role(keyspace, leaf)?;
                physical.row_count = physical
                    .row_count
                    .checked_add(1)
                    .ok_or_else(|| corrupt("V2 physical row count overflows"))?;
                physical.value_bytes =
                    physical
                        .value_bytes
                        .checked_add(leaf.value_extent.length)
                        .ok_or_else(|| corrupt("V2 physical byte total overflows"))?;
                logical.row_count = logical
                    .row_count
                    .checked_add(1)
                    .ok_or_else(|| corrupt("V2 logical row count overflows"))?;
                logical.key_bytes = logical
                    .key_bytes
                    .checked_add(key_len_as_u64(leaf.key)?)
                    .ok_or_else(|| corrupt("V2 logical key total overflows"))?;
                logical.value_bytes = logical
                    .value_bytes
                    .checked_add(leaf.logical_value_bytes)
                    .ok_or_else(|| corrupt("V2 logical value total overflows"))?;
            }
            SegmentIndexEntryV2::Internal(internal) => {
                validate_key(internal.lower_key, keyspace, limits)?;
                validate_key(internal.upper_key, keyspace, limits)?;
                if internal.lower_key > internal.upper_key
                    || previous_upper.is_some_and(|upper| upper >= internal.lower_key)
                {
                    return Err(corrupt(
                        "V2 internal child bounds overlap or are out of order",
                    ));
                }
                previous_upper = Some(internal.upper_key);
                first_key.get_or_insert(internal.lower_key);
                last_key = Some(internal.upper_key);
                validate_child_extent(internal.child_extent, limits)?;
                validate_child_summaries(internal, keyspace, limits)?;
                physical.row_count = physical
                    .row_count
                    .checked_add(internal.physical_summary.row_count)
                    .ok_or_else(|| corrupt("V2 physical row count overflows"))?;
                physical.value_bytes = physical
                    .value_bytes
                    .checked_add(internal.physical_summary.value_bytes)
                    .ok_or_else(|| corrupt("V2 physical byte total overflows"))?;
                physical.encoded_page_bytes = physical
                    .encoded_page_bytes
                    .checked_add(internal.physical_summary.encoded_page_bytes)
                    .ok_or_else(|| corrupt("V2 encoded page total overflows"))?;
                logical.row_count = logical
                    .row_count
                    .checked_add(internal.logical_summary.row_count)
                    .ok_or_else(|| corrupt("V2 logical row count overflows"))?;
                logical.key_bytes = logical
                    .key_bytes
                    .checked_add(internal.logical_summary.key_bytes)
                    .ok_or_else(|| corrupt("V2 logical key total overflows"))?;
                logical.value_bytes = logical
                    .value_bytes
                    .checked_add(internal.logical_summary.value_bytes)
                    .ok_or_else(|| corrupt("V2 logical value total overflows"))?;
            }
        }
        enforce_physical_limits(physical, limits)?;
        enforce_logical_limits(logical, limits)?;
    }
    if offset != raw_page.len() {
        return Err(corrupt("V2 page has trailing or missing bytes"));
    }
    if kind == SegmentIndexPageKindV1::Internal {
        if physical.row_count != logical.row_count {
            return Err(corrupt("V2 physical and logical row counts differ"));
        }
        physical.encoded_page_bytes = physical
            .encoded_page_bytes
            .checked_add(page_len_as_u64(raw_page)?)
            .ok_or_else(|| corrupt("V2 encoded page total overflows"))?;
    }
    validate_logical_shape(keyspace, physical, logical, limits)?;
    enforce_physical_limits(physical, limits)?;
    enforce_logical_limits(logical, limits)?;
    Ok((physical, logical, first_key, last_key))
}

fn validate_leaf_role(
    keyspace: SegmentIndexKeySpaceV1,
    leaf: SegmentIndexLeafEntryV2<'_>,
) -> Result<()> {
    if keyspace == SegmentIndexKeySpaceV1::ObjectDigest {
        if &leaf.physical_value_sha256.as_bytes()[..] != leaf.key || leaf.logical_value_bytes != 40
        {
            return Err(corrupt("V2 ObjectDigest physical or logical tuple differs"));
        }
        let mut logical_value = [0u8; 40];
        logical_value[..8].copy_from_slice(&leaf.value_extent.length.to_le_bytes());
        logical_value[8..].copy_from_slice(leaf.physical_value_sha256.as_bytes());
        if Digest256::of_bytes(&logical_value) != leaf.logical_value_sha256 {
            return Err(corrupt("V2 ObjectDigest logical tuple digest differs"));
        }
    } else if leaf.logical_value_bytes != leaf.value_extent.length
        || leaf.logical_value_sha256 != leaf.physical_value_sha256
    {
        return Err(corrupt(
            "V2 native leaf logical value differs from its physical value",
        ));
    }
    Ok(())
}

fn validate_child_summaries(
    child: SegmentIndexInternalEntryV2<'_>,
    keyspace: SegmentIndexKeySpaceV1,
    limits: SegmentIndexPageLimitsV2,
) -> Result<()> {
    if child.physical_summary.row_count == 0
        || child.physical_summary.row_count != child.logical_summary.row_count
    {
        return Err(corrupt("V2 child physical and logical row counts differ"));
    }
    if child.physical_summary.encoded_page_bytes < child.child_extent.length {
        return Err(corrupt("V2 child page total is smaller than its page"));
    }
    if (child.logical_summary.row_count == 1 && child.lower_key != child.upper_key)
        || (child.logical_summary.row_count > 1 && child.lower_key >= child.upper_key)
    {
        return Err(corrupt("V2 child bounds do not match its row count"));
    }
    let lower_key_bytes = key_len_as_u64(child.lower_key)?;
    let upper_key_bytes = key_len_as_u64(child.upper_key)?;
    if child.logical_summary.row_count == 1 {
        if child.logical_summary.key_bytes != lower_key_bytes {
            return Err(corrupt(
                "V2 single-row child key bytes differ from its bound",
            ));
        }
    } else {
        let interior_key_count = child
            .logical_summary
            .row_count
            .checked_sub(2)
            .ok_or_else(|| corrupt("V2 multi-row child key count underflows"))?;
        let minimum_key_bytes = lower_key_bytes
            .checked_add(upper_key_bytes)
            .and_then(|bytes| bytes.checked_add(interior_key_count))
            .ok_or_else(|| corrupt("V2 child key byte lower bound overflows"))?;
        if child.logical_summary.key_bytes < minimum_key_bytes {
            return Err(corrupt("V2 child key bytes cannot represent its bounds"));
        }
    }
    validate_logical_shape(
        keyspace,
        child.physical_summary,
        child.logical_summary,
        limits,
    )?;
    enforce_physical_limits(child.physical_summary, limits)?;
    enforce_logical_limits(child.logical_summary, limits)
}

fn validate_logical_shape(
    keyspace: SegmentIndexKeySpaceV1,
    physical: SegmentIndexSummaryV1,
    logical: SegmentIndexLogicalSummaryV2,
    limits: SegmentIndexPageLimitsV2,
) -> Result<()> {
    if physical.row_count != logical.row_count {
        return Err(corrupt("V2 physical and logical row counts differ"));
    }
    if logical.key_bytes < logical.row_count {
        return Err(corrupt("V2 logical key bytes cannot represent its rows"));
    }
    let key_cap = u64::try_from(limits.physical.max_key_bytes)
        .map_err(|_| budget("V2 key limit cannot be represented"))?;
    let max_key_bytes = logical.row_count.checked_mul(key_cap).unwrap_or(u64::MAX);
    if logical.key_bytes > max_key_bytes {
        return Err(corrupt("V2 logical key bytes exceed the per-row key bound"));
    }
    if keyspace == SegmentIndexKeySpaceV1::ObjectDigest {
        let expected_key_bytes = logical
            .row_count
            .checked_mul(DIGEST_BYTES as u64)
            .ok_or_else(|| corrupt("V2 ObjectDigest key byte total overflows"))?;
        let expected_value_bytes = logical
            .row_count
            .checked_mul(40)
            .ok_or_else(|| corrupt("V2 ObjectDigest value byte total overflows"))?;
        if logical.key_bytes != expected_key_bytes || logical.value_bytes != expected_value_bytes {
            return Err(corrupt("V2 ObjectDigest logical totals differ"));
        }
    } else if logical.value_bytes != physical.value_bytes {
        return Err(corrupt("V2 logical and physical value byte totals differ"));
    }
    Ok(())
}

fn validate_key(
    key: &[u8],
    keyspace: SegmentIndexKeySpaceV1,
    limits: SegmentIndexPageLimitsV2,
) -> Result<()> {
    if key.is_empty() {
        return Err(corrupt("V2 key or bound must not be empty"));
    }
    if key.len() > limits.physical.max_key_bytes {
        return Err(budget("V2 key or bound exceeds its byte limit"));
    }
    if keyspace == SegmentIndexKeySpaceV1::ObjectDigest && key.len() != DIGEST_BYTES {
        return Err(corrupt(
            "V2 ObjectDigest keys must contain exactly 32 bytes",
        ));
    }
    Ok(())
}

fn validate_extent(extent: SegmentExtentV1, max_segment_bytes: u64) -> Result<()> {
    let end = extent
        .offset
        .checked_add(extent.length)
        .ok_or_else(|| corrupt("V2 segment extent overflows"))?;
    if end > max_segment_bytes {
        return Err(budget("V2 segment extent exceeds its byte limit"));
    }
    Ok(())
}

fn validate_child_extent(extent: SegmentExtentV1, limits: SegmentIndexPageLimitsV2) -> Result<()> {
    if extent.length < SEGMENT_INDEX_PAGE_HEADER_BYTES_V2 as u64 {
        return Err(corrupt("V2 child page is shorter than its header"));
    }
    if extent.length > limits.physical.max_page_bytes as u64 {
        return Err(budget("V2 child page exceeds its byte limit"));
    }
    validate_extent(extent, limits.physical.max_segment_bytes)
}

fn enforce_physical_limits(
    summary: SegmentIndexSummaryV1,
    limits: SegmentIndexPageLimitsV2,
) -> Result<()> {
    let p = limits.physical;
    if summary.row_count > p.max_row_count
        || summary.value_bytes > p.max_value_bytes
        || summary.encoded_page_bytes > p.max_encoded_page_bytes
    {
        return Err(budget("V2 physical summary exceeds a caller limit"));
    }
    Ok(())
}

fn enforce_logical_limits(
    summary: SegmentIndexLogicalSummaryV2,
    limits: SegmentIndexPageLimitsV2,
) -> Result<()> {
    if summary.row_count > limits.physical.max_row_count
        || summary.key_bytes > limits.max_logical_key_bytes
        || summary.value_bytes > limits.max_logical_value_bytes
    {
        return Err(budget("V2 logical summary exceeds a caller limit"));
    }
    Ok(())
}

fn logical_digest(
    raw_page: &[u8],
    keyspace: SegmentIndexKeySpaceV1,
    kind: SegmentIndexPageKindV1,
    entry_count: u32,
    summary: SegmentIndexLogicalSummaryV2,
) -> Result<Digest256> {
    let mut hash = Digest256Hasher::new();
    hash.update(LOGICAL_DOMAIN);
    hash.update(&[keyspace as u8, kind_tag(kind)]);
    hash.update(&entry_count.to_le_bytes());
    let mut offset = SEGMENT_INDEX_PAGE_HEADER_BYTES_V2;
    for _ in 0..entry_count {
        let entry = decode_entry(raw_page, &mut offset, kind)?;
        match entry {
            SegmentIndexEntryV2::Leaf(leaf) => {
                feed_key(&mut hash, leaf.key)?;
                hash.update(&leaf.logical_value_bytes.to_le_bytes());
                hash.update(leaf.logical_value_sha256.as_bytes());
            }
            SegmentIndexEntryV2::Internal(internal) => {
                feed_key(&mut hash, internal.lower_key)?;
                feed_key(&mut hash, internal.upper_key)?;
                for total in [
                    internal.logical_summary.row_count,
                    internal.logical_summary.key_bytes,
                    internal.logical_summary.value_bytes,
                ] {
                    hash.update(&total.to_le_bytes());
                }
                hash.update(internal.logical_summary.logical_sha256.as_bytes());
            }
        }
    }
    if offset != raw_page.len() {
        return Err(corrupt("V2 logical transcript did not end at page EOF"));
    }
    hash.update(b"EOF\0");
    for total in [summary.row_count, summary.key_bytes, summary.value_bytes] {
        hash.update(&total.to_le_bytes());
    }
    Ok(hash.finalize())
}

fn feed_key(hash: &mut Digest256Hasher, key: &[u8]) -> Result<()> {
    let len = u32::try_from(key.len()).map_err(|_| budget("V2 key length exceeds u32"))?;
    hash.update(&len.to_le_bytes());
    hash.update(key);
    Ok(())
}

fn decode_entry<'a>(
    raw_page: &'a [u8],
    offset: &mut usize,
    kind: SegmentIndexPageKindV1,
) -> Result<SegmentIndexEntryV2<'a>> {
    match kind {
        SegmentIndexPageKindV1::Leaf => {
            let key = read_key(raw_page, offset)?;
            let value_extent = read_extent(raw_page, offset)?;
            let physical_value_sha256 = read_digest(raw_page, offset)?;
            let logical_value_bytes = read_u64(raw_page, offset)?;
            let logical_value_sha256 = read_digest(raw_page, offset)?;
            Ok(SegmentIndexEntryV2::Leaf(SegmentIndexLeafEntryV2 {
                key,
                value_extent,
                physical_value_sha256,
                logical_value_bytes,
                logical_value_sha256,
            }))
        }
        SegmentIndexPageKindV1::Internal => {
            let lower_key = read_key(raw_page, offset)?;
            let child_extent = read_extent(raw_page, offset)?;
            let child_page_sha256 = read_digest(raw_page, offset)?;
            let physical_summary = SegmentIndexSummaryV1 {
                row_count: read_u64(raw_page, offset)?,
                value_bytes: read_u64(raw_page, offset)?,
                encoded_page_bytes: read_u64(raw_page, offset)?,
            };
            let logical_summary = SegmentIndexLogicalSummaryV2 {
                row_count: physical_summary.row_count,
                key_bytes: read_u64(raw_page, offset)?,
                value_bytes: read_u64(raw_page, offset)?,
                logical_sha256: read_digest(raw_page, offset)?,
            };
            let upper_key = read_key(raw_page, offset)?;
            Ok(SegmentIndexEntryV2::Internal(SegmentIndexInternalEntryV2 {
                lower_key,
                upper_key,
                child_extent,
                child_page_sha256,
                physical_summary,
                logical_summary,
            }))
        }
    }
}

fn entry_lower_key(entry: SegmentIndexEntryV2<'_>) -> &[u8] {
    match entry {
        SegmentIndexEntryV2::Leaf(leaf) => leaf.key,
        SegmentIndexEntryV2::Internal(internal) => internal.lower_key,
    }
}

fn read_key<'a>(raw_page: &'a [u8], offset: &mut usize) -> Result<&'a [u8]> {
    let len = usize::try_from(read_u32(raw_page, offset)?)
        .map_err(|_| corrupt("V2 key length cannot be represented"))?;
    take(raw_page, offset, len)
}

fn read_extent(raw_page: &[u8], offset: &mut usize) -> Result<SegmentExtentV1> {
    Ok(SegmentExtentV1 {
        segment_sha256: read_digest(raw_page, offset)?,
        offset: read_u64(raw_page, offset)?,
        length: read_u64(raw_page, offset)?,
    })
}

fn read_digest(raw_page: &[u8], offset: &mut usize) -> Result<Digest256> {
    let bytes = take(raw_page, offset, DIGEST_BYTES)?;
    let mut digest = [0u8; DIGEST_BYTES];
    digest.copy_from_slice(bytes);
    Ok(Digest256::from_bytes(digest))
}

fn read_u16_at(raw_page: &[u8], offset: usize) -> Result<u16> {
    let bytes = raw_page
        .get(offset..offset + 2)
        .ok_or_else(|| corrupt("V2 page header is truncated"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_at(raw_page: &[u8], offset: usize) -> Result<u32> {
    let bytes = raw_page
        .get(offset..offset + 4)
        .ok_or_else(|| corrupt("V2 page header is truncated"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u32(raw_page: &[u8], offset: &mut usize) -> Result<u32> {
    let bytes = take(raw_page, offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64(raw_page: &[u8], offset: &mut usize) -> Result<u64> {
    let bytes = take(raw_page, offset, 8)?;
    let mut value = [0u8; 8];
    value.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(value))
}

fn take<'a>(raw_page: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = (*offset)
        .checked_add(len)
        .ok_or_else(|| corrupt("V2 record offset overflows"))?;
    let bytes = raw_page
        .get(*offset..end)
        .ok_or_else(|| corrupt("V2 record is truncated"))?;
    *offset = end;
    Ok(bytes)
}

fn key_len_as_u64(key: &[u8]) -> Result<u64> {
    u64::try_from(key.len()).map_err(|_| budget("V2 key length cannot be represented"))
}

fn page_len_as_u64(raw_page: &[u8]) -> Result<u64> {
    u64::try_from(raw_page.len()).map_err(|_| budget("V2 page length cannot be represented"))
}

fn keyspace_from_tag(tag: u8) -> Option<SegmentIndexKeySpaceV1> {
    Some(match tag {
        0 => SegmentIndexKeySpaceV1::ObjectDigest,
        1 => SegmentIndexKeySpaceV1::MemberPath,
        2 => SegmentIndexKeySpaceV1::IdentityId,
        3 => SegmentIndexKeySpaceV1::ForwardDependency,
        4 => SegmentIndexKeySpaceV1::ReverseDependency,
        5 => SegmentIndexKeySpaceV1::History,
        6 => SegmentIndexKeySpaceV1::Retirement,
        7 => SegmentIndexKeySpaceV1::NativeState,
        _ => return None,
    })
}

fn kind_tag(kind: SegmentIndexPageKindV1) -> u8 {
    match kind {
        SegmentIndexPageKindV1::Leaf => LEAF,
        SegmentIndexPageKindV1::Internal => INTERNAL,
    }
}

fn corrupt(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::CorruptSelectedObject, detail)
}

fn budget(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn cursor_error(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::DescriptorMismatch, detail)
}
