//! Local completion-last publication of a disposable public SQL/static build.
//! An interrupted rename restores prior generated carriers but never grants
//! source, rights, installed-current or selected-model authority.

use crate::{Error, Result, d1_public_capture::PublicCapture};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

struct Publication {
    moved: Vec<(PathBuf, PathBuf, Option<PathBuf>)>,
    retired: Vec<(PathBuf, PathBuf)>,
    markers: [PathBuf; 2],
    complete: bool,
}
impl Publication {
    fn new(output: &Path, runtime: &Path) -> Self {
        Self {
            moved: Vec::new(),
            retired: Vec::new(),
            markers: [
                output.join("__edge/build-manifest.json"),
                runtime.join("manifest.json"),
            ],
            complete: false,
        }
    }
    fn move_into(&mut self, from: &Path, to: &Path) -> Result<()> {
        if !from.exists() || from.is_symlink() || to.is_symlink() {
            return Err(Error::Invalid("public D1 publication path"));
        }
        let backup = to.with_extension(format!(
            "{}rollback",
            to.extension()
                .and_then(|e| e.to_str())
                .map(|e| format!("{e}."))
                .unwrap_or_default()
        ));
        if backup.exists() || backup.is_symlink() {
            return Err(Error::Invalid("public D1 stale rollback path"));
        }
        let prior = if to.exists() {
            fs::rename(to, &backup)?;
            Some(backup)
        } else {
            None
        };
        if let Err(error) = fs::rename(from, to) {
            if let Some(old) = prior.as_ref() {
                let _ = fs::rename(old, to);
            }
            return Err(error.into());
        }
        self.moved.push((from.to_owned(), to.to_owned(), prior));
        Ok(())
    }
    fn retire(&mut self, path: &Path) -> Result<()> {
        if path.is_symlink() {
            return Err(Error::Invalid("public D1 retired symlink"));
        }
        if path.exists() {
            let backup = path.with_extension("sql.rollback");
            if backup.exists() || backup.is_symlink() {
                return Err(Error::Invalid("public D1 retired rollback collision"));
            }
            fs::rename(path, &backup)?;
            self.retired.push((path.to_owned(), backup));
        }
        Ok(())
    }
    fn finish(mut self, capture: &PublicCapture) -> Result<()> {
        for (_, target, backup) in &self.moved {
            File::open(
                target
                    .parent()
                    .ok_or(Error::Invalid("public D1 publication parent"))?,
            )?
            .sync_all()?;
        }
        capture.charge_work(0)?;
        self.complete = true;
        // The new completion is durable. A residual backup is disposable and
        // makes the next build refuse rather than pretending cleanup succeeded.
        for (_, _, backup) in &self.moved {
            if let Some(backup) = backup {
                if let Ok(metadata) = fs::symlink_metadata(backup) {
                    if metadata.file_type().is_dir() {
                        let _ = fs::remove_dir_all(backup);
                    } else {
                        let _ = fs::remove_file(backup);
                    }
                }
            }
        }
        for (_, backup) in &self.retired {
            let _ = fs::remove_file(backup);
        }
        Ok(())
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        if self.complete {
            return;
        }
        for marker in &self.markers {
            let _ = fs::remove_file(marker);
        }
        for (_, target, backup) in self.moved.iter().rev() {
            if let Ok(metadata) = fs::symlink_metadata(target) {
                if metadata.file_type().is_dir() {
                    let _ = fs::remove_dir_all(target);
                } else {
                    let _ = fs::remove_file(target);
                }
            }
            if let Some(backup) = backup {
                let _ = fs::rename(backup, target);
            }
        }
        for (target, backup) in self.retired.iter().rev() {
            let _ = fs::rename(backup, target);
        }
    }
}

fn atomic_manifest(path: &Path, packet: &Value, capture: &PublicCapture) -> Result<()> {
    let parent = path
        .parent()
        .ok_or(Error::Invalid("public D1 manifest parent"))?;
    fs::create_dir_all(parent)?;
    let next = path.with_extension("json.next");
    if next.exists() || next.is_symlink() || path.exists() || path.is_symlink() {
        return Err(Error::Invalid("public D1 completion marker collision"));
    }
    let raw = serde_json::to_vec(packet).map_err(|e| Error::Source(e.to_string()))?;
    if raw.len() > 2 * 1024 * 1024 {
        return Err(Error::Budget("public D1 manifest bytes"));
    }
    capture.charge_work(raw.len() as u64 + 1)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)?;
    let result = (|| {
        file.write_all(&raw)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        capture.charge_work(0)?;
        fs::rename(&next, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&next);
    }
    result
}

pub(crate) fn publish(
    output: &Path,
    runtime: &Path,
    static_pending: &Path,
    sql_pending: &Path,
    baseline_pending: &Path,
    manifest: &Value,
    capture: &PublicCapture,
) -> Result<()> {
    let mut guard = Publication::new(output, runtime);
    guard.move_into(sql_pending, &runtime.join("read-model.sql"))?;
    guard.move_into(baseline_pending, &runtime.join("read-model.rows.json"))?;
    guard.move_into(static_pending, output)?;
    // A fresh full bootstrap has no affected-pair delta candidate. Remove an
    // older disposable delta only after all replacement carriers exist.
    guard.retire(&runtime.join("read-model.delta.sql"))?;
    atomic_manifest(&guard.markers[0], manifest, capture)?;
    atomic_manifest(&guard.markers[1], manifest, capture)?;
    guard.finish(capture)
}
