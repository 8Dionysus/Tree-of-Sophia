//! Bounded value-semantics bridge for the Cloudflare Worker adapters.
//!
//! Transport, D1 custody and source-reference preservation stay in the Worker;
//! this module owns only the Python-compatible operations it delegates.

use tos_foundation::{
    FoundationError, JsonLimits, JsonMode, JsonString, JsonValue, emit_value_preserved_json,
    parse_json, python_equals, python_float_text, python_lower, python_lower_json_string,
    python_member, python_repr, python_search_rank_values, python_searchable_text, python_string,
    python_truthy,
};

const REQUEST_SCHEMA: &str = "tos_worker_python_value_request_v1";
const RESPONSE_SCHEMA: &str = "tos_worker_python_value_response_v1";
const MAX_REQUEST_BYTES: usize = 10 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 1_048_576;
const MAX_DOCUMENT_DEPTH: usize = 64;
const MAX_DOCUMENT_VISITS: usize = 300_000;
const MAX_INTEGER_DIGITS: usize = 4_300;
const MAX_LOWER_UNITS: usize = 4 * 1024 * 1024;

fn limits(bytes: usize, depth: usize, visits: usize) -> JsonLimits {
    JsonLimits::new(bytes, depth, visits, MAX_INTEGER_DIGITS)
        .expect("constant bridge JSON limits are valid")
}

fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}

fn envelope(value: Option<JsonValue>, error: Option<(&str, &str)>) -> Vec<u8> {
    let mut entries = vec![
        ("schema_version", string(RESPONSE_SCHEMA)),
        ("ok", JsonValue::Bool(error.is_none())),
    ];
    if let Some(value) = value {
        entries.push(("value", value));
    }
    if let Some((code, detail)) = error {
        entries.push(("error", string(code)));
        entries.push(("message", string(detail)));
    }
    let value = object(entries);
    emit_value_preserved_json(&value, limits(MAX_RESPONSE_BYTES, 64, 600_000))
        .unwrap_or_else(|_| b"{\"schema_version\":\"tos_worker_python_value_response_v1\",\"ok\":false,\"error\":\"budget_exceeded\"}".to_vec())
}

fn field<'a>(request: &'a JsonValue, key: &str) -> Result<&'a JsonValue, FoundationError> {
    request.object_get(key).ok_or_else(|| {
        FoundationError::new(
            tos_foundation::FoundationErrorCode::UnsupportedFormat,
            "missing Python value request field",
        )
    })
}

fn text_field<'a>(request: &'a JsonValue, key: &str) -> Result<&'a str, FoundationError> {
    field(request, key)?.as_str().ok_or_else(|| {
        FoundationError::new(
            tos_foundation::FoundationErrorCode::UnsupportedFormat,
            "invalid Python value request text field",
        )
    })
}

fn positive_field(
    request: &JsonValue,
    key: &str,
    default: usize,
) -> Result<usize, FoundationError> {
    match request.object_get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| {
                let maximum = match key {
                    "max_utf16_units" => 4 * 1024 * 1024,
                    "max_visits" | "max_values" => MAX_DOCUMENT_VISITS,
                    _ => usize::MAX,
                };
                *value > 0 && *value <= maximum
            })
            .ok_or_else(|| {
                FoundationError::new(
                    tos_foundation::FoundationErrorCode::BudgetExceeded,
                    "invalid Python value request budget",
                )
            }),
    }
}

fn parse_document(raw: &str) -> Result<JsonValue, FoundationError> {
    parse_json(
        raw.as_bytes(),
        JsonMode::PublishedStrict,
        limits(MAX_DOCUMENT_BYTES, MAX_DOCUMENT_DEPTH, MAX_DOCUMENT_VISITS),
    )
    .map(|document| document.into_root())
}

