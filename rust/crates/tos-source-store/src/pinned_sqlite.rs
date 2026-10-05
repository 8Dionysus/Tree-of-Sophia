//! Exact-inode SQLite access for private derived indexes and pinned immutable models.
//!
//! SQLite's Unix pathname canonicalizer resolves `/proc/self/fd/N` symlinks.
//! For O_TMPFILE this becomes a different named `(deleted)` file. This narrow
//! VFS retains the descriptor spelling, permits only the main database, and
//! never creates filesystem objects or sidecars.

use crate::{Result, StoreError, StoreErrorCode};
use rusqlite::{Connection, OpenFlags, ffi};
use std::os::fd::FromRawFd;
use std::sync::atomic::AtomicBool;
use std::{
    cell::RefCell,
    ffi::CStr,
    fs::{File, TryLockError},
    io::{ErrorKind, Read},
    ops::{Deref, DerefMut},
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::fs::{FileExt, MetadataExt},
    },
    sync::{Arc, OnceLock},
    time::Instant,
};

// This owner already requires Linux through tos-fd-open. Use the same local
// C-ABI convention as git_capture, without a new crate or pathname allocation.
// No O_CREAT argument is used: the retained kernel FD is the sole target.
const LINUX_O_RDONLY: std::ffi::c_int = 0;
const LINUX_O_RDWR: std::ffi::c_int = 2;
const LINUX_O_CLOEXEC: std::ffi::c_int = 0x80000;
unsafe extern "C" {
    #[link_name = "open"]
    fn linux_open(path: *const std::ffi::c_char, flags: std::ffi::c_int, ...) -> std::ffi::c_int;
}

const VFS_NAME: &CStr = c"tos-pinned-fd-v1";
static VFS: OnceLock<std::result::Result<usize, i32>> = OnceLock::new();

/// Owns the descriptor used by SQLite until after the connection is closed.
/// This is IO custody only; callers retain source/currentness/rights checks.
/// Supports the existing trusted local Linux filesystem profile, not remote
/// filesystems with weaker advisory-lock semantics.
#[derive(Debug)]
pub struct PinnedSqliteConnection {
    db: Connection,
    _file: File,
    _aux_vfs: Option<Arc<crate::pinned_sqlite_aux::AuxVfsLease>>,
    ordinary_fd_without_policy: bool,
}
impl Deref for PinnedSqliteConnection {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.db
    }
}
impl DerefMut for PinnedSqliteConnection {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.db
    }
}
impl PinnedSqliteConnection {
    /// One registered FD VFS descriptor remains live for this native process.
    /// The dedicated caller reserves this once, before any pinned open; it is
    /// Rust allocation, separate from the SQLite allocator pool.
    pub fn process_rust_state_upper_bound() -> usize {
        std::mem::size_of::<ffi::sqlite3_vfs>()
    }
    /// Distinct persistent Rust heaps for the non-policy, uncached FD route.
    /// rusqlite owns an Arc<Mutex<sqlite3*>> interrupt handle; the VFS owns File.
    /// Connection inline fields remain in the containing owner's typed census.
    pub fn retained_rust_state_upper_bound(&self) -> Result<usize> {
        if !self.ordinary_fd_without_policy {
            return Err(invalid("owned census requires ordinary FD VFS"));
        }
        Ok(Self::immutable_retained_rust_state_upper_bound())
    }
    pub fn immutable_retained_rust_state_upper_bound() -> usize {
        std::mem::size_of::<File>()
            + 2 * std::mem::size_of::<std::sync::atomic::AtomicUsize>()
            + std::mem::size_of::<std::sync::Mutex<*mut ffi::sqlite3>>()
    }
    /// FD spellings are fixed stack arrays. The sole borrowed-path CString
    /// allocation in rusqlite uses Rust's len+1 slice specialization. Its exact
    /// maximum is the URI array size (including the terminating NUL).
    pub fn immutable_open_rust_workspace_upper_bound() -> usize {
        2 * std::mem::size_of::<FdSpelling>()
            + 10 * std::mem::size_of::<u8>() // FD digit scratch, nested spelling builder
            + FD_SPELLING_BYTES
            + std::mem::size_of::<std::ffi::CString>()
            + 3 * std::mem::size_of::<File>()
            + 4 * std::mem::size_of::<std::fs::Metadata>()
            + std::mem::size_of::<Connection>()
            + std::mem::size_of::<PendingPolicy>()
    }
    /// Admit actual distinct persistent heap plus opening scratch before any
    /// descriptor clone, VFS registration, CString or connection allocation.
    /// The process-lifetime VFS slot is separately reserved once by caller.
    pub fn open_readonly_immutable_with_state(
        file: &File,
        remaining_after_retained: &dyn Fn(usize) -> Result<usize>,
    ) -> Result<Self> {
        remaining_after_retained(
            Self::immutable_retained_rust_state_upper_bound()
                + Self::immutable_open_rust_workspace_upper_bound(),
        )?;
        Self::open_with_policy(file, true, None, true)
    }
    /// Maximum distinct Rust controllers for a bounded statement/cache pragma.
    /// The statement itself lives in native SQLite's separate admitted pool.
    pub fn bounded_statement_rust_workspace_upper_bound() -> usize {
        std::mem::size_of::<PinnedBoundedStatement<'_>>()
            + std::mem::size_of::<FdSpelling>()
            + 10 * std::mem::size_of::<u8>()
    }

