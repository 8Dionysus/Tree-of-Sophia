//! The maintained XHTML character-data extraction law for one selected EPUB
//! member. The command owner authenticates the member and the separate read and
//! derivation grants before calling this pure, bounded parser.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use std::io::BufReader;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{JsonValue, python_strip_unicode16_v1};
use xml::reader::{ParserConfig, XmlEvent};

const XHTML: &str = "http://www.w3.org/1999/xhtml";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const MAX_MEMBER_BYTES: usize = 8_388_608;
const MAX_TEXT_BYTES: usize = 8_388_608;
const MAX_MARKUP_TOKEN_BYTES: usize = 16_384;
const MAX_ATTRIBUTES: usize = 128;
const MAX_ELEMENTS: usize = 65_536;
const MAX_DEPTH: usize = 64;

// The exact maintained policy is a pure extraction rule, separate from owner
// grants. Compare canonical bytes so JsonValue insertion order is immaterial.
const POLICY: &str = r#"{
  "schema_version":"tos_xhtml_text_extraction_policy_v1",
  "method":"tos.xhtml.character-data.v1",
  "input_media_type":"application/xhtml+xml",
  "encoding":"UTF-8-strict",
  "xml_version":"1.0",
  "namespace":"http://www.w3.org/1999/xhtml",
  "carriage_returns":"reject-input-profile-limitation",
  "entity_and_character_references":"reject-all-ampersands",
  "doctype_and_processing_instructions":"reject-except-xml-declaration",
  "comments":"omit-bounded-markup-preserve-surrounding-character-data",
  "cdata":"literal-character-data-under-member-and-text-budgets",
  "markup_token_max_bytes":16384,
  "attributes_per_start_tag_max":128,
  "selector_schemes":["tos.xhtml.element-ordinal.v1","tos.xhtml.element-id.v1"],
  "ordinal_basis":"one-based-document-order-exact-namespace-and-local-name",
  "selected_root_elements":["a","abbr","b","bdi","bdo","blockquote","cite","code","dfn","em","h1","h2","h3","h4","h5","h6","i","kbd","li","mark","p","pre","q","s","samp","small","span","strong","sub","sup","time","u","var"],
  "inline_elements":["a","abbr","b","bdi","bdo","cite","code","dfn","em","i","kbd","mark","q","s","samp","small","span","strong","sub","sup","time","u","var"],
  "line_break_element":"br-to-one-LF",
  "text_nodes":"concatenate-in-document-order-without-selected-root-tail",
  "whitespace":"preserve-no-trim-collapse-or-inserted-block-separators",
  "unicode_normalization":"none",
  "attributes":"XML-1.0-parsed-identifiers-only-not-rendered",
  "unknown_selected_markup":"reject",
  "quality_assessment":"not-performed",
  "uncertainty":"none-recorded-is-not-reviewed-absence"
}"#;

#[derive(Clone, Copy)]
pub(crate) enum XhtmlSelector<'a> {
    Ordinal { local: &'a str, ordinal: usize },
    Id(&'a str),
}

pub(crate) fn validate_extraction_profile<'a>(
    selector: &'a JsonValue,
    policy: &JsonValue,
) -> SourceCommandResult<XhtmlSelector<'a>> {
    if cmd::canonical(policy)? != cmd::canonical(&cmd::parse(POLICY.as_bytes())?)? {
        return Err(SourceCommandError::Unsupported("XHTML extraction policy"));
    }
    cmd::exact_keys(selector, &["type", "scheme", "value"])?;
    let value = cmd::text(selector, "value")?;
    if cmd::text(selector, "type")? != "structural" || !(1..=256).contains(&value.chars().count()) {
        return Err(SourceCommandError::Invalid("XHTML selector profile"));
    }
    match cmd::text(selector, "scheme")? {
        "tos.xhtml.element-ordinal.v1" => {
            let (local, number) = value
                .split_once(':')
                .ok_or(SourceCommandError::Invalid("XHTML element ordinal"))?;
            if !(1..=32).contains(&local.len())
                || !local.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
                || !local
                    .bytes()
                    .skip(1)
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                || !root_element(local)
                || !(1..=5).contains(&number.len())
                || number.starts_with('0')
                || !number.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(SourceCommandError::Invalid("XHTML element ordinal"));
            }
            let ordinal = number
                .parse::<usize>()
                .map_err(|_| SourceCommandError::Invalid("XHTML element ordinal"))?;
            if ordinal > MAX_ELEMENTS {
                return Err(SourceCommandError::Invalid("XHTML element ordinal"));
            }
            Ok(XhtmlSelector::Ordinal { local, ordinal })
        }
        "tos.xhtml.element-id.v1" => {
            if python_strip_unicode16_v1(value, 256)
                .map_err(|_| SourceCommandError::Invalid("XHTML element identifier"))?
                != value
                || value
                    .chars()
                    .any(|ch| (ch as u32) < 33 || matches!(ch, '<' | '>' | '"' | '\'' | '&'))
            {
                return Err(SourceCommandError::Invalid("XHTML element identifier"));
            }
            Ok(XhtmlSelector::Id(value))
        }
        _ => Err(SourceCommandError::Unsupported("XHTML selector scheme")),
    }
}

