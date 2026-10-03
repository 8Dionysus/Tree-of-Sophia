//! Text-free Item inventory from the actual granted input descriptor.
//! Enumeration and one-way navigation fingerprints confer no content rights.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_item_deposit::payload_entry;
use crate::source_text_layer_zip::{read_bounded_member, visit_members};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonValue as FoundationJsonValue, parse_json,
};
use unicode_normalization::{UnicodeNormalization, is_nfc, is_nfd};
use xml::reader::{ParserConfig, XmlEvent};

include!("source_item_html_entities.rs");

#[path = "source_item_inventory_extended.rs"]
mod extended;

/// Build the complete standalone source-resource inventory from an Item
/// manifest and explicitly selected local payload root. The caller publishes
/// and schema-validates the returned wrapper; this function only observes.
pub(crate) fn build_full_inventory(
    repo_root: &Path,
    item_manifest_ref: &str,
    payload_source_root: &Path,
    event_date: &str,
    plain_text_profile: &str,
) -> std::result::Result<Value, String> {
    extended::build_full_inventory(
        repo_root,
        item_manifest_ref,
        payload_source_root,
        event_date,
        plain_text_profile,
    )
}

/// Run the owned source-resource inventory build/check command.
pub(crate) fn invoke(request: &Value) -> std::result::Result<Value, String> {
    extended::invoke(request)
}

