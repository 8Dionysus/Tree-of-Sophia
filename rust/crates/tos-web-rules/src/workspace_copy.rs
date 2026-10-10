//! Fixed admission phases for the Observatory's complete personal copy.
//! Component decoders retain their values, normalization and error ordering.
//! Native Date/JSON, storage snapshots, writes and rollback belong to the host.

use std::collections::HashSet;
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};

const SCHEMA: &str = "tos_observatory_workspace_v1";
const INVALID: &str = "invalid_workspace_copy";

fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value.object_get(name)
}
fn array<'a>(value: &'a JsonValue, name: &str, limit: usize) -> Option<&'a [JsonValue]> {
    field(value, name)?
        .as_array()
        .filter(|items| items.len() <= limit)
}
fn number(value: &JsonValue, name: &str) -> Option<u64> {
    field(value, name)?.as_u64()
}
fn yes(value: &JsonValue, name: &str) -> bool {
    matches!(field(value, name), Some(JsonValue::Bool(true)))
}
fn unique(items: &[JsonValue]) -> bool {
    let mut seen = HashSet::with_capacity(items.len());
    items.iter().all(|item| match item {
        // Native Set compares opaque JS strings, including lone surrogates.
        JsonValue::String(id) => !id.units().is_empty() && seen.insert(id.units()),
        _ => false,
    })
}

/// One existing rule receives six explicit, bounded metadata shapes in the
/// real host's admission order. No full workspace or storage value enters it.
/// Success admits only the requested phase, never source/review/canon truth.
pub fn validate_workspace_copy_v1(raw: &[u8]) -> Result<(), &'static str> {
    let packet = parse_json(
        raw,
        JsonMode::RequestLastWins,
        JsonLimits {
            max_bytes: 16_000_000,
            max_depth: 128,
            max_visits: 2_000_000,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| INVALID)?;
    let value = packet.root();
    match field(value, "phase").and_then(JsonValue::as_str) {
        Some("envelope") => {
            if field(value, "schema").and_then(JsonValue::as_str) != Some(SCHEMA)
                || number(value, "version") != Some(1)
                || !yes(value, "valid_date")
                || !number(value, "place_count").is_some_and(|count| count <= 12)
            {
                return Err(INVALID);
            }
        }
        Some("places-size") => {
            if !number(value, "characters").is_some_and(|count| count <= 800_000) {
                return Err(INVALID);
            }
        }
        Some("places") => {
            let places = array(value, "ids", 12).ok_or(INVALID)?;
            if !unique(places) {
                return Err("duplicate_workspace_copy_identity");
            }
            if !yes(value, "lenses_array") {
                return Err(INVALID);
            }
        }
        Some("lenses") => {
            let lenses = array(value, "names", 12).ok_or(INVALID)?;
            if !unique(lenses) {
                return Err("duplicate_workspace_copy_identity");
            }
        }
        Some("normalized") => {
            let invalid_history = "invalid_workspace_copy_history";
            let history = field(value, "history").ok_or(invalid_history)?;
            let entries = array(history, "ids", 100).ok_or(invalid_history)?;
            if number(history, "version") != Some(1) || !unique(entries) {
                return Err(invalid_history);
            }
            let cursor = field(history, "cursor").ok_or(invalid_history)?;
            let valid_cursor = if entries.is_empty() {
                matches!(cursor, JsonValue::Number(n) if n.lexeme == "-1")
            } else {
                cursor
                    .as_u64()
                    .is_some_and(|index| index < entries.len() as u64)
            };
            if !valid_cursor {
                return Err(invalid_history);
            }
            if !field(value, "resume").is_some_and(|resume| {
                resume.is_null() || matches!(field(resume, "id"), Some(JsonValue::String(_)))
            }) || number(value, "preferences_version") != Some(1)
                || number(value, "reading_version") != Some(1)
                || !yes(value, "research_object")
            {
                return Err(INVALID);
            }
        }
        Some("storage") => {
            let flags = array(value, "flags", 7).ok_or("changed_workspace_copy_storage")?;
            if flags.len() != 7
                || flags
                    .iter()
                    .any(|flag| !yes(flag, "present") || !yes(flag, "equal"))
            {
                return Err("changed_workspace_copy_storage");
            }
        }
        _ => return Err(INVALID),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_copy_phases_preserve_identity_cursor_and_custody_controls() {
        let envelope = br#"{"phase":"envelope","schema":"tos_observatory_workspace_v1","version":1,"valid_date":true,"place_count":12}"#;
        assert_eq!(validate_workspace_copy_v1(envelope), Ok(()));
        assert_eq!(
            validate_workspace_copy_v1(br#"{"phase":"places-size","characters":800000}"#),
            Ok(())
        );
        assert_eq!(
            validate_workspace_copy_v1(br#"{"phase":"places-size","characters":800001}"#),
            Err(INVALID)
        );
        assert_eq!(
            validate_workspace_copy_v1(
                br#"{"phase":"places","ids":["p\ud800","p\ud801"],"lenses_array":true}"#
            ),
            Ok(())
        );
        assert_eq!(
            validate_workspace_copy_v1(
                br#"{"phase":"places","ids":["p","p"],"lenses_array":true}"#
            ),
            Err("duplicate_workspace_copy_identity")
        );
        assert_eq!(
            validate_workspace_copy_v1(br#"{"phase":"lenses","names":["one","one"]}"#),
            Err("duplicate_workspace_copy_identity")
        );
        let normalized = r#"{"phase":"normalized","history":{"version":1,"ids":[],"cursor":-1},"resume":null,"preferences_version":1,"reading_version":1,"research_object":true}"#;
        assert_eq!(validate_workspace_copy_v1(normalized.as_bytes()), Ok(()));
        assert_eq!(
            validate_workspace_copy_v1(
                normalized
                    .replace("\"cursor\":-1", "\"cursor\":0")
                    .as_bytes()
            ),
            Err("invalid_workspace_copy_history")
        );
        assert_eq!(
            validate_workspace_copy_v1(br#"{"phase":"storage","flags":[]}"#),
            Err("changed_workspace_copy_storage")
        );
        let flags = "{\"present\":true,\"equal\":true}";
        let custody = format!(
            "{{\"phase\":\"storage\",\"flags\":[{}]}}",
            [flags; 7].join(",")
        );
        assert_eq!(validate_workspace_copy_v1(custody.as_bytes()), Ok(()));
        assert_eq!(
            validate_workspace_copy_v1(
                custody
                    .replace("\"equal\":true", "\"equal\":false")
                    .as_bytes()
            ),
            Err("changed_workspace_copy_storage")
        );
    }
}
