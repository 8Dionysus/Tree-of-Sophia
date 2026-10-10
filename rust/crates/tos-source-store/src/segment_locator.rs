//! Bounded codec for immutable V1 segment-index pages.
//!
//! The codec authenticates and validates one borrowed page. Its DTOs and
//! successful decode do not select a root, admit an object, establish
//! freshness/history, or prove whole-tree closure. A caller that publishes a
//! root must complete the owner's initial full-tree validation; later point
//! reads must authenticate the selected path and compare each decoded child
//! summary and key boundary with its parent claim. Typed key/value semantics
//! remain with the source-store and FND/CMD owners.

use tos_foundation::Digest256;

use crate::{Result, StoreError, StoreErrorCode};

/// Maximum encoded page size accepted by this codec.
pub const SEGMENT_INDEX_PAGE_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum caller-selected encoded key size. Owner-specific single-key and
/// pair-key rules remain the typed row owner's responsibility.
pub const SEGMENT_INDEX_KEY_MAX_BYTES_V1: usize = 8193;
pub const SEGMENT_INDEX_PAGE_HEADER_BYTES_V1: usize = 16;

const PAGE_MAGIC: &[u8; 8] = b"TOSLPAGE";
const PAGE_VERSION: u16 = 1;
const LEAF: u8 = 0;
const INTERNAL: u8 = 1;
const DIGEST_BYTES: usize = 32;

/// Physical address of an immutable extent in a content-addressed segment.
///
/// This is a mechanical address only; it grants no read or selection
/// authority. The reader must also check the actual held segment length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentExtentV1 {
    pub segment_sha256: Digest256,
    pub offset: u64,
    pub length: u64,
}

/// Native keyspace tag carried by every page and checked on every decode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SegmentIndexKeySpaceV1 {
    ObjectDigest = 0,
    MemberPath = 1,
    IdentityId = 2,
    ForwardDependency = 3,
    ReverseDependency = 4,
    History = 5,
    Retirement = 6,
    NativeState = 7,
}

impl SegmentIndexKeySpaceV1 {
    fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::ObjectDigest,
            1 => Self::MemberPath,
            2 => Self::IdentityId,
            3 => Self::ForwardDependency,
            4 => Self::ReverseDependency,
            5 => Self::History,
            6 => Self::Retirement,
            7 => Self::NativeState,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SegmentIndexPageKindV1 {
    Leaf,
    Internal,
}

/// Caller budgets for decoding one page and its advertised subtree.
///
/// Every field is an explicit finite caller limit. Segment length remains a
/// claim checked against the actual held segment by the reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexPageLimitsV1 {
    pub max_page_bytes: usize,
    pub max_entries: u32,
    pub max_key_bytes: usize,
    pub max_segment_bytes: u64,
    pub max_row_count: u64,
    pub max_value_bytes: u64,
    pub max_encoded_page_bytes: u64,
}

impl SegmentIndexPageLimitsV1 {
    fn validate(self) -> Result<Self> {
        if !(SEGMENT_INDEX_PAGE_HEADER_BYTES_V1..=SEGMENT_INDEX_PAGE_MAX_BYTES_V1)
            .contains(&self.max_page_bytes)
            || self.max_entries == u32::MAX
            || self.max_key_bytes == 0
            || self.max_key_bytes > SEGMENT_INDEX_KEY_MAX_BYTES_V1
            || self.max_segment_bytes == 0
            || self.max_segment_bytes == u64::MAX
            || self.max_row_count == u64::MAX
            || self.max_value_bytes == u64::MAX
            || self.max_encoded_page_bytes < SEGMENT_INDEX_PAGE_HEADER_BYTES_V1 as u64
            || self.max_encoded_page_bytes == u64::MAX
        {
            return Err(StoreError::new(
                StoreErrorCode::DescriptorMismatch,
                "segment-index page limits are invalid",
            ));
        }
        Ok(self)
    }
}

/// Checked aggregate claimed by an index page or one of its children.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexSummaryV1 {
    pub row_count: u64,
    pub value_bytes: u64,
    pub encoded_page_bytes: u64,
}

/// Generic selected-root descriptor. This value is not itself a root-selection
/// or admission event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexRootV1 {
    pub keyspace: SegmentIndexKeySpaceV1,
    pub root_extent: SegmentExtentV1,
    pub root_page_sha256: Digest256,
    pub row_count: u64,
    pub value_bytes: u64,
    pub encoded_page_bytes: u64,
}