fn unsupported() -> SourceCommandError {
    SourceCommandError::Unsupported("bounded native Item inventory")
}
fn whitespace(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
fn fingerprint(parts: &[String]) -> Value {
    let nfc = parts.join(" ").nfc().collect::<String>();
    let normalized = nfc
        .split(whitespace)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    json!({"algorithm":"sha256","normalization":"unicode-nfc-whitespace-collapse",
        "sha256":Digest256::of_bytes(normalized.as_bytes()).to_hex(),"character_count":normalized.chars().count()})
}
fn unescape(input: &str) -> String {
    let mut out = String::new();
    let mut offset = 0;
    while let Some(relative) = input[offset..].find('&') {
        let start = offset + relative;
        out.push_str(&input[offset..start]);
        let rest = &input[start + 1..];
        let length = rest
            .chars()
            .take_while(|c| !matches!(c, '\t' | '\n' | '\u{c}' | ' ' | '<' | '&' | '#' | ';'))
            .take(32)
            .map(char::len_utf8)
            .sum::<usize>();
        if rest.starts_with('#') {
            let (hex, begin) = if rest.starts_with("#x") || rest.starts_with("#X") {
                (true, 2)
            } else {
                (false, 1)
            };
            let end = rest[begin..]
                .bytes()
                .take_while(|c| {
                    if hex {
                        c.is_ascii_hexdigit()
                    } else {
                        c.is_ascii_digit()
                    }
                })
                .count()
                + begin;
            if end > begin {
                let value = u32::from_str_radix(&rest[begin..end], if hex { 16 } else { 10 })
                    .unwrap_or(0x110000);
                let cp = match value {
                    0 | 0xd800..=0xdfff | 0x110000..=u32::MAX => 0xfffd,
                    0x80 => 0x20ac,
                    0x82 => 0x201a,
                    0x83 => 0x192,
                    0x84 => 0x201e,
                    0x85 => 0x2026,
                    0x86 => 0x2020,
                    0x87 => 0x2021,
                    0x88 => 0x2c6,
                    0x89 => 0x2030,
                    0x8a => 0x160,
                    0x8b => 0x2039,
                    0x8c => 0x152,
                    0x8e => 0x17d,
                    0x91 => 0x2018,
                    0x92 => 0x2019,
                    0x93 => 0x201c,
                    0x94 => 0x201d,
                    0x95 => 0x2022,
                    0x96 => 0x2013,
                    0x97 => 0x2014,
                    0x98 => 0x2dc,
                    0x99 => 0x2122,
                    0x9a => 0x161,
                    0x9b => 0x203a,
                    0x9c => 0x153,
                    0x9e => 0x17e,
                    0x9f => 0x178,
                    v => v,
                };
                if !(matches!(cp,1..=8|11|14..=31|127|0xfdd0..=0xfdef)
                    || (cp & 0xffff == 0xfffe)
                    || (cp & 0xffff == 0xffff))
                {
                    if let Some(c) = char::from_u32(cp) {
                        out.push(c);
                    }
                }
                offset = start + 1 + end + usize::from(rest.as_bytes().get(end) == Some(&b';'));
                continue;
            }
        }
        let end = length + usize::from(rest.as_bytes().get(length) == Some(&b';'));
        let mut matched = None;
        for width in (1..=end).rev() {
            if !rest.is_char_boundary(width) {
                continue;
            }
            if let Ok(index) =
                HTML5_ENTITIES.binary_search_by_key(&&rest[..width], |(name, _)| name)
            {
                matched = Some((width, HTML5_ENTITIES[index].1));
                break;
            }
        }
        if let Some((width, value)) = matched {
            out.push_str(value);
            offset = start + 1 + width;
        } else {
            out.push('&');
            offset = start + 1;
        }
    }
    out.push_str(&input[offset..]);
    out
}
fn html_fingerprint(
    raw: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let text = String::from_utf8_lossy(raw);
    let mut parts = Vec::new();
    let mut offset = 0;
    let mut hidden: Option<String> = None;
    while offset < text.len() {
        active(deadline, cancelled)?;
        if let Some(tag) = &hidden {
            let lower = text[offset..].to_ascii_lowercase();
            let marker = format!("</{tag}");
            let Some(found) = lower.find(&marker) else {
                break;
            };
            offset += found;
        }
        let Some(relative) = text[offset..].find('<') else {
            if hidden.is_none() {
                parts.push(unescape(&text[offset..]));
            }
            break;
        };
        let start = offset + relative;
        if start > offset && hidden.is_none() {
            parts.push(unescape(&text[offset..start]));
        }
        let tail = &text[start..];
        if tail.starts_with("<!--") {
            let end = tail.find("-->").ok_or_else(unsupported)?;
            offset = start + end + 3;
            continue;
        }
        if tail.starts_with("<![CDATA[") {
            let end = tail.find("]]>").ok_or_else(unsupported)?;
            offset = start + end + 3;
            continue;
        }
        let mut quote = None;
        let mut closing = None;
        for (index, c) in tail.char_indices().skip(1) {
            match (quote, c) {
                (Some(q), v) if q == v => quote = None,
                (Some(_), _) => (),
                (None, '\'' | '"') => quote = Some(c),
                (None, '>') => {
                    closing = Some(index);
                    break;
                }
                _ => (),
            }
        }
        let Some(end) = closing else {
            return Err(unsupported());
        };
        if end > 65536 {
            return Err(unsupported());
        }
        let body = &tail[1..end];
        let endtag = body.starts_with('/');
        let content = body.strip_prefix('/').unwrap_or(body);
        let name = content
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_'))
            .collect::<String>()
            .to_ascii_lowercase();
        if name.is_empty() && !body.starts_with(['!', '?']) {
            if hidden.is_none() {
                parts.push("<".to_owned());
            }
            offset = start + 1;
            continue;
        }
        if endtag {
            if hidden.as_ref() == Some(&name) {
                hidden = None;
            }
        } else if matches!(name.as_str(), "script" | "style") && !body.ends_with('/') {
            hidden = Some(name);
        }
        offset = start + end + 1;
    }
    Ok(fingerprint(&parts))
}
fn xml_events(
    raw: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
    mut visit: impl FnMut(
        &[String],
        &xml::name::OwnedName,
        &[xml::attribute::OwnedAttribute],
    ) -> SourceCommandResult<()>,
) -> SourceCommandResult<()> {
    let config = ParserConfig::new()
        .max_name_length(65536)
        .max_attributes(128)
        .max_attribute_length(65536)
        .max_data_length(16 * 1024 * 1024)
        .allow_multiple_root_elements(false)
        .ignore_end_of_stream(false)
        .replace_unknown_entity_references(false);
    let mut stack = Vec::new();
    let mut count = 0;
    for event in config.create_reader(raw) {
        active(deadline, cancelled)?;
        count += 1;
        if count > 262144 {
            return Err(unsupported());
        }
        match event.map_err(|_| unsupported())? {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                if stack.len() >= 128 {
                    return Err(unsupported());
                }
                visit(&stack, &name, &attributes)?;
                stack.push(name.local_name);
            }
            XmlEvent::EndElement { .. } => {
                stack.pop().ok_or_else(unsupported)?;
            }
            _ => (),
        }
    }
    if !stack.is_empty() {
        return Err(unsupported());
    }
    Ok(())
}
fn attr<'a>(attrs: &'a [xml::attribute::OwnedAttribute], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|a| a.name.local_name == name && a.name.namespace.is_none())
        .map(|a| a.value.as_str())
}
fn member_join(parent: &str, href: &str) -> SourceCommandResult<String> {
    let mut parts = if href.starts_with('/') {
        Vec::new()
    } else {
        parent
            .split('/')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    for part in href.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                if parts.pop().is_none() {
                    return Err(unsupported());
                }
            }
            v => parts.push(v.to_owned()),
        }
    }
    Ok(parts.join("/"))
}
fn guessed_media(name: &str) -> SourceCommandResult<&'static str> {
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    Ok(match ext.as_deref() {
        None | Some("ncx") => "application/octet-stream",
        Some("opf") => "application/oebps-package+xml",
        Some("xml") => "text/xml",
        Some("xhtml" | "xht") => "application/xhtml+xml",
        Some("html" | "htm") => "text/html",
        Some("css") => "text/css",
        Some("txt") => "text/plain",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg" | "svgz") => "image/svg+xml",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("js") => "text/javascript",

        _ => return Err(unsupported()),
    })
}
fn epub(
    file: &mut File,
    config: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Value> {
    let size = config["byte_size"].as_u64().ok_or_else(unsupported)?;
    authorize()?;
    let container = read_bounded_member(
        file,
        size,
        "META-INF/container.xml",
        deadline,
        cancelled,
        authorize,
    )?;
    let mut rootfile = None;
    xml_events(&container, deadline, cancelled, |_, name, attrs| {
        if name.local_name == "rootfile" && rootfile.is_none() {
            rootfile = attr(attrs, "full-path").map(str::to_owned);
        }
        Ok(())
    })?;
    let rootfile = rootfile.filter(|v| !v.is_empty()).ok_or_else(unsupported)?;
    authorize()?;
    let opf = read_bounded_member(file, size, &rootfile, deadline, cancelled, authorize)?;
    let parent = rootfile.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    let mut manifest = BTreeMap::new();
    let mut ids = BTreeMap::new();
    let mut refs = Vec::new();
    xml_events(&opf, deadline, cancelled, |stack, name, attrs| {
        if name.local_name == "item" && stack.last().is_some_and(|p| p == "manifest") {
            if let (Some(id), Some(href)) = (
                attr(attrs, "id").filter(|v| !v.is_empty()),
                attr(attrs, "href").filter(|v| !v.is_empty()),
            ) {
                let path = member_join(parent, href)?;
                ids.insert(id.to_owned(), path.clone());
                manifest.insert(
                    path,
                    (
                        attr(attrs, "media-type")
                            .filter(|v| !v.is_empty())
                            .unwrap_or("application/octet-stream")
                            .to_owned(),
                        attr(attrs, "properties").unwrap_or("").to_owned(),
                    ),
                );
            }
        } else if name.local_name == "itemref" && stack.last().is_some_and(|p| p == "spine") {
            refs.push(attr(attrs, "idref").unwrap_or("").to_owned());
        }
        Ok(())
    })?;
    let mut spine = BTreeMap::new();
    for (index, id) in refs.iter().enumerate() {
        if let Some(path) = ids.get(id) {
            spine.insert(path.clone(), index + 1);
        }
    }
    let mut resources = Vec::new();
    let mut spine_indexes = BTreeSet::new();
    let mut xhtml_count = 0;
    let mut images = 0;
    authorize()?;
    visit_members(file, size, deadline, cancelled, authorize, |name, raw| {
        let declared = manifest.get(name);
        let media = if let Some((media, _)) = declared {
            media.as_str()
        } else if name == "mimetype" {
            "application/epub+zip"
        } else {
            guessed_media(name)?
        };
        let mut resource = json!({"resource_id":format!("epub-member-{:04}",resources.len()+1),"resource_kind":"epub_member",
            "locator":{"member_path":name,"container_order":resources.len()+1},"media_type":media,"byte_size":raw.len(),"sha256":Digest256::of_bytes(raw).to_hex(),
            "structural_role":if name=="META-INF/container.xml"{"container_metadata"}else if name.ends_with(".opf"){"package_metadata"}
                else if declared.is_some_and(|(_,props)|props.split(whitespace).any(|p|p=="nav")){"navigation"}else if spine.contains_key(name){"spine_resource"}else{"auxiliary_resource"}});
        if let Some(index) = spine.get(name) {
            resource["locator"]["spine_index"] = json!(index);
            spine_indexes.insert(*index);
        }
        if matches!(media, "application/xhtml+xml" | "text/html") {
            xhtml_count += 1;
            resource["content_fingerprint"] = html_fingerprint(raw, deadline, cancelled)?;
        }
        if media.starts_with("image/") {
            images += 1;
        }
        resources.push(resource);
        Ok(())
    })?;
    Ok(
        json!({"file_id":config["file_id"],"file_sha256":config["sha256"],"media_type":config["media_type"],"profile":"epub_resources_v1",
        "summary":{"resource_count":resources.len(),"member_count":resources.len(),"spine_item_count":spine_indexes.len(),"xhtml_count":xhtml_count,"image_resource_count":images},"resources":resources}),
    )
}
pub(crate) fn observe(
    file: &mut File,
    config: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
    authorize: &mut impl FnMut() -> SourceCommandResult<()>,
) -> SourceCommandResult<Value> {
    authorize()?;
    match config["media_type"].as_str() {
        Some("application/epub+zip") => epub(file, config, deadline, cancelled, authorize),
        Some("text/plain" | "text/markdown") => {
            let size = config["byte_size"].as_u64().ok_or_else(unsupported)?;
            if !(1..=131072).contains(&size) {
                return Err(unsupported());
            }
            file.seek(SeekFrom::Start(0)).map_err(|_| unsupported())?;
            let mut raw = Vec::with_capacity(size as usize);
            let mut buffer = [0u8; 65536];
            loop {
                active(deadline, cancelled)?;
                authorize()?;
                let take = ((size + 1).saturating_sub(raw.len() as u64) as usize).min(buffer.len());
                if take == 0 {
                    break;
                }
                match file.read(&mut buffer[..take]) {
                    Ok(0) => break,
                    Ok(n) => raw.extend_from_slice(&buffer[..n]),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err(unsupported()),
                }
            }
            authorize()?;
            let text = std::str::from_utf8(&raw).map_err(|_| unsupported())?;
            if text.contains('\0')
                || raw.len() as u64 != size
                || Digest256::of_bytes(&raw).to_hex()
                    != config["sha256"].as_str().ok_or_else(unsupported)?
            {
                return Err(unsupported());
            }
            let crlf = text.matches("\r\n").count();
            let nfc = is_nfc(text);
            let nfd = is_nfd(text);
            let entry = payload_entry(config)?;
            Ok(
                json!({"file_id":config["file_id"],"file_sha256":config["sha256"],"media_type":config["media_type"],"profile":"plain_utf8_file_v1","summary":{"resource_count":1},
                "utf8_observation":{"encoding":"UTF-8","bom_byte_count":if raw.starts_with(&[0xef,0xbb,0xbf]){3}else{0},"code_point_count":text.chars().count(),"code_point_count_includes_bom":true,
                    "crlf_count":crlf,"lone_cr_count":text.matches('\r').count()-crlf,"lone_lf_count":text.matches('\n').count()-crlf,
                    "terminal_newline":if text.ends_with("\r\n"){"crlf"}else if text.ends_with('\r'){"cr"}else if text.ends_with('\n'){"lf"}else{"none"},
                    "normalization_observation":match(nfc,nfd){(true,true)=>"nfc_and_nfd",(true,false)=>"nfc",(false,true)=>"nfd",(false,false)=>"neither"},
                    "unicode_version":"16.0.0","normalization_performed":false,"markup_interpretation_performed":false},
                "resources":[{"resource_id":"plain-utf8-file","resource_kind":"plain_text_file","locator":{"byte_start":0,"byte_end":size},
                    "media_type":entry["media_type"],"byte_size":size,"sha256":entry["sha256"],"structural_role":"contents"}]}),
            )
        }
        _ => Err(unsupported()),
    }
}

const INVENTORY_SCHEMA: &str =
    "https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json";
const INVENTORY_AUTHORITY_BOUNDARY: &str = "This inventory records resource enumeration, geometry, ordering, counts and one-way fingerprints for the selected source.";
const INVENTORY_GENERATOR_VERSION: &str = "2";
const TEI_NAMESPACE: &str = "http://www.tei-c.org/ns/1.0";
const OSIS_NAMESPACE: &str = "http://www.bibletechnologies.net/2003/OSIS/namespace";
const MAX_INVENTORY_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_INVENTORY_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_INVENTORY_JSON_VISITS: usize = 300_000;

fn inventory_read_json(path: &Path) -> SourceCommandResult<Value> {
    let raw = crate::source_acquisition_batch::read_bytes(
        path,
        None,
        false,
        false,
        MAX_INVENTORY_MANIFEST_BYTES,
    )
    .map_err(|_| SourceCommandError::Unsupported("source inventory JSON read"))?;
    crate::source_acquisition_batch::parse(&raw)
        .map_err(|_| SourceCommandError::Invalid("source inventory JSON"))
}

fn inventory_json_field<'a>(value: &'a Value, key: &str) -> SourceCommandResult<&'a Value> {
    value.get(key).ok_or(SourceCommandError::Invalid(
        "source inventory manifest field",
    ))
}

