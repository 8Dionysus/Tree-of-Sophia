use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::size_of;

use tos_foundation::{Digest256, Digest256Hasher};

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};

pub(crate) const MAGIC: &[u8; 8] = b"TOSSEG2\0";
pub(crate) const END: &[u8; 8] = b"TOSEND2\0";
pub(crate) const HEADER_BYTES: u64 = 8 + 2 + 2 + 4 + 32;
pub(crate) const FRAME_HEADER_BYTES: u64 = 8 + 32;
pub(crate) const END_BYTES: u64 = 8;
const BLOCK_BYTES: usize = 64 * 1024;
const PACKED_READ_FIXED_STATE_BYTES: usize = 1024;

/// Optional invocation meter used by ledger-bound segment operations.
pub(crate) trait SegmentIoObserver {
    fn checkpoint(&mut self) -> Result<()>;
    fn reserve_state(&mut self, bytes: usize) -> Result<()>;
    fn work_unit(&mut self) -> Result<()>;
    fn before_read(&mut self, bytes: usize) -> Result<()>;
    fn after_read(&mut self, bytes: usize) -> Result<()>;
    fn before_write(&mut self, bytes: usize) -> Result<()>;
    fn after_write(&mut self, bytes: usize) -> Result<()>;
}

/// Explicit finite limits for one physical segment, not a source ontology rule.
#[derive(Clone, Copy, Debug)]
pub struct SegmentLimits {
    pub max_segment_bytes: u64,
    pub max_frame_bytes: u64,
    pub max_frames: u32,
    pub max_journal_bytes: usize,
}

impl SegmentLimits {
    pub fn validate(self) -> Result<Self> {
        if self.max_segment_bytes <= HEADER_BYTES + END_BYTES
            || self.max_frame_bytes == 0
            || self.max_frames == 0
            || self.max_journal_bytes < 128
            || self.max_segment_bytes == u64::MAX
            || self.max_frame_bytes == u64::MAX
            || self.max_journal_bytes == usize::MAX
        {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "invalid segment limits",
            ));
        }
        Ok(self)
    }
}

/// A frame-header coordinate inside one exact sealed segment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameCoordinate {
    pub header_offset: u64,
    pub size_bytes: u64,
    pub sha256: Digest256,
}

pub(crate) fn header(domain_digest: Digest256, count: u32) -> [u8; HEADER_BYTES as usize] {
    let mut result = [0u8; HEADER_BYTES as usize];
    result[..8].copy_from_slice(MAGIC);
    result[8..10].copy_from_slice(&1u16.to_le_bytes());
    result[10..12].copy_from_slice(&0u16.to_le_bytes());
    result[12..16].copy_from_slice(&count.to_le_bytes());
    result[16..48].copy_from_slice(domain_digest.as_bytes());
    result
}

pub(crate) fn frame_header(size: u64, digest: Digest256) -> [u8; FRAME_HEADER_BYTES as usize] {
    let mut result = [0u8; FRAME_HEADER_BYTES as usize];
    result[..8].copy_from_slice(&size.to_le_bytes());
    result[8..40].copy_from_slice(digest.as_bytes());
    result
}

pub(crate) fn verify_whole(
    file: File,
    expected_segment: Digest256,
    expected_size: u64,
    domain_digest: Digest256,
    limits: SegmentLimits,
) -> Result<Vec<FrameCoordinate>> {
    verify_whole_inner(file, expected_segment, expected_size, domain_digest, limits, None)
}

pub(crate) fn verify_whole_accounted(
    file: File,
    expected_segment: Digest256,
    expected_size: u64,
    domain_digest: Digest256,
    limits: SegmentLimits,
    observer: &mut dyn SegmentIoObserver,
) -> Result<Vec<FrameCoordinate>> {
    verify_whole_inner(
        file,
        expected_segment,
        expected_size,
        domain_digest,
        limits,
        Some(observer),
    )
}

