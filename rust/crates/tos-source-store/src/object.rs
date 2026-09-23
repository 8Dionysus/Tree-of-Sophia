//! Verify one selected immutable object without scanning the object namespace.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, StoreError, StoreErrorCode};

const BLOCK_BYTES: usize = 1024 * 1024;

pub(crate) fn verify_selected_object(
    root: &Path,
    sha256: Digest256,
    expected_size: u64,
    max_bytes: u64,
    sink: &mut impl Write,
) -> Result<u64> {
    if expected_size > max_bytes {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "selected object exceeds read limit",
        ));
    }
    let object_dir = root.join("objects");
    let directory = fs::symlink_metadata(&object_dir)
        .map_err(|error| StoreError::io("cannot stat object directory", error))?;
    if !directory.is_dir() || directory.file_type().is_symlink() {
        return Err(StoreError::new(
            StoreErrorCode::UnsafePath,
            "object directory is not a regular directory",
        ));
    }
    let path = object_dir.join(sha256.to_hex());
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            StoreError::new(
                StoreErrorCode::CorruptSelectedObject,
                "selected object is absent",
            )
        } else {
            StoreError::io("cannot stat selected object", error)
        }
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StoreError::new(
            StoreErrorCode::UnsafePath,
            "selected object is not a regular file",
        ));
    }
    if metadata.len() != expected_size {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected object size differs",
        ));
    }
    let mut file =
        File::open(&path).map_err(|error| StoreError::io("cannot open selected object", error))?;
    let mut digest = Digest256Hasher::new();
    let mut actual = 0u64;
    let mut block = [0u8; BLOCK_BYTES];
    loop {
        // Read one byte beyond the smaller bound to detect growth after stat.
        let remaining = max_bytes.min(expected_size).saturating_sub(actual);
        let request = usize::try_from(remaining.saturating_add(1).min(BLOCK_BYTES as u64))
            .expect("request never exceeds BLOCK_BYTES");
        let count = file
            .read(&mut block[..request])
            .map_err(|error| StoreError::io("cannot read selected object", error))?;
        if count == 0 {
            break;
        }
        actual = actual.saturating_add(count as u64);
        if actual > max_bytes || actual > expected_size {
            return Err(StoreError::new(
                StoreErrorCode::CorruptSelectedObject,
                "selected object grew during read",
            ));
        }
        digest.update(&block[..count]);
        sink.write_all(&block[..count])
            .map_err(|error| StoreError::io("cannot write private selected-object stage", error))?;
    }
    if actual != expected_size || digest.finalize() != sha256 {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected object digest or size differs",
        ));
    }
    Ok(actual)
}

pub(crate) fn verify_without_copy(
    root: &Path,
    sha256: Digest256,
    expected_size: u64,
    max_bytes: u64,
) -> Result<()> {
    verify_selected_object(root, sha256, expected_size, max_bytes, &mut io::sink()).map(|_| ())
}
