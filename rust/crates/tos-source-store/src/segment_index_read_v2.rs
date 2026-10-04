//! Budgeted point reads and bounded mutation-path selection for V2 pages.
//!
//! The typed root remains a mechanical descriptor. This reader validates its
//! held segment ranges, physical summaries, logical summaries, and stored
//! child bounds; it does not select or admit a native root or authorize a COW
//! mutation.

use std::{
    fs::{File, Metadata},
    io::{Cursor, Write},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use tos_foundation::{Digest256, Digest256Hasher};

use crate::{
    PinnedSqliteIoBudget, PinnedSqliteSpaceBudget,
    error::{Result, StoreError, StoreErrorCode},
    segment_locator::{
        SEGMENT_INDEX_KEY_MAX_BYTES_V1, SEGMENT_INDEX_PAGE_HEADER_BYTES_V1, SegmentExtentV1,
        SegmentIndexKeySpaceV1, SegmentIndexPageKindV1, SegmentIndexSummaryV1,
    },
    segment_locator_v2::{
        DecodedSegmentIndexPageV2, SEGMENT_INDEX_KEY_MAX_BYTES_V2,
        SEGMENT_INDEX_PAGE_HEADER_BYTES_V2, SEGMENT_INDEX_PAGE_MAX_BYTES_V2,
        SegmentIndexEntryCursorV2, SegmentIndexEntryV2, SegmentIndexInternalEntryV2,
        SegmentIndexLeafEntryV2, SegmentIndexLogicalSummaryV2, SegmentIndexPageLimitsV2,
        SegmentIndexRootV2, decode_segment_index_page_v2,
    },
    segment_object::verify_segment_range,
};

pub(crate) const V2_PAGE_BUFFER_BYTES: usize = SEGMENT_INDEX_PAGE_MAX_BYTES_V2;
pub(crate) const V2_KEY_BUFFER_BYTES: usize = SEGMENT_INDEX_KEY_MAX_BYTES_V2 + 1;
pub(crate) const V2_RANGE_BUFFER_BYTES: usize = 64 * 1024;
const MAX_V2_DEPTH: u32 = 128;
pub(crate) const V2_FIXED_WORKSPACE_BYTES: usize = 4096;
const OBJECT_DIGEST_KEY_BYTES: usize = 32;
const OBJECT_LOGICAL_VALUE_BYTES: usize = 40;

/// Read limits for the V2 reader. `max_value_bytes` bounds a physical value
/// extent; the separate page limits bound logical key/value totals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexReadLimitsV2 {
    pub pages: SegmentIndexPageLimitsV2,
    pub max_depth: u32,
    pub max_value_bytes: u64,
    pub max_workspace_bytes: usize,
}

/// A mechanically located physical value plus its committed logical shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentLocatedValueV2 {
    pub value_extent: SegmentExtentV1,
    pub physical_value_sha256: Digest256,
    pub logical_value_bytes: u64,
    pub logical_value_sha256: Digest256,
}

/// One authenticated page visit from a successful or in-progress V2 walk.
/// The source owner must retain callback effects privately until EOF closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexPageVisitV2 {
    pub extent: SegmentExtentV1,
    pub page_sha256: Digest256,
    pub keyspace: SegmentIndexKeySpaceV1,
    pub kind: SegmentIndexPageKindV1,
    pub physical_summary: SegmentIndexSummaryV1,
    pub logical_summary: SegmentIndexLogicalSummaryV2,
}

/// Summaries returned only after the full tree reaches EOF and closes at root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexClosureV2 {
    pub root: SegmentIndexRootV2,
    pub physical_summary: SegmentIndexSummaryV1,
    pub logical_summary: SegmentIndexLogicalSummaryV2,
    pub page_count: u64,
}

/// Result classification for a bounded point or mutation-path lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SegmentIndexMutationSelectionV2 {
    Contains,
    Gap,
    BeforeFirst,
    AfterLast,
    EmptyRoot,
}

/// One synchronous callback in a root-to-neighbor-leaf mutation path.
/// `page` and adjacent keys borrow the single checked page buffer and must not
/// be retained past the callback. Selection bounds describe this page's
/// actual containing interval or neighboring endpoints.
#[derive(Clone, Copy, Debug)]
pub struct SegmentIndexMutationPathVisitV2<'a> {
    pub extent: SegmentExtentV1,
    pub page_sha256: Digest256,
    pub page: DecodedSegmentIndexPageV2<'a>,
    pub physical_summary: SegmentIndexSummaryV1,
    pub logical_summary: SegmentIndexLogicalSummaryV2,
    pub selected_child_ordinal: Option<u32>,
    pub selection_kind: SegmentIndexMutationSelectionV2,
    pub adjacent_lower_key: Option<&'a [u8]>,
    pub adjacent_upper_key: Option<&'a [u8]>,
}