fn inventory_json_text<'a>(value: &'a Value, key: &str) -> SourceCommandResult<&'a str> {
    inventory_json_field(value, key)?
        .as_str()
        .ok_or(SourceCommandError::Invalid(
            "source inventory manifest text",
        ))
}

fn inventory_safe_ref(reference: &str) -> SourceCommandResult<()> {
    crate::source_acquisition_batch::safe_ref(reference)
        .map_err(|_| SourceCommandError::Invalid("source inventory relative path"))
}

fn inventory_digest(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}

fn inventory_exact_fingerprint(text: &str, normalization: &str) -> Value {
    json!({"algorithm":"sha256","normalization":normalization,
        "sha256":inventory_digest(text.as_bytes()),"character_count":text.chars().count()})
}

fn inventory_text_fingerprint(text: &str) -> Value {
    let nfc = text.nfc().collect::<String>();
    let normalized = nfc
        .split(whitespace)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    json!({"algorithm":"sha256","normalization":"unicode-nfc-whitespace-collapse",
        "sha256":inventory_digest(normalized.as_bytes()),"character_count":normalized.chars().count()})
}

fn json_value_type(value: &FoundationJsonValue) -> &'static str {
    match value {
        FoundationJsonValue::Object(_) => "object",
        FoundationJsonValue::Array(_) => "array",
        FoundationJsonValue::String(_) => "string",
        FoundationJsonValue::Number(_) => "number",
        FoundationJsonValue::Bool(_) => "boolean",
        FoundationJsonValue::Null => "null",
    }
}

#[derive(Default, Clone, Copy)]
struct InventoryJsonCounts {
    object_count: usize,
    array_count: usize,
    string_count: usize,
    number_count: usize,
    boolean_count: usize,
    null_count: usize,
    object_key_count: usize,
}

