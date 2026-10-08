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
    decode_packet_from_connection(
        None,
        stored,
        declared,
        max_row_bytes,
        available,
        layout,
        work,
        work_cap,
    )
}

/// The page path borrows the already-held SQLite connection. Dictionary SQL
/// inherits that connection's existing finite VM controller; no model reopen.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_packet_from_connection(
    db: Option<&rusqlite::Connection>,
    stored: &[u8],
    declared: usize,
    max_row_bytes: usize,
    available: usize,
    layout: crate::knowledge_stage::KnowledgePayloadLayout,
    work: &mut u64,
    work_cap: u64,
) -> Result<Vec<u8>> {
    use crate::knowledge_byte_codec as codec;
    layout.verify_physical_length(stored, declared, max_row_bytes)?;
    let dictionary_frame = layout.dictionary_bytes() && codec::is_dictionary_frame(stored);
    let compressed = dictionary_frame
        || layout.packed_bytes() && codec::frame_metadata(stored, Some(declared), max_row_bytes)?.1;
    // Admit the worst dictionary (borrowed SQL plus owned copy) before looking
    // it up. Returned rows retain at most this extra 4 KiB of vector capacity.
    let dictionary_state = if dictionary_frame {
        2 * codec::DICTIONARY_BYTES
            + tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound(
            )
    } else {
        0
    };
    let decoder = if compressed {
        codec::decoder_workspace_upper()?
    } else {
        0
    };
    let capacity_bound = declared
        .checked_add(usize::from(compressed))
        .and_then(|n| {
            n.checked_add(if dictionary_frame {
                codec::DICTIONARY_BYTES
            } else {
                0
            })
        })
        .ok_or(Error::Budget("original projection decode capacity"))?;
    if capacity_bound
        .checked_add(decoder)
        .and_then(|n| n.checked_add(dictionary_state))
        .is_none_or(|n| n > available)
    {
        return Err(Error::Budget("original projection decode state"));
    }
    let dictionary = if dictionary_frame {
        let db = db.ok_or(Error::Invalid("original dictionary connection absent"))?;
        let (_, digest) = codec::dictionary_frame_metadata(stored, Some(declared), max_row_bytes)?;
        let sql = "SELECT CASE WHEN typeof(dictionary)='blob' AND length(dictionary) BETWEEN 1 AND 4096 THEN dictionary END FROM knowledge_byte_dictionaries WHERE dictionary_sha256=?1";
        charge_decode_work(work, work_cap, sql.len() + 32)?;
        let mut statement = db.prepare(sql)?;
        let mut rows = statement.query([digest.as_bytes().as_slice()])?;
        let row = rows
            .next()?
            .ok_or(Error::Invalid("original dictionary absent"))?;
        let raw = row
            .get_ref(0)?
            .as_blob()
            .map_err(|_| Error::Invalid("original dictionary type/length"))?;
        charge_decode_work(work, work_cap, raw.len())?;
        if Digest256::of_bytes(raw) != digest {
            return Err(Error::Invalid("original dictionary digest"));
        }
        charge_decode_work(work, work_cap, raw.len())?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(raw.len())
            .map_err(|_| Error::Budget("original dictionary allocation"))?;
        if owned.capacity() != raw.len() {
            return Err(Error::Budget("original dictionary capacity"));
        }
        owned.extend_from_slice(raw);
        if rows.next()?.is_some() {
            return Err(Error::Invalid("original dictionary duplicate"));
        }
        Some(owned)
    } else {
        None
    };
    let prefix = dictionary.as_ref().map_or(0, |d| d.len());
    let capacity = declared
        .checked_add(usize::from(compressed))
        .and_then(|n| n.checked_add(prefix))
        .ok_or(Error::Budget("original projection decode capacity"))?;
    charge_decode_work(work, work_cap, capacity)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity)
        .map_err(|_| Error::Budget("original projection allocation"))?;
    if raw.capacity() != capacity {
        return Err(Error::Budget("original projection allocation capacity"));
    }
    if compressed {
        raw.resize(capacity, 0);
        if let Some(dictionary) = dictionary.as_deref() {
            codec::decompress_dictionary_into(stored, dictionary, declared, &mut raw, |bytes| {
                charge_decode_work(work, work_cap, bytes)
            })?;
            charge_decode_work(work, work_cap, declared)?;
            raw.copy_within(prefix..prefix + declared, 0);
        } else {
            codec::decompress_into(stored, declared, &mut raw, |bytes| {
                charge_decode_work(work, work_cap, bytes)
            })?;
        }
        raw.truncate(declared);
    } else {
        raw.extend_from_slice(if layout.packed_bytes() {
            &stored[codec::HEADER..]
        } else {
            stored
        });
    }
    Ok(raw)
}

pub(crate) fn read(
    db: &rusqlite::Connection,
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
        // returned V3 allocation can retain a 4 KiB dictionary prefix as spare
        // capacity. Neither that capacity nor scratch contributes to receipts.
        let available = declared
            .checked_add(1)
            .and_then(|n| {
                n.checked_add(crate::knowledge_byte_codec::decoder_workspace_upper().ok()?)
            })
            .and_then(|n| n.checked_add(if layout.dictionary_bytes() {
                3 * crate::knowledge_byte_codec::DICTIONARY_BYTES
                    + tos_source_store::PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound()
            } else {0}))
            .ok_or(Error::Budget("original projection page decode state"))?;
        let raw = decode_packet_from_connection(
            Some(db),
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