/// Bounded mechanical receipt. It binds the route to the supplied root and
/// query digest; it is not a native transaction, absence proof, or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexMutationPathReceiptV2 {
    pub root: SegmentIndexRootV2,
    pub key_sha256: Digest256,
    pub key_len: u64,
    /// Number of child edges from root to the selected leaf (root is depth 0).
    pub path_depth: u32,
    /// Number of checked pages, including root.
    pub page_count: u32,
    pub leaf_present: bool,
    pub canonical_missing: bool,
    pub selection_kind: SegmentIndexMutationSelectionV2,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Stamp {
    dev: u64,
    ino: u64,
    size: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}

impl Stamp {
    fn from(metadata: &Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            size: metadata.len(),
            mode: metadata.mode(),
            uid: metadata.uid(),
            gid: metadata.gid(),
            mtime: metadata.mtime(),
            mtime_ns: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_ns: metadata.ctime_nsec(),
        }
    }
}

pub struct SegmentIndexReaderV2 {
    directory: File,
    directory_identity: (u64, u64, u32, u32, u32),
    root: SegmentIndexRootV2,
    limits: SegmentIndexReadLimitsV2,
    io: PinnedSqliteIoBudget,
    space: PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

fn refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn corrupt(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::CorruptSelectedObject, detail)
}

impl SegmentIndexReaderV2 {
    /// Transfer a securely opened segment-directory capability. The root is a
    /// typed mechanical descriptor; selection and admission remain external.
    pub fn from_held_directory(
        directory: File,
        root: SegmentIndexRootV2,
        limits: SegmentIndexReadLimitsV2,
        io: PinnedSqliteIoBudget,
        space: PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        crate::streamed_cut::check_time_budgeted(deadline, &cancelled, Some(&io))?;
        let shared_space = space.snapshot();
        if !shared_space.ledger_consistent || shared_space.allocation_anomalies != 0 {
            return Err(refusal("V2 segment space ledger is unhealthy"));
        }
        let physical = limits.pages.physical;
        let read_workspace = Self::workspace_upper_bound()?;
        if limits.max_depth == 0
            || limits.max_depth > MAX_V2_DEPTH
            || limits.max_workspace_bytes < read_workspace
            || limits.max_workspace_bytes == usize::MAX
            || limits.max_value_bytes == 0
            || limits.max_value_bytes == u64::MAX
            || limits.max_value_bytes > physical.max_value_bytes
            || !(SEGMENT_INDEX_PAGE_HEADER_BYTES_V2..=SEGMENT_INDEX_PAGE_MAX_BYTES_V2)
                .contains(&physical.max_page_bytes)
            || physical.max_entries == u32::MAX
            || physical.max_key_bytes == 0
            || physical.max_key_bytes > SEGMENT_INDEX_KEY_MAX_BYTES_V2
            || physical.max_segment_bytes == 0
            || physical.max_segment_bytes == u64::MAX
            || physical.max_row_count == u64::MAX
            || physical.max_value_bytes == u64::MAX
            || physical.max_encoded_page_bytes < SEGMENT_INDEX_PAGE_HEADER_BYTES_V2 as u64
            || physical.max_encoded_page_bytes == u64::MAX
            || limits.pages.max_logical_key_bytes == u64::MAX
            || limits.pages.max_logical_value_bytes == u64::MAX
            || limits.max_workspace_bytes == 0
            || root.root_extent.length < SEGMENT_INDEX_PAGE_HEADER_BYTES_V2 as u64
            || root.root_extent.length > physical.max_page_bytes as u64
            || root
                .root_extent
                .offset
                .checked_add(root.root_extent.length)
                .is_none_or(|end| end > physical.max_segment_bytes)
            || root.physical_summary.row_count > physical.max_row_count
            || root.physical_summary.value_bytes > physical.max_value_bytes
            || root.physical_summary.encoded_page_bytes > physical.max_encoded_page_bytes
            || root.physical_summary.encoded_page_bytes < root.root_extent.length
            || root.logical_summary.row_count != root.physical_summary.row_count
            || root.logical_summary.key_bytes > limits.pages.max_logical_key_bytes
            || root.logical_summary.value_bytes > limits.pages.max_logical_value_bytes
        {
            return Err(refusal("segment-index V2 finite read profile"));
        }
        let metadata = directory
            .metadata()
            .map_err(|error| StoreError::io("cannot stat segment directory", error))?;
        if !metadata.is_dir() {
            return Err(StoreError::new(
                StoreErrorCode::InvalidRoot,
                "segment root is not a directory",
            ));
        }
        let reader = Self {
            directory,
            directory_identity: (
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                metadata.uid(),
                metadata.gid(),
            ),
            root,
            limits,
            io,
            space,
            deadline,
            cancelled,
        };
        reader.active()?;
        Ok(reader)
    }

