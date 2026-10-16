//! Shared HTML5 character-reference decoding for inventories and research.
include!("research_html_entities.rs");
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_inventory_entity_rules() {
        assert_eq!(
            unescape("A &notit; &#128; &#0; &#x1f600; &unknown;"),
            "A ¬it; € � 😀 &unknown;"
        );
    }
}
