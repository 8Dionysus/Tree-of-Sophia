//! Bounded HTML events and the existing HTML5 character-reference semantics.
use crate::{research_execution::ResearchExecution, source_text_foundation::ensure};
use std::collections::BTreeMap;
include!("research_html_entities.rs");
#[derive(Debug)]
pub(crate) enum HtmlEvent {
    Start {
        tag: String,
        attrs: BTreeMap<String, String>,
        empty: bool,
    },
    End(String),
    Text(String),
}
fn attrs(raw: &str) -> Result<BTreeMap<String, String>, String> {
    let mut offset = 0usize;
    let mut out = BTreeMap::new();
    while offset < raw.len() {
        let rest = raw[offset..].trim_start();
        offset = raw.len() - rest.len();
        if rest.is_empty() || rest == "/" {
            break;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '=' | '/' | '>'))
            .unwrap_or(rest.len());
        ensure(end > 0, "HTML attribute name")?;
        let key = rest[..end].to_ascii_lowercase();
        offset += end;
        let rest = raw[offset..].trim_start();
        offset = raw.len() - rest.len();
        let value = if let Some(rest) = rest.strip_prefix('=') {
            let rest = rest.trim_start();
            offset = raw.len() - rest.len();
            if let Some(q) = rest.chars().next().filter(|c| matches!(c, '\'' | '"')) {
                let end = rest[1..].find(q).ok_or("HTML attribute quote")? + 1;
                let value = unescape(&rest[1..end]);
                offset += end + 1;
                value
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                let value = unescape(&rest[..end]);
                offset += end;
                value
            }
        } else {
            String::new()
        };
        out.insert(key, value);
        ensure(out.len() <= 1024, "HTML attributes bound")?;
    }
    Ok(out)
}
pub(crate) fn events(ctx: &ResearchExecution, text: &str) -> Result<Vec<HtmlEvent>, String> {
    ensure(text.len() <= 4 * 1024 * 1024, "HTML input bound")?;
    let mut out = vec![];
    let mut offset = 0usize;
    let mut raw_tag: Option<String> = None;
    while offset < text.len() {
        ctx.tick(1)?;
        if let Some(tag) = raw_tag.take() {
            let lower = text[offset..].to_ascii_lowercase();
            let marker = format!("</{tag}");
            if let Some(at) = lower.find(&marker) {
                out.push(HtmlEvent::Text(text[offset..offset + at].into()));
                offset += at
            } else {
                out.push(HtmlEvent::Text(text[offset..].into()));
                break;
            }
        }
        let Some(at) = text[offset..].find('<') else {
            out.push(HtmlEvent::Text(unescape(&text[offset..])));
            break;
        };
        let start = offset + at;
        if start > offset {
            out.push(HtmlEvent::Text(unescape(&text[offset..start])))
        }
        let tail = &text[start..];
        if tail.starts_with("<!--") {
            offset = start + tail.find("-->").ok_or("HTML comment close")? + 3;
            continue;
        }
        if tail.starts_with("<![CDATA[") {
            offset = start + tail.find("]]>").ok_or("HTML CDATA close")? + 3;
            continue;
        }
        let mut quote = None;
        let mut end = None;
        for (i, c) in tail.char_indices().skip(1) {
            ensure(i <= 65536, "HTML tag byte bound")?;
            match (quote, c) {
                (Some(q), v) if q == v => quote = None,
                (Some(_), _) => (),
                (None, '\'' | '"') => quote = Some(c),
                (None, '>') => {
                    end = Some(i);
                    break;
                }
                _ => (),
            }
        }
        let end = end.ok_or("HTML tag close")?;
        let body = &tail[1..end];
        offset = start + end + 1;
        if body.starts_with(['!', '?']) {
            continue;
        }
        let closing = body.starts_with('/');
        let inner = body.strip_prefix('/').unwrap_or(body).trim_start();
        let len = inner
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-'))
            .map(char::len_utf8)
            .sum::<usize>();
        if len == 0 {
            out.push(HtmlEvent::Text("<".into()));
            offset = start + 1;
            continue;
        }
        let tag = inner[..len].to_ascii_lowercase();
        if closing {
            out.push(HtmlEvent::End(tag))
        } else {
            let empty = body.trim_end().ends_with('/');
            if matches!(tag.as_str(), "script" | "style") && !empty {
                raw_tag = Some(tag.clone())
            }
            let attribute_text = if empty {
                inner[len..].trim_end().strip_suffix('/').unwrap()
            } else {
                &inner[len..]
            };
            out.push(HtmlEvent::Start {
                tag,
                attrs: attrs(attribute_text)?,
                empty,
            })
        }
        ensure(out.len() <= 200000, "HTML event bound")?;
    }
    Ok(out)
}
pub(crate) fn anchored_text(
    ctx: &ResearchExecution,
    text: &str,
    anchor: &str,
) -> Result<String, String> {
    let (mut seen, mut capture, mut finished, mut depth) = (false, false, false, 0usize);
    let mut parts = vec![];
    for event in events(ctx, text)? {
        match event {
            HtmlEvent::Start { tag, attrs, empty } => {
                if tag == "a" && attrs.get("name").is_some_and(|n| n == anchor) {
                    seen = true
                }
                if empty || finished {
                    continue;
                }
                if seen
                    && !capture
                    && tag == "div"
                    && attrs
                        .get("class")
                        .is_some_and(|s| s.split_whitespace().any(|c| c == "txt_block"))
                {
                    capture = true;
                    depth = 1;
                    continue;
                }
                if capture {
                    depth += 1;
                    ensure(depth <= 256, "HTML capture depth")?
                }
            }
            HtmlEvent::End(_) => {
                if capture {
                    depth -= 1;
                    if depth == 0 {
                        capture = false;
                        finished = true
                    }
                }
            }
            HtmlEvent::Text(data) => {
                if capture && !data.trim().is_empty() {
                    parts.push(data)
                }
            }
        }
    }
    ensure(
        seen && finished && !parts.is_empty(),
        "critical HTML selector unresolved",
    )?;
    Ok(parts.join(" "))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anchored_named_entities_and_nested_markup() {
        let t = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        assert_eq!(anchored_text(&ctx,r#"<script>if (a<b) x='&copy;';</script><a name='X'/><div class='other txt_block'><span>Ä&nbsp;&amp;</span> B&#223;</div><div>excluded</div>"#,"X").unwrap(),"Ä\u{a0}&  Bß");
        assert!(anchored_text(&ctx, "<a name='X'><div>no selected class</div>", "X").is_err());
    }
    #[test]
    fn existing_inventory_entity_rules() {
        assert_eq!(
            unescape("A &notit; &#128; &#0; &#x1f600; &unknown;"),
            "A ¬it; € � 😀 &unknown;"
        );
    }
}
pub fn unescape(input: &str) -> String {
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
