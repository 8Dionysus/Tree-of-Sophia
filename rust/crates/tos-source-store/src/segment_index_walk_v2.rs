//! Bounded ordered closure walk over typed V2 physical/logical page summaries.
//!
//! The walk validates the full selected tree and streams page/leaf callbacks;
//! those callbacks do not select, admit, or publish a native root.

use std::{io::Cursor, mem::size_of};

use tos_foundation::{Digest256, Digest256Hasher};

use crate::{
    error::{Result, StoreError, StoreErrorCode},
    segment_index_read_v2::{
        SegmentIndexClosureV2, SegmentIndexPageExpectationV2, SegmentIndexPageVisitV2,
        SegmentIndexReaderV2, V2_FIXED_WORKSPACE_BYTES, V2_KEY_BUFFER_BYTES, V2_PAGE_BUFFER_BYTES,
        V2_RANGE_BUFFER_BYTES, check_page_binding, located_value,
    },
    segment_locator::{SegmentExtentV1, SegmentIndexPageKindV1, SegmentIndexSummaryV1},
    segment_locator_v2::{
        DecodedSegmentIndexPageV2, SegmentIndexEntryCursorV2, SegmentIndexEntryV2,
        SegmentIndexInternalEntryV2, SegmentIndexLeafEntryV2, SegmentIndexLogicalSummaryV2,
        SegmentIndexPageLimitsV2, SegmentIndexRootV2,
    },
    segment_subtree_memo_v2::SegmentSubtreeMemoV2,
};

/// Mechanical closure of exactly `[lower, upper)` at the supplied root.
/// Returned only after every intersecting branch reaches EOF, including an
/// empty interval. It is not full-tree validation or native admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexIntervalClosureV2 {
    pub root: SegmentIndexRootV2,
    pub lower_sha256: Digest256,
    pub upper_sha256: Digest256,
    pub lower_bytes: u64,
    pub upper_bytes: u64,
    pub rows: u64,
    pub physical_value_bytes: u64,
    pub logical_key_bytes: u64,
    pub logical_value_bytes: u64,
    pub visited_page_bytes: u64,
    pub page_count: u64,
}

/// Full mechanical closure with explicit same-operation subtree reuse.
/// `page_count` inside closure includes replayed pages; newly read pages and
/// cache hits are reported separately, not credited as a fresh physical scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentIndexRetainedClosureV2 {
    pub closure: SegmentIndexClosureV2,
    pub newly_visited_pages: u64,
    pub reused_subtrees: u64,
    pub replayed_pages: u64,
}

impl SegmentIndexReaderV2 {
    /// Conservative logical workspace for a full V2 walk at configured depth.
    /// Callback captures, retained rows, spool/dedup state, and sinks remain
    /// additional caller-owned terms in the same operation envelope.
    pub fn walk_workspace_upper_bound(&self) -> Result<usize> {
        self.check_request()?;
        let depth = usize::try_from(self.read_limits().max_depth)
            .map_err(|_| refusal("V2 walk depth does not fit memory"))?;
        if depth == 0 || depth > 128 {
            return Err(refusal("V2 walk depth exceeds its finite limit"));
        }
        let workspace = self.workspace_for_frame_capacity(depth)?;
        self.check_request()?;
        Ok(workspace)
    }