    /// Logical simultaneous reader workspace. It includes one page buffer,
    /// the range-verification buffer, bounded lookup keys, V2 logical hashing,
    /// and inline page/mutation callback DTOs; caller state remains additional.
    pub fn workspace_upper_bound() -> Result<usize> {
        [
            std::mem::size_of::<Self>(),
            V2_PAGE_BUFFER_BYTES,
            V2_KEY_BUFFER_BYTES
                .checked_mul(2)
                .ok_or_else(|| refusal("V2 key workspace overflowed"))?,
            V2_RANGE_BUFFER_BYTES,
            std::mem::size_of::<DecodedSegmentIndexPageV2<'static>>(),
            std::mem::size_of::<SegmentIndexEntryCursorV2>(),
            std::mem::size_of::<SegmentIndexEntryV2<'static>>(),
            std::mem::size_of::<SegmentIndexLeafEntryV2<'static>>(),
            std::mem::size_of::<SegmentIndexInternalEntryV2<'static>>(),
            std::mem::size_of::<Digest256Hasher>(),
            std::mem::size_of::<SegmentIndexPageVisitV2>(),
            std::mem::size_of::<SegmentIndexMutationPathVisitV2<'static>>(),
            std::mem::size_of::<SegmentIndexMutationPathReceiptV2>(),
            V2_FIXED_WORKSPACE_BYTES,
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
        .ok_or_else(|| refusal("V2 reader workspace overflowed"))
    }

    pub fn shares_request(
        &self,
        io: &PinnedSqliteIoBudget,
        space: &PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: &Arc<AtomicBool>,
    ) -> bool {
        self.io.shares_with(io)
            && self.space.shares_with(space)
            && self.deadline == deadline
            && Arc::ptr_eq(&self.cancelled, cancelled)
    }

    pub const fn keyspace(&self) -> SegmentIndexKeySpaceV1 {
        self.root.keyspace
    }

    pub fn shares_reader_request(&self, other: &Self) -> bool {
        self.shares_request(&other.io, &other.space, other.deadline, &other.cancelled)
    }

    pub(crate) const fn memo_directory_identity(&self) -> (u64, u64, u32, u32, u32) {
        self.directory_identity
    }

    pub(crate) fn check_request(&self) -> Result<()> {
        self.active()
    }

    pub const fn root_descriptor(&self) -> SegmentIndexRootV2 {
        self.root
    }

    pub const fn read_limits(&self) -> SegmentIndexReadLimitsV2 {
        self.limits
    }

    pub(crate) fn read_page_extent(
        &self,
        extent: SegmentExtentV1,
        page_sha256: Digest256,
        sink: &mut impl Write,
    ) -> Result<u64> {
        self.read_extent(
            extent,
            page_sha256,
            self.limits.pages.physical.max_page_bytes as u64,
            sink,
        )
    }

    pub(crate) fn read_located_value(
        &self,
        value: SegmentLocatedValueV2,
        sink: &mut impl Write,
    ) -> Result<u64> {
        self.read_extent(
            value.value_extent,
            value.physical_value_sha256,
            self.limits.max_value_bytes,
            sink,
        )
    }

    /// Verify and stream a caller-selected exact range through this reader's
    /// original directory, I/O ledger, space identity, request clock, and
    /// cancellation capability. This does not authenticate descriptor
    /// selection or its logical relationship to another source record.
    pub fn read_verified_extent(
        &self,
        extent: SegmentExtentV1,
        raw_sha256: Digest256,
        max_bytes: u64,
        sink: &mut impl Write,
    ) -> Result<u64> {
        if max_bytes == 0
            || max_bytes == u64::MAX
            || max_bytes > self.limits.max_value_bytes
            || extent.length > max_bytes
        {
            return Err(refusal("V2 verified extent exceeds its finite byte limit"));
        }
        self.read_extent(extent, raw_sha256, max_bytes, sink)
    }

    fn active(&self) -> Result<()> {
        crate::streamed_cut::check_time_budgeted(self.deadline, &self.cancelled, Some(&self.io))?;
        let space = self.space.snapshot();
        if !space.ledger_consistent || space.allocation_anomalies != 0 {
            return Err(refusal("V2 segment space ledger is unhealthy"));
        }
        let metadata = self
            .directory
            .metadata()
            .map_err(|error| StoreError::io("cannot recheck segment directory", error))?;
        if !metadata.is_dir()
            || (
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                metadata.uid(),
                metadata.gid(),
            ) != self.directory_identity
        {
            return Err(StoreError::new(
                StoreErrorCode::UnsafePath,
                "held segment directory changed",
            ));
        }
        Ok(())
    }

    fn read_extent(
        &self,
        extent: SegmentExtentV1,
        expected_sha256: Digest256,
        object_cap: u64,
        sink: &mut impl Write,
    ) -> Result<u64> {
        self.active()?;
        let name = extent.segment_sha256.to_hex();
        let file = tos_fd_open::open_regular_at(&self.directory, Path::new(&name))
            .map_err(|_| StoreError::new(StoreErrorCode::UnsafePath, "cannot pin segment inode"))?;
        let before = Stamp::from(
            &file
                .metadata()
                .map_err(|error| StoreError::io("cannot stat selected segment", error))?,
        );
        let result = verify_segment_range(
            &file,
            extent.offset,
            extent.length,
            expected_sha256,
            self.limits.pages.physical.max_segment_bytes,
            object_cap,
            &self.io,
            self.deadline,
            &self.cancelled,
            sink,
        );
        let fence = (|| -> Result<()> {
            let held = Stamp::from(
                &file
                    .metadata()
                    .map_err(|error| StoreError::io("cannot recheck held segment", error))?,
            );
            let named =
                tos_fd_open::open_regular_at(&self.directory, Path::new(&name)).map_err(|_| {
                    StoreError::new(StoreErrorCode::UnsafePath, "selected segment name changed")
                })?;
            let named = Stamp::from(
                &named
                    .metadata()
                    .map_err(|error| StoreError::io("cannot recheck named segment", error))?,
            );
            if before != held || held != named {
                return Err(corrupt("selected segment held/named identity changed"));
            }
            self.active()
        })();
        match result {
            Err(error) => Err(error),
            Ok(bytes) => {
                fence?;
                Ok(bytes)
            }
        }
    }

    pub(crate) fn read_checked_page<'a>(
        &self,
        extent: SegmentExtentV1,
        page_sha256: Digest256,
        raw: &'a mut [u8; V2_PAGE_BUFFER_BYTES],
    ) -> Result<DecodedSegmentIndexPageV2<'a>> {
        let length = usize::try_from(extent.length)
            .map_err(|_| refusal("V2 page extent length does not fit memory"))?;
        if length < SEGMENT_INDEX_PAGE_HEADER_BYTES_V2
            || length > self.limits.pages.physical.max_page_bytes
            || length > raw.len()
        {
            return Err(refusal("V2 page exceeds its read limit"));
        }
        self.active()?;
        let (read_bytes, position) = {
            let mut sink = Cursor::new(&mut raw[..length]);
            let read_bytes = self.read_page_extent(extent, page_sha256, &mut sink)?;
            (read_bytes, sink.position())
        };
        self.active()?;
        if read_bytes != extent.length || position != extent.length {
            return Err(corrupt("V2 page range length differs"));
        }
        self.active()?;
        let page = decode_segment_index_page_v2(
            &raw[..length],
            page_sha256,
            self.root.keyspace,
            self.limits.pages,
        )?;
        self.active()?;
        Ok(page)
    }

    /// Authenticate only the selected search path. Full root closure and
    /// native admission/currentness remain owner responsibilities.
    pub fn get(&self, key: &[u8]) -> Result<Option<SegmentLocatedValueV2>> {
        self.validate_lookup_key(key)?;
        self.active()?;
        let mut raw = [0u8; V2_PAGE_BUFFER_BYTES];
        let mut expected = SegmentIndexPageExpectationV2::root(self.root);
        for depth in 0..self.limits.max_depth {
            self.active()?;
            let page = self.read_checked_page(expected.extent, expected.page_sha256, &mut raw)?;
            check_page_binding(
                page,
                expected.physical_summary,
                expected.logical_summary,
                expected.lower_bound(),
                expected.upper_bound(),
                depth == 0,
            )?;
            self.active()?;
            match page.kind() {
                SegmentIndexPageKindV1::Leaf => {
                    let found = page.find_leaf(key)?.map(located_value);
                    self.active()?;
                    return Ok(found);
                }
                SegmentIndexPageKindV1::Internal => {
                    let Some(child) = page.child_for_key(key)? else {
                        self.active()?;
                        return Ok(None);
                    };
                    expected.set_child(child)?;
                    if depth + 1 == self.limits.max_depth {
                        return Err(refusal("V2 point path exceeds its depth limit"));
                    }
                }
            }
        }
        Err(refusal("V2 point path exceeds its depth limit"))
    }

    /// Stream one selected physical value. ObjectDigest rows additionally bind
    /// their exact 40-byte logical `(length, raw SHA-256)` tuple before any
    /// object bytes are read.
    pub fn read_value(&self, key: &[u8], sink: &mut impl Write) -> Result<Option<u64>> {
        let Some(value) = self.get(key)? else {
            return Ok(None);
        };
        if self.root.keyspace == SegmentIndexKeySpaceV1::ObjectDigest {
            validate_object_digest_value(key, value)?;
        }
        self.read_located_value(value, sink).map(Some)
    }

    /// Deliver the authenticated root-to-neighbor-leaf path for a COW key.
    /// Internal gaps route to the predecessor child, while before-first and
    /// after-last route to the nearest edge child. Point `get` retains its
    /// stricter containing-only absence behavior.
    pub fn visit_mutation_path_for_key(
        &self,
        key: &[u8],
        max_workspace_bytes: usize,
        visitor: &mut impl FnMut(SegmentIndexMutationPathVisitV2<'_>) -> Result<()>,
    ) -> Result<SegmentIndexMutationPathReceiptV2> {
        self.validate_lookup_key(key)?;
        self.active()?;
        self.check_workspace(max_workspace_bytes, Self::workspace_upper_bound()?)?;

        let key_len = u64::try_from(key.len())
            .map_err(|_| refusal("V2 mutation key length does not fit u64"))?;
        let mut expectation = SegmentIndexPageExpectationV2::root(self.root);
        let mut raw = [0u8; V2_PAGE_BUFFER_BYTES];
        let mut path_depth = 0u32;
        let mut page_count = 0u32;
        let mut overall_selection = SegmentIndexMutationSelectionV2::Contains;

        for depth in 0..self.limits.max_depth {
            self.active()?;
            let page =
                self.read_checked_page(expectation.extent, expectation.page_sha256, &mut raw)?;
            check_page_binding(
                page,
                expectation.physical_summary,
                expectation.logical_summary,
                expectation.lower_bound(),
                expectation.upper_bound(),
                depth == 0,
            )?;
            self.active()?;

            let (
                selected_child,
                selected_child_ordinal,
                selection_kind,
                adjacent_lower_key,
                adjacent_upper_key,
            ) = match page.kind() {
                SegmentIndexPageKindV1::Leaf => {
                    let (selection, lower, upper, found) =
                        classify_leaf(self, page, key, depth == 0)?;
                    if overall_selection == SegmentIndexMutationSelectionV2::Contains {
                        overall_selection = selection;
                    }
                    if found.is_some()
                        && overall_selection != SegmentIndexMutationSelectionV2::Contains
                    {
                        return Err(corrupt("V2 leaf contradicts its selected parent interval"));
                    }
                    (None, None, selection, lower, upper)
                }
                SegmentIndexPageKindV1::Internal => {
                    let route = route_mutation_key(self, page, key)?;
                    if overall_selection == SegmentIndexMutationSelectionV2::Contains
                        && route.selection != SegmentIndexMutationSelectionV2::Contains
                    {
                        overall_selection = route.selection;
                    }
                    (
                        Some(route.child),
                        Some(route.ordinal),
                        route.selection,
                        route.adjacent_lower_key,
                        route.adjacent_upper_key,
                    )
                }
            };
            self.active()?;
            let visit = SegmentIndexMutationPathVisitV2 {
                extent: expectation.extent,
                page_sha256: expectation.page_sha256,
                page,
                physical_summary: page.physical_summary(),
                logical_summary: page.logical_summary(),
                selected_child_ordinal,
                selection_kind,
                adjacent_lower_key,
                adjacent_upper_key,
            };
            let callback_result = visitor(visit);
            let callback_fence = self.active();
            if let Err(error) = callback_result {
                return Err(error);
            }
            callback_fence?;
            page_count = page_count
                .checked_add(1)
                .ok_or_else(|| refusal("V2 mutation page count overflowed"))?;

            match page.kind() {
                SegmentIndexPageKindV1::Leaf => {
                    let leaf_present = selection_kind == SegmentIndexMutationSelectionV2::Contains;
                    if leaf_present
                        && overall_selection != SegmentIndexMutationSelectionV2::Contains
                    {
                        return Err(corrupt("V2 selected leaf contradicts a parent gap"));
                    }
                    let key_sha256 = Digest256::of_bytes(key);
                    self.active()?;
                    return Ok(SegmentIndexMutationPathReceiptV2 {
                        root: self.root,
                        key_sha256,
                        key_len,
                        path_depth,
                        page_count,
                        leaf_present,
                        canonical_missing: !leaf_present,
                        selection_kind: overall_selection,
                    });
                }
                SegmentIndexPageKindV1::Internal => {
                    if depth + 1 == self.limits.max_depth {
                        return Err(refusal("V2 mutation path exceeds its depth limit"));
                    }
                    let child = selected_child
                        .ok_or_else(|| corrupt("V2 mutation route omitted its child"))?;
                    expectation.set_child(child)?;
                    path_depth = path_depth
                        .checked_add(1)
                        .ok_or_else(|| refusal("V2 mutation path depth overflowed"))?;
                }
            }
        }
        Err(refusal("V2 mutation path exceeds its depth limit"))
    }

    fn validate_lookup_key(&self, key: &[u8]) -> Result<()> {
        if key.is_empty() || key.len() > self.limits.pages.physical.max_key_bytes {
            return Err(refusal("V2 lookup key is empty or exceeds its byte limit"));
        }
        if self.root.keyspace == SegmentIndexKeySpaceV1::ObjectDigest
            && key.len() != OBJECT_DIGEST_KEY_BYTES
        {
            return Err(corrupt("ObjectDigest lookup key must contain 32 bytes"));
        }
        Ok(())
    }

    fn check_workspace(&self, requested: usize, required: usize) -> Result<()> {
        if requested == 0
            || requested == usize::MAX
            || requested < required
            || requested > self.limits.max_workspace_bytes
        {
            return Err(refusal("V2 operation workspace exceeds its caller limit"));
        }
        self.active()
    }
}