    /// Prepare on the same held connection without copying native diagnostic
    /// text into a second Rust heap. SQL comes from the caller's maintained
    /// literal owner; native statement allocations remain in the admitted pool.
    pub fn prepare_static_bounded(&self, sql: &CStr) -> Result<PinnedBoundedStatement<'_>> {
        if !self.ordinary_fd_without_policy {
            return Err(invalid("bounded statement requires ordinary FD VFS"));
        }
        let mut statement = std::ptr::null_mut();
        let status = unsafe {
            ffi::sqlite3_prepare_v2(
                self.db.handle(),
                sql.as_ptr(),
                -1,
                &mut statement,
                std::ptr::null_mut(),
            )
        };
        if status != ffi::SQLITE_OK || statement.is_null() {
            if !statement.is_null() {
                unsafe {
                    ffi::sqlite3_finalize(statement);
                }
            }
            return Err(invalid("pinned SQLite bounded statement prepare failed"));
        }
        Ok(PinnedBoundedStatement {
            owner: self,
            statement,
            current_row: false,
            started: false,
            finished: std::cell::Cell::new(false),
        })
    }
    pub fn execute_static_bounded(&self, sql: &CStr) -> Result<()> {
        let mut statement = self.prepare_static_bounded(sql)?;
        while statement.step()? {}
        Ok(())
    }
    /// Exact negative existing cache cap; fixed stack spelling, no format String.
    pub fn set_cache_kib_bounded(&self, kib: u32) -> Result<()> {
        let sql = FdSpelling::build(kib as i64, b"PRAGMA cache_size=-", b"", 10)?;
        let sql = CStr::from_bytes_with_nul(&sql.bytes[..sql.length + 1])
            .map_err(|_| invalid("bounded SQLite cache spelling"))?;
        self.execute_static_bounded(sql)
    }

    /// Open only a fresh private unnamed inode. Scratch ceilings and statement
    /// budgets remain with the existing derived-index caller.
    pub fn open_private_derived(file: &File) -> Result<Self> {
        Self::open_private_derived_with_policy(file, None)
    }

    pub(super) fn open_private_derived_with_policy(
        file: &File,
        policy: Option<Arc<dyn FdIoPolicy>>,
    ) -> Result<Self> {
        let before = file
            .metadata()
            .map_err(|_| invalid("private SQLite descriptor metadata"))?;
        if !before.is_file()
            || before.nlink() != 0
            || before.len() != 0
            || before.mode() & 0o777 != 0o600
            || before.uid() != current_fs_uid()?
        {
            return Err(invalid(
                "private SQLite descriptor must be fresh unnamed regular file",
            ));
        }
        let db = Self::open_with_policy(file, false, policy, false)?;
        // These precede the first schema/data write. The VFS independently
        // refuses journal/WAL/temp filenames even if a caller changes pragmas.
        db.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=MEMORY; PRAGMA mmap_size=0;")
            .map_err(|_| invalid("private SQLite settings could not be enforced"))?;
        Ok(db)
    }

    /// Continue one owner-sanctioned preparation of an already-populated,
    /// disposable private capture. FD possession is mechanical custody;
    /// source/family authority, finite ceilings and permanent failure stay
    /// with PublicCapture. This is not the fresh unnamed derived contract.
    pub fn open_private_capture_for_family_preparation(file:&File)->Result<Self> {
        Self::open_private_capture_for_family_preparation_with_setup(file, |_| {})
    }

    /// Install the owner's existing progress hook before any settings SQL.
    pub fn open_private_capture_for_family_preparation_with_setup(
        file:&File, before_settings:impl FnOnce(&Self),
    )->Result<Self> {
        let before=file.metadata().map_err(|_|invalid("private capture descriptor metadata"))?;
        if !before.is_file() || before.nlink()!=1 || before.len()==0
            || before.mode()&0o777!=0o600 || before.uid()!=current_fs_uid()? {
            return Err(invalid("private capture must be existing owned private regular inode"));
        }
        let db=Self::open_with_policy(file,false,None,true)?;
        before_settings(&db);
        // A partial failure permanently poisons this disposable capture; no
        // rollback/reopen/adoption claim is allowed. Keep FILE temp: this VFS
        // refuses unowned auxiliary names rather than adding a temp grant.
        db.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;")
            .map_err(|_|invalid("private capture pinned settings could not be enforced"))?;
        Ok(db)
    }

    /// Reopen the exact retained inode read-only, without pathname resolution.
    /// Authenticity, schema, verity and selected-use authority stay with caller.
    pub fn open_readonly_immutable(file: &File) -> Result<Self> {
        Self::open(file, true)
    }

    /// Reopen the exact retained inode read-only while charging the caller's
    /// shared logical I/O budget on each actual pager read. The deadline and
    /// cancellation flag are checked at the same I/O boundary. Source
    /// authenticity and selected-use authority remain with the caller.
    pub fn open_readonly_immutable_budgeted(
        file: &File,
        io_budget: crate::pinned_sqlite_aux::PinnedSqliteIoBudget,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        let policy: Arc<dyn FdIoPolicy> = Arc::new(ImmutableReadPolicy {
            budget: io_budget,
            deadline,
            cancelled,
        });
        Self::open_with_policy(file, true, Some(policy), false)
    }

    /// Preserve SQLite's explicit close result and keep the pinned descriptor
    /// with the still-live connection if close refuses (for example, BUSY).
    pub fn close(self) -> std::result::Result<(), (Self, rusqlite::Error)> {
        let Self {
            db,
            _file,
            _aux_vfs,
            ordinary_fd_without_policy,
        } = self;
        match db.close() {
            Ok(()) => {
                if let Some(lease) = &_aux_vfs {
                    lease.mark_closed();
                }
                drop(_file);
                drop(_aux_vfs);
                Ok(())
            }
            Err((db, error)) => Err((
                Self {
                    db,
                    _file,
                    _aux_vfs,
                    ordinary_fd_without_policy,
                },
                error,
            )),
        }
    }

    fn open(file: &File, readonly: bool) -> Result<Self> {
        Self::open_with_policy(file, readonly, None, false)
    }

    fn open_with_policy(
        file: &File,
        readonly: bool,
        policy: Option<Arc<dyn FdIoPolicy>>,
        bounded_error_copy: bool,
    ) -> Result<Self> {
        let owned = file
            .try_clone()
            .map_err(|_| invalid("SQLite pinned descriptor clone"))?;
        let before = owned
            .metadata()
            .map_err(|_| invalid("SQLite pinned descriptor metadata"))?;
        if !before.is_file() {
            return Err(invalid("SQLite pinned descriptor is not regular"));
        }
        register_vfs()?;
        let (name, flags) = if readonly {
            (
                FdSpelling::immutable_uri(owned.as_raw_fd())?,
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE,
            )
        } else {
            (
                FdSpelling::path(owned.as_raw_fd())?,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE,
            )
        };
        let ordinary_fd_without_policy = policy.is_none();
        let _pending_policy = PendingPolicy::install(policy);
        let db = if bounded_error_copy {
            // The same selected VFS/flags/native open kernel; inspect its status
            // directly so a native error never becomes an unadmitted Rust text
            // copy in rusqlite before this owner's static refusal boundary.
            // Preserve rusqlite0.37.0's genuine safe-threading guard before
            // adopting the handle (its from_handle_owned assumes this owner).
            if unsafe { ffi::sqlite3_threadsafe() } == 0 {
                return Err(invalid("pinned SQLite single threaded build refused"));
            }
            let mutex = unsafe { ffi::sqlite3_mutex_alloc(0) };
            let single_threaded = mutex as usize == 8;
            unsafe {
                ffi::sqlite3_mutex_free(mutex);
            }
            if single_threaded {
                return Err(invalid("pinned SQLite single threaded mode refused"));
            }
            let mut raw = std::ptr::null_mut();
            let modern = unsafe { ffi::sqlite3_libversion_number() } >= 3_037_000;
            let bits = flags.bits()
                | if modern {
                    ffi::SQLITE_OPEN_EXRESCODE
                } else {
                    0
                };
            let status = unsafe {
                ffi::sqlite3_open_v2(
                    name.bytes.as_ptr().cast(),
                    &mut raw,
                    bits,
                    VFS_NAME.as_ptr(),
                )
            };
            if status != ffi::SQLITE_OK {
                if !raw.is_null() {
                    unsafe {
                        ffi::sqlite3_close(raw);
                    }
                }
                return Err(invalid("exact pinned SQLite main database open failed"));
            }
            if !modern {
                unsafe {
                    ffi::sqlite3_extended_result_codes(raw, 1);
                }
            }
            if unsafe { ffi::sqlite3_busy_timeout(raw, 5000) } != ffi::SQLITE_OK {
                unsafe {
                    ffi::sqlite3_close(raw);
                }
                return Err(invalid("exact pinned SQLite busy timeout setup failed"));
            }
            // This successful native open is exclusively owned here; maintained
            // rusqlite adoption retains its normal close/interrupt/cache owner.
            unsafe { Connection::from_handle_owned(raw) }
                .map_err(|_| invalid("exact pinned SQLite owned handle adoption failed"))?
        } else {
            Connection::open_with_flags_and_vfs(name.as_str(), flags, VFS_NAME)
                .map_err(|_| invalid("exact pinned SQLite main database open failed"))?
        };
        let after = owned
            .metadata()
            .map_err(|_| invalid("SQLite pinned descriptor recheck"))?;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.uid() != after.uid()
            || before.mode() != after.mode()
        {
            return Err(invalid("SQLite pinned descriptor identity changed"));
        }
        Ok(Self {
            db,
            _file: owned,
            _aux_vfs: None,
            ordinary_fd_without_policy,
        })
    }

    pub(super) fn open_auxiliary(
        file: &File,
        lease: Arc<crate::pinned_sqlite_aux::AuxVfsLease>,
    ) -> Result<Self> {
        let owned = file
            .try_clone()
            .map_err(|_| invalid("SQLite auxiliary main descriptor clone"))?;
        let before = owned
            .metadata()
            .map_err(|_| invalid("SQLite auxiliary main descriptor metadata"))?;
        if !before.is_file() || before.nlink() != 0 || before.uid() != current_fs_uid()? {
            return Err(invalid(
                "SQLite auxiliary main descriptor is not a private unnamed inode",
            ));
        }
        let path = format!("/proc/self/fd/{}", owned.as_raw_fd());
        lease.set_main_path(path.as_bytes().to_vec())?;
        let db = Connection::open_with_flags_and_vfs(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE,
            lease
                .name()
                .to_str()
                .map_err(|_| invalid("SQLite auxiliary VFS name"))?,
        )
        .map_err(|_| invalid("SQLite auxiliary SQLite connection open failed"))?;
        let after = owned
            .metadata()
            .map_err(|_| invalid("SQLite auxiliary main descriptor recheck"))?;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.uid() != after.uid()
            || before.mode() != after.mode()
            || before.nlink() != after.nlink()
        {
            return Err(invalid("SQLite auxiliary main descriptor identity changed"));
        }
        Ok(Self {
            db,
            _file: owned,
            _aux_vfs: Some(lease),
            ordinary_fd_without_policy: false,
        })
    }
}

