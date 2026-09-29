//! Browser-owned read rules. Fetch, AbortSignal, URL framing, WebMCP
//! registration, storage and rendering stay in the host.
//!
//! The first slice selects an advertised knowledge-search engine. It does not
//! execute a query, interpret a cursor, grant source access or alter the
//! direct knowledge API's legacy default.

#[cfg(feature = "wasm")]
mod client_inspection;
#[cfg(feature = "wasm")]
mod client_packet;
mod claim_reading;
mod claim_reference;
mod constructor_machine;
#[cfg(feature = "wasm")]
mod exploration_session;
mod human_forms;
#[cfg(feature = "wasm")]
mod inspection_session;
mod interface_preferences;
mod knowledge_envelope;
#[cfg(feature = "wasm")]
mod knowledge_scene;
#[cfg(feature = "wasm")]
mod lens_session;
mod live_resume;
mod observatory_conditions;
mod observatory_draft;
mod observatory_pose;
mod reading_resume;
#[cfg(feature = "wasm")]
mod record_context;
mod research_shelf;
mod search_mode;
#[cfg(feature = "wasm")]
mod source_dossier;
#[cfg(feature = "wasm")]
mod source_form_session;
mod temporal_session;
mod workspace_copy;
mod workspace_machine;
mod workspace_proposal;

pub use temporal_session::{
    TemporalSession, TemporalSessionBudget, TemporalSessionStep, TemporalSessionWork,
};

#[cfg(feature = "wasm")]
pub use record_context::RecordContextSession;

#[cfg(feature = "wasm")]
pub use client_inspection::ClientInspectionSession;
#[cfg(feature = "wasm")]
pub use knowledge_scene::KnowledgeSceneSession;
#[cfg(feature = "wasm")]
pub use client_packet::{ClientPacketSession,ClientJsonSession,ClientSelectorSession,ClientMaterialSession};
#[cfg(feature = "wasm")]
pub use source_dossier::SourceDossierSession;

pub use claim_reading::validate_claim_reading_v1;
pub use claim_reference::validate_claim_reference_v1;
pub use interface_preferences::normalize_interface_preferences_v1;
pub use knowledge_envelope::{
    KnowledgeEnvelopeError, KnowledgeEnvelopeErrorCode, compact_knowledge_search_page_v1,
};
pub use live_resume::{rebind_live_resume_v1, validate_live_resume_v1};
pub use observatory_conditions::normalize_observatory_conditions_v1;
pub use observatory_draft::normalize_observatory_draft_v1;
pub use observatory_pose::normalize_observatory_pose_v1;
pub use reading_resume::normalize_reading_resume_v1;
pub use research_shelf::{ShelfPacketIndex, research_shelf_rule_v1};
pub use search_mode::{
    SearchMode, SearchSelectionError, SearchSelectionErrorCode, select_knowledge_search_mode_v1,
};
pub use workspace_copy::validate_workspace_copy_v1;
pub use workspace_machine::{
    WorkspaceMachineError, WorkspaceMachineErrorCode, workspace_transition_v1,
};
pub use workspace_proposal::{
    WorkspaceProposalError, WorkspaceProposalErrorCode, workspace_proposal_digest_v1,
};