/// Exact copied expectation carried from a parent child entry to its page.
pub(crate) struct SegmentIndexPageExpectationV2 {
    pub(crate) extent: SegmentExtentV1,
    pub(crate) page_sha256: Digest256,
    pub(crate) physical_summary: SegmentIndexSummaryV1,
    pub(crate) logical_summary: SegmentIndexLogicalSummaryV2,
    lower: [u8; V2_KEY_BUFFER_BYTES],
    lower_len: u16,
    upper: [u8; V2_KEY_BUFFER_BYTES],
    upper_len: u16,
}

impl SegmentIndexPageExpectationV2 {
    pub(crate) fn root(root: SegmentIndexRootV2) -> Self {
        Self {
            extent: root.root_extent,
            page_sha256: root.root_page_sha256,
            physical_summary: root.physical_summary,
            logical_summary: root.logical_summary,
            lower: [0u8; V2_KEY_BUFFER_BYTES],
            lower_len: 0,
            upper: [0u8; V2_KEY_BUFFER_BYTES],
            upper_len: 0,
        }
    }

    pub(crate) fn child(child: SegmentIndexInternalEntryV2<'_>) -> Result<Self> {
        let mut expectation = Self {
            extent: child.child_extent,
            page_sha256: child.child_page_sha256,
            physical_summary: child.physical_summary,
            logical_summary: child.logical_summary,
            lower: [0u8; V2_KEY_BUFFER_BYTES],
            lower_len: 0,
            upper: [0u8; V2_KEY_BUFFER_BYTES],
            upper_len: 0,
        };
        expectation.set_child(child)?;
        Ok(expectation)
    }

