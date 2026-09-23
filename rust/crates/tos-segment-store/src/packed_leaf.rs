//! Canonical packed physical placement leaf. It is descriptive bytes, never a
//! source-membership certificate or a decoded read/durability capability.

use std::collections::HashMap;

use tos_foundation::Digest256;

use crate::error::{Result, SegmentError, SegmentErrorCode as Code};
use crate::format::FrameCoordinate;
use crate::generation::{GenerationShapeLimits, PartitionBoundsV1, PlacementGenerationRowV1};
use crate::placement::PlacementV1;

const MAGIC: &[u8; 8] = b"TOSGLE1\0";
const DICT_BYTES: usize = 112;
const ROW_FIXED_BYTES: usize = 4 + 4 + 4 + 32 + 4 + 8 + 8 + 32;
const NONE_BOUND: u32 = u32::MAX;

/// A bounded immutable leaf candidate. `digest` is the SHA-256 of exact wire
/// bytes, not proof that the supplied rows exhaust CMD's committed history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedPlacementLeafV1 {
    pub domain_digest: Digest256,
    pub bounds: PartitionBoundsV1,
    pub rows: Vec<PlacementGenerationRowV1>,
}

impl PackedPlacementLeafV1 {
    pub fn encode(&self, limits: GenerationShapeLimits) -> Result<Vec<u8>> {
        let limits = limits.validate()?;
        self.bounds.validate(limits.max_key_bytes)?;
        if [&self.bounds.lower_inclusive, &self.bounds.upper_exclusive]
            .into_iter()
            .flatten()
            .any(|key| key.len() >= NONE_BOUND as usize)
        {
            return Err(invalid_budget());
        }
        if self.rows.len() as u64 > limits.max_rows_per_partition
            || self.rows.len() > u32::MAX as usize
        {
            return Err(invalid_budget());
        }
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(self.domain_digest.as_bytes());
        put_bound(&mut out, &self.bounds.lower_inclusive);
        put_bound(&mut out, &self.bounds.upper_exclusive);
        if out.len() as u64 + 4 + 4 + 32 > limits.max_leaf_bytes {
            return Err(invalid_budget());
        }
        let dictionary_at = out.len();
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(self.rows.len() as u32).to_le_bytes());
        let mut dictionary = Vec::<[u8; DICT_BYTES]>::new();
        let mut indices = HashMap::<[u8; DICT_BYTES], u32>::new();
        let mut body = Vec::new();
        let mut previous = Vec::<u8>::new();
        for row in &self.rows {
            validate_row(
                row,
                self.domain_digest,
                &self.bounds,
                &previous,
                limits.max_key_bytes,
            )?;
            let segment = segment_key(&row.placement);
            let index = match indices.get(&segment) {
                Some(index) => *index,
                None => {
                    let index = u32::try_from(dictionary.len()).map_err(|_| invalid_budget())?;
                    dictionary.push(segment);
                    indices.insert(segment, index);
                    index
                }
            };
            let prefix = common_prefix(&previous, &row.key);
            body.extend_from_slice(&(prefix as u32).to_le_bytes());
            body.extend_from_slice(&((row.key.len() - prefix) as u32).to_le_bytes());
            body.extend_from_slice(&row.key[prefix..]);
            body.extend_from_slice(&index.to_le_bytes());
            body.extend_from_slice(row.placement.receipt_id.as_bytes());
            body.extend_from_slice(&row.placement.frame_index.to_le_bytes());
            body.extend_from_slice(&row.placement.coordinate.header_offset.to_le_bytes());
            body.extend_from_slice(&row.placement.coordinate.size_bytes.to_le_bytes());
            body.extend_from_slice(row.placement.coordinate.sha256.as_bytes());
            previous.clone_from(&row.key);
            let projected = out.len() as u64
                + dictionary.len() as u64 * DICT_BYTES as u64
                + body.len() as u64
                + 32;
            if projected > limits.max_leaf_bytes {
                return Err(invalid_budget());
            }
        }
        out[dictionary_at..dictionary_at + 4]
            .copy_from_slice(&(dictionary.len() as u32).to_le_bytes());
        for entry in dictionary {
            out.extend_from_slice(&entry);
        }
        out.extend_from_slice(&body);
        let digest = Digest256::of_bytes(&out);
        out.extend_from_slice(digest.as_bytes());
        if out.len() as u64 > limits.max_leaf_bytes {
            return Err(invalid_budget());
        }
        Ok(out)
    }

    pub fn decode(raw: &[u8], limits: GenerationShapeLimits) -> Result<Self> {
        let limits = limits.validate()?;
        if raw.len() as u64 > limits.max_leaf_bytes
            || raw.len() < 8 + 2 + 2 + 32 + 4 + 4 + 4 + 4 + 32
        {
            return Err(invalid_format());
        }
        let payload_end = raw.len() - 32;
        if Digest256::of_bytes(&raw[..payload_end]).as_bytes() != &raw[payload_end..]
            || &raw[..8] != MAGIC
            || raw[8..10] != 1u16.to_le_bytes()
            || raw[10..12] != [0, 0]
        {
            return Err(invalid_format());
        }
        let mut at = 12;
        let domain_digest = digest(take(raw, &mut at, 32, payload_end)?)?;
        let bounds = PartitionBoundsV1 {
            lower_inclusive: get_bound(raw, &mut at, payload_end)?,
            upper_exclusive: get_bound(raw, &mut at, payload_end)?,
        };
        bounds.validate(limits.max_key_bytes)?;
        let dictionary_len = number(raw, &mut at, payload_end)? as usize;
        let row_len = number(raw, &mut at, payload_end)? as usize;
        if row_len as u64 > limits.max_rows_per_partition
            || dictionary_len > row_len
            || dictionary_len
                .checked_mul(DICT_BYTES)
                .is_none_or(|size| size > payload_end - at)
            || row_len
                .checked_mul(ROW_FIXED_BYTES)
                .is_none_or(|size| size > payload_end - at - dictionary_len * DICT_BYTES)
        {
            return Err(invalid_format());
        }
        let mut dictionary = Vec::with_capacity(dictionary_len);
        for _ in 0..dictionary_len {
            let bytes: [u8; DICT_BYTES] = take(raw, &mut at, DICT_BYTES, payload_end)?
                .try_into()
                .expect("exact dictionary width");
            dictionary.push(bytes);
        }
        let mut rows = Vec::with_capacity(row_len);
        let mut previous = Vec::<u8>::new();
        let mut first_use = HashMap::<[u8; DICT_BYTES], usize>::new();
        for _ in 0..row_len {
            let prefix = number(raw, &mut at, payload_end)? as usize;
            let suffix_len = number(raw, &mut at, payload_end)? as usize;
            if prefix > previous.len()
                || suffix_len == 0
                || prefix
                    .checked_add(suffix_len)
                    .is_none_or(|length| length > limits.max_key_bytes)
            {
                return Err(invalid_format());
            }
            let suffix = take(raw, &mut at, suffix_len, payload_end)?;
            let mut key = previous[..prefix].to_vec();
            key.extend_from_slice(suffix);
            if prefix != common_prefix(&previous, &key) {
                return Err(invalid_format());
            }
            let index = number(raw, &mut at, payload_end)? as usize;
            let segment = *dictionary.get(index).ok_or_else(invalid_format)?;
            match first_use.get(&segment) {
                Some(prior) if *prior != index => return Err(invalid_format()),
                Some(_) => (),
                None if index == first_use.len() => {
                    first_use.insert(segment, index);
                }
                None => return Err(invalid_format()),
            }
            let receipt_id = digest(take(raw, &mut at, 32, payload_end)?)?;
            let frame_index = number(raw, &mut at, payload_end)?;
            let header_offset = wide(raw, &mut at, payload_end)?;
            let size_bytes = wide(raw, &mut at, payload_end)?;
            let sha256 = digest(take(raw, &mut at, 32, payload_end)?)?;
            let placement = placement_from_segment(
                &segment,
                receipt_id,
                frame_index,
                FrameCoordinate {
                    header_offset,
                    size_bytes,
                    sha256,
                },
            )?;
            let row = PlacementGenerationRowV1 {
                key,
                logical_digest: sha256,
                logical_length: size_bytes,
                placement,
            };
            validate_row(
                &row,
                domain_digest,
                &bounds,
                &previous,
                limits.max_key_bytes,
            )?;
            previous.clone_from(&row.key);
            rows.push(row);
        }
        if at != payload_end || first_use.len() != dictionary_len {
            return Err(invalid_format());
        }
        Ok(Self {
            domain_digest,
            bounds,
            rows,
        })
    }

    pub fn digest(raw: &[u8]) -> Digest256 {
        Digest256::of_bytes(raw)
    }
}

