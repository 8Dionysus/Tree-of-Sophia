//! Shared bounded SQLite blob row decoding for the two retained projection paths.
use crate::{Error, Result};
use tos_foundation::Digest256;
pub(crate) const MAX_PAGE_ROWS: usize = 1024;
pub(crate) const MAX_ROWS: u64 = 1_000_000;
pub(crate) const MAX_ROW_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
pub(crate) const MAX_COLD_WORK: u64 = 320 * 1024 * 1024;
pub(crate) fn maximum_limits() -> crate::NavigationOriginalLimits {
    crate::NavigationOriginalLimits {
        max_rows: MAX_ROWS,
        max_row_bytes: MAX_ROW_BYTES,
        max_total_bytes: MAX_TOTAL_BYTES,
    }
}
pub(crate) fn page_limits(
    max_rows: usize,
    max_row_bytes: usize,
    max_page_bytes: u64,
) -> Result<()> {
    if max_rows == 0
        || max_rows > MAX_PAGE_ROWS
        || max_row_bytes == 0
        || max_row_bytes > MAX_ROW_BYTES
        || max_page_bytes == 0
        || max_page_bytes > 64 * 1024 * 1024
        || max_rows
            .checked_mul(max_row_bytes)
            .is_none_or(|n| n as u64 > max_page_bytes)
    {
        return Err(Error::Budget("original projection page limits"));
    }
    Ok(())
}
/// Uncontrolled page APIs have a finite byte/work envelope. Controlled callers
/// continue to use their original CreationState through the owned readers.
pub(crate) fn page_decode_work_limit(max_page_bytes: u64) -> Result<u64> {
    max_page_bytes
        .checked_mul(16)
        .ok_or(Error::Budget("original projection decode work limit"))
}

pub(crate) fn charge_decode_work(work: &mut u64, cap: u64, bytes: usize) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .filter(|n| *n <= cap)
        .ok_or(Error::Budget("original projection decode work"))?;
    Ok(())
}

/// Copy exact logical bytes under the caller's admitted state and work limits.
/// The SQL blob remains borrowed. Compressed output admits its spare byte and
/// decoder workspace before allocation; no source-format sniffing is used.
pub(crate) fn decode_packet(
    stored: &[u8],
    declared: usize,
    max_row_bytes: usize,
    available: usize,
    layout: crate::knowledge_stage::KnowledgePayloadLayout,
    work: &mut u64,
    work_cap: u64,
) -> Result<Vec<u8>> {
    layout.verify_physical_length(stored, declared, max_row_bytes)?;
    let compressed = layout.packed_bytes()
        && crate::knowledge_byte_codec::frame_metadata(stored, Some(declared), max_row_bytes)?.1;
    let capacity = declared
        .checked_add(usize::from(compressed))
        .ok_or(Error::Budget("original projection decode capacity"))?;
    let decoder = if compressed {
        crate::knowledge_byte_codec::decoder_workspace_upper()?
    } else {
        0
    };
    if capacity.checked_add(decoder).is_none_or(|n| n > available) {
        return Err(Error::Budget("original projection decode state"));
    }
    charge_decode_work(work, work_cap, capacity)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity)
        .map_err(|_| Error::Budget("original projection allocation"))?;
    if raw.capacity() != capacity {
        return Err(Error::Budget("original projection allocation capacity"));
    }
    if compressed {
        raw.resize(capacity, 0);
        crate::knowledge_byte_codec::decompress_into(stored, declared, &mut raw, |bytes| {
            charge_decode_work(work, work_cap, bytes)
        })?;
        raw.truncate(declared);
    } else {
        raw.extend_from_slice(if layout.packed_bytes() {
            &stored[crate::knowledge_byte_codec::HEADER..]
        } else {
            stored
        });
    }
    Ok(raw)
}

pub(crate) fn read(
    scan: &mut rusqlite::Rows<'_>,
    max_page_bytes: u64,
    max_row_bytes: usize,
    layout: crate::knowledge_stage::KnowledgePayloadLayout,
    work: &mut u64,
    work_cap: u64,
) -> Result<(Vec<(i64, Vec<u8>)>, u64)> {
    let mut rows = Vec::new();
    let mut bytes = 0u64;
    while let Some(row) = scan.next()? {
        let ordinal: i64 = row.get(0)?;
        let size: i64 = row.get(1)?;
        let declared =
            usize::try_from(size).map_err(|_| Error::Invalid("original projection length"))?;
        let sha = row
            .get_ref(2)?
            .as_blob()
            .map_err(|_| Error::Invalid("original projection row digest"))?;
        let stored = row
            .get_ref(3)?
            .as_blob()
            .map_err(|_| Error::Budget("original projection row bytes"))?;
        // Navigation uses ordinal -1 for its header. Each collection owner
        // retains its own ordinal contract; this common decoder checks bytes.
        if sha.len() != 32 || declared > max_row_bytes {
            return Err(Error::Invalid("original projection row identity"));
        }
        bytes = bytes
            .checked_add(declared as u64)
            .filter(|n| *n <= max_page_bytes)
            .ok_or(Error::Budget("original projection page bytes"))?;
        // One bounded decoder workspace exists alongside the page; each
        // returned allocation may retain one spare byte (at most MAX_PAGE_ROWS).
        // Neither contributes to the logical receipt.
        let available = declared
            .checked_add(1)
            .and_then(|n| {
                n.checked_add(crate::knowledge_byte_codec::decoder_workspace_upper().ok()?)
            })
            .ok_or(Error::Budget("original projection page decode state"))?;
        let raw = decode_packet(
            stored,
            declared,
            max_row_bytes,
            available,
            layout,
            work,
            work_cap,
        )?;
        charge_decode_work(work, work_cap, raw.len())?;
        if sha != Digest256::of_bytes(&raw).as_bytes() {
            return Err(Error::Invalid("original projection row identity"));
        }
        rows.push((ordinal, raw));
    }
    Ok((rows, bytes))
}