fn verify_whole_inner(
    mut file: File,
    expected_segment: Digest256,
    expected_size: u64,
    domain_digest: Digest256,
    limits: SegmentLimits,
    mut observer: Option<&mut dyn SegmentIoObserver>,
) -> Result<Vec<FrameCoordinate>> {
    let size = file
        .metadata()
        .map_err(|error| SegmentError::io("cannot stat opened segment", error))?
        .len();
    if size != expected_size || size > limits.max_segment_bytes {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "segment length differs",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    let mut head = [0u8; HEADER_BYTES as usize];
    read_hashed_metered(&mut file, &mut head, &mut hasher, &mut observer)?;
    if &head[..8] != MAGIC
        || u16::from_le_bytes([head[8], head[9]]) != 1
        || head[10..12] != [0, 0]
        || head[16..48] != domain_digest.as_bytes()[..]
    {
        return Err(SegmentError::new(
            Code::InvalidFormat,
            "segment header differs",
        ));
    }
    let count = u32::from_le_bytes(head[12..16].try_into().expect("four bytes"));
    if count == 0
        || count > limits.max_frames
        || u64::from(count) > size.saturating_sub(HEADER_BYTES + END_BYTES) / FRAME_HEADER_BYTES
    {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "segment frame count exceeds limit",
        ));
    }
    let coordinate_state = (count as usize)
        .checked_mul(size_of::<FrameCoordinate>())
        .and_then(|bytes| bytes.checked_add(BLOCK_BYTES))
        .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "segment verifier state overflow"))?;
    if let Some(observer) = observer.as_deref_mut() {
        observer.reserve_state(coordinate_state)?;
    }
    let mut offset = HEADER_BYTES;
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(count as usize)
        .map_err(|_| SegmentError::new(Code::BudgetExceeded, "segment coordinate allocation failed"))?;
    let mut block = [0u8; BLOCK_BYTES];
    for _ in 0..count {
        let mut envelope = [0u8; FRAME_HEADER_BYTES as usize];
        read_hashed_metered(&mut file, &mut envelope, &mut hasher, &mut observer)?;
        let length = u64::from_le_bytes(envelope[..8].try_into().expect("eight bytes"));
        let digest = Digest256::from_hex(&hex_bytes(&envelope[8..40])).map_err(|_| {
            SegmentError::new(Code::InvalidFormat, "segment frame digest encoding invalid")
        })?;
        let end = offset
            .checked_add(FRAME_HEADER_BYTES)
            .and_then(|at| at.checked_add(length))
            .ok_or_else(|| {
                SegmentError::new(Code::InvalidFormat, "segment frame offset overflow")
            })?;
        if length > limits.max_frame_bytes || end > size.saturating_sub(END_BYTES) {
            return Err(SegmentError::new(
                Code::InvalidFormat,
                "segment frame exceeds declared bounds",
            ));
        }
        let mut frame_hash = Digest256Hasher::new();
        let mut remaining = length;
        while remaining > 0 {
            let request =
                usize::try_from(remaining.min(BLOCK_BYTES as u64)).expect("bounded block");
            read_hashed_metered(&mut file, &mut block[..request], &mut hasher, &mut observer)?;
            frame_hash.update(&block[..request]);
            remaining -= request as u64;
        }
        if frame_hash.finalize() != digest {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "segment frame digest differs",
            ));
        }
        frames.push(FrameCoordinate {
            header_offset: offset,
            size_bytes: length,
            sha256: digest,
        });
        offset = end;
    }
    if offset.checked_add(END_BYTES) != Some(size) {
        return Err(SegmentError::new(
            Code::InvalidFormat,
            "segment has a gap or trailing bytes",
        ));
    }
    let mut trailer = [0u8; END_BYTES as usize];
    read_hashed_metered(&mut file, &mut trailer, &mut hasher, &mut observer)?;
    if &trailer != END || hasher.finalize() != expected_segment {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "segment trailer or digest differs",
        ));
    }
    Ok(frames)
}