    pub(crate) fn set_child(&mut self, child: SegmentIndexInternalEntryV2<'_>) -> Result<()> {
        if child.lower_key.is_empty()
            || child.upper_key.is_empty()
            || child.lower_key.len() > SEGMENT_INDEX_KEY_MAX_BYTES_V2
            || child.upper_key.len() > SEGMENT_INDEX_KEY_MAX_BYTES_V2
            || child.lower_key > child.upper_key
        {
            return Err(corrupt("V2 child key bounds are invalid"));
        }
        let lower_len = u16::try_from(child.lower_key.len())
            .map_err(|_| refusal("V2 child lower bound exceeds its fixed buffer"))?;
        let upper_len = u16::try_from(child.upper_key.len())
            .map_err(|_| refusal("V2 child upper bound exceeds its fixed buffer"))?;
        self.lower[..child.lower_key.len()].copy_from_slice(child.lower_key);
        self.upper[..child.upper_key.len()].copy_from_slice(child.upper_key);
        self.lower_len = lower_len;
        self.upper_len = upper_len;
        self.extent = child.child_extent;
        self.page_sha256 = child.child_page_sha256;
        self.physical_summary = child.physical_summary;
        self.logical_summary = child.logical_summary;
        Ok(())
    }

    pub(crate) fn lower_bound(&self) -> Option<&[u8]> {
        (self.lower_len != 0).then_some(&self.lower[..usize::from(self.lower_len)])
    }

