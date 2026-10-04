//! Budgeted point reads of immutable packed source-index pages.
//!
//! A caller-supplied root is a mechanical descriptor, not native completion,
//! source admission, rights, or currentness. The source owner must bind it to
//! the selected new representation and its authentic native transaction.
use std::{
    fs::{File, Metadata},
    io::{Cursor, Write},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use crate::{
    PinnedSqliteIoBudget, PinnedSqliteSpaceBudget,
    error::{Result, StoreError, StoreErrorCode},
    segment_locator::{
        SegmentExtentV1, SegmentIndexKeySpaceV1, SegmentIndexPageKindV1, SegmentIndexPageLimitsV1,
        SegmentIndexRootV1, SegmentIndexSummaryV1, decode_segment_index_page,
    },
    segment_object::verify_segment_range,
};
use tos_foundation::Digest256;

const PAGE_BUFFER: usize = 65_536;
const KEY_BUFFER: usize = 8_193;
const RANGE_BUFFER: usize = 65_536;

#[derive(Clone, Copy, Debug)]
pub struct SegmentIndexReadLimitsV1 {
    pub pages: SegmentIndexPageLimitsV1,
    pub max_depth: u32,
    pub max_value_bytes: u64,
    pub max_workspace_bytes: usize,
}

/// An exact index value location, without any admission/selection witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentLocatedValueV1 {
    pub value_extent: SegmentExtentV1,
    pub value_sha256: Digest256,
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
    fn from(m: &Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            size: m.len(),
            mode: m.mode(),
            uid: m.uid(),
            gid: m.gid(),
            mtime: m.mtime(),
            mtime_ns: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_ns: m.ctime_nsec(),
        }
    }
}

