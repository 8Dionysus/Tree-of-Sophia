//! Browser-owned read rules. Fetch, AbortSignal, URL framing, WebMCP
//! registration, storage and rendering stay in the host.
//!
//! The first slice selects an advertised knowledge-search engine. It does not
//! execute a query, interpret a cursor, grant source access or alter the
//! direct knowledge API's legacy default.

mod knowledge_envelope;
mod search_mode;
mod temporal_session;
mod workspace_machine;
mod workspace_proposal;

pub use temporal_session::{
    TemporalSession, TemporalSessionBudget, TemporalSessionStep, TemporalSessionWork,
};

pub use knowledge_envelope::{
    KnowledgeEnvelopeError, KnowledgeEnvelopeErrorCode, compact_knowledge_search_page_v1,
};
pub use search_mode::{
    SearchMode, SearchSelectionError, SearchSelectionErrorCode, select_knowledge_search_mode_v1,
};
pub use workspace_machine::{
    WorkspaceMachineError, WorkspaceMachineErrorCode, workspace_transition_v1,
};
pub use workspace_proposal::{
    WorkspaceProposalError, WorkspaceProposalErrorCode, workspace_proposal_digest_v1,
};

#[cfg(feature = "wasm")]
mod wasm {
    use super::{
        compact_knowledge_search_page_v1, select_knowledge_search_mode_v1,
        workspace_proposal_digest_v1, workspace_transition_v1,
    };
    use wasm_bindgen::prelude::*;

    /// Private continuation. A need is only physical I/O intent; successful
    /// bytes still require the selected owner's current held disclosure lease.
    #[wasm_bindgen]
    pub struct TemporalReplaySession {
        inner: super::TemporalSession,
    }

    #[wasm_bindgen]
    pub struct TemporalReplayStep {
        need: Option<String>,
        bytes: Vec<u8>,
        error_code: Option<String>,
    }

    #[wasm_bindgen]
    impl TemporalReplayStep {
        pub fn need(&self) -> Option<String> {
            self.need.clone()
        }
        pub fn bytes(&self) -> Vec<u8> {
            self.bytes.clone()
        }
        pub fn error_code(&self) -> Option<String> {
            self.error_code.clone()
        }
    }

    #[wasm_bindgen]
    impl TemporalReplaySession {
        pub fn replay_executions(&self) -> usize {
            self.inner.work().replay_executions
        }
        pub fn retained_source_bytes(&self) -> usize {
            self.inner.work().retained_source_bytes
        }
        pub fn replayed_input_bytes(&self) -> usize {
            self.inner.work().replayed_input_bytes
        }
        pub fn exact_lookups(&self) -> usize {
            self.inner.work().exact_lookups
        }
        /// Admission is supplied by the selected caller, never inferred from
        /// projection availability. All byte/CPU caps are positive safe integers.
        #[wasm_bindgen(constructor)]
        pub fn new(
            revision: String,
            profile: String,
            request: &[u8],
            admission: &[u8],
        ) -> Result<TemporalReplaySession, JsValue> {
            let document = tos_foundation::parse_json(
                admission,
                tos_foundation::JsonMode::PublishedStrict,
                tos_foundation::JsonLimits {
                    max_bytes: 4096,
                    ..Default::default()
                },
            )
            .map_err(|_| JsValue::from_str("invalid_temporal_admission"))?;
            let value = document.root();
            let cap = |name| {
                value
                    .object_get(name)
                    .and_then(tos_foundation::JsonValue::as_u64)
                    .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| JsValue::from_str("invalid_temporal_admission"))
            };
            let budget = super::TemporalSessionBudget {
                json: tos_foundation::JsonLimits::new(
                    cap("max_json_bytes")?,
                    cap("max_json_depth")?,
                    cap("max_json_visits")?,
                    cap("max_integer_digits")?,
                )
                .map_err(|_| JsValue::from_str("invalid_temporal_admission"))?,
                max_source_bytes: cap("max_source_bytes")?,
                max_replay_bytes: cap("max_replay_bytes")?,
                max_output_bytes: cap("max_output_bytes")?,
            };
            let inner = super::TemporalSession::new(revision, profile, request, budget)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))?;
            Ok(Self { inner })
        }
        pub fn advance(&mut self) -> TemporalReplayStep {
            match self.inner.advance() {
                Ok(super::TemporalSessionStep::Need(id)) => TemporalReplayStep {
                    need: Some(id),
                    bytes: vec![],
                    error_code: None,
                },
                Ok(super::TemporalSessionStep::Complete(bytes)) => TemporalReplayStep {
                    need: None,
                    bytes,
                    error_code: None,
                },
                Err(e) => TemporalReplayStep {
                    need: None,
                    bytes: vec![],
                    error_code: Some(format!("{:?}", e.code)),
                },
            }
        }
        /// Empty bytes plus absent=true attest exact absence from selected I/O.
        pub fn provide(&mut self, id: &str, bytes: &[u8], absent: bool) -> Result<(), JsValue> {
            if absent && !bytes.is_empty() {
                return Err(JsValue::from_str("invalid_absence_response"));
            }
            self.inner
                .provide(id, if absent { None } else { Some(bytes) })
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))
        }
    }

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

    #[wasm_bindgen]
    pub struct WorkspaceProposalResult {
        digest: Option<String>,
        error_code: Option<String>,
    }

    #[wasm_bindgen]
    impl WorkspaceProposalResult {
        pub fn digest(&self) -> Option<String> {
            self.digest.clone()
        }
        pub fn error_code(&self) -> Option<String> {
            self.error_code.clone()
        }
    }

    #[wasm_bindgen]
    pub fn workspace_proposal_digest_wasm_v1(request_json: &[u8]) -> WorkspaceProposalResult {
        match workspace_proposal_digest_v1(request_json) {
            Ok(digest) => WorkspaceProposalResult {
                digest: Some(digest),
                error_code: None,
            },
            Err(error) => WorkspaceProposalResult {
                digest: None,
                error_code: Some(error.code.as_str().to_owned()),
            },
        }
    }

    #[wasm_bindgen]
    pub fn workspace_transition_wasm_v1(request_json: &[u8]) -> KnowledgeEnvelopeResult {
        match workspace_transition_v1(request_json) {
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