impl InventoryJsonCounts {
    fn value(self) -> Value {
        json!({"object_count":self.object_count,"array_count":self.array_count,
            "string_count":self.string_count,"number_count":self.number_count,
            "boolean_count":self.boolean_count,"null_count":self.null_count,
            "object_key_count":self.object_key_count})
    }
}

fn count_json_values(root: &FoundationJsonValue) -> SourceCommandResult<InventoryJsonCounts> {
    let mut counts = InventoryJsonCounts::default();
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        match value {
            FoundationJsonValue::Object(entries) => {
                counts.object_count += 1;
                counts.object_key_count += entries.len();
                for (key, child) in entries {
                    key.as_str().ok_or(SourceCommandError::Unsupported(
                        "JSON strings with unpaired surrogates",
                    ))?;
                    pending.push(child);
                }
            }
            FoundationJsonValue::Array(items) => {
                counts.array_count += 1;
                pending.extend(items);
            }
            FoundationJsonValue::String(value) => {
                counts.string_count += 1;
                value.as_str().ok_or(SourceCommandError::Unsupported(
                    "JSON strings with unpaired surrogates",
                ))?;
            }
            FoundationJsonValue::Number(_) => counts.number_count += 1,
            FoundationJsonValue::Bool(_) => counts.boolean_count += 1,
            FoundationJsonValue::Null => counts.null_count += 1,
        }
    }
    Ok(counts)
}

fn json_inventory(
    raw: &[u8],
    file_id: &str,
    file_sha256: &str,
    media_type: &str,
) -> SourceCommandResult<Value> {
    if raw.len() > MAX_INVENTORY_JSON_BYTES {
        return Err(SourceCommandError::Unsupported(
            "bounded JSON inventory profile",
        ));
    }
    let document = parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: MAX_INVENTORY_JSON_BYTES,
            max_visits: MAX_INVENTORY_JSON_VISITS,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Invalid("JSON payload is not well formed"))?;
    let root = document.into_root();
    let members: Vec<(Option<&str>, &FoundationJsonValue)> = match &root {
        FoundationJsonValue::Object(entries) => entries
            .iter()
            .map(|(key, value)| {
                Ok((
                    Some(key.as_str().ok_or(SourceCommandError::Unsupported(
                        "JSON strings with unpaired surrogates",
                    ))?),
                    value,
                ))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
        FoundationJsonValue::Array(items) => items.iter().map(|item| (None, item)).collect(),
        _ => return Err(SourceCommandError::Invalid("JSON member profile root")),
    };
    let root_counts = count_json_values(&root)?;
    let mut resources = vec![
        json!({"resource_id":"json-root","resource_kind":"json_container",
        "structural_role":"container_metadata","locator":{"json_value_type":json_value_type(&root)}}),
    ];
    for (index, (key, item)) in members.iter().enumerate() {
        let member_counts = count_json_values(item)?;
        let mut resource = json!({"resource_id":format!("json-member-{:05}",index+1),
            "resource_kind":"json_member","structural_role":"member",
            "locator":{"json_member_index":index+1,"json_value_type":json_value_type(item),
                "parent_resource_id":"json-root"},"json_value_counts":member_counts.value()});
        if let Some(key) = key {
            resource["label_fingerprint"] =
                inventory_exact_fingerprint(key, "unicode-codepoints-preserved");
        }
        if let FoundationJsonValue::String(value) = item {
            let text = value.as_str().ok_or(SourceCommandError::Unsupported(
                "JSON strings with unpaired surrogates",
            ))?;
            resource["content_fingerprint"] =
                inventory_exact_fingerprint(text, "unicode-codepoints-preserved");
        }
        resources.push(resource);
    }
    Ok(
        json!({"file_id":file_id,"file_sha256":file_sha256,"media_type":media_type,
        "profile":"json_members_v1","summary":{"resource_count":resources.len(),
            "top_level_member_count":members.len(),"json_value_counts":root_counts.value()},
        "resources":resources}),
    )
}

#[derive(Debug)]
struct InventoryXmlNode {
    namespace: Option<String>,
    local_name: String,
    attributes: BTreeMap<String, String>,
    text_content: String,
    children: Vec<InventoryXmlNode>,
}

fn inventory_xml_tree(raw: &[u8]) -> SourceCommandResult<InventoryXmlNode> {
    let config = ParserConfig::new()
        .max_name_length(65536)
        .max_attributes(128)
        .max_attribute_length(65536)
        .max_data_length(16 * 1024 * 1024)
        .allow_multiple_root_elements(false)
        .ignore_end_of_stream(false)
        .replace_unknown_entity_references(false);
    let mut stack: Vec<InventoryXmlNode> = Vec::new();
    let mut root = None;
    let mut count = 0usize;
    for event in config.create_reader(raw) {
        count += 1;
        if count > 262_144 || stack.len() > 128 {
            return Err(SourceCommandError::Unsupported(
                "bounded XML inventory profile",
            ));
        }
        match event.map_err(|_| SourceCommandError::Invalid("XML payload is not well formed"))? {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                if stack.len() >= 128 {
                    return Err(SourceCommandError::Unsupported(
                        "bounded XML inventory profile",
                    ));
                }
                let attributes = attributes
                    .into_iter()
                    .filter_map(|attribute| {
                        attribute
                            .name
                            .namespace
                            .is_none()
                            .then_some((attribute.name.local_name, attribute.value))
                    })
                    .collect();
                stack.push(InventoryXmlNode {
                    namespace: name.namespace,
                    local_name: name.local_name,
                    attributes,
                    text_content: String::new(),
                    children: Vec::new(),
                });
            }
            XmlEvent::EndElement { name } => {
                let node = stack
                    .pop()
                    .ok_or(SourceCommandError::Invalid("XML payload is not balanced"))?;
                if node.local_name != name.local_name || node.namespace != name.namespace {
                    return Err(SourceCommandError::Invalid("XML payload is not balanced"));
                }
                if let Some(parent) = stack.last_mut() {
                    parent.text_content.push_str(&node.text_content);
                    parent.children.push(node);
                } else if root.replace(node).is_some() {
                    return Err(SourceCommandError::Invalid(
                        "XML payload has multiple roots",
                    ));
                }
            }
            XmlEvent::Characters(text) | XmlEvent::Whitespace(text) | XmlEvent::CData(text) => {
                if let Some(node) = stack.last_mut() {
                    node.text_content.push_str(&text);
                }
            }
            XmlEvent::Doctype { .. } => (),
            XmlEvent::ProcessingInstruction { .. } => (),
            XmlEvent::StartDocument { .. } | XmlEvent::EndDocument | XmlEvent::Comment(_) => (),
        }
    }
    if !stack.is_empty() {
        return Err(SourceCommandError::Invalid("XML payload is truncated"));
    }
    root.ok_or(SourceCommandError::Invalid("XML payload root missing"))
}