fn root_element(local: &str) -> bool {
    inline_element(local)
        || matches!(
            local,
            "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "pre" | "blockquote"
        )
}

fn inline_element(local: &str) -> bool {
    matches!(
        local,
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "cite"
            | "code"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "mark"
            | "q"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
    )
}

// Only a lexical resource preflight. The XML parser below owns syntax,
// namespaces, attributes and whole-document validity. In particular, this
// scan cannot accept malformed closing tags or a second document root.
fn markup_preflight(
    raw: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut offset = 0usize;
    while let Some(found) = raw[offset..].iter().position(|byte| *byte == b'<') {
        active(deadline, cancelled)?;
        let start = offset + found;
        if raw[start..].starts_with(b"<!--") {
            let search_end = raw.len().min(start.saturating_add(MAX_MARKUP_TOKEN_BYTES));
            let closing = raw[start + 4..search_end]
                .windows(3)
                .position(|part| part == b"-->")
                .ok_or(SourceCommandError::Unsupported(
                    "XHTML comment token budget",
                ))?;
            offset = start + 4 + closing + 3;
            continue;
        }
        if raw[start..].starts_with(b"<![CDATA[") {
            let mut cursor = start + 9;
            loop {
                if cursor % 65_536 == 0 {
                    active(deadline, cancelled)?;
                }
                if cursor + 3 > raw.len() {
                    return Err(SourceCommandError::Invalid("XHTML CDATA terminator"));
                }
                if &raw[cursor..cursor + 3] == b"]]>" {
                    offset = cursor + 3;
                    break;
                }
                cursor += 1;
            }
            continue;
        }
        if raw[start..].starts_with(b"<!") {
            return Err(SourceCommandError::Unsupported("XHTML declaration profile"));
        }
        let start_tag = raw
            .get(start + 1)
            .is_some_and(|byte| *byte != b'/' && *byte != b'?');
        let mut quote = None;
        let mut attributes = 0usize;
        let mut end = None;
        for (relative, byte) in raw[start + 1..].iter().enumerate() {
            if relative + 2 > MAX_MARKUP_TOKEN_BYTES {
                return Err(SourceCommandError::Unsupported("XHTML markup token budget"));
            }
            match (quote, *byte) {
                (Some(mark), value) if value == mark => quote = None,
                (Some(_), _) => (),
                (None, b'\'' | b'"') => quote = Some(*byte),
                (None, b'>') => {
                    end = Some(start + relative + 2);
                    break;
                }
                (None, b'<') => return Err(SourceCommandError::Invalid("XHTML markup token")),
                (None, b'=') if start_tag => {
                    attributes += 1;
                    if attributes > MAX_ATTRIBUTES {
                        return Err(SourceCommandError::Unsupported("XHTML attribute budget"));
                    }
                }
                _ => (),
            }
        }
        offset = end.ok_or(SourceCommandError::Invalid("XHTML markup terminator"))?;
    }
    active(deadline, cancelled)
}

fn append_text(output: &mut String, value: &str) -> SourceCommandResult<()> {
    if output
        .len()
        .checked_add(value.len())
        .is_none_or(|size| size > MAX_TEXT_BYTES)
    {
        return Err(SourceCommandError::Unsupported(
            "XHTML extracted text byte budget",
        ));
    }
    output.push_str(value);
    Ok(())
}

