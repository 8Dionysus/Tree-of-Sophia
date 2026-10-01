//! Exact-inode SQLite access for private derived indexes and pinned immutable models.
//!
//! SQLite's Unix pathname canonicalizer resolves `/proc/self/fd/N` symlinks.
//! For O_TMPFILE this becomes a different named `(deleted)` file. This narrow
//! VFS retains the descriptor spelling, permits only the main database, and
//! never creates filesystem objects or sidecars.

use crate::{Result, StoreError, StoreErrorCode};
use rusqlite::{Connection, OpenFlags, ffi};
use std::{
    ffi::CStr,
    fs::{File, OpenOptions, TryLockError},
    io::{ErrorKind, Read},
    ops::{Deref, DerefMut},
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::fs::{FileExt, MetadataExt},
    },
    sync::OnceLock,
};

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
    /// Open only a fresh private unnamed inode. Scratch ceilings and statement
    /// budgets remain with the existing derived-index caller.
    pub fn open_private_derived(file: &File) -> Result<Self> {
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
        let db = Self::open(file, false)?;
        // These precede the first schema/data write. The VFS independently
        // refuses journal/WAL/temp filenames even if a caller changes pragmas.
        db.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=MEMORY; PRAGMA mmap_size=0;")
            .map_err(|_| invalid("private SQLite settings could not be enforced"))?;
        Ok(db)
    }

    /// Reopen the exact retained inode read-only, without pathname resolution.
    /// Authenticity, schema, verity and selected-use authority stay with caller.
    pub fn open_readonly_immutable(file: &File) -> Result<Self> {
        Self::open(file, true)
    }

    /// Preserve SQLite's explicit close result and keep the pinned descriptor
    /// with the still-live connection if close refuses (for example, BUSY).
    pub fn close(self) -> std::result::Result<(), (Self, rusqlite::Error)> {
        let Self { db, _file } = self;
        match db.close() {
            Ok(()) => {
                drop(_file);
                Ok(())
            }
            Err((db, error)) => Err((Self { db, _file }, error)),
        }
    }

    fn open(file: &File, readonly: bool) -> Result<Self> {
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
        let path = format!("/proc/self/fd/{}", owned.as_raw_fd());
        let (name, flags) = if readonly {
            (
                format!("file:{path}?mode=ro&immutable=1"),
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_URI
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE,
            )
        } else {
            (
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE,
            )
        };
        let db = Connection::open_with_flags_and_vfs(
            name,
            flags,
            VFS_NAME.to_str().expect("static ASCII VFS name"),
        )
        .map_err(|_| invalid("exact pinned SQLite main database open failed"))?;
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
        Ok(Self { db, _file: owned })
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
    let path = format!("/proc/self/fd/{fd}");
    // Standard open follows this retained kernel FD directly (no realpath and
    // no O_NOFOLLOW). No CREATE: it cannot instantiate the '(deleted)' name.
    let actual = match OpenOptions::new().read(true).write(!readonly).open(path) {
        Ok(file) => file,
        Err(_) => return ffi::SQLITE_CANTOPEN,
    };
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
    // Independent OFD, real kernel lease, held for the entire connection.
    // Private writers are exclusive; immutable selected readers share a lease.
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
    unsafe {
        file.cast::<FdFile>().write(FdFile {
            base: ffi::sqlite3_file { pMethods: &METHODS },
            owned: Box::into_raw(Box::new(actual)),
            readonly,
            sqlite_lock: ffi::SQLITE_LOCK_NONE,
        });
    }
    if !output_flags.is_null() {
        unsafe {
            *output_flags = flags;
        }
    }
    ffi::SQLITE_OK
}
unsafe extern "C" fn close_file(file: *mut ffi::sqlite3_file) -> i32 {
    let pointer = unsafe { fd_file(file).owned };
    let owned = unsafe { Box::from_raw(pointer) };
    // Explicit unlock prevents a fork-before-exec inherited OFD extending the
    // connection's lease after SQLite has made its close decision.
    let result = owned.unlock();
    drop(owned);
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
    let bytes = unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), amount as usize) };
    let mut at = 0usize;
    while at < bytes.len() {
        match unsafe { owned_file(file) }.read_at(&mut bytes[at..], offset as u64 + at as u64) {
            Ok(0) => {
                bytes[at..].fill(0);
                return ffi::SQLITE_IOERR_SHORT_READ;
            }
            Ok(count) => at += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return ffi::SQLITE_IOERR_READ,
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
    let bytes = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), amount as usize) };
    let mut at = 0;
    while at < bytes.len() {
        match unsafe { owned_file(file) }.write_at(&bytes[at..], offset as u64 + at as u64) {
            Ok(0) => return ffi::SQLITE_IOERR_WRITE,
            Ok(count) => at += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return ffi::SQLITE_IOERR_WRITE,
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
    if unsafe { owned_file(file) }.set_len(size as u64).is_ok() {
        ffi::SQLITE_OK
    } else {
        ffi::SQLITE_IOERR_TRUNCATE
    }
}
unsafe extern "C" fn sync_file(file: *mut ffi::sqlite3_file, _: i32) -> i32 {
    if unsafe { owned_file(file) }.sync_all().is_ok() {
        ffi::SQLITE_OK
    } else {
        ffi::SQLITE_IOERR_FSYNC
    }
}
unsafe extern "C" fn file_size(file: *mut ffi::sqlite3_file, output: *mut i64) -> i32 {
    if output.is_null() {
        return ffi::SQLITE_IOERR_FSTAT;
    }
    let Ok(metadata) = unsafe { owned_file(file) }.metadata() else {
        return ffi::SQLITE_IOERR_FSTAT;
    };
    let Ok(size) = i64::try_from(metadata.len()) else {
        return ffi::SQLITE_IOERR_FSTAT;
    };
    unsafe {
        *output = size;
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
    _: i32,
    _: *mut std::ffi::c_void,
) -> i32 {
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
