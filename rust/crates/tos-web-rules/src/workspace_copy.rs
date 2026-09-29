//! The Observatory's complete personal-copy envelope. Component decoders
//! normalize their own versioned packets before this cross-section admission.
//! Storage snapshots, compare-and-swap and rollback belong to the browser.

use std::collections::HashSet;
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};

const SCHEMA: &str = "tos_observatory_workspace_v1";

fn field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
    value.object_get(name)
}
fn array<'a>(value: &'a JsonValue, name: &str, limit: usize) -> Option<&'a [JsonValue]> {
    field(value, name)?
        .as_array()
        .filter(|items| items.len() <= limit)
}
fn unique(items: &[JsonValue], key: &str) -> bool {
    let mut seen = HashSet::with_capacity(items.len());
    items.iter().all(|item| {
        field(item, key)
            .and_then(JsonValue::as_str)
            .is_some_and(|value| !value.is_empty() && seen.insert(value.to_owned()))
    })
}

/// Verify the normalized copy that the component owners will store. This
/// never grants source, rights, review or canon authority to local material.
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
    .map_err(|_| "invalid_workspace_copy")?;
    let value = packet.root();
    if field(value, "schema").and_then(JsonValue::as_str) != Some(SCHEMA)
        || field(value, "v").and_then(JsonValue::as_u64) != Some(1)
        || field(value, "exportedAt")
            .and_then(JsonValue::as_str)
            .is_none()
    {
        return Err("invalid_workspace_copy");
    }
    let places = array(value, "places", 12).ok_or("invalid_workspace_copy")?;
    let lenses = array(value, "lenses", 12).ok_or("invalid_workspace_copy")?;
    if !unique(places, "id") || !unique(lenses, "name") {
        return Err("duplicate_workspace_copy_identity");
    }
    let history = field(value, "history").ok_or("invalid_workspace_copy")?;
    let entries = array(history, "entries", 100).ok_or("invalid_workspace_copy")?;
    if field(history, "v").and_then(JsonValue::as_u64) != Some(1) || !unique(entries, "id") {
        return Err("invalid_workspace_copy_history");
    }
    let cursor = field(history, "cursor").ok_or("invalid_workspace_copy_history")?;
    let valid_cursor = if entries.is_empty() {
        matches!(cursor, JsonValue::Number(n) if n.lexeme == "-1")
    } else {
        cursor
            .as_u64()
            .is_some_and(|index| index < entries.len() as u64)
    };
    if !valid_cursor {
        return Err("invalid_workspace_copy_history");
    }
    if !field(value, "resume").is_some_and(|resume| {
        resume.is_null() || field(resume, "id").and_then(JsonValue::as_str).is_some()
    }) || !field(value, "preferences")
        .is_some_and(|item| field(item, "v").and_then(JsonValue::as_u64) == Some(1))
        || !field(value, "reading")
            .is_some_and(|item| field(item, "v").and_then(JsonValue::as_u64) == Some(1))
        || !field(value, "research").is_some_and(|item| item.as_object().is_some())
    {
        return Err("invalid_workspace_copy");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const COPY: &str = r#"{"schema":"tos_observatory_workspace_v1","v":1,"exportedAt":"2026-09-07T20:00:00.000Z","history":{"v":1,"entries":[],"cursor":-1},"places":[],"lenses":[],"resume":null,"preferences":{"v":1},"reading":{"v":1},"research":{}}"#;

    #[test]
    fn accepts_empty_complete_copy_and_rejects_identity_or_cursor_damage() {
        assert_eq!(validate_workspace_copy_v1(COPY.as_bytes()), Ok(()));
        let cursor = COPY.replace("\"cursor\":-1", "\"cursor\":0");
        assert_eq!(
            validate_workspace_copy_v1(cursor.as_bytes()),
            Err("invalid_workspace_copy_history")
        );
        let duplicate = COPY.replace(
            "\"places\":[]",
            "\"places\":[{\"id\":\"p\"},{\"id\":\"p\"}]",
        );
        assert_eq!(
            validate_workspace_copy_v1(duplicate.as_bytes()),
            Err("duplicate_workspace_copy_identity")
        );
    }
}
