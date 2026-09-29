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
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;
use unicode_normalization::{UnicodeNormalization, is_nfc, is_nfd};
use xml::reader::{ParserConfig, XmlEvent};

include!("source_item_html_entities.rs");

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
