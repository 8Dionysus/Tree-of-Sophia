//! Software-owned prompt templates; none accepts source meaning or rights.
use crate::common::{json_string, json_string_len};
use crate::{AccessError, AccessErrorCode};
use tos_foundation::JsonValue;

const CORPUS_DESCRIPTION: &str = "Prompt route for reviewing ToS corpus graph context.";
const PHILOSOPHY_DESCRIPTION: &str =
    "Prompt route for reviewing ToS philosophy graph projection context.";
const WORD_DESCRIPTION: &str =
    "Prompt route for source-first morphology, semantics, etymology, and English rendering.";
pub(crate) fn list() -> Vec<u8> {
    let mut out = b"{\"prompts\":[".to_vec();
    for (i, (name, description, arguments)) in [
        (
            "tos-corpus-review",
            CORPUS_DESCRIPTION,
            &[("view_id", false), ("query", false)][..],
        ),
        (
            "tos-philosophy-graph-review",
            PHILOSOPHY_DESCRIPTION,
            &[("view_id", false), ("query", false)][..],
        ),
        (
            "tos-zarathustra-word-analysis",
            WORD_DESCRIPTION,
            &[("query", true), ("language", false), ("rank", false)][..],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if i != 0 {
            out.push(b',');
        }
        out.extend_from_slice(b"{\"name\":");
        out.extend(json_string(name));
        out.extend_from_slice(b",\"description\":");
        out.extend(json_string(description));
        out.extend_from_slice(b",\"arguments\":[");
        for (i, (argument, required)) in arguments.iter().enumerate() {
            if i != 0 {
                out.push(b',');
            }
            out.extend_from_slice(b"{\"name\":");
            out.extend(json_string(argument));
            out.extend_from_slice(if *required {
                b",\"required\":true}"
            } else {
                b",\"required\":false}"
            });
        }
        out.extend_from_slice(b"]}");
    }
    out.extend_from_slice(b"]}");
    out
}
fn invalid(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::InvalidRequest, message)
}
fn string<'a>(
    arguments: Option<&'a JsonValue>,
    key: &str,
    default: Option<&'a str>,
) -> Result<&'a str, AccessError> {
    match arguments.and_then(|a| a.object_get(key)) {
        None => default.ok_or_else(|| invalid("required prompt argument absent")),
        Some(value) => value
            .as_str()
            .ok_or_else(|| invalid("prompt arguments must be strings")),
    }
}
fn repr_units(value: &[u16]) -> String {
    let quote = if value.contains(&39) && !value.contains(&34) {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for decoded in char::decode_utf16(value.iter().copied()) {
        let ch = match decoded {
            Ok(ch) => ch,
            Err(error) => {
                out.push_str(&format!("\\u{:04x}", error.unpaired_surrogate()));
                continue;
            }
        };
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch == quote => {
                out.push('\\');
                out.push(ch);
            }
            ch if !tos_foundation::python_printable_unicode16_v1(ch) => {
                let point = ch as u32;
                if point <= 0xff {
                    out.push_str(&format!("\\x{point:02x}"));
                } else if point <= 0xffff {
                    out.push_str(&format!("\\u{point:04x}"));
                } else {
                    out.push_str(&format!("\\U{point:08x}"));
                }
            }
            ch => out.push(ch),
        }
    }
    out.push(quote);
    out
}
fn repr_argument(
    arguments: Option<&JsonValue>,
    key: &str,
    default: Option<&str>,
) -> Result<String, AccessError> {
    match arguments.and_then(|a| a.object_get(key)) {
        Some(JsonValue::String(value)) => Ok(repr_units(value.units())),
        None => default
            .map(|v| repr_units(&v.encode_utf16().collect::<Vec<_>>()))
            .ok_or_else(|| invalid("required prompt argument absent")),
        _ => Err(invalid("prompt arguments must be strings")),
    }
}
// MCP prompt arguments are strings. Match the maintained Pydantic integral
// string coercion without narrowing rank to a machine-sized integer.
pub(crate) fn rank(value: &str) -> Result<String, AccessError> {
    let value = value.trim();
    let (negative, value) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next();
    if parts.next().is_some()
        || fraction.is_some_and(|v| v.is_empty() || !v.bytes().all(|b| b == b'0'))
    {
        return Err(invalid("prompt rank must be integral"));
    }
    if whole.is_empty()
        || whole.starts_with('_')
        || whole.ends_with('_')
        || whole.contains("__")
        || whole.bytes().any(|b| !b.is_ascii_digit() && b != b'_')
    {
        return Err(invalid("prompt rank must be integral"));
    }
    let digits: String = whole.chars().filter(|ch| *ch != '_').collect();
    if digits.len() > 4300 {
        return Err(invalid("prompt rank exceeds integer digit budget"));
    }
    let digits = digits.trim_start_matches('0');
    Ok(if digits.is_empty() {
        "0".into()
    } else if negative {
        format!("-{digits}")
    } else {
        digits.to_owned()
    })
}
pub(crate) fn get(params: Option<&JsonValue>, cap: usize) -> Result<Vec<u8>, AccessError> {
    let name = params
        .and_then(|p| p.object_get("name"))
        .and_then(JsonValue::as_str)
        .ok_or_else(|| invalid("prompt name required"))?;
    let arguments = params
        .and_then(|p| p.object_get("arguments"))
        .filter(|value| !matches!(value, JsonValue::Null));
    let allowed: &[&str] = match name {
        "tos-corpus-review" | "tos-philosophy-graph-review" => &["view_id", "query"],
        "tos-zarathustra-word-analysis" => &["query", "language", "rank"],
        _ => return Err(invalid("unknown ToS prompt")),
    };
    if let Some(arguments) = arguments {
        if arguments.as_object().is_none_or(|a| {
            a.iter()
                .any(|(key, _)| !key.as_str().is_some_and(|key| allowed.contains(&key)))
        }) {
            return Err(invalid("unknown prompt argument"));
        }
    }
    let (description, text) = match name {
        "tos-corpus-review" => (
            CORPUS_DESCRIPTION,
            format!(
                "Use tos_corpus_status(), then tos_corpus_packet(query={}, view_id={}). Treat Tree-of-Sophia source_refs returned by the packet as authority; treat native MCP and standalone runtime as read-only access surfaces.",
                repr_argument(arguments, "query", Some(""))?,
                repr_argument(arguments, "view_id", Some("corpus-topology"))?,
            ),
        ),
        "tos-philosophy-graph-review" => {
            let view = repr_argument(arguments, "view_id", Some("chronology"))?;
            let query = repr_argument(arguments, "query", Some(""))?;
            (
                PHILOSOPHY_DESCRIPTION,
                format!(
                    "Use tos_philosophy_graph_status(), tos_philosophy_graph_layers(), tos_philosophy_graph_review_packet(view_id={view}), then tos_philosophy_graph_packet(query={query}, view_id={view}). Treat ToS source_ref values as meaning authority; treat native MCP, UI, and optional integrations as projection/access surfaces only."
                ),
            )
        }
        _ => (
            WORD_DESCRIPTION,
            format!(
                "Call tos_zarathustra_prepare_word_analysis(query={}, language={}, rank={}). Analyze every required stage in the returned task. Use point citations for etymology, keep German as source authority, Russian as a historical comparator, and English as an unreviewed candidate. Do not infer contextual meaning from etymology alone.",
                repr_argument(arguments, "query", None)?,
                repr_argument(arguments, "language", Some("ru"))?,
                rank(string(arguments, "rank", Some("1"))?)?
            ),
        ),
    };
    if text.len() > cap {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "prompt text exceeds byte budget",
        ));
    }
    let serialized = json_string_len(&text)
        .and_then(|n| json_string_len(description).and_then(|d| n.checked_add(d)))
        .and_then(|n| n.checked_add(80));
    if !serialized.is_some_and(|n| n <= cap) {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "prompt response exceeds byte budget",
        ));
    }
    let mut out = b"{\"description\":".to_vec();
    out.extend(json_string(description));
    out.extend_from_slice(
        b",\"messages\":[{\"role\":\"user\",\"content\":{\"type\":\"text\",\"text\":",
    );
    out.extend(json_string(&text));
    out.extend_from_slice(b"}}]}");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcp_maintained_prompt_repr_integer_coercion_and_output_cap() {
        assert_eq!(repr_units(&[0xd800, 10, 39]), "\"\\ud800\\n'\"");
        assert_eq!(repr_units(&[0xd83d, 0xde00]), "'😀'");
        assert_eq!(repr_units(&[0xa0]), "'\\xa0'");
        for (value, expected) in [
            ("+002.00", "2"),
            ("1_000", "1000"),
            ("-000", "0"),
            (" -02 ", "-2"),
        ] {
            assert_eq!(rank(value).unwrap(), expected);
        }
        for value in ["1.5", "1e0", "_1", "1__0", "١"] {
            assert!(rank(value).is_err());
        }
        let doc = tos_foundation::parse_json(
            br#"{"name":"tos-corpus-review"}"#,
            tos_foundation::JsonMode::RequestLastWins,
            tos_foundation::JsonLimits::default(),
        )
        .unwrap();
        assert!(get(Some(doc.root()), 64).is_err());
        assert!(get(Some(doc.root()), 4096).is_ok());
    }
}
