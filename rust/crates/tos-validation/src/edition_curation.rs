//! Structural checks for edition-curation records before selecting source bytes.
use std::{cmp::Ordering, collections::BTreeSet, str::FromStr};

use num_bigint::BigInt;

use serde_json::Value;

use crate::{SchemaBackendProbe, SchemaProbeError, published_value};

pub const SCHEMA_URI: &str =
    "https://tree-of-sophia.local/ToS/contracts/edition-curation.v1.schema.json";

#[derive(Debug, PartialEq, Eq)]
pub enum EditionCurationError {
    Schema(SchemaProbeError),
    InvalidSchema,
    Duplicate(&'static str),
    InvalidRange(&'static str),
}

/// Validate exact curation JSON using the caller's selected schema resources.
pub fn validate(raw: &[u8], schema: &SchemaBackendProbe) -> Result<(), EditionCurationError> {
    let value = published_value(raw, SchemaBackendProbe::MAX_INSTANCE_BYTES)
        .map_err(EditionCurationError::Schema)?;
    if !schema
        .is_valid_value(SCHEMA_URI, &value)
        .map_err(EditionCurationError::Schema)?
    {
        return Err(EditionCurationError::InvalidSchema);
    }
    check_rules(&value)
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum MarkerPart {
    Text(String),
    Number(bool, String, BigInt),
}

fn check_rules(value: &Value) -> Result<(), EditionCurationError> {
    match value {
        Value::Object(object) => {
            if object.get("stage").and_then(Value::as_str) == Some("numbered-label-correspondence")
            {
                let count = |key: &str| object.get(key).and_then(Value::as_u64);
                let size = |key: &str| {
                    object
                        .get(key)
                        .and_then(Value::as_array)
                        .and_then(|a| u64::try_from(a.len()).ok())
                };
                let shared =
                    count("expected_shared_count").ok_or(EditionCurationError::InvalidSchema)?;
                for (total, exclusive) in [
                    ("expected_source_count", "source_only_keys"),
                    ("expected_target_count", "target_only_keys"),
                ] {
                    if !matches!((size(exclusive).and_then(|n| shared.checked_add(n)), count(total)), (Some(expected), Some(actual)) if expected == actual)
                    {
                        return Err(EditionCurationError::InvalidRange(total));
                    }
                }
            }
            for (lower, upper) in [
                ("start_page", "end_page"),
                ("represented_start_page", "represented_end_page"),
                ("page_min", "page_max"),
                ("x_min", "x_max"),
                ("y_min", "y_max"),
                ("min_words", "max_words"),
            ] {
                if let (Some(a), Some(b)) = (object.get(lower), object.get(upper)) {
                    if !matches!(
                        compare_numbers(a, b),
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        return Err(EditionCurationError::InvalidRange(lower));
                    }
                }
            }
            if let Some(Value::Array(order)) = object.get("basis_rule_order") {
                if order.last().and_then(Value::as_str) != Some("default") {
                    return Err(EditionCurationError::InvalidRange("basis_rule_order"));
                }
                for rule in order {
                    let field = match rule.as_str() {
                        Some("repeated-number-review") => Some("repeated_number_review_keys"),
                        Some("gap-review") => Some("gap_review_keys"),
                        Some("page-override") => Some("page_overrides"),
                        Some("ocr-disambiguation") => Some("ocr_disambiguation_keys"),
                        _ => None,
                    };
                    if field.is_some_and(|field| !object.contains_key(field)) {
                        return Err(EditionCurationError::InvalidSchema);
                    }
                }
            }
            for (array, field) in [
                ("page_overrides", "key"),
                ("locators", "role"),
                ("work_mappings", "work_ref"),
                ("works", "work_ref"),
                ("series", "series_key"),
            ] {
                if let Some(Value::Array(items)) = object.get(array) {
                    let mut seen = BTreeSet::new();
                    for item in items {
                        if let Some(key) = item.get(field).and_then(Value::as_str) {
                            if !seen.insert(key) {
                                return Err(EditionCurationError::Duplicate(field));
                            }
                        }
                    }
                }
            }
            if object.get("page_order").and_then(Value::as_str) == Some("nondecreasing") {
                let overrides = object["page_overrides"]
                    .as_array()
                    .ok_or(EditionCurationError::InvalidSchema)?;
                for pair in overrides.windows(2) {
                    if !matches!(
                        compare_numbers(&pair[0]["page"], &pair[1]["page"]),
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        return Err(EditionCurationError::InvalidRange("page_overrides"));
                    }
                }
            }
            if let Some(Value::Array(items)) = object.get("marker_returns") {
                let mut seen = BTreeSet::new();
                for item in items {
                    if let Some(key) = item.get("key").and_then(Value::as_array) {
                        let key = key
                            .iter()
                            .map(|part| match part {
                                Value::String(text) => Some(MarkerPart::Text(text.clone())),
                                Value::Number(_) => {
                                    number_parts(part).map(|(sign, digits, magnitude)| {
                                        MarkerPart::Number(sign, digits, magnitude)
                                    })
                                }
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or(EditionCurationError::InvalidSchema)?;
                        if !seen.insert(key) {
                            return Err(EditionCurationError::Duplicate("marker key"));
                        }
                    }
                }
            }
            if let Some(Value::Array(bounds)) = object.get("line_bbox") {
                for (lower, upper) in [(0, 2), (1, 3)] {
                    if !matches!(
                        compare_numbers(&bounds[lower], &bounds[upper]),
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        return Err(EditionCurationError::InvalidRange("line_bbox"));
                    }
                }
            }
            for child in object.values() {
                check_rules(child)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                check_rules(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn number_parts(value: &Value) -> Option<(bool, String, BigInt)> {
    let text = value.as_number()?.to_string();
    let (negative, text) = match text.strip_prefix('-') {
        Some(text) => (true, text),
        None => (false, text.as_str()),
    };
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], BigInt::from_str(&text[index + 1..]).ok()?),
        None => (text, BigInt::from(0)),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some((false, String::new(), BigInt::from(0)));
    }
    let magnitude = exponent + BigInt::from(digits.len()) - BigInt::from(fraction.len());
    Some((negative, digits.trim_end_matches('0').to_owned(), magnitude))
}

// Compare JSON decimals without rounding coordinates or allocating by exponent.
fn compare_numbers(a: &Value, b: &Value) -> Option<Ordering> {
    let (an, a, am) = number_parts(a)?;
    let (bn, b, bm) = number_parts(b)?;
    if a.is_empty() || b.is_empty() {
        return Some(match (a.is_empty(), b.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => {
                if bn {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (false, true) => {
                if an {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            _ => unreachable!(),
        });
    }
    if an != bn {
        return Some(if an {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let magnitude_order = am.cmp(&bm).then_with(|| {
        let len = a.len().max(b.len());
        a.bytes()
            .chain(std::iter::repeat(b'0'))
            .take(len)
            .cmp(b.bytes().chain(std::iter::repeat(b'0')).take(len))
    });
    Some(if an {
        magnitude_order.reverse()
    } else {
        magnitude_order
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_range_comparison_keeps_exact_precision() {
        fn number(raw: &str) -> Value {
            serde_json::from_str(raw).unwrap()
        }
        for (a, b, expected) in [
            ("9007199254740993", "9007199254740992", Ordering::Greater),
            ("1.00000000000000001", "1.0", Ordering::Greater),
            ("-1.01", "-1.001", Ordering::Less),
            ("1e1000", "9e999", Ordering::Greater),
            ("-0.0", "0", Ordering::Equal),
        ] {
            assert_eq!(compare_numbers(&number(a), &number(b)), Some(expected));
        }
    }
}
