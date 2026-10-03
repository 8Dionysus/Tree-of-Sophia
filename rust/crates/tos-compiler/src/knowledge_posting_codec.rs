//! Canonical bounded position blocks for the selected Search index.

use crate::{Error, Result};

pub const MAX_POSTINGS_PER_BLOCK: usize = 256;
pub const MAX_POSTING_DELTA_BYTES: usize = 255 * 9;

pub(crate) fn encode_posting_block(positions: &[u64]) -> Result<(u64, u64, u16, Vec<u8>)> {
    if positions.is_empty() || positions.len() > MAX_POSTINGS_PER_BLOCK {
        return Err(Error::Invalid("search posting block count"));
    }
    let first = positions[0];
    if first > i64::MAX as u64 {
        return Err(Error::Invalid("search posting position"));
    }
    let mut deltas = Vec::new();
    deltas
        .try_reserve_exact((positions.len() - 1) * 9)
        .map_err(|_| Error::Budget("search posting deltas"))?;
    let mut previous = first;
    for &position in &positions[1..] {
        if position <= previous || position > i64::MAX as u64 {
            return Err(Error::Invalid("search posting order"));
        }
        let mut delta = position - previous;
        while delta >= 0x80 {
            deltas.push((delta as u8 & 0x7f) | 0x80);
            delta >>= 7;
        }
        deltas.push(delta as u8);
        previous = position;
    }
    let count =
        u16::try_from(positions.len()).map_err(|_| Error::Invalid("search posting block count"))?;
    Ok((first, previous, count, deltas))
}

/// Decodes one complete canonical block. Each delta is positive, minimal
/// unsigned LEB128; a block never allocates more than 256 positions.
pub fn decode_posting_block(first: u64, last: u64, count: u16, deltas: &[u8]) -> Result<Vec<u64>> {
    let count = usize::from(count);
    if count == 0
        || count > MAX_POSTINGS_PER_BLOCK
        || first > i64::MAX as u64
        || last > i64::MAX as u64
        || first > last
        || deltas.len() > MAX_POSTING_DELTA_BYTES
    {
        return Err(Error::Invalid("search posting block shape"));
    }
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(count)
        .map_err(|_| Error::Budget("search posting decode"))?;
    positions.push(first);
    let mut previous = first;
    let mut offset = 0usize;
    for _ in 1..count {
        let start = offset;
        let mut delta = 0u64;
        loop {
            let byte = *deltas
                .get(offset)
                .ok_or(Error::Invalid("search posting delta count"))?;
            offset += 1;
            if offset - start > 9 {
                return Err(Error::Invalid("search posting delta length"));
            }
            delta |= u64::from(byte & 0x7f) << (7 * (offset - start - 1));
            if byte & 0x80 == 0 {
                if delta == 0 || (offset - start > 1 && byte == 0) {
                    return Err(Error::Invalid("search posting delta canonical"));
                }
                break;
            }
        }
        previous = previous
            .checked_add(delta)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or(Error::Invalid("search posting delta overflow"))?;
        positions.push(previous);
    }
    if offset != deltas.len() || previous != last {
        return Err(Error::Invalid("search posting block terminal"));
    }
    Ok(positions)
}
