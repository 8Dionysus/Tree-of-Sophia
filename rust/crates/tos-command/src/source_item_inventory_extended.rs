//! Native continuation of the source resource inventory owner.
//!
//! This child module owns the legacy deterministic file profiles while the
//! parent module owns Item observation and the current registry JSON/XML
//! profiles. It emits navigation metadata only and grants no source or rights
//! authority.

use super::*;
use flate2::read::MultiGzDecoder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

const MAX_PROFILE_BYTES: u64 = 300 * 1024 * 1024;
const MAX_EXTERNAL_STDOUT: usize = 16 * 1024 * 1024;
const MAX_XML_EVENTS: usize = 262_144;
const MAX_XML_DEPTH: usize = 128;
const MAX_PLAIN_UTF8_BYTES: usize = 131_072;
const LEGACY_AUTHORITY_BOUNDARY_V1: &str = "resource enumeration, geometry, ordering, counts, and one-way fingerprints only; no source text, bibliographic acceptance, textual acceptance, rights clearance, translation, semantics, or canon authority";
const ABBYY_NAMESPACE: &str = "http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml";

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    super::active(deadline, cancelled)
}

fn file_field<'a>(entry: &'a Value, key: &str) -> SourceCommandResult<&'a str> {
    super::inventory_json_text(entry, key)
}

fn profile_error(message: &'static str) -> SourceCommandError {
    SourceCommandError::Unsupported(message)
}

fn profile_value(entry: &Value, profile: &str, summary: Value, resources: Vec<Value>) -> Value {
    json!({"file_id":entry["file_id"],"file_sha256":entry["sha256"],
        "media_type":entry["media_type"],"profile":profile,"summary":summary,
        "resources":resources})
}

fn descendants<'a>(root: &'a InventoryXmlNode, out: &mut Vec<&'a InventoryXmlNode>) {
    for child in &root.children {
        out.push(child);
        descendants(child, out);
    }
}

fn direct_child<'a>(node: &'a InventoryXmlNode, name: &str) -> Option<&'a InventoryXmlNode> {
    node.children
        .iter()
        .find(|child| child.namespace.is_none() && child.local_name == name)
}

fn direct_text<'a>(node: &'a InventoryXmlNode, name: &str) -> Option<&'a str> {
    direct_child(node, name).map(|child| child.text_content.as_str())
}

fn parse_u32(text: &str, message: &'static str) -> SourceCommandResult<u32> {
    text.trim()
        .parse::<u32>()
        .map_err(|_| SourceCommandError::Invalid(message))
}