fn validate_row(
    row: &PlacementGenerationRowV1,
    domain: Digest256,
    bounds: &PartitionBoundsV1,
    previous: &[u8],
    max_key_bytes: usize,
) -> Result<()> {
    if row.key.is_empty()
        || row.key.len() > max_key_bytes
        || (!previous.is_empty() && row.key.as_slice() <= previous)
        || !bounds.contains(&row.key)
        || row.placement.domain_digest != domain
        || row.logical_digest != row.placement.coordinate.sha256
        || row.logical_length != row.placement.coordinate.size_bytes
        || PlacementV1::decode(&row.placement.encode()).is_err()
    {
        return Err(invalid_format());
    }
    Ok(())
}

fn segment_key(placement: &PlacementV1) -> [u8; DICT_BYTES] {
    let mut out = [0; DICT_BYTES];
    out[..16].copy_from_slice(&placement.store_id);
    out[16..48].copy_from_slice(placement.domain_digest.as_bytes());
    out[48..64].copy_from_slice(&placement.pin_id);
    out[64..72].copy_from_slice(&placement.fence_epoch.to_le_bytes());
    out[72..104].copy_from_slice(placement.segment_digest.as_bytes());
    out[104..112].copy_from_slice(&placement.segment_size.to_le_bytes());
    out
}

