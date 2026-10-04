//! Bounded ordered closure walk over an authenticated packed segment index.
//!
//! The walk verifies mechanical tree closure and exposes leaf rows in their
//! encoded order. It does not select a root or establish native completion,
//! admission, rights, or currentness.

use std::io::Cursor;
use std::mem::size_of;

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, StoreError, StoreErrorCode};
use crate::segment_index_read::SegmentIndexReaderV1;
use crate::segment_locator::{
    DecodedSegmentIndexPageV1, SEGMENT_INDEX_KEY_MAX_BYTES_V1, SEGMENT_INDEX_PAGE_HEADER_BYTES_V1,
    SegmentExtentV1, SegmentIndexEntryCursorV1, SegmentIndexEntryV1, SegmentIndexInternalEntryV1,
    SegmentIndexKeySpaceV1, SegmentIndexLeafEntryV1, SegmentIndexPageKindV1, SegmentIndexRootV1,
    SegmentIndexSummaryV1,
};

const PAGE_BUFFER_BYTES: usize = 64 * 1024;
const RANGE_TRANSFER_BUFFER_BYTES: usize = 64 * 1024;
const MAX_WALK_DEPTH: usize = 128;
const FIXED_METADATA_AND_CONTROL_BYTES: usize = 4096;

/// Authenticated mechanical description of one index page already checked by
/// the ordered walk. It does not establish root selection or source authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexPageVisitV1 {
    pub extent: SegmentExtentV1,
    pub page_sha256: Digest256,
    pub keyspace: SegmentIndexKeySpaceV1,
    pub kind: SegmentIndexPageKindV1,
    pub summary: SegmentIndexSummaryV1,
}

impl SegmentIndexReaderV1 {
    /// Conservative logical workspace required for a full ordered walk at the
    /// reader's configured maximum depth. The caller must precharge this
    /// amount against its operation envelope before invoking either walk API.
    /// This includes the reader, DFS stack, simultaneous page/range buffers,
    /// and fixed control state. It is a logical source estimate, not measured
    /// RSS or a physical allocator guarantee; callback and sink-owned storage
    /// remain caller costs.
    pub fn walk_workspace_upper_bound(&self) -> Result<usize> {
        self.check_request()?;
        let limits = self.read_limits();
        let depth = usize::try_from(limits.max_depth)
            .map_err(|_| refusal("segment-index walk depth does not fit memory"))?;
        if depth == 0 || depth > MAX_WALK_DEPTH {
            return Err(refusal("segment-index walk depth exceeds its finite limit"));
        }
        let workspace = self.workspace_for_frame_capacity(depth)?;
        self.check_request()?;
        Ok(workspace)
    }