/// Selected frame bytes are not written to the caller until their digest is
/// verified. The caller's explicit cap is also this function's memory cap.
pub(crate) fn read_selected(
    mut file: File,
    segment_size: u64,
    coordinate: FrameCoordinate,
    domain_digest: Digest256,
    expected_count: u32,
    frame_index: u32,
    max_bytes: u64,
    sink: &mut impl Write,
) -> Result<u64> {
    if coordinate.size_bytes > max_bytes {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "selected frame exceeds read limit",
        ));
    }
    let capacity = usize::try_from(coordinate.size_bytes).map_err(|_| {
        SegmentError::new(Code::BudgetExceeded, "selected frame exceeds address space")
    })?;
    let mut verified = Vec::new();
    verified
        .try_reserve_exact(capacity)
        .map_err(|_| SegmentError::new(Code::BudgetExceeded, "selected frame allocation failed"))?;
    let end = coordinate
        .header_offset
        .checked_add(FRAME_HEADER_BYTES)
        .and_then(|at| at.checked_add(coordinate.size_bytes))
        .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "frame coordinate overflow"))?;
    if end > segment_size.saturating_sub(END_BYTES) {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "frame coordinate exceeds segment",
        ));
    }
    if frame_index >= expected_count || coordinate.header_offset < HEADER_BYTES {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "frame index or offset differs",
        ));
    }
    let mut head = [0u8; HEADER_BYTES as usize];
    file.read_exact(&mut head)
        .map_err(|error| SegmentError::io("cannot read selected segment header", error))?;
    if head != header(domain_digest, expected_count) {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "selected segment header differs",
        ));
    }
    file.seek(SeekFrom::End(-(END_BYTES as i64)))
        .map_err(|error| SegmentError::io("cannot seek segment trailer", error))?;
    let mut trailer = [0u8; END_BYTES as usize];
    file.read_exact(&mut trailer)
        .map_err(|error| SegmentError::io("cannot read segment trailer", error))?;
    if &trailer != END {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "selected segment trailer differs",
        ));
    }
    file.seek(SeekFrom::Start(coordinate.header_offset))
        .map_err(|error| SegmentError::io("cannot seek selected frame", error))?;
    let mut envelope = [0u8; FRAME_HEADER_BYTES as usize];
    file.read_exact(&mut envelope)
        .map_err(|error| SegmentError::io("cannot read selected frame header", error))?;
    if envelope != frame_header(coordinate.size_bytes, coordinate.sha256) {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "selected frame header differs",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    let mut remaining = coordinate.size_bytes;
    let mut block = [0u8; BLOCK_BYTES];
    while remaining > 0 {
        let request = usize::try_from(remaining.min(BLOCK_BYTES as u64)).expect("bounded block");
        let count = file
            .read(&mut block[..request])
            .map_err(|error| SegmentError::io("cannot read selected frame", error))?;
        if count == 0 {
            return Err(SegmentError::new(
                Code::CorruptBytes,
                "selected frame truncated",
            ));
        }
        hasher.update(&block[..count]);
        verified.extend_from_slice(&block[..count]);
        remaining -= count as u64;
    }
    if hasher.finalize() != coordinate.sha256 {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "selected frame digest differs",
        ));
    }
    sink.write_all(&verified)
        .map_err(|error| SegmentError::io("cannot write verified selected frame", error))?;
    Ok(coordinate.size_bytes)
}