    pub(crate) fn upper_bound(&self) -> Option<&[u8]> {
        (self.upper_len != 0).then_some(&self.upper[..usize::from(self.upper_len)])
    }
}

pub(crate) fn check_page_binding(
    page: DecodedSegmentIndexPageV2<'_>,
    expected_physical: SegmentIndexSummaryV1,
    expected_logical: SegmentIndexLogicalSummaryV2,
    lower: Option<&[u8]>,
    upper: Option<&[u8]>,
    is_root: bool,
) -> Result<()> {
    if page.physical_summary() != expected_physical {
        return Err(corrupt("V2 page physical summary differs from its parent"));
    }
    if page.logical_summary() != expected_logical {
        return Err(corrupt("V2 page logical summary differs from its parent"));
    }
    if page.entry_count() == 0 && (!is_root || page.kind() != SegmentIndexPageKindV1::Leaf) {
        return Err(corrupt("only the V2 root may be an empty leaf"));
    }
    if lower.is_some() != upper.is_some() {
        return Err(corrupt("V2 child must carry both stored key bounds"));
    }
    if let Some(lower) = lower {
        if page.first_key() != Some(lower) {
            return Err(corrupt(
                "V2 child first key differs from its stored lower bound",
            ));
        }
    }
    if let Some(upper) = upper {
        if page.last_key() != Some(upper) {
            return Err(corrupt(
                "V2 child last key differs from its stored upper bound",
            ));
        }
    }
    Ok(())
}