fn run(request: &JsonValue) -> Result<JsonValue, FoundationError> {
    if text_field(request, "schema_version")? != REQUEST_SCHEMA {
        return Err(FoundationError::new(
            tos_foundation::FoundationErrorCode::UnsupportedFormat,
            "unsupported Python value request schema",
        ));
    }
    let operation = text_field(request, "operation")?;
    let left = parse_document(text_field(request, "left_json")?)?;
    let max_utf16 = positive_field(request, "max_utf16_units", 1_048_576)?;
    let max_visits = positive_field(request, "max_visits", MAX_DOCUMENT_VISITS)?;
    match operation {
        "truthy" => Ok(JsonValue::Bool(python_truthy(&left))),
        "equals" => {
            let right = parse_document(text_field(request, "right_json")?)?;
            python_equals(&left, &right, max_visits).map(JsonValue::Bool)
        }
        "member" => {
            let right = parse_document(text_field(request, "right_json")?)?;
            python_member(&left, &right, max_visits).map(JsonValue::Bool)
        }
        "str" | "repr" => {
            if operation == "str" {
                if let JsonValue::String(value) = &left {
                    return Ok(JsonValue::String(value.clone()));
                }
                python_string(&left, max_utf16).map(|value| string(&value))
            } else {
                python_repr(&left, max_utf16).map(|value| string(&value))
            }
        }
        "sort_key" => {
            if !python_truthy(&left) {
                return Ok(string(""));
            }
            let value = match &left {
                JsonValue::String(value) => python_lower_json_string(value, max_utf16)?,
                _ => JsonString::from_utf8(&python_lower(
                    &python_string(&left, max_utf16)?,
                    max_utf16,
                )?),
            };
            Ok(JsonValue::String(value))
        }
        "lower" => {
            let JsonValue::String(value) = left else {
                return Err(FoundationError::new(
                    tos_foundation::FoundationErrorCode::UnsupportedFormat,
                    "lower operation requires a string",
                ));
            };
            python_lower_json_string(&value, max_utf16).map(JsonValue::String)
        }
        "float" => {
            let JsonValue::Number(number) = left else {
                return Err(FoundationError::new(
                    tos_foundation::FoundationErrorCode::UnsupportedFormat,
                    "float operation requires a number",
                ));
            };
            let value = number.lexeme.parse::<f64>().map_err(|_| {
                FoundationError::new(
                    tos_foundation::FoundationErrorCode::InvalidJson,
                    "invalid finite float",
                )
            })?;
            python_float_text(value).map(|text| string(&text))
        }
        "searchable" => python_searchable_text(&left, max_utf16).map(|text| string(&text)),
        "rank_values" => {
            let fields_value = parse_document(text_field(request, "right_json")?)?;
            let fields = fields_value
                .as_array()
                .ok_or_else(|| {
                    FoundationError::new(
                        tos_foundation::FoundationErrorCode::UnsupportedFormat,
                        "rank fields must be an array",
                    )
                })?
                .iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        FoundationError::new(
                            tos_foundation::FoundationErrorCode::UnsupportedFormat,
                            "rank field must be a string",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let max_values = positive_field(request, "max_values", MAX_DOCUMENT_VISITS)?;
            python_search_rank_values(&left, &fields, max_values, max_utf16)
                .map(|values| JsonValue::Array(values.into_iter().map(JsonValue::String).collect()))
        }
        _ => Err(FoundationError::new(
            tos_foundation::FoundationErrorCode::UnsupportedFormat,
            "unknown Python value operation",
        )),
    }
}

/// Native entry point used by the Worker bridge. It always returns a bounded,
/// versioned envelope, including when parsing or a semantic budget fails.
pub fn worker_python_value_v1(request_json: &[u8]) -> Vec<u8> {
    if request_json.len() > MAX_REQUEST_BYTES {
        return envelope(
            None,
            Some((
                "budget_exceeded",
                "Python value request exceeds byte budget",
            )),
        );
    }
    let request = match parse_json(
        request_json,
        JsonMode::PublishedStrict,
        limits(MAX_REQUEST_BYTES, 8, 64),
    ) {
        Ok(document) => document.into_root(),
        Err(error) => return envelope(None, Some((error.code.as_str(), &error.detail))),
    };
    match run(&request) {
        Ok(value) => envelope(Some(value), None),
        Err(error) => envelope(None, Some((error.code.as_str(), &error.detail))),
    }
}

/// Lossless UTF-16 adapter for callers that already own a JS string. Keeping
/// the UTF-16 units avoids a JSON round-trip for lone surrogates.
pub fn worker_python_lower_utf16_v1(
    units: &[u16],
    max_utf16_units: usize,
) -> Result<Vec<u16>, FoundationError> {
    if max_utf16_units == 0 || max_utf16_units > MAX_LOWER_UNITS || units.len() > max_utf16_units {
        return Err(FoundationError::new(
            tos_foundation::FoundationErrorCode::BudgetExceeded,
            "Unicode lowercase input budget exceeded",
        ));
    }
    python_lower_json_string(&JsonString::from_utf16(units), max_utf16_units)
        .map(|value| value.units().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(raw: &[u8]) -> JsonValue {
        parse_json(
            raw,
            JsonMode::PublishedStrict,
            limits(MAX_RESPONSE_BYTES, 64, 600_000),
        )
        .unwrap()
        .into_root()
    }

    #[test]
    fn worker_envelope_preserves_numeric_string_and_rank_contracts() {
        let equal = worker_python_value_v1(
            br#"{"schema_version":"tos_worker_python_value_request_v1","operation":"equals","left_json":"0","right_json":"-0.0","max_visits":32}"#,
        );
        let equal = response(&equal);
        assert_eq!(
            equal.object_get("ok").and_then(JsonValue::as_bool),
            Some(true)
        );
        assert_eq!(
            equal.object_get("value").and_then(JsonValue::as_bool),
            Some(true)
        );

        let lone = worker_python_value_v1(
            br#"{"schema_version":"tos_worker_python_value_request_v1","operation":"str","left_json":"\"\\ud800\"","max_utf16_units":16}"#,
        );
        let lone = response(&lone);
        let JsonValue::String(value) = lone.object_get("value").unwrap() else {
            panic!("string operation must return a JSON string");
        };
        assert_eq!(value.units(), &[0xd800]);

        let ranked = worker_python_value_v1(
            r#"{"schema_version":"tos_worker_python_value_request_v1","operation":"rank_values","left_json":"{\"display\":{\"title\":{\"default\":\"STRASSE\",\"de\":\"İ\"}}}","right_json":"[\"title\"]","max_values":8,"max_utf16_units":64}"#
                .as_bytes(),
        );
        let ranked = response(&ranked);
        let values: Vec<_> = ranked
            .object_get("value")
            .and_then(JsonValue::as_array)
            .unwrap()
            .iter()
            .map(JsonValue::as_str)
            .collect();
        assert_eq!(values, [Some("strasse"), Some("i\u{0307}")]);
    }
}
