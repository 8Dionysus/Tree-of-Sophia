//! Bounded ZIP member read for the existing owner-local EPUB extraction route.
//! The caller owns the protected payload descriptor, its complete-file SHA and
//! ancestor/currentness checks. A member digest is never a read grant.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use flate2::{Decompress, FlushDecompress, Status};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;

const MAX_ZIP_MEMBERS: usize = 2048;
const MAX_DIRECTORY_BYTES: usize = 1_048_576;
const MAX_MEMBER_BYTES: usize = 16_777_216;
const MAX_EXPANDED_BYTES: u64 = 67_108_864;
const MAX_MEMBER_NAME_BYTES: usize = 1024;
const TAIL_BYTES: u64 = 65_557;
const CHUNK_BYTES: usize = 65_536;

// Python's ZIP filename fallback is CP437 when the UTF-8 bit is clear. Keep
// that exact name comparison rather than narrowing the maintained profile to
// ASCII; the selected logical path is still checked independently below.
const CP437_HIGH: [u16; 128] = [
    0x00c7, 0x00fc, 0x00e9, 0x00e2, 0x00e4, 0x00e0, 0x00e5, 0x00e7, 0x00ea, 0x00eb, 0x00e8, 0x00ef,
    0x00ee, 0x00ec, 0x00c4, 0x00c5, 0x00c9, 0x00e6, 0x00c6, 0x00f4, 0x00f6, 0x00f2, 0x00fb, 0x00f9,
    0x00ff, 0x00d6, 0x00dc, 0x00a2, 0x00a3, 0x00a5, 0x20a7, 0x0192, 0x00e1, 0x00ed, 0x00f3, 0x00fa,
    0x00f1, 0x00d1, 0x00aa, 0x00ba, 0x00bf, 0x2310, 0x00ac, 0x00bd, 0x00bc, 0x00a1, 0x00ab, 0x00bb,
    0x2591, 0x2592, 0x2593, 0x2502, 0x2524, 0x2561, 0x2562, 0x2556, 0x2555, 0x2563, 0x2551, 0x2557,
    0x255d, 0x255c, 0x255b, 0x2510, 0x2514, 0x2534, 0x252c, 0x251c, 0x2500, 0x253c, 0x255e, 0x255f,
    0x255a, 0x2554, 0x2569, 0x2566, 0x2560, 0x2550, 0x256c, 0x2567, 0x2568, 0x2564, 0x2565, 0x2559,
    0x2558, 0x2552, 0x2553, 0x256b, 0x256a, 0x2518, 0x250c, 0x2588, 0x2584, 0x258c, 0x2590, 0x2580,
    0x03b1, 0x00df, 0x0393, 0x03c0, 0x03a3, 0x03c3, 0x00b5, 0x03c4, 0x03a6, 0x0398, 0x03a9, 0x03b4,
    0x221e, 0x03c6, 0x03b5, 0x2229, 0x2261, 0x00b1, 0x2265, 0x2264, 0x2320, 0x2321, 0x00f7, 0x2248,
    0x00b0, 0x2219, 0x00b7, 0x221a, 0x207f, 0x00b2, 0x25a0, 0x00a0,
];

#[derive(Clone)]
struct Entry {
    local_offset: u64,
    compressed: u64,
    expanded: u64,
    crc32: u32,
    flags: u16,
    method: u16,
    name: String,
}

fn invalid() -> SourceCommandError {
    SourceCommandError::Invalid("native EPUB ZIP member or directory")
}

fn le16(bytes: &[u8], offset: usize) -> SourceCommandResult<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}

fn le32(bytes: &[u8], offset: usize) -> SourceCommandResult<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}