/// A native statement borrowing the exact pinned owner. No Send/Sync model
/// transfer or error-message heap is introduced. Column borrows end before
/// the next mutable step; finalization always precedes connection release.
pub struct PinnedBoundedStatement<'a> {
    owner: &'a PinnedSqliteConnection,
    statement: *mut ffi::sqlite3_stmt,
    current_row: bool,
    started: bool,
    finished: std::cell::Cell<bool>,
}
impl PinnedBoundedStatement<'_> {
    pub fn bind_i64(&mut self, index: i32, value: i64) -> Result<()> {
        if self.finished.get()
            || self.started
            || index <= 0
            || unsafe { ffi::sqlite3_bind_int64(self.statement, index, value) } != ffi::SQLITE_OK
        {
            self.finished.set(true);
            self.started = true;
            self.current_row = false;
            return Err(invalid("pinned SQLite bounded integer binding failed"));
        }
        Ok(())
    }
    /// Copy one already-admitted borrowed text parameter into the same dedicated
    /// SQLite pool. No Rust String/CString or error-message copy is allocated.
    pub fn bind_text(&mut self, index: i32, value: &str) -> Result<()> {
        if self.finished.get() || self.started || index <= 0 || value.len() > i32::MAX as usize {
            self.finished.set(true);
            self.started = true;
            self.current_row = false;
            return Err(invalid("pinned SQLite bounded text binding failed"));
        }
        let status = unsafe {
            ffi::sqlite3_bind_text(
                self.statement,
                index,
                value.as_ptr().cast(),
                value.len() as i32,
                ffi::SQLITE_TRANSIENT(),
            )
        };
        if status != ffi::SQLITE_OK {
            self.finished.set(true);
            self.started = true;
            self.current_row = false;
            return Err(invalid("pinned SQLite bounded text binding failed"));
        }
        Ok(())
    }
    pub fn step(&mut self) -> Result<bool> {
        if self.finished.get() {
            return Err(invalid("pinned SQLite bounded statement already terminal"));
        }
        self.started = true;
        let status = unsafe { ffi::sqlite3_step(self.statement) };
        self.current_row = status == ffi::SQLITE_ROW;
        self.finished.set(status != ffi::SQLITE_ROW);
        match status {
            ffi::SQLITE_ROW => Ok(true),
            ffi::SQLITE_DONE => Ok(false),
            _ => Err(invalid("pinned SQLite bounded statement step failed")),
        }
    }
    fn text_bytes(&self, column: i32) -> Result<&[u8]> {
        if self.finished.get()
            || !self.current_row
            || column < 0
            || column >= unsafe { ffi::sqlite3_column_count(self.statement) }
            || unsafe { ffi::sqlite3_column_type(self.statement, column) } != ffi::SQLITE_TEXT
        {
            self.finished.set(true);
            return Err(invalid("pinned SQLite bounded text column required"));
        }
        let length = unsafe { ffi::sqlite3_column_bytes(self.statement, column) };
        let pointer = unsafe { ffi::sqlite3_column_text(self.statement, column) };
        if length < 0 || pointer.is_null() {
            self.finished.set(true);
            return Err(invalid("pinned SQLite bounded text unavailable"));
        }
        // Native SQLite storage stays valid through this borrow; step requires
        // &mut self, so it cannot invalidate text while the borrowed str lives.
        let bytes = unsafe { std::slice::from_raw_parts(pointer, length as usize) };
        Ok(bytes)
    }
    pub fn text(&self, column: i32) -> Result<&str> {
        std::str::from_utf8(self.text_bytes(column)?).map_err(|_| {
            self.finished.set(true);
            invalid("pinned SQLite bounded text UTF-8")
        })
    }
    /// Caller admits these actual borrowed validation controller/error slots
    /// before text_with_check, independently of the shared native SQLite pool.
    pub fn text_validation_rust_workspace_upper_bound() -> usize {
        std::mem::size_of::<(&[u8], usize, usize, i32, *const u8)>()
            + std::mem::size_of::<std::str::Utf8Error>()
            + std::mem::size_of::<Result<&str>>()
            + std::mem::size_of::<Result<&[u8]>>()
            + std::mem::size_of::<&Self>()
            + std::mem::size_of::<&mut dyn FnMut(usize) -> Result<()>>()
    }
    /// Validate the genuine borrowed SQLite text only after the caller has
    /// charged each bounded byte scan against its original work/cutoff owner.
    /// No copied buffer, assumed UTF-8, reset or error allocation is introduced.
    pub fn text_with_check(
        &self,
        column: i32,
        check: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<&str> {
        let result = (|| {
            let bytes = self.text_bytes(column)?;
            let mut at = 0usize;
            while at < bytes.len() {
                let end = at.saturating_add(65536).min(bytes.len());
                check(end - at)?;
                match std::str::from_utf8(&bytes[at..end]) {
                    Ok(_) => at = end,
                    Err(error)
                        if error.error_len().is_none()
                            && end < bytes.len()
                            && error.valid_up_to() > 0 =>
                    {
                        // Recheck the at-most-three-byte trailing fragment in
                        // the next admitted chunk; never skip partial input.
                        at += error.valid_up_to();
                    }
                    Err(_) => return Err(invalid("pinned SQLite bounded text UTF-8")),
                }
            }
            check(0)?;
            // Every byte was validated above. Text remains borrowed from the
            // same statement; mutable step still cannot overlap this borrow.
            Ok(unsafe { std::str::from_utf8_unchecked(bytes) })
        })();
        if result.is_err() {
            self.finished.set(true);
        }
        result
    }
}
impl Drop for PinnedBoundedStatement<'_> {
    fn drop(&mut self) {
        let _ = self.owner;
        unsafe {
            ffi::sqlite3_finalize(self.statement);
        }
    }
}