/// Read and verify one packed-object frame under the operation's shared
/// accounting. The pack's full content digest is checked by the cold closure;
/// this point read checks the anchored digest-named file, segment envelope,
/// exact frame ordinal and selected payload before disclosing bytes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_selected_accounted(
    mut file: File,
    segment_size: u64,
    header_offset: u64,
    payload_digest: Digest256,
    payload_size: u64,
    domain_digest: Digest256,
    expected_count: u32,
    frame_index: u32,
    max_frame_bytes: u64,
    max_bytes: u64,
    observer: &mut dyn SegmentIoObserver,
) -> Result<Vec<u8>> {
    if expected_count == 0
        || frame_index >= expected_count
        || header_offset < HEADER_BYTES
        || payload_size > max_bytes
        || payload_size > max_frame_bytes
        || segment_size < HEADER_BYTES + END_BYTES
    {
        return Err(SegmentError::new(
            Code::BudgetExceeded,
            "packed frame exceeds selected read limits",
        ));
    }
    let capacity = usize::try_from(payload_size)
        .map_err(|_| SegmentError::new(Code::BudgetExceeded, "packed frame exceeds address space"))?;
    let state_bytes = capacity
        .checked_add(PACKED_READ_FIXED_STATE_BYTES)
        .ok_or_else(|| SegmentError::new(Code::BudgetExceeded, "packed frame state overflow"))?;
    observer.reserve_state(state_bytes)?;

    let frame_end = header_offset
        .checked_add(FRAME_HEADER_BYTES)
        .and_then(|value| value.checked_add(payload_size))
        .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "packed frame offset overflow"))?;
    if frame_end > segment_size.saturating_sub(END_BYTES) {
        return Err(SegmentError::new(
            Code::InvalidReceipt,
            "packed frame exceeds segment bounds",
        ));
    }

    let mut head = [0u8; HEADER_BYTES as usize];
    read_observed(&mut file, &mut head, observer)?;
    if head != header(domain_digest, expected_count) {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "packed segment header differs",
        ));
    }

    // The authenticated extent commits the exact offset and frame index.
    // Reading an extent is constant in the pack's cardinality: the selected
    // envelope and payload are checked below. The cold pack-closure audit
    // verifies every ordinal/offset pair and the full segment digest, so a
    // point read need not rescan all preceding frame headers.

    file.seek(SeekFrom::End(-(END_BYTES as i64)))
        .map_err(|error| SegmentError::io("cannot seek packed segment trailer", error))?;
    let mut trailer = [0u8; END_BYTES as usize];
    read_observed(&mut file, &mut trailer, observer)?;
    if &trailer != END {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "packed segment trailer differs",
        ));
    }

    file.seek(SeekFrom::Start(header_offset))
        .map_err(|error| SegmentError::io("cannot seek packed frame", error))?;
    let mut envelope = [0u8; FRAME_HEADER_BYTES as usize];
    read_observed(&mut file, &mut envelope, observer)?;
    if envelope != frame_header(payload_size, payload_digest) {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "packed frame header differs",
        ));
    }

    let mut verified = Vec::new();
    verified
        .try_reserve_exact(capacity)
        .map_err(|_| SegmentError::new(Code::BudgetExceeded, "packed frame allocation failed"))?;
    verified.resize(capacity, 0);
    file.seek(SeekFrom::Start(header_offset + FRAME_HEADER_BYTES))
        .map_err(|error| SegmentError::io("cannot seek packed frame payload", error))?;
    read_observed(&mut file, &mut verified, observer)?;
    if Digest256::of_bytes(&verified) != payload_digest {
        return Err(SegmentError::new(
            Code::CorruptBytes,
            "packed frame payload digest differs",
        ));
    }
    Ok(verified)
}

fn read_hashed(file: &mut File, buffer: &mut [u8], hasher: &mut Digest256Hasher) -> Result<()> {
    read_hashed_metered(file, buffer, hasher, &mut None)
}

fn read_hashed_metered(
    file: &mut File,
    buffer: &mut [u8],
    hasher: &mut Digest256Hasher,
    observer: &mut Option<&mut dyn SegmentIoObserver>,
) -> Result<()> {
    match observer.as_deref_mut() {
        Some(observer) => read_observed(file, buffer, observer)?,
        None => file
            .read_exact(buffer)
            .map_err(|error| SegmentError::io("cannot read sealed segment", error))?,
    }
    hasher.update(buffer);
    Ok(())
}

fn read_observed(
    file: &mut File,
    buffer: &mut [u8],
    observer: &mut dyn SegmentIoObserver,
) -> Result<()> {
    observer.checkpoint()?;
    observer.before_read(buffer.len())?;
    let mut filled = 0usize;
    let mut attempted = false;
    while filled < buffer.len() {
        if attempted {
            observer.work_unit()?;
        }
        attempted = true;
        match file.read(&mut buffer[filled..]) {
            Ok(0) => {
                return Err(SegmentError::new(
                    Code::CorruptBytes,
                    "sealed segment truncated",
                ));
            }
            Ok(count) => {
                observer.after_read(count)?;
                filled += count;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(SegmentError::io("cannot read sealed segment", error)),
        }
    }
    Ok(())
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}
