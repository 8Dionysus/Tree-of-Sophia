//! Bounded exact diagnostic views shared by witness comparison producers.
use crate::{research_execution::ResearchExecution, source_text_foundation::ensure};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;
type Result<T> = std::result::Result<T, String>;
pub(crate) fn alpha(c: char) -> bool {
    static LETTER: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^\p{L}$").unwrap());
    let mut bytes = [0u8; 4];
    LETTER.is_match(c.encode_utf8(&mut bytes))
}
pub(crate) fn space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
pub(crate) fn alpha_tokens(text: &str) -> Vec<String> {
    let normalized = text.nfc().collect::<String>();
    normalized
        .split(|c| !alpha(c))
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}
pub(crate) fn source_aware_text(lines: &[String]) -> String {
    let raw = lines
        .iter()
        .map(|l| l.trim_matches(space))
        .collect::<Vec<_>>()
        .join("\n");
    let chars = raw.chars().collect::<Vec<_>>();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '-'
            && i > 0
            && i + 2 < chars.len()
            && chars[i + 1] == '\n'
            && alpha(chars[i - 1])
            && alpha(chars[i + 2])
        {
            i += 2;
            continue;
        }
        out.push(if c == '\n' { ' ' } else { c });
        i += 1;
    }
    out.split(space)
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Opcode {
    pub tag: &'static str,
    pub i1: usize,
    pub i2: usize,
    pub j1: usize,
    pub j2: usize,
}
/// SequenceMatcher with no junk and autojunk disabled: longest contiguous block,
/// earliest left then right tie break, recursively matched gaps, adjacent merge.
pub(crate) fn opcodes<T: Ord>(ctx: &ResearchExecution, a: &[T], b: &[T]) -> Result<Vec<Opcode>> {
    ensure(
        a.len() <= 20000 && b.len() <= 20000,
        "comparison token bound",
    )?;
    let mut index = BTreeMap::<&T, Vec<usize>>::new();
    for (j, t) in b.iter().enumerate() {
        index.entry(t).or_default().push(j)
    }
    let mut queue = vec![(0, a.len(), 0, b.len())];
    let mut matches = vec![];
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (mut bi, mut bj, mut size) = (alo, blo, 0usize);
        let mut previous = BTreeMap::<usize, usize>::new();
        for (i, t) in a.iter().enumerate().take(ahi).skip(alo) {
            ctx.tick(1)?;
            let mut next = BTreeMap::new();
            if let Some(js) = index.get(t) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    ctx.tick(1)?;
                    let k = if j > 0 {
                        previous.get(&(j - 1)).copied().unwrap_or(0) + 1
                    } else {
                        1
                    };
                    next.insert(j, k);
                    if k > size {
                        bi = i + 1 - k;
                        bj = j + 1 - k;
                        size = k
                    }
                }
            }
            previous = next;
        }
        if size > 0 {
            matches.push((bi, bj, size));
            if alo < bi && blo < bj {
                queue.push((alo, bi, blo, bj))
            }
            if bi + size < ahi && bj + size < bhi {
                queue.push((bi + size, ahi, bj + size, bhi))
            }
        }
    }
    matches.sort_unstable();
    let mut blocks = Vec::<(usize, usize, usize)>::new();
    for m in matches {
        if let Some(last) = blocks.last_mut() {
            if last.0 + last.2 == m.0 && last.1 + last.2 == m.1 {
                last.2 += m.2;
                continue;
            }
        }
        blocks.push(m)
    }
    blocks.push((a.len(), b.len(), 0));
    let (mut i, mut j) = (0, 0);
    let mut out = vec![];
    for (ai, bj, size) in blocks {
        let tag = if i < ai && j < bj {
            Some("replace")
        } else if i < ai {
            Some("delete")
        } else if j < bj {
            Some("insert")
        } else {
            None
        };
        if let Some(tag) = tag {
            out.push(Opcode {
                tag,
                i1: i,
                i2: ai,
                j1: j,
                j2: bj,
            })
        }
        if size > 0 {
            out.push(Opcode {
                tag: "equal",
                i1: ai,
                i2: ai + size,
                j1: bj,
                j2: bj + size,
            })
        }
        i = ai + size;
        j = bj + size;
    }
    Ok(out)
}
pub(crate) fn token_diff(ctx: &ResearchExecution, a: &[String], b: &[String]) -> Result<Value> {
    let (mut equal, mut missing, mut extra, mut replace, mut delete, mut insert) =
        (0, 0, 0, 0, 0, 0);
    for op in opcodes(ctx, a, b)? {
        match op.tag {
            "equal" => equal += op.i2 - op.i1,
            "replace" => {
                replace += 1;
                missing += op.i2 - op.i1;
                extra += op.j2 - op.j1
            }
            "delete" => {
                delete += 1;
                missing += op.i2 - op.i1
            }
            "insert" => {
                insert += 1;
                extra += op.j2 - op.j1
            }
            _ => unreachable!(),
        }
    }
    Ok(
        json!({"reference_token_count":a.len(),"candidate_token_count":b.len(),"equal_token_count":equal,"reference_missing_token_count":missing,"candidate_extra_token_count":extra,"replacement_block_count":replace,"deletion_block_count":delete,"insertion_block_count":insert,"exact_alpha_token_sequence":a==b}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_sequence_matcher_oracle() {
        let t = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        let cases: Value =
            serde_json::from_str(include_str!("research_text_comparison_cases.json")).unwrap();
        for row in cases.as_array().unwrap() {
            let left = row["a"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect::<Vec<_>>();
            let right = row["b"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect::<Vec<_>>();
            let actual = opcodes(&ctx, &left, &right)
                .unwrap()
                .into_iter()
                .map(|o| json!([o.tag, o.i1, o.i2, o.j1, o.j2]))
                .collect::<Vec<_>>();
            assert_eq!(json!(actual), row["opcodes"]);
        }
    }
    #[test]
    fn source_aware_hyphen_and_nfc() {
        assert_eq!(
            source_aware_text(&[" Wort- ".into(), " ende —".into(), " weiter".into()]),
            "Wortende — weiter"
        );
        assert_eq!(alpha_tokens("A\u{308} ₂12ß—x\u{301}"), vec!["Ä", "ß", "x"]);
    }
}