/// Object-digest-keyspace view retained for object-only callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentLocatorRootV1 {
    pub root_extent: SegmentExtentV1,
    pub root_page_sha256: Digest256,
    pub object_count: u64,
    pub object_bytes: u64,
    pub encoded_page_bytes: u64,
}

impl SegmentIndexRootV1 {
    /// Convert only the ObjectDigest role into the object-only mechanical DTO.
    pub fn as_object_locator(self) -> Option<SegmentLocatorRootV1> {
        (self.keyspace == SegmentIndexKeySpaceV1::ObjectDigest).then_some(SegmentLocatorRootV1 {
            root_extent: self.root_extent,
            root_page_sha256: self.root_page_sha256,
            object_count: self.row_count,
            object_bytes: self.value_bytes,
            encoded_page_bytes: self.encoded_page_bytes,
        })
    }

    pub const fn summary(self) -> SegmentIndexSummaryV1 {
        SegmentIndexSummaryV1 {
            row_count: self.row_count,
            value_bytes: self.value_bytes,
            encoded_page_bytes: self.encoded_page_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexLeafEntryV1<'a> {
    pub key: &'a [u8],
    pub value_extent: SegmentExtentV1,
    pub value_sha256: Digest256,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexInternalEntryV1<'a> {
    pub key: &'a [u8],
    pub child_extent: SegmentExtentV1,
    pub child_page_sha256: Digest256,
    pub summary: SegmentIndexSummaryV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SegmentIndexEntryV1<'a> {
    Leaf(SegmentIndexLeafEntryV1<'a>),
    Internal(SegmentIndexInternalEntryV1<'a>),
}

/// A fully checked page that borrows its original encoded bytes.
#[derive(Clone, Copy, Debug)]
pub struct DecodedSegmentIndexPageV1<'a> {
    raw_page: &'a [u8],
    page_sha256: Digest256,
    keyspace: SegmentIndexKeySpaceV1,
    kind: SegmentIndexPageKindV1,
    entry_count: u32,
    summary: SegmentIndexSummaryV1,
    limits: SegmentIndexPageLimitsV1,
}

/// Opaque sequential position in one already checked page.
///
/// The identity fields make a copied cursor unusable with a different page
/// without requiring a prefix rescan on every step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SegmentIndexEntryCursorV1 {
    byte_offset: usize,
    ordinal: u32,
    page_sha256: Digest256,
    page_len: usize,
}

impl<'a> DecodedSegmentIndexPageV1<'a> {
    pub const fn keyspace(self) -> SegmentIndexKeySpaceV1 {
        self.keyspace
    }

    pub const fn kind(self) -> SegmentIndexPageKindV1 {
        self.kind
    }

    pub const fn entry_count(self) -> u32 {
        self.entry_count
    }

    pub const fn summary(self) -> SegmentIndexSummaryV1 {
        self.summary
    }

    pub(crate) fn initial_cursor(&self) -> SegmentIndexEntryCursorV1 {
        SegmentIndexEntryCursorV1 {
            byte_offset: SEGMENT_INDEX_PAGE_HEADER_BYTES_V1,
            ordinal: 0,
            page_sha256: self.page_sha256,
            page_len: self.raw_page.len(),
        }
    }

    /// Read and advance one borrowed entry. A copied cursor can be used to peek
    /// without advancing the caller's position.
    pub(crate) fn next_entry(
        &self,
        cursor: &mut SegmentIndexEntryCursorV1,
    ) -> Result<Option<SegmentIndexEntryV1<'a>>> {
        self.validate_cursor(cursor)?;
        if cursor.ordinal == self.entry_count {
            return Ok(None);
        }

        let mut byte_offset = cursor.byte_offset;
        let entry = decode_entry(self.raw_page, &mut byte_offset, self.kind)?;
        let ordinal = cursor
            .ordinal
            .checked_add(1)
            .ok_or_else(|| cursor_mismatch("segment-index cursor ordinal overflows"))?;
        if byte_offset <= cursor.byte_offset
            || byte_offset > self.raw_page.len()
            || (ordinal == self.entry_count && byte_offset != self.raw_page.len())
            || (ordinal < self.entry_count && byte_offset == self.raw_page.len())
        {
            return Err(cursor_mismatch(
                "segment-index cursor does not end at a checked entry boundary",
            ));
        }
        cursor.byte_offset = byte_offset;
        cursor.ordinal = ordinal;
        Ok(Some(entry))
    }

    fn validate_cursor(&self, cursor: &SegmentIndexEntryCursorV1) -> Result<()> {
        if cursor.page_sha256 != self.page_sha256 || cursor.page_len != self.raw_page.len() {
            return Err(cursor_mismatch(
                "segment-index cursor belongs to a different page",
            ));
        }
        if cursor.ordinal > self.entry_count
            || cursor.byte_offset < SEGMENT_INDEX_PAGE_HEADER_BYTES_V1
            || cursor.byte_offset > self.raw_page.len()
            || (cursor.ordinal == 0 && cursor.byte_offset != SEGMENT_INDEX_PAGE_HEADER_BYTES_V1)
            || (cursor.ordinal == self.entry_count && cursor.byte_offset != self.raw_page.len())
            || (cursor.ordinal < self.entry_count && cursor.byte_offset == self.raw_page.len())
        {
            return Err(cursor_mismatch(
                "segment-index cursor offset or ordinal is invalid",
            ));
        }
        Ok(())
    }

    /// Return a borrowed entry by ordinal. Locating a variable-length record
    /// scans preceding entry boundaries without building an entry collection.
    pub fn entry_at(self, ordinal: u32) -> Result<Option<SegmentIndexEntryV1<'a>>> {
        if ordinal >= self.entry_count {
            return Ok(None);
        }
        let mut cursor = SEGMENT_INDEX_PAGE_HEADER_BYTES_V1;
        for _ in 0..ordinal {
            let _ = decode_entry(self.raw_page, &mut cursor, self.kind)?;
        }
        decode_entry(self.raw_page, &mut cursor, self.kind).map(Some)
    }

