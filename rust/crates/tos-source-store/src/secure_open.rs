//! Corpus-specific directory layout over the shared Linux FD opener.
//! The trusted root and opened inodes remain owner controlled.

use std::fs::File;
use std::path::{Component, Path};

use tos_fd_open::{OpenError, OpenErrorCode};

use crate::error::{Result, StoreError, StoreErrorCode};

#[derive(Debug)]
pub(crate) struct StoreRoot {
    root: File,
    revisions: File,
    objects: File,
}

impl StoreRoot {
    pub(crate) fn open_existing(path: &Path) -> Result<Self> {
        if path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, Component::RootDir))
        {
            return Err(StoreError::new(
                StoreErrorCode::InvalidRoot,
                "filesystem root is not a corpus root",
            ));
        }
        let root = tos_fd_open::open_absolute_directory(path).map_err(|error| {
            map_open(
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

    pub(crate) fn open_existing_at(held_root: &File) -> Result<Self> {
        let root = held_root
            .try_clone()
            .map_err(|error| StoreError::io("cannot clone held corpus root", error))?;
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

fn open_directory(parent: &File, name: &str) -> Result<File> {
    tos_fd_open::open_directory_at(parent, Path::new(name)).map_err(|error| {
        map_open(
            error,
            StoreErrorCode::UnsafePath,
            "cannot securely open corpus directory",
        )
    })
}

fn open_regular(parent: &File, name: &str) -> Result<File> {
    tos_fd_open::open_regular_at(parent, Path::new(name)).map_err(|error| {
        map_open(
            error,
            StoreErrorCode::UnsafePath,
            "cannot securely open corpus file",
        )
    })
}

pub(crate) fn map_open(
    error: OpenError,
    unsafe_code: StoreErrorCode,
    detail: &'static str,
) -> StoreError {
    match error.code {
        OpenErrorCode::InvalidPath | OpenErrorCode::UnsafePath => {
            StoreError::new(unsafe_code, detail)
        }
        OpenErrorCode::UnsupportedPlatform => StoreError::new(
            StoreErrorCode::UnsupportedPlatform,
            "Linux openat2 unavailable",
        ),
        OpenErrorCode::BudgetExceeded => StoreError::new(StoreErrorCode::BudgetExceeded, detail),
        OpenErrorCode::Io => match error.source {
            Some(source) => StoreError::io(detail, source),
            None => StoreError::new(StoreErrorCode::Io, detail),
        },
    }
}
