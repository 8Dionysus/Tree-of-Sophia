//! Private, same-operation completed-subtree memo for retained backup walks.
//! No SQL/file/insert capability or persistent reusable proof is exported.
use crate::{
    PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteConnection, PinnedSqliteIoBudget,
    PinnedSqliteSpaceBudget, Result, StoreError, StoreErrorCode,
    segment_index_read_v2::{
        SegmentIndexPageExpectationV2, SegmentIndexReadLimitsV2, SegmentIndexReaderV2,
        V2_KEY_BUFFER_BYTES,
    },
    segment_locator::SegmentIndexKeySpaceV1,
};
use rusqlite::{OptionalExtension, params};
use std::{
    fs::File,
    io::{Cursor, Write},
    mem::size_of,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::Digest256;
const KEY_BUFFER: usize = 2 * V2_KEY_BUFFER_BYTES + 256;

/// Finite caller-selected cache/state terms. SQLite cache_size is advisory;
/// SQLite native heap/live page pins remain additional whole-process terms.
#[derive(Clone, Copy, Debug)]
pub struct SegmentSubtreeMemoLimitsV2 {
    pub max_entries: u64,
    pub cache_bytes: usize,
    pub max_state_bytes: usize,
    pub retained_caller_bytes: usize,
}

/// Owns a newly created unnamed auxiliary database. Only the successful full
/// walker can mint rows; a caller cannot turn a digest/boolean into subtree EOF.
/// The backup's monotone segment inventory must remain held throughout reuse.
pub struct SegmentSubtreeMemoV2 {
    db: PinnedSqliteConnection,
    _scope: PinnedSqliteAuxScope,
    io: PinnedSqliteIoBudget,
    space: PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    directory: (u64, u64, u32, u32, u32),
    basis: Digest256,
    reader_limits: SegmentIndexReadLimitsV2,
    limits: SegmentSubtreeMemoLimitsV2,
    entries: u64,
    abandoned: bool,
}
pub(crate) struct CompletedSubtreeV2 {
    pub(crate) pages: u64,
    pub(crate) height: u32,
}
impl SegmentSubtreeMemoV2 {
    /// Declared simultaneous memo state charge, not a measured RSS or a hard
    /// SQLite heap ceiling. Caller must add SQLite heap/pins to its whole bill.
    pub fn state_charge(limits: SegmentSubtreeMemoLimitsV2) -> Result<usize> {
        if limits.max_entries == 0
            || limits.max_entries == u64::MAX
            || limits.cache_bytes == 0
            || limits.cache_bytes == usize::MAX
            || limits.max_state_bytes == 0
            || limits.max_state_bytes == usize::MAX
        {
            return Err(refusal("V2 subtree memo limits are not finite"));
        }
        size_of::<Self>()
            .checked_add(limits.cache_bytes)
            .and_then(|n| n.checked_add(2 * KEY_BUFFER))
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| refusal("V2 subtree memo state charge overflowed"))
    }
    /// `basis` binds the owner's frozen retention basis AND inventory identity.
    /// It is an equality fence, not selected-root or native admission authority.
    pub fn new(
        reader: &SegmentIndexReaderV2,
        workspace: File,
        request: PinnedSqliteAuxRequest,
        basis: Digest256,
        limits: SegmentSubtreeMemoLimitsV2,
    ) -> Result<Self> {
        reader.check_request()?;
        if !reader.shares_request(
            &request.io_budget,
            &request.space_budget,
            request.deadline,
            &request.cancelled,
        ) {
            return Err(refusal("V2 subtree memo request differs from held reader"));
        }
        let charge = Self::state_charge(limits)?;
        if charge
            .checked_add(limits.retained_caller_bytes)
            .is_none_or(|n| n > limits.max_state_bytes)
        {
            return Err(refusal("V2 subtree memo exceeds its state precharge"));
        }
        // Negative cache_size uses KiB. Round DOWN so it never grants a larger
        // nominal cache; the separate native heap/pin bill remains explicit.
        let cache_kib = limits.cache_bytes / 1024;
        if cache_kib == 0 || cache_kib > i32::MAX as usize {
            return Err(refusal("V2 subtree memo cache charge is not representable"));
        }
        let io = request.io_budget.clone();
        let space = request.space_budget.clone();
        let deadline = request.deadline;
        let cancelled = request.cancelled.clone();
        let mut scope = PinnedSqliteAuxScope::new(workspace, request)?;
        reader.check_request()?;
        let db = scope.open_connection()?;
        reader.check_request()?;
        let settings = format!(
            "PRAGMA page_size=4096; PRAGMA cache_size=-{cache_kib}; PRAGMA mmap_size=0; PRAGMA temp_store=FILE; PRAGMA cache_spill=ON; CREATE TABLE completed(k BLOB PRIMARY KEY, pages BLOB NOT NULL CHECK(length(pages)=12)) WITHOUT ROWID;"
        );
        db.execute_batch(&settings).map_err(sql_error)?;
        reader.check_request()?;
        Ok(Self {
            db,
            _scope: scope,
            io,
            space,
            deadline,
            cancelled,
            directory: reader.memo_directory_identity(),
            basis,
            reader_limits: reader.read_limits(),
            limits,
            entries: 0,
            abandoned: false,
        })
    }
    pub fn required_state_charge(&self) -> Result<usize> {
        Self::state_charge(self.limits)
    }
    pub(crate) fn matches(&self, reader: &SegmentIndexReaderV2, basis: Digest256) -> Result<()> {
        reader.check_request()?;
        if self.abandoned
            || self.basis != basis
            || self.reader_limits != reader.read_limits()
            || self.directory != reader.memo_directory_identity()
            || !reader.shares_request(&self.io, &self.space, self.deadline, &self.cancelled)
        {
            return Err(refusal("V2 subtree memo basis or original request differs"));
        }
        Ok(())
    }
    pub(crate) fn abandon(&mut self) {
        self.abandoned = true;
    }
    pub(crate) fn lookup(
        &self,
        reader: &SegmentIndexReaderV2,
        basis: Digest256,
        expectation: &SegmentIndexPageExpectationV2,
        is_root: bool,
        callback_domain: Digest256,
    ) -> Result<Option<CompletedSubtreeV2>> {
        self.matches(reader, basis)?;
        let mut raw = [0u8; KEY_BUFFER];
        let key = encode_key(
            reader.keyspace(),
            expectation,
            is_root,
            callback_domain,
            &mut raw,
        )?;
        let count = self
            .db
            .query_row(
                "SELECT pages FROM completed WHERE k=?1",
                params![key],
                |row| {
                    let rusqlite::types::ValueRef::Blob(raw) = row.get_ref(0)? else {
                        return Err(rusqlite::Error::InvalidQuery);
                    };
                    let bytes: [u8; 12] =
                        raw.try_into().map_err(|_| rusqlite::Error::InvalidQuery)?;
                    let mut count_bytes = [0u8; 8];
                    count_bytes.copy_from_slice(&bytes[..8]);
                    let mut height_bytes = [0u8; 4];
                    height_bytes.copy_from_slice(&bytes[8..]);
                    let pages = u64::from_le_bytes(count_bytes);
                    let height = u32::from_le_bytes(height_bytes);
                    if pages == 0 || height == 0 || u64::from(height) > pages {
                        return Err(rusqlite::Error::InvalidQuery);
                    }
                    Ok(CompletedSubtreeV2 { pages, height })
                },
            )
            .optional()
            .map_err(sql_error)?;
        self.matches(reader, basis)?;
        if count.as_ref().is_some_and(|done| {
            done.pages > expectation.physical_summary.encoded_page_bytes / 16
                || done.height > self.reader_limits.max_depth
        }) {
            return Err(refusal(
                "V2 subtree memo completion shape differs from its page budget",
            ));
        }
        Ok(count)
    }
    /// Only crate-private walker EOF can invoke this mint. Inventory callbacks
    /// have succeeded, and actual subtree totals have closed at this expectation.
    pub(crate) fn record_completed(
        &mut self,
        reader: &SegmentIndexReaderV2,
        basis: Digest256,
        expectation: &SegmentIndexPageExpectationV2,
        is_root: bool,
        callback_domain: Digest256,
        page_count: u64,
        height: u32,
    ) -> Result<()> {
        self.matches(reader, basis)?;
        if page_count == 0 || height == 0 || self.entries >= self.limits.max_entries {
            return Err(refusal("V2 subtree completion exceeds its memo row cap"));
        }
        let mut raw = [0u8; KEY_BUFFER];
        let key = encode_key(
            reader.keyspace(),
            expectation,
            is_root,
            callback_domain,
            &mut raw,
        )?;
        let mut bytes = [0u8; 12];
        bytes[..8].copy_from_slice(&page_count.to_le_bytes());
        bytes[8..].copy_from_slice(&height.to_le_bytes());
        let changed = self
            .db
            .execute(
                "INSERT INTO completed(k,pages) VALUES(?1,?2)",
                params![key, &bytes[..]],
            )
            .map_err(sql_error)?;
        self.matches(reader, basis)?;
        if changed != 1 {
            return Err(refusal("V2 subtree completion was not uniquely stored"));
        }
        self.entries = self
            .entries
            .checked_add(1)
            .ok_or_else(|| refusal("V2 subtree memo row count overflowed"))?;
        Ok(())
    }
    /// Close the private database explicitly; no extraction/rebind is offered.
    /// Drop still releases the scope on failure, but failure is never success.
    pub fn close(self) -> Result<()> {
        let Self { db, _scope, .. } = self;
        let closed = match db.close() {
            Ok(()) => Ok(()),
            Err((db, error)) => {
                drop(db);
                Err(sql_error(error))
            }
        };
        drop(_scope);
        closed
    }
}
fn encode_key<'a>(
    keyspace: SegmentIndexKeySpaceV1,
    e: &SegmentIndexPageExpectationV2,
    is_root: bool,
    callback_domain: Digest256,
    raw: &'a mut [u8; KEY_BUFFER],
) -> Result<&'a [u8]> {
    let mut sink = Cursor::new(&mut raw[..]);
    sink.write_all(callback_domain.as_bytes())
        .map_err(key_error)?;
    sink.write_all(&[keyspace as u8, u8::from(is_root)])
        .map_err(key_error)?;
    sink.write_all(e.extent.segment_sha256.as_bytes())
        .map_err(key_error)?;
    for n in [e.extent.offset, e.extent.length] {
        sink.write_all(&n.to_le_bytes()).map_err(key_error)?;
    }
    sink.write_all(e.page_sha256.as_bytes())
        .map_err(key_error)?;
    for n in [
        e.physical_summary.row_count,
        e.physical_summary.value_bytes,
        e.physical_summary.encoded_page_bytes,
        e.logical_summary.row_count,
        e.logical_summary.key_bytes,
        e.logical_summary.value_bytes,
    ] {
        sink.write_all(&n.to_le_bytes()).map_err(key_error)?;
    }
    sink.write_all(e.logical_summary.logical_sha256.as_bytes())
        .map_err(key_error)?;
    for bound in [e.lower_bound(), e.upper_bound()] {
        let bytes = bound.unwrap_or(b"");
        let n = u16::try_from(bytes.len())
            .map_err(|_| refusal("V2 memo bound exceeds its fixed key"))?;
        sink.write_all(&n.to_le_bytes()).map_err(key_error)?;
        sink.write_all(bytes).map_err(key_error)?;
    }
    let used =
        usize::try_from(sink.position()).map_err(|_| refusal("V2 memo key length overflowed"))?;
    drop(sink);
    Ok(&raw[..used])
}
fn key_error(error: std::io::Error) -> StoreError {
    StoreError::io("V2 memo fixed key encoding failed", error)
}
fn sql_error(error: rusqlite::Error) -> StoreError {
    StoreError::io(
        "V2 private subtree memo failed",
        std::io::Error::other(error),
    )
}
fn refusal(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}