    /// Visit every leaf entry in strict byte-key order, then return the actual
    /// row/value/page totals only after the entire tree reaches true EOF.
    /// Visitor side effects must remain private until this method returns its
    /// summary because a later row or closure check can still fail.
    pub fn walk_leaves(
        &self,
        max_workspace_bytes: usize,
        visitor: &mut impl FnMut(SegmentIndexLeafEntryV1<'_>) -> Result<()>,
    ) -> Result<SegmentIndexSummaryV1> {
        self.walk_pages_and_leaves(max_workspace_bytes, &mut |_| Ok(()), visitor)
    }

    /// Visit each checked page frame once, including the root, and stream all
    /// leaf entries in strict byte-key order. Internal pages are reread when
    /// resuming after a child, but their page callback is emitted only on the
    /// frame's first checked read. Reads use the reader's shared budget and
    /// original request clock. Earlier callback calls may have run if a later
    /// row or closure check fails, so callers must keep their side effects
    /// private until this method returns its summary.
    pub fn walk_pages_and_leaves(
        &self,
        max_workspace_bytes: usize,
        page_visitor: &mut impl FnMut(SegmentIndexPageVisitV1) -> Result<()>,
        leaf_visitor: &mut impl FnMut(SegmentIndexLeafEntryV1<'_>) -> Result<()>,
    ) -> Result<SegmentIndexSummaryV1> {
        self.check_request()?;
        let limits = self.read_limits();
        let depth_limit = usize::try_from(limits.max_depth)
            .map_err(|_| refusal("segment-index walk depth does not fit memory"))?;
        if depth_limit == 0
            || depth_limit > MAX_WALK_DEPTH
            || limits.max_workspace_bytes == 0
            || limits.max_workspace_bytes == usize::MAX
            || max_workspace_bytes == 0
            || max_workspace_bytes == usize::MAX
            || limits.pages.max_page_bytes < SEGMENT_INDEX_PAGE_HEADER_BYTES_V1
            || limits.pages.max_page_bytes > PAGE_BUFFER_BYTES
        {
            return Err(refusal("segment-index walk profile is not finite"));
        }

        // Treat both the method argument and the reader's profile as already
        // caller-priced ceilings. Refuse before asking the allocator for the
        // bounded DFS capacity or constructing the page buffer.
        let required_workspace = self.workspace_for_frame_capacity(depth_limit)?;
        if required_workspace > max_workspace_bytes
            || required_workspace > limits.max_workspace_bytes
        {
            return Err(refusal(
                "segment-index walk workspace exceeds its precharged limit",
            ));
        }
        self.check_request()?;
        let mut stack = Vec::<WalkFrame>::new();
        stack
            .try_reserve_exact(depth_limit)
            .map_err(|_| refusal("segment-index walk frame reservation failed"))?;
        self.check_request()?;
        let actual_workspace = self.workspace_for_frame_capacity(stack.capacity())?;
        if stack.capacity() < depth_limit
            || actual_workspace > max_workspace_bytes
            || actual_workspace > limits.max_workspace_bytes
        {
            return Err(refusal(
                "segment-index walk allocator capacity exceeds its precharged limit",
            ));
        }

        let root = self.root_descriptor();
        let mut control = WalkControl {
            root,
            pages: limits.pages,
            depth_limit,
            actual: SegmentIndexSummaryV1 {
                row_count: 0,
                value_bytes: 0,
                encoded_page_bytes: 0,
            },
        };
        self.check_request()?;
        let root_frame = WalkFrame::root(root.root_extent, root.root_page_sha256, root.summary());
        self.check_request()?;
        stack.push(root_frame);
        self.check_request()?;
        let mut page_bytes = [0u8; PAGE_BUFFER_BYTES];

        while !stack.is_empty() {
            self.check_request()?;
            let top = stack.len() - 1;

            // After the last child returns, the parent summary and all its
            // descendants have been checked; no extra page reread is needed.
            let finished_internal = {
                let frame = &stack[top];
                frame.child_count != 0 && frame.next_child == frame.child_count
            };
            if finished_internal {
                stack.pop();
                continue;
            }

            let frame = &stack[top];
            let page = self.read_checked_page(
                frame,
                &mut page_bytes,
                control.root.keyspace,
                control.pages,
            )?;
            self.check_request()?;

            let page_count = page.entry_count();
            let is_internal = page.kind() == SegmentIndexPageKindV1::Internal;
            {
                let frame = &stack[top];
                if frame.child_count != 0 && (!is_internal || frame.child_count != page_count) {
                    return Err(corrupt(
                        "segment-index page role changed during ordered walk",
                    ));
                }
            }

            if !stack[top].page_accounted {
                let visit = SegmentIndexPageVisitV1 {
                    extent: stack[top].extent,
                    page_sha256: stack[top].page_sha256,
                    keyspace: control.root.keyspace,
                    kind: page.kind(),
                    summary: page.summary(),
                };
                self.check_request()?;
                let callback_result = page_visitor(visit);
                let callback_fence = self.check_request();
                if let Err(error) = callback_result {
                    return Err(error);
                }
                callback_fence?;
                add_page_bytes(
                    &mut control.actual,
                    page_bytes_len(stack[top].extent)?,
                    control.root.summary(),
                    control.pages,
                )?;
                stack[top].page_accounted = true;
            }

            match page.kind() {
                SegmentIndexPageKindV1::Leaf => {
                    let mut ordinal = 0u32;
                    let mut cursor = page.initial_cursor();
                    loop {
                        self.check_request()?;
                        let entry = page.next_entry(&mut cursor)?;
                        self.check_request()?;
                        let Some(entry) = entry else {
                            break;
                        };
                        if ordinal >= page_count {
                            return Err(corrupt("segment-index leaf cursor exceeds its page"));
                        }
                        let SegmentIndexEntryV1::Leaf(leaf) = entry else {
                            return Err(corrupt(
                                "segment-index leaf page contains a non-leaf entry",
                            ));
                        };
                        add_leaf_row(
                            &mut control.actual,
                            leaf.value_extent.length,
                            control.root.summary(),
                            control.pages,
                        )?;
                        self.check_request()?;
                        let callback_result = leaf_visitor(leaf);
                        let callback_fence = self.check_request();
                        if let Err(error) = callback_result {
                            return Err(error);
                        }
                        callback_fence?;
                        ordinal = ordinal
                            .checked_add(1)
                            .ok_or_else(|| refusal("segment-index leaf ordinal overflowed"))?;
                    }
                    if ordinal != page_count {
                        return Err(corrupt("segment-index leaf cursor ended before its page"));
                    }
                    stack.pop();
                }
                SegmentIndexPageKindV1::Internal => {
                    if page_count == 0 {
                        return Err(corrupt("segment-index internal page contains no children"));
                    }
                    let next_child = stack[top].next_child;
                    if next_child >= page_count {
                        return Err(corrupt("segment-index child cursor exceeds its page"));
                    }
                    self.check_request()?;
                    let mut cursor = match stack[top].entry_cursor {
                        Some(cursor) => cursor,
                        None if next_child == 0 => page.initial_cursor(),
                        None => {
                            return Err(corrupt("segment-index internal cursor is missing"));
                        }
                    };
                    let current = page.next_entry(&mut cursor)?;
                    self.check_request()?;
                    let Some(SegmentIndexEntryV1::Internal(child)) = current else {
                        return Err(corrupt(
                            "segment-index internal page contains a non-internal entry",
                        ));
                    };
                    let next_index = next_child
                        .checked_add(1)
                        .ok_or_else(|| refusal("segment-index child ordinal overflowed"))?;
                    let next_separator = if next_index < page_count {
                        self.check_request()?;
                        let mut peek_cursor = cursor;
                        let next = page.next_entry(&mut peek_cursor)?;
                        self.check_request()?;
                        match next {
                            Some(SegmentIndexEntryV1::Internal(entry)) => Some(entry.key),
                            _ => {
                                return Err(corrupt("segment-index internal separator is missing"));
                            }
                        }
                    } else {
                        self.check_request()?;
                        let mut eof_cursor = cursor;
                        let eof = page.next_entry(&mut eof_cursor)?;
                        self.check_request()?;
                        if eof.is_some() {
                            return Err(corrupt("segment-index internal cursor exceeds its page"));
                        }
                        None
                    };
                    let parent_upper = stack[top].upper_bound();
                    let upper = next_separator.or(parent_upper);
                    self.check_request()?;
                    let child_frame = WalkFrame::child(child, upper)?;
                    self.check_request()?;
                    {
                        let parent = &mut stack[top];
                        if parent.child_count == 0 {
                            parent.child_count = page_count;
                        }
                        parent.entry_cursor = Some(cursor);
                        parent.next_child = parent
                            .next_child
                            .checked_add(1)
                            .ok_or_else(|| refusal("segment-index child cursor overflowed"))?;
                    }
                    self.check_request()?;
                    if stack.len() >= control.depth_limit {
                        return Err(refusal("segment-index walk exceeds its bounded depth"));
                    }
                    self.check_request()?;
                    stack.push(child_frame);
                    self.check_request()?;
                }
            }
        }

        self.check_request()?;
        if control.actual != control.root.summary() {
            return Err(corrupt(
                "segment-index ordered leaf closure differs from the root summary",
            ));
        }
        self.check_request()?;
        Ok(control.actual)
    }

    fn workspace_for_frame_capacity(&self, frame_capacity: usize) -> Result<usize> {
        let frame_slots = frame_capacity
            .checked_add(2)
            .ok_or_else(|| refusal("segment-index walk frame count overflowed"))?;
        let frames = size_of::<WalkFrame>()
            .checked_mul(frame_slots)
            .ok_or_else(|| refusal("segment-index walk frame workspace overflowed"))?;
        let fixed = [
            size_of::<Self>(),
            size_of::<Vec<WalkFrame>>(),
            size_of::<WalkControl>(),
            size_of::<[u8; PAGE_BUFFER_BYTES]>(),
            size_of::<[u8; RANGE_TRANSFER_BUFFER_BYTES]>(),
            size_of::<DecodedSegmentIndexPageV1<'static>>(),
            size_of::<Cursor<&mut [u8]>>(),
            size_of::<SegmentIndexEntryV1<'static>>(),
            size_of::<SegmentIndexLeafEntryV1<'static>>(),
            size_of::<SegmentIndexEntryCursorV1>(),
            size_of::<SegmentIndexPageVisitV1>(),
            size_of::<Digest256Hasher>(),
            size_of::<String>(),
            64, // lowercase SHA-256 segment name built by the held reader
            FIXED_METADATA_AND_CONTROL_BYTES,
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
        .ok_or_else(|| refusal("segment-index walk fixed workspace overflowed"))?;
        fixed
            .checked_add(frames)
            .ok_or_else(|| refusal("segment-index walk workspace overflowed"))
    }

    fn read_checked_page<'a>(
        &self,
        frame: &WalkFrame,
        buffer: &'a mut [u8; PAGE_BUFFER_BYTES],
        keyspace: SegmentIndexKeySpaceV1,
        limits: crate::segment_locator::SegmentIndexPageLimitsV1,
    ) -> Result<DecodedSegmentIndexPageV1<'a>> {
        let length = usize::try_from(frame.extent.length)
            .map_err(|_| refusal("segment-index page length does not fit memory"))?;
        if length < SEGMENT_INDEX_PAGE_HEADER_BYTES_V1
            || length > PAGE_BUFFER_BYTES
            || length > limits.max_page_bytes
        {
            return Err(refusal("segment-index page exceeds its finite byte limit"));
        }
        self.check_request()?;
        let (read_bytes, cursor_position) = {
            let mut sink = Cursor::new(&mut buffer[..length]);
            let read_bytes = self.read_page_extent(frame.extent, frame.page_sha256, &mut sink)?;
            (read_bytes, sink.position())
        };
        self.check_request()?;
        if read_bytes != frame.extent.length || cursor_position != frame.extent.length {
            return Err(corrupt("segment-index page extent length differs"));
        }
        self.check_request()?;
        let page = crate::segment_locator::decode_segment_index_page(
            &buffer[..length],
            frame.page_sha256,
            keyspace,
            limits,
        )?;
        self.check_request()?;
        if page.summary() != frame.expected_summary {
            return Err(corrupt("segment-index child summary differs"));
        }
        validate_page_bounds(self, page, frame)?;
        Ok(page)
    }
}

#[derive(Clone, Copy)]
struct WalkControl {
    root: SegmentIndexRootV1,
    pages: crate::segment_locator::SegmentIndexPageLimitsV1,
    depth_limit: usize,
    actual: SegmentIndexSummaryV1,
}

struct WalkFrame {
    extent: SegmentExtentV1,
    page_sha256: Digest256,
    expected_summary: SegmentIndexSummaryV1,
    next_child: u32,
    child_count: u32,
    entry_cursor: Option<SegmentIndexEntryCursorV1>,
    page_accounted: bool,
    lower: [u8; SEGMENT_INDEX_KEY_MAX_BYTES_V1],
    lower_len: u16,
    upper: [u8; SEGMENT_INDEX_KEY_MAX_BYTES_V1],
    upper_len: u16,
}

impl WalkFrame {
    fn root(
        extent: SegmentExtentV1,
        page_sha256: Digest256,
        expected_summary: SegmentIndexSummaryV1,
    ) -> Self {
        Self {
            extent,
            page_sha256,
            expected_summary,
            next_child: 0,
            child_count: 0,
            entry_cursor: None,
            page_accounted: false,
            lower: [0; SEGMENT_INDEX_KEY_MAX_BYTES_V1],
            lower_len: 0,
            upper: [0; SEGMENT_INDEX_KEY_MAX_BYTES_V1],
            upper_len: 0,
        }
    }