    pub fn first_key(self) -> Result<Option<&'a [u8]>> {
        Ok(self.entry_at(0)?.map(entry_key))
    }

    pub fn last_key(self) -> Result<Option<&'a [u8]>> {
        let Some(last) = self.entry_count.checked_sub(1) else {
            return Ok(None);
        };
        Ok(self.entry_at(last)?.map(entry_key))
    }

    /// Return the first page key strictly greater than the supplied key.
    /// Scans encoded entry boundaries once and returns a borrow into this page.
    pub fn next_key_after(self, key: &[u8]) -> Result<Option<&'a [u8]>> {
        if key.len() > self.limits.max_key_bytes {
            return Err(budget_exceeded(
                "segment-index lookup key exceeds its limit",
            ));
        }
        let mut cursor = SEGMENT_INDEX_PAGE_HEADER_BYTES_V1;
        for _ in 0..self.entry_count {
            let entry = decode_entry(self.raw_page, &mut cursor, self.kind)?;
            let entry_key = entry_key(entry);
            if entry_key > key {
                return Ok(Some(entry_key));
            }
        }
        Ok(None)
    }

    /// Find an exact key in a leaf page using only borrowed page bytes.
    pub fn find_leaf(self, key: &[u8]) -> Result<Option<SegmentIndexLeafEntryV1<'a>>> {
        if key.len() > self.limits.max_key_bytes {
            return Err(budget_exceeded(
                "segment-index lookup key exceeds its limit",
            ));
        }
        if self.kind != SegmentIndexPageKindV1::Leaf {
            return Ok(None);
        }
        let mut cursor = SEGMENT_INDEX_PAGE_HEADER_BYTES_V1;
        for _ in 0..self.entry_count {
            let entry = decode_entry(self.raw_page, &mut cursor, self.kind)?;
            let SegmentIndexEntryV1::Leaf(entry) = entry else {
                return Err(corrupt("segment-index leaf contains an internal entry"));
            };
            match entry.key.cmp(key) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => return Ok(Some(entry)),
                std::cmp::Ordering::Greater => return Ok(None),
            }
        }
        Ok(None)
    }

    /// Select the child whose lower-bound key is the greatest key <= the query.
    /// A key below the first lower bound has no child in this page.
    pub fn child_for_key(self, key: &[u8]) -> Result<Option<SegmentIndexInternalEntryV1<'a>>> {
        if key.len() > self.limits.max_key_bytes {
            return Err(budget_exceeded(
                "segment-index lookup key exceeds its limit",
            ));
        }
        if self.kind != SegmentIndexPageKindV1::Internal {
            return Ok(None);
        }
        let mut cursor = SEGMENT_INDEX_PAGE_HEADER_BYTES_V1;
        let mut selected = None;
        for _ in 0..self.entry_count {
            let entry = decode_entry(self.raw_page, &mut cursor, self.kind)?;
            let SegmentIndexEntryV1::Internal(entry) = entry else {
                return Err(corrupt("segment-index internal page contains a leaf entry"));
            };
            if entry.key > key {
                break;
            }
            selected = Some(entry);
        }
        Ok(selected)
    }
}

