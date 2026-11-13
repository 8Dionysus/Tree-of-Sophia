//! Common metadata rules for private source-byte custody inventories.

use serde_json::Value;

pub fn valid_entry(entry: &Value) -> bool {
    let Some(sha) = entry["sha256"].as_str() else {
        return false;
    };
    let Some(revision) = entry["source_revision"].as_str() else {
        return false;
    };
    let lowercase_hex = |value: &str, length: usize| {
        value.len() == length
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    lowercase_hex(sha, 64)
        && lowercase_hex(revision, 40)
        && entry["bytes"].as_u64().is_some()
        && entry["bucket"] == "tos-source-bytes"
        && entry["access"] == "private"
        && entry["key"].as_str() == Some(format!("sha256/{sha}").as_str())
}
