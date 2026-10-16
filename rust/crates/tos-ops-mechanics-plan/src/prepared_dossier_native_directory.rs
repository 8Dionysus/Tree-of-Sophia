//! Native selected-root streaming inventory. Draft; no type/runtime claim.
use crate::prepared_dossier_readiness::{
    PreparedDossierDirectoryEntry, PreparedDossierDirectoryStatus, PreparedDossierEntryKind,
};
use std::ffi::{CStr, OsString};
use std::fs::File;
use std::os::fd::{AsRawFd, IntoRawFd};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};
use tos_compiler::research_execution::ResearchExecution;

struct DirectoryStream(*mut libc::DIR);
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

fn identity(file: &File) -> Result<(u64, u64, u32, u64, i64, i64, i64, i64), String> {
    let m = file.metadata().map_err(|e| e.to_string())?;
    Ok((
        m.dev(),
        m.ino(),
        m.mode(),
        m.size(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}

/// `selected` is the DOCX root already selected under the same original
/// ResearchExecution deadline, cancellation, IO and work ledgers. Caller
/// keeps that selected root descriptor alive for the complete operation.
pub fn visit_selected_directory(
    selected: &ResearchExecution,
    path: &Path,
    visit: &mut dyn FnMut(PreparedDossierDirectoryEntry) -> Result<(), String>,
) -> Result<PreparedDossierDirectoryStatus, String> {
    selected.check()?;
    let relative = path
        .strip_prefix(selected.root())
        .map_err(|_| "inventory path outside selected DOCX root")?;
    let mut directory =
        tos_fd_open::reopen_directory(selected.root_directory()).map_err(|e| e.to_string())?;
    let mut depth = 0u64;
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err("inventory path must contain only normal components".into());
        };
        depth = depth.checked_add(1).ok_or("inventory depth overflow")?;
        if depth > 64 {
            return Err("inventory directory depth exceeds native path profile".into());
        }
        selected.tick(1)?;
        directory = match tos_fd_open::open_directory_at(&directory, Path::new(name)) {
            Ok(next) => next,
            Err(error)
                if error.source.as_ref().and_then(|e| e.raw_os_error()) == Some(libc::ENOENT) =>
            {
                return Ok(PreparedDossierDirectoryStatus::Missing);
            }
            Err(error) => return Err(error.to_string()),
        };
    }
    let before = identity(&directory)?;
    let stream_file = tos_fd_open::reopen_directory(&directory).map_err(|e| e.to_string())?;
    let fd = stream_file.into_raw_fd();
    let raw = unsafe { libc::fdopendir(fd) };
    if raw.is_null() {
        let error = std::io::Error::last_os_error();
        unsafe {
            libc::close(fd);
        }
        return Err(error.to_string());
    }
    let stream = DirectoryStream(raw);
    loop {
        selected.check()?;
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let error = unsafe { *libc::__errno_location() };
            if error == libc::EINTR {
                continue;
            }
            if error != 0 {
                return Err(std::io::Error::from_raw_os_error(error).to_string());
            }
            break;
        }
        selected.tick(1)?;
        // Copy this one entry before the next readdir, which may overwrite it.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }
        let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                metadata.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mode = unsafe { metadata.assume_init() }.st_mode & libc::S_IFMT;
        let kind = match mode {
            libc::S_IFREG => PreparedDossierEntryKind::RegularFile,
            libc::S_IFDIR => PreparedDossierEntryKind::Directory,
            libc::S_IFLNK => PreparedDossierEntryKind::Symlink,
            _ => PreparedDossierEntryKind::Special,
        };
        visit(PreparedDossierDirectoryEntry {
            name: OsString::from_vec(name.to_bytes().to_vec()),
            kind,
        })?;
    }
    selected.check()?;
    if identity(&directory)? != before {
        return Err("selected inventory directory changed during enumeration".into());
    }
    // A changed pathname must not silently turn this retained inode into a
    // different source route. Re-open every child through the selected root.
    let mut current =
        tos_fd_open::reopen_directory(selected.root_directory()).map_err(|e| e.to_string())?;
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err("inventory path changed".into());
        };
        selected.tick(1)?;
        current =
            tos_fd_open::open_directory_at(&current, Path::new(name)).map_err(|e| e.to_string())?;
    }
    if identity(&current)? != before {
        return Err("selected inventory route changed during enumeration".into());
    }
    selected.tick(1)?;
    let root_now =
        tos_fd_open::open_absolute_directory(selected.root()).map_err(|e| e.to_string())?;
    let root_meta = root_now.metadata().map_err(|e| e.to_string())?;
    if (root_meta.dev(), root_meta.ino()) != selected.root_identity()? {
        return Err("selected DOCX root pathname changed during enumeration".into());
    }
    selected.check()?;
    Ok(PreparedDossierDirectoryStatus::Present)
}