    fn child(entry: SegmentIndexInternalEntryV1<'_>, upper: Option<&[u8]>) -> Result<Self> {
        let lower_len = u16::try_from(entry.key.len())
            .map_err(|_| refusal("segment-index child lower bound exceeds its fixed buffer"))?;
        let upper_len = upper
            .map(|key| {
                u16::try_from(key.len()).map_err(|_| {
                    refusal("segment-index child upper bound exceeds its fixed buffer")
                })
            })
            .transpose()?
            .unwrap_or(0);
        if entry.key.is_empty()
            || entry.key.len() > SEGMENT_INDEX_KEY_MAX_BYTES_V1
            || upper.is_some_and(|key| {
                key.is_empty() || key.len() > SEGMENT_INDEX_KEY_MAX_BYTES_V1 || entry.key >= key
            })
        {
            return Err(corrupt("segment-index child bounds are invalid"));
        }
        let mut frame = Self {
            extent: entry.child_extent,
            page_sha256: entry.child_page_sha256,
            expected_summary: entry.summary,
            next_child: 0,
            child_count: 0,
            entry_cursor: None,
            page_accounted: false,
            lower: [0u8; SEGMENT_INDEX_KEY_MAX_BYTES_V1],
            lower_len,
            upper: [0u8; SEGMENT_INDEX_KEY_MAX_BYTES_V1],
            upper_len,
        };
        frame.lower[..entry.key.len()].copy_from_slice(entry.key);
        if let Some(key) = upper {
            frame.upper[..key.len()].copy_from_slice(key);
        }
        Ok(frame)
    }

