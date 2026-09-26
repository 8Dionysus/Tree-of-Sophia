//! Durable private pin journal. It is a storage fence, not CMD authority.

use tos_foundation::Digest256;

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::format::{FrameCoordinate, SegmentLimits};
use crate::store::OwnerBinding;

const MAGIC: &[u8; 8] = b"TOSPIN2\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PinState {
    Preparing,
    Sealed,
    Aborted,
}

impl PinState {
    fn byte(self) -> u8 {
        match self {
            Self::Preparing => 0,
            Self::Sealed => 1,
            Self::Aborted => 2,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct JournalFrame {
    pub coordinate: FrameCoordinate,
    pub binding: OwnerBinding,
}

#[derive(Clone, Debug)]
pub(crate) struct PinJournal {
    pub state: PinState,
    pub pin_id: [u8; 16],
    pub store_id: [u8; 16],
    pub fence_epoch: u64,
    pub domain_digest: Digest256,
    pub segment_digest: Option<Digest256>,
    pub segment_size: u64,
    pub prepare_id: Vec<u8>,
    pub frames: Vec<JournalFrame>,
}

impl PinJournal {
    pub fn encode(&self, limits: SegmentLimits) -> Result<Vec<u8>> {
        if self.prepare_id.is_empty()
            || self.prepare_id.len() > u16::MAX as usize
            || self.frames.len() > limits.max_frames as usize
            || self.fence_epoch == 0
            || !valid_state_fields(
                self.state,
                self.segment_digest,
                self.segment_size,
                self.frames.len(),
            )
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "invalid pin journal fields",
            ));
        }
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(self.state.byte());
        out.push(u8::from(self.segment_digest.is_some()));
        out.extend_from_slice(&[0u8; 6]);
        out.extend_from_slice(&self.pin_id);
        out.extend_from_slice(&self.store_id);
        out.extend_from_slice(&self.fence_epoch.to_le_bytes());
        out.extend_from_slice(self.domain_digest.as_bytes());
        out.extend_from_slice(
            &self
                .segment_digest
                .map(|digest| *digest.as_bytes())
                .unwrap_or([0; 32]),
        );
        out.extend_from_slice(&self.segment_size.to_le_bytes());
        out.extend_from_slice(&(self.frames.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.prepare_id.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.prepare_id);
        for frame in &self.frames {
            let binding = &frame.binding;
            if binding.profile_id.is_empty()
                || binding.profile_version.is_empty()
                || binding.subject_key.is_empty()
                || binding.profile_id.len() > u16::MAX as usize
                || binding.profile_version.len() > u16::MAX as usize
                || binding.subject_key.len() > u32::MAX as usize
            {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "invalid owner binding",
                ));
            }
            out.extend_from_slice(&frame.coordinate.header_offset.to_le_bytes());
            out.extend_from_slice(&frame.coordinate.size_bytes.to_le_bytes());
            out.extend_from_slice(frame.coordinate.sha256.as_bytes());
            put_u16_bytes(&mut out, &binding.profile_id);
            put_u16_bytes(&mut out, &binding.profile_version);
            out.extend_from_slice(&(binding.subject_key.len() as u32).to_le_bytes());
            out.extend_from_slice(&binding.subject_key);
            out.extend_from_slice(&binding.member_slot.to_le_bytes());
            if out.len() > limits.max_journal_bytes.saturating_sub(32) {
                return Err(SegmentError::new(
                    Code::BudgetExceeded,
                    "pin journal exceeds limit",
                ));
            }
        }
        let digest = Digest256::of_bytes(&out);
        out.extend_from_slice(digest.as_bytes());
        if out.len() > limits.max_journal_bytes {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "pin journal exceeds limit",
            ));
        }
        Ok(out)
    }

    pub fn decode(raw: &[u8], limits: SegmentLimits) -> Result<Self> {
        if raw.len() > limits.max_journal_bytes
            || raw.len() < 8 + 1 + 7 + 16 + 16 + 8 + 32 + 32 + 8 + 4 + 2 + 32
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin journal size invalid",
            ));
        }
        let (body, claimed) = raw.split_at(raw.len() - 32);
        if Digest256::of_bytes(body).as_bytes() != claimed {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin journal checksum differs",
            ));
        }
        let mut cursor = Cursor { raw: body, at: 0 };
        if cursor.take(8)? != MAGIC {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin journal magic differs",
            ));
        }
        let state = match cursor.take(1)?[0] {
            0 => PinState::Preparing,
            1 => PinState::Sealed,
            2 => PinState::Aborted,
            _ => {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "pin journal state unknown",
                ));
            }
        };
        let segment_present = match cursor.take(1)?[0] {
            0 => false,
            1 => true,
            _ => {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "pin segment presence invalid",
                ));
            }
        };
        if cursor.take(6)? != [0u8; 6] {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin journal reserved bytes differ",
            ));
        }
        let pin_id: [u8; 16] = cursor.take(16)?.try_into().expect("fixed bytes");
        let store_id: [u8; 16] = cursor.take(16)?.try_into().expect("fixed bytes");
        let fence_epoch = cursor.u64()?;
        let domain_digest = digest_from_raw(cursor.take(32)?)?;
        let segment_raw = cursor.take(32)?;
        let segment_digest = if segment_present {
            Some(digest_from_raw(segment_raw)?)
        } else if segment_raw == [0u8; 32] {
            None
        } else {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "absent pin segment has nonzero digest",
            ));
        };
        let segment_size = cursor.u64()?;
        let count = cursor.u32()?;
        if count > limits.max_frames || (count as usize) > body.len() / 60 {
            return Err(SegmentError::new(
                Code::BudgetExceeded,
                "pin frame count exceeds limit",
            ));
        }
        let prepare_len = cursor.u16()? as usize;
        let prepare_id = cursor.sized(prepare_len)?.to_vec();
        if prepare_id.is_empty() {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin prepare ID is empty",
            ));
        }
        let mut frames = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let header_offset = cursor.u64()?;
            let size_bytes = cursor.u64()?;
            let sha256 = digest_from_raw(cursor.take(32)?)?;
            let profile_len = cursor.u16()? as usize;
            let profile_id = cursor.sized(profile_len)?.to_vec();
            let version_len = cursor.u16()? as usize;
            let profile_version = cursor.sized(version_len)?.to_vec();
            let subject_len = cursor.u32()? as usize;
            let subject_key = cursor.sized(subject_len)?.to_vec();
            let member_slot = cursor.u32()?;
            if profile_id.is_empty() || profile_version.is_empty() || subject_key.is_empty() {
                return Err(SegmentError::new(
                    Code::InvalidReceipt,
                    "pin owner binding is empty",
                ));
            }
            frames.push(JournalFrame {
                coordinate: FrameCoordinate {
                    header_offset,
                    size_bytes,
                    sha256,
                },
                binding: OwnerBinding {
                    profile_id,
                    profile_version,
                    subject_key,
                    member_slot,
                },
            });
        }
        if cursor.at != body.len()
            || fence_epoch == 0
            || !valid_state_fields(state, segment_digest, segment_size, frames.len())
        {
            return Err(SegmentError::new(
                Code::InvalidReceipt,
                "pin journal framing differs",
            ));
        }
        Ok(Self {
            state,
            pin_id,
            store_id,
            fence_epoch,
            domain_digest,
            segment_digest,
            segment_size,
            prepare_id,
            frames,
        })
    }
}

fn valid_state_fields(
    state: PinState,
    digest: Option<Digest256>,
    size: u64,
    frames: usize,
) -> bool {
    match state {
        PinState::Preparing => digest.is_none() && size == 0 && frames == 0,
        PinState::Sealed | PinState::Aborted => digest.is_some() && size > 0 && frames > 0,
    }
}

fn put_u16_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn digest_from_raw(raw: &[u8]) -> Result<Digest256> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(64);
    for byte in raw {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 15) as usize] as char);
    }
    Digest256::from_hex(&text)
        .map_err(|_| SegmentError::new(Code::InvalidReceipt, "pin digest invalid"))
}

struct Cursor<'a> {
    raw: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(count)
            .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "pin offset overflow"))?;
        let part = self
            .raw
            .get(self.at..end)
            .ok_or_else(|| SegmentError::new(Code::InvalidReceipt, "pin journal truncated"))?;
        self.at = end;
        Ok(part)
    }
    fn sized(&mut self, count: usize) -> Result<&'a [u8]> {
        self.take(count)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("fixed bytes"),
        ))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("fixed bytes"),
        ))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("fixed bytes"),
        ))
    }
}