fn inventory_find_tei_text(node: &InventoryXmlNode) -> Option<&InventoryXmlNode> {
    for child in &node.children {
        if child.namespace.as_deref() == Some(TEI_NAMESPACE) && child.local_name == "text" {
            return Some(child);
        }
        if let Some(found) = inventory_find_tei_text(child) {
            return Some(found);
        }
    }
    None
}

fn inventory_element_path(parent: &str, child: &InventoryXmlNode, index: usize) -> String {
    format!("{parent}/{}[{index}]", child.local_name)
}

fn inventory_tei_walk(
    element: &InventoryXmlNode,
    path: &str,
    division_depth: usize,
    parent_division_id: Option<&str>,
    page_label: &mut Option<String>,
    resources: &mut Vec<Value>,
    page_break_count: &mut usize,
    division_count: &mut usize,
    max_division_depth: &mut usize,
) {
    let mut sibling_counts = BTreeMap::<&str, usize>::new();
    for child in &element.children {
        let index = sibling_counts.entry(child.local_name.as_str()).or_default();
        *index += 1;
        let child_path = inventory_element_path(path, child, *index);
        match child.local_name.as_str() {
            "pb" => {
                *page_break_count += 1;
                let page_n = child.attributes.get("n").filter(|value| !value.is_empty());
                let facs = child
                    .attributes
                    .get("facs")
                    .filter(|value| !value.is_empty());
                if let Some(label) = page_n.or(facs) {
                    *page_label = Some(label.clone());
                }
                let mut locator = json!({"tei_path":child_path});
                if let Some(label) = page_n {
                    locator["tei_page_label"] = json!(label);
                }
                if let Some(facs) = facs {
                    locator["tei_facs_ref"] = json!(facs);
                }
                if let Some(parent) = parent_division_id {
                    locator["parent_resource_id"] = json!(parent);
                }
                resources.push(json!({"resource_id":format!("tei-pb-{:04}",page_break_count),
                    "resource_kind":"tei_page_break","locator":locator,"structural_role":"page_break"}));
                inventory_tei_walk(
                    child,
                    &child_path,
                    division_depth,
                    parent_division_id,
                    page_label,
                    resources,
                    page_break_count,
                    division_count,
                    max_division_depth,
                );
            }
            "div" => {
                *division_count += 1;
                let resource_id = format!("tei-div-{:04}", division_count);
                let current_depth = division_depth + 1;
                *max_division_depth = (*max_division_depth).max(current_depth);
                let mut locator = json!({"tei_path":child_path,"tei_depth":current_depth});
                if let Some(label) = page_label.as_deref() {
                    locator["tei_page_label"] = json!(label);
                }
                if let Some(value) = child.attributes.get("n") {
                    if !value.is_empty() {
                        locator["tei_n"] = json!(value);
                    }
                }
                if let Some(value) = child.attributes.get("type") {
                    if !value.is_empty() {
                        locator["tei_type"] = json!(value);
                    }
                }
                if let Some(parent) = parent_division_id {
                    locator["parent_resource_id"] = json!(parent);
                }
                let mut resource = json!({"resource_id":resource_id.clone(),"resource_kind":"tei_division",
                    "locator":locator,"structural_role":if child.attributes.get("type").is_some_and(|v|v=="contents") {"contents"} else {"division"},
                    "content_fingerprint":inventory_text_fingerprint(&child.text_content)});
                if let Some(head) = child.children.iter().find(|candidate| {
                    candidate.namespace.as_deref() == Some(TEI_NAMESPACE)
                        && candidate.local_name == "head"
                }) {
                    resource["label_fingerprint"] = inventory_text_fingerprint(&head.text_content);
                }
                resources.push(resource);
                inventory_tei_walk(
                    child,
                    &child_path,
                    current_depth,
                    Some(&resource_id),
                    page_label,
                    resources,
                    page_break_count,
                    division_count,
                    max_division_depth,
                );
            }
            _ => inventory_tei_walk(
                child,
                &child_path,
                division_depth,
                parent_division_id,
                page_label,
                resources,
                page_break_count,
                division_count,
                max_division_depth,
            ),
        }
    }
}

fn tei_inventory(
    raw: &[u8],
    file_id: &str,
    file_sha256: &str,
    media_type: &str,
) -> SourceCommandResult<Value> {
    let root = inventory_xml_tree(raw)?;
    let text = inventory_find_tei_text(&root).ok_or(SourceCommandError::Invalid(
        "TEI payload has no text element",
    ))?;
    let mut resources = Vec::new();
    let mut page_break_count = 0;
    let mut division_count = 0;
    let mut max_division_depth = 0;
    inventory_tei_walk(
        text,
        "TEI/text[1]",
        0,
        None,
        &mut None,
        &mut resources,
        &mut page_break_count,
        &mut division_count,
        &mut max_division_depth,
    );
    if resources.is_empty() {
        return Err(SourceCommandError::Invalid(
            "TEI payload yielded no structural resources",
        ));
    }
    Ok(
        json!({"file_id":file_id,"file_sha256":file_sha256,"media_type":media_type,
        "profile":"tei_structure_v1","summary":{"resource_count":resources.len(),
            "page_break_count":page_break_count,"division_count":division_count,
            "max_division_depth":max_division_depth},"resources":resources}),
    )
}

fn osis_id_is_valid(identifier: &str, verse: bool) -> bool {
    let parts = identifier.split('.').collect::<Vec<_>>();
    if parts.len() != if verse { 3 } else { 2 } {
        return false;
    }
    let book = parts[0];
    let letters = if book.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        if book.len() < 2 || !matches!(book.as_bytes()[0], b'1'..=b'4') {
            return false;
        }
        &book[1..]
    } else {
        book
    };
    if letters.is_empty() || !letters.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return false;
    }
    if parts[1].is_empty() || !parts[1].bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    if !verse {
        return true;
    }
    let verse_part = parts[2];
    let digits = verse_part.trim_end_matches(|c: char| c.is_ascii_lowercase());
    let suffix = &verse_part[digits.len()..];
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && suffix.len() <= 1
        && suffix.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn inventory_xml_word_count(node: &InventoryXmlNode) -> usize {
    let mut count = 0;
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.namespace.as_deref() == Some(OSIS_NAMESPACE) && current.local_name == "w" {
            count += 1;
        }
        pending.extend(&current.children);
    }
    count
}

