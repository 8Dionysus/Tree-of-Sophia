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
pub(crate) fn read(
    scan: &mut rusqlite::Rows<'_>,
    max_page_bytes: u64,
) -> Result<(Vec<(i64, Vec<u8>)>, u64)> {
    let mut rows = Vec::new();
    let mut bytes = 0u64;
    while let Some(row) = scan.next()? {
        let ordinal: i64 = row.get(0)?;
        let size: i64 = row.get(1)?;
        let sha: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(2)?
            .ok_or(Error::Invalid("original projection row digest"))?;
        let raw: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>(3)?
            .ok_or(Error::Budget("original projection row bytes"))?;
        bytes = bytes
            .checked_add(raw.len() as u64)
            .filter(|n| *n <= max_page_bytes)
            .ok_or(Error::Budget("original projection page bytes"))?;
        if size < 0
            || size as usize != raw.len()
            || sha.as_slice() != Digest256::of_bytes(&raw).as_bytes()
        {
            return Err(Error::Invalid("original projection row identity"));
        }
        rows.push((ordinal, raw));
    }
    Ok((rows, bytes))
}