/// Narrow callbacks let the auxiliary VFS reuse the exact same descriptor IO
/// implementation while attaching scope-specific budgets and inode ceilings.
pub(super) trait FdIoPolicy: Send + Sync {
    fn begin_read(&self, _bytes: u64) -> bool {
        true
    }
    fn record_read(&self, _bytes: u64) -> bool {
        true
    }
    fn before_write(&self, _file: &File, _offset: u64, _bytes: u64) -> bool {
        true
    }
    fn record_write(&self, _bytes: u64) -> bool {
        true
    }
    fn before_truncate(&self, _file: &File, _size: u64) -> bool {
        true
    }
    fn after_mutation(&self, _file: &File) -> bool {
        true
    }
    fn check_operation(&self) -> bool {
        true
    }
    fn failed_io(&self) {}
    fn file_closed(&self) {}
}

pub(super) fn sqlite_file_size() -> usize {
    std::mem::size_of::<FdFile>()
}

struct PolicyHolder(Arc<dyn FdIoPolicy>);
thread_local! {
    static PENDING_POLICY: RefCell<Option<Arc<dyn FdIoPolicy>>> = const { RefCell::new(None) };
}
struct PendingPolicy(Option<Arc<dyn FdIoPolicy>>);
impl PendingPolicy {
    fn install(policy: Option<Arc<dyn FdIoPolicy>>) -> Self {
        let previous = PENDING_POLICY.with(|slot| slot.replace(policy));
        Self(previous)
    }
}
impl Drop for PendingPolicy {
    fn drop(&mut self) {
        let previous = self.0.take();
        PENDING_POLICY.with(|slot| {
            slot.replace(previous);
        });
    }
}

