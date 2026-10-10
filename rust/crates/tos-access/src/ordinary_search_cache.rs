//! Native-owned ordinary Reference v3 cache. Path authority is issued before
//! confinement; content always hydrates from the same normalized source model.
use std::{
    ffi::{CString, OsStr},
    fs::File,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tos_compiler::{
    ControlledKnowledgeModel, ControlledSidecarModel, Error, Result, SearchSidecarAdmissionError,
    private_tmpfs_stage::PrivateTmpfsStageIsolation,
};
use tos_source_store::{
    PinnedSqliteConnection, PinnedSqliteDerivedWriteGuard, PinnedSqliteIoBudget,
    PinnedSqliteSpaceBudget, StoreError, StoreErrorCode,
};

pub(crate) struct OrdinarySearchCacheSelection<'a> {
    pub path: &'a Path,
    pub graph_schema: &'a str,
    pub max_bytes: u64,
    pub max_postings: u64,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct FileState {
    dev: u64,
    ino: u64,
    size: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
fn state(file: &File) -> Result<FileState> {
    let m = file.metadata()?;
    if !m.is_file() {
        return Err(Error::Invalid("ordinary search cache inode type"));
    }
    Ok(FileState {
        dev: m.dev(),
        ino: m.ino(),
        size: m.len(),
        mtime: m.mtime(),
        mtime_ns: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_ns: m.ctime_nsec(),
    })
}
fn store(error: StoreError) -> Error {
    match error.code {
        StoreErrorCode::BudgetExceeded => {
            Error::Budget("ordinary search cache original resource budget")
        }
        StoreErrorCode::Io => error
            .source
            .map_or(Error::Invalid("ordinary search cache VFS I/O"), Error::Io),
        _ => Error::Invalid("ordinary search cache pinned VFS refused"),
    }
}
fn original(io: &PinnedSqliteIoBudget, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
        return Err(Error::Budget(
            "ordinary search cache original cutoff/cancellation",
        ));
    }
    if io.snapshot().failure.is_some() {
        return Err(Error::Budget("ordinary search cache original I/O failure"));
    }
    Ok(())
}
fn open_leaf(parent: &File, name: &CString) -> Result<Option<File>> {
    let raw = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if raw < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error.into());
    }
    let file = unsafe { File::from_raw_fd(raw) };
    let m = file.metadata()?;
    if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
        return Err(Error::Invalid("ordinary search cache leaf custody"));
    }
    Ok(Some(file))
}
fn named_matches(parent: &File, name: &CString, file: &File, expected: FileState) -> Result<()> {
    if state(file)? != expected {
        return Err(Error::Invalid("ordinary search cache held inode mutated"));
    }
    let named =
        open_leaf(parent, name)?.ok_or(Error::Invalid("ordinary search cache leaf disappeared"))?;
    if state(&named)? != expected {
        return Err(Error::Invalid("ordinary search cache pathname rebound"));
    }
    Ok(())
}
fn initial_matches(parent: &File, name: &CString, initial: Option<FileState>) -> Result<()> {
    let now = open_leaf(parent, name)?;
    match (now, initial) {
        (None, None) => Ok(()),
        (Some(file), Some(expected)) if state(&file)? == expected => Ok(()),
        _ => Err(Error::Invalid(
            "ordinary search cache changed during rebuild",
        )),
    }
}
struct BudgetedBuildInode {
    file: File,
    reservation: PinnedSqliteDerivedWriteGuard,
}
struct NamedTemp<'a> {
    parent: &'a File,
    name: CString,
    linked: bool,
}
impl NamedTemp<'_> {
    fn discard(&mut self) -> Result<()> {
        if self.linked {
            if unsafe { libc::unlinkat(self.parent.as_raw_fd(), self.name.as_ptr(), 0) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            self.linked = false;
        }
        Ok(())
    }
}
impl Drop for NamedTemp<'_> {
    fn drop(&mut self) {
        if self.linked {
            unsafe { libc::unlinkat(self.parent.as_raw_fd(), self.name.as_ptr(), 0) };
        }
    }
}
fn close(db: PinnedSqliteConnection) -> Result<()> {
    db.close().map_err(|(db, error)| {
        drop(db);
        Error::from(error)
    })
}

