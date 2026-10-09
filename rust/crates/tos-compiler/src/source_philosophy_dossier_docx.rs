// Draft for rust/crates/tos-compiler/src/source_philosophy_dossier_docx.rs.
// Kept outside the pinned source worktree. No package is accepted by this module.
use flate2::read::DeflateDecoder;
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
    name::QName,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 16_384;
const MAX_XML_EVENTS: usize = 4_000_000;
const MAX_XML_DEPTH: usize = 256;

fn python_lower(value: &str) -> String {
    let points = value.chars().count();
    tos_foundation::python_lower_unicode16_v1(value, points, usize::MAX, usize::MAX)
        .expect("Unicode16 lowercase with input-derived unbounded output limits")
}

#[derive(Clone, Debug, Default)]
pub struct DocxMetadata {
    pub creator: Option<String>,
    pub custom_generator: Option<String>,
    pub last_modified_by: Option<String>,
    pub signature_part_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DocxTable {
    /// python-docx Row.cells order, including horizontal/vertical merge repeats.
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct DocxDocument {
    pub paragraphs: Vec<String>,
    pub tables: Vec<DocxTable>,
    pub metadata: DocxMetadata,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct DocxValidationIssue {
    pub code: String,
    pub message: String,
    pub blocking: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DocxValidation {
    pub title: String,
    pub table_row: String,
    pub metadata_identity_posture: String,
    pub identity_diagnostics: Vec<String>,
    pub paragraph_count: usize,
    pub table_count: usize,
    pub metadata_headers: Vec<Vec<String>>,
    pub coverage_tables: Vec<serde_json::Value>,
}

#[derive(Clone, Debug)]
struct ZipMember {
    name: Vec<u8>,
    flags: u16,
    method: u16,
    crc32: u32,
    compressed_bytes: u64,
    uncompressed_bytes: u64,
    local_header_offset: u64,
}

#[derive(Clone, Debug)]
struct PhysicalCell {
    text: String,
    span: usize,
    vertical_merge: VerticalMerge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VerticalMerge {
    None,
    Restart,
    Continue,
}

#[derive(Default)]
struct RowBuilder {
    cells: Vec<PhysicalCell>,
    grid_before: usize,
}

struct TableBuilder {
    rows: Vec<Vec<String>>,
    previous_vertical: Vec<Option<String>>,
}

impl TableBuilder {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            previous_vertical: Vec::new(),
        }
    }

    fn append_row(
        &mut self,
        row: RowBuilder,
        check: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<(), String> {
        for previous in self.previous_vertical.iter().flatten() {
            check(previous.len() as u64)?;
        }
        let mut grid_col = row.grid_before;
        let mut values = Vec::new();
        let mut next_vertical = self.previous_vertical.clone();

        for cell in row.cells {
            let end = grid_col
                .checked_add(cell.span)
                .ok_or("DOCX grid span overflow")?;
            if end > 16_384 {
                return Err("DOCX table grid exceeds cell limit".into());
            }
            if next_vertical.len() < end {
                next_vertical.resize(end, None);
            }
            for column in grid_col..end {
                let repeated = match cell.vertical_merge {
                    VerticalMerge::Continue => self
                        .previous_vertical
                        .get(column)
                        .and_then(Clone::clone)
                        .unwrap_or_else(|| cell.text.clone()),
                    VerticalMerge::Restart | VerticalMerge::None => cell.text.clone(),
                };
                check(repeated.len() as u64)?;
                values.push(repeated.clone());
                next_vertical[column] = match cell.vertical_merge {
                    VerticalMerge::Restart | VerticalMerge::Continue => Some(repeated),
                    VerticalMerge::None => None,
                };
            }
            grid_col = end;
        }
        self.previous_vertical = next_vertical;
        self.rows.push(values);
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ZipDirectory {
    entry_count: usize,
    offset: usize,
    size: usize,
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    let end = at.checked_add(2).ok_or("ZIP offset overflow")?;
    let raw: [u8; 2] = bytes
        .get(at..end)
        .ok_or("truncated ZIP u16")?
        .try_into()
        .map_err(|_| "truncated ZIP u16")?;
    Ok(u16::from_le_bytes(raw))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    let end = at.checked_add(4).ok_or("ZIP offset overflow")?;
    let raw: [u8; 4] = bytes
        .get(at..end)
        .ok_or("truncated ZIP u32")?
        .try_into()
        .map_err(|_| "truncated ZIP u32")?;
    Ok(u32::from_le_bytes(raw))
}

fn u64_at(bytes: &[u8], at: usize) -> Result<u64, String> {
    let end = at.checked_add(8).ok_or("ZIP offset overflow")?;
    let raw: [u8; 8] = bytes
        .get(at..end)
        .ok_or("truncated ZIP u64")?
        .try_into()
        .map_err(|_| "truncated ZIP u64")?;
    Ok(u64::from_le_bytes(raw))
}

fn checked_range(total: usize, offset: u64, length: u64) -> Result<std::ops::Range<usize>, String> {
    let start = usize::try_from(offset).map_err(|_| "ZIP offset exceeds address space")?;
    let count = usize::try_from(length).map_err(|_| "ZIP length exceeds address space")?;
    let end = start.checked_add(count).ok_or("ZIP range overflow")?;
    if end > total {
        return Err("ZIP range outside archive".into());
    }
    Ok(start..end)
}

fn find_eocd(raw: &[u8]) -> Result<usize, String> {
    let search_from = raw.len().saturating_sub(22 + u16::MAX as usize);
    for at in (search_from..=raw.len().saturating_sub(22)).rev() {
        if raw.get(at..at + 4) == Some(b"PK\x05\x06") {
            let comment = usize::from(u16_at(raw, at + 20)?);
            if at.checked_add(22 + comment) == Some(raw.len()) {
                return Ok(at);
            }
        }
    }
    Err("DOCX ZIP end record missing or has trailing bytes".into())
}

fn directory_from_eocd(raw: &[u8], eocd: usize) -> Result<ZipDirectory, String> {
    let disk = u16_at(raw, eocd + 4)?;
    let central_disk = u16_at(raw, eocd + 6)?;
    let disk_entries = u16_at(raw, eocd + 8)?;
    let total_entries = u16_at(raw, eocd + 10)?;
    let size32 = u32_at(raw, eocd + 12)?;
    let offset32 = u32_at(raw, eocd + 16)?;
    if disk != 0 || central_disk != 0 {
        return Err("multi-disk DOCX ZIP is unsupported".into());
    }

    let zip64 = total_entries == u16::MAX
        || disk_entries == u16::MAX
        || size32 == u32::MAX
        || offset32 == u32::MAX;
    let (entry_count, size, offset) = if zip64 {
        let locator = eocd.checked_sub(20).ok_or("ZIP64 locator missing")?;
        if raw.get(locator..locator + 4) != Some(b"PK\x06\x07") {
            return Err("ZIP64 locator missing".into());
        }
        if u32_at(raw, locator + 4)? != 0 || u32_at(raw, locator + 16)? != 1 {
            return Err("multi-disk ZIP64 DOCX is unsupported".into());
        }
        let record_offset = u64_at(raw, locator + 8)?;
        let record = checked_range(raw.len(), record_offset, 56)?;
        if raw.get(record.start..record.start + 4) != Some(b"PK\x06\x06") {
            return Err("ZIP64 end record missing".into());
        }
        let record_size = u64_at(raw, record.start + 4)?;
        let record_end = record_offset
            .checked_add(12)
            .and_then(|value| value.checked_add(record_size))
            .ok_or("ZIP64 end record size overflow")?;
        if record_size < 44 || record_end > locator as u64 {
            return Err("invalid ZIP64 end record size".into());
        }
        let disk = u32_at(raw, record.start + 16)?;
        let central_disk = u32_at(raw, record.start + 20)?;
        let disk_entries = u64_at(raw, record.start + 24)?;
        let total_entries = u64_at(raw, record.start + 32)?;
        if disk != 0 || central_disk != 0 || disk_entries != total_entries {
            return Err("multi-disk ZIP64 DOCX is unsupported".into());
        }
        (
            usize::try_from(total_entries).map_err(|_| "ZIP64 entry count overflow")?,
            u64_at(raw, record.start + 40)?,
            u64_at(raw, record.start + 48)?,
        )
    } else {
        if disk_entries != total_entries {
            return Err("ZIP entry count differs across disks".into());
        }
        (
            usize::from(total_entries),
            u64::from(size32),
            u64::from(offset32),
        )
    };
    if entry_count > MAX_ZIP_ENTRIES {
        return Err("DOCX ZIP entry count exceeds limit".into());
    }
    let directory = checked_range(raw.len(), offset, size)?;
    if directory.end > eocd {
        return Err("DOCX central directory overlaps end record".into());
    }
    Ok(ZipDirectory {
        entry_count,
        offset: directory.start,
        size: directory.len(),
    })
}

fn zip64_values(extra: &[u8], member: &mut ZipMember, has_disk_start: bool) -> Result<(), String> {
    let mut at = 0;
    while at < extra.len() {
        if at + 4 > extra.len() {
            return Err("truncated ZIP extra field".into());
        }
        let id = u16_at(extra, at)?;
        let length = usize::from(u16_at(extra, at + 2)?);
        at += 4;
        let end = at.checked_add(length).ok_or("ZIP extra length overflow")?;
        let field = extra.get(at..end).ok_or("truncated ZIP extra value")?;
        if id == 0x0001 {
            let mut cursor = 0;
            if member.uncompressed_bytes == u32::MAX as u64 {
                member.uncompressed_bytes = u64_at(field, cursor)?;
                cursor += 8;
            }
            if member.compressed_bytes == u32::MAX as u64 {
                member.compressed_bytes = u64_at(field, cursor)?;
                cursor += 8;
            }
            if member.local_header_offset == u32::MAX as u64 {
                member.local_header_offset = u64_at(field, cursor)?;
                cursor += 8;
            }
            if has_disk_start && u32_at(field, cursor)? != 0 {
                return Err("multi-disk ZIP64 member is unsupported".into());
            }
            return Ok(());
        }
        at = end;
    }
    if member.uncompressed_bytes == u32::MAX as u64
        || member.compressed_bytes == u32::MAX as u64
        || member.local_header_offset == u32::MAX as u64
        || has_disk_start
    {
        return Err("required ZIP64 extra value missing".into());
    }
    Ok(())
}

fn central_members(raw: &[u8], directory: ZipDirectory) -> Result<Vec<ZipMember>, String> {
    let end = directory
        .offset
        .checked_add(directory.size)
        .ok_or("central directory overflow")?;
    let mut at = directory.offset;
    let mut members = Vec::new();
    members
        .try_reserve(directory.entry_count)
        .map_err(|error| error.to_string())?;
    for _ in 0..directory.entry_count {
        if at.checked_add(46).is_none_or(|next| next > end)
            || raw.get(at..at + 4) != Some(b"PK\x01\x02")
        {
            return Err("invalid DOCX central directory member".into());
        }
        let flags = u16_at(raw, at + 8)?;
        let method = u16_at(raw, at + 10)?;
        let crc32 = u32_at(raw, at + 16)?;
        let compressed32 = u32_at(raw, at + 20)?;
        let uncompressed32 = u32_at(raw, at + 24)?;
        let name_len = usize::from(u16_at(raw, at + 28)?);
        let extra_len = usize::from(u16_at(raw, at + 30)?);
        let comment_len = usize::from(u16_at(raw, at + 32)?);
        let disk_start = u16_at(raw, at + 34)?;
        let local32 = u32_at(raw, at + 42)?;
        let variable = name_len
            .checked_add(extra_len)
            .and_then(|value| value.checked_add(comment_len))
            .ok_or("central member variable bytes overflow")?;
        let next = at
            .checked_add(46 + variable)
            .ok_or("central directory offset overflow")?;
        if next > end {
            return Err("truncated DOCX central directory member".into());
        }
        let name_start = at + 46;
        let extra_start = name_start + name_len;
        let mut member = ZipMember {
            name: raw[name_start..extra_start].to_vec(),
            flags,
            method,
            crc32,
            compressed_bytes: u64::from(compressed32),
            uncompressed_bytes: u64::from(uncompressed32),
            local_header_offset: u64::from(local32),
        };
        zip64_values(
            &raw[extra_start..extra_start + extra_len],
            &mut member,
            disk_start == u16::MAX,
        )?;
        if disk_start != 0 && disk_start != u16::MAX {
            return Err("multi-disk DOCX member is unsupported".into());
        }
        members.push(member);
        at = next;
    }
    if at != end {
        return Err("unaccounted bytes in DOCX central directory".into());
    }
    Ok(members)
}

fn crc32(bytes: &[u8], check: &mut dyn FnMut(u64) -> Result<(), String>) -> Result<u32, String> {
    let mut crc = !0u32;
    for chunk in bytes.chunks(64 * 1024) {
        check(chunk.len() as u64)?;
        for &byte in chunk {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    Ok(!crc)
}

fn extract_member(
    raw: &[u8],
    member: &ZipMember,
    central_offset: usize,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    if member.flags & 0x2041 != 0 {
        return Err("encrypted DOCX ZIP member is unsupported".into());
    }
    if !matches!(member.method, 0 | 8) {
        return Err(format!(
            "DOCX ZIP compression method {} is unsupported",
            member.method
        ));
    }
    let local = usize::try_from(member.local_header_offset)
        .map_err(|_| "DOCX local header offset overflow")?;
    let signature_end = local
        .checked_add(4)
        .ok_or("DOCX local signature offset overflow")?;
    if raw.get(local..signature_end) != Some(b"PK\x03\x04") {
        return Err("DOCX local file header missing".into());
    }
    let flags = u16_at(raw, local + 6)?;
    let method = u16_at(raw, local + 8)?;
    let name_len = usize::from(u16_at(raw, local + 26)?);
    let extra_len = usize::from(u16_at(raw, local + 28)?);
    if flags != member.flags || method != member.method {
        return Err("DOCX local and central header differ".into());
    }
    let name_start = local
        .checked_add(30)
        .ok_or("DOCX local header offset overflow")?;
    let data_start = name_start
        .checked_add(name_len)
        .and_then(|value| value.checked_add(extra_len))
        .ok_or("DOCX local header size overflow")?;
    let name_end = name_start
        .checked_add(name_len)
        .ok_or("DOCX local name range overflow")?;
    if raw.get(name_start..name_end) != Some(member.name.as_slice()) {
        return Err("DOCX local and central member names differ".into());
    }
    let compressed = checked_range(raw.len(), data_start as u64, member.compressed_bytes)?;
    if compressed.end > central_offset {
        return Err("DOCX member data overlaps the central directory".into());
    }
    let expected = usize::try_from(member.uncompressed_bytes)
        .map_err(|_| "DOCX member length exceeds address space")?;
    let mut output = Vec::new();
    match member.method {
        0 => {
            if member.compressed_bytes != member.uncompressed_bytes {
                return Err("stored DOCX member length mismatch".into());
            }
            check(expected as u64)?;
            output
                .try_reserve(expected)
                .map_err(|error| error.to_string())?;
            output.extend_from_slice(&raw[compressed]);
        }
        8 => {
            let mut decoder = DeflateDecoder::new(&raw[compressed]);
            let mut buffer = [0u8; 64 * 1024];
            loop {
                check(0)?;
                let count = decoder
                    .read(&mut buffer)
                    .map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                check(count as u64)?;
                if output
                    .len()
                    .checked_add(count)
                    .is_none_or(|size| size > expected)
                {
                    return Err("DOCX member expands beyond declared length".into());
                }
                output
                    .try_reserve(count)
                    .map_err(|error| error.to_string())?;
                output.extend_from_slice(&buffer[..count]);
            }
        }
        _ => unreachable!("method was screened by central_members"),
    }
    if output.len() != expected {
        return Err("DOCX member length differs from central directory".into());
    }
    if crc32(&output, check)? != member.crc32 {
        return Err("DOCX member CRC-32 mismatch".into());
    }
    Ok(output)
}

fn local_name(name: &[u8]) -> Result<&str, String> {
    let name = std::str::from_utf8(name)
        .map_err(|error| format!("DOCX XML name is not UTF-8: {error}"))?;
    Ok(name.rsplit(':').next().unwrap_or(name))
}

/// ElementTree accepts XML UTF-8 and UTF-16 byte streams. `quick-xml` is built
/// without its optional legacy-encoding feature, so normalize the XML encodings
/// that are self-identifying in an XML byte stream before parsing.
fn decode_xml_bytes(raw: &[u8]) -> Result<String, String> {
    let (bytes, little_endian) = if raw.starts_with(&[0xff, 0xfe]) {
        (&raw[2..], Some(true))
    } else if raw.starts_with(&[0xfe, 0xff]) {
        (&raw[2..], Some(false))
    } else if raw.starts_with(&[b'<', 0]) {
        (raw, Some(true))
    } else if raw.starts_with(&[0, b'<']) {
        (raw, Some(false))
    } else {
        let bytes = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(raw);
        return std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|error| format!("DOCX XML encoding is unsupported: {error}"));
    };
    if bytes.len() % 2 != 0 {
        return Err("DOCX UTF-16 XML has an odd byte count".into());
    }
    let units = bytes.chunks_exact(2).map(|pair| {
        if little_endian == Some(true) {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .map_err(|error| format!("DOCX UTF-16 XML contains invalid surrogate: {error}"))
}

fn element_namespace<'a>(qname: &[u8], scope: &'a BTreeMap<String, String>) -> Option<&'a str> {
    let qname = std::str::from_utf8(qname).ok()?;
    let (prefix, _local) = qname.split_once(':').unwrap_or(("", qname));
    scope.get(prefix).map(String::as_str)
}

fn word_element_name(name: QName<'_>, scope: &BTreeMap<String, String>) -> Result<String, String> {
    let local_binding = name.local_name();
    let local = local_name(local_binding.as_ref())?;
    if element_namespace(name.as_ref(), scope)
        == Some("http://schemas.openxmlformats.org/wordprocessingml/2006/main")
    {
        Ok(local.to_owned())
    } else {
        // Keep foreign-namespace elements in the structural stack while making
        // them invisible to WordprocessingML extraction rules.
        Ok(format!("\0{local}"))
    }
}

fn extend_namespace_scope(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    scope: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let key = attribute.key.as_ref();
        let prefix = if key == b"xmlns" {
            Some("")
        } else {
            std::str::from_utf8(key)
                .ok()
                .and_then(|name| name.strip_prefix("xmlns:"))
        };
        if let Some(prefix) = prefix {
            let value = attribute
                .decode_and_unescape_value(decoder)
                .map_err(|error| error.to_string())?;
            scope.insert(prefix.to_owned(), value.into_owned());
        }
    }
    Ok(())
}

fn unqualified_attribute(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    wanted: &[u8],
) -> Result<Option<String>, String> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        if attribute.key.as_ref() == wanted {
            return attribute
                .decode_and_unescape_value(decoder)
                .map(|value| Some(value.into_owned()))
                .map_err(|error| error.to_string());
        }
    }
    Ok(None)
}

fn word_attribute_value(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    scope: &BTreeMap<String, String>,
    wanted_local_name: &str,
) -> Result<Option<String>, String> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let qname = std::str::from_utf8(attribute.key.as_ref())
            .map_err(|error| format!("DOCX XML attribute name is not UTF-8: {error}"))?;
        let (prefix, local) = qname.split_once(':').unwrap_or(("", qname));
        if local == wanted_local_name
            && !prefix.is_empty()
            && scope.get(prefix).map(String::as_str)
                == Some("http://schemas.openxmlformats.org/wordprocessingml/2006/main")
        {
            let value = attribute
                .decode_and_unescape_value(decoder)
                .map_err(|error| error.to_string())?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn append_cell_paragraph(cell: &mut PhysicalCell, paragraph: String) {
    if !cell.text.is_empty() {
        cell.text.push('\n');
    }
    cell.text.push_str(&paragraph);
}

fn start_word_element(
    name: &str,
    element: Option<&BytesStart<'_>>,
    decoder: quick_xml::encoding::Decoder,
    scope: &BTreeMap<String, String>,
    stack: &[String],
    body_depth: &mut Option<usize>,
    table_depth: &mut usize,
    table: &mut Option<TableBuilder>,
    paragraph: &mut Option<(bool, String)>,
    cell: &mut Option<PhysicalCell>,
    row: &mut Option<RowBuilder>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    check(1)?;
    let parent = stack.last().map(String::as_str);
    if name == "body" && body_depth.is_none() && parent == Some("document") {
        *body_depth = Some(stack.len());
    } else if name == "tbl" && body_depth.is_some() && *table_depth == 0 && parent == Some("body") {
        *table_depth = 1;
        *table = Some(TableBuilder::new());
    } else if name == "tbl" && *table_depth > 0 {
        *table_depth += 1;
    }
    if name == "tr" && *table_depth == 1 && parent == Some("tbl") {
        *row = Some(RowBuilder::default());
    }
    if name == "tc" && *table_depth == 1 && row.is_some() && parent == Some("tr") {
        *cell = Some(PhysicalCell {
            text: String::new(),
            span: 1,
            vertical_merge: VerticalMerge::None,
        });
    }
    if name == "gridBefore" && *table_depth == 1 && parent == Some("trPr") {
        if let Some(value) = element
            .map(|element| word_attribute_value(element, decoder, scope, "val"))
            .transpose()?
            .flatten()
            .and_then(|value| value.parse::<usize>().ok())
        {
            if value > 16_384 {
                return Err("DOCX row gridBefore exceeds limit".into());
            }
            if let Some(row) = row {
                row.grid_before = value;
            }
        }
    }
    if name == "gridSpan" && *table_depth == 1 && parent == Some("tcPr") {
        if let Some(value) = element
            .map(|element| word_attribute_value(element, decoder, scope, "val"))
            .transpose()?
            .flatten()
            .and_then(|value| value.parse::<usize>().ok())
        {
            if value == 0 || value > 16_384 {
                return Err("DOCX cell gridSpan outside limit".into());
            }
            if let Some(cell) = cell {
                cell.span = value;
            }
        }
    }
    if name == "vMerge" && *table_depth == 1 && parent == Some("tcPr") {
        let value = element
            .map(|element| word_attribute_value(element, decoder, scope, "val"))
            .transpose()?
            .flatten();
        if let Some(cell) = cell {
            cell.vertical_merge = if value.as_deref().is_none_or(|value| value == "continue") {
                VerticalMerge::Continue
            } else if value.as_deref() == Some("restart") {
                VerticalMerge::Restart
            } else {
                return Err("DOCX vMerge value is unsupported".into());
            };
        }
    }
    if name == "p" && body_depth.is_some() {
        if *table_depth == 0 && parent == Some("body") {
            *paragraph = Some((false, String::new()));
        } else if *table_depth == 1 && cell.is_some() && parent == Some("tc") {
            *paragraph = Some((true, String::new()));
        }
    }
    if name == "tab" && paragraph.is_some() {
        if let Some((_, target)) = paragraph {
            target.push('\t');
        }
    }
    if matches!(name, "br" | "cr") && paragraph.is_some() {
        if let Some((_, target)) = paragraph {
            target.push('\n');
        }
    }
    if name == "noBreakHyphen" && paragraph.is_some() {
        if let Some((_, target)) = paragraph {
            target.push('\u{2011}');
        }
    }
    if name == "softHyphen" && paragraph.is_some() {
        if let Some((_, target)) = paragraph {
            target.push('\u{00ad}');
        }
    }
    Ok(())
}

fn finish_word_element(
    name: &str,
    body_depth: &mut Option<usize>,
    stack_depth: usize,
    table_depth: &mut usize,
    table: &mut Option<TableBuilder>,
    paragraphs: &mut Vec<String>,
    paragraph: &mut Option<(bool, String)>,
    cell: &mut Option<PhysicalCell>,
    row: &mut Option<RowBuilder>,
    tables: &mut Vec<DocxTable>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    check(1)?;
    match name {
        "p" => {
            if let Some((in_cell, text)) = paragraph.take() {
                if in_cell {
                    if let Some(cell) = cell {
                        append_cell_paragraph(cell, text);
                    }
                } else {
                    paragraphs.push(text);
                }
            }
        }
        "tc" if *table_depth == 1 => {
            let cell = cell.take().ok_or("DOCX cell close without cell")?;
            row.as_mut()
                .ok_or("DOCX cell outside row")?
                .cells
                .push(cell);
        }
        "tr" if *table_depth == 1 => {
            let row = row.take().ok_or("DOCX row close without row")?;
            table
                .as_mut()
                .ok_or("DOCX row outside table")?
                .append_row(row, check)?;
        }
        "tbl" if *table_depth == 1 => {
            let finished = table.take().ok_or("DOCX table close without table")?;
            tables.push(DocxTable {
                rows: finished.rows,
            });
            *table_depth = 0;
        }
        "tbl" if *table_depth > 1 => *table_depth -= 1,
        "body" if body_depth.is_some_and(|depth| depth == stack_depth) => {
            *body_depth = None;
        }
        _ => {}
    }
    Ok(())
}

fn push_text(
    paragraph: &mut Option<(bool, String)>,
    text: &str,
    total_text: &mut usize,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    let next = total_text
        .checked_add(text.len())
        .ok_or("DOCX XML text accounting overflow")?;
    check(text.len() as u64)?;
    *total_text = next;
    if let Some((_, target)) = paragraph {
        target
            .try_reserve(text.len())
            .map_err(|error| error.to_string())?;
        target.push_str(text);
    }
    Ok(())
}

fn parse_document_xml(
    raw: &[u8],
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(Vec<String>, Vec<DocxTable>), String> {
    check(raw.len() as u64)?;
    let xml = decode_xml_bytes(raw)?;
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = true;
    let mut stack = Vec::<String>::new();
    let mut namespaces = vec![BTreeMap::<String, String>::new()];
    let mut body_depth = None;
    let mut table_depth = 0usize;
    let mut table = None;
    let mut row = None;
    let mut cell = None;
    let mut paragraph: Option<(bool, String)> = None;
    let mut in_text_depth: Option<usize> = None;
    let mut paragraphs = Vec::new();
    let mut tables = Vec::new();
    let mut total_text = 0usize;
    let mut events = 0usize;

    loop {
        check(1)?;
        events += 1;
        if events > MAX_XML_EVENTS || stack.len() > MAX_XML_DEPTH {
            return Err("DOCX document.xml structural limit exceeded".into());
        }
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(element) => {
                let mut scope = namespaces.last().cloned().unwrap_or_default();
                extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                let name = word_element_name(element.name(), &scope)?;
                if name == "t" && paragraph.is_some() {
                    in_text_depth = Some(stack.len() + 1);
                }
                start_word_element(
                    &name,
                    Some(&element),
                    reader.decoder(),
                    &scope,
                    &stack,
                    &mut body_depth,
                    &mut table_depth,
                    &mut table,
                    &mut paragraph,
                    &mut cell,
                    &mut row,
                    check,
                )?;
                stack.push(name);
                namespaces.push(scope);
            }
            Event::Empty(element) => {
                let mut scope = namespaces.last().cloned().unwrap_or_default();
                extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                let name = word_element_name(element.name(), &scope)?;
                start_word_element(
                    &name,
                    Some(&element),
                    reader.decoder(),
                    &scope,
                    &stack,
                    &mut body_depth,
                    &mut table_depth,
                    &mut table,
                    &mut paragraph,
                    &mut cell,
                    &mut row,
                    check,
                )?;
                finish_word_element(
                    &name,
                    &mut body_depth,
                    stack.len(),
                    &mut table_depth,
                    &mut table,
                    &mut paragraphs,
                    &mut paragraph,
                    &mut cell,
                    &mut row,
                    &mut tables,
                    check,
                )?;
            }
            Event::Text(text) if in_text_depth.is_some() => {
                let text = text.xml10_content().map_err(|error| error.to_string())?;
                push_text(&mut paragraph, &text, &mut total_text, check)?;
            }
            Event::CData(text) if in_text_depth.is_some() => {
                let text = text.xml10_content().map_err(|error| error.to_string())?;
                push_text(&mut paragraph, &text, &mut total_text, check)?;
            }
            Event::GeneralRef(reference) if in_text_depth.is_some() => {
                let name = reference.decode().map_err(|error| error.to_string())?;
                let text = if let Some(value) = reference
                    .resolve_char_ref()
                    .map_err(|error| error.to_string())?
                {
                    value.to_string()
                } else {
                    quick_xml::escape::resolve_predefined_entity(&name)
                        .ok_or("DOCX document.xml contains unknown entity")?
                        .to_owned()
                };
                push_text(&mut paragraph, &text, &mut total_text, check)?;
            }
            Event::End(element) => {
                let scope = namespaces.last().cloned().unwrap_or_default();
                let name = word_element_name(element.name(), &scope)?;
                if name == "t" {
                    in_text_depth = None;
                }
                let depth = stack.len().saturating_sub(1);
                finish_word_element(
                    &name,
                    &mut body_depth,
                    depth,
                    &mut table_depth,
                    &mut table,
                    &mut paragraphs,
                    &mut paragraph,
                    &mut cell,
                    &mut row,
                    &mut tables,
                    check,
                )?;
                if stack.pop().as_deref() != Some(name.as_str()) {
                    return Err("DOCX document.xml element stack mismatch".into());
                }
                namespaces.pop();
            }
            Event::DocType(_) => return Err("DOCX document.xml DTD is unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }

    let paragraphs = paragraphs
        .into_iter()
        .map(|paragraph| scrub(&paragraph))
        .filter(|paragraph| !paragraph.is_empty())
        .collect();
    for table in &mut tables {
        for row in &mut table.rows {
            for cell in row {
                *cell = scrub(cell);
            }
        }
    }
    Ok((paragraphs, tables))
}

fn parse_property_xml(
    raw: &[u8],
    custom_generator: bool,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<BTreeMap<String, String>, String> {
    check(raw.len() as u64)?;
    let xml = decode_xml_bytes(raw)?;
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = true;
    let mut stack: Vec<String> = Vec::new();
    let mut namespaces = vec![BTreeMap::<String, String>::new()];
    let mut values = BTreeMap::<String, String>::new();
    let mut selected: Option<(String, String, usize)> = None;
    let mut seen_core = BTreeSet::<String>::new();
    let mut events = 0usize;

    loop {
        check(1)?;
        events += 1;
        if events > MAX_XML_EVENTS || stack.len() > MAX_XML_DEPTH {
            return Err("DOCX properties XML structural limit exceeded".into());
        }
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(element) => {
                let local_binding = element.local_name();
                let name = local_name(local_binding.as_ref())?.to_owned();
                let depth = stack.len();
                let mut scope = namespaces.last().cloned().unwrap_or_default();
                extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                if depth == 1 && custom_generator {
                    if unqualified_attribute(&element, reader.decoder(), b"name")?.as_deref()
                        == Some("generator")
                    {
                        selected = Some(("custom_generator".into(), String::new(), depth));
                    }
                } else if depth == 1 && !custom_generator {
                    let namespace = element_namespace(element.name().as_ref(), &scope);
                    let field = match (namespace, name.as_str()) {
                        (Some("http://purl.org/dc/elements/1.1/"), "creator") => Some("creator"),
                        (
                            Some(
                                "http://schemas.openxmlformats.org/package/2006/metadata/core-properties",
                            ),
                            "lastModifiedBy",
                        ) => Some("lastModifiedBy"),
                        _ => None,
                    };
                    if let Some(field) = field {
                        if seen_core.insert(field.to_owned()) {
                            selected = Some((field.to_owned(), String::new(), depth));
                        }
                    }
                }
                stack.push(name);
                namespaces.push(scope);
            }
            Event::Empty(element) => {
                let local_binding = element.local_name();
                let name = local_name(local_binding.as_ref())?;
                if stack.len() == 1 && custom_generator {
                    if unqualified_attribute(&element, reader.decoder(), b"name")?.as_deref()
                        == Some("generator")
                    {
                        // ElementTree's custom-properties loop records only non-empty values.
                    }
                } else if stack.len() == 1 && !custom_generator {
                    let mut scope = namespaces.last().cloned().unwrap_or_default();
                    extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                    let namespace = element_namespace(element.name().as_ref(), &scope);
                    let field = match (namespace, name) {
                        (Some("http://purl.org/dc/elements/1.1/"), "creator") => Some("creator"),
                        (
                            Some(
                                "http://schemas.openxmlformats.org/package/2006/metadata/core-properties",
                            ),
                            "lastModifiedBy",
                        ) => Some("lastModifiedBy"),
                        _ => None,
                    };
                    if let Some(field) = field {
                        seen_core.insert(field.to_owned());
                    }
                }
            }
            Event::Text(text) if selected.is_some() => {
                let text = text.xml10_content().map_err(|error| error.to_string())?;
                if let Some((_, value, _)) = selected.as_mut()
                    && (custom_generator || stack.len() == 2)
                {
                    value.push_str(&text);
                }
            }
            Event::CData(text) if selected.is_some() => {
                let text = text.xml10_content().map_err(|error| error.to_string())?;
                if let Some((_, value, _)) = selected.as_mut()
                    && (custom_generator || stack.len() == 2)
                {
                    value.push_str(&text);
                }
            }
            Event::GeneralRef(reference) if selected.is_some() => {
                let name = reference.decode().map_err(|error| error.to_string())?;
                let text = if let Some(value) = reference
                    .resolve_char_ref()
                    .map_err(|error| error.to_string())?
                {
                    value.to_string()
                } else {
                    quick_xml::escape::resolve_predefined_entity(&name)
                        .ok_or("DOCX property XML contains unknown entity")?
                        .to_owned()
                };
                if let Some((_, value, _)) = selected.as_mut()
                    && (custom_generator || stack.len() == 2)
                {
                    value.push_str(&text);
                }
            }
            Event::End(element) => {
                let local_binding = element.local_name();
                let name = local_name(local_binding.as_ref())?;
                if selected.is_some()
                    && selected
                        .as_ref()
                        .is_some_and(|(_, _, depth)| *depth == stack.len().saturating_sub(1))
                {
                    let (key, value, _) = selected.take().expect("selected property exists");
                    let value = scrub(&value);
                    if !value.is_empty() {
                        values.insert(key, value);
                    }
                }
                if stack.pop().as_deref() != Some(name) {
                    return Err("DOCX properties XML element stack mismatch".into());
                }
                namespaces.pop();
            }
            Event::DocType(_) => return Err("DOCX properties XML DTD is unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(values)
}

fn read_named_member(
    raw: &[u8],
    members: &[ZipMember],
    directory: ZipDirectory,
    name: &[u8],
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<Option<Vec<u8>>, String> {
    // `zipfile.ZipFile.read(name)` resolves duplicate names to the last entry in
    // the central-directory list. Do not inflate unrelated members: Python's
    // old consumer did not read or CRC-check an unused archive member either.
    let Some(member) = members.iter().rev().find(|member| member.name == name) else {
        return Ok(None);
    };
    extract_member(raw, member, directory.offset, check).map(Some)
}

const OPC_RELATIONSHIPS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OPC_CONTENT_TYPES_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const OFFICE_DOCUMENT_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const WORD_DOCUMENT_MAIN_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

#[derive(Clone, Debug)]
struct OpcRelationship {
    target: String,
    relation_type: String,
    external: bool,
}

fn relationship_element(
    element: &BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
    scope: &BTreeMap<String, String>,
) -> Result<Option<OpcRelationship>, String> {
    let local_binding = element.local_name();
    if local_name(local_binding.as_ref())? != "Relationship"
        || element_namespace(element.name().as_ref(), scope) != Some(OPC_RELATIONSHIPS_NS)
    {
        return Ok(None);
    }
    let target = unqualified_attribute(element, decoder, b"Target")?
        .filter(|value| !value.is_empty())
        .ok_or("OPC relationship Target is missing")?;
    let relation_type = unqualified_attribute(element, decoder, b"Type")?
        .filter(|value| !value.is_empty())
        .ok_or("OPC relationship Type is missing")?;
    let external =
        unqualified_attribute(element, decoder, b"TargetMode")?.as_deref() == Some("External");
    Ok(Some(OpcRelationship {
        target,
        relation_type,
        external,
    }))
}

fn parse_relationships_xml(
    raw: &[u8],
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<Vec<OpcRelationship>, String> {
    check(raw.len() as u64)?;
    let xml = decode_xml_bytes(raw)?;
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = true;
    let mut stack = Vec::<String>::new();
    let mut namespaces = vec![BTreeMap::<String, String>::new()];
    let mut relationships = Vec::new();
    let mut root_seen = false;
    let mut events = 0usize;
    loop {
        check(1)?;
        events += 1;
        if events > MAX_XML_EVENTS || stack.len() > MAX_XML_DEPTH {
            return Err("OPC relationships XML structural limit exceeded".into());
        }
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(element) => {
                let local_binding = element.local_name();
                let local = local_name(local_binding.as_ref())?.to_owned();
                let mut scope = namespaces.last().cloned().unwrap_or_default();
                extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                if stack.is_empty() {
                    if local != "Relationships"
                        || element_namespace(element.name().as_ref(), &scope)
                            != Some(OPC_RELATIONSHIPS_NS)
                    {
                        return Err("OPC relationship part has an unexpected root".into());
                    }
                    root_seen = true;
                } else if stack.len() == 1 {
                    if let Some(relationship) =
                        relationship_element(&element, reader.decoder(), &scope)?
                    {
                        relationships.push(relationship);
                    }
                }
                stack.push(local);
                namespaces.push(scope);
            }
            Event::Empty(element) => {
                let local_binding = element.local_name();
                let local = local_name(local_binding.as_ref())?;
                let mut scope = namespaces.last().cloned().unwrap_or_default();
                extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                if stack.is_empty() {
                    if local != "Relationships"
                        || element_namespace(element.name().as_ref(), &scope)
                            != Some(OPC_RELATIONSHIPS_NS)
                    {
                        return Err("OPC relationship part has an unexpected root".into());
                    }
                    root_seen = true;
                } else if stack.len() == 1 {
                    if let Some(relationship) =
                        relationship_element(&element, reader.decoder(), &scope)?
                    {
                        relationships.push(relationship);
                    }
                }
            }
            Event::End(element) => {
                let local_binding = element.local_name();
                let local = local_name(local_binding.as_ref())?;
                if stack.pop().as_deref() != Some(local) {
                    return Err("OPC relationships element stack mismatch".into());
                }
                namespaces.pop();
            }
            Event::DocType(_) => return Err("OPC relationships XML DTD is unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen || !stack.is_empty() {
        return Err("OPC relationships XML root is incomplete".into());
    }
    Ok(relationships)
}

fn normalized_partname(base_uri: &str, reference: &str) -> Result<String, String> {
    if reference.is_empty()
        || reference.contains('?')
        || reference.contains('#')
        || reference.contains('\\')
    {
        return Err("OPC relationship target is not a supported package part URI".into());
    }
    let joined = if reference.starts_with('/') {
        reference.to_owned()
    } else {
        format!("{}/{}", base_uri.trim_end_matches('/'), reference)
    };
    let mut components = Vec::<&str>::new();
    for component in joined.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err("OPC relationship target escapes the package root".into());
                }
            }
            value => components.push(value),
        }
    }
    if components.is_empty() {
        return Err("OPC relationship target names the package root".into());
    }
    Ok(format!("/{}", components.join("/")))
}

fn relationship_partname(partname: &str) -> String {
    if partname == "/" {
        "/_rels/.rels".into()
    } else {
        let (directory, filename) = partname.rsplit_once('/').unwrap_or(("", partname));
        format!("{directory}/_rels/{filename}.rels")
    }
}

fn parse_content_types_xml(
    raw: &[u8],
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>), String> {
    check(raw.len() as u64)?;
    let xml = decode_xml_bytes(raw)?;
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(false);
    reader.config_mut().check_end_names = true;
    let mut stack = Vec::<String>::new();
    let mut namespaces = vec![BTreeMap::<String, String>::new()];
    let mut overrides = BTreeMap::new();
    let mut defaults = BTreeMap::new();
    let mut root_seen = false;
    let mut events = 0usize;
    loop {
        check(1)?;
        events += 1;
        if events > MAX_XML_EVENTS || stack.len() > MAX_XML_DEPTH {
            return Err("OPC content types XML structural limit exceeded".into());
        }
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(element) => {
                let local_binding = element.local_name();
                let local = local_name(local_binding.as_ref())?.to_owned();
                let mut scope = namespaces.last().cloned().unwrap_or_default();
                extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                if stack.is_empty() {
                    if local != "Types"
                        || element_namespace(element.name().as_ref(), &scope)
                            != Some(OPC_CONTENT_TYPES_NS)
                    {
                        return Err("OPC content types part has an unexpected root".into());
                    }
                    root_seen = true;
                } else if stack.len() == 1
                    && element_namespace(element.name().as_ref(), &scope)
                        == Some(OPC_CONTENT_TYPES_NS)
                {
                    if local == "Override" {
                        let part = unqualified_attribute(&element, reader.decoder(), b"PartName")?
                            .filter(|value| !value.is_empty())
                            .ok_or("OPC content type Override has no PartName")?;
                        let value =
                            unqualified_attribute(&element, reader.decoder(), b"ContentType")?
                                .filter(|value| !value.is_empty())
                                .ok_or("OPC content type Override has no ContentType")?;
                        overrides.insert(part.to_ascii_lowercase(), value);
                    } else if local == "Default" {
                        let extension =
                            unqualified_attribute(&element, reader.decoder(), b"Extension")?
                                .filter(|value| !value.is_empty())
                                .ok_or("OPC content type Default has no Extension")?;
                        let value =
                            unqualified_attribute(&element, reader.decoder(), b"ContentType")?
                                .filter(|value| !value.is_empty())
                                .ok_or("OPC content type Default has no ContentType")?;
                        defaults.insert(extension.to_ascii_lowercase(), value);
                    }
                }
                stack.push(local);
                namespaces.push(scope);
            }
            Event::Empty(element) => {
                let local_binding = element.local_name();
                let local = local_name(local_binding.as_ref())?;
                let mut scope = namespaces.last().cloned().unwrap_or_default();
                extend_namespace_scope(&element, reader.decoder(), &mut scope)?;
                if stack.is_empty() {
                    if local != "Types"
                        || element_namespace(element.name().as_ref(), &scope)
                            != Some(OPC_CONTENT_TYPES_NS)
                    {
                        return Err("OPC content types part has an unexpected root".into());
                    }
                    root_seen = true;
                } else if stack.len() == 1
                    && element_namespace(element.name().as_ref(), &scope)
                        == Some(OPC_CONTENT_TYPES_NS)
                {
                    if local == "Override" {
                        let part = unqualified_attribute(&element, reader.decoder(), b"PartName")?
                            .filter(|value| !value.is_empty())
                            .ok_or("OPC content type Override has no PartName")?;
                        let value =
                            unqualified_attribute(&element, reader.decoder(), b"ContentType")?
                                .filter(|value| !value.is_empty())
                                .ok_or("OPC content type Override has no ContentType")?;
                        overrides.insert(part.to_ascii_lowercase(), value);
                    } else if local == "Default" {
                        let extension =
                            unqualified_attribute(&element, reader.decoder(), b"Extension")?
                                .filter(|value| !value.is_empty())
                                .ok_or("OPC content type Default has no Extension")?;
                        let value =
                            unqualified_attribute(&element, reader.decoder(), b"ContentType")?
                                .filter(|value| !value.is_empty())
                                .ok_or("OPC content type Default has no ContentType")?;
                        defaults.insert(extension.to_ascii_lowercase(), value);
                    }
                }
            }
            Event::End(element) => {
                let local_binding = element.local_name();
                let local = local_name(local_binding.as_ref())?;
                if stack.pop().as_deref() != Some(local) {
                    return Err("OPC content types element stack mismatch".into());
                }
                namespaces.pop();
            }
            Event::DocType(_) => return Err("OPC content types XML DTD is unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen || !stack.is_empty() {
        return Err("OPC content types XML root is incomplete".into());
    }
    Ok((overrides, defaults))
}

fn content_type_for<'a>(
    partname: &str,
    overrides: &'a BTreeMap<String, String>,
    defaults: &'a BTreeMap<String, String>,
) -> Result<&'a str, String> {
    if let Some(content_type) = overrides.get(&partname.to_ascii_lowercase()) {
        return Ok(content_type);
    }
    let extension = partname
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    defaults
        .get(&extension)
        .map(String::as_str)
        .ok_or_else(|| format!("OPC content type is missing for part {partname}"))
}

fn discover_main_document_part(
    raw: &[u8],
    members: &[ZipMember],
    directory: ZipDirectory,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<
    (
        String,
        Vec<u8>,
        BTreeMap<String, String>,
        BTreeMap<String, String>,
        Vec<OpcRelationship>,
    ),
    String,
> {
    let content_types = read_named_member(raw, members, directory, b"[Content_Types].xml", check)?
        .ok_or("DOCX [Content_Types].xml is missing")?;
    let (overrides, defaults) = parse_content_types_xml(&content_types, check)?;
    let root_rels = read_named_member(raw, members, directory, b"_rels/.rels", check)?
        .ok_or("DOCX package relationships are missing")?;
    let relationships = parse_relationships_xml(&root_rels, check)?;
    let office_documents = relationships
        .iter()
        .filter(|relationship| relationship.relation_type == OFFICE_DOCUMENT_REL)
        .collect::<Vec<_>>();
    if office_documents.len() != 1 || office_documents[0].external {
        return Err(
            "DOCX package must have exactly one internal office-document relationship".into(),
        );
    }
    let partname = normalized_partname("/", &office_documents[0].target)?;
    let content_type = content_type_for(&partname, &overrides, &defaults)?;
    if content_type != WORD_DOCUMENT_MAIN_TYPE {
        return Err(format!(
            "DOCX main part has unexpected content type {content_type}"
        ));
    }
    let membername = partname.trim_start_matches('/').as_bytes();
    let document_xml = read_named_member(raw, members, directory, membername, check)?
        .ok_or("DOCX office-document relationship target is missing")?;
    Ok((partname, document_xml, overrides, defaults, relationships))
}

fn validate_xml_part(
    raw: &[u8],
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    let xml = decode_xml_bytes(raw)?;
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().check_end_names = true;
    let mut stack = Vec::<Vec<u8>>::new();
    loop {
        check(1)?;
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(element) => stack.push(element.name().as_ref().to_vec()),
            Event::Empty(_) => {}
            Event::End(element) => {
                if stack.pop().as_deref() != Some(element.name().as_ref()) {
                    return Err("OPC XML part element stack mismatch".into());
                }
            }
            Event::DocType(_) => return Err("OPC XML part DTD is unsupported".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.is_empty() {
        Ok(())
    } else {
        Err("OPC XML part root is incomplete".into())
    }
}

fn validate_reachable_parts(
    raw: &[u8],
    members: &[ZipMember],
    directory: ZipDirectory,
    main_partname: &str,
    root_relationships: &[OpcRelationship],
    overrides: &BTreeMap<String, String>,
    defaults: &BTreeMap<String, String>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<(), String> {
    let mut pending = root_relationships
        .iter()
        .filter(|relationship| !relationship.external)
        .map(|relationship| normalized_partname("/", &relationship.target))
        .collect::<Result<Vec<_>, _>>()?;
    let mut visited = BTreeSet::<String>::new();
    while let Some(partname) = pending.pop() {
        check(1)?;
        if !visited.insert(partname.clone()) {
            continue;
        }
        let content_type = content_type_for(&partname, overrides, defaults)?;
        if partname != main_partname {
            let membername = partname.trim_start_matches('/').as_bytes();
            let bytes = read_named_member(raw, members, directory, membername, check)?
                .ok_or_else(|| format!("OPC relationship target is missing: {partname}"))?;
            if content_type.ends_with("+xml") || content_type == "application/xml" {
                validate_xml_part(&bytes, check)?;
            }
        }
        let rels_name = relationship_partname(&partname);
        if let Some(rels_xml) = read_named_member(
            raw,
            members,
            directory,
            rels_name.trim_start_matches('/').as_bytes(),
            check,
        )? {
            let relationships = parse_relationships_xml(&rels_xml, check)?;
            let base_uri = partname.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("/");
            for relationship in relationships
                .iter()
                .filter(|relationship| !relationship.external)
            {
                pending.push(normalized_partname(base_uri, &relationship.target)?);
            }
        }
    }
    Ok(())
}

pub fn parse_docx(
    raw: &[u8],
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<DocxDocument, String> {
    check(0)?;
    if raw.is_empty() || raw.len() > MAX_ARCHIVE_BYTES {
        return Err("DOCX archive outside byte limit".into());
    }
    let eocd = find_eocd(raw)?;
    let directory = directory_from_eocd(raw, eocd)?;
    let members = central_members(raw, directory)?;
    for _ in &members {
        check(1)?;
    }
    let signatures = members
        .iter()
        .filter(|member| member.name.starts_with(b"_xmlsignatures/"))
        .count();
    let (main_partname, document_xml, overrides, defaults, root_relationships) =
        discover_main_document_part(raw, &members, directory, check)?;
    validate_reachable_parts(
        raw,
        &members,
        directory,
        &main_partname,
        &root_relationships,
        &overrides,
        &defaults,
        check,
    )?;
    let (paragraphs, tables) = parse_document_xml(&document_xml, check)?;

    let mut metadata = DocxMetadata {
        signature_part_count: signatures,
        ..DocxMetadata::default()
    };
    if let Some(raw) = read_named_member(raw, &members, directory, b"docProps/core.xml", check)? {
        let values = parse_property_xml(&raw, false, check)?;
        metadata.creator = values.get("creator").cloned();
        metadata.last_modified_by = values.get("lastModifiedBy").cloned();
    }
    if let Some(raw) = read_named_member(raw, &members, directory, b"docProps/custom.xml", check)? {
        let values = parse_property_xml(&raw, true, check)?;
        metadata.custom_generator = values.get("custom_generator").cloned();
    }

    let mut digest = tos_foundation::Digest256Hasher::new();
    for chunk in raw.chunks(64 * 1024) {
        check(chunk.len() as u64)?;
        digest.update(chunk);
    }
    Ok(DocxDocument {
        paragraphs,
        tables,
        metadata,
        size_bytes: raw.len() as u64,
        sha256: digest.finalize().to_hex(),
    })
}

pub(crate) fn normalized_header_cell(value: &str) -> String {
    let value = python_lower(&scrub(value)).replace('ё', "е");
    value
        .split('/')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" / ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_headers(header: &[String]) -> Vec<String> {
    header
        .iter()
        .map(|value| normalized_header_cell(value))
        .collect()
}

fn same_header(values: &[String], expected: &[&str]) -> bool {
    values.len() == expected.len()
        && values
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

pub(crate) fn table_family(header: &[String]) -> &'static str {
    let values = normalized_headers(header);
    if values.len() == 6
        && values[0] == "node id"
        && matches!(values[1].as_str(), "тип узла" | "тип")
        && same_header(&values[2..4], &["название", "период"])
        && matches!(
            values[4].as_str(),
            "связи" | "связи / функция" | "основные связи" | "ключевые связи"
        )
        && matches!(values[5].as_str(), "приоритет" | "приор.")
    {
        return "proposed_nodes";
    }
    if (values.len() == 5
        && (same_header(&values[..3], &["source node", "relation", "target node"])
            || same_header(
                &values[..3],
                &["исходный узел", "отношение", "целевой узел"],
            ))
        && values[3] == "комментарий"
        && matches!(values[4].as_str(), "уверенность" | "увер." | "ув."))
        || (values.len() == 6
            && same_header(
                &values[..5],
                &["edge id", "source", "relation", "target", "комментарий"],
            )
            && matches!(values[5].as_str(), "уверенность" | "увер."))
    {
        return "proposed_relations";
    }
    let source_core = if values
        .first()
        .is_some_and(|value| matches!(value.as_str(), "id" | "код" | "маркер"))
    {
        &values[1..]
    } else {
        &values[..]
    };
    let source_labels = [
        "источник / корпус",
        "источник",
        "корпус / архив",
        "корпус / портал",
        "корпус",
        "id / источник",
    ];
    if matches!(source_core.len(), 4..=6)
        && source_labels.contains(&source_core[0].as_str())
        && (source_core[1..]
            .iter()
            .any(|value| value.starts_with("что дает"))
            || (source_core.iter().any(|value| value == "дата / слой")
                && source_core[1..]
                    .iter()
                    .any(|value| value.starts_with("тип"))))
    {
        return "corpus_or_edition_anchors";
    }
    if matches!(source_core.len(), 4 | 5)
        && matches!(source_core[0].as_str(), "источник" | "id / источник")
        && source_core[1..]
            .iter()
            .any(|value| value.starts_with("тип"))
        && source_core.iter().any(|value| value == "зачем нужен")
        && source_core[1..]
            .iter()
            .any(|value| value.contains("ограничен"))
    {
        return "control_or_review_anchors";
    }
    if matches!(values.len(), 2..=4)
        && matches!(values[0].as_str(), "риск" | "главный риск")
        && values[1..].iter().any(|value| value.contains("контрол"))
    {
        return "risk_control_source_needs";
    }
    if matches!(values.len(), 2..=4) && matches!(values[0].as_str(), "проблема" | "риск")
    {
        if values.len() == 2 && values[1] == "что контролировать" {
            return "risk_control_source_needs";
        }
        if values.len() == 3
            && matches!(values[1].as_str(), "почему существенен" | "в чем ловушка")
            && matches!(
                values[2].as_str(),
                "контроль" | "контроль tos" | "контроль в tos"
            )
        {
            return "risk_control_source_needs";
        }
        if values.len() == 4
            && matches!(values[1].as_str(), "в чем риск" | "риск")
            && matches!(
                values[2].as_str(),
                "как контролировать в tos" | "контроль в tos" | "контроль tos"
            )
            && matches!(
                values[3].as_str(),
                "какие источники нужны" | "нужные источники" | "что требуется"
            )
        {
            return "risk_control_source_needs";
        }
    }
    if values.len() == 5
        && same_header(&values[..2], &["термин", "язык"])
        && values[2].starts_with("транслитерация")
        && matches!(values[3].as_str(), "краткое значение" | "значение")
        && values[4] == "роль в tos"
    {
        return "terms";
    }
    if values.len() == 5
        && matches!(
            values[0].as_str(),
            "источник / предыдущий узел" | "источник / previous node"
        )
        && values[1] == "что передано"
        && matches!(values[2].as_str(), "канал передачи" | "канал")
        && same_header(&values[3..], &["уверенность", "примечание"])
    {
        return "incoming_transmissions";
    }
    if values.len() == 5
        && same_header(
            &values[..4],
            &[
                "следующий узел / эпоха",
                "что передается",
                "канал",
                "уверенность",
            ],
        )
        && matches!(
            values[4].as_str(),
            "что проверить дальше" | "проверить дальше"
        )
    {
        return "outgoing_transmissions";
    }
    if values.len() == 2
        && matches!(values[0].as_str(), "поле" | "параметр")
        && matches!(values[1].as_str(), "значение" | "идентификация")
    {
        return if values[0] == "поле" {
            "dossier_identity_metadata"
        } else {
            "dossier_identity_metadata_alias"
        };
    }
    if values.len() == 6
        && matches!(
            values[0].as_str(),
            "корпус / текст" | "корпус / текст / артефакт" | "корпус / артефакт"
        )
        && matches!(values[1].as_str(), "дата / слой" | "дата")
        && values[2] == "язык"
        && matches!(values[3].as_str(), "жанр" | "жанр / жанры")
        && values[4] == "сохранность"
        && matches!(
            values[5].as_str(),
            "почему важен для tos" | "значение tos" | "tos-функция"
        )
    {
        return "corpora_texts_artifacts";
    }
    if values.len() == 5
        && matches!(
            values[0].as_str(),
            "фигура / тип авторства" | "фигура / тип"
        )
        && same_header(&values[1..3], &["период", "роль"])
        && (values[3] == "связанные тексты" || values[3].starts_with("связанные тексты / "))
        && values[4] == "уверенность"
    {
        return "figures_authorship";
    }
    if values.len() == 5
        && matches!(
            values[0].as_str(),
            "язык / письменность / медиум" | "язык / письмо / медиум"
        )
        && matches!(values[1].as_str(), "роль в строке" | "роль")
        && same_header(&values[2..], &["период", "что сохранилось", "риск"])
    {
        return "language_script_medium";
    }
    if values.len() == 2
        && matches!(values[0].as_str(), "уровень" | "уровень оценки")
        && matches!(values[1].as_str(), "оценка" | "вывод")
    {
        return "audit_levels";
    }
    if same_header(&values, &["измерение", "граница"]) {
        return "boundary_dimensions";
    }
    if same_header(
        &values,
        &["жанр", "функция", "примеры", "философская значимость"],
    ) {
        return "genres";
    }
    "other_context"
}

fn structured_header_aliases(family: &str) -> &'static [&'static str] {
    match family {
        "proposed_nodes" => &[
            "Node ID",
            "Тип узла",
            "Тип",
            "Название",
            "Период",
            "Связи",
            "Связи / функция",
            "Основные связи",
            "Ключевые связи",
            "Приоритет",
            "Приор.",
        ],
        "proposed_relations" => &[
            "Edge ID",
            "Source node",
            "Source",
            "Исходный узел",
            "Relation",
            "Отношение",
            "Target node",
            "Target",
            "Целевой узел",
            "Комментарий",
            "Уверенность",
            "Увер.",
            "Ув.",
        ],
        "corpus_or_edition_anchors" => &[
            "ID",
            "Код",
            "Маркер",
            "Источник / корпус",
            "Источник",
            "Корпус / архив",
            "Корпус / портал",
            "Корпус",
            "ID / источник",
            "Тип",
            "Тип / дата",
            "Тип / содержание",
            "Дата / слой",
            "Что даёт",
            "Что даёт ToS",
            "Что даёт / где искать",
            "Что даёт / доступ",
            "Доступ / где искать",
            "Доступ",
            "Доступ / stable URL",
            "Доступ / надёжность",
            "Доступ / замечание",
            "Ссылка",
            "URL / DOI",
            "Надёжность",
            "Надёжность / ограничение",
            "Надёжность / ограничения",
            "Надёжность и ограничения",
            "Надёжность / caveat",
            "Ограничения",
        ],
        "control_or_review_anchors" => &[
            "ID",
            "Код",
            "Маркер",
            "Источник",
            "ID / источник",
            "Тип",
            "Тип / дата",
            "Тип / содержание",
            "Зачем нужен",
            "Ограничения",
            "Ограничение",
            "Ограничения / контроль",
            "Ограничения / доступ",
            "Доступ / где искать",
            "Доступ",
            "Доступ / stable URL",
            "Ссылка",
            "URL / DOI",
        ],
        "risk_control_source_needs" => &[
            "Проблема",
            "Риск",
            "Главный риск",
            "В чём риск",
            "В чём опасность",
            "Почему критичен",
            "Почему опасен",
            "В чём искажение",
            "Как искажает строку",
            "Почему возникает",
            "Почему критичен для T2-51",
            "Почему существенен",
            "Проявление",
            "В чём ловушка",
            "Как контролировать в ToS",
            "Контроль в ToS",
            "Контроль ToS",
            "Контроль",
            "Что контролировать",
            "Что именно контролировать",
            "Контрольный принцип",
            "Какие источники нужны",
            "Нужные источники",
            "Что требуется",
        ],
        "terms" => &[
            "Термин",
            "Язык",
            "Транслитерация",
            "Транслитерация / форма",
            "Транслитерация / перевод",
            "Транслитерация / аббр.",
            "Краткое значение",
            "Значение",
            "Роль в ToS",
        ],
        "incoming_transmissions" => &[
            "Источник / предыдущий узел",
            "Источник / previous node",
            "Что передано",
            "Канал передачи",
            "Канал",
            "Уверенность",
            "Примечание",
        ],
        "outgoing_transmissions" => &[
            "Следующий узел / эпоха",
            "Что передаётся",
            "Канал",
            "Уверенность",
            "Что проверить дальше",
            "Проверить дальше",
        ],
        _ => &[],
    }
}

fn structured_header_is_mapped(family: &str, header: &[String]) -> bool {
    let aliases = structured_header_aliases(family)
        .iter()
        .map(|alias| normalized_header_cell(alias))
        .collect::<BTreeSet<_>>();
    header
        .iter()
        .all(|cell| aliases.contains(&normalized_header_cell(cell)))
}

pub(crate) fn table_body_rows(table: &DocxTable) -> Vec<(usize, &[String])> {
    table
        .rows
        .iter()
        .skip(1)
        .filter(|row| row.iter().any(|cell| !scrub(cell).is_empty()))
        .enumerate()
        .map(|(index, row)| (index + 1, row.as_slice()))
        .collect()
}

pub(crate) fn row_value(header: &[String], cells: &[String], aliases: &[&str]) -> String {
    for alias in aliases {
        let wanted = normalized_header_cell(alias);
        if let Some(index) = header
            .iter()
            .rposition(|cell| normalized_header_cell(cell) == wanted)
        {
            if let Some(value) = cells.get(index) {
                let value = scrub(value);
                if !value.is_empty() {
                    return value;
                }
            }
        }
    }
    String::new()
}

fn structured_original_node_id(header: &[String], cells: &[String]) -> String {
    row_value(header, cells, &["Node ID"])
}

fn dossier_id_matches(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut matches = Vec::new();
    for start in 0..bytes.len() {
        let token_len = if bytes[start] == b'A'
            && bytes.get(start + 1).is_some_and(u8::is_ascii_digit)
            && bytes.get(start + 2).is_some_and(u8::is_ascii_digit)
        {
            3
        } else if bytes[start] == b'T'
            && matches!(bytes.get(start + 1), Some(b'2' | b'3'))
            && bytes.get(start + 2) == Some(&b'-')
            && bytes.get(start + 3).is_some_and(u8::is_ascii_digit)
            && bytes.get(start + 4).is_some_and(u8::is_ascii_digit)
        {
            5
        } else {
            continue;
        };
        let before_is_alnum = start > 0 && bytes[start - 1].is_ascii_alphanumeric();
        let after_is_digit = bytes.get(start + token_len).is_some_and(u8::is_ascii_digit);
        if !before_is_alnum && !after_is_digit {
            matches.push(text[start..start + token_len].to_owned());
        }
    }
    matches
}

fn normalized_row_to_expand(value: &str, dossier_id: &str) -> Option<String> {
    if let Some(found) = dossier_id_matches(&scrub(value)).first() {
        return Some(found.clone());
    }
    let value = scrub(value);
    let digits = if let Some((number, rest)) = value.split_once('(') {
        if !rest.ends_with(')') || rest[..rest.len() - 1].contains(')') {
            return None;
        }
        number.trim()
    } else {
        value.as_str()
    };
    if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let prefix = dossier_id
        .split_once('-')
        .map_or(dossier_id, |(prefix, _)| prefix);
    Some(format!("{prefix}-{:02}", digits.parse::<u32>().ok()?))
}

fn normalized_title(value: &str) -> String {
    let value = python_lower(value).replace('ё', "е").replace('ā', "a");
    let mut output = String::with_capacity(value.len());
    let mut space = false;
    for character in value.chars() {
        let allowed = character.is_ascii_alphanumeric() || ('а'..='я').contains(&character);
        if allowed {
            output.push(character);
            space = false;
        } else if !space {
            output.push(' ');
            space = true;
        }
    }
    output.trim().to_owned()
}

fn clean_title_prefix(value: &str, prefix: &str, underscores: bool) -> String {
    let Some(rest) = value
        .get(..prefix.len())
        .filter(|candidate| candidate.eq_ignore_ascii_case(prefix))
    else {
        return value.to_owned();
    };
    let _ = rest;
    let remainder = &value[prefix.len()..];
    let cleaned = remainder.trim_start_matches(|character: char| {
        character.is_whitespace()
            || matches!(character, ':' | '—' | '-')
            || (underscores && character == '_')
    });
    cleaned.to_owned()
}

fn clean_dossier_title(title: &str, dossier_id: &str) -> String {
    let value = scrub(title);
    let value = clean_title_prefix(&value, "ToS Deep Research", true);
    clean_title_prefix(&value, dossier_id, false)
        .trim()
        .to_owned()
}

fn issue(message: impl Into<String>) -> Vec<DocxValidationIssue> {
    vec![DocxValidationIssue {
        code: "docx_content_invalid".into(),
        message: message.into(),
        blocking: true,
    }]
}

fn json_truthy_string(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(value)) => value.clone(),
        Some(serde_json::Value::Bool(true)) => "True".into(),
        Some(serde_json::Value::Bool(false) | serde_json::Value::Null) | None => String::new(),
        Some(serde_json::Value::Number(number)) if number.as_f64() == Some(0.0) => String::new(),
        Some(value) => value.to_string(),
    }
}

pub fn validate_identity_and_headers(
    document: &DocxDocument,
    table_id: &str,
    dossier_id: &str,
    master_row: &serde_json::Value,
    route: Option<&serde_json::Value>,
    blocked: Option<&serde_json::Value>,
    check: &mut dyn FnMut(u64) -> Result<(), String>,
) -> Result<DocxValidation, Vec<DocxValidationIssue>> {
    check(master_row.to_string().len() as u64)
        .map_err(|error| issue(format!("{dossier_id} readiness budget refused: {error}")))?;
    if master_row.get("row_id").and_then(serde_json::Value::as_str) != Some(dossier_id)
        || master_row
            .get("table_id")
            .and_then(serde_json::Value::as_str)
            != Some(table_id)
    {
        return Err(issue(format!(
            "{dossier_id} does not match its {table_id} master row"
        )));
    }
    if route.is_some() == blocked.is_some() {
        return Err(issue(format!(
            "{dossier_id} must be either routed or explicitly blocked"
        )));
    }
    let normalized_master = master_row
        .get("normalized")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            issue(format!(
                "{dossier_id} master row must expose normalized metadata"
            ))
        })?;
    let title = document
        .paragraphs
        .iter()
        .find(|paragraph| !scrub(paragraph).is_empty())
        .cloned()
        .unwrap_or_else(|| format!("ToS Deep Research: {dossier_id}"));
    let mut report = DocxValidation {
        title: title.clone(),
        table_row: dossier_id.to_owned(),
        metadata_identity_posture: "master_table_identity_fallback".into(),
        paragraph_count: document.paragraphs.len(),
        table_count: document.tables.len(),
        ..DocxValidation::default()
    };
    let overrides = match route.and_then(|value| value.get("reviewed_node_label_overrides")) {
        None => serde_json::Map::new(),
        Some(serde_json::Value::Object(values)) => values.clone(),
        Some(_) => {
            return Err(issue(format!(
                "{dossier_id} reviewed_node_label_overrides must be an object"
            )));
        }
    };
    let mut applied_overrides = BTreeSet::<String>::new();
    let mut observed_table_value = String::new();
    let mut observed_row_value = String::new();
    let metadata_families = [
        "dossier_identity_metadata",
        "dossier_identity_metadata_alias",
    ];
    let structured_families = [
        "proposed_nodes",
        "proposed_relations",
        "corpus_or_edition_anchors",
        "control_or_review_anchors",
        "risk_control_source_needs",
        "terms",
        "incoming_transmissions",
        "outgoing_transmissions",
    ];
    for table in &document.tables {
        let table_bytes = table
            .rows
            .iter()
            .flatten()
            .try_fold(1u64, |total, cell| total.checked_add(cell.len() as u64))
            .ok_or_else(|| issue(format!("{dossier_id} DOCX table work accounting overflow")))?;
        check(table_bytes)
            .map_err(|error| issue(format!("{dossier_id} readiness budget refused: {error}")))?;
        let header = table.rows.first().cloned().unwrap_or_default();
        let family = table_family(&header);
        if blocked.is_none()
            && structured_families.contains(&family)
            && !structured_header_is_mapped(family, &header)
        {
            let unmapped = header
                .iter()
                .filter(|cell| {
                    !structured_header_aliases(family)
                        .iter()
                        .any(|alias| normalized_header_cell(alias) == normalized_header_cell(cell))
                })
                .cloned()
                .collect::<Vec<_>>();
            return Err(issue(format!(
                "{dossier_id} structured {family} table has unmapped headers: {unmapped:?}"
            )));
        }
        if !metadata_families.contains(&family) {
            if family == "proposed_nodes" && !overrides.is_empty() {
                for (row_index, cells) in table_body_rows(table) {
                    check(1).map_err(|error| {
                        issue(format!("{dossier_id} readiness budget refused: {error}"))
                    })?;
                    let original_id = {
                        let original_id = structured_original_node_id(&header, cells);
                        if original_id.is_empty() {
                            format!("{dossier_id}-node-{row_index:03}")
                        } else {
                            original_id
                        }
                    };
                    let Some(override_value) = overrides.get(&original_id) else {
                        continue;
                    };
                    let Some(override_value) = override_value.as_object() else {
                        return Err(issue(format!(
                            "{dossier_id} node label override for {original_id} must be an object"
                        )));
                    };
                    let label = scrub(&json_truthy_string(override_value.get("label")));
                    let reason = scrub(&json_truthy_string(override_value.get("reason")));
                    if label.is_empty() || reason.is_empty() {
                        return Err(issue(format!(
                            "{dossier_id} node label override for {original_id} requires label and reason"
                        )));
                    }
                    applied_overrides.insert(original_id);
                }
            }
            continue;
        }
        report.metadata_headers.push(header.clone());
        for (_, cells) in table_body_rows(table) {
            check(1).map_err(|error| {
                issue(format!("{dossier_id} readiness budget refused: {error}"))
            })?;
            let field_name = row_value(&header, cells, &["Поле", "Параметр"]);
            let field_value = row_value(&header, cells, &["Значение", "Идентификация"]);
            if field_name == "ROW_TO_EXPAND" && !field_value.is_empty() {
                observed_row_value = field_value;
            } else if field_name == "Таблица" && !field_value.is_empty() {
                observed_table_value = field_value;
            }
        }
    }
    let unapplied = overrides
        .keys()
        .filter(|key| !applied_overrides.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    if !unapplied.is_empty() {
        return Err(issue(format!(
            "{dossier_id} node label overrides did not match extracted node ids: {unapplied:?}"
        )));
    }
    if !observed_row_value.is_empty() {
        let observed_id = normalized_row_to_expand(&observed_row_value, dossier_id);
        if observed_id.as_deref() != Some(dossier_id) {
            return Err(issue(format!(
                "{dossier_id} DOCX ROW_TO_EXPAND does not match its master-table identity"
            )));
        }
        report.table_row = observed_row_value.clone();
    }
    if !observed_table_value.is_empty() {
        let accepted = match table_id {
            "table-i" => ["I", "Table I", "Таблица I"].as_slice(),
            "table-ii" => ["II", "Table II", "Таблица II"].as_slice(),
            "table-iii" => [
                "III",
                "Table III",
                "Таблица III",
                "III — модерность и современность",
                "III — модерность и современность как сеть письменных инфраструктур",
                "Таблица III — модерность и современность",
            ]
            .as_slice(),
            _ => [].as_slice(),
        };
        if !accepted.contains(&observed_table_value.as_str()) {
            return Err(issue(format!(
                "{dossier_id} DOCX table identity is not {table_id}: {observed_table_value}"
            )));
        }
    }
    report.metadata_identity_posture = match (
        observed_row_value.is_empty(),
        observed_table_value.is_empty(),
    ) {
        (false, false) => "docx_metadata_cross_checked",
        (false, true) | (true, false) => "partial_docx_metadata_cross_checked",
        (true, true) => "master_table_identity_fallback",
    }
    .into();

    if table_id == "table-i" && observed_row_value.is_empty() {
        let ids = dossier_id_matches(&title);
        let observed_title_id = ids.first();
        if let Some(observed_title_id) = observed_title_id {
            if observed_title_id != dossier_id {
                return Err(issue(format!(
                    "{dossier_id} DOCX title does not match its master-table identity: {observed_title_id}"
                )));
            }
        } else {
            let expected_title =
                json_truthy_string(route.and_then(|value| value.get("accepted_input_title")));
            if expected_title.is_empty()
                || normalized_title(&title) != normalized_title(&expected_title)
            {
                return Err(issue(format!(
                    "{dossier_id} DOCX title does not match its reviewed Table I route: {title:?} != {expected_title:?}"
                )));
            }
            report.metadata_identity_posture = "filename_title_reviewed_route_cross_checked".into();
        }
    }
    if matches!(table_id, "table-ii" | "table-iii") {
        let clean_title = clean_dossier_title(&title, dossier_id);
        if let Some(blocked) = blocked {
            let expected = json_truthy_string(blocked.get("observed_input_title"));
            if normalized_title(&clean_title) != normalized_title(&expected) {
                return Err(issue(format!(
                    "{dossier_id} blocked artifact title changed; review quarantine before planting"
                )));
            }
            report
                .identity_diagnostics
                .push("master_identity_mismatch_quarantined".into());
            report.metadata_identity_posture =
                "filename_and_artifact_title_recorded_master_mismatch".into();
        } else if let Some(route) = route {
            let expected_title = if route
                .as_object()
                .is_some_and(|object| object.contains_key("accepted_input_title"))
            {
                let expected = scrub(&json_truthy_string(route.get("accepted_input_title")));
                if normalized_title(&expected).is_empty() {
                    return Err(issue(format!(
                        "{dossier_id} accepted_input_title must be non-empty"
                    )));
                }
                expected
            } else {
                json_truthy_string(normalized_master.get("research_node"))
            };
            let normalized_expected = normalized_title(&expected_title);
            let body_text = normalized_title(
                &document
                    .paragraphs
                    .iter()
                    .skip(1)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            let title_matches = normalized_title(&clean_title) == normalized_expected
                || (!normalized_expected.is_empty() && body_text.contains(&normalized_expected));
            if !title_matches {
                return Err(issue(format!(
                    "{dossier_id} DOCX title does not match its reviewed {table_id} route: {clean_title:?} != {expected_title:?}"
                )));
            }
            if observed_row_value.is_empty() && observed_table_value.is_empty() {
                report.metadata_identity_posture = "filename_title_master_cross_checked".into();
            }
        }
    }
    Ok(report)
}

/// Python scrub uses a narrow citation marker and then Unicode whitespace collapse.
pub(crate) fn scrub(text: &str) -> String {
    let marker = "\u{e200}filecite\u{e202}";
    let end_marker = '\u{e201}';
    let mut without_citations = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(relative) = text[cursor..].find(marker) {
        let start = cursor + relative;
        without_citations.push_str(&text[cursor..start]);
        let content_start = start + marker.len();
        let stop = text[content_start..]
            .find(|ch| ch == ' ' || ch == '\n' || ch == '\t')
            .map(|relative| content_start + relative)
            .unwrap_or(text.len());
        if let Some(end) = text[content_start..stop].rfind(end_marker) {
            cursor = content_start + end + end_marker.len_utf8();
        } else {
            without_citations.push_str(marker);
            cursor = content_start;
        }
    }
    without_citations.push_str(&text[cursor..]);

    let mut out = String::with_capacity(without_citations.len());
    let mut whitespace = false;
    for ch in without_citations.replace('\u{00a0}', " ").chars() {
        if ch.is_whitespace() {
            if !whitespace {
                out.push(' ');
                whitespace = true;
            }
        } else {
            out.push(ch);
            whitespace = false;
        }
    }
    out.trim().to_owned()
}

/// Shared bounded OOXML container access. Registry readers retain their own
/// XML and normalization rules; DOCX content assessment remains separate.
pub(crate) struct OfficeArchive<'a> {
    raw: &'a [u8],
    directory: ZipDirectory,
    members: Vec<ZipMember>,
}
impl<'a> OfficeArchive<'a> {
    pub(crate) fn open(
        raw: &'a [u8],
        check: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<Self, String> {
        check(raw.len() as u64)?;
        if raw.is_empty() || raw.len() > MAX_ARCHIVE_BYTES {
            return Err("OOXML archive outside byte limit".into());
        }
        let directory = directory_from_eocd(raw, find_eocd(raw)?)?;
        let members = central_members(raw, directory)?;
        for member in &members {
            check(member.name.len() as u64)?;
            if member.uncompressed_bytes > MAX_ARCHIVE_BYTES as u64 {
                return Err("OOXML part outside decoded byte limit".into());
            }
        }
        Ok(Self {
            raw,
            directory,
            members,
        })
    }
    pub(crate) fn names(&self) -> Result<Vec<&str>, String> {
        self.members
            .iter()
            .map(|m| std::str::from_utf8(&m.name).map_err(|e| e.to_string()))
            .collect()
    }
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.members.iter().any(|m| m.name == name.as_bytes())
    }
    pub(crate) fn read(
        &self,
        name: &str,
        check: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<Vec<u8>, String> {
        read_named_member(
            self.raw,
            &self.members,
            self.directory,
            name.as_bytes(),
            check,
        )?
        .ok_or_else(|| format!("missing OOXML part: {name}"))
    }
    pub(crate) fn xml_text(
        &self,
        name: &str,
        check: &mut dyn FnMut(u64) -> Result<(), String>,
    ) -> Result<String, String> {
        decode_xml_bytes(&self.read(name, check)?)
    }
}