    /// Visit each checked page frame once and each leaf in strict byte-key
    /// order. A parent with `k` children is still reread, decoded, and hashed
    /// `k` times; the page callback is emitted only on that frame's first
    /// checked read. Each page read performs range hashing, physical page
    /// hashing, and logical transcript hashing. Earlier callback effects must
    /// stay private until this method returns closure at true EOF.
    pub fn walk_pages_and_leaves(
        &self,
        max_workspace_bytes: usize,
        page_visitor: &mut impl FnMut(SegmentIndexPageVisitV2) -> Result<()>,
        leaf_visitor: &mut impl FnMut(SegmentIndexLeafEntryV2<'_>) -> Result<()>,
    ) -> Result<SegmentIndexClosureV2> {
        let control = self.walk_selected_pages_and_leaves(
            max_workspace_bytes,
            None,
            None,
            page_visitor,
            leaf_visitor,
        )?;
        Ok(SegmentIndexClosureV2 {
            root: control.root,
            physical_summary: control.actual_physical,
            logical_summary: control.root.logical_summary,
            page_count: control.page_count,
        })
    }

    /// Retained graph walk using a private completed-subtree memo. Initial
    /// misses undergo full page/leaf EOF and summary replay. A later hit skips
    /// only a subtree fully completed in this same original operation, with
    /// unchanged read profile/directory and frozen retention+inventory basis.
    /// `callback_domain` names the fixed source-versioned interpretation of
    /// inventory callbacks; different locator/native roles need distinct domains.
    /// Inventory callbacks must synchronously add all segment references to a
    /// monotone private inventory retained through final copy/fixity. Changing
    /// or dropping that inventory requires a new memo. Any error abandons this
    /// memo; partial callbacks are never reusable completion authority.
    pub fn walk_retained_pages_and_leaves(
        &self,
        memo: &mut SegmentSubtreeMemoV2,
        basis: Digest256,
        callback_domain: Digest256,
        max_workspace_bytes: usize,
        page_visitor: &mut impl FnMut(SegmentIndexPageVisitV2) -> Result<()>,
        leaf_visitor: &mut impl FnMut(SegmentIndexLeafEntryV2<'_>) -> Result<()>,
    ) -> Result<SegmentIndexRetainedClosureV2> {
        let result = (|| {
            memo.matches(self, basis)?;
            let memo_charge = memo.required_state_charge()?;
            let walk_charge = self.walk_workspace_upper_bound()?;
            if max_workspace_bytes == 0
                || max_workspace_bytes == usize::MAX
                || max_workspace_bytes > self.read_limits().max_workspace_bytes
                || memo_charge
                    .checked_add(walk_charge)
                    .is_none_or(|n| n > max_workspace_bytes)
            {
                return Err(refusal(
                    "V2 retained walk exceeds its simultaneous precharge",
                ));
            }
            let control = self.walk_selected_pages_and_leaves(
                max_workspace_bytes - memo_charge,
                None,
                Some((&mut *memo, basis, callback_domain)),
                page_visitor,
                leaf_visitor,
            )?;
            self.check_request()?;
            Ok(SegmentIndexRetainedClosureV2 {
                closure: SegmentIndexClosureV2 {
                    root: control.root,
                    physical_summary: control.actual_physical,
                    logical_summary: control.root.logical_summary,
                    page_count: control.page_count,
                },
                newly_visited_pages: control
                    .page_count
                    .checked_sub(control.replayed_pages)
                    .ok_or_else(|| corrupt("V2 replayed page count exceeds closure"))?,
                reused_subtrees: control.reused_subtrees,
                replayed_pages: control.replayed_pages,
            })
        })();
        if result.is_err() {
            memo.abandon();
        }
        result
    }

    /// Whole simultaneous interval workspace, excluding caller-owned bounds,
    /// captures, retained output and native state. Values use one fixed buffer.
    pub fn interval_workspace_upper_bound(&self, max_value_buffer_bytes: usize) -> Result<usize> {
        self.check_request()?;
        if max_value_buffer_bytes == 0
            || max_value_buffer_bytes == usize::MAX
            || max_value_buffer_bytes as u64 > self.read_limits().max_value_bytes
        {
            return Err(refusal("V2 interval value buffer is not finite"));
        }
        let required = self
            .walk_workspace_upper_bound()?
            .checked_add(interval_value_workspace(max_value_buffer_bytes)?)
            .ok_or_else(|| refusal("V2 interval workspace overflowed"))?;
        self.check_request()?;
        Ok(required)
    }

    /// Stream borrowed physical values in strict key order for `[lower, upper)`.
    /// Only intersecting child pages are read. Checked parent summaries and
    /// inclusive child endpoints justify skipping siblings; this presupposes
    /// the owner's initial full-tree admission. Values are range/SHA checked
    /// under the original reader request before callbacks. The codec also
    /// checks the native raw/logical binding (ObjectDigest uses its 40-byte
    /// logical tuple); typed semantics remain with the caller. Keep effects
    /// private until interval closure; callback borrows cannot escape.
    pub fn walk_interval_values(
        &self,
        lower: &[u8],
        upper: &[u8],
        max_workspace_bytes: usize,
        max_value_buffer_bytes: usize,
        visitor: &mut impl FnMut(SegmentIndexLeafEntryV2<'_>, &[u8]) -> Result<()>,
    ) -> Result<SegmentIndexIntervalClosureV2> {
        self.check_request()?;
        if lower.is_empty()
            || upper.is_empty()
            || lower.len() >= V2_KEY_BUFFER_BYTES
            || upper.len() >= V2_KEY_BUFFER_BYTES
            || lower > upper
            || max_value_buffer_bytes == 0
            || max_value_buffer_bytes == usize::MAX
            || max_value_buffer_bytes as u64 > self.read_limits().max_value_bytes
            || max_workspace_bytes == 0
            || max_workspace_bytes == usize::MAX
            || max_workspace_bytes > self.read_limits().max_workspace_bytes
        {
            return Err(refusal("V2 interval bounds or workspace are not finite"));
        }
        let value_overhead = interval_value_workspace(max_value_buffer_bytes)?;
        let required = self.interval_workspace_upper_bound(max_value_buffer_bytes)?;
        if required > max_workspace_bytes {
            return Err(refusal("V2 interval exceeds its precharged workspace"));
        }
        self.check_request()?;
        let mut value = Vec::new();
        value
            .try_reserve_exact(max_value_buffer_bytes)
            .map_err(|_| refusal("V2 interval value reservation failed"))?;
        self.check_request()?;
        if value.capacity() != max_value_buffer_bytes {
            return Err(refusal("V2 interval value capacity differs from precharge"));
        }
        value.resize(max_value_buffer_bytes, 0);
        self.check_request()?;
        let walk_budget = max_workspace_bytes - value_overhead;
        let control = self.walk_selected_pages_and_leaves(
            walk_budget,
            Some((lower, upper)),
            None,
            &mut |_| Ok(()),
            &mut |leaf| {
                self.check_request()?;
                let length = usize::try_from(leaf.value_extent.length)
                    .map_err(|_| refusal("V2 interval value length does not fit memory"))?;
                if length > value.len() {
                    return Err(refusal("V2 interval value exceeds its fixed buffer"));
                }
                let mut sink = Cursor::new(&mut value[..length]);
                let read = self.read_located_value(located_value(leaf), &mut sink)?;
                if read != leaf.value_extent.length || sink.position() != read {
                    return Err(corrupt("V2 interval value ended before its extent"));
                }
                self.check_request()?;
                let result = visitor(leaf, &value[..length]);
                let fence = self.check_request();
                result?;
                fence
            },
        )?;
        self.check_request()?;
        let receipt = SegmentIndexIntervalClosureV2 {
            root: control.root,
            lower_sha256: Digest256::of_bytes(lower),
            upper_sha256: Digest256::of_bytes(upper),
            lower_bytes: lower.len() as u64,
            upper_bytes: upper.len() as u64,
            rows: control.actual_physical.row_count,
            physical_value_bytes: control.actual_physical.value_bytes,
            logical_key_bytes: control.actual_logical_key_bytes,
            logical_value_bytes: control.actual_logical_value_bytes,
            visited_page_bytes: control.actual_physical.encoded_page_bytes,
            page_count: control.page_count,
        };
        self.check_request()?;
        Ok(receipt)
    }

    fn walk_selected_pages_and_leaves(
        &self,
        max_workspace_bytes: usize,
        interval: Option<(&[u8], &[u8])>,
        mut memo: Option<(&mut SegmentSubtreeMemoV2, Digest256, Digest256)>,
        page_visitor: &mut impl FnMut(SegmentIndexPageVisitV2) -> Result<()>,
        leaf_visitor: &mut impl FnMut(SegmentIndexLeafEntryV2<'_>) -> Result<()>,
    ) -> Result<WalkControlV2> {
        self.check_request()?;
        let limits = self.read_limits();
        let depth_limit = usize::try_from(limits.max_depth)
            .map_err(|_| refusal("V2 walk depth does not fit memory"))?;
        if depth_limit == 0
            || depth_limit > 128
            || limits.max_workspace_bytes == 0
            || limits.max_workspace_bytes == usize::MAX
            || max_workspace_bytes == 0
            || max_workspace_bytes == usize::MAX
            || limits.pages.physical.max_page_bytes
                < crate::segment_locator_v2::SEGMENT_INDEX_PAGE_HEADER_BYTES_V2
            || limits.pages.physical.max_page_bytes > V2_PAGE_BUFFER_BYTES
        {
            return Err(refusal("V2 walk profile is not finite"));
        }

        let required_workspace = self.workspace_for_frame_capacity(depth_limit)?;
        if required_workspace > max_workspace_bytes
            || required_workspace > limits.max_workspace_bytes
        {
            return Err(refusal("V2 walk exceeds its precharged workspace"));
        }
        self.check_request()?;
        let mut stack = Vec::<WalkFrameV2>::new();
        stack
            .try_reserve_exact(depth_limit)
            .map_err(|_| refusal("V2 walk frame reservation failed"))?;
        self.check_request()?;
        let actual_workspace = self.workspace_for_frame_capacity(stack.capacity())?;
        if stack.capacity() < depth_limit
            || actual_workspace > max_workspace_bytes
            || actual_workspace > limits.max_workspace_bytes
        {
            return Err(refusal("V2 walk allocator capacity exceeds its precharge"));
        }

        let root = self.root_descriptor();
        let mut control = WalkControlV2 {
            root,
            pages: limits.pages,
            depth_limit,
            actual_physical: SegmentIndexSummaryV1 {
                row_count: 0,
                value_bytes: 0,
                encoded_page_bytes: 0,
            },
            actual_logical_rows: 0,
            actual_logical_key_bytes: 0,
            actual_logical_value_bytes: 0,
            page_count: 0,
            reused_subtrees: 0,
            replayed_pages: 0,
        };
        self.check_request()?;
        let root_frame = WalkFrameV2::root(root);
        self.check_request()?;
        stack.push(root_frame);
        self.check_request()?;
        let mut page_bytes = [0u8; V2_PAGE_BUFFER_BYTES];

        while !stack.is_empty() {
            self.check_request()?;
            let top = stack.len() - 1;
            let finished_internal = {
                let frame = &stack[top];
                frame.child_count != 0 && frame.next_child == frame.child_count
            };
            if finished_internal {
                self.finish_walk_frame(&mut stack, &control, interval.is_none(), &mut memo)?;
                continue;
            }

            if stack[top].start.is_none() {
                stack[top].start = Some(control.baseline());
                if let Some((cache, basis, callback_domain)) = memo.as_mut() {
                    if let Some(done) = cache.lookup(
                        self,
                        *basis,
                        &stack[top].expectation,
                        top == 0,
                        *callback_domain,
                    )? {
                        let height = usize::try_from(done.height)
                            .map_err(|_| refusal("V2 cached subtree height exceeds memory"))?;
                        if top
                            .checked_add(height)
                            .is_none_or(|n| n > control.depth_limit)
                        {
                            return Err(refusal("V2 cached subtree exceeds current depth"));
                        }
                        replay_subtree(&mut control, &stack[top].expectation, done.pages)?;
                        stack[top].height = done.height;
                        // Already minted by real EOF and complete inventory
                        // callbacks. Never mint a replacement from a hit.
                        let frame = stack
                            .pop()
                            .ok_or_else(|| corrupt("V2 replay frame missing"))?;
                        update_parent_height(&mut stack, frame.height)?;
                        self.check_request()?;
                        continue;
                    }
                }
            }
            let frame = &stack[top];
            let page = self.read_checked_page(
                frame.expectation.extent,
                frame.expectation.page_sha256,
                &mut page_bytes,
            )?;
            check_page_binding(
                page,
                frame.expectation.physical_summary,
                frame.expectation.logical_summary,
                frame.expectation.lower_bound(),
                frame.expectation.upper_bound(),
                top == 0,
            )?;
            self.check_request()?;

            let page_count = page.entry_count();
            let is_internal = page.kind() == SegmentIndexPageKindV1::Internal;
            if stack[top].child_count != 0 && (!is_internal || stack[top].child_count != page_count)
            {
                return Err(corrupt("V2 page role or child count changed during walk"));
            }

            if !stack[top].page_accounted {
                let visit = SegmentIndexPageVisitV2 {
                    extent: stack[top].expectation.extent,
                    page_sha256: stack[top].expectation.page_sha256,
                    keyspace: control.root.keyspace,
                    kind: page.kind(),
                    physical_summary: page.physical_summary(),
                    logical_summary: page.logical_summary(),
                };
                self.check_request()?;
                let callback_result = page_visitor(visit);
                let callback_fence = self.check_request();
                if let Err(error) = callback_result {
                    return Err(error);
                }
                callback_fence?;
                add_page_bytes_v2(&mut control, stack[top].expectation.extent)?;
                stack[top].page_accounted = true;
                control.page_count = control
                    .page_count
                    .checked_add(1)
                    .ok_or_else(|| refusal("V2 walked page count overflowed"))?;
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
                            return Err(corrupt("V2 leaf cursor exceeds its page"));
                        }
                        let SegmentIndexEntryV2::Leaf(leaf) = entry else {
                            return Err(corrupt("V2 leaf page contains an internal entry"));
                        };
                        if interval
                            .is_some_and(|(lower, upper)| leaf.key < lower || leaf.key >= upper)
                        {
                            ordinal = ordinal
                                .checked_add(1)
                                .ok_or_else(|| refusal("V2 leaf ordinal overflowed"))?;
                            continue;
                        }
                        add_leaf_v2(&mut control, leaf)?;
                        self.check_request()?;
                        let callback_result = leaf_visitor(leaf);
                        let callback_fence = self.check_request();
                        if let Err(error) = callback_result {
                            return Err(error);
                        }
                        callback_fence?;
                        ordinal = ordinal
                            .checked_add(1)
                            .ok_or_else(|| refusal("V2 leaf ordinal overflowed"))?;
                    }
                    if ordinal != page_count {
                        return Err(corrupt("V2 leaf cursor ended before its page"));
                    }
                    self.finish_walk_frame(&mut stack, &control, interval.is_none(), &mut memo)?;
                }
                SegmentIndexPageKindV1::Internal => {
                    if page_count == 0 {
                        return Err(corrupt("V2 internal page contains no children"));
                    }
                    let next_child = stack[top].next_child;
                    if next_child >= page_count {
                        return Err(corrupt("V2 child cursor exceeds its page"));
                    }
                    self.check_request()?;
                    let mut cursor = match stack[top].entry_cursor {
                        Some(cursor) => cursor,
                        None if next_child == 0 => page.initial_cursor(),
                        None => return Err(corrupt("V2 internal cursor is missing")),
                    };
                    let entry = page.next_entry(&mut cursor)?;
                    self.check_request()?;
                    let Some(SegmentIndexEntryV2::Internal(child)) = entry else {
                        return Err(corrupt("V2 internal cursor did not select a child"));
                    };
                    let next_index = next_child
                        .checked_add(1)
                        .ok_or_else(|| refusal("V2 child cursor overflowed"))?;
                    if next_index == page_count {
                        self.check_request()?;
                        let mut eof_cursor = cursor;
                        let eof = page.next_entry(&mut eof_cursor)?;
                        self.check_request()?;
                        if eof.is_some() {
                            return Err(corrupt("V2 internal cursor exceeds its page"));
                        }
                    }
                    // Stored child endpoints are inclusive. Every child entry
                    // is cursor checked, but disjoint child pages stay unread.
                    let intersects = interval.is_none_or(|(lower, upper)| {
                        lower < upper && child.upper_key >= lower && child.lower_key < upper
                    });
                    let child_frame = if intersects {
                        Some(WalkFrameV2::child(child)?)
                    } else {
                        None
                    };
                    self.check_request()?;
                    {
                        let parent = &mut stack[top];
                        if parent.child_count == 0 {
                            parent.child_count = page_count;
                        }
                        parent.entry_cursor = Some(cursor);
                        parent.next_child = next_index;
                    }
                    self.check_request()?;
                    let Some(child_frame) = child_frame else {
                        continue;
                    };
                    if stack.len() >= control.depth_limit {
                        return Err(refusal("V2 walk exceeds its bounded depth"));
                    }
                    self.check_request()?;
                    stack.push(child_frame);
                    self.check_request()?;
                }
            }
        }

        self.check_request()?;
        if interval.is_none()
            && (control.actual_physical != root.physical_summary
                || control.actual_logical_rows != root.logical_summary.row_count
                || control.actual_logical_key_bytes != root.logical_summary.key_bytes
                || control.actual_logical_value_bytes != root.logical_summary.value_bytes)
        {
            return Err(corrupt(
                "V2 ordered walk does not close at its root summaries",
            ));
        }
        self.check_request()?;
        Ok(control)
    }

    fn finish_walk_frame(
        &self,
        stack: &mut Vec<WalkFrameV2>,
        control: &WalkControlV2,
        complete: bool,
        memo: &mut Option<(&mut SegmentSubtreeMemoV2, Digest256, Digest256)>,
    ) -> Result<()> {
        self.check_request()?;
        let top = stack
            .len()
            .checked_sub(1)
            .ok_or_else(|| corrupt("V2 completed frame missing"))?;
        let frame = &stack[top];
        if complete {
            let before = frame
                .start
                .ok_or_else(|| corrupt("V2 subtree initial counters missing"))?;
            let actual = control.delta(before)?;
            if actual.physical != frame.expectation.physical_summary
                || actual.logical_rows != frame.expectation.logical_summary.row_count
                || actual.logical_keys != frame.expectation.logical_summary.key_bytes
                || actual.logical_values != frame.expectation.logical_summary.value_bytes
            {
                return Err(corrupt("V2 subtree EOF does not close at parent summaries"));
            }
            if let Some((cache, basis, callback_domain)) = memo.as_mut() {
                cache.record_completed(
                    self,
                    *basis,
                    &frame.expectation,
                    top == 0,
                    *callback_domain,
                    actual.pages,
                    frame.height,
                )?;
            }
        }
        let frame = stack
            .pop()
            .ok_or_else(|| corrupt("V2 completed frame disappeared"))?;
        update_parent_height(stack, frame.height)?;
        self.check_request()
    }

    fn workspace_for_frame_capacity(&self, capacity: usize) -> Result<usize> {
        let frame_slots = capacity
            .checked_add(2)
            .ok_or_else(|| refusal("V2 walk frame count overflowed"))?;
        let frames = size_of::<WalkFrameV2>()
            .checked_mul(frame_slots)
            .ok_or_else(|| refusal("V2 walk frame workspace overflowed"))?;
        let fixed = [
            size_of::<Self>(),
            size_of::<Vec<WalkFrameV2>>(),
            size_of::<WalkControlV2>(),
            V2_PAGE_BUFFER_BYTES,
            V2_RANGE_BUFFER_BYTES,
            V2_KEY_BUFFER_BYTES,
            size_of::<DecodedSegmentIndexPageV2<'static>>(),
            size_of::<SegmentIndexEntryCursorV2>(),
            size_of::<SegmentIndexEntryV2<'static>>(),
            size_of::<SegmentIndexLeafEntryV2<'static>>(),
            size_of::<SegmentIndexInternalEntryV2<'static>>(),
            size_of::<SegmentIndexPageVisitV2>(),
            size_of::<Digest256Hasher>(),
            size_of::<String>(),
            64, // lowercase SHA-256 segment filename
            V2_FIXED_WORKSPACE_BYTES,
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
        .ok_or_else(|| refusal("V2 walk fixed workspace overflowed"))?;
        fixed
            .checked_add(frames)
            .ok_or_else(|| refusal("V2 walk workspace overflowed"))
    }
}

struct WalkFrameV2 {
    expectation: SegmentIndexPageExpectationV2,
    next_child: u32,
    child_count: u32,
    entry_cursor: Option<SegmentIndexEntryCursorV2>,
    page_accounted: bool,
    start: Option<WalkBaselineV2>,
    height: u32,
}

impl WalkFrameV2 {
    fn root(root: SegmentIndexRootV2) -> Self {
        Self {
            expectation: SegmentIndexPageExpectationV2::root(root),
            next_child: 0,
            child_count: 0,
            entry_cursor: None,
            page_accounted: false,
            start: None,
            height: 1,
        }
    }

    fn child(child: SegmentIndexInternalEntryV2<'_>) -> Result<Self> {
        Ok(Self {
            expectation: SegmentIndexPageExpectationV2::child(child)?,
            next_child: 0,
            child_count: 0,
            entry_cursor: None,
            page_accounted: false,
            start: None,
            height: 1,
        })
    }
}

struct WalkControlV2 {
    root: SegmentIndexRootV2,
    pages: SegmentIndexPageLimitsV2,
    depth_limit: usize,
    actual_physical: SegmentIndexSummaryV1,
    actual_logical_rows: u64,
    actual_logical_key_bytes: u64,
    actual_logical_value_bytes: u64,
    page_count: u64,
    reused_subtrees: u64,
    replayed_pages: u64,
}

#[derive(Clone, Copy)]
struct WalkBaselineV2 {
    physical: SegmentIndexSummaryV1,
    logical_rows: u64,
    logical_keys: u64,
    logical_values: u64,
    pages: u64,
}
impl WalkControlV2 {
    fn baseline(&self) -> WalkBaselineV2 {
        WalkBaselineV2 {
            physical: self.actual_physical,
            logical_rows: self.actual_logical_rows,
            logical_keys: self.actual_logical_key_bytes,
            logical_values: self.actual_logical_value_bytes,
            pages: self.page_count,
        }
    }
    fn delta(&self, before: WalkBaselineV2) -> Result<WalkBaselineV2> {
        let now = self.baseline();
        let sub = |a: u64, b: u64| {
            a.checked_sub(b)
                .ok_or_else(|| corrupt("V2 subtree counters decreased"))
        };
        Ok(WalkBaselineV2 {
            physical: SegmentIndexSummaryV1 {
                row_count: sub(now.physical.row_count, before.physical.row_count)?,
                value_bytes: sub(now.physical.value_bytes, before.physical.value_bytes)?,
                encoded_page_bytes: sub(
                    now.physical.encoded_page_bytes,
                    before.physical.encoded_page_bytes,
                )?,
            },
            logical_rows: sub(now.logical_rows, before.logical_rows)?,
            logical_keys: sub(now.logical_keys, before.logical_keys)?,
            logical_values: sub(now.logical_values, before.logical_values)?,
            pages: sub(now.pages, before.pages)?,
        })
    }
}
fn update_parent_height(stack: &mut [WalkFrameV2], child: u32) -> Result<()> {
    if let Some(parent) = stack.last_mut() {
        parent.height = parent.height.max(
            child
                .checked_add(1)
                .ok_or_else(|| refusal("V2 completed subtree height overflowed"))?,
        );
    }
    Ok(())
}
fn replay_subtree(
    control: &mut WalkControlV2,
    e: &SegmentIndexPageExpectationV2,
    pages: u64,
) -> Result<()> {
    let add = |a: u64, b: u64| {
        a.checked_add(b)
            .ok_or_else(|| corrupt("V2 subtree replay total overflowed"))
    };
    control.actual_physical.row_count = add(
        control.actual_physical.row_count,
        e.physical_summary.row_count,
    )?;
    control.actual_physical.value_bytes = add(
        control.actual_physical.value_bytes,
        e.physical_summary.value_bytes,
    )?;
    control.actual_physical.encoded_page_bytes = add(
        control.actual_physical.encoded_page_bytes,
        e.physical_summary.encoded_page_bytes,
    )?;
    control.actual_logical_rows = add(control.actual_logical_rows, e.logical_summary.row_count)?;
    control.actual_logical_key_bytes = add(
        control.actual_logical_key_bytes,
        e.logical_summary.key_bytes,
    )?;
    control.actual_logical_value_bytes = add(
        control.actual_logical_value_bytes,
        e.logical_summary.value_bytes,
    )?;
    control.page_count = add(control.page_count, pages)?;
    control.replayed_pages = add(control.replayed_pages, pages)?;
    control.reused_subtrees = add(control.reused_subtrees, 1)?;
    let p = control.root.physical_summary;
    let l = control.root.logical_summary;
    let caps = control.pages;
    if control.actual_physical.row_count > p.row_count
        || control.actual_physical.value_bytes > p.value_bytes
        || control.actual_physical.encoded_page_bytes > p.encoded_page_bytes
        || control.actual_logical_rows > l.row_count
        || control.actual_logical_key_bytes > l.key_bytes
        || control.actual_logical_value_bytes > l.value_bytes
        || control.actual_physical.row_count > caps.physical.max_row_count
        || control.actual_physical.value_bytes > caps.physical.max_value_bytes
        || control.actual_physical.encoded_page_bytes > caps.physical.max_encoded_page_bytes
        || control.actual_logical_key_bytes > caps.max_logical_key_bytes
        || control.actual_logical_value_bytes > caps.max_logical_value_bytes
    {
        return Err(corrupt(
            "V2 replay exceeds selected root or profile summaries",
        ));
    }
    Ok(())
}

fn add_page_bytes_v2(control: &mut WalkControlV2, extent: SegmentExtentV1) -> Result<()> {
    let _ = extent
        .offset
        .checked_add(extent.length)
        .ok_or_else(|| corrupt("V2 page extent overflows its file offset"))?;
    control.actual_physical.encoded_page_bytes = control
        .actual_physical
        .encoded_page_bytes
        .checked_add(extent.length)
        .ok_or_else(|| corrupt("V2 actual encoded-page total overflows"))?;
    if control.actual_physical.encoded_page_bytes > control.root.physical_summary.encoded_page_bytes
        || control.actual_physical.encoded_page_bytes
            > control.pages.physical.max_encoded_page_bytes
    {
        return Err(corrupt("V2 actual encoded-page bytes exceed root claim"));
    }
    Ok(())
}

fn add_leaf_v2(control: &mut WalkControlV2, leaf: SegmentIndexLeafEntryV2<'_>) -> Result<()> {
    let key_bytes = u64::try_from(leaf.key.len())
        .map_err(|_| refusal("V2 leaf key length does not fit u64"))?;
    control.actual_physical.row_count = control
        .actual_physical
        .row_count
        .checked_add(1)
        .ok_or_else(|| corrupt("V2 actual physical row count overflows"))?;
    control.actual_physical.value_bytes = control
        .actual_physical
        .value_bytes
        .checked_add(leaf.value_extent.length)
        .ok_or_else(|| corrupt("V2 actual physical byte total overflows"))?;
    control.actual_logical_rows = control
        .actual_logical_rows
        .checked_add(1)
        .ok_or_else(|| corrupt("V2 actual logical row count overflows"))?;
    control.actual_logical_key_bytes = control
        .actual_logical_key_bytes
        .checked_add(key_bytes)
        .ok_or_else(|| corrupt("V2 actual logical key total overflows"))?;
    control.actual_logical_value_bytes = control
        .actual_logical_value_bytes
        .checked_add(leaf.logical_value_bytes)
        .ok_or_else(|| corrupt("V2 actual logical value total overflows"))?;

    let expected_physical = control.root.physical_summary;
    let expected_logical = control.root.logical_summary;
    if control.actual_physical.row_count > expected_physical.row_count
        || control.actual_physical.value_bytes > expected_physical.value_bytes
        || control.actual_physical.row_count > control.pages.physical.max_row_count
        || control.actual_physical.value_bytes > control.pages.physical.max_value_bytes
        || control.actual_logical_rows > expected_logical.row_count
        || control.actual_logical_key_bytes > expected_logical.key_bytes
        || control.actual_logical_value_bytes > expected_logical.value_bytes
        || control.actual_logical_key_bytes > control.pages.max_logical_key_bytes
        || control.actual_logical_value_bytes > control.pages.max_logical_value_bytes
    {
        return Err(corrupt(
            "V2 actual leaf totals exceed root or caller limits",
        ));
    }
    Ok(())
}

fn interval_value_workspace(max_value_buffer_bytes: usize) -> Result<usize> {
    max_value_buffer_bytes
        .checked_add(size_of::<Vec<u8>>())
        .and_then(|bytes| bytes.checked_add(size_of::<Cursor<&mut [u8]>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<SegmentIndexIntervalClosureV2>()))
        .ok_or_else(|| refusal("V2 interval value workspace overflowed"))
}

fn refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn corrupt(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::CorruptSelectedObject, detail)
}