fn read_at(
    file: &mut File,
    offset: u64,
    buffer: &mut [u8],
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<()> {
    file.seek(SeekFrom::Start(offset)).map_err(|_| invalid())?;
    read_exact_authorized(file, buffer, authorize)
}

fn read_exact_authorized(
    file: &mut File,
    mut buffer: &mut [u8],
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<()> {
    while !buffer.is_empty() {
        authorize()?;
        match file.read(buffer) {
            Ok(0) => return Err(invalid()),
            Ok(n) => buffer = &mut buffer[n..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(invalid()),
        }
    }
    Ok(())
}

fn decode_name(bytes: &[u8], flags: u16) -> SourceCommandResult<String> {
    if flags & 0x800 != 0 {
        return String::from_utf8(bytes.to_vec()).map_err(|_| invalid());
    }
    let mut result = String::with_capacity(bytes.len());
    for byte in bytes {
        let point = if *byte < 128 {
            u16::from(*byte)
        } else {
            CP437_HIGH[usize::from(*byte - 128)]
        };
        result.push(char::from_u32(u32::from(point)).ok_or_else(invalid)?);
    }
    Ok(result)
}

fn member_path(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_MEMBER_NAME_BYTES
        && !name.contains('\0')
        && !name.contains('\\')
        && !name.starts_with('/')
        && name
            .trim_end_matches('/')
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
}

fn directory(
    file: &mut File,
    size: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<(Vec<Entry>, u64)> {
    active(deadline, cancelled)?;
    let tail_start = size.saturating_sub(TAIL_BYTES);
    let mut tail = vec![0; usize::try_from(size - tail_start).map_err(|_| invalid())?];
    read_at(file, tail_start, &mut tail, authorize)?;
    let end = tail
        .windows(4)
        .rposition(|slice| slice == b"PK\x05\x06")
        .ok_or_else(invalid)?;
    if end + 22 > tail.len() {
        return Err(invalid());
    }
    let directory_pos = tail_start + end as u64;
    let disk = le16(&tail, end + 4)?;
    let directory_disk = le16(&tail, end + 6)?;
    let disk_count = usize::from(le16(&tail, end + 8)?);
    let declared = usize::from(le16(&tail, end + 10)?);
    let length = usize::try_from(le32(&tail, end + 12)?).map_err(|_| invalid())?;
    let offset = u64::from(le32(&tail, end + 16)?);
    let comment = usize::from(le16(&tail, end + 20)?);
    if disk != 0
        || directory_disk != 0
        || disk_count != declared
        || !(1..=MAX_ZIP_MEMBERS).contains(&declared)
        || length > MAX_DIRECTORY_BYTES
        || offset.checked_add(length as u64) != Some(directory_pos)
        || end + 22 + comment != tail.len()
    {
        return Err(invalid());
    }
    if directory_pos >= 20 {
        let mut marker = [0; 4];
        read_at(file, directory_pos - 20, &mut marker, authorize)?;
        if marker == *b"PK\x06\x07" {
            return Err(SourceCommandError::Unsupported("native EPUB ZIP64"));
        }
    }
    let mut raw = vec![0; length];
    read_at(file, offset, &mut raw, authorize)?;
    let mut cursor = 0usize;
    let mut count = 0usize;
    let mut total_expanded = 0u64;
    let mut names = BTreeSet::new();
    let mut entries = Vec::with_capacity(declared);
    while cursor < raw.len() {
        if count % 64 == 0 {
            active(deadline, cancelled)?;
        }
        let head = raw.get(cursor..cursor + 46).ok_or_else(invalid)?;
        if &head[..4] != b"PK\x01\x02" {
            return Err(invalid());
        }
        let name_len = usize::from(le16(head, 28)?);
        let extra_len = usize::from(le16(head, 30)?);
        let comment_len = usize::from(le16(head, 32)?);
        let end = cursor
            .checked_add(46)
            .and_then(|n| n.checked_add(name_len))
            .and_then(|n| n.checked_add(extra_len))
            .and_then(|n| n.checked_add(comment_len))
            .ok_or_else(invalid)?;
        count += 1;
        if count > MAX_ZIP_MEMBERS || end > raw.len() || name_len > MAX_MEMBER_NAME_BYTES {
            return Err(invalid());
        }
        let flags = le16(head, 8)?;
        let method = le16(head, 10)?;
        let expanded = u64::from(le32(head, 24)?);
        let compressed = u64::from(le32(head, 20)?);
        total_expanded = total_expanded.checked_add(expanded).ok_or_else(invalid)?;
        if flags & !0x808 != 0
            || !matches!(method, 0 | 8)
            || expanded > MAX_MEMBER_BYTES as u64
            || compressed > size
            || total_expanded > MAX_EXPANDED_BYTES
            || (le32(head, 40)? >> 16) & 0o170000 == 0o120000
        {
            return Err(invalid());
        }
        let name_raw = &raw[cursor + 46..cursor + 46 + name_len];
        if name_raw.contains(&0) {
            return Err(invalid());
        }
        let name = decode_name(name_raw, flags)?;
        if !member_path(&name) || !names.insert(name.clone()) {
            return Err(invalid());
        }
        let mut extra = cursor + 46 + name_len;
        let extra_end = extra + extra_len;
        while extra < extra_end {
            let kind = le16(&raw, extra)?;
            let width = usize::from(le16(&raw, extra + 2)?);
            extra = extra
                .checked_add(4)
                .and_then(|n| n.checked_add(width))
                .ok_or_else(invalid)?;
            if kind == 1 || extra > extra_end {
                return Err(invalid());
            }
        }
        let entry = Entry {
            local_offset: u64::from(le32(head, 42)?),
            compressed,
            expanded,
            crc32: le32(head, 16)?,
            flags,
            method,
            name,
        };
        entries.push(entry);
        cursor = end;
    }
    if count != declared {
        return Err(invalid());
    }
    Ok((entries, directory_pos))
}

pub(crate) fn read_selected_member(
    file: &mut File,
    size: u64,
    selected_path: &str,
    expected_sha256: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    if size == 0
        || size > 512 * 1024 * 1024
        || !member_path(selected_path)
        || expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(SourceCommandError::Invalid(
            "native EPUB selected member binding",
        ));
    }
    let raw = read_bounded_member(file, size, selected_path, deadline, cancelled, &mut || {
        Ok(())
    })?;
    if Digest256::of_bytes(&raw).to_hex() != expected_sha256 {
        return Err(SourceCommandError::Conflict(
            "native EPUB selected member bytes differ",
        ));
    }
    Ok(raw)
}

/// Metadata member read used by the Item owner before its complete resource
/// enumeration. The separately granted descriptor/fixity stays with the caller.
pub(crate) fn read_bounded_member(
    file: &mut File,
    size: u64,
    selected_path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Vec<u8>> {
    if size == 0 || size > 512 * 1024 * 1024 || !member_path(selected_path) {
        return Err(invalid());
    }
    let (entries, directory_pos) = directory(file, size, deadline, cancelled, authorize)?;
    let entry = entries
        .iter()
        .find(|entry| entry.name == selected_path)
        .ok_or_else(invalid)?;
    let next_offset = entries
        .iter()
        .filter(|other| other.local_offset > entry.local_offset)
        .map(|other| other.local_offset)
        .min()
        .unwrap_or(directory_pos);
    let raw = read_member(
        file,
        entry,
        directory_pos.min(next_offset),
        deadline,
        cancelled,
        authorize,
    )?;
    Ok(raw)
}

/// The Item inventory enumerates the actual bounded directory once and reads
/// each member through the same CRC/expanded-size protected decoder. Payload
/// bytes live only for the callback and never enter the tracked inventory.
pub(crate) fn visit_members(
    file: &mut File,
    size: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
    mut observe: impl FnMut(&str, &[u8]) -> SourceCommandResult<()>,
) -> SourceCommandResult<()> {
    if size == 0 || size > 512 * 1024 * 1024 {
        return Err(invalid());
    }
    let (entries, directory_pos) = directory(file, size, deadline, cancelled, authorize)?;
    let offsets: BTreeSet<_> = entries.iter().map(|entry| entry.local_offset).collect();
    if offsets.len() != entries.len() {
        return Err(invalid());
    }
    let mut expanded = 0u64;
    for entry in &entries {
        active(deadline, cancelled)?;
        let next = offsets
            .range((
                std::ops::Bound::Excluded(entry.local_offset),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .copied()
            .unwrap_or(directory_pos);
        let bytes = read_member(
            file,
            entry,
            directory_pos.min(next),
            deadline,
            cancelled,
            authorize,
        )?;
        expanded = expanded
            .checked_add(bytes.len() as u64)
            .ok_or_else(invalid)?;
        if expanded > MAX_EXPANDED_BYTES {
            return Err(invalid());
        }
        if !entry.name.ends_with('/') {
            observe(&entry.name, &bytes)?;
        }
    }
    Ok(())
}

fn read_member(
    file: &mut File,
    entry: &Entry,
    boundary: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Vec<u8>> {
    let mut local = [0; 30];
    read_at(file, entry.local_offset, &mut local, authorize)?;
    if &local[..4] != b"PK\x03\x04"
        || le16(&local, 6)? != entry.flags
        || le16(&local, 8)? != entry.method
    {
        return Err(invalid());
    }
    let name_len = usize::from(le16(&local, 26)?);
    let extra_len = usize::from(le16(&local, 28)?);
    if name_len > MAX_MEMBER_NAME_BYTES {
        return Err(invalid());
    }
    let mut name = vec![0; name_len];
    let local_name = entry.local_offset.checked_add(30).ok_or_else(invalid)?;
    read_at(file, local_name, &mut name, authorize)?;
    if decode_name(&name, entry.flags)? != entry.name {
        return Err(invalid());
    }
    let content = local_name
        .checked_add(name_len as u64)
        .and_then(|n| n.checked_add(extra_len as u64))
        .ok_or_else(invalid)?;
    let end = content.checked_add(entry.compressed).ok_or_else(invalid)?;
    if end > boundary {
        return Err(invalid());
    }
    file.seek(SeekFrom::Start(content)).map_err(|_| invalid())?;
    let mut remaining = entry.compressed;
    let mut result = Vec::with_capacity(usize::try_from(entry.expanded).map_err(|_| invalid())?);
    let mut input = [0; CHUNK_BYTES];
    let mut output = [0; CHUNK_BYTES];
    let mut decoder = (entry.method == 8).then(|| Decompress::new(false));
    let mut stream_ended = false;
    let mut crc = crc32fast::Hasher::new();
    while remaining > 0 {
        active(deadline, cancelled)?;
        let take = usize::try_from(remaining.min(CHUNK_BYTES as u64)).map_err(|_| invalid())?;
        read_exact_authorized(file, &mut input[..take], authorize)?;
        remaining -= take as u64;
        if let Some(decoder) = &mut decoder {
            let mut consumed = 0usize;
            while consumed < take {
                active(deadline, cancelled)?;
                let before_in = decoder.total_in();
                let before_out = decoder.total_out();
                let status = decoder
                    .decompress(&input[consumed..take], &mut output, FlushDecompress::None)
                    .map_err(|_| invalid())?;
                let used =
                    usize::try_from(decoder.total_in() - before_in).map_err(|_| invalid())?;
                let produced =
                    usize::try_from(decoder.total_out() - before_out).map_err(|_| invalid())?;
                consumed += used;
                if result
                    .len()
                    .checked_add(produced)
                    .is_none_or(|n| n > MAX_MEMBER_BYTES)
                {
                    return Err(invalid());
                }
                result.extend_from_slice(&output[..produced]);
                crc.update(&output[..produced]);
                if status == Status::StreamEnd {
                    if consumed != take || remaining != 0 {
                        return Err(invalid());
                    }
                    stream_ended = true;
                    break;
                }
                if used == 0 && produced == 0 {
                    return Err(invalid());
                }
            }
        } else {
            if result
                .len()
                .checked_add(take)
                .is_none_or(|n| n > MAX_MEMBER_BYTES)
            {
                return Err(invalid());
            }
            result.extend_from_slice(&input[..take]);
            crc.update(&input[..take]);
        }
    }
    if result.len() as u64 != entry.expanded
        || crc.finalize() != entry.crc32
        || decoder.is_some() && !stream_ended
        || decoder
            .as_ref()
            .is_some_and(|d| d.total_out() != entry.expanded || d.total_in() != entry.compressed)
    {
        return Err(SourceCommandError::Conflict(
            "native EPUB selected member bytes differ",
        ));
    }
    active(deadline, cancelled)?;
    authorize()?;
    Ok(result)
}