pub(crate) fn located_value(entry: SegmentIndexLeafEntryV2<'_>) -> SegmentLocatedValueV2 {
    SegmentLocatedValueV2 {
        value_extent: entry.value_extent,
        physical_value_sha256: entry.physical_value_sha256,
        logical_value_bytes: entry.logical_value_bytes,
        logical_value_sha256: entry.logical_value_sha256,
    }
}

/// Preserve the ObjectDigest row's exact 40-byte logical tuple binding before
/// any raw object range is read.
pub(crate) fn validate_object_digest_value(key: &[u8], value: SegmentLocatedValueV2) -> Result<()> {
    if key.len() != OBJECT_DIGEST_KEY_BYTES
        || key != &value.physical_value_sha256.as_bytes()[..]
        || value.logical_value_bytes != OBJECT_LOGICAL_VALUE_BYTES as u64
    {
        return Err(corrupt("V2 ObjectDigest value descriptor differs"));
    }
    let mut logical_value = [0u8; OBJECT_LOGICAL_VALUE_BYTES];
    logical_value[..8].copy_from_slice(&value.value_extent.length.to_le_bytes());
    logical_value[8..].copy_from_slice(value.physical_value_sha256.as_bytes());
    if Digest256::of_bytes(&logical_value) != value.logical_value_sha256 {
        return Err(corrupt("V2 ObjectDigest logical tuple digest differs"));
    }
    Ok(())
}

struct MutationRouteV2<'a> {
    child: SegmentIndexInternalEntryV2<'a>,
    ordinal: u32,
    selection: SegmentIndexMutationSelectionV2,
    adjacent_lower_key: Option<&'a [u8]>,
    adjacent_upper_key: Option<&'a [u8]>,
}