pub struct SegmentIndexReaderV1 {
    directory: File,
    directory_identity: (u64, u64, u32, u32, u32),
    root: SegmentIndexRootV1,
    limits: SegmentIndexReadLimitsV1,
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

impl SegmentIndexReaderV1 {
    /// Transfer a securely opened segment-directory capability. This does not
    /// create storage, reserve a new quota, or authenticate a native baseline.
    pub fn from_held_directory(
        directory: File,
        root: SegmentIndexRootV1,
        limits: SegmentIndexReadLimitsV1,
        io: PinnedSqliteIoBudget,
        space: PinnedSqliteSpaceBudget,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        let workspace = Self::workspace_upper_bound()?;
        if limits.max_depth == 0
            || limits.max_depth > 128
            || limits.max_workspace_bytes < workspace
            || limits.max_workspace_bytes == usize::MAX
            || limits.max_value_bytes == 0
            || limits.max_value_bytes == u64::MAX
            || limits.pages.max_page_bytes < 16
            || limits.pages.max_page_bytes > PAGE_BUFFER
            || limits.pages.max_key_bytes == 0
            || limits.pages.max_key_bytes > KEY_BUFFER
            || limits.pages.max_entries == 0
            || limits.pages.max_entries == u32::MAX
            || limits.pages.max_segment_bytes == 0
            || limits.pages.max_segment_bytes == u64::MAX
            || limits.pages.max_row_count == 0
            || limits.pages.max_row_count == u64::MAX
            || limits.pages.max_value_bytes == 0
            || limits.pages.max_value_bytes == u64::MAX
            || limits.pages.max_encoded_page_bytes == 0
            || limits.pages.max_encoded_page_bytes == u64::MAX
            || root.root_extent.length < 16
            || root.root_extent.length > limits.pages.max_page_bytes as u64
            || root
                .root_extent
                .offset
                .checked_add(root.root_extent.length)
                .is_none_or(|end| end > limits.pages.max_segment_bytes)
            || root.row_count > limits.pages.max_row_count
            || root.value_bytes > limits.pages.max_value_bytes
            || root.encoded_page_bytes > limits.pages.max_encoded_page_bytes
            || root.encoded_page_bytes < root.root_extent.length
        {
            return Err(refusal("packed index finite read profile"));
        }
        let m = directory
            .metadata()
            .map_err(|e| StoreError::io("cannot stat segment directory", e))?;
        if !m.is_dir() {
            return Err(StoreError::new(
                StoreErrorCode::InvalidRoot,
                "segment root is not a directory",
            ));
        }
        let reader = Self {
            directory,
            directory_identity: (m.dev(), m.ino(), m.mode(), m.uid(), m.gid()),
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

    /// Stack/page/key/range buffers and fixed reader/entry state, not an RSS or
    /// allocator claim. Persistent segments are the caller's retained baseline.
    pub fn workspace_upper_bound() -> Result<usize> {
        std::mem::size_of::<Self>()
            .checked_add(PAGE_BUFFER)
            .and_then(|n| n.checked_add(KEY_BUFFER.checked_mul(2)?))
            .and_then(|n| n.checked_add(RANGE_BUFFER))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| refusal("packed index workspace overflow"))
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

    pub(crate) fn check_request(&self) -> Result<()> {
        self.active()
    }

    pub(crate) const fn root_descriptor(&self) -> SegmentIndexRootV1 {
        self.root
    }

    pub(crate) const fn read_limits(&self) -> SegmentIndexReadLimitsV1 {
        self.limits
    }

    pub(crate) fn read_page_extent(
        &self,
        extent: SegmentExtentV1,
        sha: Digest256,
        sink: &mut impl Write,
    ) -> Result<u64> {
        self.read_extent(extent, sha, self.limits.pages.max_page_bytes as u64, sink)
    }

    pub(crate) fn read_located_value(
        &self,
        value: SegmentLocatedValueV1,
        sink: &mut impl Write,
    ) -> Result<u64> {
        self.read_extent(
            value.value_extent,
            value.value_sha256,
            self.limits.max_value_bytes,
            sink,
        )
    }

    fn active(&self) -> Result<()> {
        crate::streamed_cut::check_time_budgeted(self.deadline, &self.cancelled, Some(&self.io))?;
        let m = self
            .directory
            .metadata()
            .map_err(|e| StoreError::io("cannot recheck segment directory", e))?;
        if !m.is_dir() || (m.dev(), m.ino(), m.mode(), m.uid(), m.gid()) != self.directory_identity
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
        sha: Digest256,
        cap: u64,
        sink: &mut impl Write,
    ) -> Result<u64> {
        self.active()?;
        let name = extent.segment_sha256.to_hex();
        let file = tos_fd_open::open_regular_at(&self.directory, Path::new(&name))
            .map_err(|_| StoreError::new(StoreErrorCode::UnsafePath, "cannot pin segment inode"))?;
        let before = Stamp::from(
            &file
                .metadata()
                .map_err(|e| StoreError::io("cannot stat selected segment", e))?,
        );
        let result = verify_segment_range(
            &file,
            extent.offset,
            extent.length,
            sha,
            self.limits.pages.max_segment_bytes,
            cap,
            &self.io,
            self.deadline,
            &self.cancelled,
            sink,
        );
        let fence = (|| -> Result<()> {
            let held = Stamp::from(
                &file
                    .metadata()
                    .map_err(|e| StoreError::io("cannot recheck held segment", e))?,
            );
            let named =
                tos_fd_open::open_regular_at(&self.directory, Path::new(&name)).map_err(|_| {
                    StoreError::new(StoreErrorCode::UnsafePath, "selected segment name changed")
                })?;
            let named = Stamp::from(
                &named
                    .metadata()
                    .map_err(|e| StoreError::io("cannot recheck named segment", e))?,
            );
            if before != held || held != named {
                return Err(corrupt("selected segment held/named identity changed"));
            }
            self.active()
        })();
        // Always attempt the held/named fence, but retain the primary read or
        // sink error. Successful bytes are returned only after every fence.
        match result {
            Err(error) => Err(error),
            Ok(bytes) => {
                fence?;
                Ok(bytes)
            }
        }
    }

    /// Authenticate only the search path. Full root closure and semantic row
    /// validation remain native initial-completion/delta-transaction duties.
    pub fn get(&self, key: &[u8]) -> Result<Option<SegmentLocatedValueV1>> {
        self.active()?;
        if key.is_empty() || key.len() > self.limits.pages.max_key_bytes {
            return Err(refusal("packed index lookup key exceeds profile"));
        }
        if self.root.keyspace == SegmentIndexKeySpaceV1::ObjectDigest && key.len() != 32 {
            return Err(corrupt("packed object lookup requires exact digest key"));
        }
        let mut raw = [0u8; PAGE_BUFFER];
        let mut lower = [0u8; KEY_BUFFER];
        let mut lower_len = 0usize;
        let mut upper = [0u8; KEY_BUFFER];
        let mut upper_len = 0usize;
        let mut extent = self.root.root_extent;
        let mut sha = self.root.root_page_sha256;
        let mut expected = SegmentIndexSummaryV1 {
            row_count: self.root.row_count,
            value_bytes: self.root.value_bytes,
            encoded_page_bytes: self.root.encoded_page_bytes,
        };
        for _ in 0..self.limits.max_depth {
            self.active()?;
            let len = usize::try_from(extent.length)
                .map_err(|_| refusal("packed index page length overflow"))?;
            if len < 16 || len > self.limits.pages.max_page_bytes || len > raw.len() {
                return Err(refusal("packed index page exceeds profile"));
            }
            let mut sink = Cursor::new(&mut raw[..len]);
            self.read_extent(extent, sha, len as u64, &mut sink)?;
            if sink.position() != len as u64 {
                return Err(corrupt("packed index page length differs"));
            }
            let page =
                decode_segment_index_page(&raw[..len], sha, self.root.keyspace, self.limits.pages)?;
            if page.summary() != expected {
                return Err(corrupt("packed index child summary differs"));
            }
            if lower_len != 0 && page.first_key()? != Some(&lower[..lower_len]) {
                return Err(corrupt("packed index child lower boundary differs"));
            }
            if upper_len != 0
                && page
                    .last_key()?
                    .is_some_and(|last| last >= &upper[..upper_len])
            {
                return Err(corrupt("packed index child upper boundary differs"));
            }
            match page.kind() {
                SegmentIndexPageKindV1::Leaf => {
                    let found = page.find_leaf(key)?.map(|entry| SegmentLocatedValueV1 {
                        value_extent: entry.value_extent,
                        value_sha256: entry.value_sha256,
                    });
                    self.active()?;
                    return Ok(found);
                }
                SegmentIndexPageKindV1::Internal => {
                    let Some(child) = page.child_for_key(key)? else {
                        self.active()?;
                        return Ok(None);
                    };
                    // Capture the next separator without retaining any page/key allocation.
                    let next_upper = page.next_key_after(child.key)?;
                    lower_len = child.key.len();
                    lower[..lower_len].copy_from_slice(child.key);
                    if let Some(next) = next_upper {
                        upper_len = next.len();
                        upper[..upper_len].copy_from_slice(next);
                    }
                    extent = child.child_extent;
                    sha = child.child_page_sha256;
                    expected = child.summary;
                }
            }
        }
        Err(refusal("packed index depth exceeds profile"))
    }

    /// The sink is private until complete value SHA verification succeeds;
    /// caller-owned writes/reservations/disclosure fences remain separate.
    pub fn read_value(&self, key: &[u8], sink: &mut impl Write) -> Result<Option<u64>> {
        let Some(value) = self.get(key)? else {
            return Ok(None);
        };
        self.read_located_value(value, sink).map(Some)
    }
}
