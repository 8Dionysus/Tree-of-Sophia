//! Versioned WASM adapter over the shared foundation codec.
//!
//! Input and successful output are raw UTF-8 bytes. The host never parses
//! source JSON before Rust, so numeric lexemes and duplicate keys survive.
//! No source authority, rights, admission or corpus access is conferred.

use tos_foundation::{
    CanonicalProfile, FoundationError, JsonLimits, JsonMode, canonical_bytes_v1, capabilities,
    emit_preserved_json, parse_json,
};

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

pub const ABI_VERSION: &str = "tos_web_codec_v1";

/// Separate success bytes and stable mechanical error code. The original
/// input remains opaque to JS; byte offsets are into that exact input.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub struct CodecResult {
    bytes: Vec<u8>,
    error_code: Option<String>,
    error_byte_offset: Option<usize>,
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
impl CodecResult {
    pub fn ok(&self) -> bool {
        self.error_code.is_none()
    }
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
    pub fn error_code(&self) -> Option<String> {
        self.error_code.clone()
    }
    /// `-1` means no byte offset was supplied by the codec.
    pub fn error_byte_offset(&self) -> i32 {
        self.error_byte_offset
            .map_or(-1, |offset| i32::try_from(offset).unwrap_or(-1))
    }
}

impl CodecResult {
    fn success(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            error_code: None,
            error_byte_offset: None,
        }
    }

    fn failure(error: FoundationError) -> Self {
        Self {
            bytes: Vec::new(),
            error_code: Some(error.code.as_str().to_owned()),
            error_byte_offset: error.byte_offset,
        }
    }

    fn unsupported() -> Self {
        Self {
            bytes: Vec::new(),
            error_code: Some("unsupported_format".to_owned()),
            error_byte_offset: None,
        }
    }
}

/// Operations are versioned strings so an unknown future profile fails
/// closed. `parse_preserve` emits source member order and exact number lexemes.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn codec_v1(raw: &[u8], operation: &str, profile: &str) -> CodecResult {
    let mode = match (operation, profile) {
        ("parse_preserve", "PublishedStrict") => JsonMode::PublishedStrict,
        ("parse_preserve", "RequestLastWins") => JsonMode::RequestLastWins,
        ("canonical", selected) if selected == CanonicalProfile::CorpusSnapshotV1.as_str() => {
            JsonMode::PublishedStrict
        }
        _ => return CodecResult::unsupported(),
    };
    let limits = JsonLimits::default();
    let result = parse_json(raw, mode, limits).and_then(|document| {
        if operation == "canonical" {
            canonical_bytes_v1(document.root(), CanonicalProfile::CorpusSnapshotV1, limits)
        } else {
            emit_preserved_json(&document, limits)
        }
    });
    match result {
        Ok(bytes) => CodecResult::success(bytes),
        Err(error) => CodecResult::failure(error),
    }
}

/// Version and only the capabilities actually implemented by this codec.
/// The fixed JSON contains no source-derived strings.
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn codec_capabilities_v1() -> String {
    let cap = capabilities();
    format!(
        "{{\"abi\":\"{ABI_VERSION}\",\"json_format\":\"{}\",\"descriptor_format\":\"{}\",\"canonical_profile\":\"{}\",\"canonical_float_supported\":{},\"strict_duplicate_rejection\":{},\"request_last_wins\":{},\"escaped_lone_surrogate_preserved\":{},\"canonical_lone_surrogate_supported\":{}}}",
        cap.json_format,
        cap.descriptor_format,
        cap.canonical_profile,
        cap.canonical_float_supported,
        cap.strict_duplicate_rejection,
        cap.request_last_wins,
        cap.escaped_lone_surrogate_preserved,
        cap.canonical_lone_surrogate_supported,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_keeps_exact_numeric_and_duplicate_profiles() {
        let request = codec_v1(
            br#"{"a":1,"b":2,"\u0061":9007199254740993}"#,
            "parse_preserve",
            "RequestLastWins",
        );
        assert!(request.ok());
        assert_eq!(request.bytes(), br#"{"a":9007199254740993,"b":2}"#);
        let strict = codec_v1(
            br#"{"a":1,"\u0061":2}"#,
            "parse_preserve",
            "PublishedStrict",
        );
        assert_eq!(strict.error_code().as_deref(), Some("duplicate_member"));
        assert_eq!(strict.bytes(), b"");
        let canonical = codec_v1(
            "{\"z\":-0,\"a\":\"é\"}".as_bytes(),
            "canonical",
            CanonicalProfile::CorpusSnapshotV1.as_str(),
        );
        assert_eq!(canonical.bytes(), "{\"a\":\"é\",\"z\":0}\n".as_bytes());
    }

    #[test]
    fn unknown_profile_is_explicit() {
        let result = codec_v1(b"{}", "canonical", "FutureV2");
        assert_eq!(result.error_code().as_deref(), Some("unsupported_format"));
        assert_eq!(result.error_byte_offset(), -1);
    }
}
