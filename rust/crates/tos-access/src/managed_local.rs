//! Explicit managed-local projection composition. ReleaseStore is the holder;
//! neither CMP custody nor these callbacks grant raw source/payload access.
use crate::exploration_checkpoints::{
    CheckpointLimits, ProcessExplorationCheckpoints, SelectedExplorationCheckpoints as Checkpoints,
};
use crate::indexed_cursor::{MAX_CURSOR_BYTES, NativeIndexedCursorCodec};
use crate::release_state::{ManagedRelease, ReleaseLease};
use crate::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, KnowledgeOperation as O,
    KnowledgeRequest as R, Params, PreparedPacket,
};
use std::{
    os::unix::ffi::OsStrExt,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tos_compiler::{
    ColdOpenLimits, NavigationOriginalReceipt, PhilosophyOriginalCollection,
    PhilosophyOriginalReceipt, QueryVocabulary, VerifiedKnowledgeModel,
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonString, JsonValue, canonical_bytes_v1,
    parse_json,
};
use tos_query::search_v2::{
    CurrentPolicyBinding, IndexedSearchV2Request, SearchContinuationState, SearchV2Error,
    SearchV2ErrorCode,
};
use tos_query::{
    AbortProbe, AbortReason, BoundCmpKnowledge, CatalogCurrentAuthority, CatalogDisclosureLease,
    CatalogDisclosureScope, CatalogError, CatalogErrorCode, IndexedDisclosureLease,
    IndexedDisclosureScope, IndexedKnowledgeAuthority, InspectCurrentAuthority,
    InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier, ObservedSearchCandidate,
};

pub struct ManagedLocalExecutor {
    release: Arc<ManagedRelease>,
    model: Mutex<VerifiedKnowledgeModel<'static>>,
    vocabulary: QueryVocabulary,
    descriptor: Vec<u8>,
    registries: [Vec<u8>; 2],
    original: Option<NavigationOriginalReceipt>,
    philosophy_original: Option<PhilosophyOriginalReceipt>,
    corpus_original: Option<tos_compiler::CorpusOriginalReceipt>,
    corpus_context: Option<tos_query::corpus_read::CorpusReadContext>,
    corpus_guards: Vec<crate::release_state::ReleaseMemberGuard>,
    cold: ColdOpenLimits,
    profile: AccessProfile,
    checkpoints: Checkpoints,
}
fn unavailable(message: &'static str) -> AccessError {
    AccessError::new(AccessErrorCode::Unavailable, message)
}
fn query_error(message: &'static str) -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::StaleSelection,
        message,
    }
}
fn catalog_error(message: &'static str) -> CatalogError {
    CatalogError {
        code: CatalogErrorCode::StaleSelection,
        message,
    }
}
impl ManagedLocalExecutor {
    pub fn open(root: &Path, profile: AccessProfile) -> Result<Self, AccessError> {
        Self::open_with_checkpoints(root, profile, None)
    }
    /// Explicit disposable persistence for imported callers with separate native
    /// children. Checkpoints supply neither release selection nor source grants.
    pub fn open_with_checkpoints(
        root: &Path,
        profile: AccessProfile,
        checkpoint_path: Option<&Path>,
    ) -> Result<Self, AccessError> {
        Self::open_with_selected_source_root(root, None, profile, checkpoint_path)
    }
    /// Select the release and its exact logical SourceRoot in one native
    /// owner open. A caller cannot splice arbitrary carriers into a managed pair.
    pub fn open_with_selected_source_root(
        root: &Path,
        selected_source_root: Option<&Path>,
        profile: AccessProfile,
        checkpoint_path: Option<&Path>,
    ) -> Result<Self, AccessError> {
        if checkpoint_path.is_some_and(|path| {
            !path.is_absolute()
                || path.starts_with(root)
                || selected_source_root.is_some_and(|source_root| path.starts_with(source_root))
        }) {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "checkpoint requires an absolute path outside the selected release and source root",
            ));
        }
        let release = ManagedRelease::open_selected_source_root(root, selected_source_root)?;
        let mut cold_hold = release.acquire()?;
        let raw = release.selection_bytes()?;
        let bootstrap = tos_foundation::parse_json(
            &raw,
            tos_foundation::JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .map_err(|_| unavailable("native selection metadata invalid"))?;
        let paths = bootstrap
            .root()
            .object_get("paths")
            .ok_or_else(|| unavailable("native selection paths absent"))?;
        let member = |name: &str| -> Result<&str, AccessError> {
            paths
                .object_get(name)
                .and_then(tos_foundation::JsonValue::as_str)
                .ok_or_else(|| unavailable("native selection path absent"))
        };
        let descriptor =
            release.member_bytes(member("descriptor")?, JsonLimits::default().max_bytes)?;
        let entity =
            release.member_bytes(member("entity_registry")?, JsonLimits::default().max_bytes)?;
        let relation = release.member_bytes(
            member("relation_registry")?,
            JsonLimits::default().max_bytes,
        )?;
        let selection = tos_compiler::NativeKnowledgeSelection::decode(
            &raw,
            &descriptor,
            &entity,
            &relation,
            tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
            JsonLimits::default().max_bytes,
        )
        .map_err(|_| unavailable("native producer selection is not admitted"))?;
        // A persistent release manifest does not own CMD's scoped current hold.
        // Managed-source first consumption stays inside with_current_model.
        if selection.producer().managed_source.is_some() {
            return Err(unavailable(
                "managed source current holder is not available in persistent release",
            ));
        }
        if release.query_schema != selection.expectation().model_abi
            || release.compiler_version != tos_compiler::COMPILER_VERSION
        {
            return Err(unavailable(
                "native release code and selected model ABI differ",
            ));
        }
        let (size, sha) = release.member_binding(&selection.paths().model)?;
        if size != selection.expectation().model_size_bytes
            || sha.to_hex() != selection.expectation().model_sha256
        {
            return Err(unavailable(
                "native model manifest and producer expectation differ",
            ));
        }
        let model_path = release.member_path(&selection.paths().model)?;
        let custody: Arc<dyn tos_compiler::ImmutableKnowledgeCustody> = Arc::new(
            tos_compiler::LinuxFsVerityCustody::new(
                selection.fs_verity().clone(),
                selection.process_limits(),
            )
            .map_err(|_| unavailable("native fs-verity or process custody unavailable"))?,
        );
        let model = tos_compiler::open_selected_knowledge_model_owned(
            &model_path,
            selection.expectation().clone(),
            custody,
            selection.cold_limits(),
        )
        .map_err(|_| unavailable("native selected model cold admission refused"))?;
        let corpus_original = selection.producer().corpus_original.clone();
        let (corpus_context, corpus_guards) = match &corpus_original {
            Some(receipt) => {
                let (context, guards) = cold_hold.admit_corpus_members(
                    receipt,
                    usize::try_from(selection.cold_limits().max_work_bytes).unwrap_or(usize::MAX),
                )?;
                (Some(context), guards)
            }
            None => (None, vec![]),
        };
        cold_hold.recheck()?;
        let mut executor = Self::from_admitted_selection(
            release,
            model,
            selection.vocabulary().clone(),
            descriptor,
            [entity, relation],
            selection.producer().navigation_original.clone(),
            selection.producer().philosophy_original.clone(),
            corpus_original,
            corpus_context,
            corpus_guards,
            selection.cold_limits(),
            profile,
        )?;
        if let Some(path) = checkpoint_path {
            executor.checkpoints = Checkpoints::Persistent(
                crate::persistent_exploration_checkpoints::PersistentExplorationCheckpoints::open(
                    path,
                    &model_path,
                    executor.checkpoints.limits(),
                    executor.budgets().exploration,
                )
                .map_err(AccessError::from)?,
            );
        }
        Ok(executor)
    }
    // Called only by the explicit manifest/companion cold-admission path below.
    fn from_admitted_selection(
        release: Arc<ManagedRelease>,
        model: VerifiedKnowledgeModel<'static>,
        vocabulary: QueryVocabulary,
        descriptor: Vec<u8>,
        registries: [Vec<u8>; 2],
        original: Option<NavigationOriginalReceipt>,
        philosophy_original: Option<PhilosophyOriginalReceipt>,
        corpus_original: Option<tos_compiler::CorpusOriginalReceipt>,
        corpus_context: Option<tos_query::corpus_read::CorpusReadContext>,
        corpus_guards: Vec<crate::release_state::ReleaseMemberGuard>,
        cold: ColdOpenLimits,
        profile: AccessProfile,
    ) -> Result<Self, AccessError> {
        // This caps canonical encoded checkpoint residency, not parsed RSS.
        // QRY session node/relation/JSON/work limits bound individual states;
        // the required live process envelope remains the allocation boundary.
        let checkpoint_bytes = usize::try_from(cold.max_work_bytes)
            .unwrap_or(usize::MAX)
            .min(32 * 1024 * 1024);
        let checkpoints = ProcessExplorationCheckpoints::new(CheckpointLimits {
            ttl: Duration::from_secs(900),
            max_entries: 128,
            max_encoded_bytes: checkpoint_bytes,
        })?;
        // Bind once during admission; each request binds its bounded fork again.
        tos_query::bind_verified_knowledge(&model, &vocabulary, &descriptor)?;
        Ok(Self {
            release,
            model: Mutex::new(model),
            vocabulary,
            descriptor,
            registries,
            original,
            philosophy_original,
            corpus_original,
            corpus_context,
            corpus_guards,
            cold,
            profile,
            checkpoints: Checkpoints::Process(checkpoints),
        })
    }
    fn budgets(&self) -> crate::knowledge::SelectedKnowledgeBudgets {
        let rows = usize::try_from(self.cold.max_rows).unwrap_or(usize::MAX);
        let work = usize::try_from(self.cold.max_work_bytes).unwrap_or(usize::MAX);
        let json = JsonLimits {
            max_bytes: self.cold.max_row_bytes.max(self.profile.max_response_bytes),
            ..JsonLimits::default()
        };
        let inspect = tos_query::InspectBudget {
            max_open_vm_steps: self.cold.max_vm_steps,
            max_read_vm_steps: self.cold.max_vm_steps,
            max_matches: rows,
            max_rows: self.cold.max_rows,
            max_field_bytes: self.cold.max_row_bytes,
            max_payload_bytes: self.cold.max_row_bytes,
            max_decoded_bytes: self.cold.max_work_bytes,
            max_response_bytes: self.profile.max_response_bytes,
            json,
        };
        crate::knowledge::SelectedKnowledgeBudgets {
            catalog: tos_query::CatalogBudget {
                max_open_vm_steps: self.cold.max_vm_steps,
                max_read_vm_steps: self.cold.max_vm_steps,
                max_packet_bytes: self.profile.max_response_bytes,
                max_decoded_bytes: work,
                json,
            },
            inspect,
            lens: tos_query::knowledge_lens::LensBudget {
                inspect,
                max_candidates: rows,
                max_path_steps: usize::try_from(self.cold.max_vm_steps).unwrap_or(usize::MAX),
                max_adjacency_rows: rows,
                block_size: 128,
            },
            // Existing maintained disposable exploration profile, bounded again
            // by the selected cold owner's work envelope.
            exploration: tos_query::knowledge_exploration::ExplorationBudget {
                read: inspect,
                max_work_units: 512,
                max_session_nodes: 10_000,
                max_session_relations: 20_000,
                max_state_bytes: work.min(32 * 1024 * 1024),
                max_checkpoint_bytes: work.min(32 * 1024 * 1024),
                max_checkpoints: 128,
            },
        }
    }
    fn indexed_budget(&self) -> tos_query::IndexedPageBudget {
        use tos_query::search_candidate::{CandidateReadBudget, CandidateVerifyBudget};
        use tos_query::search_index::{GramSeekBudget, PostingSeekBudget};
        let rows = self.cold.max_rows;
        // Two kinds each own bounded gram, posting, candidate and verification
        // counters. Split the selected work envelope across those eight costs;
        // startup and final packet/transport have their own declared caps.
        let work = self.cold.max_work_bytes / 8;
        let vm = self.cold.max_vm_steps / 8;
        let row_bytes = usize::try_from(self.cold.max_row_bytes)
            .unwrap_or(usize::MAX)
            .min(usize::try_from(work).unwrap_or(usize::MAX));
        let field_bytes = row_bytes.min(8_192);
        let work_bytes = usize::try_from(work).unwrap_or(usize::MAX);
        let kind = tos_query::SearchKindBudget {
            grams: GramSeekBudget {
                max_lookups: 256,
                max_candidates: rows,
                max_vm_steps: vm,
                max_rows: self.cold.max_rows.min(256),
                max_decoded_bytes: work,
            },
            postings: PostingSeekBudget {
                max_probes: rows.saturating_add(1),
                max_rows: self.cold.max_rows.min(rows.saturating_add(1)),
                max_decoded_bytes: work,
                max_vm_steps: vm,
                page_rows: usize::try_from(rows.min(128)).unwrap_or(128),
            },
            candidate: CandidateReadBudget {
                max_vm_steps: vm,
                max_decoded_bytes: work,
                max_payload_bytes: row_bytes,
                max_field_bytes: field_bytes,
                max_document_chars: work,
            },
            verify: CandidateVerifyBudget {
                document: tos_query::SearchDocumentBudget {
                    max_carrier_bytes: row_bytes,
                    max_document_bytes: work_bytes,
                    max_document_code_points: work_bytes,
                    json: JsonLimits {
                        max_bytes: row_bytes,
                        ..JsonLimits::default()
                    },
                },
                max_rank_field_bytes: field_bytes,
                max_rank_values: 64,
            },
            max_candidate_vm_steps: vm,
            max_candidate_decoded_bytes: work,
            max_verified_chars: work,
            max_verified_bytes: work,
            max_observed_candidates: usize::try_from(rows).unwrap_or(usize::MAX),
            max_observed_bytes: work,
            max_selected_result_bytes: self.profile.max_response_bytes.min(work_bytes),
        };
        tos_query::IndexedPageBudget {
            nodes: kind,
            relations: kind,
            max_open_vm_steps: self.cold.max_vm_steps,
            max_response_bytes: self.profile.max_response_bytes,
            max_cursor_bytes: MAX_CURSOR_BYTES,
            json: JsonLimits {
                max_bytes: self.profile.max_response_bytes,
                ..JsonLimits::default()
            },
        }
    }
}
fn intended(operation: O) -> &'static str {
    if operation.is_corpus() {
        return tos_query::corpus_read::CORPUS_INTENDED_USE;
    }
    if operation.is_philosophy() {
        return tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE;
    }
    match operation {
        O::Catalog => tos_query::CATALOG_INTENDED_USE,
        O::Temporal => tos_query::TEMPORAL_INTENDED_USE,
        O::Lens => tos_query::knowledge_lens::LENS_INTENDED_USE,
        O::Focus => tos_query::knowledge_lens::FOCUS_INTENDED_USE,
        O::StoredLens => tos_query::knowledge_lens::STORED_LENS_INTENDED_USE,
        O::Explore => tos_query::knowledge_exploration::EXPLORATION_INTENDED_USE,
        O::Contracts => tos_query::knowledge_contracts::KNOWLEDGE_CONTRACTS_INTENDED_USE,
        O::ExplorationContracts => {
            unreachable!("software contracts do not select source authority")
        }
        O::SearchCapabilities => {
            tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_INTENDED_USE
        }
        O::Dossier => tos_query::source_dossier::DOSSIER_INTENDED_USE,
        O::Node | O::Relation => tos_query::INSPECT_INTENDED_USE,
        _ => unreachable!("philosophy handled above"),
    }
}
impl AccessExecutor for ManagedLocalExecutor {
    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        Err(unavailable(
            "exact source owner is not selected by the release holder",
        ))
    }
    fn access_health_available(&self) -> bool {
        self.model.lock().is_ok() && self.release.acquire().is_ok()
    }
    fn access_health(
        &self,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        self.access_health_report(probe).map(|report| report.packet)
    }
    fn access_health_report(
        &self,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<crate::common::PreparedHealth<'static>, AccessError> {
        execute_selected_access_health(self, probe)
    }
    fn exploration_runtime_capabilities(&self) -> tos_foundation::JsonValue {
        let selected = self
            .release
            .acquire()
            .ok()
            .map(|_hold| (self.checkpoints.limits(), self.budgets().exploration));
        crate::exploration_contracts::runtime_capabilities(selected)
    }
    fn source_gap_available(&self) -> bool {
        true
    }
    fn source_gap(
        &self,
        request: tos_query::source_gap::SourceGapRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        execute_selected_source_gap(
            &self.release,
            &request,
            tos_query::source_gap::SourceGapBudget {
                json: self.budgets().inspect.json,
                max_work_steps: self.cold.max_vm_steps,
                max_response_bytes: self.profile.max_response_bytes,
            },
            usize::try_from(self.cold.max_work_bytes).unwrap_or(usize::MAX),
            probe,
        )
    }
    fn knowledge_available(&self, operation: O) -> bool {
        if operation == O::AccessHealth {
            return self.access_health_available();
        }
        if operation.is_corpus() {
            return self.corpus_original.is_some()
                && self.corpus_context.is_some()
                && self
                    .release
                    .acquire()
                    .and_then(|mut hold| hold.retain_member_guards(&self.corpus_guards))
                    .is_ok();
        }
        if operation == O::EvidenceLens {
            return self
                .release
                .member_binding("data/ToS/derived-exports/epistemic_evidence_projection.min.json")
                .is_ok()
                && (self.philosophy_original.is_some()
                    || (self.corpus_original.is_some() && self.corpus_context.is_some()));
        }
        if operation.is_philosophy() {
            self.philosophy_original.is_some()
        } else {
            operation != O::Dossier || self.original.is_some()
        }
    }
    fn knowledge_search_legacy_available(&self) -> bool {
        true
    }
    fn knowledge_search_indexed_available(&self) -> bool {
        self.release.acquire().is_ok()
    }
    fn knowledge_search_indexed(
        &self,
        request: crate::IndexedSearchParams,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        crate::knowledge::check_abort(&probe)?;
        let cold = self
            .model
            .lock()
            .map_err(|_| unavailable("selected native model lock poisoned"))?;
        let mut model = cold
            .fork_reader_with_vm_budget(self.cold.max_vm_steps)
            .map_err(|_| unavailable("selected native reader unavailable"))?;
        drop(cold);
        let bound = tos_query::bind_verified_knowledge(&model, &self.vocabulary, &self.descriptor)?;
        let authority = Authority::new(
            self,
            &bound,
            tos_query::INDEXED_SEARCH_OPERATION_ID,
            tos_query::INDEXED_SEARCH_INTENDED_USE,
        )?;
        let mut authority = IndexedAuthority {
            authority,
            probe: Arc::clone(&probe),
        };
        let indexed_request = IndexedSearchV2Request {
            query: request.query,
            sources: request.sources,
            kind_ids: request.kind_ids,
            predicate_ids: request.predicate_ids,
            limit: request.limit,
        };
        let normalized = indexed_request
            .clone()
            .normalize(bound.selection(), &bound)?;
        let initial = SearchContinuationState::new(
            bound.selection().clone(),
            normalized,
            authority.authority.policy.clone(),
            &bound,
        )?;
        let mut cursor_codec = NativeIndexedCursorCodec::new(initial, bound.owner_receipt_id());
        let packet = tos_query::execute_indexed_search_page(
            &mut model,
            &bound,
            &mut authority,
            &mut cursor_codec,
            indexed_request,
            request.cursor.as_deref(),
            self.indexed_budget(),
        )?;
        crate::knowledge::check_abort(&probe)?;
        Ok(crate::knowledge::from_indexed_search(packet))
    }
    fn knowledge_search_legacy(
        &self,
        request: tos_query::knowledge_legacy_search::LegacySearchRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        let cold = self
            .model
            .lock()
            .map_err(|_| unavailable("selected native model lock poisoned"))?;
        let mut model = cold
            .fork_reader_with_vm_budget(self.cold.max_vm_steps)
            .map_err(|_| unavailable("selected native reader unavailable"))?;
        drop(cold);
        let bound = tos_query::bind_verified_knowledge(&model, &self.vocabulary, &self.descriptor)?;
        let mut authority = Authority::new(
            self,
            &bound,
            tos_query::knowledge_legacy_search::LEGACY_SEARCH_OPERATION,
            tos_query::knowledge_legacy_search::LEGACY_SEARCH_INTENDED_USE,
        )?;
        let inspect = self.budgets().inspect;
        let work = usize::try_from(self.cold.max_work_bytes).unwrap_or(usize::MAX);
        let budget = tos_query::knowledge_legacy_search::LegacySearchBudget {
            inspect,
            document: tos_query::SearchDocumentBudget {
                max_carrier_bytes: self.cold.max_row_bytes,
                max_document_bytes: work,
                max_document_code_points: work,
                json: inspect.json,
            },
            max_candidates: usize::try_from(self.cold.max_rows).unwrap_or(usize::MAX),
            max_document_bytes: self.cold.max_work_bytes,
            max_document_code_points: self.cold.max_work_bytes,
            max_retained_per_kind: usize::try_from(self.cold.max_rows).unwrap_or(usize::MAX),
            max_retained_bytes: work,
            block_size: 128,
        };
        crate::knowledge::execute_selected_legacy_search(
            &mut model,
            &bound,
            &mut authority,
            &request,
            budget,
            probe,
        )
    }
    fn knowledge(
        &self,
        request: R,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        let cold = self
            .model
            .lock()
            .map_err(|_| unavailable("selected native model lock poisoned"))?;
        let mut model = cold
            .fork_reader_with_vm_budget(self.cold.max_vm_steps)
            .map_err(|_| unavailable("selected native reader unavailable"))?;
        drop(cold);
        let bound = tos_query::bind_verified_knowledge(&model, &self.vocabulary, &self.descriptor)?;
        if matches!(request, R::ExplorationContracts) {
            return crate::exploration_contracts::execute(self, self.profile.max_response_bytes);
        }
        let operation = request.operation();
        let intended_use = match &request {
            R::EvidenceLens(request) => request.intended_use(),
            _ => intended(operation),
        };
        let mut inspect = Authority::new(self, &bound, operation.id(), intended_use)?;
        let budgets = self.budgets();
        if matches!(request, R::PhilosophyAudit) {
            crate::knowledge::check_abort(&probe)?;
            let cap = budgets
                .inspect
                .max_payload_bytes
                .min(self.profile.max_response_bytes)
                .min(usize::try_from(budgets.inspect.max_decoded_bytes).unwrap_or(usize::MAX))
                .min(usize::try_from(budgets.inspect.max_read_vm_steps).unwrap_or(usize::MAX));
            let hold = inspect
                .lease
                .as_mut()
                .ok_or_else(|| unavailable("audit current hold absent"))?;
            let (path, raw) = hold.public_philosophy_audit(cap)?;
            crate::knowledge::check_abort(&probe)?;
            return crate::knowledge::execute_selected_philosophy_audit(
                &mut model,
                &bound,
                &mut inspect,
                &path,
                raw.as_deref(),
                tos_query::philosophy_read::PhilosophyReadBudget {
                    inspect: budgets.inspect,
                    max_work_steps: budgets.inspect.max_read_vm_steps,
                },
                probe,
            );
        }
        if let R::EvidenceLens(request) = &request {
            crate::knowledge::check_abort(&probe)?;
            let cap = budgets
                .inspect
                .max_payload_bytes
                .min(self.profile.max_response_bytes)
                .min(usize::try_from(budgets.inspect.max_decoded_bytes).unwrap_or(usize::MAX))
                .min(usize::try_from(budgets.inspect.max_read_vm_steps).unwrap_or(usize::MAX));
            let hold = inspect
                .lease
                .as_mut()
                .ok_or_else(|| unavailable("Evidence Lens current hold absent"))?;
            if matches!(
                request.mode,
                tos_query::philosophy_read::EvidenceMode::Corpus
            ) {
                hold.retain_member_guards(&self.corpus_guards)?;
            }
            let raw = hold.public_evidence_projection(cap)?;
            crate::knowledge::check_abort(&probe)?;
            return crate::knowledge::execute_selected_evidence(
                &mut model,
                &bound,
                &mut inspect,
                request,
                &raw,
                self.corpus_context.as_ref(),
                tos_query::philosophy_read::PhilosophyReadBudget {
                    inspect: budgets.inspect,
                    max_work_steps: budgets.inspect.max_read_vm_steps,
                },
                probe,
            );
        }
        if matches!(request, R::CorpusViewIds) {
            if self.corpus_context.is_none() {
                return Err(unavailable("selected corpus source context unavailable"));
            }
            return crate::knowledge::execute_selected_corpus_view_ids(
                &mut model,
                &bound,
                &mut inspect,
                tos_query::corpus_read::CorpusReadBudget {
                    inspect: budgets.inspect,
                    max_work_steps: budgets.inspect.max_read_vm_steps,
                },
                probe,
            );
        }
        if let R::Corpus(request) = &request {
            let context = self
                .corpus_context
                .as_ref()
                .ok_or_else(|| unavailable("selected corpus source context unavailable"))?;
            return crate::knowledge::execute_selected_corpus(
                &mut model,
                &bound,
                &mut inspect,
                context,
                request,
                tos_query::corpus_read::CorpusReadBudget {
                    inspect: budgets.inspect,
                    max_work_steps: budgets.inspect.max_read_vm_steps,
                },
                probe,
            );
        }
        if matches!(request, R::Contracts) {
            let budget = tos_query::knowledge_contracts::KnowledgeContractBudget {
                max_input_bytes: usize::try_from(self.cold.max_work_bytes).unwrap_or(usize::MAX),
                max_registry_bytes: budgets.inspect.max_payload_bytes,
                max_response_bytes: self.profile.max_response_bytes,
                json: budgets.inspect.json,
            };
            return crate::knowledge::execute_selected_knowledge_contracts(
                &mut model,
                &bound,
                &mut inspect,
                [&self.registries[0], &self.registries[1]],
                budget,
                budgets.inspect,
                probe,
            );
        }
        let mut catalog = Authority::new(self, &bound, O::Catalog.id(), intended(O::Catalog))?;
        let mut checkpoints = self.checkpoints.clone();
        if let Checkpoints::Persistent(store) = &mut checkpoints {
            store.set_abort_probe(probe.clone());
        }
        crate::knowledge::execute_selected_knowledge(
            &mut model,
            &bound,
            &mut catalog,
            &mut inspect,
            checkpoints.store(),
            request,
            budgets,
            probe,
        )
    }
}
const HEALTH_CHILDREN: usize = 5;
pub(crate) fn partition_health_inspect_budget(
    mut budget: tos_query::InspectBudget,
    max_response_bytes: usize,
    max_work_bytes: u64,
    max_rows: u64,
    max_vm_steps: u64,
) -> Result<tos_query::InspectBudget, AccessError> {
    let response = max_response_bytes / (HEALTH_CHILDREN + 1);
    let decoded = max_work_bytes / (HEALTH_CHILDREN as u64 + 1);
    let rows = max_rows / (HEALTH_CHILDREN as u64 + 1);
    let vm = max_vm_steps / (HEALTH_CHILDREN as u64 + 1);
    if response == 0 || decoded == 0 || rows == 0 || vm == 0 {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "selected access health aggregate budget unavailable",
        ));
    }
    budget.max_open_vm_steps = max_vm_steps;
    budget.max_read_vm_steps = vm;
    budget.max_matches = budget.max_matches / (HEALTH_CHILDREN + 1);
    budget.max_rows = rows;
    budget.max_decoded_bytes = decoded;
    budget.max_response_bytes = response;
    budget.json.max_bytes = budget
        .json
        .max_bytes
        .min(usize::try_from(decoded).unwrap_or(usize::MAX).max(response));
    if budget.max_matches == 0
        || budget.max_payload_bytes == 0
        || budget.max_field_bytes == 0
        || budget.json.max_bytes == 0
    {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "selected access health aggregate budget unavailable",
        ));
    }
    Ok(budget)
}
pub(crate) fn health_child_packet(
    body: &[u8],
    limits: JsonLimits,
    meter: &mut tos_query::InspectVisitMeter,
) -> Result<JsonValue, AccessError> {
    let mut limits = limits;
    limits.max_bytes = limits.max_bytes.min(body.len());
    meter
        .parse_json(body, JsonMode::PublishedStrict, limits)
        .map(|packet| packet.into_root())
        .map_err(|reason| {
            AccessError::new(
                if reason.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                    AccessErrorCode::BudgetExceeded
                } else {
                    AccessErrorCode::CorruptSelectedCarrier
                },
                "selected access health child packet invalid",
            )
        })
}
fn health_required_text(
    value: &JsonValue,
    key: &str,
    maximum: usize,
) -> Result<String, AccessError> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .filter(|value| !value.is_empty() && value.len() <= maximum)
        .map(str::to_owned)
        .ok_or_else(|| {
            AccessError::new(
                AccessErrorCode::CorruptSelectedCarrier,
                "selected access health child field invalid",
            )
        })
}
fn health_optional_text(
    value: &JsonValue,
    key: &str,
    maximum: usize,
) -> Result<Option<String>, AccessError> {
    match value.object_get(key) {
        Some(JsonValue::Null) => Ok(None),
        Some(JsonValue::String(value)) => value
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= maximum)
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                AccessError::new(
                    AccessErrorCode::CorruptSelectedCarrier,
                    "selected access health child field invalid",
                )
            }),
        _ => Err(AccessError::new(
            AccessErrorCode::CorruptSelectedCarrier,
            "selected access health child field invalid",
        )),
    }
}
fn health_child_schema(value: &JsonValue, expected: &str) -> Result<(), AccessError> {
    if value.as_object().is_some()
        && value
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            == Some(expected)
    {
        Ok(())
    } else {
        Err(AccessError::new(
            AccessErrorCode::CorruptSelectedCarrier,
            "selected access health child schema invalid",
        ))
    }
}
fn health_subject(
    status: &str,
    code: Option<&str>,
    message: Option<&str>,
    source_schema: Option<&str>,
    view_id: Option<&str>,
) -> JsonValue {
    JsonValue::Object(
        vec![
            (
                JsonString::from_utf8("status"),
                JsonValue::String(JsonString::from_utf8(status)),
            ),
            (
                JsonString::from_utf8("code"),
                code.map(|value| JsonValue::String(JsonString::from_utf8(value)))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                JsonString::from_utf8("message"),
                message
                    .map(|value| JsonValue::String(JsonString::from_utf8(value)))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                JsonString::from_utf8("source_schema"),
                source_schema
                    .map(|value| JsonValue::String(JsonString::from_utf8(value)))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                JsonString::from_utf8("view_id"),
                view_id
                    .map(|value| JsonValue::String(JsonString::from_utf8(value)))
                    .unwrap_or(JsonValue::Null),
            ),
        ]
        .into_iter()
        .collect(),
    )
}
fn health_object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}
fn health_error_subject(error: &AccessError) -> JsonValue {
    health_subject(
        "refused",
        Some(error.code_str()),
        Some(error.message),
        None,
        None,
    )
}
fn health_error_is_global(error: &AccessError) -> bool {
    matches!(
        error.code,
        AccessErrorCode::StaleSelection
            | AccessErrorCode::PublicationPending
            | AccessErrorCode::Cancelled
            | AccessErrorCode::DeadlineExceeded
    )
}
fn check_health_visit_meter(meter: &tos_query::InspectVisitMeter) -> Result<(), AccessError> {
    if meter.failed() {
        Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "selected access health cumulative JSON visit budget exceeded",
        ))
    } else {
        Ok(())
    }
}
fn append_health_error(errors: &mut Vec<JsonValue>, message: String) {
    errors.push(JsonValue::String(JsonString::from_utf8(&message)));
}
fn consume_inspect_health_packet(
    packet: tos_query::DisclosableInspect<'_>,
    probe: &Arc<dyn AbortProbe>,
    limits: JsonLimits,
    meter: &mut tos_query::InspectVisitMeter,
) -> Result<JsonValue, AccessError> {
    let (body, mut lease) = packet.into_parts();
    lease.recheck()?;
    crate::knowledge::check_abort(probe)?;
    health_child_packet(&body, limits, meter)
}
fn consume_catalog_health_packet(
    packet: tos_query::DisclosableCatalog<'_>,
    probe: &Arc<dyn AbortProbe>,
    limits: JsonLimits,
    meter: &mut tos_query::InspectVisitMeter,
) -> Result<JsonValue, AccessError> {
    let (body, mut lease) = packet.into_parts();
    lease.recheck()?;
    crate::knowledge::check_abort(probe)?;
    health_child_packet(&body, limits, meter)
}
pub(crate) enum HealthChild<'a> {
    CorpusSeed,
    CorpusView(&'a str),
    PhilosophySeed,
    PhilosophyView(&'a str),
    Knowledge,
}
/// The same five maintained health checks, under one aggregate visit meter.
/// Adapters supply genuine child authorities and retain their disclosure fences.
pub(crate) fn compose_selected_access_health(
    inspect_budget: tos_query::InspectBudget,
    max_response_bytes: usize,
    output_json: JsonLimits,
    probe: &Arc<dyn AbortProbe>,
    child: impl FnMut(
        HealthChild<'_>,
        tos_query::InspectBudget,
        &mut tos_query::InspectVisitMeter,
    ) -> Result<JsonValue, AccessError>,
) -> Result<(Vec<u8>, bool), AccessError> {
    compose_selected_access_health_original(
        inspect_budget,
        max_response_bytes,
        output_json,
        probe,
        None,
        child,
    )
}

/// Optional borrowed Original owner for native composite delivery. Existing
/// selected adapters retain their current visit-meter behavior.
pub(crate) trait HealthOriginalBudget {
    fn check(&self) -> Result<(), AccessError>;
    fn admit_workspace(&self, bytes: usize) -> Result<(), AccessError>;
    fn charge_work(&self, units: usize) -> Result<(), AccessError>;
    fn emit(
        &self,
        value: &JsonValue,
        limits: JsonLimits,
        meter: &mut tos_query::InspectVisitMeter,
    ) -> Result<Vec<u8>, AccessError>;
}

pub(crate) fn compose_selected_access_health_original(
    inspect_budget: tos_query::InspectBudget,
    max_response_bytes: usize,
    output_json: JsonLimits,
    probe: &Arc<dyn AbortProbe>,
    original: Option<&dyn HealthOriginalBudget>,
    mut child: impl FnMut(
        HealthChild<'_>,
        tos_query::InspectBudget,
        &mut tos_query::InspectVisitMeter,
    ) -> Result<JsonValue, AccessError>,
) -> Result<(Vec<u8>, bool), AccessError> {
    if let Some(original) = original {
        original.check()?;
        // Fixed report/subject/error envelopes. Child-owned strings/count maps
        // are admitted from their actual geometry by the native child adapter.
        original.admit_workspace(
            128 * std::mem::size_of::<(JsonString, JsonValue)>()
                + 16 * 512 * (1 + 2 * std::mem::size_of::<u16>()),
        )?;
        original.charge_work(128)?;
    }
    let mut visit_meter = tos_query::InspectVisitMeter::new(inspect_budget.json.max_visits);
    let mut errors = Vec::with_capacity(8);
    let child_json = inspect_budget.json;
    let mut corpus_source_schema = None;
    let mut corpus_view_id = None;
    let mut corpus_index_status = health_subject("unavailable", None, None, None, None);
    let mut corpus_view_status = health_subject("not_checked", None, None, None, None);
    let corpus_seed = child(HealthChild::CorpusSeed, inspect_budget, &mut visit_meter);
    match corpus_seed {
        Ok(seed) => {
            health_child_schema(&seed, "tos_selected_corpus_health_seed_v1")?;
            corpus_source_schema = Some(health_required_text(
                &seed,
                "index_schema_version",
                inspect_budget.max_field_bytes,
            )?);
            corpus_view_id =
                health_optional_text(&seed, "first_graph_view_id", inspect_budget.max_field_bytes)?;
            if corpus_source_schema.as_deref() != Some("tos_corpus_index_v1") {
                append_health_error(&mut errors, "unsupported corpus index schema".into());
                corpus_index_status = health_subject(
                    "refused",
                    Some("unsupported_schema"),
                    Some("unsupported corpus index schema"),
                    corpus_source_schema.as_deref(),
                    corpus_view_id.as_deref(),
                );
            } else {
                corpus_index_status = health_subject(
                    "ready",
                    None,
                    None,
                    corpus_source_schema.as_deref(),
                    corpus_view_id.as_deref(),
                );
                match corpus_view_id.as_deref() {
                    None => {
                        append_health_error(
                            &mut errors,
                            "corpus index has no supported graph views".into(),
                        );
                        corpus_view_status = health_subject(
                            "unavailable",
                            Some("no_supported_view"),
                            Some("corpus index has no supported graph views"),
                            corpus_source_schema.as_deref(),
                            None,
                        );
                    }
                    Some(view_id) => {
                        let sample = child(
                            HealthChild::CorpusView(view_id),
                            inspect_budget,
                            &mut visit_meter,
                        )
                        .and_then(|value| {
                            if value.object_get("schema").and_then(JsonValue::as_str)
                                != Some("tos_corpus_mcp_graph_view_v1")
                                || value
                                    .object_get("view")
                                    .and_then(|view| view.object_get("view_id"))
                                    .and_then(JsonValue::as_str)
                                    != Some(view_id)
                            {
                                return Err(AccessError::new(
                                    AccessErrorCode::CorruptSelectedCarrier,
                                    "selected corpus graph-view sample differs",
                                ));
                            }
                            Ok::<(), AccessError>(())
                        });
                        match sample {
                            Ok(()) => {
                                corpus_view_status = health_subject(
                                    "ready",
                                    None,
                                    None,
                                    corpus_source_schema.as_deref(),
                                    Some(view_id),
                                );
                            }
                            Err(error) if health_error_is_global(&error) => return Err(error),
                            Err(error) => {
                                append_health_error(
                                    &mut errors,
                                    format!("corpus index invalid: {}", error.message),
                                );
                                corpus_view_status = health_error_subject(&error);
                            }
                        }
                    }
                }
            }
        }
        Err(error) if health_error_is_global(&error) => return Err(error),
        Err(error) => {
            append_health_error(
                &mut errors,
                format!("corpus index invalid: {}", error.message),
            );
            corpus_index_status = health_error_subject(&error);
            corpus_view_status = health_subject(
                "not_checked",
                Some("source_unavailable"),
                Some("corpus view sample requires a valid selected index"),
                None,
                None,
            );
        }
    }
    check_health_visit_meter(&visit_meter)?;
    if let Some(original) = original {
        original.check()?;
    }

    let mut philosophy_schema = None;
    let mut philosophy_view_id = None;
    let mut philosophy_projection_status = health_subject("unavailable", None, None, None, None);
    let mut philosophy_view_status = health_subject("not_checked", None, None, None, None);
    let philosophy_seed = child(
        HealthChild::PhilosophySeed,
        inspect_budget,
        &mut visit_meter,
    );
    match philosophy_seed {
        Ok(seed) => {
            health_child_schema(&seed, "tos_selected_philosophy_health_seed_v1")?;
            philosophy_schema = Some(health_required_text(
                &seed,
                "projection_schema_version",
                inspect_budget.max_field_bytes,
            )?);
            philosophy_view_id =
                health_optional_text(&seed, "first_view_id", inspect_budget.max_field_bytes)?;
            if !matches!(
                philosophy_schema.as_deref(),
                Some("tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2")
            ) {
                append_health_error(
                    &mut errors,
                    "philosophy projection invalid: unsupported source schema".into(),
                );
                philosophy_projection_status = health_subject(
                    "refused",
                    Some("unsupported_schema"),
                    Some("philosophy projection invalid: unsupported source schema"),
                    philosophy_schema.as_deref(),
                    philosophy_view_id.as_deref(),
                );
            } else {
                philosophy_projection_status = health_subject(
                    "ready",
                    None,
                    None,
                    philosophy_schema.as_deref(),
                    philosophy_view_id.as_deref(),
                );
                match philosophy_view_id.as_deref() {
                    None => {
                        append_health_error(
                            &mut errors,
                            "philosophy projection has no graph views".into(),
                        );
                        philosophy_view_status = health_subject(
                            "unavailable",
                            Some("no_graph_views"),
                            Some("philosophy projection has no graph views"),
                            philosophy_schema.as_deref(),
                            None,
                        );
                    }
                    Some(view_id) => {
                        let sample = child(
                            HealthChild::PhilosophyView(view_id),
                            inspect_budget,
                            &mut visit_meter,
                        )
                        .and_then(|value| {
                            if value.object_get("schema").and_then(JsonValue::as_str)
                                != Some("tos_philosophy_mcp_view_v1")
                                || value
                                    .object_get("view")
                                    .and_then(|view| view.object_get("view_id"))
                                    .and_then(JsonValue::as_str)
                                    != Some(view_id)
                            {
                                return Err(AccessError::new(
                                    AccessErrorCode::CorruptSelectedCarrier,
                                    "selected philosophy graph-view sample differs",
                                ));
                            }
                            Ok::<(), AccessError>(())
                        });
                        match sample {
                            Ok(()) => {
                                philosophy_view_status = health_subject(
                                    "ready",
                                    None,
                                    None,
                                    philosophy_schema.as_deref(),
                                    Some(view_id),
                                );
                            }
                            Err(error) if health_error_is_global(&error) => return Err(error),
                            Err(error) => {
                                append_health_error(
                                    &mut errors,
                                    format!("philosophy projection invalid: {}", error.message),
                                );
                                philosophy_view_status = health_error_subject(&error);
                            }
                        }
                    }
                }
            }
        }
        Err(error) if health_error_is_global(&error) => return Err(error),
        Err(error) => {
            append_health_error(
                &mut errors,
                format!("philosophy projection invalid: {}", error.message),
            );
            philosophy_projection_status = health_error_subject(&error);
            philosophy_view_status = health_subject(
                "not_checked",
                Some("source_unavailable"),
                Some("philosophy view sample requires a valid selected projection"),
                None,
                None,
            );
        }
    }
    check_health_visit_meter(&visit_meter)?;
    if let Some(original) = original {
        original.check()?;
    }

    let mut knowledge_schema = JsonValue::Null;
    let mut knowledge_counts = JsonValue::Null;
    let mut knowledge_graph_status = health_subject("unavailable", None, None, None, None);
    let mut knowledge_catalog_status = health_subject("unavailable", None, None, None, None);
    let knowledge = child(HealthChild::Knowledge, inspect_budget, &mut visit_meter);
    match knowledge {
        Ok(metadata) => {
            health_child_schema(&metadata, "tos_selected_knowledge_health_metadata_v1")?;
            let graph_schema =
                health_required_text(&metadata, "graph_schema", inspect_budget.max_field_bytes)?;
            let catalog_schema =
                health_required_text(&metadata, "catalog_schema", inspect_budget.max_field_bytes)?;
            let mut metadata_fields = match metadata {
                JsonValue::Object(fields) => fields,
                _ => {
                    return Err(AccessError::new(
                        AccessErrorCode::CorruptSelectedCarrier,
                        "selected knowledge health counts invalid",
                    ));
                }
            };
            let counts_index = metadata_fields
                .iter()
                .position(|(key, _)| key.as_str() == Some("counts"))
                .ok_or_else(|| {
                    AccessError::new(
                        AccessErrorCode::CorruptSelectedCarrier,
                        "selected knowledge health counts invalid",
                    )
                })?;
            let counts = metadata_fields.swap_remove(counts_index).1;
            drop(metadata_fields);
            if counts.as_object().is_none() {
                return Err(AccessError::new(
                    AccessErrorCode::CorruptSelectedCarrier,
                    "selected knowledge health counts invalid",
                ));
            }
            let nodes = counts
                .object_get("nodes")
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| {
                    AccessError::new(
                        AccessErrorCode::CorruptSelectedCarrier,
                        "selected knowledge health node count invalid",
                    )
                })?;
            let relations = counts
                .object_get("relations")
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| {
                    AccessError::new(
                        AccessErrorCode::CorruptSelectedCarrier,
                        "selected knowledge health relation count invalid",
                    )
                })?;
            let coverage = counts
                .object_get("display_coverage")
                .filter(|coverage| coverage.as_object().is_some())
                .ok_or_else(|| {
                    AccessError::new(
                        AccessErrorCode::CorruptSelectedCarrier,
                        "selected knowledge health display coverage invalid",
                    )
                })?;
            let mut graph_errors = false;
            for (field, expected, message) in [
                (
                    "node_titles",
                    nodes,
                    "knowledge graph node title coverage is incomplete",
                ),
                (
                    "node_summaries",
                    nodes,
                    "knowledge graph node summary coverage is incomplete",
                ),
                (
                    "relation_labels",
                    relations,
                    "knowledge graph relation label coverage is incomplete",
                ),
                (
                    "relation_statements",
                    relations,
                    "knowledge graph relation statement coverage is incomplete",
                ),
                (
                    "relation_explanations",
                    relations,
                    "knowledge graph relation explanation coverage is incomplete",
                ),
            ] {
                if coverage.object_get(field).and_then(JsonValue::as_u64) != Some(expected) {
                    graph_errors = true;
                    append_health_error(&mut errors, message.into());
                }
            }
            let semantic_errors = counts
                .object_get("semantic_validation")
                .is_some_and(|report| {
                    report.object_get("valid").and_then(JsonValue::as_bool) == Some(false)
                        || report
                            .object_get("violations")
                            .and_then(JsonValue::as_array)
                            .is_some_and(|violations| !violations.is_empty())
                });
            if semantic_errors {
                append_health_error(
                    &mut errors,
                    "knowledge graph semantic validation reports violations".into(),
                );
            }
            knowledge_schema = JsonValue::String(JsonString::from_utf8(&graph_schema));
            knowledge_counts = counts;
            knowledge_graph_status = health_subject(
                if graph_errors || semantic_errors {
                    "degraded"
                } else {
                    "ready"
                },
                if semantic_errors {
                    Some("semantic_validation_failed")
                } else {
                    graph_errors.then_some("coverage_incomplete")
                },
                if semantic_errors {
                    Some("mechanical semantic validation reports violations")
                } else {
                    graph_errors.then_some("one or more display-coverage counts differ")
                },
                Some(&graph_schema),
                None,
            );
            let catalog_mismatch = catalog_schema != "tos_knowledge_catalog_v1";
            if catalog_mismatch {
                append_health_error(
                    &mut errors,
                    "knowledge catalog schema is not current".into(),
                );
            }
            knowledge_catalog_status = health_subject(
                if catalog_mismatch {
                    "degraded"
                } else {
                    "ready"
                },
                catalog_mismatch.then_some("unsupported_schema"),
                catalog_mismatch.then_some("knowledge catalog schema is not current"),
                Some(&catalog_schema),
                None,
            );
        }
        Err(error) if health_error_is_global(&error) => return Err(error),
        Err(error) => {
            append_health_error(
                &mut errors,
                format!("knowledge graph invalid: {}", error.message),
            );
            knowledge_graph_status = health_error_subject(&error);
            knowledge_catalog_status = health_subject(
                "not_checked",
                Some("source_unavailable"),
                Some("knowledge catalog schema requires valid graph metadata"),
                None,
                None,
            );
        }
    }
    check_health_visit_meter(&visit_meter)?;
    if let Some(original) = original {
        original.check()?;
    }

    crate::knowledge::check_abort(&probe)?;
    let final_response_limit =
        max_response_bytes - inspect_budget.max_response_bytes * HEALTH_CHILDREN;
    if final_response_limit == 0 {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "selected access health response budget unavailable",
        ));
    }
    let ok = errors.is_empty();
    let report = health_object(vec![
        (
            "schema_version",
            JsonValue::String(JsonString::from_utf8("tos_selected_access_health_v1")),
        ),
        (
            "service",
            JsonValue::String(JsonString::from_utf8("tree-of-sophia-access")),
        ),
        ("ok", JsonValue::Bool(ok)),
        ("write_enabled", JsonValue::Bool(false)),
        ("errors", JsonValue::Array(errors)),
        ("knowledge_schema", knowledge_schema),
        ("knowledge_counts", knowledge_counts),
        (
            "subjects",
            health_object(vec![
                ("corpus_index", corpus_index_status),
                ("corpus_graph_view", corpus_view_status),
                ("philosophy_projection", philosophy_projection_status),
                ("philosophy_graph_view", philosophy_view_status),
                ("knowledge_graph", knowledge_graph_status),
                ("knowledge_catalog", knowledge_catalog_status),
            ]),
        ),
    ]);
    let mut output_limits = output_json;
    output_limits.max_bytes = final_response_limit;
    let body = if let Some(original) = original {
        original.check()?;
        original.emit(&report, output_limits, &mut visit_meter)?
    } else {
        visit_meter
            .canonical_bytes(
                &report,
                CanonicalProfile::SourceRecordDigestV1,
                output_limits,
            )
            .map_err(|_| {
                AccessError::new(
                    AccessErrorCode::BudgetExceeded,
                    "selected access health response exceeds budget",
                )
            })?
    };
    if body.len() > final_response_limit {
        return Err(AccessError::new(
            AccessErrorCode::BudgetExceeded,
            "selected access health response exceeds budget",
        ));
    }
    crate::knowledge::check_abort(&probe)?;
    Ok((body, ok))
}