struct ImmutableReadPolicy {
    budget: crate::pinned_sqlite_aux::PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl FdIoPolicy for ImmutableReadPolicy {
    fn begin_read(&self, bytes: u64) -> bool {
        if self.budget.charge_read(bytes).is_err() {
            return false;
        }
        if Instant::now() >= self.deadline {
            self.budget
                .fail(crate::pinned_sqlite_aux::PinnedSqliteIoFailure::Deadline);
            return false;
        }
        if self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            self.budget
                .fail(crate::pinned_sqlite_aux::PinnedSqliteIoFailure::Cancelled);
            return false;
        }
        true
    }
    fn record_read(&self, bytes: u64) -> bool {
        if self.budget.record_read_returned(bytes).is_err() {
            return false;
        }
        if Instant::now() >= self.deadline {
            self.budget
                .fail(crate::pinned_sqlite_aux::PinnedSqliteIoFailure::Deadline);
            return false;
        }
        if self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            self.budget
                .fail(crate::pinned_sqlite_aux::PinnedSqliteIoFailure::Cancelled);
            return false;
        }
        true
    }
    fn check_operation(&self) -> bool {
        if Instant::now() >= self.deadline {
            self.budget
                .fail(crate::pinned_sqlite_aux::PinnedSqliteIoFailure::Deadline);
            false
        } else if self.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            self.budget
                .fail(crate::pinned_sqlite_aux::PinnedSqliteIoFailure::Cancelled);
            false
        } else {
            true
        }
    }
    fn failed_io(&self) {
        self.budget
            .fail(crate::pinned_sqlite_aux::PinnedSqliteIoFailure::Io);
    }
}

const FD_SPELLING_BYTES: usize =
    b"file:/proc/self/fd/".len() + 10 + b"?mode=ro&immutable=1".len() + 1;
struct FdSpelling {
    bytes: [u8; FD_SPELLING_BYTES],
    length: usize,
}
impl FdSpelling {
    fn build(fd: i64, prefix: &[u8], suffix: &[u8], maximum_digits: usize) -> Result<Self> {
        if fd < 0 {
            return Err(invalid("SQLite negative retained descriptor"));
        }
        let mut out = Self {
            bytes: [0; FD_SPELLING_BYTES],
            length: 0,
        };
        out.bytes[..prefix.len()].copy_from_slice(prefix);
        out.length = prefix.len();
        let mut digits = [0u8; 10];
        if fd > u32::MAX as i64 || maximum_digits != 10 {
            return Err(invalid("SQLite bounded decimal spelling"));
        }
        let mut n = fd as u32;
        let mut start = digits.len();
        loop {
            start -= 1;
            digits[start] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        let count = digits.len() - start;
        out.bytes[out.length..out.length + count].copy_from_slice(&digits[start..]);
        out.length += count;
        out.bytes[out.length..out.length + suffix.len()].copy_from_slice(suffix);
        out.length += suffix.len();
        Ok(out)
    }
    fn path(fd: i32) -> Result<Self> {
        Self::build(fd as i64, b"/proc/self/fd/", b"", 10)
    }
    fn immutable_uri(fd: i32) -> Result<Self> {
        Self::build(
            fd as i64,
            b"file:/proc/self/fd/",
            b"?mode=ro&immutable=1",
            10,
        )
    }
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.length]).expect("fixed ASCII FD spelling")
    }
}

fn invalid(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::DescriptorMismatch, detail)
}

fn register_vfs() -> Result<()> {
    let registration = VFS.get_or_init(|| {
        // SQLite serializes registration. A single process-lifetime descriptor
        // is deliberately leaked because SQLite retains the registered pointer.
        unsafe {
            let base = ffi::sqlite3_vfs_find(c"unix".as_ptr());
            if base.is_null() {
                return Err(ffi::SQLITE_CANTOPEN);
            }
            let mut vfs = Box::new(*base);
            vfs.pNext = std::ptr::null_mut();
            vfs.zName = VFS_NAME.as_ptr();
            vfs.pAppData = std::ptr::null_mut();
            vfs.szOsFile = std::mem::size_of::<FdFile>() as i32;
            vfs.xSetSystemCall = None;
            vfs.xGetSystemCall = None;
            vfs.xNextSystemCall = None;
            vfs.xFullPathname = Some(full_path);
            vfs.xOpen = Some(open_main);
            vfs.xDelete = Some(no_delete);
            vfs.xAccess = Some(access_main);
            let pointer = Box::into_raw(vfs);
            let code = ffi::sqlite3_vfs_register(pointer, 0);
            if code != ffi::SQLITE_OK {
                drop(Box::from_raw(pointer));
                return Err(code);
            }
            Ok(pointer as usize)
        }
    });
    if registration.is_err() {
        return Err(invalid("exact pinned SQLite VFS unavailable"));
    }
    Ok(())
}

