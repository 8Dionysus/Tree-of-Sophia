//! Verify one selected immutable object without scanning the object namespace.

use std::io::{self, Read, Write};

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, StoreError, StoreErrorCode};
use crate::secure_open::StoreRoot;

const BLOCK_BYTES: usize = 1024 * 1024;

pub(crate) fn verify_selected_object(
    root: &StoreRoot,
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
    let mut file = root.open_object(&sha256.to_hex()).map_err(|error| {
        if error.code == StoreErrorCode::Io
            && error
                .source
                .as_ref()
                .is_some_and(|source| source.kind() == io::ErrorKind::NotFound)
        {
            StoreError::new(
                StoreErrorCode::CorruptSelectedObject,
                "selected object is absent",
            )
        } else {
            error
        }
    })?;
    let metadata = file
        .metadata()
        .map_err(|error| StoreError::io("cannot stat opened selected object", error))?;
    if metadata.len() != expected_size {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected object size differs",
        ));
    }
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
    root: &StoreRoot,
    sha256: Digest256,
    expected_size: u64,
    max_bytes: u64,
) -> Result<()> {
    verify_selected_object(root, sha256, expected_size, max_bytes, &mut io::sink()).map(|_| ())
}
