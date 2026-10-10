//! Shared Linux descriptor-relative opener for trusted local ToS carriers.
//! This establishes file identity/type only. Digest, source meaning, rights,
//! selected cut, and retention are separately owned by its callers.

#[cfg(not(target_os = "linux"))]
compile_error!("tos-fd-open requires Linux openat2");

use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Component, Path};

use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};
use rustix::io::Errno;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenErrorCode {
    InvalidPath,
    UnsafePath,
    UnsupportedPlatform,
    BudgetExceeded,
    Io,
}

#[derive(Debug)]
pub struct OpenError {
    pub code: OpenErrorCode,
    pub detail: &'static str,
    pub source: Option<io::Error>,
}

impl OpenError {
    fn new(code: OpenErrorCode, detail: &'static str) -> Self {
        Self {
            code,
            detail,
            source: None,
        }
    }
    fn io(detail: &'static str, source: io::Error) -> Self {
        Self {
            code: OpenErrorCode::Io,
            detail,
            source: Some(source),
        }
    }
}

impl fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.detail)
    }
}
impl std::error::Error for OpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

pub type Result<T> = std::result::Result<T, OpenError>;

fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
}
fn resolve_flags() -> ResolveFlags {
    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS
}

fn map_open(error: Errno, detail: &'static str) -> OpenError {
    if error == Errno::NOSYS {
        OpenError::new(
            OpenErrorCode::UnsupportedPlatform,
            "Linux openat2 unavailable",
        )
    } else if matches!(
        error,
        Errno::LOOP | Errno::NOTDIR | Errno::XDEV | Errno::AGAIN | Errno::NXIO
    ) {
        OpenError::new(OpenErrorCode::UnsafePath, detail)
    } else {
        OpenError::io(detail, error.into())
    }
}

fn normal_absolute(path: &Path) -> Result<&Path> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(OpenError::new(
            OpenErrorCode::InvalidPath,
            "expected normalized absolute path",
        ));
    }
    path.strip_prefix("/")
        .map_err(|_| OpenError::new(OpenErrorCode::InvalidPath, "absolute path prefix invalid"))
}

fn normal_leaf(path: &Path) -> Result<&OsStr> {
    let mut parts = path.components();
    match (parts.next(), parts.next()) {
        (Some(Component::Normal(name)), None) => Ok(name),
        _ => Err(OpenError::new(
            OpenErrorCode::InvalidPath,
            "expected one normal path component",
        )),
    }
}

/// Open an absolute directory from the filesystem root without following
/// any symlink component. The returned descriptor anchors later operations.
pub fn open_absolute_directory(path: &Path) -> Result<File> {
    let relative = normal_absolute(path)?;
    let anchor =
        File::open("/").map_err(|error| OpenError::io("cannot open filesystem root", error))?;
    if relative.as_os_str().is_empty() {
        return Ok(anchor);
    }
    openat2(
        &anchor,
        relative,
        directory_flags(),
        Mode::empty(),
        resolve_flags(),
    )
    .map(File::from)
    .map_err(|error| map_open(error, "cannot securely open directory"))
}

/// Open exactly one child directory relative to an already pinned directory.
pub fn open_directory_at(parent: &File, leaf: &Path) -> Result<File> {
    normal_leaf(leaf)?;
    openat2(
        parent,
        leaf,
        directory_flags(),
        Mode::empty(),
        resolve_flags(),
    )
    .map(File::from)
    .map_err(|error| map_open(error, "cannot securely open child directory"))
}

/// Obtain a new open-file description of the same retained directory inode.
/// Unlike `File::try_clone`, independent Linux `flock` locks on this FD
/// conflict with other descriptions in the same process as well as elsewhere.
pub fn reopen_directory(parent: &File) -> Result<File> {
    openat2(
        parent,
        ".",
        directory_flags(),
        Mode::empty(),
        resolve_flags(),
    )
    .map(File::from)
    .map_err(|error| map_open(error, "cannot reopen pinned directory"))
}

/// Open exactly one child without following symlinks. `NONBLOCK` allows a
/// replaced FIFO to be rejected by `fstat` without hanging in `open`.
pub fn open_regular_at(parent: &File, leaf: &Path) -> Result<File> {
    normal_leaf(leaf)?;
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let file: File = openat2(parent, leaf, flags, Mode::empty(), resolve_flags())
        .map(File::from)
        .map_err(|error| map_open(error, "cannot securely open regular file"))?;
    if !file
        .metadata()
        .map_err(|error| OpenError::io("cannot stat opened file", error))?
        .is_file()
    {
        return Err(OpenError::new(
            OpenErrorCode::UnsafePath,
            "opened file is not regular",
        ));
    }
    Ok(file)
}

/// Convenience for a selected absolute file. The cap is checked on the
/// opened inode, after the same no-follow traversal and regular-file check.
pub fn open_absolute_regular(path: &Path, max_size_bytes: u64) -> Result<File> {
    normal_absolute(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| OpenError::new(OpenErrorCode::InvalidPath, "file parent absent"))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| OpenError::new(OpenErrorCode::InvalidPath, "file leaf absent"))?;
    let directory = open_absolute_directory(parent)?;
    let file = open_regular_at(&directory, Path::new(leaf))?;
    if file
        .metadata()
        .map_err(|error| OpenError::io("cannot stat selected file", error))?
        .len()
        > max_size_bytes
    {
        return Err(OpenError::new(
            OpenErrorCode::BudgetExceeded,
            "opened file exceeds caller cap",
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::fs::{Mode, mkfifoat};
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn pinned_open_survives_path_replacement_and_refuses_symlink_fifo() {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root =
            std::env::temp_dir().join(format!("tos-fd-open-{}", u128::from_le_bytes(random)));
        fs::create_dir(&root).unwrap();
        let store = root.join("original");
        let moved = root.join("moved");
        fs::create_dir(&store).unwrap();
        fs::write(store.join("object"), b"exact").unwrap();
        let pinned = open_absolute_directory(&store).unwrap();
        fs::rename(&store, &moved).unwrap();
        symlink(&moved, &store).unwrap();
        let mut file = open_regular_at(&pinned, Path::new("object")).unwrap();
        let mut data = Vec::new();
        use std::io::Read;
        file.read_to_end(&mut data).unwrap();
        assert_eq!(data, b"exact");
        assert_eq!(
            open_absolute_directory(&store).unwrap_err().code,
            OpenErrorCode::UnsafePath
        );
        let clone = reopen_directory(&pinned).unwrap();
        assert_eq!(
            open_regular_at(&clone, Path::new("../object"))
                .unwrap_err()
                .code,
            OpenErrorCode::InvalidPath
        );
        symlink(moved.join("object"), moved.join("alias")).unwrap();
        assert_eq!(
            open_regular_at(&pinned, Path::new("alias"))
                .unwrap_err()
                .code,
            OpenErrorCode::UnsafePath
        );
        mkfifoat(&pinned, "pipe", Mode::RUSR | Mode::WUSR).unwrap();
        assert_eq!(
            open_regular_at(&pinned, Path::new("pipe"))
                .unwrap_err()
                .code,
            OpenErrorCode::UnsafePath
        );
        fs::remove_file(&store).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}