fn execute_selected_access_health(
    owner: &ManagedLocalExecutor,
    probe: Arc<dyn AbortProbe>,
) -> Result<crate::common::PreparedHealth<'static>, AccessError> {
    crate::knowledge::check_abort(&probe)?;
    let inspect_budget = partition_health_inspect_budget(
        owner.budgets().inspect,
        owner.profile.max_response_bytes,
        owner.cold.max_work_bytes,
        owner.cold.max_rows,
        owner.cold.max_vm_steps,
    )?;
    let work_steps = owner.cold.max_vm_steps / (HEALTH_CHILDREN as u64 + 1);
    let mut lease = owner.release.acquire()?;
    if owner.corpus_original.is_some() {
        lease.retain_member_guards(&owner.corpus_guards)?;
    }
    let shared_lease = Arc::new(Mutex::new(lease));
    let cold = owner
        .model
        .lock()
        .map_err(|_| unavailable("selected native model lock poisoned"))?;
    let mut model = cold
        .fork_reader_with_vm_budget(owner.cold.max_vm_steps)
        .map_err(|_| unavailable("selected native reader unavailable"))?;
    drop(cold);
    let bound = tos_query::bind_verified_knowledge(&model, &owner.vocabulary, &owner.descriptor)?;

    let (body, ok) = compose_selected_access_health(
        inspect_budget,
        owner.profile.max_response_bytes,
        owner.budgets().inspect.json,
        &probe,
        |child, inspect_budget, visit_meter| match child {
            HealthChild::CorpusSeed => {
                let context = owner
                    .corpus_context
                    .as_ref()
                    .ok_or_else(|| unavailable("selected corpus source context unavailable"))?;
                let mut authority = Authority::new_shared(
                    owner,
                    &bound,
                    O::CorpusStatus.id(),
                    tos_query::corpus_read::CORPUS_INTENDED_USE,
                    Arc::clone(&shared_lease),
                    Arc::clone(&probe),
                )?;
                let packet = tos_query::corpus_read::execute_selected_corpus_health_seed_metered(
                    &mut model,
                    &bound,
                    &mut authority,
                    context,
                    tos_query::corpus_read::CorpusReadBudget {
                        inspect: inspect_budget,
                        max_work_steps: work_steps,
                    },
                    visit_meter,
                )?;
                consume_inspect_health_packet(packet, &probe, inspect_budget.json, visit_meter)
            }
            HealthChild::CorpusView(view_id) => {
                let context = owner
                    .corpus_context
                    .as_ref()
                    .ok_or_else(|| unavailable("selected corpus source context unavailable"))?;
                let mut authority = Authority::new_shared(
                    owner,
                    &bound,
                    O::CorpusGraphView.id(),
                    tos_query::corpus_read::CORPUS_INTENDED_USE,
                    Arc::clone(&shared_lease),
                    Arc::clone(&probe),
                )?;
                let request = tos_query::corpus_read::CorpusReadRequest::GraphView {
                    view_id: view_id.to_owned(),
                    limit: 1,
                };
                let packet = tos_query::corpus_read::execute_selected_corpus_metered(
                    &mut model,
                    &bound,
                    &mut authority,
                    context,
                    &request,
                    tos_query::corpus_read::CorpusReadBudget {
                        inspect: inspect_budget,
                        max_work_steps: work_steps,
                    },
                    visit_meter,
                )?;
                let value = consume_inspect_health_packet(
                    packet,
                    &probe,
                    inspect_budget.json,
                    visit_meter,
                )?;
                Ok(value)
            }
            HealthChild::PhilosophySeed => {
                let mut authority = Authority::new_shared(
                    owner,
                    &bound,
                    O::PhilosophyViews.id(),
                    tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE,
                    Arc::clone(&shared_lease),
                    Arc::clone(&probe),
                )?;
                let packet =
                    tos_query::philosophy_read::execute_selected_philosophy_health_seed_metered(
                        &mut model,
                        &bound,
                        &mut authority,
                        tos_query::philosophy_read::PhilosophyReadBudget {
                            inspect: inspect_budget,
                            max_work_steps: work_steps,
                        },
                        visit_meter,
                    )?;
                consume_inspect_health_packet(packet, &probe, inspect_budget.json, visit_meter)
            }
            HealthChild::PhilosophyView(view_id) => {
                let mut authority = Authority::new_shared(
                    owner,
                    &bound,
                    O::PhilosophyView.id(),
                    tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE,
                    Arc::clone(&shared_lease),
                    Arc::clone(&probe),
                )?;
                let request = tos_query::philosophy_read::PhilosophyReadRequest::View {
                    view_id: view_id.to_owned(),
                    limit: 1,
                };
                let packet = tos_query::philosophy_read::execute_selected_philosophy_metered(
                    &mut model,
                    &bound,
                    &mut authority,
                    &request,
                    tos_query::philosophy_read::PhilosophyReadBudget {
                        inspect: inspect_budget,
                        max_work_steps: work_steps,
                    },
                    visit_meter,
                )?;
                let value = consume_inspect_health_packet(
                    packet,
                    &probe,
                    inspect_budget.json,
                    visit_meter,
                )?;
                Ok(value)
            }
            HealthChild::Knowledge => {
                let mut authority = Authority::new_shared(
                    owner,
                    &bound,
                    O::Catalog.id(),
                    tos_query::CATALOG_INTENDED_USE,
                    Arc::clone(&shared_lease),
                    Arc::clone(&probe),
                )?;
                let packet = tos_query::execute_selected_knowledge_health_metadata(
                    &mut model,
                    &bound,
                    &mut authority,
                    tos_query::CatalogBudget {
                        max_open_vm_steps: owner.cold.max_vm_steps,
                        max_read_vm_steps: inspect_budget.max_read_vm_steps,
                        max_packet_bytes: inspect_budget.max_response_bytes,
                        max_decoded_bytes: usize::try_from(inspect_budget.max_decoded_bytes)
                            .unwrap_or(usize::MAX),
                        json: inspect_budget.json,
                    },
                    visit_meter,
                )?;
                consume_catalog_health_packet(packet, &probe, inspect_budget.json, visit_meter)
            }
        },
    )?;
    shared_lease
        .lock()
        .map_err(|_| {
            AccessError::new(
                AccessErrorCode::StaleSelection,
                "selected native release hold poisoned",
            )
        })?
        .recheck()?;
    crate::knowledge::check_abort(&probe)?;
    Ok(crate::common::PreparedHealth {
        ok,
        packet: PreparedPacket {
            body,
            fence: Box::new(AccessHealthFence {
                lease: shared_lease,
                probe,
            }),
        },
    })
}

