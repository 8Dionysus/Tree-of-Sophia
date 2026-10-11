//! Literal, non-overlapping reader occurrences over exact UTF-16 display text.
//! Search meaning lives here; the browser host only marshals the request and
//! consumes the returned offsets.

use tos_foundation::{
    FoundationErrorCode, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json, unicode_simple_casefold_scalar16_v1,
};

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_VISITS: usize = 300_000;
const MAX_QUERY_UTF16_UNITS: usize = 160;
const MAX_MATCHES: usize = 500;

fn limits(max_bytes: usize) -> JsonLimits {
    JsonLimits::new(max_bytes, 8, MAX_VISITS, 32).expect("static reader JSON limits are valid")
}

fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value.object_get(name)
}

fn text(value: &JsonValue) -> Option<&JsonString> {
    match value {
        JsonValue::String(text) => Some(text),
        _ => None,
    }
}

fn is_ecmascript_trim_whitespace(point: u32) -> bool {
    matches!(
        point,
        0x0009..=0x000d
            | 0x0020
            | 0x00a0
            | 0x1680
            | 0x2000..=0x200a
            | 0x2028..=0x2029
            | 0x202f
            | 0x205f
            | 0x3000
            | 0xfeff
    )
}

fn scalar_at(units: &[u16], index: usize) -> (u32, usize) {
    let first = units[index];
    if (0xd800..=0xdbff).contains(&first)
        && let Some(second) = units.get(index + 1).copied()
        && (0xdc00..=0xdfff).contains(&second)
    {
        (
            0x10000 + (((first as u32 - 0xd800) << 10) | (second as u32 - 0xdc00)),
            2,
        )
    } else {
        (first as u32, 1)
    }
}

fn limit(value: Option<&JsonValue>) -> Option<usize> {
    match value {
        None => Some(MAX_MATCHES),
        Some(JsonValue::Number(number)) => {
            let value = number.lexeme.parse::<f64>().ok()?;
            (value.is_finite()
                && value.fract() == 0.0
                && (1.0..=MAX_MATCHES as f64).contains(&value))
            .then_some(value as usize)
        }
        _ => None,
    }
}

fn output(matches: Vec<(usize, usize, usize)>, truncated: bool) -> Result<Vec<u8>, &'static str> {
    let matches = matches
        .into_iter()
        .map(|(paragraph, index, length)| {
            JsonValue::Object(vec![
                (JsonString::from_utf8("paragraph"), integer(paragraph)),
                (JsonString::from_utf8("index"), integer(index)),
                (JsonString::from_utf8("length"), integer(length)),
            ])
        })
        .collect();
    let value = JsonValue::Object(vec![
        (JsonString::from_utf8("matches"), JsonValue::Array(matches)),
        (
            JsonString::from_utf8("truncated"),
            JsonValue::Bool(truncated),
        ),
    ]);
    emit_value_preserved_json(&value, limits(MAX_OUTPUT_BYTES)).map_err(|error| {
        if error.code == FoundationErrorCode::BudgetExceeded {
            "limit"
        } else {
            "invalid-input"
        }
    })
}

fn integer(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

/// Bounded JSON adapter for the reader's established `findInParagraphs` API.
/// The request is `{paragraphs, query, limit?}` and the response is
/// `{matches:[{paragraph,index,length}],truncated}`. Offsets and lengths are
/// UTF-16 code units, matching JavaScript string indexing; no normalization is
/// applied. Errors retain the host API's `invalid-input` and `limit` names.
pub fn reader_occurrences_v1(raw: &[u8]) -> Result<Vec<u8>, &'static str> {
    let document =
        parse_json(raw, JsonMode::RequestLastWins, limits(MAX_INPUT_BYTES)).map_err(|error| {
            if error.code == FoundationErrorCode::BudgetExceeded {
                "limit"
            } else {
                "invalid-input"
            }
        })?;
    let root = document.root();
    let paragraphs = field(root, "paragraphs")
        .and_then(JsonValue::as_array)
        .ok_or("invalid-input")?;
    if paragraphs.iter().any(|value| text(value).is_none()) {
        return Err("invalid-input");
    }
    let query = field(root, "query").and_then(text).ok_or("invalid-input")?;
    let max_matches = limit(field(root, "limit")).ok_or("invalid-input")?;
    let query_units = query.units();
    if query_units.len() > MAX_QUERY_UTF16_UNITS {
        return Err("limit");
    }

    let mut folded_query = Vec::with_capacity(query_units.len());
    let mut query_index = 0;
    let mut empty_after_trim = true;
    while query_index < query_units.len() {
        let (point, width) = scalar_at(query_units, query_index);
        if !is_ecmascript_trim_whitespace(point) {
            empty_after_trim = false;
        }
        folded_query.push(unicode_simple_casefold_scalar16_v1(point));
        query_index += width;
    }
    if empty_after_trim {
        return output(Vec::new(), false);
    }

    let mut prefix = vec![0usize; folded_query.len()];
    let mut prefix_len = 0;
    for index in 1..folded_query.len() {
        while prefix_len > 0 && folded_query[index] != folded_query[prefix_len] {
            prefix_len = prefix[prefix_len - 1];
        }
        if folded_query[index] == folded_query[prefix_len] {
            prefix_len += 1;
        }
        prefix[index] = prefix_len;
    }

    let mut found = Vec::with_capacity(max_matches);
    let mut truncated = false;
    'paragraphs: for (paragraph_index, value) in paragraphs.iter().enumerate() {
        let units = text(value)
            .expect("paragraph strings were validated")
            .units();
        let mut ring = vec![0usize; folded_query.len()];
        let mut scalar_index = 0usize;
        let mut text_index = 0usize;
        let mut matched = 0usize;
        while text_index < units.len() {
            let start = text_index;
            let (point, width) = scalar_at(units, text_index);
            let ring_slot = scalar_index % ring.len();
            ring[ring_slot] = start;
            let folded = unicode_simple_casefold_scalar16_v1(point);
            while matched > 0 && folded_query[matched] != folded {
                matched = prefix[matched - 1];
            }
            if folded_query[matched] == folded {
                matched += 1;
            }
            text_index += width;
            scalar_index += 1;
            if matched == folded_query.len() {
                if found.len() == max_matches {
                    truncated = true;
                    break 'paragraphs;
                }
                let first_scalar = scalar_index - folded_query.len();
                let occurrence_start = ring[first_scalar % ring.len()];
                found.push((
                    paragraph_index,
                    occurrence_start,
                    text_index - occurrence_start,
                ));
                // Global regex matches do not overlap; start the next search
                // after the matched text instead of preserving a KMP suffix.
                matched = 0;
            }
        }
    }
    output(found, truncated)
}