fn inventory_osis_walk(
    element: &InventoryXmlNode,
    chapter: Option<(String, String, usize)>,
    inside_verse: bool,
    resources: &mut Vec<Value>,
    chapter_count: &mut usize,
    verse_count: &mut usize,
    word_count: &mut usize,
    seen_ids: &mut BTreeSet<String>,
    chapter_counts: &mut BTreeMap<String, (usize, usize)>,
) -> SourceCommandResult<()> {
    if element.namespace.as_deref() == Some(OSIS_NAMESPACE)
        && matches!(element.local_name.as_str(), "chapter" | "verse")
    {
        let is_verse = element.local_name == "verse";
        let identifier = element
            .attributes
            .get("osisID")
            .map(String::as_str)
            .unwrap_or("");
        if !osis_id_is_valid(identifier, is_verse) {
            return Err(SourceCommandError::Invalid(
                "OSIS chapter or verse identifier",
            ));
        }
        if element.attributes.contains_key("sID") || element.attributes.contains_key("eID") {
            return Err(SourceCommandError::Unsupported("OSIS milestone profile"));
        }
        if !seen_ids.insert(identifier.to_owned()) {
            return Err(SourceCommandError::Invalid(
                "duplicate OSIS chapter or verse identifier",
            ));
        }
        if !is_verse {
            if chapter.is_some() || inside_verse {
                return Err(SourceCommandError::Invalid("nested OSIS chapter"));
            }
            *chapter_count += 1;
            let index = *chapter_count;
            let resource_id = format!("osis-chapter-{index:04}");
            let chapter_resource_index = resources.len();
            chapter_counts.insert(resource_id.clone(), (0, 0));
            let mut resource = json!({"resource_id":resource_id.clone(),"resource_kind":"osis_chapter",
                "structural_role":"chapter","locator":{"osis_id":identifier,
                    "chapter_index":index,"container_order":resources.len()+1},"verse_count":0,"word_count":0});
            resources.push(resource.clone());
            for child in &element.children {
                inventory_osis_walk(
                    child,
                    Some((identifier.to_owned(), resource_id.clone(), index)),
                    false,
                    resources,
                    chapter_count,
                    verse_count,
                    word_count,
                    seen_ids,
                    chapter_counts,
                )?;
            }
            let (chapter_verses, chapter_words) = chapter_counts
                .get(&resource_id)
                .copied()
                .ok_or(SourceCommandError::Invalid("OSIS chapter summary missing"))?;
            if chapter_verses == 0 {
                return Err(SourceCommandError::Invalid(
                    "OSIS chapter yielded no verses",
                ));
            }
            resource["verse_count"] = json!(chapter_verses);
            resource["word_count"] = json!(chapter_words);
            resources[chapter_resource_index] = resource;
            return Ok(());
        }
        let (chapter_id, chapter_resource_id, chapter_index) = chapter.as_ref().ok_or(
            SourceCommandError::Invalid("OSIS verse has no containing chapter"),
        )?;
        if inside_verse
            || identifier.rsplit_once('.').map(|(prefix, _)| prefix) != Some(chapter_id.as_str())
        {
            return Err(SourceCommandError::Invalid(
                "OSIS verse differs from its containing chapter",
            ));
        }
        *verse_count += 1;
        let words = inventory_xml_word_count(element);
        *word_count += words;
        let chapter_summary = chapter_counts
            .get_mut(chapter_resource_id)
            .ok_or(SourceCommandError::Invalid("OSIS chapter summary missing"))?;
        chapter_summary.0 += 1;
        chapter_summary.1 += words;
        resources.push(json!({"resource_id":format!("osis-verse-{:05}",verse_count),
            "resource_kind":"osis_verse","structural_role":"verse","locator":{
                "osis_id":identifier,"chapter_index":chapter_index,"verse_index":verse_count,
                "container_order":resources.len()+1,"parent_resource_id":chapter_resource_id},
            "word_count":words,"content_fingerprint":inventory_exact_fingerprint(&element.text_content,
                "xml-character-data-preserved")}));
        for child in &element.children {
            inventory_osis_walk(
                child,
                chapter.clone(),
                true,
                resources,
                chapter_count,
                verse_count,
                word_count,
                seen_ids,
                chapter_counts,
            )?;
        }
        return Ok(());
    }
    for child in &element.children {
        inventory_osis_walk(
            child,
            chapter.clone(),
            inside_verse,
            resources,
            chapter_count,
            verse_count,
            word_count,
            seen_ids,
            chapter_counts,
        )?;
    }
    Ok(())
}

fn osis_inventory(
    raw: &[u8],
    file_id: &str,
    file_sha256: &str,
    media_type: &str,
) -> SourceCommandResult<Value> {
    let root = inventory_xml_tree(raw)?;
    if root.namespace.as_deref() != Some(OSIS_NAMESPACE)
        || root.local_name != "osis"
        || root
            .children
            .iter()
            .filter(|child| {
                child.namespace.as_deref() == Some(OSIS_NAMESPACE) && child.local_name == "osisText"
            })
            .count()
            != 1
    {
        return Err(SourceCommandError::Invalid("OSIS profile root"));
    }
    let osis_text = root
        .children
        .iter()
        .find(|child| {
            child.namespace.as_deref() == Some(OSIS_NAMESPACE) && child.local_name == "osisText"
        })
        .ok_or(SourceCommandError::Invalid("OSIS profile root"))?;
    let mut resources = Vec::new();
    let mut chapter_count = 0;
    let mut verse_count = 0;
    let mut word_count = 0;
    let mut seen_ids = BTreeSet::new();
    let mut chapter_counts = BTreeMap::new();
    inventory_osis_walk(
        osis_text,
        None,
        false,
        &mut resources,
        &mut chapter_count,
        &mut verse_count,
        &mut word_count,
        &mut seen_ids,
        &mut chapter_counts,
    )?;
    if chapter_count == 0 || verse_count == 0 {
        return Err(SourceCommandError::Invalid(
            "OSIS payload has no contained chapter/verse",
        ));
    }
    Ok(
        json!({"file_id":file_id,"file_sha256":file_sha256,"media_type":media_type,
        "profile":"osis_structure_v1","summary":{"resource_count":resources.len(),
            "chapter_count":chapter_count,"verse_count":verse_count,"word_count":word_count},
        "resources":resources}),
    )
}