/// Authenticate and decode one immutable page, retaining only a borrow of its
/// input bytes and fixed-size summary state. No storage selection occurs here.
pub fn decode_segment_index_page(
    raw_page: &[u8],
    expected_page_sha: Digest256,
    expected_keyspace: SegmentIndexKeySpaceV1,
    limits: SegmentIndexPageLimitsV1,
) -> Result<DecodedSegmentIndexPageV1<'_>> {
    let limits = limits.validate()?;
    if raw_page.len() > limits.max_page_bytes {
        return Err(budget_exceeded("segment-index page exceeds its byte limit"));
    }
    if raw_page.len() < SEGMENT_INDEX_PAGE_HEADER_BYTES_V1 {
        return Err(corrupt("segment-index page header is truncated"));
    }
    if Digest256::of_bytes(raw_page) != expected_page_sha {
        return Err(corrupt("segment-index page digest does not match"));
    }
    if raw_page.get(..8) != Some(&PAGE_MAGIC[..]) {
        return Err(corrupt("segment-index page magic is invalid"));
    }
    let version = read_u16_at(raw_page, 8)?;
    if version != PAGE_VERSION {
        return Err(StoreError::new(
            StoreErrorCode::UnsupportedFormat,
            "segment-index page version is unsupported",
        ));
    }
    let kind = match raw_page[10] {
        LEAF => SegmentIndexPageKindV1::Leaf,
        INTERNAL => SegmentIndexPageKindV1::Internal,
        _ => {
            return Err(StoreError::new(
                StoreErrorCode::UnsupportedFormat,
                "segment-index page kind is unsupported",
            ));
        }
    };
    let keyspace = SegmentIndexKeySpaceV1::from_tag(raw_page[11]).ok_or_else(|| {
        StoreError::new(
            StoreErrorCode::UnsupportedFormat,
            "segment-index keyspace is unsupported",
        )
    })?;
    if keyspace != expected_keyspace {
        return Err(corrupt("segment-index page keyspace does not match"));
    }
    let entry_count = read_u32_at(raw_page, 12)?;
    if entry_count > limits.max_entries {
        return Err(budget_exceeded(
            "segment-index entry count exceeds its limit",
        ));
    }
    if kind == SegmentIndexPageKindV1::Internal && entry_count == 0 {
        return Err(corrupt("segment-index internal page must contain a child"));
    }

    let summary = validate_entries(raw_page, keyspace, kind, entry_count, limits)?;
    if summary.encoded_page_bytes > limits.max_encoded_page_bytes {
        return Err(budget_exceeded(
            "segment-index encoded subtree exceeds its byte limit",
        ));
    }
    Ok(DecodedSegmentIndexPageV1 {
        raw_page,
        page_sha256: expected_page_sha,
        keyspace,
        kind,
        entry_count,
        summary,
        limits,
    })
}