fn distinct_geometry_count(resources: &[Value]) -> usize {
    resources
        .iter()
        .filter_map(|resource| {
            let locator = resource.get("locator")?;
            Some((
                locator["width_pixels"].as_u64()?,
                locator["height_pixels"].as_u64()?,
                locator["resolution_dpi"].as_u64()?,
            ))
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn scandata_inventory(raw: &[u8], entry: &Value) -> SourceCommandResult<Value> {
    let root = super::inventory_xml_tree(raw)?;
    if root.local_name != "book" {
        return Err(SourceCommandError::Invalid("scandata XML root"));
    }
    let book_data = direct_child(&root, "bookData")
        .ok_or(SourceCommandError::Invalid("scandata bookData missing"))?;
    let leaf_count = parse_u32(
        direct_text(book_data, "leafCount")
            .ok_or(SourceCommandError::Invalid("scandata leafCount missing"))?,
        "scandata leafCount invalid",
    )? as usize;
    let dpi = parse_u32(
        direct_text(book_data, "dpi").ok_or(SourceCommandError::Invalid("scandata dpi missing"))?,
        "scandata dpi invalid",
    )?;
    let page_data = direct_child(&root, "pageData")
        .ok_or(SourceCommandError::Invalid("scandata pageData missing"))?;
    let mut resources = Vec::new();
    for page in page_data
        .children
        .iter()
        .filter(|page| page.namespace.is_none() && page.local_name == "page")
    {
        let page_index = resources.len() + 1;
        let leaf_number = page
            .attributes
            .get("leafNum")
            .ok_or(SourceCommandError::Invalid("scandata leafNum missing"))?
            .parse::<usize>()
            .map_err(|_| SourceCommandError::Invalid("scandata leafNum invalid"))?;
        if leaf_number != page_index - 1 {
            return Err(SourceCommandError::Invalid(
                "scandata leaf numbering is not contiguous",
            ));
        }
        let width = parse_u32(
            direct_text(page, "origWidth")
                .ok_or(SourceCommandError::Invalid("scandata width missing"))?,
            "scandata width invalid",
        )?;
        let height = parse_u32(
            direct_text(page, "origHeight")
                .ok_or(SourceCommandError::Invalid("scandata height missing"))?,
            "scandata height invalid",
        )?;
        resources.push(
            json!({"resource_id":format!("scandata-page-{page_index:04}"),
            "resource_kind":"scan_data_page","locator":{"page_index":page_index,
                "leaf_number":leaf_number,"width_pixels":width,"height_pixels":height,
                "resolution_dpi":dpi},"structural_role":"page"}),
        );
    }
    if resources.is_empty() || resources.len() != leaf_count {
        return Err(SourceCommandError::Invalid("scandata leaf count differs"));
    }
    let geometries = distinct_geometry_count(&resources);
    Ok(profile_value(
        entry,
        "scandata_pages_v1",
        json!({"resource_count":resources.len(),"page_count":resources.len(),
            "distinct_page_geometry_count":geometries}),
        resources,
    ))
}

fn djvu_xml_inventory(raw: &[u8], entry: &Value) -> SourceCommandResult<Value> {
    let root = super::inventory_xml_tree(raw)?;
    if root.local_name != "DjVuXML" {
        return Err(SourceCommandError::Invalid("DjVu XML root"));
    }
    let mut nodes = Vec::new();
    descendants(&root, &mut nodes);
    let mut resources = Vec::new();
    for object in nodes
        .into_iter()
        .filter(|node| node.namespace.is_none() && node.local_name == "OBJECT")
    {
        let page_index = resources.len() + 1;
        let width = object
            .attributes
            .get("width")
            .ok_or(SourceCommandError::Invalid("DjVu page width missing"))?
            .parse::<u32>()
            .map_err(|_| SourceCommandError::Invalid("DjVu page width invalid"))?;
        let height = object
            .attributes
            .get("height")
            .ok_or(SourceCommandError::Invalid("DjVu page height missing"))?
            .parse::<u32>()
            .map_err(|_| SourceCommandError::Invalid("DjVu page height invalid"))?;
        let dpi = object
            .children
            .iter()
            .find(|child| {
                child.namespace.is_none()
                    && child.local_name == "PARAM"
                    && child
                        .attributes
                        .get("name")
                        .is_some_and(|value| value == "DPI")
            })
            .and_then(|parameter| parameter.attributes.get("value"))
            .ok_or(SourceCommandError::Invalid("DjVu page DPI missing"))?
            .parse::<u32>()
            .map_err(|_| SourceCommandError::Invalid("DjVu page DPI invalid"))?;
        let mut page_nodes = Vec::new();
        descendants(object, &mut page_nodes);
        let paragraphs = page_nodes
            .iter()
            .filter(|node| node.namespace.is_none() && node.local_name == "PARAGRAPH")
            .count();
        let lines = page_nodes
            .iter()
            .filter(|node| node.namespace.is_none() && node.local_name == "LINE")
            .count();
        let words = page_nodes
            .iter()
            .filter(|node| node.namespace.is_none() && node.local_name == "WORD")
            .collect::<Vec<_>>();
        let page_text = words
            .iter()
            .map(|word| word.text_content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        resources.push(
            json!({"resource_id":format!("djvu-ocr-page-{page_index:04}"),
            "resource_kind":"ocr_page","locator":{"page_index":page_index,
                "width_pixels":width,"height_pixels":height,"resolution_dpi":dpi},
            "structural_role":"page","paragraph_count":paragraphs,"line_count":lines,
            "word_count":words.len(),"content_fingerprint":super::fingerprint(&[page_text])}),
        );
    }
    if resources.is_empty() {
        return Err(SourceCommandError::Invalid("DjVu XML has no OBJECT pages"));
    }
    let paragraph_count = resources
        .iter()
        .map(|row| row["paragraph_count"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let line_count = resources
        .iter()
        .map(|row| row["line_count"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let word_count = resources
        .iter()
        .map(|row| row["word_count"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let geometries = distinct_geometry_count(&resources);
    Ok(profile_value(
        entry,
        "djvu_xml_pages_v1",
        json!({"resource_count":resources.len(),
        "page_count":resources.len(),"paragraph_count":paragraph_count,"line_count":line_count,
        "word_count":word_count,"distinct_page_geometry_count":geometries}),
        resources,
    ))
}

fn chunk_bounds(
    data: &[u8],
    offset: usize,
    parent_end: usize,
) -> SourceCommandResult<(usize, usize)> {
    let header_end = offset
        .checked_add(8)
        .ok_or(SourceCommandError::Invalid("DjVu chunk offset"))?;
    if header_end > parent_end || header_end > data.len() {
        return Err(SourceCommandError::Invalid(
            "DjVu chunk header exceeds container",
        ));
    }
    let size = u32::from_be_bytes(data[offset + 4..header_end].try_into().unwrap()) as usize;
    let start = header_end;
    let end = start
        .checked_add(size)
        .ok_or(SourceCommandError::Invalid("DjVu chunk size overflow"))?;
    if end > parent_end || end > data.len() {
        return Err(SourceCommandError::Invalid("DjVu chunk exceeds container"));
    }
    Ok((start, end))
}

fn djvu_page_info(data: &[u8], form_offset: usize) -> SourceCommandResult<(u32, u32, u32)> {
    if data.get(form_offset..form_offset + 4) != Some(b"FORM") {
        return Err(SourceCommandError::Invalid("DjVu page FORM missing"));
    }
    let form_size =
        u32::from_be_bytes(data[form_offset + 4..form_offset + 8].try_into().unwrap()) as usize;
    let form_end = form_offset
        .checked_add(8)
        .and_then(|v| v.checked_add(form_size))
        .ok_or(SourceCommandError::Invalid("DjVu page FORM size overflow"))?;
    if form_end > data.len() || data.get(form_offset + 8..form_offset + 12) != Some(b"DJVU") {
        return Err(SourceCommandError::Invalid("DjVu page FORM identity"));
    }
    let mut cursor = form_offset + 12;
    while cursor + 8 <= form_end {
        let kind = &data[cursor..cursor + 4];
        let (start, end) = chunk_bounds(data, cursor, form_end)?;
        if kind == b"INFO" {
            if end - start < 10 {
                return Err(SourceCommandError::Invalid("DjVu INFO chunk is short"));
            }
            let width = u16::from_be_bytes(data[start..start + 2].try_into().unwrap()) as u32;
            let height = u16::from_be_bytes(data[start + 2..start + 4].try_into().unwrap()) as u32;
            let dpi = u16::from_le_bytes(data[start + 6..start + 8].try_into().unwrap()) as u32;
            if width == 0 || height == 0 || dpi == 0 {
                return Err(SourceCommandError::Invalid("DjVu INFO geometry"));
            }
            return Ok((width, height, dpi));
        }
        cursor = end + ((end - start) % 2);
    }
    Err(SourceCommandError::Invalid("DjVu page INFO chunk missing"))
}

fn djvu_page_offsets(data: &[u8]) -> SourceCommandResult<Vec<usize>> {
    if !data.starts_with(b"AT&TFORM") || data.len() < 16 {
        return Err(SourceCommandError::Invalid("DjVu root FORM missing"));
    }
    let root_size = u32::from_be_bytes(data[8..12].try_into().unwrap()) as usize;
    let root_end = 12usize
        .checked_add(root_size)
        .ok_or(SourceCommandError::Invalid("DjVu root FORM size overflow"))?;
    if root_end != data.len() {
        return Err(SourceCommandError::Invalid("DjVu root FORM size differs"));
    }
    match &data[12..16] {
        b"DJVU" => Ok(vec![4]),
        b"DJVM" => {
            let mut cursor = 16;
            let mut directory = None;
            while cursor + 8 <= root_end {
                let kind = &data[cursor..cursor + 4];
                let (start, end) = chunk_bounds(data, cursor, root_end)?;
                if kind == b"DIRM" {
                    directory = Some(&data[start..end]);
                    break;
                }
                cursor = end + ((end - start) % 2);
            }
            let directory = directory.ok_or(SourceCommandError::Invalid("DjVu DIRM missing"))?;
            if directory.len() < 3 {
                return Err(SourceCommandError::Invalid("DjVu DIRM too short"));
            }
            let count = u16::from_be_bytes(directory[1..3].try_into().unwrap()) as usize;
            let table_end = 3usize
                .checked_add(
                    count
                        .checked_mul(4)
                        .ok_or(SourceCommandError::Invalid("DjVu DIRM table overflow"))?,
                )
                .ok_or(SourceCommandError::Invalid("DjVu DIRM table overflow"))?;
            if count == 0 || directory.len() < table_end {
                return Err(SourceCommandError::Invalid("DjVu DIRM table invalid"));
            }
            let mut component_offsets = Vec::with_capacity(count);
            for offset in (3..table_end).step_by(4) {
                component_offsets.push(u32::from_be_bytes(
                    directory[offset..offset + 4].try_into().unwrap(),
                ) as usize);
            }
            if component_offsets.windows(2).any(|pair| pair[0] >= pair[1])
                || component_offsets
                    .iter()
                    .any(|offset| *offset < 16 || offset + 12 > root_end)
            {
                return Err(SourceCommandError::Invalid(
                    "DjVu DIRM component offsets invalid",
                ));
            }
            let mut pages = Vec::new();
            for offset in component_offsets {
                if data.get(offset..offset + 4) != Some(b"FORM") {
                    return Err(SourceCommandError::Invalid("DjVu DIRM target is not FORM"));
                }
                let (start, end) = chunk_bounds(data, offset, root_end)?;
                match data.get(start..start + 4) {
                    Some(b"DJVU") => pages.push(offset),
                    Some(b"DJVI") | Some(b"THUM") => (),
                    _ => return Err(SourceCommandError::Invalid("DjVu DIRM FORM type")),
                }
                if end < start {
                    return Err(SourceCommandError::Invalid("DjVu DIRM component range"));
                }
            }
            if pages.is_empty() {
                return Err(SourceCommandError::Invalid(
                    "DjVu bundle has no page components",
                ));
            }
            Ok(pages)
        }
        _ => Err(SourceCommandError::Invalid("DjVu root FORM type")),
    }
}

fn djvu_inventory(raw: &[u8], entry: &Value) -> SourceCommandResult<Value> {
    let mut resources = Vec::new();
    for (index, offset) in djvu_page_offsets(raw)?.into_iter().enumerate() {
        let page_index = index + 1;
        let (width, height, dpi) = djvu_page_info(raw, offset)?;
        resources.push(json!({"resource_id":format!("djvu-page-{page_index:04}"),
            "resource_kind":"djvu_page","locator":{"page_index":page_index,
                "width_pixels":width,"height_pixels":height,"resolution_dpi":dpi},
            "structural_role":"page"}));
    }
    let geometries = distinct_geometry_count(&resources);
    Ok(profile_value(
        entry,
        "djvu_pages_v1",
        json!({"resource_count":resources.len(),
        "page_count":resources.len(),"distinct_page_geometry_count":geometries}),
        resources,
    ))
}

fn abbyy_inventory(
    raw: &[u8],
    entry: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let decoder = MultiGzDecoder::new(Cursor::new(raw));
    let parser = ParserConfig::new()
        .max_name_length(65536)
        .max_attributes(128)
        .max_attribute_length(65536)
        .max_data_length(16 * 1024 * 1024)
        .allow_multiple_root_elements(false)
        .ignore_end_of_stream(false)
        .replace_unknown_entity_references(false)
        .create_reader(decoder);
    let mut stack: Vec<(Option<String>, String)> = Vec::new();
    let mut page_depth = None;
    let mut page_width = 0u32;
    let mut page_height = 0u32;
    let mut page_dpi = 0u32;
    let mut paragraph_count = 0usize;
    let mut line_count = 0usize;
    let mut word_count = 0usize;
    let mut page_text = String::new();
    let mut char_params_depth = None;
    let mut char_params_word_start = false;
    let mut char_params_text = String::new();
    let mut resources = Vec::new();
    let mut event_count = 0usize;
    for event in parser {
        active(deadline, cancelled)?;
        event_count += 1;
        if event_count > MAX_XML_EVENTS || stack.len() > MAX_XML_DEPTH {
            return Err(profile_error("bounded ABBYY XML profile"));
        }
        match event.map_err(|_| SourceCommandError::Invalid("ABBYY XML is not well formed"))? {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                let local = name.local_name.clone();
                let namespace = name.namespace.clone();
                stack.push((namespace.clone(), local.clone()));
                if namespace.as_deref() == Some(ABBYY_NAMESPACE) && local == "page" {
                    if page_depth.is_some() {
                        return Err(SourceCommandError::Invalid("nested ABBYY pages"));
                    }
                    let attrs = attributes
                        .iter()
                        .filter(|attr| attr.name.namespace.is_none())
                        .map(|attr| (attr.name.local_name.as_str(), attr.value.as_str()))
                        .collect::<BTreeMap<_, _>>();
                    page_width = attrs
                        .get("width")
                        .ok_or(SourceCommandError::Invalid("ABBYY page width missing"))?
                        .parse()
                        .map_err(|_| SourceCommandError::Invalid("ABBYY page width invalid"))?;
                    page_height = attrs
                        .get("height")
                        .ok_or(SourceCommandError::Invalid("ABBYY page height missing"))?
                        .parse()
                        .map_err(|_| SourceCommandError::Invalid("ABBYY page height invalid"))?;
                    page_dpi = attrs
                        .get("resolution")
                        .ok_or(SourceCommandError::Invalid("ABBYY page resolution missing"))?
                        .parse()
                        .map_err(|_| {
                            SourceCommandError::Invalid("ABBYY page resolution invalid")
                        })?;
                    page_depth = Some(stack.len());
                    paragraph_count = 0;
                    line_count = 0;
                    word_count = 0;
                    page_text.clear();
                } else if page_depth.is_some() && namespace.as_deref() == Some(ABBYY_NAMESPACE) {
                    match local.as_str() {
                        "par" => paragraph_count += 1,
                        "line" => line_count += 1,
                        "charParams" => {
                            if char_params_depth.is_some() {
                                return Err(SourceCommandError::Invalid("nested ABBYY charParams"));
                            }
                            char_params_depth = Some(stack.len());
                            char_params_word_start = attributes.iter().any(|attr| {
                                attr.name.namespace.is_none()
                                    && attr.name.local_name == "wordStart"
                                    && attr.value == "true"
                            });
                            char_params_text.clear();
                        }
                        _ => (),
                    }
                }
            }
            XmlEvent::Characters(text) | XmlEvent::Whitespace(text) | XmlEvent::CData(text) => {
                if char_params_depth == Some(stack.len()) {
                    char_params_text.push_str(&text);
                }
            }
            XmlEvent::EndElement { name } => {
                let (namespace, local) = stack
                    .pop()
                    .ok_or(SourceCommandError::Invalid("ABBYY XML nesting"))?;
                if namespace != name.namespace || local != name.local_name {
                    return Err(SourceCommandError::Invalid("ABBYY XML nesting"));
                }
                if namespace.as_deref() == Some(ABBYY_NAMESPACE) && local == "charParams" {
                    page_text.push_str(&char_params_text);
                    word_count += usize::from(char_params_word_start);
                    char_params_depth = None;
                    char_params_word_start = false;
                    char_params_text.clear();
                } else if namespace.as_deref() == Some(ABBYY_NAMESPACE) && local == "page" {
                    let page_index = resources.len() + 1;
                    resources.push(json!({"resource_id":format!("abbyy-ocr-page-{page_index:04}"),
                        "resource_kind":"ocr_page","locator":{"page_index":page_index,
                            "width_pixels":page_width,"height_pixels":page_height,"resolution_dpi":page_dpi},
                        "structural_role":"page","paragraph_count":paragraph_count,
                        "line_count":line_count,"word_count":word_count,
                        "content_fingerprint":super::fingerprint(&[std::mem::take(&mut page_text)])}));
                    page_depth = None;
                }
            }
            XmlEvent::Doctype { .. } | XmlEvent::ProcessingInstruction { .. } => (),
            XmlEvent::StartDocument { .. } | XmlEvent::EndDocument | XmlEvent::Comment(_) => (),
        }
    }
    if !stack.is_empty()
        || page_depth.is_some()
        || char_params_depth.is_some()
        || resources.is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "ABBYY XML pages are incomplete",
        ));
    }
    let paragraph_total = resources
        .iter()
        .map(|row| row["paragraph_count"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let line_total = resources
        .iter()
        .map(|row| row["line_count"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let word_total = resources
        .iter()
        .map(|row| row["word_count"].as_u64().unwrap_or(0))
        .sum::<u64>();
    let geometries = distinct_geometry_count(&resources);
    Ok(profile_value(
        entry,
        "abbyy_xml_pages_v1",
        json!({"resource_count":resources.len(),
        "page_count":resources.len(),"paragraph_count":paragraph_total,"line_count":line_total,
        "word_count":word_total,"distinct_page_geometry_count":geometries}),
        resources,
    ))
}

fn jp2_zip_inventory(
    file: &mut File,
    entry: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let size = entry["byte_size"]
        .as_u64()
        .ok_or(SourceCommandError::Invalid("JP2 ZIP byte_size"))?;
    let mut resources = Vec::new();
    let mut authorize = || active(deadline, cancelled);
    super::visit_members(
        file,
        size,
        deadline,
        cancelled,
        &mut authorize,
        |name, member| {
            let basename = name.rsplit('/').next().unwrap_or(name);
            let stem = basename
                .strip_suffix(".jp2")
                .ok_or(SourceCommandError::Invalid("JP2 ZIP non-page member"))?;
            let (prefix, digits) = stem
                .rsplit_once('_')
                .ok_or(SourceCommandError::Invalid("JP2 ZIP member name"))?;
            if prefix.is_empty() {
                return Err(SourceCommandError::Invalid("JP2 ZIP member name"));
            }
            if digits.len() != 4 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(SourceCommandError::Invalid("JP2 ZIP member name"));
            }
            let leaf_number = digits
                .parse::<usize>()
                .map_err(|_| SourceCommandError::Invalid("JP2 ZIP leaf number"))?;
            let container_order = resources.len() + 1;
            if leaf_number != container_order - 1 {
                return Err(SourceCommandError::Invalid(
                    "JP2 ZIP leaf numbering differs",
                ));
            }
            resources.push(
                json!({"resource_id":format!("jp2-page-{container_order:04}"),
            "resource_kind":"image_page","locator":{"page_index":container_order,
                "leaf_number":leaf_number,"member_path":name,"container_order":container_order},
            "media_type":"image/jp2","byte_size":member.len(),
            "sha256":Digest256::of_bytes(member).to_hex(),"structural_role":"page"}),
            );
            Ok(())
        },
    )?;
    if resources.is_empty() {
        return Err(SourceCommandError::Invalid("JP2 ZIP yielded no pages"));
    }
    Ok(profile_value(
        entry,
        "jp2_zip_pages_v1",
        json!({"resource_count":resources.len(),
        "page_count":resources.len(),"member_count":resources.len()}),
        resources,
    ))
}

fn pdf_run(
    program: &str,
    args: &[&str],
    raw: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let output = crate::source_text_owner_ocr::bounded_process_with_stdin(
        program,
        args,
        None,
        Some(raw),
        MAX_EXTERNAL_STDOUT,
        deadline,
        cancelled,
    )?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn pdf_page_count(info: &str) -> SourceCommandResult<usize> {
    info.lines()
        .find_map(|line| line.strip_prefix("Pages:").map(str::trim))
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .ok_or(SourceCommandError::Invalid(
            "pdfinfo did not report a page count",
        ))
}

fn pdf_page_geometry(
    output: &str,
    page_count: usize,
) -> SourceCommandResult<(BTreeMap<usize, (f64, f64)>, BTreeMap<usize, i32>)> {
    let mut sizes = BTreeMap::new();
    let mut rotations = BTreeMap::new();
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Page ") {
            if let Some((index, value)) = rest.split_once(" size:") {
                let index = index
                    .parse::<usize>()
                    .map_err(|_| SourceCommandError::Invalid("pdfinfo page index"))?;
                let dimensions = value
                    .trim()
                    .split_once(" pts")
                    .map(|(dimensions, _)| dimensions.trim())
                    .ok_or(SourceCommandError::Invalid("pdfinfo page size"))?;
                let dimensions = dimensions
                    .split_once('(')
                    .map(|(dimensions, _)| dimensions.trim())
                    .unwrap_or(dimensions);
                let (width, height) = dimensions
                    .split_once(" x ")
                    .ok_or(SourceCommandError::Invalid("pdfinfo page dimensions"))?;
                let width = width
                    .parse::<f64>()
                    .map_err(|_| SourceCommandError::Invalid("pdfinfo page width"))?;
                let height = height
                    .parse::<f64>()
                    .map_err(|_| SourceCommandError::Invalid("pdfinfo page height"))?;
                if !width.is_finite()
                    || !height.is_finite()
                    || width <= 0.0
                    || height <= 0.0
                    || sizes.insert(index, (width, height)).is_some()
                {
                    return Err(SourceCommandError::Invalid("pdfinfo page geometry"));
                }
            } else if let Some((index, value)) = rest.split_once(" rot:") {
                let index = index
                    .parse::<usize>()
                    .map_err(|_| SourceCommandError::Invalid("pdfinfo rotation page index"))?;
                let value = value
                    .trim()
                    .parse::<i32>()
                    .map_err(|_| SourceCommandError::Invalid("pdfinfo page rotation"))?;
                rotations.insert(index, value);
            }
        }
    }
    if sizes.keys().copied().collect::<BTreeSet<_>>() != (1..=page_count).collect() {
        return Err(SourceCommandError::Invalid(
            "pdfinfo did not enumerate every page",
        ));
    }
    if rotations
        .keys()
        .any(|page| *page == 0 || *page > page_count)
    {
        return Err(SourceCommandError::Invalid(
            "pdfinfo rotation page out of range",
        ));
    }
    Ok((sizes, rotations))
}

fn pdf_inventory(
    raw: &[u8],
    entry: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let base = pdf_run("pdfinfo", &["-"], raw, deadline, cancelled)?;
    let page_count = pdf_page_count(&base)?;
    let page_count_text = page_count.to_string();
    let boxes = pdf_run(
        "pdfinfo",
        &["-f", "1", "-l", &page_count_text, "-box", "-"],
        raw,
        deadline,
        cancelled,
    )?;
    let (sizes, rotations) = pdf_page_geometry(&boxes, page_count)?;
    let image_output = pdf_run("pdfimages", &["-list", "-"], raw, deadline, cancelled)?;
    let mut image_counts = BTreeMap::<usize, usize>::new();
    for line in image_output.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 3 {
            if let (Ok(page), Some(kind)) = (fields[0].parse::<usize>(), fields.get(2).copied()) {
                if matches!(kind, "image" | "mask" | "smask") {
                    *image_counts.entry(page).or_default() += 1;
                }
            }
        }
    }
    if image_counts
        .keys()
        .any(|page| *page == 0 || *page > page_count)
    {
        return Err(SourceCommandError::Invalid(
            "pdfimages page index out of range",
        ));
    }
    let mut resources = Vec::with_capacity(page_count);
    for page_index in 1..=page_count {
        let (width, height) = sizes[&page_index];
        resources.push(json!({"resource_id":format!("pdf-page-{page_index:04}"),
            "resource_kind":"pdf_page","locator":{"page_index":page_index,
                "width_points":width,"height_points":height,
                "rotation_degrees":rotations.get(&page_index).copied().unwrap_or(0)},
            "structural_role":"page","image_resource_count":image_counts.get(&page_index).copied().unwrap_or(0)}));
    }
    let geometry = resources
        .iter()
        .map(|resource| {
            (
                resource["locator"]["width_points"]
                    .as_f64()
                    .unwrap_or_default()
                    .to_bits(),
                resource["locator"]["height_points"]
                    .as_f64()
                    .unwrap_or_default()
                    .to_bits(),
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
    let image_count = image_counts.values().sum::<usize>();
    Ok(profile_value(
        entry,
        "pdf_pages_v1",
        json!({"resource_count":resources.len(),
        "page_count":page_count,"image_resource_count":image_count,
        "distinct_page_geometry_count":geometry}),
        resources,
    ))
}

fn plain_text_inventory(raw: &[u8], entry: &Value) -> SourceCommandResult<Value> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| SourceCommandError::Invalid("plain-text payload is not UTF-8"))?;
    let resource = json!({"resource_id":"plain-text-file","resource_kind":"plain_text_file",
        "locator":{"container_order":1},"structural_role":"member",
        "content_fingerprint":super::inventory_exact_fingerprint(text,"unicode-codepoints-preserved")});
    Ok(profile_value(
        entry,
        "plain_text_v1",
        json!({"resource_count":1}),
        vec![resource],
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenPayloadMetadata {
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl OpenPayloadMetadata {
    fn read(file: &File) -> SourceCommandResult<Self> {
        let metadata = file
            .metadata()
            .map_err(|_| SourceCommandError::Unsupported("Item inventory payload metadata"))?;
        if !metadata.is_file() {
            return Err(SourceCommandError::Invalid(
                "Item inventory payload is not regular",
            ));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }
}

fn verify_open_payload_bytes(
    file: &mut File,
    size: u64,
    expected_sha256: &str,
) -> SourceCommandResult<()> {
    let mut hasher = tos_foundation::Digest256Hasher::new();
    let mut observed_size = 0u64;
    let mut buffer = [0u8; 65_536];
    file.seek(SeekFrom::Start(0))
        .map_err(|_| SourceCommandError::Unsupported("Item inventory payload seek"))?;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Unsupported("Item inventory payload read"))?;
        if count == 0 {
            break;
        }
        observed_size =
            observed_size
                .checked_add(count as u64)
                .ok_or(SourceCommandError::Invalid(
                    "Item inventory payload size overflow",
                ))?;
        if observed_size > size {
            return Err(SourceCommandError::Invalid(
                "Item inventory payload size differs",
            ));
        }
        hasher.update(&buffer[..count]);
    }
    if observed_size != size || hasher.finalize().to_hex() != expected_sha256 {
        return Err(SourceCommandError::Invalid(
            "Item inventory payload fixity differs",
        ));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| SourceCommandError::Unsupported("Item inventory payload seek"))?;
    Ok(())
}

fn open_verified_payload(
    path: &Path,
    entry: &Value,
) -> SourceCommandResult<(File, OpenPayloadMetadata)> {
    let size = entry["byte_size"]
        .as_u64()
        .ok_or(SourceCommandError::Invalid(
            "Item inventory payload byte_size",
        ))?;
    let expected_sha256 = file_field(entry, "sha256")?;
    let mut file = tos_fd_open::open_absolute_regular(path, size)
        .map_err(|_| SourceCommandError::Unsupported("Item inventory payload access"))?;
    let metadata = OpenPayloadMetadata::read(&file)?;
    if metadata.size != size {
        return Err(SourceCommandError::Invalid(
            "Item inventory payload size differs",
        ));
    }
    verify_open_payload_bytes(&mut file, size, expected_sha256)?;
    if OpenPayloadMetadata::read(&file)? != metadata {
        return Err(SourceCommandError::Invalid(
            "Item inventory payload changed during verification",
        ));
    }
    Ok((file, metadata))
}

fn verify_payload_unchanged(
    file: &mut File,
    before: OpenPayloadMetadata,
    entry: &Value,
) -> SourceCommandResult<()> {
    if OpenPayloadMetadata::read(file)? != before {
        return Err(SourceCommandError::Invalid(
            "Item inventory payload changed during observation",
        ));
    }
    let size = entry["byte_size"]
        .as_u64()
        .ok_or(SourceCommandError::Invalid(
            "Item inventory payload byte_size",
        ))?;
    verify_open_payload_bytes(file, size, file_field(entry, "sha256")?)?;
    if OpenPayloadMetadata::read(file)? != before {
        return Err(SourceCommandError::Invalid(
            "Item inventory payload changed during verification",
        ));
    }
    Ok(())
}

fn legacy_file_inventory(
    path: &Path,
    raw: &[u8],
    entry: &Value,
    plain_text_profile: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let media_type = file_field(entry, "media_type")?;
    let relative_path = file_field(entry, "relative_path")?;
    if plain_text_profile == "plain_text_v1" && media_type != "text/plain" {
        return Err(SourceCommandError::Invalid(
            "plain_text_v1 is only supported for text/plain",
        ));
    }
    let size = entry["byte_size"]
        .as_u64()
        .ok_or(SourceCommandError::Invalid("payload byte_size"))?;
    match media_type {
        "application/pdf" => pdf_inventory(raw, entry, deadline, cancelled),
        "image/vnd.djvu" => djvu_inventory(raw, entry),
        "application/vnd.djvu+xml" => djvu_xml_inventory(raw, entry),
        "application/gzip" if relative_path.ends_with(".abbyy.xml.gz") => {
            abbyy_inventory(raw, entry, deadline, cancelled)
        }
        "application/zip" if relative_path.ends_with("_jp2.zip") => {
            let (mut file, metadata) = open_verified_payload(path, entry)?;
            let inventory = jp2_zip_inventory(&mut file, entry, deadline, cancelled)?;
            verify_payload_unchanged(&mut file, metadata, entry)?;
            Ok(inventory)
        }
        "application/xml" | "text/xml" if relative_path.ends_with("_scandata.xml") => {
            scandata_inventory(raw, entry)
        }
        "text/plain" if plain_text_profile == "plain_text_v1" => plain_text_inventory(raw, entry),
        "application/epub+zip" => {
            let (mut file, metadata) = open_verified_payload(path, entry)?;
            let config = json!({"byte_size":size,"file_id":entry["file_id"],
                "sha256":entry["sha256"],"media_type":media_type,
                "relative_path":relative_path});
            let mut authorize = || active(deadline, cancelled);
            let inventory =
                super::observe(&mut file, &config, deadline, cancelled, &mut authorize)?;
            verify_payload_unchanged(&mut file, metadata, entry)?;
            Ok(inventory)
        }
        "text/plain" | "text/markdown" => {
            let config = json!({"byte_size":size,"file_id":entry["file_id"],
                "sha256":entry["sha256"],"media_type":media_type,
                "relative_path":relative_path});
            let mut file = tos_fd_open::open_absolute_regular(path, size)
                .map_err(|_| SourceCommandError::Unsupported("Item inventory payload access"))?;
            let mut authorize = || active(deadline, cancelled);
            super::observe(&mut file, &config, deadline, cancelled, &mut authorize)
        }
        _ => super::inventory_file(raw, entry),
    }
}

fn preserve_prior_pdf_number_shapes(files: &mut [Value], prior: &Value) {
    let Some(prior_files) = prior.get("files").and_then(Value::as_array) else {
        return;
    };
    let prior_by_id = prior_files
        .iter()
        .filter_map(|file| {
            let id = file.get("file_id")?.as_str()?;
            Some((id, file))
        })
        .collect::<BTreeMap<_, _>>();
    for file in files
        .iter_mut()
        .filter(|file| file["profile"] == "pdf_pages_v1")
    {
        let Some(prior_file) = file["file_id"]
            .as_str()
            .and_then(|id| prior_by_id.get(id).copied())
        else {
            continue;
        };
        let Some(prior_resources) = prior_file.get("resources").and_then(Value::as_array) else {
            continue;
        };
        let prior_resources = prior_resources
            .iter()
            .filter_map(|resource| {
                let id = resource.get("resource_id")?.as_str()?;
                Some((id, resource))
            })
            .collect::<BTreeMap<_, _>>();
        let Some(resources) = file["resources"].as_array_mut() else {
            continue;
        };
        for resource in resources {
            let Some(id) = resource["resource_id"].as_str() else {
                continue;
            };
            let Some(prior_resource) = prior_resources.get(id).copied() else {
                continue;
            };
            for field in ["width_points", "height_points"] {
                let Some(prior_number) = prior_resource
                    .get("locator")
                    .and_then(|locator| locator.get(field))
                    .and_then(Value::as_number)
                else {
                    continue;
                };
                let prior_integer = prior_number
                    .as_u64()
                    .map(|number| number as f64)
                    .or_else(|| prior_number.as_i64().map(|number| number as f64));
                let Some(prior_integer) = prior_integer else {
                    continue;
                };
                let Some(current) = resource["locator"][field].as_f64() else {
                    continue;
                };
                if !current.is_finite() || current.fract() != 0.0 || current != prior_integer {
                    continue;
                }
                resource["locator"][field] = Value::Number(prior_number.clone());
            }
        }
    }
}

fn read_prior_inventory(path: &Path) -> Result<Option<Value>, String> {
    let Some(raw) = output_current(path)? else {
        return Ok(None);
    };
    match serde_json::from_slice::<Value>(&raw) {
        Ok(value @ Value::Object(_)) => Ok(Some(value)),
        _ => Ok(None),
    }
}

/// Complete native successor for the standalone source inventory builder.
/// The wrapper is returned for the caller to publish at the Item's declared
/// resource_inventory_ref; this function never writes repository files.
fn build_full_inventory_optional(
    repo_root: &Path,
    item_manifest_ref: &str,
    payload_source_root: &Path,
    event_date: &str,
    plain_text_profile: &str,
) -> std::result::Result<Option<Value>, String> {
    let source_command_result = || -> SourceCommandResult<Option<Value>> {
        if !matches!(plain_text_profile, "plain_utf8_file_v1" | "plain_text_v1") {
            return Err(SourceCommandError::Invalid("plain text inventory profile"));
        }
        super::inventory_safe_ref(item_manifest_ref)?;
        let item_root_ref = item_manifest_ref
            .strip_suffix("/item.manifest.json")
            .ok_or(SourceCommandError::Invalid("Item manifest reference"))?;
        let source_tail = item_root_ref
            .strip_prefix("ToS/source-witnesses/")
            .ok_or(SourceCommandError::Invalid("Item manifest source route"))?;
        super::inventory_safe_ref(source_tail)?;
        let manifest_path =
            crate::source_acquisition_batch::path_under(repo_root, item_manifest_ref)
                .map_err(|_| SourceCommandError::Invalid("Item manifest reference"))?;
        let manifest = super::inventory_read_json(&manifest_path)?;
        let item_id = super::inventory_json_text(&manifest, "item_id")?;
        let entries = super::inventory_json_field(&manifest, "payload_files")?
            .as_array()
            .ok_or(SourceCommandError::Invalid("Item manifest payload_files"))?;
        if entries.is_empty() {
            return Err(SourceCommandError::Invalid(
                "Item manifest has no payload files",
            ));
        }
        let mut files = Vec::with_capacity(entries.len());
        let deadline = Instant::now() + Duration::from_secs(600);
        let cancelled = AtomicBool::new(false);
        for entry in entries {
            active(deadline, &cancelled)?;
            let relative_path = super::inventory_json_text(entry, "relative_path")?;
            let payload_ref = super::inventory_payload_ref(item_manifest_ref, relative_path)?;
            match fs::symlink_metadata(payload_source_root) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(_) => {
                    return Err(SourceCommandError::Unsupported(
                        "Item payload source root access",
                    ));
                }
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                    return Err(SourceCommandError::Unsupported(
                        "Item payload source root type",
                    ));
                }
                Ok(_) => (),
            }
            let payload_path =
                crate::source_acquisition_batch::path_under(payload_source_root, &payload_ref)
                    .map_err(|_| SourceCommandError::Invalid("Item payload source path"))?;
            match fs::symlink_metadata(&payload_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(_) => {
                    return Err(SourceCommandError::Unsupported(
                        "Item payload source path access",
                    ));
                }
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(SourceCommandError::Unsupported(
                        "Item payload source file type",
                    ));
                }
                Ok(metadata) if !metadata.is_file() => return Ok(None),
                Ok(_) => (),
            }
            let byte_size = super::inventory_json_field(entry, "byte_size")?
                .as_u64()
                .ok_or(SourceCommandError::Invalid("Item payload byte_size"))?;
            if byte_size > MAX_PROFILE_BYTES {
                return Err(profile_error(
                    "Item payload exceeds native inventory byte bound",
                ));
            }
            let expected_sha256 = super::inventory_json_text(entry, "sha256")?;
            let raw = crate::source_acquisition_batch::read_bytes(
                &payload_path,
                None,
                false,
                false,
                byte_size,
            )
            .map_err(|_| SourceCommandError::Unsupported("Item payload fixity read"))?;
            if raw.len() as u64 != byte_size || super::inventory_digest(&raw) != expected_sha256 {
                return Err(SourceCommandError::Invalid(
                    "Item payload fixity differs from manifest",
                ));
            }
            files.push(legacy_file_inventory(
                &payload_path,
                &raw,
                entry,
                plain_text_profile,
                deadline,
                &cancelled,
            )?);
        }
        let inventory_ref = super::inventory_json_text(&manifest, "resource_inventory_ref")?;
        super::inventory_safe_ref(inventory_ref)?;
        if inventory_ref != format!("{item_root_ref}/resource-inventory.json") {
            return Err(SourceCommandError::Invalid("Item resource inventory path"));
        }
        let default_event_ref = super::inventory_event_ref(item_id, event_date)?;
        let prior_path = crate::source_acquisition_batch::path_under(repo_root, inventory_ref)
            .map_err(|_| SourceCommandError::Invalid("resource inventory reference"))?;
        let prior = read_prior_inventory(&prior_path)
            .map_err(|_| SourceCommandError::Unsupported("prior resource inventory read"))?;
        let event_ref = prior
            .as_ref()
            .and_then(|value| value.get("provenance_event_ref"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or(default_event_ref);
        let inventory_version = prior
            .as_ref()
            .and_then(|value| value.get("inventory_version"))
            .and_then(Value::as_u64)
            .filter(|version| *version >= 1)
            .unwrap_or(1);
        let supersedes_inventory_ref = prior
            .as_ref()
            .and_then(|value| value.get("supersedes_inventory_ref"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(prior) = prior.as_ref() {
            preserve_prior_pdf_number_shapes(&mut files, prior);
        }
        Ok(Some(
            json!({"$schema":super::INVENTORY_SCHEMA,"schema_version":"tos_source_resource_inventory_v1",
            "item_id":item_id,"generated_from_manifest_ref":item_manifest_ref,
            "inventory_authority":"mechanical_metadata_only","source_text_included":false,
            "files":files,"generator":{"name":"build_source_resource_inventories.py",
                "version":super::INVENTORY_GENERATOR_VERSION},"provenance_event_ref":event_ref,
            "inventory_version":inventory_version,"supersedes_inventory_ref":supersedes_inventory_ref,
            "authority_boundary":super::INVENTORY_AUTHORITY_BOUNDARY}),
        ))
    };
    source_command_result().map_err(|error| format!("native source inventory failed: {error:?}"))
}

/// Complete native successor for one standalone source inventory item. The
/// wrapper is returned for the caller to publish; this function never writes.
pub(crate) fn build_full_inventory(
    repo_root: &Path,
    item_manifest_ref: &str,
    payload_source_root: &Path,
    event_date: &str,
    plain_text_profile: &str,
) -> std::result::Result<Value, String> {
    build_full_inventory_optional(
        repo_root,
        item_manifest_ref,
        payload_source_root,
        event_date,
        plain_text_profile,
    )?
    .ok_or_else(|| "no local payload set is available for resource inventory".into())
}

const MAX_MANIFEST_DISCOVERY_ENTRIES: usize = 200_000;
const MAX_MANIFEST_DISCOVERY_DEPTH: usize = 128;
const MAX_INVENTORY_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const INVENTORY_SCHEMA_REF: &str = "ToS/contracts/source-resource-inventory.schema.json";

fn inventory_schema_probe(repo_root: &Path) -> Result<tos_validation::SchemaBackendProbe, String> {
    let schema_path = crate::source_acquisition_batch::path_under(repo_root, INVENTORY_SCHEMA_REF)?;
    let raw = crate::source_acquisition_batch::read_bytes(
        &schema_path,
        None,
        false,
        false,
        tos_validation::SchemaBackendProbe::MAX_RESOURCE_BYTES as u64,
    )?;
    let schema = crate::source_acquisition_batch::parse(&raw)?;
    let uri = schema
        .get("$id")
        .and_then(Value::as_str)
        .ok_or("resource-inventory schema has no $id")?;
    if uri != super::INVENTORY_SCHEMA {
        return Err("resource-inventory schema id differs from its owner reference".into());
    }
    tos_validation::SchemaBackendProbe::new(
        [tos_validation::SchemaResource {
            uri: uri.to_owned(),
            raw,
        }],
        tos_validation::FormatProfile::LegacyPythonObserved20260923,
    )
    .map_err(|error| format!("resource-inventory schema refused: {error:?}"))
}

fn validate_inventory_value(
    probe: &tos_validation::SchemaBackendProbe,
    inventory: &Value,
) -> Result<(), String> {
    if !probe
        .is_valid_value(super::INVENTORY_SCHEMA, inventory)
        .map_err(|error| format!("resource-inventory schema execution failed: {error:?}"))?
    {
        return Err("source resource inventory failed schema validation".into());
    }
    Ok(())
}

fn inventory_manifests(repo_root: &Path) -> Result<Vec<String>, String> {
    let source_root_ref = "ToS/source-witnesses";
    let source_root = crate::source_acquisition_batch::path_under(repo_root, source_root_ref)?;
    let metadata = match fs::symlink_metadata(&source_root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("source-witness root cannot be read: {error}")),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("source-witness root is not a regular directory".into());
        }
        Ok(metadata) => metadata,
    };
    let _ = metadata;

    let mut pending = vec![(source_root, String::new(), 0usize)];
    let mut manifests = Vec::new();
    let mut visited = 0usize;
    while let Some((directory, relative, depth)) = pending.pop() {
        if depth > MAX_MANIFEST_DISCOVERY_DEPTH {
            return Err("source-witness tree exceeds inventory depth bound".into());
        }
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("source-witness directory read failed: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("source-witness directory entry failed: {error}"))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            visited = visited
                .checked_add(1)
                .ok_or("source-witness entry count overflow")?;
            if visited > MAX_MANIFEST_DISCOVERY_ENTRIES {
                return Err("source-witness tree exceeds inventory entry bound".into());
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "source-witness path is not UTF-8")?;
            let child_relative = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            let kind = entry
                .file_type()
                .map_err(|error| format!("source-witness entry type failed: {error}"))?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                pending.push((entry.path(), child_relative, depth + 1));
            } else if kind.is_file() && name == "item.manifest.json" {
                let reference = format!("{source_root_ref}/{child_relative}");
                crate::source_acquisition_batch::safe_ref(&reference)?;
                manifests.push(reference);
            }
        }
    }
    manifests.sort();
    Ok(manifests)
}

fn preferred_keys(object: &serde_json::Map<String, Value>) -> &'static [&'static str] {
    if object.contains_key("$schema") {
        return &[
            "$schema",
            "schema_version",
            "item_id",
            "generated_from_manifest_ref",
            "inventory_authority",
            "source_text_included",
            "files",
            "generator",
            "provenance_event_ref",
            "inventory_version",
            "supersedes_inventory_ref",
            "authority_boundary",
        ];
    }
    if let Some(profile) = object.get("profile").and_then(Value::as_str) {
        return match profile {
            "plain_utf8_file_v1" => &[
                "file_id",
                "file_sha256",
                "media_type",
                "profile",
                "summary",
                "utf8_observation",
                "resources",
            ],
            _ => &[
                "file_id",
                "file_sha256",
                "media_type",
                "profile",
                "summary",
                "resources",
            ],
        };
    }
    if let Some(kind) = object.get("resource_kind").and_then(Value::as_str) {
        return match kind {
            "pdf_page" => &[
                "resource_id",
                "resource_kind",
                "locator",
                "structural_role",
                "image_resource_count",
            ],
            "epub_member" => &[
                "resource_id",
                "resource_kind",
                "locator",
                "media_type",
                "byte_size",
                "sha256",
                "structural_role",
                "content_fingerprint",
            ],
            "tei_division" => &[
                "resource_id",
                "resource_kind",
                "locator",
                "structural_role",
                "content_fingerprint",
                "label_fingerprint",
            ],
            "osis_chapter" => &[
                "resource_id",
                "resource_kind",
                "structural_role",
                "locator",
                "verse_count",
                "word_count",
            ],
            "osis_verse" => &[
                "resource_id",
                "resource_kind",
                "structural_role",
                "locator",
                "word_count",
                "content_fingerprint",
            ],
            "json_container" => &["resource_id", "resource_kind", "structural_role", "locator"],
            "json_member" => &[
                "resource_id",
                "resource_kind",
                "structural_role",
                "locator",
                "json_value_counts",
                "label_fingerprint",
                "content_fingerprint",
            ],
            "image_page" => &[
                "resource_id",
                "resource_kind",
                "locator",
                "media_type",
                "byte_size",
                "sha256",
                "structural_role",
            ],
            "ocr_page" => &[
                "resource_id",
                "resource_kind",
                "locator",
                "structural_role",
                "paragraph_count",
                "line_count",
                "word_count",
                "content_fingerprint",
            ],
            "plain_text_file" if object.contains_key("media_type") => &[
                "resource_id",
                "resource_kind",
                "locator",
                "media_type",
                "byte_size",
                "sha256",
                "structural_role",
            ],
            "plain_text_file" => &[
                "resource_id",
                "resource_kind",
                "locator",
                "structural_role",
                "content_fingerprint",
            ],
            _ => &["resource_id", "resource_kind", "locator", "structural_role"],
        };
    }
    if object.contains_key("page_index")
        || object.contains_key("byte_start")
        || object.contains_key("tei_path")
        || object.contains_key("osis_id")
        || object.contains_key("json_member_index")
    {
        return &[
            "page_index",
            "byte_start",
            "byte_end",
            "width_points",
            "height_points",
            "rotation_degrees",
            "leaf_number",
            "width_pixels",
            "height_pixels",
            "resolution_dpi",
            "member_path",
            "container_order",
            "spine_index",
            "tei_path",
            "tei_depth",
            "tei_page_label",
            "tei_facs_ref",
            "tei_n",
            "tei_type",
            "osis_id",
            "chapter_index",
            "verse_index",
            "json_member_index",
            "json_value_type",
            "parent_resource_id",
        ];
    }
    if object.contains_key("algorithm") {
        return &["algorithm", "normalization", "sha256", "character_count"];
    }
    if object.contains_key("object_count") {
        return &[
            "object_count",
            "array_count",
            "string_count",
            "number_count",
            "boolean_count",
            "null_count",
            "object_key_count",
        ];
    }
    if object.contains_key("resource_count") {
        if object.contains_key("top_level_member_count") {
            return &[
                "resource_count",
                "top_level_member_count",
                "json_value_counts",
            ];
        }
        if object.contains_key("page_break_count") {
            return &[
                "resource_count",
                "page_break_count",
                "division_count",
                "max_division_depth",
            ];
        }
        if object.contains_key("chapter_count") {
            return &[
                "resource_count",
                "chapter_count",
                "verse_count",
                "word_count",
            ];
        }
        if object.contains_key("spine_item_count") {
            return &[
                "resource_count",
                "member_count",
                "spine_item_count",
                "xhtml_count",
                "image_resource_count",
            ];
        }
        if object.contains_key("paragraph_count") {
            return &[
                "resource_count",
                "page_count",
                "paragraph_count",
                "line_count",
                "word_count",
                "distinct_page_geometry_count",
            ];
        }
        if object.contains_key("member_count") {
            return &["resource_count", "page_count", "member_count"];
        }
        if object.contains_key("image_resource_count") {
            return &[
                "resource_count",
                "page_count",
                "image_resource_count",
                "distinct_page_geometry_count",
            ];
        }
        if object.contains_key("page_count") {
            return &[
                "resource_count",
                "page_count",
                "distinct_page_geometry_count",
            ];
        }
        return &["resource_count"];
    }
    if object.contains_key("name") && object.contains_key("version") {
        return &["name", "version"];
    }
    if object.contains_key("encoding") {
        return &[
            "encoding",
            "bom_byte_count",
            "code_point_count",
            "code_point_count_includes_bom",
            "crlf_count",
            "lone_cr_count",
            "lone_lf_count",
            "terminal_newline",
            "normalization_observation",
            "unicode_version",
            "normalization_performed",
            "markup_interpretation_performed",
        ];
    }
    &[]
}

fn render_inventory_value(value: &Value, depth: usize, output: &mut String) -> Result<(), String> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => {
            output.push_str(&serde_json::to_string(value).map_err(|error| error.to_string())?)
        }
        Value::Array(items) if items.is_empty() => output.push_str("[]"),
        Value::Array(items) => {
            output.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                output.push_str(&"  ".repeat(depth + 1));
                render_inventory_value(item, depth + 1, output)?;
                if index + 1 != items.len() {
                    output.push(',');
                }
                output.push('\n');
            }
            output.push_str(&"  ".repeat(depth));
            output.push(']');
        }
        Value::Object(object) if object.is_empty() => output.push_str("{}"),
        Value::Object(object) => {
            let preferred = preferred_keys(object);
            let mut fields = object.iter().collect::<Vec<_>>();
            fields.sort_by(|(left, _), (right, _)| {
                let left_rank = preferred
                    .iter()
                    .position(|key| *key == left.as_str())
                    .unwrap_or(usize::MAX);
                let right_rank = preferred
                    .iter()
                    .position(|key| *key == right.as_str())
                    .unwrap_or(usize::MAX);
                left_rank.cmp(&right_rank).then_with(|| left.cmp(right))
            });
            output.push_str("{\n");
            for (index, (key, child)) in fields.iter().enumerate() {
                output.push_str(&"  ".repeat(depth + 1));
                output.push_str(&serde_json::to_string(key).map_err(|error| error.to_string())?);
                output.push_str(": ");
                render_inventory_value(child, depth + 1, output)?;
                if index + 1 != fields.len() {
                    output.push(',');
                }
                output.push('\n');
            }
            output.push_str(&"  ".repeat(depth));
            output.push('}');
        }
    }
    Ok(())
}

fn render_inventory(inventory: &Value) -> Result<Vec<u8>, String> {
    let mut output = String::new();
    render_inventory_value(inventory, 0, &mut output)?;
    output.push('\n');
    if output.len() > MAX_INVENTORY_OUTPUT_BYTES {
        return Err("source inventory output exceeds its native byte bound".into());
    }
    Ok(output.into_bytes())
}

fn output_current(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("resource inventory output read failed: {error}")),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("resource inventory output is not a regular file".into())
        }
        Ok(_) => crate::source_acquisition_batch::read_bytes(
            path,
            None,
            false,
            false,
            MAX_INVENTORY_OUTPUT_BYTES as u64,
        )
        .map(Some),
    }
}

fn write_inventory(path: &Path, body: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("resource inventory output directory failed: {error}"))?;
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err("resource inventory output is not a regular file".into());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!(
                "resource inventory output metadata failed: {error}"
            ));
        }
        _ => (),
    }
    fs::write(path, body).map_err(|error| format!("resource inventory write failed: {error}"))
}