fn inventory_file(raw: &[u8], entry: &Value) -> SourceCommandResult<Value> {
    let file_id = inventory_json_text(entry, "file_id")?;
    let file_sha256 = inventory_json_text(entry, "sha256")?;
    let media_type = inventory_json_text(entry, "media_type")?;
    match media_type {
        "application/json" => json_inventory(raw, file_id, file_sha256, media_type),
        "application/osis+xml" => osis_inventory(raw, file_id, file_sha256, media_type),
        "application/tei+xml" => tei_inventory(raw, file_id, file_sha256, media_type),
        "application/xml" | "text/xml" => {
            let root = inventory_xml_tree(raw)?;
            if root.namespace.as_deref() == Some(OSIS_NAMESPACE) && root.local_name == "osis" {
                osis_inventory(raw, file_id, file_sha256, media_type)
            } else if inventory_find_tei_text(&root).is_some() {
                tei_inventory(raw, file_id, file_sha256, media_type)
            } else {
                Err(SourceCommandError::Unsupported(
                    "generic XML inventory profile",
                ))
            }
        }
        _ => Err(SourceCommandError::Unsupported(
            "maintained native registry inventory profile",
        )),
    }
}

fn inventory_payload_ref(
    item_manifest_ref: &str,
    relative_path: &str,
) -> SourceCommandResult<String> {
    inventory_safe_ref(relative_path)?;
    if !relative_path.starts_with("payload/") {
        return Err(SourceCommandError::Invalid("Item payload relative path"));
    }
    let item_root_ref = item_manifest_ref
        .strip_suffix("/item.manifest.json")
        .ok_or(SourceCommandError::Invalid("Item manifest reference"))?;
    let source_tail = item_root_ref
        .strip_prefix("ToS/source-witnesses/")
        .ok_or(SourceCommandError::Invalid("Item manifest source route"))?;
    inventory_safe_ref(source_tail)?;
    Ok(format!("{source_tail}/{relative_path}"))
}

fn inventory_event_ref(item_id: &str, event_date: &str) -> SourceCommandResult<String> {
    let suffix = item_id
        .strip_prefix("tos.item.")
        .ok_or(SourceCommandError::Invalid("Item identifier"))?;
    if suffix.is_empty()
        || !suffix.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
        || event_date.len() != 10
        || !event_date.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 4 | 7) {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
    {
        return Err(SourceCommandError::Invalid(
            "resource inventory event binding",
        ));
    }
    Ok(format!(
        "tos.event.resource-inventory.{suffix}.{event_date}"
    ))
}

fn prior_inventory_binding(
    repo_root: &Path,
    inventory_ref: &str,
    default_event_ref: String,
) -> SourceCommandResult<(String, u64, Option<String>)> {
    let path = crate::source_acquisition_batch::path_under(repo_root, inventory_ref)
        .map_err(|_| SourceCommandError::Invalid("resource inventory output reference"))?;
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((default_event_ref, 1, None))
        }
        Err(_) => Err(SourceCommandError::Unsupported(
            "prior resource inventory read",
        )),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            SourceCommandError::Unsupported("prior resource inventory file type"),
        ),
        Ok(_) => {
            let prior = inventory_read_json(&path)?;
            let event_ref = prior
                .get("provenance_event_ref")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or(default_event_ref);
            let version = prior
                .get("inventory_version")
                .and_then(Value::as_u64)
                .filter(|value| *value >= 1)
                .unwrap_or(1);
            let supersedes = prior
                .get("supersedes_inventory_ref")
                .and_then(Value::as_str)
                .map(str::to_owned);
            Ok((event_ref, version, supersedes))
        }
    }
}

/// Build the maintained registry XML/JSON inventory wrapper from exact Item
/// manifest bindings and explicitly selected source bytes. This observes and
/// returns metadata only; it never writes the inventory companion.
pub(crate) fn build_registry_inventory(
    repo_root: &Path,
    item_manifest_ref: &str,
    payload_source_root: &Path,
    event_date: &str,
) -> std::result::Result<Value, String> {
    let source_command_result = || -> SourceCommandResult<Value> {
        inventory_safe_ref(item_manifest_ref)?;
        let item_root_ref = item_manifest_ref
            .strip_suffix("/item.manifest.json")
            .ok_or(SourceCommandError::Invalid("Item manifest reference"))?;
        let source_tail = item_root_ref
            .strip_prefix("ToS/source-witnesses/")
            .ok_or(SourceCommandError::Invalid("Item manifest source route"))?;
        inventory_safe_ref(source_tail)?;
        let manifest_path =
            crate::source_acquisition_batch::path_under(repo_root, item_manifest_ref)
                .map_err(|_| SourceCommandError::Invalid("Item manifest reference"))?;
        let manifest = inventory_read_json(&manifest_path)?;
        let item_id = inventory_json_text(&manifest, "item_id")?;
        let files = inventory_json_field(&manifest, "payload_files")?
            .as_array()
            .ok_or(SourceCommandError::Invalid("Item manifest payload_files"))?;
        if files.is_empty() {
            return Err(SourceCommandError::Invalid(
                "Item manifest has no payload files",
            ));
        }
        let mut inventories = Vec::with_capacity(files.len());
        for entry in files {
            let relative_path = inventory_json_text(entry, "relative_path")?;
            let payload_ref = inventory_payload_ref(item_manifest_ref, relative_path)?;
            let payload_path =
                crate::source_acquisition_batch::path_under(payload_source_root, &payload_ref)
                    .map_err(|_| SourceCommandError::Invalid("Item payload source path"))?;
            let byte_size = inventory_json_field(entry, "byte_size")?
                .as_u64()
                .ok_or(SourceCommandError::Invalid("Item payload byte_size"))?;
            let sha256 = inventory_json_text(entry, "sha256")?;
            let raw = crate::source_acquisition_batch::read_bytes(
                &payload_path,
                None,
                false,
                false,
                byte_size,
            )
            .map_err(|_| SourceCommandError::Unsupported("Item payload fixity read"))?;
            if raw.len() as u64 != byte_size || inventory_digest(&raw) != sha256 {
                return Err(SourceCommandError::Invalid(
                    "Item payload fixity differs from manifest",
                ));
            }
            inventories.push(inventory_file(&raw, entry)?);
        }
        let inventory_ref = inventory_json_text(&manifest, "resource_inventory_ref")?;
        inventory_safe_ref(inventory_ref)?;
        if inventory_ref != format!("{item_root_ref}/resource-inventory.json") {
            return Err(SourceCommandError::Invalid("Item resource inventory path"));
        }
        let event_ref = inventory_event_ref(item_id, event_date)?;
        let (event_ref, inventory_version, supersedes_inventory_ref) =
            prior_inventory_binding(repo_root, inventory_ref, event_ref)?;
        Ok(
            json!({"$schema":INVENTORY_SCHEMA,"schema_version":"tos_source_resource_inventory_v1",
            "item_id":item_id,"generated_from_manifest_ref":item_manifest_ref,
            "inventory_authority":"mechanical_metadata_only","source_text_included":false,
            "files":inventories,"generator":{"name":"build_source_resource_inventories.py",
                "version":INVENTORY_GENERATOR_VERSION},"provenance_event_ref":event_ref,
            "inventory_version":inventory_version,"supersedes_inventory_ref":supersedes_inventory_ref,
            "authority_boundary":INVENTORY_AUTHORITY_BOUNDARY}),
        )
    };
    source_command_result().map_err(|error| format!("native source inventory failed: {error:?}"))
}