unsafe fn is_fd_name(name: *const std::ffi::c_char) -> bool {
    if name.is_null() {
        return false;
    }
    let bytes = unsafe { CStr::from_ptr(name) }.to_bytes();
    let Some(digits) = bytes.strip_prefix(b"/proc/self/fd/") else {
        return false;
    };
    !digits.is_empty() && digits.len() <= 10 && digits.iter().all(u8::is_ascii_digit)
}
unsafe extern "C" fn full_path(
    _: *mut ffi::sqlite3_vfs,
    name: *const std::ffi::c_char,
    count: i32,
    output: *mut std::ffi::c_char,
) -> i32 {
    if !unsafe { is_fd_name(name) } || output.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    let raw = unsafe { CStr::from_ptr(name) }.to_bytes_with_nul();
    if count <= 0 || raw.len() >= count as usize {
        return ffi::SQLITE_CANTOPEN;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(raw.as_ptr().cast(), output, raw.len());
    }
    ffi::SQLITE_OK
}
// SQLite allocates this fixed-size storage with the alignment required by
// sqlite3_file. pMethods is installed only after ownership/locking succeeds.
#[repr(C)]
struct FdFile {
    base: ffi::sqlite3_file,
    owned: *mut File,
    policy: *mut PolicyHolder,
    readonly: bool,
    sqlite_lock: i32,
}
static METHODS: ffi::sqlite3_io_methods = ffi::sqlite3_io_methods {
    iVersion: 1,
    xClose: Some(close_file),
    xRead: Some(read_file),
    xWrite: Some(write_file),
    xTruncate: Some(truncate_file),
    xSync: Some(sync_file),
    xFileSize: Some(file_size),
    xLock: Some(lock_level),
    xUnlock: Some(unlock_level),
    xCheckReservedLock: Some(reserved_lock),
    xFileControl: Some(file_control),
    xSectorSize: Some(sector_size),
    xDeviceCharacteristics: Some(device_flags),
    xShmMap: None,
    xShmLock: None,
    xShmBarrier: None,
    xShmUnmap: None,
    xFetch: None,
    xUnfetch: None,
};
unsafe fn fd_file<'a>(file: *mut ffi::sqlite3_file) -> &'a FdFile {
    unsafe { &*file.cast::<FdFile>() }
}
unsafe fn owned_file<'a>(file: *mut ffi::sqlite3_file) -> &'a File {
    unsafe { &*fd_file(file).owned }
}
unsafe fn io_policy<'a>(file: *mut ffi::sqlite3_file) -> Option<&'a dyn FdIoPolicy> {
    let pointer = unsafe { fd_file(file).policy };
    if pointer.is_null() {
        None
    } else {
        Some(unsafe { &*pointer }.0.as_ref())
    }
}
unsafe extern "C" fn open_main(
    _: *mut ffi::sqlite3_vfs,
    name: *const std::ffi::c_char,
    file: *mut ffi::sqlite3_file,
    flags: i32,
    output_flags: *mut i32,
) -> i32 {
    if file.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    // SQLite may close an unsuccessful xOpen only if pMethods is non-null.
    unsafe {
        (*file).pMethods = std::ptr::null();
    }
    if !unsafe { is_fd_name(name) }
        || flags & ffi::SQLITE_OPEN_MAIN_DB == 0
        || flags & ffi::SQLITE_OPEN_CREATE != 0
    {
        return ffi::SQLITE_CANTOPEN;
    }
    let raw = unsafe { CStr::from_ptr(name) }.to_bytes();
    let Some(fd) = std::str::from_utf8(&raw[b"/proc/self/fd/".len()..])
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
    else {
        return ffi::SQLITE_CANTOPEN;
    };
    // Borrow only while the wrapper retains its cloned FD. Observing a clone
    // prevents accidentally acquiring ownership of SQLite's input descriptor.
    let observer = match unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned() {
        Ok(fd) => File::from(fd),
        Err(_) => return ffi::SQLITE_CANTOPEN,
    };
    let readonly = flags & ffi::SQLITE_OPEN_READONLY != 0;
    if readonly == (flags & ffi::SQLITE_OPEN_READWRITE != 0) {
        return ffi::SQLITE_CANTOPEN;
    }
    let path = match FdSpelling::path(fd) {
        Ok(path) => path,
        Err(_) => return ffi::SQLITE_CANTOPEN,
    };
    // Standard open follows this retained kernel FD directly (no realpath and
    // no O_NOFOLLOW). No CREATE: it cannot instantiate the '(deleted)' name.
    // Fixed ASCII spelling includes a trailing NUL. Borrowing it avoids the
    // std pathname adapter's separate temporary C buffer. No CREATE, and the
    // exact observer/actual inode check below remains the custody authority.
    let access = if readonly {
        LINUX_O_RDONLY
    } else {
        LINUX_O_RDWR
    };
    let opened = unsafe { linux_open(path.bytes.as_ptr().cast(), access | LINUX_O_CLOEXEC) };
    if opened < 0 {
        return ffi::SQLITE_CANTOPEN;
    }
    // open returned a new owned descriptor; File closes it on every refusal.
    let actual = unsafe { File::from_raw_fd(opened) };
    let (Ok(expected), Ok(observed)) = (observer.metadata(), actual.metadata()) else {
        return ffi::SQLITE_CANTOPEN;
    };
    if !observed.is_file()
        || expected.dev() != observed.dev()
        || expected.ino() != observed.ino()
        || expected.uid() != observed.uid()
        || expected.mode() != observed.mode()
        || expected.nlink() != observed.nlink()
    {
        return ffi::SQLITE_CANTOPEN;
    }
    let policy = PENDING_POLICY.with(|slot| slot.borrow().clone());
    let install_result = unsafe { install_fd_file(file, actual, readonly, policy) };
    if install_result != ffi::SQLITE_OK {
        return install_result;
    }
    if !output_flags.is_null() {
        unsafe {
            *output_flags = flags;
        }
    }
    ffi::SQLITE_OK
}

