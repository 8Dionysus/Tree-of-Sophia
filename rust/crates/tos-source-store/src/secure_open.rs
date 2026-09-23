//! Linux descriptor-relative, no-follow opens for the read-only v1 carrier.
//!
//! `openat2` requires Linux 5.6 or newer. We fail closed on an older kernel;
//! no path-based fallback silently weakens traversal guarantees. Mounts and
//! the contents of an already opened regular file remain owner-controlled.

#[cfg(not(target_os = "linux"))]
compile_error!("tos-source-store currently supports Linux openat2 only");

use std::fs::File;
use std::io;
use std::path::{Component, Path};

use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};
use rustix::io::Errno;

use crate::error::{Result, StoreError, StoreErrorCode};

#[derive(Debug)]
pub(crate) struct StoreRoot {
    root: File,
    revisions: File,
    objects: File,
}

impl StoreRoot {
    pub(crate) fn open_existing(path: &Path) -> Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        {
            return Err(StoreError::new(
                StoreErrorCode::InvalidRoot,
                "corpus root must be an absolute normalized path",
            ));
        }
        let relative = path.strip_prefix("/").map_err(|_| {
            StoreError::new(StoreErrorCode::InvalidRoot, "corpus root is not absolute")
        })?;
        if relative.as_os_str().is_empty() {
            return Err(StoreError::new(
                StoreErrorCode::InvalidRoot,
                "filesystem root is not a corpus root",
            ));
        }
        let anchor = File::open("/")
            .map_err(|error| StoreError::io("cannot open filesystem root", error))?;
        let root = openat2(
            &anchor,
            relative,
            directory_flags(),
            Mode::empty(),
            resolve_flags(),
        )
        .map(File::from)
        .map_err(|error| {
            map_open_error(
                error,
                StoreErrorCode::InvalidRoot,
                "cannot securely open corpus root",
            )
        })?;
        let revisions = open_directory(&root, "revisions")?;
        let objects = open_directory(&root, "objects")?;
        Ok(Self {
            root,
            revisions,
            objects,
        })
    }

    pub(crate) fn open_revision(&self, revision_hex: &str) -> Result<File> {
        open_directory(&self.revisions, revision_hex)
    }

    pub(crate) fn open_pointer(&self) -> Result<File> {
        open_regular(&self.root, "current.json")
    }

    pub(crate) fn open_manifest(&self, revision: &File) -> Result<File> {
        open_regular(revision, "snapshot.json")
    }

    pub(crate) fn open_object(&self, digest_hex: &str) -> Result<File> {
        open_regular(&self.objects, digest_hex)
    }
}

fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
}

fn resolve_flags() -> ResolveFlags {
    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS
}

fn open_directory(parent: &File, name: &str) -> Result<File> {
    openat2(
        parent,
        name,
        directory_flags(),
        Mode::empty(),
        resolve_flags(),
    )
    .map(File::from)
    .map_err(|error| {
        map_open_error(
            error,
            StoreErrorCode::UnsafePath,
            "cannot securely open corpus directory",
        )
    })
}

fn open_regular(parent: &File, name: &str) -> Result<File> {
    // NONBLOCK prevents a FIFO swapped into place from hanging before fstat.
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let file: File = openat2(parent, name, flags, Mode::empty(), resolve_flags())
        .map(File::from)
        .map_err(|error| {
            map_open_error(
                error,
                StoreErrorCode::UnsafePath,
                "cannot securely open corpus file",
            )
        })?;
    if !file
        .metadata()
        .map_err(|error| StoreError::io("cannot stat opened corpus file", error))?
        .is_file()
    {
        return Err(StoreError::new(
            StoreErrorCode::UnsafePath,
            "opened corpus file is not regular",
        ));
    }
    Ok(file)
}

fn map_open_error(error: Errno, unsafe_code: StoreErrorCode, detail: &'static str) -> StoreError {
    if error == Errno::NOSYS {
        StoreError::new(
            StoreErrorCode::UnsupportedPlatform,
            "Linux openat2 is unavailable",
        )
    } else if matches!(
        error,
        Errno::LOOP | Errno::XDEV | Errno::NOTDIR | Errno::AGAIN
    ) {
        StoreError::new(unsafe_code, detail)
    } else {
        StoreError::io(detail, io::Error::from(error))
    }
}