#[cfg(feature = "wasm")]
mod wasm {
    use super::{
        ShelfPacketIndex, compact_knowledge_search_page_v1, normalize_interface_preferences_v1,
        normalize_observatory_conditions_v1, normalize_observatory_draft_v1,
        normalize_observatory_pose_v1, normalize_reading_resume_v1, rebind_live_resume_v1,
        research_shelf_rule_v1, select_knowledge_search_mode_v1, validate_claim_reading_v1, validate_claim_reference_v1,
        validate_live_resume_v1, validate_workspace_copy_v1, workspace_proposal_digest_v1,
        workspace_transition_v1,
    };
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn validate_claim_reading_wasm_v1(request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
        validate_claim_reading_v1(request_json).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn validate_claim_reference_wasm_v1(request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
        validate_claim_reference_v1(request_json).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn normalize_reading_resume_wasm_v1(request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
        normalize_reading_resume_v1(request_json).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn validate_live_resume_wasm_v1(request_json: &[u8]) -> Result<(), JsValue> {
        validate_live_resume_v1(request_json).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn rebind_live_resume_wasm_v1(request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
        rebind_live_resume_v1(request_json).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn research_shelf_rule_wasm_v1(request_json: &[u8]) -> Result<Vec<u8>, JsValue> {
        research_shelf_rule_v1(request_json).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub struct ResearchShelfPacketIndex {
        inner: ShelfPacketIndex,
    }

    #[wasm_bindgen]
    impl ResearchShelfPacketIndex {
        #[wasm_bindgen(constructor)]
        pub fn new(header_json: &[u8]) -> Result<ResearchShelfPacketIndex, JsValue> {
            Ok(Self {
                inner: ShelfPacketIndex::new(header_json).map_err(JsValue::from_str)?,
            })
        }
        pub fn accept_records(&mut self, ids_json: &[u8]) -> Result<(), JsValue> {
            self.inner
                .accept_records(ids_json)
                .map_err(JsValue::from_str)
        }
        pub fn accept_collections(&mut self, ids_json: &[u8]) -> Result<(), JsValue> {
            self.inner
                .accept_collections(ids_json)
                .map_err(JsValue::from_str)
        }
        pub fn finish(&self) -> Result<(), JsValue> {
            self.inner.finish().map_err(JsValue::from_str)
        }
    }

    fn lens_json_limits(admission: &[u8]) -> Result<tos_foundation::JsonLimits, JsValue> {
        let document = tos_foundation::parse_json(
            admission,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits {
                max_bytes: 4096,
                ..Default::default()
            },
        )
        .map_err(|_| JsValue::from_str("invalid_lens_admission"))?;
        let cap = |name| {
            document
                .root()
                .object_get(name)
                .and_then(tos_foundation::JsonValue::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| JsValue::from_str("invalid_lens_admission"))
        };
        Ok(tos_foundation::JsonLimits {
            max_bytes: cap("max_json_bytes")?,
            max_depth: cap("max_json_depth")?,
            max_visits: cap("max_json_visits")?,
            max_integer_digits: cap("max_integer_digits")?,
            ..Default::default()
        })
    }

    /** The same shared shape parser used by the actual continuation runs
     * before any publication/D1 read. Registry binding follows verified I/O. */
    #[wasm_bindgen]
    pub fn validate_lens_request_wasm_v1(
        request: &[u8],
        operation: &str,
        admission: &[u8],
    ) -> Result<(), JsValue> {
        super::lens_session::request(request, operation, lens_json_limits(admission)?)
            .map(|_| ())
            .map_err(|error| JsValue::from_str(&format!("{:?}", error.code)))
    }

    fn lens_budget(admission: &[u8]) -> Result<tos_query::lens_plan::PublishedLensBudget, JsValue> {
        let inspect = inspection_budget(admission)?;
        let document = tos_foundation::parse_json(
            admission,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits {
                max_bytes: 4096,
                ..Default::default()
            },
        )
        .map_err(|_| JsValue::from_str("invalid_lens_admission"))?;
        let cap = |name| {
            document
                .root()
                .object_get(name)
                .and_then(tos_foundation::JsonValue::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| JsValue::from_str("invalid_lens_admission"))
        };
        Ok(tos_query::lens_plan::PublishedLensBudget {
            lens: tos_query::knowledge_lens::LensBudget {
                inspect,
                max_candidates: cap("max_candidates")?,
                max_path_steps: cap("max_path_steps")?,
                max_adjacency_rows: cap("max_adjacency_rows")?,
                block_size: cap("block_size")?,
            },
            max_callbacks: cap("max_callbacks")?,
            max_sort_bytes: cap("max_sort_bytes")?,
            max_cache_bytes: cap("max_cache_bytes")?,
            max_cache_entries: cap("max_cache_entries")?,
        })
    }
    fn lens_error(error: tos_query::search_v2::SearchV2Error) -> JsValue {
        JsValue::from_str(&format!("{:?}", error.code))
    }

    #[wasm_bindgen]
    pub struct LensSession {
        inner: super::lens_session::LensSession,
    }
    #[wasm_bindgen]
    impl LensSession {
        #[wasm_bindgen(constructor)]
        pub fn new(
            request: &[u8],
            operation: &str,
            revision: &str,
            top: &[u8],
            metadata: &[u8],
            catalog: &[u8],
            publication: &[u8],
            admission: &[u8],
        ) -> Result<LensSession, JsValue> {
            Ok(Self {
                inner: super::lens_session::LensSession::new(
                    request,
                    operation,
                    revision,
                    top,
                    metadata,
                    catalog,
                    publication,
                    lens_budget(admission)?,
                )
                .map_err(lens_error)?,
            })
        }
        pub fn need(&mut self) -> Result<Option<Vec<u8>>, JsValue> {
            self.inner.need_bytes().map_err(lens_error)
        }
        pub fn resume_rows(&mut self, rows: &[u8], sizes: &[u32]) -> Result<(), JsValue> {
            self.inner.resume_rows(rows, sizes).map_err(lens_error)
        }
        pub fn resume_candidates(&mut self, rows: &[u8]) -> Result<(), JsValue> {
            self.inner.resume_candidates(rows).map_err(lens_error)
        }
        pub fn resume_ids(&mut self, ids: &[u8]) -> Result<(), JsValue> {
            self.inner.resume_ids(ids).map_err(lens_error)
        }
        pub fn resume_headers(&mut self, headers: &[u8]) -> Result<(), JsValue> {
            self.inner.resume_headers(headers).map_err(lens_error)
        }
        pub fn resume_sources(&mut self, sources: &[u8]) -> Result<(), JsValue> {
            self.inner.resume_sources(sources).map_err(lens_error)
        }
        pub fn resume_count(&mut self, count: u64) -> Result<(), JsValue> {
            self.inner.resume_count(count).map_err(lens_error)
        }
        pub fn resume_stores(&mut self, compact: bool, membership: bool) -> Result<(), JsValue> {
            self.inner
                .resume_stores(compact, membership)
                .map_err(lens_error)
        }
        pub fn finish(&mut self) -> Result<Vec<u8>, JsValue> {
            self.inner.finish().map_err(lens_error)
        }
    }

    fn inspection_budget(admission: &[u8]) -> Result<tos_query::InspectBudget, JsValue> {
        let document = tos_foundation::parse_json(
            admission,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits {
                max_bytes: 4096,
                ..Default::default()
            },
        )
        .map_err(|_| JsValue::from_str("invalid_inspection_admission"))?;
        let cap = |name| {
            document
                .root()
                .object_get(name)
                .and_then(tos_foundation::JsonValue::as_u64)
                .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| JsValue::from_str("invalid_inspection_admission"))
        };
        Ok(tos_query::InspectBudget {
            max_open_vm_steps: cap("max_open_vm_steps")? as u64,
            max_read_vm_steps: cap("max_read_vm_steps")? as u64,
            max_matches: cap("max_matches")?,
            max_rows: cap("max_rows")? as u64,
            max_field_bytes: cap("max_field_bytes")?,
            max_payload_bytes: cap("max_payload_bytes")?,
            max_decoded_bytes: cap("max_decoded_bytes")? as u64,
            max_response_bytes: cap("max_response_bytes")?,
            json: tos_foundation::JsonLimits::new(
                cap("max_json_bytes")?,
                cap("max_json_depth")?,
                cap("max_json_visits")?,
                cap("max_integer_digits")?,
            )
            .map_err(|_| JsValue::from_str("invalid_inspection_admission"))?,
        })
    }

    fn exploration_budget(
        admission: &[u8],
    ) -> Result<tos_query::exploration_plan::PublishedExplorationBudget, JsValue> {
        let read = inspection_budget(admission)?;
        let document = tos_foundation::parse_json(
            admission,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits {
                max_bytes: 4096,
                ..Default::default()
            },
        )
        .map_err(|_| JsValue::from_str("invalid_exploration_admission"))?;
        let cap = |name| {
            document
                .root()
                .object_get(name)
                .and_then(tos_foundation::JsonValue::as_u64)
                .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991)
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| JsValue::from_str("invalid_exploration_admission"))
        };
        Ok(tos_query::exploration_plan::PublishedExplorationBudget {
            exploration: tos_query::knowledge_exploration::ExplorationBudget {
                read,
                max_work_units: cap("max_work_units")?,
                max_session_nodes: cap("max_session_nodes")?,
                max_session_relations: cap("max_session_relations")?,
                max_state_bytes: cap("max_state_bytes")?,
                max_checkpoint_bytes: cap("max_checkpoint_bytes")?,
                max_checkpoints: cap("max_checkpoints")?,
            },
            max_cache_bytes: cap("max_cache_bytes")?,
            max_cache_entries: cap("max_cache_entries")?,
        })
    }
    fn exploration_error(error: tos_query::search_v2::SearchV2Error) -> JsValue {
        JsValue::from_str(&format!("{:?}", error.code))
    }
    #[wasm_bindgen]
    pub fn validate_exploration_request_wasm_v1(
        request: &[u8],
        admission: &[u8],
    ) -> Result<Option<String>, JsValue> {
        let budget = exploration_budget(admission)?;
        let document = tos_foundation::parse_json(
            request,
            tos_foundation::JsonMode::RequestLastWins,
            tos_foundation::JsonLimits {
                max_bytes: 65536,
                ..budget.exploration.read.json
            },
        )
        .map_err(|_| JsValue::from_str("InvalidRequest"))?;
        tos_query::knowledge_exploration::validate_published_exploration_request(document.root())
            .map_err(exploration_error)
    }
    #[wasm_bindgen]
    pub fn validate_exploration_replay_wasm_v1(
        packet: &[u8],
        admission: &[u8],
    ) -> Result<(), JsValue> {
        let budget = exploration_budget(admission)?;
        tos_query::knowledge_exploration::validate_published_exploration_replay(
            packet,
            tos_foundation::JsonLimits {
                max_bytes: budget.exploration.read.max_response_bytes,
                ..budget.exploration.read.json
            },
        )
        .map_err(exploration_error)
    }
    #[wasm_bindgen]
    pub fn exploration_cache_version_wasm_v1() -> String {
        tos_query::knowledge_exploration::PUBLISHED_EXPLORATION_CACHE_VERSION.to_owned()
    }
    #[wasm_bindgen]
    pub struct ExplorationSession {
        inner: super::exploration_session::ExplorationSession,
    }
    #[wasm_bindgen]
    impl ExplorationSession {
        #[wasm_bindgen(constructor)]
        pub fn new(
            request: &[u8],
            revision: String,
            data_revision: String,
            epoch: u64,
            top: &[u8],
            state: &[u8],
            admission: &[u8],
        ) -> Result<Self, JsValue> {
            Ok(Self {
                inner: super::exploration_session::ExplorationSession::new(
                    request,
                    &revision,
                    &data_revision,
                    epoch,
                    top,
                    state,
                    exploration_budget(admission)?,
                )
                .map_err(exploration_error)?,
            })
        }
        pub fn need(&mut self) -> Result<Option<Vec<u8>>, JsValue> {
            self.inner.need_bytes().map_err(exploration_error)
        }
        pub fn resume_rows(
            &mut self,
            rows: &[u8],
            sizes: &[u32],
            ambiguous: &[u8],
        ) -> Result<(), JsValue> {
            self.inner
                .resume_rows(rows, sizes, ambiguous)
                .map_err(exploration_error)
        }
        pub fn resume_focus(
            &mut self,
            matched: usize,
            rows: &[u8],
            sizes: &[u32],
        ) -> Result<(), JsValue> {
            self.inner
                .resume_focus(matched, rows, sizes)
                .map_err(exploration_error)
        }
        pub fn resume_ids(&mut self, ids: &[u8]) -> Result<(), JsValue> {
            self.inner.resume_ids(ids).map_err(exploration_error)
        }
        pub fn paused(&self) -> Result<bool, JsValue> {
            self.inner.paused().map_err(exploration_error)
        }
        pub fn state(&self) -> Result<Vec<u8>, JsValue> {
            self.inner.state().map_err(exploration_error)
        }
        pub fn finish(&mut self, cursor: Option<String>) -> Result<Vec<u8>, JsValue> {
            self.inner
                .finish(cursor.as_deref())
                .map_err(exploration_error)
        }
    }

    /// The same request rule as the selected native plan, before any D1 read.
    #[wasm_bindgen]
    pub fn validate_inspect_request_wasm_v1(
        request: &[u8],
        admission: &[u8],
    ) -> Result<(), JsValue> {
        let budget = inspection_budget(admission)?;
        let document = tos_foundation::parse_json(
            request,
            tos_foundation::JsonMode::RequestLastWins,
            tos_foundation::JsonLimits {
                max_bytes: 65536,
                ..budget.json
            },
        )
        .map_err(|_| JsValue::from_str("InvalidRequest"))?;
        tos_query::validate_inspect_request(document.root(), budget)
            .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))
    }

    /// A concrete node/relation plan. Each authenticated batch resumes once;
    /// no query replay, publication issuer, source access or runtime grant.
    #[wasm_bindgen]
    pub struct InspectionSession {
        inner: super::inspection_session::InspectionSession,
    }
    #[wasm_bindgen]
    impl InspectionSession {
        #[wasm_bindgen(constructor)]
        pub fn new(
            request: &[u8],
            revision: String,
            top: &[u8],
            admission: &[u8],
        ) -> Result<Self, JsValue> {
            let inner = super::inspection_session::InspectionSession::new(
                request,
                revision,
                top,
                inspection_budget(admission)?,
            )
            .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))?;
            Ok(Self { inner })
        }
        pub fn need(&self) -> Result<Option<Vec<u8>>, JsValue> {
            self.inner
                .need_bytes()
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))
        }
        pub fn resume_lookup(&mut self, rows: &[u8]) -> Result<(), JsValue> {
            self.inner
                .resume_lookup(rows)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))
        }
        pub fn resume_incident(&mut self, total: u64, rows: &[u8]) -> Result<(), JsValue> {
            self.inner
                .resume_incident(total, rows)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))
        }
        pub fn resume_endpoints(&mut self, rows: &[u8]) -> Result<(), JsValue> {
            self.inner
                .resume_endpoints(rows)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))
        }
        pub fn finish(&mut self) -> Result<Vec<u8>, JsValue> {
            self.inner
                .finish()
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))
        }
    }

    fn temporal_budget(admission: &[u8]) -> Result<super::TemporalSessionBudget, JsValue> {
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
        Ok(budget)
    }

    /// The maintained published HTTP route validates request shape before any
    /// D1 access. Actual selected revision matching remains in the comparator.
    #[wasm_bindgen]
    pub fn validate_temporal_request_wasm_v1(
        request: &[u8],
        admission: &[u8],
    ) -> Result<(), JsValue> {
        let budget = temporal_budget(admission)?;
        let document = tos_foundation::parse_json(
            request,
            tos_foundation::JsonMode::RequestLastWins,
            budget.json,
        )
        .map_err(|_| JsValue::from_str("InvalidRequest"))?;
        tos_query::validate_temporal_request(document.root())
            .map_err(|error| JsValue::from_str(&format!("{:?}", error.code)))
    }

    /// A need is physical I/O intent. Successful bytes do not select a model,
    /// publication or disclosure rule; those belong to the actual consumer.
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
        /// Admission is supplied by the caller. Caps bound parsing/replay/output,
        /// not measured CPU instructions. Optional published_output selects
        /// insertion-ordered Python compact bytes; absent/false stays canonical.
        #[wasm_bindgen(constructor)]
        pub fn new(
            revision: String,
            profile: String,
            request: &[u8],
            admission: &[u8],
            published_output: Option<bool>,
        ) -> Result<TemporalReplaySession, JsValue> {
            let budget = temporal_budget(admission)?;
            let inner = super::TemporalSession::new(revision, profile, request, budget)
                .map_err(|e| JsValue::from_str(&format!("{:?}", e.code)))?;
            let inner = if published_output == Some(true) {
                inner.with_published_output()
            } else {
                inner
            };
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

    #[wasm_bindgen]
    pub struct BrowserSearchModeSession {
        inner: super::search_mode::BrowserSearchMode,
    }
    #[wasm_bindgen]
    impl BrowserSearchModeSession {
        #[wasm_bindgen(constructor)]
        pub fn new(requested: u8) -> Self {
            Self {
                inner: super::search_mode::BrowserSearchMode::new(requested),
            }
        }
        pub fn phase(&self) -> u8 {
            self.inner.phase()
        }
        pub fn current(&self) -> String {
            self.inner.current().as_str().to_owned()
        }
        pub fn mode(&self) -> Option<String> {
            self.inner.selected().map(|mode| mode.as_str().to_owned())
        }
        pub fn error_code(&self) -> Option<String> {
            self.inner
                .error()
                .map(|error| error.code.as_str().to_owned())
        }
        pub fn query(&mut self, present: bool, units: &[u16]) {
            self.inner.query(present, units);
        }
        pub fn availability(&mut self, value: bool) {
            self.inner.availability(value);
        }
        pub fn minimum(&mut self, nullish: bool, numeric: bool, value: f64) {
            self.inner.minimum(nullish, numeric, value);
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

    #[wasm_bindgen]
    pub fn validate_workspace_copy_wasm_v1(packet_json: &[u8]) -> Result<(), JsValue> {
        validate_workspace_copy_v1(packet_json).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn normalize_interface_preferences_wasm_v1(raw: &[u8]) -> Result<Vec<u8>, JsValue> {
        normalize_interface_preferences_v1(raw).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn normalize_observatory_pose_wasm_v1(raw: &[u8]) -> Result<Vec<u8>, JsValue> {
        normalize_observatory_pose_v1(raw).map_err(JsValue::from_str)
    }

    /// Validate the actual Observatory human-form selection and its bounded
    /// source identity projection. This remains delivery validation only.
    #[wasm_bindgen]
    pub struct HumanFormRuleSession {
        inner: super::human_forms::HumanFormRuleSession,
    }

    #[wasm_bindgen]
    impl HumanFormRuleSession {
        #[wasm_bindgen(constructor)]
        pub fn new(
            selection_json: &[u8],
            source_json: &[u8],
            requested: Option<String>,
            forms_count: u32,
        ) -> Result<Self, JsValue> {
            Ok(Self {
                inner: super::human_forms::HumanFormRuleSession::new(
                    selection_json,
                    source_json,
                    requested.as_deref(),
                    forms_count as usize,
                )
                .map_err(JsValue::from_str)?,
            })
        }

        pub fn identity(&self) -> Result<String, JsValue> {
            self.inner.identity().map_err(JsValue::from_str)
        }

        #[wasm_bindgen(js_name = inspectionIndex)]
        pub fn inspection_index(&self, role: &str) -> Option<u32> {
            self.inner.inspection_index(role)
        }

        #[wasm_bindgen(js_name = inspectionSourcePointer)]
        pub fn inspection_source_pointer(&self, role: &str) -> Option<String> {
            self.inner.inspection_source_pointer(role)
        }

        #[wasm_bindgen(js_name = validateInspectedPacket)]
        pub fn validate_inspected_packet(
            &self,
            role: &str,
            index: u32,
            packet_json: &[u8],
        ) -> Result<(), JsValue> {
            self.inner
                .validate_inspected_packet(role, index, packet_json)
                .map_err(JsValue::from_str)
        }
    }

    #[wasm_bindgen]
    pub fn human_form_content_language_wasm_v1(value: &str) -> bool {
        super::human_forms::content_language_v1(value)
    }

    #[wasm_bindgen]
    pub fn human_form_exact_ref_wasm_v1(value_json: &[u8]) -> bool {
        super::human_forms::exact_ref_json_v1(value_json)
    }

    #[wasm_bindgen]
    pub fn human_form_same_ref_wasm_v1(left_json: &[u8], right_json: &[u8]) -> bool {
        super::human_forms::same_ref_json_v1(left_json, right_json)
    }

    #[wasm_bindgen]
    pub fn human_form_valid_identity_wasm_v1(identity: &str, requested: Option<String>) -> bool {
        super::human_forms::valid_identity_v1(identity, requested.as_deref())
    }
    #[wasm_bindgen]
    pub fn normalize_observatory_conditions_wasm_v1(raw: &[u8]) -> Result<Vec<u8>, JsValue> {
        normalize_observatory_conditions_v1(raw).map_err(JsValue::from_str)
    }

    #[wasm_bindgen]
    pub fn normalize_observatory_draft_wasm_v1(raw: &[u8]) -> Result<Vec<u8>, JsValue> {
        normalize_observatory_draft_v1(raw).map_err(JsValue::from_str)
    }
}