    fn lower_bound(&self) -> Option<&[u8]> {
        (self.lower_len != 0).then_some(&self.lower[..usize::from(self.lower_len)])
    }

    fn upper_bound(&self) -> Option<&[u8]> {
        (self.upper_len != 0).then_some(&self.upper[..usize::from(self.upper_len)])
    }
}

fn validate_page_bounds(
    reader: &SegmentIndexReaderV1,
    page: DecodedSegmentIndexPageV1<'_>,
    frame: &WalkFrame,
) -> Result<()> {
    if let Some(lower) = frame.lower_bound() {
        reader.check_request()?;
        let first = page.first_key()?;
        reader.check_request()?;
        if first != Some(lower) {
            return Err(corrupt(
                "segment-index child first key differs from its parent lower separator",
            ));
        }
    }
    if let Some(upper) = frame.upper_bound() {
        reader.check_request()?;
        let last = page.last_key()?;
        reader.check_request()?;
        if last.is_some_and(|key| key >= upper) {
            return Err(corrupt(
                "segment-index child last key reaches its parent upper separator",
            ));
        }
    }
    Ok(())
}

fn add_page_bytes(
    actual: &mut SegmentIndexSummaryV1,
    bytes: u64,
    expected: SegmentIndexSummaryV1,
    limits: crate::segment_locator::SegmentIndexPageLimitsV1,
) -> Result<()> {
    actual.encoded_page_bytes = actual
        .encoded_page_bytes
        .checked_add(bytes)
        .ok_or_else(|| corrupt("segment-index actual page byte total overflows"))?;
    if actual.encoded_page_bytes > expected.encoded_page_bytes
        || actual.encoded_page_bytes > limits.max_encoded_page_bytes
    {
        return Err(corrupt(
            "segment-index actual page bytes exceed its root claim",
        ));
    }
    Ok(())
}

fn add_leaf_row(
    actual: &mut SegmentIndexSummaryV1,
    value_bytes: u64,
    expected: SegmentIndexSummaryV1,
    limits: crate::segment_locator::SegmentIndexPageLimitsV1,
) -> Result<()> {
    actual.row_count = actual
        .row_count
        .checked_add(1)
        .ok_or_else(|| corrupt("segment-index actual row count overflows"))?;
    actual.value_bytes = actual
        .value_bytes
        .checked_add(value_bytes)
        .ok_or_else(|| corrupt("segment-index actual value byte total overflows"))?;
    if actual.row_count > expected.row_count
        || actual.value_bytes > expected.value_bytes
        || actual.row_count > limits.max_row_count
        || actual.value_bytes > limits.max_value_bytes
    {
        return Err(corrupt(
            "segment-index actual leaf total exceeds its root claim",
        ));
    }
    Ok(())
}

fn page_bytes_len(extent: SegmentExtentV1) -> Result<u64> {
    let _ = extent
        .offset
        .checked_add(extent.length)
        .ok_or_else(|| corrupt("segment-index page extent overflows its file offset"))?;
    Ok(extent.length)
}

fn refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn corrupt(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::CorruptSelectedObject, detail)
}
