//! Exact physical frame coordinates. A decoded placement is descriptive data,
//! never a durability or read capability: `recover_placement` checks its pin
//! and all actual segment bytes before constructing a private receipt.

use tos_foundation::Digest256;

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::format::{END_BYTES, FRAME_HEADER_BYTES, FrameCoordinate, HEADER_BYTES};

const MAGIC: &[u8; 8] = b"TOSPLCV1";
const WIRE_BYTES: usize = 208;

/// Stable within one placement generation; a logical record can later point
/// to a different generation without changing its source identity or rights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacementV1 {
    pub(crate) store_id: [u8; 16],
    pub(crate) domain_digest: Digest256,
    pub(crate) pin_id: [u8; 16],
    pub(crate) fence_epoch: u64,
    pub(crate) receipt_id: Digest256,
    pub(crate) segment_digest: Digest256,
    pub(crate) segment_size: u64,
    pub(crate) frame_index: u32,
    pub(crate) coordinate: FrameCoordinate,
}

impl PlacementV1 {
    pub const ENCODED_BYTES: usize = WIRE_BYTES;

    pub fn store_id(&self) -> [u8; 16] {
        self.store_id
    }
    pub fn domain_digest(&self) -> Digest256 {
        self.domain_digest
    }
    pub fn pin_id(&self) -> [u8; 16] {
        self.pin_id
    }
    pub fn fence_epoch(&self) -> u64 {
        self.fence_epoch
    }
    pub fn receipt_id(&self) -> Digest256 {
        self.receipt_id
    }
    pub fn segment_digest(&self) -> Digest256 {
        self.segment_digest
    }
    pub fn segment_size(&self) -> u64 {
        self.segment_size
    }
    pub fn frame_index(&self) -> u32 {
        self.frame_index
    }
    pub fn coordinate(&self) -> FrameCoordinate {
        self.coordinate
    }

    pub fn encode(&self) -> [u8; WIRE_BYTES] {
        let mut out = [0u8; WIRE_BYTES];
        let mut at = 0;
        put(&mut out, &mut at, MAGIC);
        put(&mut out, &mut at, &1u16.to_le_bytes());
        put(&mut out, &mut at, &0u16.to_le_bytes());
        put(&mut out, &mut at, &self.store_id);
        put(&mut out, &mut at, self.domain_digest.as_bytes());
        put(&mut out, &mut at, &self.pin_id);
        put(&mut out, &mut at, &self.fence_epoch.to_le_bytes());
        put(&mut out, &mut at, self.receipt_id.as_bytes());
        put(&mut out, &mut at, self.segment_digest.as_bytes());
        put(&mut out, &mut at, &self.segment_size.to_le_bytes());
        put(&mut out, &mut at, &self.frame_index.to_le_bytes());
        put(
            &mut out,
            &mut at,
            &self.coordinate.header_offset.to_le_bytes(),
        );
        put(&mut out, &mut at, &self.coordinate.size_bytes.to_le_bytes());
        put(&mut out, &mut at, self.coordinate.sha256.as_bytes());
        debug_assert_eq!(at, WIRE_BYTES);
        out
    }

    pub fn decode(raw: &[u8]) -> Result<Self> {
        if raw.len() != WIRE_BYTES
            || &raw[..8] != MAGIC
            || raw[8..10] != 1u16.to_le_bytes()
            || raw[10..12] != [0, 0]
        {
            return Err(SegmentError::new(
                Code::InvalidFormat,
                "placement version or length differs",
            ));
        }
        let mut at = 12;
        let store_id = take::<16>(raw, &mut at);
        let domain_digest = digest(take::<32>(raw, &mut at))?;
        let pin_id = take::<16>(raw, &mut at);
        let fence_epoch = u64::from_le_bytes(take::<8>(raw, &mut at));
        let receipt_id = digest(take::<32>(raw, &mut at))?;
        let segment_digest = digest(take::<32>(raw, &mut at))?;
        let segment_size = u64::from_le_bytes(take::<8>(raw, &mut at));
        let frame_index = u32::from_le_bytes(take::<4>(raw, &mut at));
        let header_offset = u64::from_le_bytes(take::<8>(raw, &mut at));
        let size_bytes = u64::from_le_bytes(take::<8>(raw, &mut at));
        let sha256 = digest(take::<32>(raw, &mut at))?;
        if at != WIRE_BYTES
            || fence_epoch == 0
            || segment_size < HEADER_BYTES + FRAME_HEADER_BYTES + END_BYTES
            || header_offset < HEADER_BYTES
            || header_offset
                .checked_add(FRAME_HEADER_BYTES)
                .and_then(|at| at.checked_add(size_bytes))
                .is_none_or(|end| end > segment_size - END_BYTES)
        {
            return Err(SegmentError::new(
                Code::InvalidFormat,
                "placement frame bounds differ",
            ));
        }
        Ok(Self {
            store_id,
            domain_digest,
            pin_id,
            fence_epoch,
            receipt_id,
            segment_digest,
            segment_size,
            frame_index,
            coordinate: FrameCoordinate {
                header_offset,
                size_bytes,
                sha256,
            },
        })
    }
}

fn put(out: &mut [u8], at: &mut usize, bytes: &[u8]) {
    out[*at..*at + bytes.len()].copy_from_slice(bytes);
    *at += bytes.len();
}

fn take<const N: usize>(raw: &[u8], at: &mut usize) -> [u8; N] {
    let bytes = raw[*at..*at + N]
        .try_into()
        .expect("fixed placement size checked");
    *at += N;
    bytes
}

fn digest(bytes: [u8; 32]) -> Result<Digest256> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = [0u8; 64];
    for (index, byte) in bytes.into_iter().enumerate() {
        text[2 * index] = HEX[(byte >> 4) as usize];
        text[2 * index + 1] = HEX[(byte & 15) as usize];
    }
    Digest256::from_hex(std::str::from_utf8(&text).expect("hex is ASCII"))
        .map_err(|_| SegmentError::new(Code::InvalidFormat, "placement digest encoding differs"))
}