fn placement_from_segment(
    segment: &[u8; DICT_BYTES],
    receipt_id: Digest256,
    frame_index: u32,
    coordinate: FrameCoordinate,
) -> Result<PlacementV1> {
    let placement = PlacementV1 {
        store_id: segment[..16].try_into().expect("fixed"),
        domain_digest: digest(&segment[16..48])?,
        pin_id: segment[48..64].try_into().expect("fixed"),
        fence_epoch: u64::from_le_bytes(segment[64..72].try_into().expect("fixed")),
        receipt_id,
        segment_digest: digest(&segment[72..104])?,
        segment_size: u64::from_le_bytes(segment[104..112].try_into().expect("fixed")),
        frame_index,
        coordinate,
    };
    PlacementV1::decode(&placement.encode())
}

fn put_bound(out: &mut Vec<u8>, bound: &Option<Vec<u8>>) {
    match bound {
        None => out.extend_from_slice(&NONE_BOUND.to_le_bytes()),
        Some(key) => {
            out.extend_from_slice(&(key.len() as u32).to_le_bytes());
            out.extend_from_slice(key);
        }
    }
}

fn get_bound(raw: &[u8], at: &mut usize, end: usize) -> Result<Option<Vec<u8>>> {
    let length = number(raw, at, end)?;
    if length == NONE_BOUND {
        return Ok(None);
    }
    Ok(Some(take(raw, at, length as usize, end)?.to_vec()))
}