struct Authority {
    policy: CurrentPolicyBinding,
    inspect: IndexedDisclosureScope,
    catalog: CatalogDisclosureScope,
    lease: Option<ReleaseLease>,
    shared_lease: Option<Arc<Mutex<ReleaseLease>>>,
    probe: Option<Arc<dyn AbortProbe>>,
    registries: [(String, Digest256); 2],
    registry_grants: u8,
    original: Option<NavigationOriginalReceipt>,
    original_granted: bool,
    philosophy_original: Option<PhilosophyOriginalReceipt>,
    philosophy_granted: bool,
    corpus_original: Option<tos_compiler::CorpusOriginalReceipt>,
    corpus_granted: bool,
}
impl Authority {
    fn new(
        owner: &ManagedLocalExecutor,
        bound: &BoundCmpKnowledge<'_>,
        operation: &str,
        intended: &str,
    ) -> Result<Self, AccessError> {
        let mut lease = owner.release.acquire()?;
        if owner.corpus_original.is_some() {
            lease.retain_member_guards(&owner.corpus_guards)?;
        }
        Self::scoped(owner, bound, operation, intended, Some(lease), None, None)
    }
    fn new_shared(
        owner: &ManagedLocalExecutor,
        bound: &BoundCmpKnowledge<'_>,
        operation: &str,
        intended: &str,
        lease: Arc<Mutex<ReleaseLease>>,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<Self, AccessError> {
        Self::scoped(
            owner,
            bound,
            operation,
            intended,
            None,
            Some(lease),
            Some(probe),
        )
    }
    fn scoped(
        owner: &ManagedLocalExecutor,
        bound: &BoundCmpKnowledge<'_>,
        operation: &str,
        intended: &str,
        lease: Option<ReleaseLease>,
        shared_lease: Option<Arc<Mutex<ReleaseLease>>>,
        probe: Option<Arc<dyn AbortProbe>>,
    ) -> Result<Self, AccessError> {
        if lease.is_some() == shared_lease.is_some() {
            return Err(unavailable("selected release hold binding invalid"));
        }
        // These fields identify the existing local holder and selected release;
        // they are not a source-rights receipt or a public issuer credential.
        let policy = CurrentPolicyBinding {
            scope: "managed-local-admitted-projection-v1".into(),
            issuer_ref: format!(
                "managed-local-release-holder:{}",
                Digest256::of_bytes(owner.release.root_path().as_os_str().as_bytes()).to_hex()
            ),
            authorization_receipt_id: owner.release.pair_id().into(),
            policy_epoch: owner.release.pair_id().into(),
            withdrawal_generation: owner.release.pair_id().into(),
        };
        let selected = bound.selection();
        let inspect = IndexedDisclosureScope {
            operation_id: operation.into(),
            carrier_layer: tos_query::CATALOG_CARRIER_LAYER.into(),
            intended_use: intended.into(),
            selected_model_receipt_id: bound.owner_receipt_id().into(),
            source_cut: selected.source_cut.clone(),
            through_commit_seq: selected.through_commit_seq,
            source_membership_root: selected.source_membership_root,
            descriptor_sha256: selected.vocabulary.descriptor_sha256,
            selected_index_sha256: selected.index_root_sha256,
            policy_issuer_ref: policy.issuer_ref.clone(),
            policy_receipt_id: policy.authorization_receipt_id.clone(),
            policy_scope: policy.scope.clone(),
            policy_epoch: policy.policy_epoch.clone(),
            withdrawal_generation: policy.withdrawal_generation.clone(),
        };
        let catalog = CatalogDisclosureScope {
            operation_id: O::Catalog.id().into(),
            carrier_layer: inspect.carrier_layer.clone(),
            intended_use: tos_query::CATALOG_INTENDED_USE.into(),
            selected_model_receipt_id: inspect.selected_model_receipt_id.clone(),
            source_cut: inspect.source_cut.clone(),
            through_commit_seq: inspect.through_commit_seq,
            source_membership_root: inspect.source_membership_root,
            descriptor_sha256: inspect.descriptor_sha256,
            selected_index_sha256: inspect.selected_index_sha256,
            catalog_packet_sha256: selected.catalog_packet_sha256,
            policy_issuer_ref: policy.issuer_ref.clone(),
            policy_receipt_id: policy.authorization_receipt_id.clone(),
            policy_scope: policy.scope.clone(),
            policy_epoch: policy.policy_epoch.clone(),
            withdrawal_generation: policy.withdrawal_generation.clone(),
        };
        Ok(Self {
            policy,
            inspect,
            catalog,
            lease,
            shared_lease,
            probe,
            registries: [
                (
                    selected.entity_registry_id.clone(),
                    selected.entity_registry_sha256,
                ),
                (
                    selected.relation_registry_id.clone(),
                    selected.relation_registry_sha256,
                ),
            ],
            registry_grants: 0,
            original: owner.original.clone(),
            original_granted: false,
            philosophy_original: owner.philosophy_original.clone(),
            philosophy_granted: false,
            corpus_original: owner.corpus_original.clone(),
            corpus_granted: false,
        })
    }
    fn check(&mut self) -> Result<(), SearchV2Error> {
        if let Some(shared) = &self.shared_lease {
            return shared
                .lock()
                .map_err(|_| query_error("selected native release hold poisoned"))?
                .check_hold()
                .map_err(|_| query_error("selected local release changed or revoked"));
        }
        self.lease
            .as_mut()
            .ok_or_else(|| query_error("release disclosure hold consumed"))?
            .check_hold()
            .map_err(|_| query_error("selected local release changed or revoked"))
    }
    fn take(&mut self) -> Result<AuthorityDisclosureLease, SearchV2Error> {
        self.check()?;
        if let Some(shared) = &self.shared_lease {
            return Ok(AuthorityDisclosureLease::Shared(Arc::clone(shared)));
        }
        self.lease
            .take()
            .map(AuthorityDisclosureLease::Owned)
            .ok_or_else(|| query_error("release disclosure hold consumed"))
    }
}

enum AuthorityDisclosureLease {
    Owned(ReleaseLease),
    Shared(Arc<Mutex<ReleaseLease>>),
}
impl InspectDisclosureLease for AuthorityDisclosureLease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        match self {
            Self::Owned(lease) => InspectDisclosureLease::recheck(lease),
            Self::Shared(shared) => InspectDisclosureLease::recheck(
                &mut *shared
                    .lock()
                    .map_err(|_| query_error("selected native release hold poisoned"))?,
            ),
        }
    }
}
impl IndexedDisclosureLease for AuthorityDisclosureLease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        match self {
            Self::Owned(lease) => IndexedDisclosureLease::recheck(lease),
            Self::Shared(shared) => IndexedDisclosureLease::recheck(
                &mut *shared
                    .lock()
                    .map_err(|_| query_error("selected native release hold poisoned"))?,
            ),
        }
    }
}
impl CatalogDisclosureLease for AuthorityDisclosureLease {
    fn recheck(&mut self) -> Result<(), CatalogError> {
        match self {
            Self::Owned(lease) => CatalogDisclosureLease::recheck(lease),
            Self::Shared(shared) => CatalogDisclosureLease::recheck(
                &mut *shared
                    .lock()
                    .map_err(|_| catalog_error("selected native release hold poisoned"))?,
            ),
        }
    }
}
impl InspectDisclosureLease for ReleaseLease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        ReleaseLease::recheck(self)
            .map_err(|_| query_error("selected local release changed or revoked"))
    }
}
impl IndexedDisclosureLease for ReleaseLease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        ReleaseLease::recheck(self)
            .map_err(|_| query_error("selected local release changed or revoked"))
    }
}
impl CatalogDisclosureLease for ReleaseLease {
    fn recheck(&mut self) -> Result<(), CatalogError> {
        ReleaseLease::recheck(self)
            .map_err(|_| catalog_error("selected local release changed or revoked"))
    }
}
struct IndexedAuthority {
    authority: Authority,
    probe: Arc<dyn AbortProbe>,
}
impl IndexedAuthority {
    fn check(&mut self) -> Result<(), SearchV2Error> {
        if let Some(reason) = self.probe.reason() {
            return Err(SearchV2Error {
                code: match reason {
                    AbortReason::Cancelled => SearchV2ErrorCode::Cancelled,
                    AbortReason::DeadlineExceeded => SearchV2ErrorCode::DeadlineExceeded,
                },
                message: "indexed search interrupted",
            });
        }
        self.authority.check()
    }
}
impl IndexedKnowledgeAuthority for IndexedAuthority {
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.authority.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.authority.inspect.clone()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        self.check()
    }
    fn authorize_current(
        &mut self,
        _: &tos_query::search_candidate::SelectedSearchCandidate,
    ) -> Result<(), SearchV2Error> {
        // The held release admits this whole public projection, including
        // consulted filtered and gram-false-positive selected carriers.
        self.check()
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        _: &[ObservedSearchCandidate],
    ) -> Result<Box<dyn IndexedDisclosureLease>, SearchV2Error> {
        if scope != &self.authority.inspect {
            return Err(query_error("selected indexed disclosure scope changed"));
        }
        self.check()?;
        Ok(Box::new(self.authority.take()?))
    }
}
impl<'hold> InspectCurrentAuthority<'hold> for Authority {
    fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
        self.probe.clone()
    }
    fn authorize_corpus_view_identity_current(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        ordinal: u64,
        _view_id: Option<&str>,
        _sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check()?;
        let expected = self
            .corpus_original
            .as_ref()
            .ok_or_else(|| query_error("selected corpus original unavailable"))?;
        let valid = expected.collections.iter().any(|c| {
            c.collection == tos_compiler::CorpusOriginalCollection::GraphViews.as_str()
                && ordinal < c.rows
        });
        // QRY's cold-verified index binds ID/SHA; this callback grants only its
        // exact retained component under the current managed release hold.
        if self.inspect.operation_id != O::CorpusSummary.id()
            || self.inspect.intended_use != tos_query::corpus_read::CORPUS_INTENDED_USE
            || !same_corpus_receipt(receipt, expected)
            || !valid
        {
            return Err(query_error("selected corpus view identity scope changed"));
        }
        self.corpus_granted = true;
        Ok(())
    }
    fn authorize_corpus_original_current(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        collection: tos_compiler::CorpusOriginalCollection,
        ordinal: u64,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check()?;
        let expected = self
            .corpus_original
            .as_ref()
            .ok_or_else(|| query_error("selected corpus original unavailable"))?;
        let ordinal_valid = match collection {
            tos_compiler::CorpusOriginalCollection::Header => {
                ordinal == 0 && sha.to_hex() == expected.header_sha256
            }
            _ => expected
                .collections
                .iter()
                .any(|c| c.collection == collection.as_str() && ordinal < c.rows),
        };
        if self.inspect.intended_use != tos_query::corpus_read::CORPUS_INTENDED_USE
            || !same_corpus_receipt(receipt, expected)
            || !ordinal_valid
            || Digest256::of_bytes(raw) != sha
        {
            return Err(query_error("selected corpus original scope changed"));
        }
        self.corpus_granted = true;
        Ok(())
    }

    fn authorize_philosophy_original_current(
        &mut self,
        receipt: &PhilosophyOriginalReceipt,
        collection: PhilosophyOriginalCollection,
        ordinal: u64,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check()?;
        let expected = self
            .philosophy_original
            .as_ref()
            .ok_or_else(|| query_error("selected philosophy original unavailable"))?;
        let ordinal_valid = match collection {
            PhilosophyOriginalCollection::Header => {
                ordinal == 0 && sha.to_hex() == expected.header_sha256
            }
            PhilosophyOriginalCollection::Nodes => ordinal < expected.nodes,
            PhilosophyOriginalCollection::Edges => ordinal < expected.edges,
        };
        if self.inspect.intended_use != tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE
            || receipt.profile != expected.profile
            || receipt.descriptor_sha256 != expected.descriptor_sha256
            || receipt.source_cut != expected.source_cut
            || receipt.membership_root != expected.membership_root
            || receipt.source_graph != expected.source_graph
            || receipt.nodes != expected.nodes
            || receipt.edges != expected.edges
            || receipt.node_input_root_sha256 != expected.node_input_root_sha256
            || receipt.edge_input_root_sha256 != expected.edge_input_root_sha256
            || receipt.header_sha256 != expected.header_sha256
            || receipt.nodes_root_sha256 != expected.nodes_root_sha256
            || receipt.edges_root_sha256 != expected.edges_root_sha256
            || receipt.component_root_sha256 != expected.component_root_sha256
            || receipt.total_bytes != expected.total_bytes
            || !ordinal_valid
            || Digest256::of_bytes(raw) != sha
        {
            return Err(query_error("selected philosophy original scope changed"));
        }
        self.philosophy_granted = true;
        Ok(())
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.inspect.clone()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        self.check()
    }
    fn authorize_current(&mut self, _: &InspectedCarrier) -> Result<(), SearchV2Error> {
        self.check()
    }
    fn authorize_catalog_current(&mut self, sha: Digest256) -> Result<(), SearchV2Error> {
        self.check()?;
        if self.inspect.operation_id != O::StoredLens.id()
            || sha != self.catalog.catalog_packet_sha256
        {
            return Err(query_error("selected catalog scope changed"));
        }
        Ok(())
    }
    fn authorize_registry_current(
        &mut self,
        id: &str,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check()?;
        if self.inspect.operation_id != O::Contracts.id() || Digest256::of_bytes(raw) != sha {
            return Err(query_error("selected registry scope changed"));
        }
        let at = self
            .registries
            .iter()
            .position(|(selected_id, selected_sha)| selected_id == id && *selected_sha == sha)
            .ok_or_else(|| query_error("selected registry binding changed"))?;
        self.registry_grants |= 1 << at;
        Ok(())
    }
    fn authorize_navigation_original_current(
        &mut self,
        receipt: &NavigationOriginalReceipt,
        ordinal: i64,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        self.check()?;
        let expected = self
            .original
            .as_ref()
            .ok_or_else(|| query_error("selected original component unavailable"))?;
        if self.inspect.operation_id != O::Dossier.id()
            || receipt.component_root_sha256 != expected.component_root_sha256
            || receipt.member_index_root_sha256 != expected.member_index_root_sha256
            || receipt.header_sha256 != expected.header_sha256
            || receipt.rights_root_sha256 != expected.rights_root_sha256
            || receipt.descriptor_sha256 != expected.descriptor_sha256
            || receipt.profile != expected.profile
            || receipt.source_graph != expected.source_graph
            || receipt.nodes != expected.nodes
            || receipt.edges != expected.edges
            || receipt.rights != expected.rights
            || receipt.node_input_root_sha256 != expected.node_input_root_sha256
            || receipt.edge_input_root_sha256 != expected.edge_input_root_sha256
            || receipt.total_bytes != expected.total_bytes
            || receipt.member_index_bytes != expected.member_index_bytes
            || receipt.source_cut != expected.source_cut
            || receipt.membership_root != expected.membership_root
            || Digest256::of_bytes(raw) != sha
            || ordinal < -1
            || (ordinal >= 0 && ordinal as u64 >= expected.rights)
        {
            return Err(query_error("selected dossier original scope changed"));
        }
        self.original_granted = true;
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        _: &[ObservedInspectCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
        if scope != &self.inspect
            || (scope.operation_id == O::Contracts.id() && self.registry_grants != 3)
            || (scope.operation_id == O::Dossier.id() && !self.original_granted)
            || (scope.intended_use == tos_query::corpus_read::CORPUS_INTENDED_USE
                && !self.corpus_granted)
            || (scope.intended_use == tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE
                && !self.philosophy_granted)
        {
            return Err(query_error("selected local disclosure scope incomplete"));
        }
        Ok(Box::new(self.take()?))
    }
}
impl<'hold> CatalogCurrentAuthority<'hold> for Authority {
    fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
        self.probe.clone()
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> CatalogDisclosureScope {
        self.catalog.clone()
    }
    fn check_selected(&mut self) -> Result<(), CatalogError> {
        self.check()
            .map_err(|_| catalog_error("selected local release changed or revoked"))
    }
    fn authorize_current(&mut self, sha: Digest256) -> Result<(), CatalogError> {
        CatalogCurrentAuthority::check_selected(self)?;
        if sha != self.catalog.catalog_packet_sha256 {
            return Err(catalog_error("selected catalog binding changed"));
        }
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &CatalogDisclosureScope,
        sha: Digest256,
    ) -> Result<Box<dyn CatalogDisclosureLease + 'hold>, CatalogError> {
        if scope != &self.catalog || sha != self.catalog.catalog_packet_sha256 {
            return Err(catalog_error("selected catalog binding changed"));
        }
        Ok(Box::new(self.take().map_err(|_| {
            catalog_error("selected local disclosure hold unavailable")
        })?))
    }
}