pub(crate) fn extract_xhtml_text(
    raw: &[u8],
    selector: XhtmlSelector<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    if raw.is_empty() || raw.len() > MAX_MEMBER_BYTES || raw.contains(&b'\r') || raw.contains(&b'&')
    {
        return Err(SourceCommandError::Unsupported(
            "XHTML exact member profile",
        ));
    }
    std::str::from_utf8(raw).map_err(|_| SourceCommandError::Invalid("XHTML strict UTF-8"))?;
    markup_preflight(raw, deadline, cancelled)?;
    let config = ParserConfig::new()
        .trim_whitespace(false)
        .whitespace_to_characters(true)
        .cdata_to_characters(false)
        .ignore_comments(true)
        .coalesce_characters(false)
        .allow_multiple_root_elements(false)
        .ignore_end_of_stream(false)
        .ignore_invalid_encoding_declarations(false)
        .replace_unknown_entity_references(false)
        .max_name_length(MAX_MARKUP_TOKEN_BYTES)
        .max_attributes(MAX_ATTRIBUTES)
        .max_attribute_length(MAX_MARKUP_TOKEN_BYTES)
        .max_data_length(MAX_TEXT_BYTES);
    let mut reader = config.create_reader(BufReader::new(raw));
    let mut depth = 0usize;
    let mut elements = 0usize;
    let mut matched = 0usize;
    let mut selected_depth = None;
    let mut selected_once = false;
    let mut br_depth = None;
    let mut output = String::new();
    let mut events = 0usize;
    loop {
        if events % 1024 == 0 {
            active(deadline, cancelled)?;
        }
        events += 1;
        let event = reader
            .next()
            .map_err(|_| SourceCommandError::Invalid("XHTML XML 1.0 document"))?;
        match event {
            XmlEvent::StartDocument {
                version, encoding, ..
            } => {
                if version != xml::common::XmlVersion::Version10
                    || !encoding.eq_ignore_ascii_case("utf-8")
                {
                    return Err(SourceCommandError::Unsupported(
                        "XHTML XML 1.0 UTF-8 declaration",
                    ));
                }
            }
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                depth += 1;
                elements += 1;
                if depth > MAX_DEPTH || elements > MAX_ELEMENTS || attributes.len() > MAX_ATTRIBUTES
                {
                    return Err(SourceCommandError::Unsupported("XHTML tree budget"));
                }
                let xhtml = name.namespace.as_deref() == Some(XHTML);
                if depth == 1 && !(xhtml && name.local_name == "html") {
                    return Err(SourceCommandError::Invalid("XHTML document namespace"));
                }
                if br_depth.is_some() {
                    return Err(SourceCommandError::Invalid(
                        "XHTML break contains child markup",
                    ));
                }
                let candidate = match selector {
                    XhtmlSelector::Ordinal { local, ordinal } => {
                        if xhtml && name.local_name == local {
                            matched += 1;
                            matched == ordinal
                        } else {
                            false
                        }
                    }
                    XhtmlSelector::Id(id) => {
                        let found = attributes.iter().any(|attribute| {
                            attribute.value == id
                                && attribute.name.local_name == "id"
                                && (attribute.name.namespace.is_none()
                                    || attribute.name.namespace.as_deref() == Some(XML))
                        });
                        if found {
                            matched += 1;
                            if matched > 1 {
                                return Err(SourceCommandError::Invalid(
                                    "XHTML selector ambiguous",
                                ));
                            }
                        }
                        found
                    }
                };
                if selected_depth.is_none() && candidate && !selected_once {
                    if !xhtml || !root_element(&name.local_name) {
                        return Err(SourceCommandError::Invalid("XHTML selected root element"));
                    }
                    selected_once = true;
                    selected_depth = Some(depth);
                } else if selected_depth.is_some() {
                    if !xhtml || !(inline_element(&name.local_name) || name.local_name == "br") {
                        return Err(SourceCommandError::Invalid("XHTML selected descendant"));
                    }
                    if name.local_name == "br" {
                        append_text(&mut output, "\n")?;
                        br_depth = Some(depth);
                    }
                }
            }
            XmlEvent::EndElement { .. } => {
                if br_depth == Some(depth) {
                    br_depth = None;
                }
                if selected_depth == Some(depth) {
                    selected_depth = None;
                }
                depth = depth
                    .checked_sub(1)
                    .ok_or(SourceCommandError::Invalid("XHTML depth"))?;
            }
            XmlEvent::Characters(value) | XmlEvent::Whitespace(value) | XmlEvent::CData(value) => {
                if br_depth.is_some() && !value.is_empty() {
                    return Err(SourceCommandError::Invalid(
                        "XHTML break contains character data",
                    ));
                }
                if selected_depth.is_some() {
                    append_text(&mut output, &value)?;
                }
            }
            XmlEvent::ProcessingInstruction { .. } | XmlEvent::Doctype { .. } => {
                return Err(SourceCommandError::Unsupported("XHTML declaration profile"));
            }
            XmlEvent::EndDocument => break,
            XmlEvent::Comment(_) => (),
            _ => return Err(SourceCommandError::Unsupported("XHTML event profile")),
        }
    }
    active(deadline, cancelled)?;
    if depth != 0 || !selected_once || output.is_empty() {
        return Err(SourceCommandError::Invalid("XHTML selected text absent"));
    }
    Ok(output)
}

#[cfg(test)]
#[path = "source_text_layer_xml_tests.rs"]
mod tests;