fn take<'a>(raw: &'a [u8], at: &mut usize, length: usize, end: usize) -> Result<&'a [u8]> {
    let next = at.checked_add(length).ok_or_else(invalid_format)?;
    if next > end {
        return Err(invalid_format());
    }
    let bytes = &raw[*at..next];
    *at = next;
    Ok(bytes)
}

fn number(raw: &[u8], at: &mut usize, end: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        take(raw, at, 4, end)?.try_into().expect("four bytes"),
    ))
}

fn wide(raw: &[u8], at: &mut usize, end: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        take(raw, at, 8, end)?.try_into().expect("eight bytes"),
    ))
}

fn digest(raw: &[u8]) -> Result<Digest256> {
    let bytes: [u8; 32] = raw.try_into().map_err(|_| invalid_format())?;
    Ok(Digest256::from_bytes(bytes))
}

fn common_prefix(left: &[u8], right: &[u8]) -> usize {
    left.iter().zip(right).take_while(|(a, b)| a == b).count()
}

fn invalid_format() -> SegmentError {
    SegmentError::new(Code::InvalidFormat, "packed placement leaf differs")
}

fn invalid_budget() -> SegmentError {
    SegmentError::new(Code::BudgetExceeded, "packed placement leaf exceeds limit")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> GenerationShapeLimits {
        GenerationShapeLimits {
            max_partitions: 4,
            max_rows_per_partition: 8,
            max_key_bytes: 64,
            max_leaf_bytes: 4096,
        }
    }

    fn leaf() -> PackedPlacementLeafV1 {
        let domain_digest = Digest256::of_bytes(b"domain");
        let segment_digest = Digest256::of_bytes(b"segment");
        let rows = [b"history/a/1".as_slice(), b"history/a/2", b"history/b/1"]
            .into_iter()
            .enumerate()
            .map(|(index, key)| {
                let sha256 = Digest256::of_bytes(key);
                PlacementGenerationRowV1 {
                    key: key.to_vec(),
                    logical_digest: sha256,
                    logical_length: key.len() as u64,
                    placement: PlacementV1 {
                        store_id: [1; 16],
                        domain_digest,
                        pin_id: [2; 16],
                        fence_epoch: 1,
                        receipt_id: Digest256::of_bytes(&[index as u8]),
                        segment_digest,
                        segment_size: 4096,
                        frame_index: index as u32,
                        coordinate: FrameCoordinate {
                            header_offset: 48 + index as u64 * 128,
                            size_bytes: key.len() as u64,
                            sha256,
                        },
                    },
                }
            })
            .collect();
        PackedPlacementLeafV1 {
            domain_digest,
            bounds: PartitionBoundsV1 {
                lower_inclusive: None,
                upper_exclusive: None,
            },
            rows,
        }
    }

    #[test]
    fn round_trip_deduplicates_segment_and_prefix_without_losing_placement() {
        let leaf = leaf();
        let raw = leaf.encode(limits()).unwrap();
        assert_eq!(PackedPlacementLeafV1::decode(&raw, limits()).unwrap(), leaf);
        assert!(raw.len() < 3 * PlacementV1::ENCODED_BYTES);
    }

    #[test]
    fn corruption_trailing_and_noncanonical_dictionary_refuse() {
        let raw = leaf().encode(limits()).unwrap();
        let mut changed = raw.clone();
        let altered = changed.len() - 33;
        changed[altered] ^= 1;
        assert_eq!(
            PackedPlacementLeafV1::decode(&changed, limits())
                .unwrap_err()
                .code,
            Code::InvalidFormat
        );
        let mut trailing = raw.clone();
        trailing.push(0);
        assert_eq!(
            PackedPlacementLeafV1::decode(&trailing, limits())
                .unwrap_err()
                .code,
            Code::InvalidFormat
        );
        let mut out_of_range = leaf();
        out_of_range.bounds.lower_inclusive = Some(b"z".to_vec());
        assert_eq!(
            out_of_range.encode(limits()).unwrap_err().code,
            Code::InvalidFormat
        );
    }
}