fn route_mutation_key<'a>(
    reader: &SegmentIndexReaderV2,
    page: DecodedSegmentIndexPageV2<'a>,
    key: &[u8],
) -> Result<MutationRouteV2<'a>> {
    if page.kind() != SegmentIndexPageKindV1::Internal || page.entry_count() == 0 {
        return Err(corrupt(
            "V2 mutation route requires a nonempty internal page",
        ));
    }
    let mut cursor = page.initial_cursor();
    let mut ordinal = 0u32;
    let mut previous: Option<(u32, SegmentIndexInternalEntryV2<'a>)> = None;
    loop {
        reader.check_request()?;
        let Some(entry) = page.next_entry(&mut cursor)? else {
            break;
        };
        reader.check_request()?;
        let SegmentIndexEntryV2::Internal(child) = entry else {
            return Err(corrupt("V2 internal page contains a leaf entry"));
        };
        if key < child.lower_key {
            if let Some((previous_ordinal, predecessor)) = previous {
                return Ok(MutationRouteV2 {
                    child: predecessor,
                    ordinal: previous_ordinal,
                    selection: SegmentIndexMutationSelectionV2::Gap,
                    adjacent_lower_key: Some(predecessor.upper_key),
                    adjacent_upper_key: Some(child.lower_key),
                });
            }
            return Ok(MutationRouteV2 {
                child,
                ordinal,
                selection: SegmentIndexMutationSelectionV2::BeforeFirst,
                adjacent_lower_key: None,
                adjacent_upper_key: Some(child.lower_key),
            });
        }
        if key <= child.upper_key {
            return Ok(MutationRouteV2 {
                child,
                ordinal,
                selection: SegmentIndexMutationSelectionV2::Contains,
                adjacent_lower_key: Some(child.lower_key),
                adjacent_upper_key: Some(child.upper_key),
            });
        }
        previous = Some((ordinal, child));
        ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| refusal("V2 mutation child ordinal overflowed"))?;
    }
    let Some((ordinal, child)) = previous else {
        return Err(corrupt("V2 internal page has no mutation neighbor"));
    };
    Ok(MutationRouteV2 {
        child,
        ordinal,
        selection: SegmentIndexMutationSelectionV2::AfterLast,
        adjacent_lower_key: Some(child.upper_key),
        adjacent_upper_key: None,
    })
}

fn classify_leaf<'a>(
    reader: &SegmentIndexReaderV2,
    page: DecodedSegmentIndexPageV2<'a>,
    key: &[u8],
    is_root: bool,
) -> Result<(
    SegmentIndexMutationSelectionV2,
    Option<&'a [u8]>,
    Option<&'a [u8]>,
    Option<SegmentIndexLeafEntryV2<'a>>,
)> {
    if page.kind() != SegmentIndexPageKindV1::Leaf {
        return Err(corrupt("V2 leaf classification received an internal page"));
    }
    if page.entry_count() == 0 {
        if is_root {
            return Ok((SegmentIndexMutationSelectionV2::EmptyRoot, None, None, None));
        }
        return Err(corrupt("V2 non-root leaf page is empty"));
    }
    let mut cursor = page.initial_cursor();
    let mut ordinal = 0u32;
    let mut previous: Option<&'a [u8]> = None;
    loop {
        reader.check_request()?;
        let Some(entry) = page.next_entry(&mut cursor)? else {
            break;
        };
        reader.check_request()?;
        let SegmentIndexEntryV2::Leaf(leaf) = entry else {
            return Err(corrupt("V2 leaf page contains an internal entry"));
        };
        if key == leaf.key {
            return Ok((
                SegmentIndexMutationSelectionV2::Contains,
                Some(leaf.key),
                Some(leaf.key),
                Some(leaf),
            ));
        }
        if key < leaf.key {
            return Ok(match previous {
                Some(lower) => (
                    SegmentIndexMutationSelectionV2::Gap,
                    Some(lower),
                    Some(leaf.key),
                    None,
                ),
                None => (
                    SegmentIndexMutationSelectionV2::BeforeFirst,
                    None,
                    Some(leaf.key),
                    None,
                ),
            });
        }
        previous = Some(leaf.key);
        ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| refusal("V2 leaf ordinal overflowed"))?;
    }
    if ordinal != page.entry_count() {
        return Err(corrupt("V2 leaf cursor ended before its page"));
    }
    let Some(last) = previous else {
        return Err(corrupt("V2 non-root leaf page has no keys"));
    };
    Ok((
        SegmentIndexMutationSelectionV2::AfterLast,
        Some(last),
        None,
        None,
    ))
}