struct AccessHealthFence {
    lease: Arc<Mutex<ReleaseLease>>,
    probe: Arc<dyn AbortProbe>,
}
impl crate::DisclosureFence for AccessHealthFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        self.lease
            .lock()
            .map_err(|_| {
                AccessError::new(
                    AccessErrorCode::StaleSelection,
                    "selected native release hold poisoned",
                )
            })?
            .recheck()?;
        crate::knowledge::check_abort(&self.probe)
    }
}

/// Whole published-ledger composition uses the existing release holder only.
/// It does not manufacture native graph/source CurrentPolicy authority.
pub fn execute_selected_source_gap(
    release: &Arc<ManagedRelease>,
    request: &tos_query::source_gap::SourceGapRequest,
    budget: tos_query::source_gap::SourceGapBudget,
    max_input_bytes: usize,
    probe: Arc<dyn AbortProbe>,
) -> Result<PreparedPacket<'static>, AccessError> {
    crate::knowledge::check_abort(&probe)?;
    let mut hold = release.acquire()?;
    let owned = hold.public_source_gap_records(max_input_bytes)?;
    let records = owned
        .iter()
        .map(|(path, raw)| tos_query::source_gap::PublicSourceGapRecord {
            source_ref: path,
            raw,
        })
        .collect::<Vec<_>>();
    let body = tos_query::source_gap::compute_source_gap_packet(
        &records,
        request,
        budget,
        probe.as_ref(),
    )?;
    hold.recheck()?;
    crate::knowledge::check_abort(&probe)?;
    Ok(PreparedPacket {
        body,
        fence: Box::new(PublicLedgerFence { hold, probe }),
    })
}
struct PublicLedgerFence {
    hold: ReleaseLease,
    probe: Arc<dyn AbortProbe>,
}
impl crate::DisclosureFence for PublicLedgerFence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        self.hold.recheck()?;
        crate::knowledge::check_abort(&self.probe)
    }
}