fn validate_entries(
    raw_page: &[u8],
    keyspace: SegmentIndexKeySpaceV1,
    kind: SegmentIndexPageKindV1,
    entry_count: u32,
    limits: SegmentIndexPageLimitsV1,
) -> Result<SegmentIndexSummaryV1> {
    let mut cursor = SEGMENT_INDEX_PAGE_HEADER_BYTES_V1;
    let mut previous_key: Option<&[u8]> = None;
    let mut summary = match kind {
        SegmentIndexPageKindV1::Leaf => SegmentIndexSummaryV1 {
            row_count: u64::from(entry_count),
            value_bytes: 0,
            encoded_page_bytes: page_len_as_u64(raw_page)?,
        },
        SegmentIndexPageKindV1::Internal => SegmentIndexSummaryV1 {
            row_count: 0,
            value_bytes: 0,
            encoded_page_bytes: 0,
        },
    };
    enforce_summary_caps(summary, limits)?;

    for _ in 0..entry_count {
        let entry = decode_entry(raw_page, &mut cursor, kind)?;
        let key = entry_key(entry);
        validate_key(key, keyspace, limits)?;
        if previous_key.is_some_and(|previous| previous >= key) {
            return Err(corrupt(
                "segment-index keys are duplicated or not strictly ordered",
            ));
        }
        previous_key = Some(key);

        match entry {
            SegmentIndexEntryV1::Leaf(leaf) => {
                validate_extent(leaf.value_extent, limits.max_segment_bytes)?;
                if keyspace == SegmentIndexKeySpaceV1::ObjectDigest
                    && &leaf.value_sha256.as_bytes()[..] != leaf.key
                {
                    return Err(corrupt("ObjectDigest value digest does not match its key"));
                }
                summary.value_bytes = summary
                    .value_bytes
                    .checked_add(leaf.value_extent.length)
                    .ok_or_else(|| corrupt("segment-index value byte total overflows"))?;
                enforce_summary_caps(summary, limits)?;
            }
            SegmentIndexEntryV1::Internal(internal) => {
                validate_child_extent(internal.child_extent, limits)?;
                if internal.summary.row_count == 0 {
                    return Err(corrupt("segment-index internal child has no rows"));
                }
                if internal.summary.encoded_page_bytes < internal.child_extent.length {
                    return Err(corrupt(
                        "segment-index child page total is smaller than its page",
                    ));
                }
                enforce_summary_caps(internal.summary, limits)?;
                summary.row_count = summary
                    .row_count
                    .checked_add(internal.summary.row_count)
                    .ok_or_else(|| corrupt("segment-index row total overflows"))?;
                summary.value_bytes = summary
                    .value_bytes
                    .checked_add(internal.summary.value_bytes)
                    .ok_or_else(|| corrupt("segment-index value byte total overflows"))?;
                summary.encoded_page_bytes = summary
                    .encoded_page_bytes
                    .checked_add(internal.summary.encoded_page_bytes)
                    .ok_or_else(|| corrupt("segment-index page byte total overflows"))?;
                enforce_summary_caps(summary, limits)?;
            }
        }
    }
    if cursor != raw_page.len() {
        return Err(corrupt("segment-index page has trailing or missing bytes"));
    }
    if kind == SegmentIndexPageKindV1::Internal {
        summary.encoded_page_bytes = summary
            .encoded_page_bytes
            .checked_add(page_len_as_u64(raw_page)?)
            .ok_or_else(|| corrupt("segment-index page byte total overflows"))?;
        enforce_summary_caps(summary, limits)?;
    }
    Ok(summary)
}

fn validate_key(
    key: &[u8],
    keyspace: SegmentIndexKeySpaceV1,
    limits: SegmentIndexPageLimitsV1,
) -> Result<()> {
    if key.is_empty() {
        return Err(corrupt("segment-index keys must not be empty"));
    }
    if key.len() > limits.max_key_bytes {
        return Err(budget_exceeded("segment-index key exceeds its byte limit"));
    }
    if keyspace == SegmentIndexKeySpaceV1::ObjectDigest && key.len() != DIGEST_BYTES {
        return Err(corrupt("ObjectDigest keys must contain exactly 32 bytes"));
    }
    Ok(())
}

fn validate_extent(extent: SegmentExtentV1, max_segment_bytes: u64) -> Result<()> {
    let end = extent
        .offset
        .checked_add(extent.length)
        .ok_or_else(|| corrupt("segment-index extent overflows"))?;
    if end > max_segment_bytes {
        return Err(budget_exceeded(
            "segment-index extent exceeds its segment byte limit",
        ));
    }
    Ok(())
}

fn validate_child_extent(extent: SegmentExtentV1, limits: SegmentIndexPageLimitsV1) -> Result<()> {
    if extent.length < SEGMENT_INDEX_PAGE_HEADER_BYTES_V1 as u64 {
        return Err(corrupt(
            "segment-index child page is shorter than its header",
        ));
    }
    if extent.length > limits.max_page_bytes as u64 {
        return Err(budget_exceeded(
            "segment-index child page exceeds its byte limit",
        ));
    }
    validate_extent(extent, limits.max_segment_bytes)
}

fn enforce_summary_caps(
    summary: SegmentIndexSummaryV1,
    limits: SegmentIndexPageLimitsV1,
) -> Result<()> {
    if summary.row_count > limits.max_row_count
        || summary.value_bytes > limits.max_value_bytes
        || summary.encoded_page_bytes > limits.max_encoded_page_bytes
    {
        return Err(budget_exceeded(
            "segment-index summary exceeds a caller limit",
        ));
    }
    Ok(())
}