/// Borrow authentic aggregate IO/space owners supplied once by native operation
/// admission. Neither this function nor a cache open constructs fresh counters.
/// `check_slot` is the source-owner guard for exact leaf/temp names and inode
/// aliases; containing source files in this directory is a valid Reference case.
pub(crate) fn with_ordinary_search_cache(
    source: &mut ControlledKnowledgeModel<'_, '_, '_>,
    isolation: &PrivateTmpfsStageIsolation,
    selected: OrdinarySearchCacheSelection<'_>,
    io: &PinnedSqliteIoBudget,
    space: &PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    check_source: &dyn Fn() -> Result<()>,
    check_slot: &dyn Fn(&OsStr, Option<&File>) -> Result<()>,
    consume: impl FnOnce(&mut ControlledSidecarModel<'_, '_, '_, '_>) -> Result<()>,
) -> Result<()> {
    source.verify_search_cache_operation(deadline, cancelled)?;
    original(io, deadline, cancelled)?;
    check_source()?;
    let (issued_build_bytes, issued_temp_bytes) = isolation
        .search_cache_limits(selected.path)
        .map_err(|_| Error::Invalid("ordinary search cache ticket profile absent"))?;
    let build_bytes = selected.max_bytes.min(issued_build_bytes);
    let leaf = selected
        .path
        .file_name()
        .ok_or(Error::Invalid("ordinary cache leaf absent"))?;
    if leaf.as_bytes().is_empty() || leaf.as_bytes().len() > 255 || leaf.as_bytes().contains(&0) {
        return Err(Error::Invalid("ordinary cache leaf shape"));
    }

    check_slot(leaf, None)?;
    let workspace = PinnedSqliteConnection::budgeted_derived_rust_state_upper_bound()
        .map_err(store)?
        .checked_add(PinnedSqliteConnection::immutable_retained_rust_state_upper_bound())
        .and_then(|n| {
            n.checked_add(PinnedSqliteConnection::immutable_open_rust_workspace_upper_bound())
        })
        .and_then(|n| n.checked_add(4 * 255 + 8192 + 32 * std::mem::size_of::<std::fs::Metadata>()))
        .and_then(|n| {
            n.checked_add(
                tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_VERIFY_COST.workspace_bytes,
            )
        })
        .ok_or(Error::Budget("ordinary cache workspace forecast"))?;
    source.with_owned_search_cache_workspace(workspace, |source, charge_work| {
        let verify_cost = tos_compiler::private_tmpfs_stage::PRIVATE_TMPFS_VERIFY_COST;
        charge_work(verify_cost.read_bytes as usize)?;
        io.charge_read_upper_bound(verify_cost.read_bytes)
            .map_err(store)?;
        let parent =
            isolation.search_cache_custody(selected.path, issued_build_bytes, issued_temp_bytes)?;
        source
            .charge_query_work(leaf.as_bytes().len() + std::mem::size_of::<std::fs::Metadata>())?;
        let name = CString::new(leaf.as_bytes())
            .map_err(|_| Error::Invalid("ordinary cache leaf encoding"))?;
        let guard = || -> Result<()> {
            original(io, deadline, cancelled)?;
            check_source()?;
            charge_work(
                verify_cost.read_bytes as usize + 2 * std::mem::size_of::<std::fs::Metadata>(),
            )?;
            io.charge_read_upper_bound(
                verify_cost.read_bytes + 2 * std::mem::size_of::<std::fs::Metadata>() as u64,
            )
            .map_err(store)?;
            isolation.search_cache_custody(selected.path, issued_build_bytes, issued_temp_bytes)?;
            Ok(())
        };
        // The immutable opener has no file-size/build quota check. A healthy
        // existing carrier can be larger than the selected new-build budget.
        guard()?;
        let existing = open_leaf(parent, &name)?;
        let initial = existing.as_ref().map(state).transpose()?;
        let mut consume = Some(consume);
        if let Some(file) = existing.as_ref() {
            check_slot(leaf, Some(file))?;
            let before = initial.ok_or(Error::Invalid("ordinary cache initial state absent"))?;
            let cache_guard = || -> Result<()> {
                guard()?;
                named_matches(parent, &name, file, before)?;
                check_slot(leaf, Some(file))
            };
            cache_guard()?;
            let opened = PinnedSqliteConnection::open_readonly_immutable_budgeted(
                file,
                io.clone(),
                deadline,
                cancelled.clone(),
            );
            original(io, deadline, cancelled)?;
            match opened {
                Ok(db) => {
                    let admission =
                        source.admit_search_sidecar(&db, selected.graph_schema, &cache_guard);
                    match admission {
                        Ok(()) => {
                            let result = source.with_search_sidecar(
                                &db,
                                selected.graph_schema,
                                &cache_guard,
                                consume.take().ok_or(Error::Invalid(
                                    "ordinary cache callback already consumed",
                                ))?,
                            );
                            let closing = close(db);
                            cache_guard()?;
                            result?;
                            closing?;
                            return Ok(());
                        }
                        Err(SearchSidecarAdmissionError::RebuildRequired) => {
                            close(db)?;
                            cache_guard()?;
                        }
                        Err(SearchSidecarAdmissionError::Operation(error)) => {
                            let closing = close(db);
                            return match closing {
                                Ok(()) => Err(error),
                                Err(close) => Err(close),
                            };
                        }
                    }
                }
                Err(error) if error.code == StoreErrorCode::CorruptSelectedObject => {
                    cache_guard()?;
                }
                Err(error) => return Err(store(error)),
            }
        }
        // Rebuild only after cache admission. All currentness/operation errors
        // above fail without publishing or choosing another search route.
        guard()?;
        initial_matches(parent, &name, initial)?;
        let raw = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                c".".as_ptr(),
                libc::O_TMPFILE | libc::O_RDWR | libc::O_CLOEXEC,
                0o600,
            )
        };
        if raw < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let fresh = unsafe { File::from_raw_fd(raw) };
        check_slot(leaf, Some(&fresh))?;
        let (db, reservation) = PinnedSqliteConnection::open_private_derived_budgeted_unconfigured(
            &fresh,
            io.clone(),
            space.clone(),
            build_bytes,
            issued_temp_bytes,
            deadline,
            cancelled.clone(),
        )
        .map_err(store)?;
        let build = BudgetedBuildInode {
            file: fresh,
            reservation,
        };
        let fresh = &build.file;
        let fresh_guard = || -> Result<()> {
            guard()?;
            initial_matches(parent, &name, initial)?;
            check_slot(leaf, Some(&fresh))
        };
        let built = source.build_search_sidecar(
            &db,
            selected.graph_schema,
            build_bytes,
            selected.max_postings,
            &fresh_guard,
        );
        let closing = close(db);
        built?;
        closing?;
        fresh_guard()?;
        // A genuine close and fsync precede named publication. The reservation
        // guard keeps original disk allocation admitted through rename/fsync.
        fresh.sync_all()?;
        fresh_guard()?;
        let mut stamp: libc::timespec = unsafe { std::mem::zeroed() };
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut stamp) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        charge_work(leaf.as_bytes().len() + 512)?;
        let temp_bytes = format!(
            ".{}.{}-{}-{}.tmp",
            leaf.to_string_lossy(),
            std::process::id(),
            stamp.tv_sec,
            stamp.tv_nsec
        );
        if temp_bytes.as_bytes().len() > 255 {
            return Err(Error::Invalid(
                "ordinary cache temp leaf exceeds filesystem name cap",
            ));
        }
        let temp_name = CString::new(temp_bytes.as_bytes())
            .map_err(|_| Error::Invalid("ordinary cache temp encoding"))?;
        check_slot(OsStr::from_bytes(temp_name.as_bytes()), None)?;
        let mut temp = NamedTemp {
            parent,
            name: temp_name,
            linked: false,
        };
        let spelling = CString::new(format!("/proc/self/fd/{}", fresh.as_raw_fd()))
            .map_err(|_| Error::Invalid("ordinary cache fd spelling"))?;
        if unsafe {
            libc::linkat(
                libc::AT_FDCWD,
                spelling.as_ptr(),
                parent.as_raw_fd(),
                temp.name.as_ptr(),
                libc::AT_SYMLINK_FOLLOW,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        temp.linked = true;
        let publishing = (|| -> Result<()> {
            fresh_guard()?;
            check_slot(OsStr::from_bytes(temp.name.as_bytes()), Some(&fresh))?;
            // Only these two exact declared names may change; parent directory may
            // legitimately also contain source inputs. Source-owner slot admission
            // accounts for their directory-stamp effects without admitting sources.
            if unsafe {
                libc::renameat(
                    parent.as_raw_fd(),
                    temp.name.as_ptr(),
                    parent.as_raw_fd(),
                    name.as_ptr(),
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            temp.linked = false;
            Ok(())
        })();
        if let Err(error) = publishing {
            temp.discard()?;
            return Err(error);
        }
        let directory_raw = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if directory_raw < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let directory = unsafe { File::from_raw_fd(directory_raw) };
        directory.sync_all()?;
        guard()?;
        let published = state(&fresh)?;
        let cache_guard = || -> Result<()> {
            guard()?;
            named_matches(parent, &name, &fresh, published)?;
            check_slot(leaf, Some(&fresh))
        };
        cache_guard()?;
        let db = PinnedSqliteConnection::open_readonly_immutable_budgeted(
            &fresh,
            io.clone(),
            deadline,
            cancelled.clone(),
        )
        .map_err(store)?;
        let result = source.with_search_sidecar(
            &db,
            selected.graph_schema,
            &cache_guard,
            consume
                .take()
                .ok_or(Error::Invalid("ordinary cache callback already consumed"))?,
        );
        let closing = close(db);
        cache_guard()?;
        result?;
        closing?;
        drop(directory);
        drop(temp);
        drop(build);
        Ok(())
    })
}
