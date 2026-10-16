//! Exact existing immutable authored-cut bytes to a fresh filesystem.
//! This does not transfer the old root-bound writer configuration/grant.
use crate::{CorpusReader, CutReadLimits, Result, StoreError, StoreErrorCode};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, SourceRevision};

pub struct SourceCutRestoreResult {
    pub revision: SourceRevision,
    pub member_count: u64,
    pub source_bytes: u64,
    pub membership_sha256: Digest256,
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "source-cut restore interrupted",
        ))
    } else {
        Ok(())
    }
}
/// Source store remains immutable/exclusively controlled; destination parent is
/// owner-controlled. Failure leaves partial bytes, never a completed result.
/// Exact original/current stores and historical profiles remain separate inputs
/// to the read-only resolver; this restores only the selected current membership.
pub fn restore_source_cut(
    reader: &CorpusReader,
    revision: SourceRevision,
    destination: &Path,
    limits: CutReadLimits,
    max_directories: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SourceCutRestoreResult> {
    active(deadline, cancelled)?;
    let cut = reader.open_source_cut(revision, limits, deadline, cancelled)?;
    if max_directories == 0 || max_directories == usize::MAX {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "invalid directory limit",
        ));
    }
    let mut directories = BTreeSet::new();
    for member in cut.current().members() {
        active(deadline, cancelled)?;
        let mut prefix = String::new();
        let mut parts = member.path.as_str().split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                break;
            }
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            directories.insert(prefix.clone());
            if directories.len() > max_directories {
                return Err(StoreError::new(
                    StoreErrorCode::BudgetExceeded,
                    "source-cut directory budget",
                ));
            }
        }
    }
    let mut stream = cut.stream(revision)?;
    let expected = stream.expectation();
    let output = crate::archive::fresh_destination(destination)?;
    let mut source_bytes = 0u64;
    while let Some(member) = stream.next_member(deadline, cancelled)? {
        active(deadline, cancelled)?;
        let metadata = cut.current().member(&member.path).ok_or_else(|| {
            StoreError::new(
                StoreErrorCode::DescriptorMismatch,
                "source-cut member absent",
            )
        })?;
        let mut file =
            crate::archive::new_file(&output, member.path.as_str(), deadline, cancelled)?;
        for bytes in member.raw.chunks(65536) {
            active(deadline, cancelled)?;
            file.write_all(bytes)
                .map_err(|e| StoreError::io("source-cut restore write", e))?;
        }
        file.set_permissions(fs::Permissions::from_mode(metadata.mode))
            .map_err(|e| StoreError::io("source-cut restore mode", e))?;
        file.sync_all()
            .map_err(|e| StoreError::io("source-cut restore sync", e))?;
        source_bytes = source_bytes
            .checked_add(member.raw.len() as u64)
            .ok_or_else(|| {
                StoreError::new(StoreErrorCode::BudgetExceeded, "source-cut byte overflow")
            })?;
    }
    if stream.coverage() != Some(expected) {
        return Err(StoreError::new(
            StoreErrorCode::DescriptorMismatch,
            "incomplete source-cut restore",
        ));
    }
    output
        .sync_all()
        .map_err(|e| StoreError::io("source-cut directory sync", e))?;
    let current = tos_fd_open::open_absolute_directory(destination)
        .map_err(|_| StoreError::new(StoreErrorCode::UnsafePath, "restored source path changed"))?;
    let held = output
        .metadata()
        .map_err(|e| StoreError::io("source-cut held directory", e))?;
    let now = current
        .metadata()
        .map_err(|e| StoreError::io("source-cut selected directory", e))?;
    if (held.dev(), held.ino()) != (now.dev(), now.ino()) {
        return Err(StoreError::new(
            StoreErrorCode::UnsafePath,
            "restored source directory changed",
        ));
    }
    active(deadline, cancelled)?;
    Ok(SourceCutRestoreResult {
        revision,
        member_count: expected.count,
        source_bytes,
        membership_sha256: expected.digest,
    })
}
