//! Exact mechanical foundation shared by native and WASM ToS consumers.
//!
//! No type here grants source authority, rights, review, canon or admission.

mod capability;
mod coordinates;
mod digest;
mod error;
mod identity;
mod json;
mod logical_ref;
mod path;
mod unicode;

pub use capability::{FoundationCapabilities, capabilities};
pub use coordinates::{ByteSpan, CodePointSpan};
pub use digest::{Digest256, Digest256Hasher};
pub use error::{FoundationError, FoundationErrorCode, Result};
pub use identity::{ExactRecordRef, RecordVersion, SourceRevision, StableId};
pub use json::{
    CanonicalProfile, EncodedJson, JsonDocument, JsonEmissionProfile, JsonLimits, JsonMode,
    JsonNumber, JsonNumberKind, JsonString, JsonValue, canonical_bytes_v1, canonical_digest_v1,
    canonical_raw_bytes_profile, canonical_raw_bytes_v1, emit_json_profile, emit_preserved_json,
    emit_python_compact_json, emit_value_preserved_json, parse_json, parse_json_profile,
};
pub use logical_ref::LogicalRecordRefV1;
pub use path::RelativePath;
pub use unicode::{
    UnicodeProfile, python_casefold_unicode16_v1, python_lower_unicode16_v1,
    python_strip_unicode16_v1,
};

/// Minimal transport-independent observation for a native/WASM executable parity harness.
/// Full source/runtime compatibility needs the independent owner fixture corpus.
pub fn parse_preserve_observation(raw: &[u8], mode: &str) -> String {
    let mode = match mode {
        "PublishedStrict" => JsonMode::PublishedStrict,
        "RequestLastWins" => JsonMode::RequestLastWins,
        _ => return "error:unsupported_format".to_owned(),
    };
    match parse_json(raw, mode, JsonLimits::default())
        .and_then(|document| emit_preserved_json(&document, JsonLimits::default()))
    {
        Ok(bytes) => format!(
            "ok:{}",
            String::from_utf8(bytes).expect("writer produces UTF-8")
        ),
        Err(error) => format!("error:{}", error.code.as_str()),
    }
}

#[cfg(feature = "wasm")]
mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn wasm_parse_preserve_observation(raw: &[u8], mode: &str) -> String {
        super::parse_preserve_observation(raw, mode)
    }
}