/// Install a validated descriptor in SQLite's fixed file storage, acquiring
/// the same connection-lifetime OFD lease used by the strict pinned VFS.
pub(super) unsafe fn install_fd_file(
    file: *mut ffi::sqlite3_file,
    actual: File,
    readonly: bool,
    policy: Option<Arc<dyn FdIoPolicy>>,
) -> i32 {
    if file.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    let locked = if readonly {
        actual.try_lock_shared()
    } else {
        actual.try_lock()
    };
    match locked {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return ffi::SQLITE_BUSY,
        Err(TryLockError::Error(_)) => return ffi::SQLITE_IOERR_LOCK,
    }
    let policy = policy.map(|value| Box::into_raw(Box::new(PolicyHolder(value))));
    unsafe {
        file.cast::<FdFile>().write(FdFile {
            base: ffi::sqlite3_file { pMethods: &METHODS },
            owned: Box::into_raw(Box::new(actual)),
            policy: policy.unwrap_or(std::ptr::null_mut()),
            readonly,
            sqlite_lock: ffi::SQLITE_LOCK_NONE,
        });
    }
    ffi::SQLITE_OK
}
unsafe extern "C" fn close_file(file: *mut ffi::sqlite3_file) -> i32 {
    let state = unsafe { fd_file(file) };
    let pointer = state.owned;
    let policy = state.policy;
    let owned = unsafe { Box::from_raw(pointer) };
    // Explicit unlock prevents a fork-before-exec inherited OFD extending the
    // connection's lease after SQLite has made its close decision.
    let result = owned.unlock();
    drop(owned);
    if !policy.is_null() {
        unsafe { &*policy }.0.file_closed();
        drop(unsafe { Box::from_raw(policy) });
    }
    unsafe {
        (*file).pMethods = std::ptr::null();
    }
    if result.is_ok() {
        ffi::SQLITE_OK
    } else {
        ffi::SQLITE_IOERR_CLOSE
    }
}
unsafe extern "C" fn read_file(
    file: *mut ffi::sqlite3_file,
    buffer: *mut std::ffi::c_void,
    amount: i32,
    offset: i64,
) -> i32 {
    if amount < 0 || offset < 0 || offset.checked_add(i64::from(amount)).is_none() {
        return ffi::SQLITE_IOERR_READ;
    }
    if amount == 0 {
        return ffi::SQLITE_OK;
    }
    if buffer.is_null() {
        return ffi::SQLITE_IOERR_READ;
    }
    if unsafe { io_policy(file) }.is_some_and(|policy| !policy.begin_read(amount as u64)) {
        return ffi::SQLITE_IOERR_READ;
    }
    let bytes = unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), amount as usize) };
    let mut at = 0usize;
    while at < bytes.len() {
        match unsafe { owned_file(file) }.read_at(&mut bytes[at..], offset as u64 + at as u64) {
            Ok(0) => {
                bytes[at..].fill(0);
                if unsafe { io_policy(file) }.is_some_and(|policy| !policy.check_operation()) {
                    return ffi::SQLITE_IOERR_READ;
                }
                return ffi::SQLITE_IOERR_SHORT_READ;
            }
            Ok(count) => {
                at += count;
                if unsafe { io_policy(file) }
                    .is_some_and(|policy| !policy.record_read(count as u64))
                {
                    return ffi::SQLITE_IOERR_READ;
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => {
                if let Some(policy) = unsafe { io_policy(file) } {
                    policy.failed_io();
                }
                return ffi::SQLITE_IOERR_READ;
            }
        }
    }
    ffi::SQLITE_OK
}
unsafe extern "C" fn write_file(
    file: *mut ffi::sqlite3_file,
    buffer: *const std::ffi::c_void,
    amount: i32,
    offset: i64,
) -> i32 {
    if unsafe { fd_file(file).readonly } {
        return ffi::SQLITE_READONLY;
    }
    if amount < 0 || offset < 0 || offset.checked_add(i64::from(amount)).is_none() {
        return ffi::SQLITE_IOERR_WRITE;
    }
    if amount == 0 {
        return ffi::SQLITE_OK;
    }
    if buffer.is_null() {
        return ffi::SQLITE_IOERR_WRITE;
    }
    if unsafe { io_policy(file) }.is_some_and(|policy| {
        !policy.before_write(unsafe { owned_file(file) }, offset as u64, amount as u64)
    }) {
        return ffi::SQLITE_IOERR_WRITE;
    }
    let bytes = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), amount as usize) };
    let mut at = 0;
    while at < bytes.len() {
        match unsafe { owned_file(file) }.write_at(&bytes[at..], offset as u64 + at as u64) {
            Ok(0) => return ffi::SQLITE_IOERR_WRITE,
            Ok(count) => {
                at += count;
                let policy = unsafe { io_policy(file) };
                let returned_ok = policy.is_none_or(|policy| policy.record_write(count as u64));
                let mutation_ok =
                    policy.is_none_or(|policy| policy.after_mutation(unsafe { owned_file(file) }));
                if !returned_ok || !mutation_ok {
                    return ffi::SQLITE_IOERR_WRITE;
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => {
                if let Some(policy) = unsafe { io_policy(file) } {
                    policy.failed_io();
                }
                return ffi::SQLITE_IOERR_WRITE;
            }
        }
    }
    ffi::SQLITE_OK
}
unsafe extern "C" fn truncate_file(file: *mut ffi::sqlite3_file, size: i64) -> i32 {
    if unsafe { fd_file(file).readonly } {
        return ffi::SQLITE_READONLY;
    }
    if size < 0 {
        return ffi::SQLITE_IOERR_TRUNCATE;
    }
    let owned = unsafe { owned_file(file) };
    if unsafe { io_policy(file) }.is_some_and(|policy| !policy.before_truncate(owned, size as u64))
    {
        return ffi::SQLITE_IOERR_TRUNCATE;
    }
    if owned.set_len(size as u64).is_ok()
        && !unsafe { io_policy(file) }.is_some_and(|policy| !policy.after_mutation(owned))
    {
        ffi::SQLITE_OK
    } else {
        if let Some(policy) = unsafe { io_policy(file) } {
            policy.failed_io();
        }
        ffi::SQLITE_IOERR_TRUNCATE
    }
}
unsafe extern "C" fn sync_file(file: *mut ffi::sqlite3_file, _: i32) -> i32 {
    let policy = unsafe { io_policy(file) };
    if policy.is_some_and(|policy| !policy.check_operation()) {
        return ffi::SQLITE_IOERR_FSYNC;
    }
    if unsafe { owned_file(file) }.sync_all().is_ok()
        && !policy.is_some_and(|policy| !policy.check_operation())
    {
        ffi::SQLITE_OK
    } else {
        if let Some(policy) = policy {
            policy.failed_io();
        }
        ffi::SQLITE_IOERR_FSYNC
    }
}
unsafe extern "C" fn file_size(file: *mut ffi::sqlite3_file, output: *mut i64) -> i32 {
    if output.is_null() {
        return ffi::SQLITE_IOERR_FSTAT;
    }
    let policy = unsafe { io_policy(file) };
    if policy.is_some_and(|policy| !policy.check_operation()) {
        return ffi::SQLITE_IOERR_FSTAT;
    }
    let Ok(metadata) = unsafe { owned_file(file) }.metadata() else {
        if let Some(policy) = policy {
            policy.failed_io();
        }
        return ffi::SQLITE_IOERR_FSTAT;
    };
    let Ok(size) = i64::try_from(metadata.len()) else {
        return ffi::SQLITE_IOERR_FSTAT;
    };
    unsafe {
        *output = size;
    }
    if policy.is_some_and(|policy| !policy.check_operation()) {
        return ffi::SQLITE_IOERR_FSTAT;
    }
    ffi::SQLITE_OK
}
// SQLite lock bookkeeping is separate from the stronger connection-lifetime
// kernel lease. A SQLite unlock never releases physical custody prematurely.
unsafe extern "C" fn lock_level(file: *mut ffi::sqlite3_file, level: i32) -> i32 {
    let state = unsafe { &mut *file.cast::<FdFile>() };
    if !(ffi::SQLITE_LOCK_SHARED..=ffi::SQLITE_LOCK_EXCLUSIVE).contains(&level)
        || level < state.sqlite_lock
    {
        return ffi::SQLITE_IOERR_LOCK;
    }
    if state.readonly && level > ffi::SQLITE_LOCK_SHARED {
        return ffi::SQLITE_READONLY;
    }
    state.sqlite_lock = level;
    ffi::SQLITE_OK
}
unsafe extern "C" fn unlock_level(file: *mut ffi::sqlite3_file, level: i32) -> i32 {
    let state = unsafe { &mut *file.cast::<FdFile>() };
    if !matches!(level, ffi::SQLITE_LOCK_NONE | ffi::SQLITE_LOCK_SHARED)
        || level > state.sqlite_lock
    {
        return ffi::SQLITE_IOERR_UNLOCK;
    }
    state.sqlite_lock = level;
    ffi::SQLITE_OK
}
unsafe extern "C" fn reserved_lock(file: *mut ffi::sqlite3_file, output: *mut i32) -> i32 {
    if output.is_null() {
        return ffi::SQLITE_IOERR_CHECKRESERVEDLOCK;
    }
    unsafe {
        *output = i32::from(fd_file(file).sqlite_lock >= ffi::SQLITE_LOCK_RESERVED);
    }
    ffi::SQLITE_OK
}
unsafe extern "C" fn file_control(
    _: *mut ffi::sqlite3_file,
    operation: i32,
    argument: *mut std::ffi::c_void,
) -> i32 {
    if operation == ffi::SQLITE_FCNTL_MMAP_SIZE {
        if argument.is_null() {
            return ffi::SQLITE_IOERR;
        }
        // This FD VFS exposes no xFetch/xUnfetch. Explicitly report its zero
        // mmap capability so SQLite policy readback yields the owned scalar.
        // Ignore requested nonzero sizes; mapping is never enabled here.
        unsafe { *argument.cast::<i64>() = 0 };
        return ffi::SQLITE_OK;
    }
    ffi::SQLITE_NOTFOUND
}
unsafe extern "C" fn sector_size(_: *mut ffi::sqlite3_file) -> i32 {
    4096
}
unsafe extern "C" fn device_flags(_: *mut ffi::sqlite3_file) -> i32 {
    0
}
unsafe extern "C" fn no_delete(
    _: *mut ffi::sqlite3_vfs,
    _: *const std::ffi::c_char,
    _: i32,
) -> i32 {
    ffi::SQLITE_IOERR_DELETE
}
unsafe extern "C" fn access_main(
    _: *mut ffi::sqlite3_vfs,
    name: *const std::ffi::c_char,
    _: i32,
    output: *mut i32,
) -> i32 {
    if output.is_null() {
        return ffi::SQLITE_IOERR_ACCESS;
    }
    let exists = if unsafe { is_fd_name(name) } {
        std::str::from_utf8(unsafe { CStr::from_ptr(name) }.to_bytes())
            .ok()
            .and_then(|s| std::fs::metadata(s).ok())
            .is_some()
    } else {
        false
    };
    unsafe {
        *output = i32::from(exists);
    }
    ffi::SQLITE_OK
}

pub(crate) fn current_fs_uid() -> Result<u32> {
    let mut raw = String::new();
    File::open("/proc/self/status")
        .map_err(|error| StoreError::io("cannot inspect current process identity", error))?
        .take(16 * 1024)
        .read_to_string(&mut raw)
        .map_err(|error| StoreError::io("cannot read current process identity", error))?;
    if raw.len() >= 16 * 1024 {
        return Err(StoreError::new(
            StoreErrorCode::UnsupportedPlatform,
            "process identity record exceeds limit",
        ));
    }
    let fsuid = raw
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|values| values.split_whitespace().nth(3))
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::UnsupportedPlatform,
                "process filesystem identity is unavailable",
            )
        })?;
    Ok(fsuid)
}