fn decode_entry<'a>(
    raw_page: &'a [u8],
    cursor: &mut usize,
    kind: SegmentIndexPageKindV1,
) -> Result<SegmentIndexEntryV1<'a>> {
    let key_len = usize::try_from(read_u32(raw_page, cursor)?)
        .map_err(|_| corrupt("segment-index key length cannot be represented"))?;
    let key = take(raw_page, cursor, key_len)?;
    match kind {
        SegmentIndexPageKindV1::Leaf => {
            let value_extent = read_extent(raw_page, cursor)?;
            let value_sha256 = read_digest(raw_page, cursor)?;
            Ok(SegmentIndexEntryV1::Leaf(SegmentIndexLeafEntryV1 {
                key,
                value_extent,
                value_sha256,
            }))
        }
        SegmentIndexPageKindV1::Internal => {
            let child_extent = read_extent(raw_page, cursor)?;
            let child_page_sha256 = read_digest(raw_page, cursor)?;
            let summary = SegmentIndexSummaryV1 {
                row_count: read_u64(raw_page, cursor)?,
                value_bytes: read_u64(raw_page, cursor)?,
                encoded_page_bytes: read_u64(raw_page, cursor)?,
            };
            Ok(SegmentIndexEntryV1::Internal(SegmentIndexInternalEntryV1 {
                key,
                child_extent,
                child_page_sha256,
                summary,
            }))
        }
    }
}

fn entry_key<'a>(entry: SegmentIndexEntryV1<'a>) -> &'a [u8] {
    match entry {
        SegmentIndexEntryV1::Leaf(leaf) => leaf.key,
        SegmentIndexEntryV1::Internal(internal) => internal.key,
    }
}

fn read_extent(raw_page: &[u8], cursor: &mut usize) -> Result<SegmentExtentV1> {
    Ok(SegmentExtentV1 {
        segment_sha256: read_digest(raw_page, cursor)?,
        offset: read_u64(raw_page, cursor)?,
        length: read_u64(raw_page, cursor)?,
    })
}

fn read_digest(raw_page: &[u8], cursor: &mut usize) -> Result<Digest256> {
    let bytes = take(raw_page, cursor, DIGEST_BYTES)?;
    let mut digest = [0u8; DIGEST_BYTES];
    digest.copy_from_slice(bytes);
    Ok(Digest256::from_bytes(digest))
}