fn valid_event_date(date: &str) -> bool {
    date.len() == 10
        && date.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 4 | 7) {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|error| format!("current directory unavailable: {error}"))
    }
}

fn request_text_or<'a>(request: &'a Value, key: &str, default: &'a str) -> Result<&'a str, String> {
    match request.get(key) {
        None => Ok(default),
        Some(Value::String(value)) if !value.is_empty() => Ok(value),
        Some(_) => Err(format!("inventory request {key} must be nonempty text")),
    }
}

fn inventory_metadata() -> Value {
    json!({
        "schema_ref": super::INVENTORY_SCHEMA,
        "generator_version": super::INVENTORY_GENERATOR_VERSION,
        "authority_boundaries": {
            "1": LEGACY_AUTHORITY_BOUNDARY_V1,
            "2": super::INVENTORY_AUTHORITY_BOUNDARY
        },
        "max_plain_utf8_bytes": MAX_PLAIN_UTF8_BYTES
    })
}

fn invoke_file_inventory(request: &Value) -> Result<Value, String> {
    let result = || -> SourceCommandResult<Value> {
        let path_text = request
            .get("payload_path")
            .and_then(Value::as_str)
            .ok_or(SourceCommandError::Invalid("inventory payload path"))?;
        if path_text.is_empty() {
            return Err(SourceCommandError::Invalid("inventory payload path"));
        }
        let path = absolute_path(Path::new(path_text))
            .map_err(|_| SourceCommandError::Invalid("inventory payload path"))?;
        let entry = request
            .get("payload_entry")
            .ok_or(SourceCommandError::Invalid("inventory payload entry"))?;
        let byte_size = entry["byte_size"]
            .as_u64()
            .ok_or(SourceCommandError::Invalid("inventory payload byte_size"))?;
        if byte_size > MAX_PROFILE_BYTES {
            return Err(profile_error(
                "Item payload exceeds native inventory byte bound",
            ));
        }
        let expected_sha256 = file_field(entry, "sha256")?;
        if expected_sha256.len() != 64
            || !expected_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(SourceCommandError::Invalid("inventory payload sha256"));
        }
        let plain_text_profile = match request.get("plain_text_profile") {
            None => "plain_utf8_file_v1",
            Some(Value::String(value)) if !value.is_empty() => value.as_str(),
            Some(_) => {
                return Err(SourceCommandError::Invalid("plain text inventory profile"));
            }
        };
        if !matches!(plain_text_profile, "plain_utf8_file_v1" | "plain_text_v1") {
            return Err(SourceCommandError::Invalid("plain text inventory profile"));
        }
        let raw = crate::source_acquisition_batch::read_bytes(&path, None, false, false, byte_size)
            .map_err(|_| SourceCommandError::Unsupported("Item inventory payload access"))?;
        if raw.len() as u64 != byte_size || super::inventory_digest(&raw) != expected_sha256 {
            return Err(SourceCommandError::Invalid(
                "Item payload fixity differs from manifest",
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(600);
        let cancelled = AtomicBool::new(false);
        legacy_file_inventory(&path, &raw, entry, plain_text_profile, deadline, &cancelled)
    };
    result().map_err(|error| format!("native source inventory failed: {error:?}"))
}

fn invoke_item_inventory(request: &Value) -> Result<Value, String> {
    let requested_repo = request
        .get("repo_root")
        .and_then(Value::as_str)
        .ok_or("inventory request repo_root is required")?;
    let repo_root = fs::canonicalize(requested_repo)
        .map_err(|error| format!("inventory repository root cannot be resolved: {error}"))?;
    let item_manifest_ref = request
        .get("item_manifest_ref")
        .and_then(Value::as_str)
        .ok_or("inventory request item_manifest_ref is required")?;
    let requested_payload = request
        .get("payload_source_root")
        .and_then(Value::as_str)
        .ok_or("inventory request payload_source_root is required")?;
    if requested_payload.is_empty() {
        return Err("inventory request payload_source_root must be nonempty text".into());
    }
    let payload_source_root = absolute_path(Path::new(requested_payload))?;
    let event_date = request_text_or(request, "event_date", "2026-07-28")?;
    if !valid_event_date(event_date) {
        return Err("--event-date must use YYYY-MM-DD".into());
    }
    let plain_text_profile = request_text_or(request, "plain_text_profile", "plain_utf8_file_v1")?;
    if !matches!(plain_text_profile, "plain_utf8_file_v1" | "plain_text_v1") {
        return Err("unknown plain text inventory profile".into());
    }
    match build_full_inventory_optional(
        &repo_root,
        item_manifest_ref,
        &payload_source_root,
        event_date,
        plain_text_profile,
    )? {
        Some(inventory) => Ok(inventory),
        None => Ok(Value::Null),
    }
}

/// Native CLI behavior for the original build/check entrypoint. Writes are
/// confined to each Item's declared inventory path; schema and file fixity are
/// checked before an output is published.
pub(crate) fn invoke(request: &Value) -> Result<Value, String> {
    match request.get("operation") {
        Some(Value::String(operation)) if operation == "metadata" => {
            return Ok(inventory_metadata());
        }
        Some(Value::String(operation)) if operation == "file" => {
            return invoke_file_inventory(request);
        }
        Some(Value::String(operation)) if operation == "item" => {
            return invoke_item_inventory(request);
        }
        Some(Value::String(operation)) if operation == "build" => (),
        None => (),
        Some(Value::String(_)) => return Err("unknown source inventory operation".into()),
        Some(_) => return Err("inventory request operation must be text".into()),
    }
    let requested_repo = request
        .get("repo_root")
        .and_then(Value::as_str)
        .ok_or("inventory request repo_root is required")?;
    let repo_root = fs::canonicalize(requested_repo)
        .map_err(|error| format!("inventory repository root cannot be resolved: {error}"))?;
    let requested_payload = match request.get("payload_source_root") {
        None => repo_root.join("ToS/source-witnesses"),
        Some(Value::String(path)) if !path.is_empty() => PathBuf::from(path),
        Some(_) => {
            return Err("inventory request payload_source_root must be nonempty text".into());
        }
    };
    let payload_source_root = absolute_path(&requested_payload)?;
    let event_date = request_text_or(request, "event_date", "2026-07-28")?;
    if !valid_event_date(event_date) {
        return Err("--event-date must use YYYY-MM-DD".into());
    }
    let plain_text_profile = request_text_or(request, "plain_text_profile", "plain_utf8_file_v1")?;
    if !matches!(plain_text_profile, "plain_utf8_file_v1" | "plain_text_v1") {
        return Err("unknown plain text inventory profile".into());
    }
    let check = match request.get("check") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err("inventory request check must be boolean".into()),
    };

    let probe = inventory_schema_probe(&repo_root)?;
    let mut outputs: Vec<(String, PathBuf, Vec<u8>)> = Vec::new();
    let mut skipped = 0usize;
    for item_manifest_ref in inventory_manifests(&repo_root)? {
        let Some(inventory) = build_full_inventory_optional(
            &repo_root,
            &item_manifest_ref,
            &payload_source_root,
            event_date,
            plain_text_profile,
        )?
        else {
            skipped += 1;
            continue;
        };
        validate_inventory_value(&probe, &inventory)?;
        let body = render_inventory(&inventory)?;
        let output_ref = format!(
            "{}resource-inventory.json",
            item_manifest_ref
                .strip_suffix("item.manifest.json")
                .ok_or("Item manifest reference does not end in item.manifest.json")?
        );
        let output_path = crate::source_acquisition_batch::path_under(&repo_root, &output_ref)?;
        outputs.push((output_ref, output_path, body));
    }

    let mut drift = Vec::new();
    let mut changed = 0usize;
    if check {
        for (reference, path, expected) in &outputs {
            if output_current(path)?.as_deref() != Some(expected.as_slice()) {
                drift.push(reference.clone());
            }
        }
    } else {
        for (_, path, expected) in &outputs {
            if output_current(path)?.as_deref() != Some(expected.as_slice()) {
                write_inventory(path, expected)?;
                changed += 1;
            }
        }
    }
    Ok(json!({
        "check":check,
        "processed":outputs.len(),
        "skipped":skipped,
        "changed":changed,
        "drift":drift
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::Write;

    const ITEM_REF: &str =
        "ToS/source-witnesses/works/fixture/editions/test/items/native-full/item.manifest.json";

    fn fixture_inventory(
        media_type: &str,
        filename: &str,
        payload: &[u8],
        plain_text_profile: &str,
    ) -> Value {
        let temporary = tempfile::tempdir().expect("temporary fixture directory");
        let repository = temporary.path().join("repository");
        let payload_root = temporary.path().join("payload-root");
        let source_relative =
            format!("works/fixture/editions/test/items/native-full/payload/{filename}");
        let manifest_path = repository.join(ITEM_REF);
        let payload_path = payload_root.join(&source_relative);
        fs::create_dir_all(manifest_path.parent().unwrap()).expect("manifest parents");
        fs::create_dir_all(payload_path.parent().unwrap()).expect("payload parents");
        fs::write(&payload_path, payload).expect("payload bytes");
        let digest = Digest256::of_bytes(payload).to_hex();
        let inventory_ref = ITEM_REF
            .strip_suffix("item.manifest.json")
            .unwrap()
            .to_owned()
            + "resource-inventory.json";
        let manifest = json!({
            "item_id":"tos.item.fixture.native-full",
            "payload_files":[{
                "file_id":format!("tos.file.sha256.{digest}"),
                "relative_path":format!("payload/{filename}"),
                "original_basename":filename,
                "media_type":media_type,
                "byte_size":payload.len(),
                "sha256":digest
            }],
            "resource_inventory_ref":inventory_ref
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("manifest bytes");

        let inventory = build_full_inventory(
            &repository,
            ITEM_REF,
            &payload_root,
            "2026-10-01",
            plain_text_profile,
        )
        .expect("native full inventory");
        assert_eq!(
            inventory["schema_version"],
            "tos_source_resource_inventory_v1"
        );
        assert_eq!(inventory["item_id"], "tos.item.fixture.native-full");
        assert_eq!(inventory["files"][0]["file_sha256"], digest);
        assert_eq!(inventory["source_text_included"], false);
        assert!(
            !repository.join(inventory_ref).exists(),
            "builder must return metadata without publishing it"
        );
        inventory
    }

    fn djvu_page(width: u16, height: u16, dpi: u16) -> Vec<u8> {
        let mut info = Vec::from(width.to_be_bytes());
        info.extend_from_slice(&height.to_be_bytes());
        info.extend_from_slice(&[0, 0]);
        info.extend_from_slice(&dpi.to_le_bytes());
        info.extend_from_slice(&[22, 0]);
        let mut page = b"FORM".to_vec();
        let form_size = 4 + 8 + info.len();
        page.extend_from_slice(&(form_size as u32).to_be_bytes());
        page.extend_from_slice(b"DJVUINFO");
        page.extend_from_slice(&(info.len() as u32).to_be_bytes());
        page.extend_from_slice(&info);
        let root_size = form_size as u32;
        let mut raw = b"AT&T".to_vec();
        raw.extend_from_slice(&page[0..4]);
        raw.extend_from_slice(&root_size.to_be_bytes());
        raw.extend_from_slice(&page[8..]);
        raw
    }

    fn put_le16(output: &mut Vec<u8>, value: u16) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn put_le32(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn stored_zip_member(name: &str, payload: &[u8]) -> Vec<u8> {
        let name = name.as_bytes();
        let crc = crc32fast::hash(payload);
        let size = payload.len() as u32;
        let mut raw = Vec::new();
        put_le32(&mut raw, 0x0403_4b50);
        put_le16(&mut raw, 20);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le32(&mut raw, crc);
        put_le32(&mut raw, size);
        put_le32(&mut raw, size);
        put_le16(&mut raw, name.len() as u16);
        put_le16(&mut raw, 0);
        raw.extend_from_slice(name);
        raw.extend_from_slice(payload);
        let directory_offset = raw.len() as u32;

        put_le32(&mut raw, 0x0201_4b50);
        put_le16(&mut raw, 20);
        put_le16(&mut raw, 20);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le32(&mut raw, crc);
        put_le32(&mut raw, size);
        put_le32(&mut raw, size);
        put_le16(&mut raw, name.len() as u16);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le32(&mut raw, 0);
        put_le32(&mut raw, 0);
        raw.extend_from_slice(name);
        let directory_size = raw.len() as u32 - directory_offset;

        put_le32(&mut raw, 0x0605_4b50);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 0);
        put_le16(&mut raw, 1);
        put_le16(&mut raw, 1);
        put_le32(&mut raw, directory_size);
        put_le32(&mut raw, directory_offset);
        put_le16(&mut raw, 0);
        raw
    }

    #[test]
    fn legacy_xml_profiles_keep_page_counts_without_source_text() {
        let scandata = br#"<book><bookData><leafCount>1</leafCount><dpi>300</dpi></bookData><pageData><page leafNum="0"><origWidth>1200</origWidth><origHeight>1800</origHeight></page></pageData></book>"#;
        let inventory = fixture_inventory(
            "application/xml",
            "book_scandata.xml",
            scandata,
            "plain_utf8_file_v1",
        );
        assert_eq!(inventory["files"][0]["profile"], "scandata_pages_v1");
        assert_eq!(inventory["files"][0]["summary"]["page_count"], 1);

        let djvu_xml = br#"<DjVuXML><BODY><OBJECT width="1200" height="1800"><PARAM name="DPI" value="300"/><PARAGRAPH><LINE><WORD>private phrase</WORD></LINE></PARAGRAPH></OBJECT></BODY></DjVuXML>"#;
        let inventory = fixture_inventory(
            "application/vnd.djvu+xml",
            "book.djvu.xml",
            djvu_xml,
            "plain_utf8_file_v1",
        );
        let encoded = serde_json::to_string(&inventory).unwrap();
        assert_eq!(inventory["files"][0]["profile"], "djvu_xml_pages_v1");
        assert_eq!(inventory["files"][0]["summary"]["word_count"], 1);
        assert!(!encoded.contains("private phrase"));
    }

    #[test]
    fn binary_djvu_profile_preserves_page_geometry() {
        let inventory = fixture_inventory(
            "image/vnd.djvu",
            "book.djvu",
            &djvu_page(1200, 1800, 300),
            "plain_utf8_file_v1",
        );
        assert_eq!(inventory["files"][0]["profile"], "djvu_pages_v1");
        assert_eq!(
            inventory["files"][0]["resources"][0]["locator"]["width_pixels"],
            1200
        );
        assert_eq!(
            inventory["files"][0]["resources"][0]["locator"]["resolution_dpi"],
            300
        );
    }

    #[test]
    fn abbyy_gzip_profile_counts_pages_words_and_fingerprints_only() {
        let source = br#"<document xmlns="http://www.abbyy.com/FineReader_xml/FineReader6-schema-v1.xml"><page width="1200" height="1800" resolution="300"><block><par><line><formatting><charParams wordStart="true">private</charParams><charParams wordStart="false"> text</charParams></formatting></line></par></block></page></document>"#;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(source).unwrap();
        let payload = encoder.finish().unwrap();
        let inventory = fixture_inventory(
            "application/gzip",
            "scan.abbyy.xml.gz",
            &payload,
            "plain_utf8_file_v1",
        );
        let encoded = serde_json::to_string(&inventory).unwrap();
        assert_eq!(inventory["files"][0]["profile"], "abbyy_xml_pages_v1");
        assert_eq!(inventory["files"][0]["summary"]["word_count"], 1);
        assert!(!encoded.contains("private text"));
    }

    #[test]
    fn jp2_zip_profile_binds_page_member_bytes_and_order() {
        let payload = stored_zip_member("tree/page_0000.jp2", b"codestream");
        let inventory = fixture_inventory(
            "application/zip",
            "book_jp2.zip",
            &payload,
            "plain_utf8_file_v1",
        );
        assert_eq!(inventory["files"][0]["profile"], "jp2_zip_pages_v1");
        assert_eq!(inventory["files"][0]["summary"]["member_count"], 1);
        assert_eq!(
            inventory["files"][0]["resources"][0]["locator"]["leaf_number"],
            0
        );
        assert_eq!(
            inventory["files"][0]["resources"][0]["sha256"],
            Digest256::of_bytes(b"codestream").to_hex()
        );
    }

    #[test]
    fn plain_text_and_plain_utf8_remain_distinct_profiles() {
        let payload = "e\u{301}\r\ntext".as_bytes();
        let opaque = fixture_inventory("text/plain", "plain.txt", payload, "plain_text_v1");
        assert_eq!(opaque["files"][0]["profile"], "plain_text_v1");
        assert_eq!(
            opaque["files"][0]["resources"][0]["content_fingerprint"]["normalization"],
            "unicode-codepoints-preserved"
        );

        let measured = fixture_inventory("text/plain", "plain.txt", payload, "plain_utf8_file_v1");
        assert_eq!(measured["files"][0]["profile"], "plain_utf8_file_v1");
        assert_eq!(measured["files"][0]["utf8_observation"]["crlf_count"], 1);
    }

    #[test]
    fn explicit_inventory_selectors_fail_closed() {
        let temporary = tempfile::tempdir().expect("temporary fixture directory");
        let root = temporary.path().to_string_lossy().into_owned();
        assert_eq!(
            request_text_or(&json!({}), "event_date", "2026-07-28"),
            Ok("2026-07-28")
        );
        assert!(invoke(&json!({"operation":false})).is_err());
        assert!(
            invoke(&json!({
                "operation":"build",
                "repo_root":root,
                "payload_source_root":null
            }))
            .is_err()
        );
        assert!(
            invoke(&json!({
                "operation":"build",
                "repo_root":root,
                "payload_source_root":root,
                "event_date":null
            }))
            .is_err()
        );
        assert!(
            invoke(&json!({
                "operation":"build",
                "repo_root":root,
                "payload_source_root":root,
                "event_date":"2026-10-01",
                "plain_text_profile":null
            }))
            .is_err()
        );
        assert!(
            invoke(&json!({
                "operation":"item",
                "repo_root":root,
                "item_manifest_ref":"ToS/source-witnesses/fixture/item.manifest.json",
                "payload_source_root":root,
                "event_date":false
            }))
            .is_err()
        );
        assert!(invoke(&json!({
            "operation":"file",
            "payload_path":"/not-read-before-selector-validation",
            "payload_entry":{"byte_size":0,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
            "plain_text_profile":null
        })).is_err());
    }

    #[test]
    fn pdfinfo_geometry_parser_requires_every_page_and_preserves_rotation() {
        let output =
            "Page 1 size: 612 x 792 pts (letter)\nPage 1 rot: 90\nPage 2 size: 612.5 x 792 pts\n";
        let (sizes, rotations) = pdf_page_geometry(output, 2).expect("page geometries");
        assert_eq!(sizes[&1], (612.0, 792.0));
        assert_eq!(sizes[&2], (612.5, 792.0));
        assert_eq!(rotations[&1], 90);
        assert!(pdf_page_geometry("Page 1 size: 612 x 792 pts\n", 2).is_err());
    }

    #[test]
    fn native_cli_builds_schema_validates_and_checks_the_declared_inventory() {
        let temporary = tempfile::tempdir().expect("temporary fixture directory");
        let repository = temporary.path().join("repository");
        let source_root = repository.join("ToS/source-witnesses");
        let item_root = source_root.join("works/fixture/editions/test/items/native-cli");
        let payload_path = item_root.join("payload/source.json");
        let manifest_path = item_root.join("item.manifest.json");
        let schema_path = repository.join(INVENTORY_SCHEMA_REF);
        fs::create_dir_all(payload_path.parent().unwrap()).expect("payload parents");
        fs::create_dir_all(schema_path.parent().unwrap()).expect("schema parents");
        fs::write(
            schema_path,
            include_bytes!("../../../../ToS/contracts/source-resource-inventory.schema.json"),
        )
        .expect("schema source");
        let payload = br#"{"member":"private text"}"#;
        fs::write(&payload_path, payload).expect("payload bytes");
        let digest = Digest256::of_bytes(payload).to_hex();
        let inventory_ref = "ToS/source-witnesses/works/fixture/editions/test/items/native-cli/resource-inventory.json";
        let manifest = json!({
            "item_id":"tos.item.fixture.native-cli",
            "payload_files":[{
                "file_id":format!("tos.file.sha256.{digest}"),
                "relative_path":"payload/source.json",
                "original_basename":"source.json",
                "media_type":"application/json",
                "byte_size":payload.len(),
                "sha256":digest
            }],
            "resource_inventory_ref":inventory_ref
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("manifest bytes");

        let request = json!({
            "repo_root":repository,
            "event_date":"2026-10-01"
        });
        let item_manifest_ref =
            "ToS/source-witnesses/works/fixture/editions/test/items/native-cli/item.manifest.json";
        let item = invoke(&json!({
            "operation":"item",
            "repo_root":repository,
            "item_manifest_ref":item_manifest_ref,
            "payload_source_root":source_root,
            "event_date":"2026-10-01"
        }))
        .expect("native per-item inventory");
        assert_eq!(item["files"][0]["profile"], "json_members_v1");
        assert!(!repository.join(inventory_ref).exists());

        let result = invoke(&request).expect("native inventory build");
        assert_eq!(result["processed"], 1);
        assert_eq!(result["skipped"], 0);
        assert_eq!(result["changed"], 1);
        let output_path = repository.join(inventory_ref);
        let output = fs::read(&output_path).expect("published inventory");
        let decoded: Value = serde_json::from_slice(&output).expect("inventory JSON");
        assert_eq!(decoded["files"][0]["profile"], "json_members_v1");
        assert!(
            !String::from_utf8(output.clone())
                .unwrap()
                .contains("private text")
        );

        let check = invoke(&json!({
            "repo_root":repository,
            "event_date":"2026-10-01",
            "check":true
        }))
        .expect("native inventory check");
        assert_eq!(check["drift"], json!([]));
        fs::write(&output_path, b"{}\n").expect("introduce fixture drift");
        let drift = invoke(&json!({
            "repo_root":repository,
            "event_date":"2026-10-01",
            "check":true
        }))
        .expect("native inventory drift check");
        assert_eq!(drift["drift"], json!([inventory_ref]));
    }

    #[test]
    fn native_imported_file_consumer_checks_fixity_and_metadata_is_owner_supplied() {
        let temporary = tempfile::tempdir().expect("temporary fixture directory");
        let payload_path = temporary.path().join("plain.txt");
        let payload = b"bounded local source\n";
        fs::write(&payload_path, payload).expect("payload bytes");
        let digest = Digest256::of_bytes(payload).to_hex();
        let entry = json!({
            "file_id":"tos.file.fixture.native-file-operation",
            "relative_path":"payload/plain.txt",
            "media_type":"text/plain",
            "byte_size":payload.len(),
            "sha256":digest
        });
        let inventory = invoke(&json!({
            "operation":"file",
            "payload_path":payload_path,
            "payload_entry":entry
        }))
        .expect("native file inventory operation");
        assert_eq!(inventory["profile"], "plain_utf8_file_v1");
        assert_eq!(inventory["file_sha256"], digest);

        let metadata = invoke(&json!({"operation":"metadata"})).expect("native metadata");
        assert_eq!(metadata["schema_ref"], super::super::INVENTORY_SCHEMA);
        assert_eq!(metadata["generator_version"], "2");
        assert_eq!(metadata["max_plain_utf8_bytes"], MAX_PLAIN_UTF8_BYTES);
        assert_eq!(
            metadata["authority_boundaries"]["1"],
            LEGACY_AUTHORITY_BOUNDARY_V1
        );

        let mut wrong_fixity = entry.clone();
        wrong_fixity["sha256"] = json!("0".repeat(64));
        assert!(
            invoke(&json!({
                "operation":"file",
                "payload_path":payload_path,
                "payload_entry":wrong_fixity
            }))
            .is_err()
        );

        assert!(invoke(&json!({"operation":null})).is_err());
        assert!(
            invoke(&json!({
                "repo_root":temporary.path(),
                "payload_source_root":null
            }))
            .is_err()
        );
        assert!(
            invoke(&json!({
                "operation":"item",
                "repo_root":temporary.path(),
                "item_manifest_ref":"ToS/source-witnesses/fixture/item.manifest.json",
                "payload_source_root":temporary.path(),
                "event_date":null
            }))
            .is_err()
        );
        assert!(
            invoke(&json!({
                "operation":"file",
                "payload_path":payload_path,
                "payload_entry":entry,
                "plain_text_profile":null
            }))
            .is_err()
        );
    }
}
