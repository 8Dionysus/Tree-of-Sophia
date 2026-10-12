//! Shared source-value matching used by the maintained Corpus/Philosophy
//! readers and bounded platform adapters. Search examines JSON values only:
//! object keys, number spellings and serialized punctuation are not content.

use crate::search_v2::{SearchV2Error, SearchV2ErrorCode};
use tos_foundation::{JsonValue, python_lower_unicode16_v1, python_strip_unicode16_v1};

const CORPUS_COLLECTIONS: &[&str] = &["nodes", "resources", "manifests", "branches", "graph_views"];
const PHILOSOPHY_COLLECTIONS: &[&str] = &[
    "views",
    "nodes",
    "edges",
    "clusters",
    "review_packets",
    "graph_layers",
];

fn budget_error() -> SearchV2Error {
    SearchV2Error::new(
        SearchV2ErrorCode::BudgetExceeded,
        "source-value search budget exceeded",
    )
}

fn corrupt_error() -> SearchV2Error {
    SearchV2Error::new(
        SearchV2ErrorCode::CorruptSelectedCarrier,
        "source-value search candidate is invalid",
    )
}

/// Normalize a source-value query using the same Python lower/strip profile
/// used by the maintained QRY readers. The callback charges its own work meter.
pub fn normalize_query(
    query: &str,
    charge: &mut impl FnMut(usize) -> Result<(), SearchV2Error>,
) -> Result<String, SearchV2Error> {
    let chars = query.chars().count();
    charge(chars)?;
    if query.is_empty() {
        return Ok(String::new());
    }
    let lower = python_lower_unicode16_v1(
        query,
        chars,
        chars.saturating_mul(3),
        query.len().saturating_mul(3),
    )
    .map_err(|_| budget_error())?;
    python_strip_unicode16_v1(&lower, lower.chars().count())
        .map(str::to_owned)
        .map_err(|_| budget_error())
}

/// Match only string values recursively, without searching object keys or a
/// JSON serialization. Its work callback mirrors the readers' node and scalar
/// visits, and permits native cancellation/deadline probes to remain active.
pub fn contains_values(
    value: &JsonValue,
    needle: &str,
    charge: &mut impl FnMut(usize) -> Result<(), SearchV2Error>,
) -> Result<bool, SearchV2Error> {
    charge(1)?;
    match value {
        JsonValue::String(value) => {
            let value = value.as_str().ok_or_else(corrupt_error)?;
            if value.is_empty() {
                return Ok(needle.is_empty());
            }
            let chars = value.chars().count();
            charge(chars)?;
            let lower = python_lower_unicode16_v1(
                value,
                chars,
                chars.saturating_mul(3),
                value.len().saturating_mul(3),
            )
            .map_err(|_| budget_error())?;
            Ok(lower.contains(needle))
        }
        JsonValue::Array(values) => {
            for value in values {
                if contains_values(value, needle, charge)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        JsonValue::Object(values) => {
            for (_, value) in values {
                if contains_values(value, needle, charge)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// The exact collection order shared by each selected public reader.
pub fn collections(mode: &str) -> Option<&'static [&'static str]> {
    match mode {
        "corpus" => Some(CORPUS_COLLECTIONS),
        "philosophy" => Some(PHILOSOPHY_COLLECTIONS),
        _ => None,
    }
}

pub fn collection_rank(mode: &str, collection: &str) -> Option<u8> {
    collections(mode)?
        .iter()
        .position(|candidate| *candidate == collection)
        .and_then(|rank| u8::try_from(rank).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::{JsonLimits, JsonMode, parse_json};

    fn value(raw: &str) -> JsonValue {
        parse_json(
            raw.as_bytes(),
            JsonMode::PublishedStrict,
            JsonLimits::new(1024, 8, 128, 64).unwrap(),
        )
        .unwrap()
        .into_root()
    }

    #[test]
    fn searches_values_and_uses_python_query_normalization() {
        let fixture = value(include_str!(
            "../tests/fixtures/source-value-search-parity.json"
        ));
        let item = value(
            fixture
                .object_get("candidate_json")
                .unwrap()
                .as_str()
                .unwrap(),
        );
        let mut charged = 0usize;
        let mut charge = |steps| {
            charged += steps;
            Ok(())
        };
        let query = normalize_query(
            fixture.object_get("query").unwrap().as_str().unwrap(),
            &mut charge,
        )
        .unwrap();
        assert_eq!(
            query,
            fixture
                .object_get("expected_query")
                .unwrap()
                .as_str()
                .unwrap()
        );
        assert!(contains_values(&item, &query, &mut charge).unwrap());
        assert!(
            !contains_values(
                &item,
                fixture.object_get("key_query").unwrap().as_str().unwrap(),
                &mut charge
            )
            .unwrap()
        );
        assert!(charged > 0);
    }

    #[test]
    fn collection_order_is_owner_defined() {
        assert_eq!(collection_rank("corpus", "graph_views"), Some(4));
        assert_eq!(collection_rank("philosophy", "graph_layers"), Some(5));
        assert_eq!(collection_rank("philosophy", "missing"), None);
    }
}