fn read_u16_at(raw_page: &[u8], offset: usize) -> Result<u16> {
    let bytes = raw_page
        .get(offset..offset + 2)
        .ok_or_else(|| corrupt("segment-index page header is truncated"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32_at(raw_page: &[u8], offset: usize) -> Result<u32> {
    let bytes = raw_page
        .get(offset..offset + 4)
        .ok_or_else(|| corrupt("segment-index page header is truncated"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u32(raw_page: &[u8], cursor: &mut usize) -> Result<u32> {
    let bytes = take(raw_page, cursor, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64(raw_page: &[u8], cursor: &mut usize) -> Result<u64> {
    let bytes = take(raw_page, cursor, 8)?;
    let mut value = [0u8; 8];
    value.copy_from_slice(bytes);
    Ok(u64::from_le_bytes(value))
}

fn take<'a>(raw_page: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8]> {
    let end = (*cursor)
        .checked_add(length)
        .ok_or_else(|| corrupt("segment-index record offset overflows"))?;
    let bytes = raw_page
        .get(*cursor..end)
        .ok_or_else(|| corrupt("segment-index record is truncated"))?;
    *cursor = end;
    Ok(bytes)
}

fn page_len_as_u64(raw_page: &[u8]) -> Result<u64> {
    u64::try_from(raw_page.len())
        .map_err(|_| budget_exceeded("segment-index page length cannot be represented"))
}

fn corrupt(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::CorruptSelectedObject, detail)
}

fn budget_exceeded(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn cursor_mismatch(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::DescriptorMismatch, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> SegmentIndexPageLimitsV1 {
        SegmentIndexPageLimitsV1 {
            max_page_bytes: SEGMENT_INDEX_PAGE_MAX_BYTES_V1,
            max_entries: 32,
            max_key_bytes: SEGMENT_INDEX_KEY_MAX_BYTES_V1,
            max_segment_bytes: 1024 * 1024,
            max_row_count: 1024,
            max_value_bytes: 1024 * 1024,
            max_encoded_page_bytes: 1024 * 1024,
        }
    }

    fn header(kind: u8, keyspace: u8, count: u32) -> Vec<u8> {
        let mut page = b"TOSLPAGE".to_vec();
        page.extend_from_slice(&PAGE_VERSION.to_le_bytes());
        page.push(kind);
        page.push(keyspace);
        page.extend_from_slice(&count.to_le_bytes());
        page
    }

    fn append_key(page: &mut Vec<u8>, key: &[u8]) {
        page.extend_from_slice(&(key.len() as u32).to_le_bytes());
        page.extend_from_slice(key);
    }

    fn append_extent(page: &mut Vec<u8>, segment: [u8; 32], offset: u64, length: u64) {
        page.extend_from_slice(&segment);
        page.extend_from_slice(&offset.to_le_bytes());
        page.extend_from_slice(&length.to_le_bytes());
    }

    fn leaf_page(keyspace: u8, keys: &[&[u8]], offset: u64, length: u64) -> Vec<u8> {
        let mut page = header(LEAF, keyspace, keys.len() as u32);
        for key in keys {
            append_key(&mut page, key);
            append_extent(&mut page, [9; 32], offset, length);
            page.extend_from_slice(&[7; 32]);
        }
        page
    }

    fn internal_page() -> Vec<u8> {
        let mut page = header(INTERNAL, SegmentIndexKeySpaceV1::MemberPath as u8, 2);
        for (key, offset) in [(b"a".as_slice(), 0), (b"c".as_slice(), 16)] {
            append_key(&mut page, key);
            append_extent(&mut page, [3; 32], offset, 16);
            page.extend_from_slice(&[4; 32]);
            page.extend_from_slice(&1u64.to_le_bytes());
            page.extend_from_slice(&0u64.to_le_bytes());
            page.extend_from_slice(&16u64.to_le_bytes());
        }
        page
    }

    #[test]
    fn fixed_wire_leaf_decodes_borrowed_keys_and_totals() {
        let keys = [&b"a"[..], &b"b"[..]];
        let page = leaf_page(SegmentIndexKeySpaceV1::MemberPath as u8, &keys, 10, 3);
        assert_eq!(
            &page[..SEGMENT_INDEX_PAGE_HEADER_BYTES_V1],
            &[
                b'T', b'O', b'S', b'L', b'P', b'A', b'G', b'E', 1, 0, 0, 1, 2, 0, 0, 0,
            ]
        );
        let decoded = decode_segment_index_page(
            &page,
            Digest256::of_bytes(&page),
            SegmentIndexKeySpaceV1::MemberPath,
            limits(),
        )
        .expect("fixed page should decode");
        assert_eq!(decoded.kind(), SegmentIndexPageKindV1::Leaf);
        assert_eq!(decoded.entry_count(), 2);
        assert_eq!(
            decoded.summary(),
            SegmentIndexSummaryV1 {
                row_count: 2,
                value_bytes: 6,
                encoded_page_bytes: page.len() as u64,
            }
        );
        assert_eq!(decoded.first_key().expect("first key"), Some(&b"a"[..]));
        assert_eq!(decoded.last_key().expect("last key"), Some(&b"b"[..]));
        assert_eq!(
            decoded.next_key_after(b"a").expect("next key"),
            Some(&b"b"[..])
        );
        assert_eq!(decoded.next_key_after(b"z").expect("no next key"), None);
        let found = decoded
            .find_leaf(b"b")
            .expect("find should succeed")
            .expect("key should exist");
        assert_eq!(found.key, b"b");
        assert_eq!(found.value_extent.offset, 10);
        assert_eq!(found.value_extent.length, 3);
        assert_eq!(found.key.as_ptr(), page[105..].as_ptr());
        assert!(decoded.entry_at(2).expect("out of range").is_none());
    }

    #[test]
    fn sequential_cursor_peeks_without_advancing_and_rejects_other_pages() {
        let page = leaf_page(
            SegmentIndexKeySpaceV1::MemberPath as u8,
            &[&b"a"[..], &b"b"[..]],
            0,
            1,
        );
        let decoded = decode_segment_index_page(
            &page,
            Digest256::of_bytes(&page),
            SegmentIndexKeySpaceV1::MemberPath,
            limits(),
        )
        .expect("page should decode");
        let mut cursor = decoded.initial_cursor();
        let first = decoded
            .next_entry(&mut cursor)
            .expect("first step")
            .expect("first entry");
        assert_eq!(entry_key(first), b"a");

        let mut peek = cursor;
        let peeked = decoded
            .next_entry(&mut peek)
            .expect("peek step")
            .expect("second entry");
        assert_eq!(entry_key(peeked), b"b");
        let actual = decoded
            .next_entry(&mut cursor)
            .expect("actual second step")
            .expect("second entry");
        assert_eq!(entry_key(actual), b"b");
        assert!(decoded.next_entry(&mut cursor).expect("EOF").is_none());

        let other_page = leaf_page(
            SegmentIndexKeySpaceV1::MemberPath as u8,
            &[&b"c"[..], &b"d"[..]],
            0,
            1,
        );
        let other = decode_segment_index_page(
            &other_page,
            Digest256::of_bytes(&other_page),
            SegmentIndexKeySpaceV1::MemberPath,
            limits(),
        )
        .expect("other page should decode");
        let mut wrong_page_cursor = decoded.initial_cursor();
        assert!(other.next_entry(&mut wrong_page_cursor).is_err());
    }

    #[test]
    fn digest_trailing_bytes_and_duplicate_keys_are_rejected() {
        let valid = leaf_page(SegmentIndexKeySpaceV1::MemberPath as u8, &[&b"a"[..]], 0, 1);
        assert_eq!(
            decode_segment_index_page(
                &valid,
                Digest256::from_bytes([0; 32]),
                SegmentIndexKeySpaceV1::MemberPath,
                limits(),
            )
            .err()
            .map(|error| error.code),
            Some(StoreErrorCode::CorruptSelectedObject)
        );

        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(
            decode_segment_index_page(
                &trailing,
                Digest256::of_bytes(&trailing),
                SegmentIndexKeySpaceV1::MemberPath,
                limits(),
            )
            .is_err()
        );

        let duplicate = leaf_page(
            SegmentIndexKeySpaceV1::MemberPath as u8,
            &[&b"x"[..], &b"x"[..]],
            0,
            1,
        );
        assert!(
            decode_segment_index_page(
                &duplicate,
                Digest256::of_bytes(&duplicate),
                SegmentIndexKeySpaceV1::MemberPath,
                limits(),
            )
            .is_err()
        );
    }

    #[test]
    fn object_digest_leaf_requires_key_value_digest_identity() {
        let mut page = header(LEAF, SegmentIndexKeySpaceV1::ObjectDigest as u8, 1);
        append_key(&mut page, &[7; 32]);
        append_extent(&mut page, [9; 32], 0, 1);
        page.extend_from_slice(&[8; 32]);
        assert!(
            decode_segment_index_page(
                &page,
                Digest256::of_bytes(&page),
                SegmentIndexKeySpaceV1::ObjectDigest,
                limits(),
            )
            .is_err()
        );
    }

    #[test]
    fn internal_lookup_selects_the_lower_bound_child_and_checks_totals() {
        let page = internal_page();
        let decoded = decode_segment_index_page(
            &page,
            Digest256::of_bytes(&page),
            SegmentIndexKeySpaceV1::MemberPath,
            limits(),
        )
        .expect("internal page should decode");
        assert_eq!(decoded.summary().row_count, 2);
        assert_eq!(decoded.summary().encoded_page_bytes, page.len() as u64 + 32);
        assert!(decoded.child_for_key(b"0").expect("lookup").is_none());
        assert_eq!(
            decoded
                .child_for_key(b"b")
                .expect("lookup")
                .expect("first child")
                .key,
            b"a"
        );
        assert_eq!(
            decoded
                .child_for_key(b"z")
                .expect("lookup")
                .expect("last child")
                .key,
            b"c"
        );
    }

    #[test]
    fn extent_addition_overflow_is_rejected_before_accessors() {
        let page = leaf_page(
            SegmentIndexKeySpaceV1::MemberPath as u8,
            &[&b"x"[..]],
            u64::MAX,
            1,
        );
        assert!(
            decode_segment_index_page(
                &page,
                Digest256::of_bytes(&page),
                SegmentIndexKeySpaceV1::MemberPath,
                limits(),
            )
            .is_err()
        );
    }

    #[test]
    fn object_only_root_mapping_checks_the_keyspace() {
        let extent = SegmentExtentV1 {
            segment_sha256: Digest256::from_bytes([1; 32]),
            offset: 16,
            length: 100,
        };
        let object_root = SegmentIndexRootV1 {
            keyspace: SegmentIndexKeySpaceV1::ObjectDigest,
            root_extent: extent,
            root_page_sha256: Digest256::from_bytes([2; 32]),
            row_count: 4,
            value_bytes: 40,
            encoded_page_bytes: 100,
        };
        assert_eq!(object_root.as_object_locator().unwrap().object_count, 4);
        let member_root = SegmentIndexRootV1 {
            keyspace: SegmentIndexKeySpaceV1::MemberPath,
            ..object_root
        };
        assert!(member_root.as_object_locator().is_none());
    }
}
