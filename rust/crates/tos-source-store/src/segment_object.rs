//! Bounded verification of one caller-selected range in a held segment file.
//!
//! This helper authenticates bytes against the supplied range digest. It does
//! not select or authorize the range: that binding remains with the caller's
//! authenticated segment index and source-root checks.

use std::fs::{File, Metadata};
use std::io::{self, Write};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, StoreError, StoreErrorCode};
use crate::pinned_sqlite_aux::{PinnedSqliteIoBudget, PinnedSqliteIoFailure};

const BLOCK_BYTES: usize = 64 * 1024;

/// Verify and stream exactly one selected segment extent into the caller's
/// private sink. No path is opened and no extent-sized allocation is made.
///
/// The caller supplies the extent and digest from its authenticated index;
/// this function grants no source, locator, admission, or publication
/// authority. `file` must be a held regular descriptor securely opened
/// read-only by the caller. The shared logical-I/O budget charges each actual
/// `read_at` request before the syscall and records its returned byte count,
/// including zero on EOF or an I/O error. Writes to `sink` remain the caller's
/// bounded staging responsibility.
pub(crate) fn verify_segment_range(
    file: &File,
    offset: u64,
    length: u64,
    expected_sha256: Digest256,
    max_segment_bytes: u64,
    max_object_bytes: u64,
    io_budget: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    sink: &mut impl Write,
) -> Result<u64> {
    if max_segment_bytes == 0
        || max_object_bytes == 0
        || max_segment_bytes == u64::MAX
        || max_object_bytes == u64::MAX
    {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "segment and object limits must be finite and positive",
        ));
    }
    let end = offset.checked_add(length).ok_or_else(|| {
        StoreError::new(
            StoreErrorCode::DescriptorMismatch,
            "selected segment range overflows its file offset",
        )
    })?;

    crate::streamed_cut::check_time_budgeted(deadline, cancelled, Some(io_budget))?;
    let before_metadata = file
        .metadata()
        .map_err(|error| StoreError::io("cannot inspect selected segment descriptor", error))?;
    if !before_metadata.is_file() {
        return Err(descriptor_error(
            "selected segment descriptor is not a regular file",
        ));
    }
    let before = HeldMetadata::from(&before_metadata);
    if before.length > max_segment_bytes {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "selected segment exceeds its read limit",
        ));
    }
    if length > max_object_bytes {
        return Err(StoreError::new(
            StoreErrorCode::BudgetExceeded,
            "selected object exceeds its read limit",
        ));
    }
    if end > before.length {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected segment range extends beyond the opened file",
        ));
    }

    crate::streamed_cut::check_time_budgeted(deadline, cancelled, Some(io_budget))?;
    let read_result = read_and_verify_range(
        file,
        offset,
        length,
        expected_sha256,
        io_budget,
        deadline,
        cancelled,
        sink,
    );

    // Take the second descriptor snapshot even if reading, hashing, or the
    // caller's sink failed. Preserve the original operation error when one
    // already exists; otherwise require the held inode state to be unchanged.
    let after = HeldMetadata::read(file);
    let verified_bytes = match read_result {
        Err(error) => return Err(error),
        Ok(bytes) => bytes,
    };
    let after = after?;
    if before != after {
        return Err(descriptor_error(
            "selected segment changed while its range was read",
        ));
    }
    crate::streamed_cut::check_time_budgeted(deadline, cancelled, Some(io_budget))?;
    Ok(verified_bytes)
}

fn read_and_verify_range(
    file: &File,
    offset: u64,
    length: u64,
    expected_sha256: Digest256,
    io_budget: &PinnedSqliteIoBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
    sink: &mut impl Write,
) -> Result<u64> {
    let mut digest = Digest256Hasher::new();
    let mut returned = 0u64;
    let mut buffer = [0u8; BLOCK_BYTES];

    while returned < length {
        crate::streamed_cut::check_time_budgeted(deadline, cancelled, Some(io_budget))?;
        let request_bytes = (length - returned).min(BLOCK_BYTES as u64);
        let request = usize::try_from(request_bytes).map_err(|_| {
            StoreError::new(
                StoreErrorCode::BudgetExceeded,
                "selected segment read request does not fit memory",
            )
        })?;
        io_budget.charge_read(request_bytes)?;
        let read_offset = offset.checked_add(returned).ok_or_else(|| {
            io_budget.fail(PinnedSqliteIoFailure::Io);
            StoreError::new(
                StoreErrorCode::DescriptorMismatch,
                "selected segment read offset overflowed",
            )
        })?;

        match file.read_at(&mut buffer[..request], read_offset) {
            Ok(count) => {
                let count = count as u64;
                io_budget.record_read_returned(count)?;
                crate::streamed_cut::check_time_budgeted(deadline, cancelled, Some(io_budget))?;
                if count == 0 {
                    return Err(StoreError::new(
                        StoreErrorCode::CorruptSelectedObject,
                        "selected segment ended before the declared range length",
                    ));
                }
                let count_usize = count as usize;
                digest.update(&buffer[..count_usize]);
                returned = returned.checked_add(count).ok_or_else(|| {
                    io_budget.fail(PinnedSqliteIoFailure::Io);
                    StoreError::new(
                        StoreErrorCode::BudgetExceeded,
                        "selected segment byte count overflowed",
                    )
                })?;
                sink.write_all(&buffer[..count_usize]).map_err(|error| {
                    StoreError::io("cannot write selected segment to private sink", error)
                })?;
                crate::streamed_cut::check_time_budgeted(deadline, cancelled, Some(io_budget))?;
            }
            Err(error) => {
                io_budget.record_read_returned(0)?;
                let time_check =
                    crate::streamed_cut::check_time_budgeted(deadline, cancelled, Some(io_budget));
                if error.kind() == io::ErrorKind::Interrupted {
                    time_check?;
                    continue;
                }
                time_check?;
                io_budget.fail(PinnedSqliteIoFailure::Io);
                return Err(StoreError::io("cannot read selected segment range", error));
            }
        }
    }

    if returned != length || digest.finalize() != expected_sha256 {
        return Err(StoreError::new(
            StoreErrorCode::CorruptSelectedObject,
            "selected segment range digest or length differs",
        ));
    }
    Ok(returned)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HeldMetadata {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    owner: u32,
    group: u32,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl HeldMetadata {
    fn read(file: &File) -> Result<Self> {
        let metadata = file
            .metadata()
            .map_err(|error| StoreError::io("cannot inspect selected segment descriptor", error))?;
        Ok(Self::from(&metadata))
    }

    fn from(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            mode: metadata.mode(),
            owner: metadata.uid(),
            group: metadata.gid(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

fn descriptor_error(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::DescriptorMismatch, detail)
}