#[cfg(test)]
mod registry_inventory_tests {
    use super::*;
    use std::fs;

    fn try_build_fixture(
        media_type: &str,
        filename: &str,
        body: &[u8],
        manifest_sha256: Option<&str>,
    ) -> std::result::Result<Value, String> {
        let temporary = tempfile::tempdir().expect("temporary fixture directory");
        let repository = temporary.path().join("repository");
        let payload_root = temporary.path().join("payload-root");
        let item_ref = "ToS/source-witnesses/works/fixture/editions/test/items/registry-item/item.manifest.json";
        let payload_relative =
            format!("works/fixture/editions/test/items/registry-item/payload/{filename}");
        let manifest_path = repository.join(item_ref);
        let payload_path = payload_root.join(&payload_relative);
        fs::create_dir_all(manifest_path.parent().unwrap()).expect("manifest parents");
        fs::create_dir_all(payload_path.parent().unwrap()).expect("payload parents");
        fs::write(&payload_path, body).expect("payload bytes");
        let digest = inventory_digest(body);
        let manifest = json!({
            "item_id":"tos.item.fixture.registry-item",
            "payload_files":[{
                "file_id":format!("tos.file.sha256.{digest}"),
                "relative_path":format!("payload/{filename}"),
                "original_basename":filename,
                "media_type":media_type,
                "byte_size":body.len(),
                "sha256":manifest_sha256.unwrap_or(&digest)
            }],
            "resource_inventory_ref":"ToS/source-witnesses/works/fixture/editions/test/items/registry-item/resource-inventory.json"
        });
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).expect("manifest bytes");
        build_registry_inventory(&repository, item_ref, &payload_root, "2026-10-01")
    }

    fn build_fixture(media_type: &str, filename: &str, body: &[u8]) -> Value {
        try_build_fixture(media_type, filename, body, None).expect("registry inventory")
    }

    #[test]
    fn registry_json_profile_keeps_member_order_counts_and_only_fingerprints() {
        let raw = br#"{"oe":{"a":[1,true,null]},"text":"e\u0301 secret"}"#;
        let inventory = build_fixture("application/json", "source.json", raw);
        let file = &inventory["files"][0];
        assert_eq!(file["profile"], "json_members_v1");
        assert_eq!(file["summary"]["resource_count"], 3);
        assert_eq!(file["summary"]["top_level_member_count"], 2);
        assert_eq!(file["summary"]["json_value_counts"]["object_count"], 2);
        assert_eq!(file["summary"]["json_value_counts"]["array_count"], 1);
        assert_eq!(file["summary"]["json_value_counts"]["object_key_count"], 3);
        assert_eq!(file["resources"][1]["locator"]["json_member_index"], 1);
        assert_eq!(
            file["resources"][1]["label_fingerprint"]["sha256"],
            inventory_digest("oe".as_bytes())
        );
        assert_eq!(
            inventory["generated_from_manifest_ref"],
            "ToS/source-witnesses/works/fixture/editions/test/items/registry-item/item.manifest.json"
        );
        assert_eq!(
            inventory["provenance_event_ref"],
            "tos.event.resource-inventory.fixture.registry-item.2026-10-01"
        );
        assert!(
            !serde_json::to_string(&inventory)
                .unwrap()
                .contains("secret")
        );
    }

    #[test]
    fn registry_tei_profile_retains_page_and_division_structure() {
        let raw = format!(
            "<TEI xmlns=\"{TEI_NAMESPACE}\"><text><body><pb n=\"3\"/><div type=\"contents\" n=\"start\"><head>Intro</head><p>private text</p><div type=\"section\" n=\"1\">Inner</div></div></body></text></TEI>"
        );
        let inventory = build_fixture("application/tei+xml", "source.xml", raw.as_bytes());
        let file = &inventory["files"][0];
        assert_eq!(file["profile"], "tei_structure_v1");
        assert_eq!(file["summary"]["page_break_count"], 1);
        assert_eq!(file["summary"]["division_count"], 2);
        assert_eq!(file["resources"][1]["structural_role"], "contents");
        assert_eq!(file["resources"][1]["locator"]["tei_page_label"], "3");
        assert_eq!(
            file["resources"][2]["locator"]["parent_resource_id"],
            "tei-div-0001"
        );
        assert!(
            !serde_json::to_string(&inventory)
                .unwrap()
                .contains("private text")
        );
    }

    #[test]
    fn registry_osis_profile_keeps_document_order_and_contained_verse_counts() {
        let raw = format!(
            "<osis xmlns=\"{OSIS_NAMESPACE}\"><osisText><div type=\"book\" osisID=\"Prov\"><chapter osisID=\"Prov.1\"><verse osisID=\"Prov.1.2\"><w>private</w><w>words</w></verse><verse osisID=\"Prov.1.1\"><w>another</w></verse></chapter><chapter osisID=\"Prov.2\"><verse osisID=\"Prov.2.1\"><w>last</w></verse></chapter></div></osisText></osis>"
        );
        let inventory = build_fixture("application/osis+xml", "source.xml", raw.as_bytes());
        let file = &inventory["files"][0];
        assert_eq!(file["profile"], "osis_structure_v1");
        assert_eq!(file["summary"]["chapter_count"], 2);
        assert_eq!(file["summary"]["verse_count"], 3);
        assert_eq!(file["summary"]["word_count"], 4);
        assert_eq!(file["resources"][1]["locator"]["osis_id"], "Prov.1.2");
        assert_eq!(file["resources"][0]["verse_count"], 2);
        assert!(
            !serde_json::to_string(&inventory)
                .unwrap()
                .contains("private")
        );
    }

    #[test]
    fn registry_profiles_reject_duplicate_json_keys_and_payload_fixity_drift() {
        assert!(
            try_build_fixture("application/json", "source.json", br#"{"x":1,"x":2}"#, None)
                .is_err()
        );
        assert!(
            try_build_fixture(
                "application/json",
                "source.json",
                br#"{"x":1}"#,
                Some(&"0".repeat(64))
            )
            .unwrap_err()
            .contains("fixity")
        );
    }
}