fn same_corpus_receipt(
    a: &tos_compiler::CorpusOriginalReceipt,
    b: &tos_compiler::CorpusOriginalReceipt,
) -> bool {
    a.profile == b.profile
        && a.descriptor_sha256 == b.descriptor_sha256
        && a.source_cut == b.source_cut
        && a.membership_root == b.membership_root
        && a.header_sha256 == b.header_sha256
        && a.component_root_sha256 == b.component_root_sha256
        && a.total_bytes == b.total_bytes
        && a.collections.len() == b.collections.len()
        && a.collections.iter().zip(&b.collections).all(|(a, b)| {
            a.collection == b.collection
                && a.rows == b.rows
                && a.ordered_root_sha256 == b.ordered_root_sha256
        })
        && a.origin.profile == b.origin.profile
        && a.origin.source_git_commit == b.origin.source_git_commit
        && a.origin.source_git_tree == b.origin.source_git_tree
        && a.origin.capture_manifest_sha256 == b.origin.capture_manifest_sha256
        && a.origin.native_producer.as_ref() == b.origin.native_producer.as_ref()
        && a.origin.source_path == b.origin.source_path
        && a.origin.source_sha256 == b.origin.source_sha256
        && a.origin.source_size_bytes == b.origin.source_size_bytes
        && a.origin.member_root_sha256 == b.origin.member_root_sha256
        && a.origin.members.len() == b.origin.members.len()
        && a.origin
            .members
            .iter()
            .zip(&b.origin.members)
            .all(|(a, b)| a.path == b.path && a.size_bytes == b.size_bytes && a.sha256 == b.sha256)
}
