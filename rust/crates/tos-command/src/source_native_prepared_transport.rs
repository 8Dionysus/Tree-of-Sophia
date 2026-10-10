//! Exact protected prepared transport shared by the two fixed source publishers.
//! Family selection, grants and transactions remain with their typed callers.
use super::{absolute, capped, digest, exact, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_text_owner::read_absolute;
use serde_json::Value;
use std::{
    fs::{self, File},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::Digest256;
const META: usize = 1_048_576;
pub(super) fn failure(_: impl std::fmt::Debug) -> SourceCommandError {
    SourceCommandError::Conflict("Agent prepared publication transport")
}
pub(super) fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Acquire) {
        return Err(SourceCommandError::Denied(
            "Agent operation cancelled or expired",
        ));
    }
    Ok(())
}
pub(super) fn protected(
    path: &Path,
    root: &Path,
    uid: u32,
    cap: u64,
) -> SourceCommandResult<fs::Metadata> {
    if !path.is_absolute() || !path.starts_with(root) || path.starts_with(root.join("ToS")) {
        return Err(SourceCommandError::Denied("Agent prepared companion scope"));
    }
    let parent = path
        .parent()
        .ok_or(SourceCommandError::Invalid("Agent companion parent"))?;
    let directory = tos_fd_open::open_absolute_directory(parent).map_err(failure)?;
    let dm = directory.metadata().map_err(failure)?;
    if dm.uid() != uid || dm.mode() & 0o077 != 0 {
        return Err(SourceCommandError::Denied("Agent companion private parent"));
    }
    let m = fs::symlink_metadata(path).map_err(failure)?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.uid() != uid
        || m.mode() & 0o777 != 0o600
        || m.nlink() != 1
        || m.len() > cap
    {
        return Err(SourceCommandError::Denied(
            "Agent protected existing companion",
        ));
    }
    Ok(m)
}
pub(super) fn companion(
    inv: &Value,
    key: &str,
    hash: Option<&str>,
    root: &Path,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = absolute(text(inv, key)?)?;
    protected(&path, root, uid, META as u64)?;
    let raw = read_absolute(&path, uid, true, META, deadline, cancelled)?;
    if let Some(field) = hash {
        if Digest256::of_bytes(&raw) != digest(text(inv, field)?)? {
            return Err(SourceCommandError::Conflict("Agent companion digest"));
        }
    }
    Ok(raw)
}
pub(super) fn typed(v: &Value) -> SourceCommandResult<tos_foundation::JsonValue> {
    cmd::parse(&serde_json::to_vec(v).map_err(failure)?)
}
// Existing limit structures keep their complete exact grammar. Every supplied
// number is bounded by this transport's fixed finite profile, never a grant.
pub(super) fn bounded_limits<T: serde::de::DeserializeOwned + serde::Serialize + Default>(
    v: &Value,
) -> SourceCommandResult<T> {
    let ceiling = serde_json::to_value(T::default()).map_err(failure)?;
    let keys = ceiling
        .as_object()
        .ok_or(SourceCommandError::Invalid("Agent limit profile"))?
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    exact(v, &keys)?;
    for key in keys {
        capped(
            v,
            key,
            ceiling[key]
                .as_u64()
                .ok_or(SourceCommandError::Invalid("Agent limit ceiling"))?,
        )?;
    }
    serde_json::from_value(v.clone()).map_err(failure)
}
pub(super) struct DatabaseFence {
    path: PathBuf,
    root: PathBuf,
    uid: u32,
    maximum: u64,
    anchor: File,
    parent: File,
    sides: Vec<(PathBuf, File)>,
}
impl DatabaseFence {
    pub(super) fn open(
        path: PathBuf,
        root: &Path,
        uid: u32,
        maximum: u64,
        read_only: bool,
    ) -> SourceCommandResult<(Self, rusqlite::Connection)> {
        let m = protected(&path, root, uid, maximum)?;
        let parent = tos_fd_open::open_absolute_directory(
            path.parent()
                .ok_or(SourceCommandError::Invalid("Agent DB parent"))?,
        )
        .map_err(failure)?;
        let anchor = tos_fd_open::open_regular_at(
            &parent,
            Path::new(
                path.file_name()
                    .ok_or(SourceCommandError::Invalid("Agent DB filename"))?,
            ),
        )
        .map_err(failure)?;
        let am = anchor.metadata().map_err(failure)?;
        if am.dev() != m.dev() || am.ino() != m.ino() {
            return Err(SourceCommandError::Conflict("Agent DB anchor"));
        }
        for suffix in ["-journal", "-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{suffix}", path.display()));
            match fs::symlink_metadata(&p) {
                Ok(_) => {
                    protected(&p, root, uid, maximum)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(failure(e)),
            }
        }
        let db = rusqlite::Connection::open_with_flags(
            &path,
            (if read_only {
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            } else {
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            }) | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(failure)?;
        let journal: String = db
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .map_err(failure)?;
        if journal != "wal" {
            return Err(SourceCommandError::Denied(
                "Agent existing WAL database required",
            ));
        }
        // Force SQLite's existing WAL reader open before pinning its companions.
        let _: i64 = db
            .query_row("PRAGMA schema_version", [], |r| r.get(0))
            .map_err(failure)?;
        let mut fence = Self {
            path,
            root: root.to_owned(),
            uid,
            maximum,
            anchor,
            parent: parent.try_clone().map_err(failure)?,
            sides: Vec::new(),
        };
        for suffix in ["-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{suffix}", fence.path.display()));
            protected(&p, root, uid, maximum)?;
            let f = tos_fd_open::open_regular_at(&parent, Path::new(p.file_name().unwrap()))
                .map_err(failure)?;
            fence.sides.push((p, f));
        }
        fence.verify()?;
        Ok((fence, db))
    }
    pub(super) fn verify(&self) -> SourceCommandResult<()> {
        let parent_path = self
            .path
            .parent()
            .ok_or(SourceCommandError::Invalid("Agent DB parent"))?;
        let selected_parent = tos_fd_open::open_absolute_directory(parent_path).map_err(failure)?;
        let now = selected_parent.metadata().map_err(failure)?;
        let held = self.parent.metadata().map_err(failure)?;
        if now.dev() != held.dev() || now.ino() != held.ino() {
            return Err(SourceCommandError::Conflict("Agent DB parent replaced"));
        }
        let mut allocated = 0u64;
        for (p, f) in std::iter::once((&self.path, &self.anchor))
            .chain(self.sides.iter().map(|(p, f)| (p, f)))
        {
            let m = protected(p, &self.root, self.uid, self.maximum)?;
            let a = f.metadata().map_err(failure)?;
            if m.dev() != a.dev() || m.ino() != a.ino() {
                return Err(SourceCommandError::Conflict(
                    "Agent selected database replaced",
                ));
            }
            allocated = allocated
                .checked_add(
                    m.blocks()
                        .checked_mul(512)
                        .ok_or(SourceCommandError::Invalid("Agent DB allocation"))?,
                )
                .filter(|v| *v <= self.maximum)
                .ok_or(SourceCommandError::Denied(
                    "Agent DB and WAL physical ceiling",
                ))?;
        }
        if fs::symlink_metadata(format!("{}-journal", self.path.display())).is_ok() {
            return Err(SourceCommandError::Denied(
                "Agent unexpected rollback journal",
            ));
        }
        Ok(())
    }
}
