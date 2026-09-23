//! Browser-owned read rules. Fetch, AbortSignal, URL framing, WebMCP
//! registration, storage and rendering stay in the host.
//!
//! The first slice selects an advertised knowledge-search engine. It does not
//! execute a query, interpret a cursor, grant source access or alter the
//! direct knowledge API's legacy default.

mod knowledge_envelope;
mod search_mode;

pub use knowledge_envelope::{
    KnowledgeEnvelopeError, KnowledgeEnvelopeErrorCode, compact_knowledge_search_page_v1,
};
pub use search_mode::{
    SearchMode, SearchSelectionError, SearchSelectionErrorCode, select_knowledge_search_mode_v1,
};

#[cfg(feature = "wasm")]
mod wasm {
    use super::{compact_knowledge_search_page_v1, select_knowledge_search_mode_v1};
    use wasm_bindgen::prelude::*;

    /// Raw request JSON is parsed by the same Rust rule in native and WASM.
    /// Fixed error codes keep host wording and localization outside this core.
    #[wasm_bindgen]
    pub struct SearchModeResult {
        mode: Option<String>,
        error_code: Option<String>,
        minimum: Option<u64>,
    }

    #[wasm_bindgen]
    impl SearchModeResult {
        pub fn mode(&self) -> Option<String> {
            self.mode.clone()
        }
        pub fn error_code(&self) -> Option<String> {
            self.error_code.clone()
        }
        pub fn minimum(&self) -> f64 {
            self.minimum.map_or(-1.0, |value| value as f64)
        }
    }

    #[wasm_bindgen]
    pub fn select_knowledge_search_mode_wasm_v1(request_json: &[u8]) -> SearchModeResult {
        match select_knowledge_search_mode_v1(request_json) {
            Ok(mode) => SearchModeResult {
                mode: Some(mode.as_str().to_owned()),
                error_code: None,
                minimum: None,
            },
            Err(error) => SearchModeResult {
                mode: None,
                error_code: Some(error.code.as_str().to_owned()),
                minimum: error.minimum,
            },
        }
    }

    #[wasm_bindgen]
    pub struct KnowledgeEnvelopeResult {
        bytes: Vec<u8>,
        error_code: Option<String>,
    }

    #[wasm_bindgen]
    impl KnowledgeEnvelopeResult {
        pub fn ok(&self) -> bool {
            self.error_code.is_none()
        }
        pub fn bytes(&self) -> Vec<u8> {
            self.bytes.clone()
        }
        pub fn error_code(&self) -> Option<String> {
            self.error_code.clone()
        }
    }

    #[wasm_bindgen]
    pub fn compact_knowledge_search_page_wasm_v1(request_json: &[u8]) -> KnowledgeEnvelopeResult {
        match compact_knowledge_search_page_v1(request_json) {
            Ok(bytes) => KnowledgeEnvelopeResult {
                bytes,
                error_code: None,
            },
            Err(error) => KnowledgeEnvelopeResult {
                bytes: Vec::new(),
                error_code: Some(error.code.as_str().to_owned()),
            },
        }
    }
}
