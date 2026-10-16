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

fn leb_bytes(mut n: u64) -> usize {
    let mut bytes = 1;
    while n >= 128 {
        n >>= 7;
        bytes += 1;
    }
    bytes
}
fn write_leb(mut n: u64, output: &mut Vec<u8>) {
    while n >= 128 {
        output.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    output.push(n as u8);
}
fn runs(positions: &[u64], mut consume: impl FnMut(u64, u64)) {
    let mut delta = 0;
    let mut count = 0;
    for pair in positions.windows(2) {
        let next = pair[1] - pair[0];
        if next != delta && count > 0 {
            consume(delta, count);
            count = 0;
        }
        delta = next;
        count += 1;
    }
    if count > 0 {
        consume(delta, count);
    }
}
fn run_bytes(positions: &[u64]) -> usize {
    let mut length = 1; // zero is invalid as the first V1 positive delta
    runs(positions, |delta, count| {
        length += leb_bytes(delta) + leb_bytes(count)
    });
    length
}

/// V2 keeps the V1 byte ceiling. The zero-prefixed form stores maximal
/// (positive delta, repetition count) runs only when strictly smaller.
pub(crate) fn encode_posting_block_v2(positions: &[u64]) -> Result<(u64, u64, u16, Vec<u8>)> {
    let (first, last, count, mut bytes) = encode_posting_block(positions)?;
    if run_bytes(positions) < bytes.len() {
        bytes.clear();
        bytes.push(0);
        runs(positions, |delta, count| {
            write_leb(delta, &mut bytes);
            write_leb(count, &mut bytes);
        });
    }
    Ok((first, last, count, bytes))
}
fn read_leb(bytes: &[u8], offset: &mut usize) -> Result<u64> {
    let start = *offset;
    let mut n = 0;
    loop {
        let byte = *bytes
            .get(*offset)
            .ok_or(Error::Invalid("posting run truncated"))?;
        *offset += 1;
        if *offset - start > 9 {
            return Err(Error::Invalid("posting run integer length"));
        }
        n |= u64::from(byte & 127) << (7 * (*offset - start - 1));
        if byte & 128 == 0 {
            if n == 0 || *offset - start > 1 && byte == 0 {
                return Err(Error::Invalid("posting run integer canonical"));
            }
            return Ok(n);
        }
    }
}
pub(crate) fn decode_posting_block_v2(
    first: u64,
    last: u64,
    count: u16,
    bytes: &[u8],
) -> Result<Vec<u64>> {
    if bytes.first() != Some(&0) {
        let positions = decode_posting_block(first, last, count, bytes)?;
        if run_bytes(&positions) < bytes.len() {
            return Err(Error::Invalid("posting run noncanonical plain form"));
        }
        return Ok(positions);
    }
    let count = usize::from(count);
    if !(2..=MAX_POSTINGS_PER_BLOCK).contains(&count)
        || first > last
        || last > i64::MAX as u64
        || bytes.len() > MAX_POSTING_DELTA_BYTES
    {
        return Err(Error::Invalid("posting run block shape"));
    }
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(count)
        .map_err(|_| Error::Budget("posting run decode allocation"))?;
    positions.push(first);
    let mut offset = 1;
    let mut previous_delta = None;
    let mut value = first;
    let mut plain_bytes = 0;
    while offset < bytes.len() {
        let delta = read_leb(bytes, &mut offset)?;
        let run = read_leb(bytes, &mut offset)?;
        if previous_delta == Some(delta) || run > (count - positions.len()) as u64 {
            return Err(Error::Invalid("posting run count or maximality"));
        }
        previous_delta = Some(delta);
        plain_bytes += leb_bytes(delta) * run as usize;
        for _ in 0..run {
            value = value
                .checked_add(delta)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or(Error::Invalid("posting run overflow"))?;
            positions.push(value);
        }
    }
    if positions.len() != count || value != last || bytes.len() >= plain_bytes {
        return Err(Error::Invalid("posting run terminal or canonical form"));
    }
    Ok(positions)
}

/// Model admission authenticates the ABI before selecting this decoder. Older
/// model ABIs retain strict V1 decoding and cannot smuggle in V2 runs.
pub fn decode_posting_block_for_abi(
    abi: &str,
    first: u64,
    last: u64,
    count: u16,
    bytes: &[u8],
) -> Result<Vec<u64>> {
    if abi == tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4 {
        decode_posting_block_v2(first, last, count, bytes)
    } else {
        decode_posting_block(first, last, count, bytes)
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;
    #[test]
    fn posting_runs_preserve_dense_sparse_and_extreme_order_and_reject_wrong_abi() {
        for positions in [
            vec![i64::MAX as u64],
            (17..273).collect(),
            (0..256).map(|i| i * 16384).collect(),
            (0..128).map(|i| i * i + 3).collect(),
            vec![0, 1, 2, 3, 4, 1000, 1001, 1002, 1003, 1004],
        ] {
            let (first, last, count, stored) = encode_posting_block_v2(&positions).unwrap();
            assert_eq!(
                decode_posting_block_v2(first, last, count, &stored).unwrap(),
                positions
            );
            assert_eq!(
                decode_posting_block_for_abi(
                    tos_foundation::KNOWLEDGE_MODEL_ABI_V5_POSTINGS_V2_CARRIER_ONCE_V4,
                    first,
                    last,
                    count,
                    &stored
                )
                .unwrap(),
                positions
            );
            let legacy = encode_posting_block(&positions).unwrap().3;
            assert_eq!(
                decode_posting_block(first, last, count, &legacy).unwrap(),
                positions
            );
            if stored.first() == Some(&0) {
                assert!(stored.len() < legacy.len());
                assert!(decode_posting_block(first, last, count, &stored).is_err());
                assert!(decode_posting_block_v2(first, last, count, &legacy).is_err());
            }
        }
        let positions: Vec<_> = (10..266).collect();
        let (first, last, count, stored) = encode_posting_block_v2(&positions).unwrap();
        assert_eq!(stored, [0, 1, 255, 1]);
        for bad in [
            vec![0, 1, 0],
            vec![0, 0, 255, 1],
            vec![0, 1, 128, 2],
            vec![0, 1, 127, 1, 128, 1],
            vec![0, 129, 0, 255, 1],
            [stored.as_slice(), &[0]].concat(),
            stored[..3].to_vec(),
        ] {
            assert!(decode_posting_block_v2(first, last, count, &bad).is_err());
        }
        assert!(
            decode_posting_block_v2(i64::MAX as u64 - 1, i64::MAX as u64, 256, &stored).is_err()
        );
    }
}
