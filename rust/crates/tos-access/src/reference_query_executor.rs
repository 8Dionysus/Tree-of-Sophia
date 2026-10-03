//! Callback-local HTTP executor over one exact verified source-root callback.
use crate::knowledge::{KnowledgeOperation, KnowledgeRequest, SelectedKnowledgeBudgets};
use crate::reference_root_query::{ReferenceMetadataContext, ReferenceMetadataOperation};
use crate::{AccessError, AccessErrorCode, PreparedPacket, ScopedAccessExecutor};
use std::cell::RefCell;
use std::sync::Arc;
use tos_compiler::VerifiedKnowledgeModel;
use tos_query::{AbortProbe, BoundCmpKnowledge, CatalogCurrentAuthority, InspectCurrentAuthority};

/// A request context may come from Reference's local callback or from another
/// owner-provided current-authority callback. The context lends the exact
/// authorities to one query operation; it does not construct a policy binding.
pub trait SelectedQueryContext<'hold> {
    fn validate_inputs(
        &self,
        bound: &BoundCmpKnowledge<'_>,
        view: &tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>,
        evidence: Option<&tos_compiler::native_snapshot::CompletedEvidenceProjectionView<'_>>,
    ) -> Result<(), AccessError>;

    fn prepare_operation<T>(
        &mut self,
        operation: ReferenceMetadataOperation,
        probe: Arc<dyn AbortProbe>,
        consume: impl FnOnce(
            &mut dyn CatalogCurrentAuthority<'hold>,
            &mut dyn InspectCurrentAuthority<'hold>,
        ) -> Result<T, AccessError>,
    ) -> Result<T, AccessError>;
}

impl<'hold, 'capture> SelectedQueryContext<'hold> for ReferenceMetadataContext<'hold, 'capture> {
    fn validate_inputs(
        &self,
        bound: &BoundCmpKnowledge<'_>,
        view: &tos_compiler::native_snapshot::CompletedCaptureCarriers<'_>,
        evidence: Option<&tos_compiler::native_snapshot::CompletedEvidenceProjectionView<'_>>,
    ) -> Result<(), AccessError> {
        let revision = bound.require_source_revision().map_err(|_| {
            AccessError::new(
                AccessErrorCode::Unavailable,
                "selected Reference source revision unavailable",
            )
        })?;
        if !self.uses_capture_view(view)
            || view.source_revision() != Some(revision)
            || evidence.is_some_and(|evidence| evidence.source_revision() != revision)
        {
            return Err(AccessError::new(
                AccessErrorCode::StaleSelection,
                "selected Reference callback inputs are from different source revisions",
            ));
        }
        Ok(())
    }

    fn prepare_operation<T>(
        &mut self,
        operation: ReferenceMetadataOperation,
        probe: Arc<dyn AbortProbe>,
        consume: impl FnOnce(
            &mut dyn CatalogCurrentAuthority<'hold>,
            &mut dyn InspectCurrentAuthority<'hold>,
        ) -> Result<T, AccessError>,
    ) -> Result<T, AccessError> {
        ReferenceMetadataContext::prepare_operation(self, operation, probe, |catalog, inspect| {
            consume(catalog, inspect)
        })
    }
}

/// The native owner supplies all limits and the same retained checkpoint store.
/// No field constructs a model, capture, authority, clock, or budget admission.
pub struct ReferenceQueryExecutor<'owner, 'model, 'bound, 'view, 'capture, 'evidence, C> {
    model: RefCell<&'owner mut VerifiedKnowledgeModel<'model>>,
    bound: &'owner BoundCmpKnowledge<'bound>,
    context: RefCell<&'owner mut C>,
    checkpoints: RefCell<&'owner mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints>,
    budgets: SelectedKnowledgeBudgets,
    legacy: tos_query::knowledge_legacy_search::LegacySearchBudget,
    indexed: tos_query::IndexedPageBudget,
    view: &'view tos_compiler::native_snapshot::CompletedCaptureCarriers<'capture>,
    contracts: tos_query::knowledge_contracts::KnowledgeContractBudget,
    evidence:
        Option<&'owner tos_compiler::native_snapshot::CompletedEvidenceProjectionView<'evidence>>,
    corpus: Option<&'owner tos_query::corpus_read::CorpusReadContext>,
    navigation_original_available: bool,
    source_navigation_descend_available: bool,
    philosophy_original_available: bool,
    corpus_original_available: bool,
    philosophy_audit_available: bool,
}
impl<'owner, 'model, 'bound, 'view, 'capture, 'evidence, C>
    ReferenceQueryExecutor<'owner, 'model, 'bound, 'view, 'capture, 'evidence, C>
where
    C: SelectedQueryContext<'view>,
{
    /// Call inside the owner callback which supplies `context`, `view`, and any
    /// Evidence view. The socket must finish flushing before those callbacks
    /// return. RefCell guards serialize the existing mutable QRY session.
    pub fn new(
        model: &'owner mut VerifiedKnowledgeModel<'model>,
        bound: &'owner BoundCmpKnowledge<'bound>,
        context: &'owner mut C,
        checkpoints: &'owner mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints,
        budgets: SelectedKnowledgeBudgets,
        legacy: tos_query::knowledge_legacy_search::LegacySearchBudget,
        indexed: tos_query::IndexedPageBudget,
        view: &'view tos_compiler::native_snapshot::CompletedCaptureCarriers<'capture>,
        contracts: tos_query::knowledge_contracts::KnowledgeContractBudget,
        evidence: Option<
            &'owner tos_compiler::native_snapshot::CompletedEvidenceProjectionView<'evidence>,
        >,
        corpus: Option<&'owner tos_query::corpus_read::CorpusReadContext>,
    ) -> Result<Self, AccessError> {
        context.validate_inputs(bound, view, evidence)?;
        let navigation_original_available = model.navigation_original_available();
        let source_navigation_descend_available =
            tos_query::source_dossier::selected_source_navigation_descend_available(model, bound);
        let philosophy_original_available = model.philosophy_original_available();
        let corpus_original_available = model.corpus_original_available();
        let philosophy_audit_available = view.philosophy_audit_path().is_ok();
        Ok(Self {
            model: RefCell::new(model),
            bound,
            context: RefCell::new(context),
            checkpoints: RefCell::new(checkpoints),
            budgets,
            legacy,
            indexed,
            view,
            contracts,
            evidence,
            corpus,
            navigation_original_available,
            source_navigation_descend_available,
            philosophy_original_available,
            corpus_original_available,
            philosophy_audit_available,
        })
    }
}
fn busy() -> AccessError {
    AccessError::new(
        AccessErrorCode::Unavailable,
        "selected Reference query already borrowed",
    )
}
fn operation(request: &KnowledgeRequest) -> Option<ReferenceMetadataOperation> {
    use KnowledgeRequest as R;
    use ReferenceMetadataOperation as O;
    Some(match request {
        R::Catalog => O::Catalog,
        R::Contracts => O::Contracts,
        R::Node { .. } => O::Node,
        R::Relation { .. } => O::Relation,
        R::Temporal(_) => O::Temporal,
        R::Lens(_) => O::Lens,
        R::Explore(_) => O::Exploration,
        R::Focus(_) => O::Focus,
        R::StoredLens { .. } => O::StoredLens,
        R::Dossier { .. } => O::Dossier,
        R::SearchCapabilities => O::SearchCapabilities,
        R::Philosophy(request) => O::for_philosophy(request),
        R::Corpus(request) => O::for_corpus(request),
        R::CorpusViewIds => O::for_corpus(&tos_query::corpus_read::CorpusReadRequest::Summary),
        R::PhilosophyViewIds => {
            O::for_philosophy(&tos_query::philosophy_read::PhilosophyReadRequest::Views)
        }
        // Exact original inputs have a separate held owner integration.
        R::PhilosophyAudit => O::PhilosophyAudit,
        R::EvidenceLens(request) => O::for_evidence(request),
        R::ExplorationContracts => return None,
    })
}
impl<
    'delivery,
    'owner: 'delivery,
    'model,
    'bound,
    'view: 'delivery,
    'capture,
    'evidence: 'delivery,
    C,
> ScopedAccessExecutor<'delivery>
    for ReferenceQueryExecutor<'owner, 'model, 'bound, 'view, 'capture, 'evidence, C>
where
    C: SelectedQueryContext<'delivery>,
{
    fn source_descend_available(&self) -> bool {
        self.source_navigation_descend_available
    }
    fn source_descend(
        &self,
        request: crate::Params,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'delivery>, AccessError> {
        let request = tos_query::SourceDescendRequest {
            node_id: request.node_id,
            max_depth: request.max_depth,
            limit: request.limit,
            at_least_commit_seq: None,
        };
        let mut model = self.model.try_borrow_mut().map_err(|_| busy())?;
        let mut context = self.context.try_borrow_mut().map_err(|_| busy())?;
        context.prepare_operation(
            ReferenceMetadataOperation::SourceDescend,
            Arc::clone(&probe),
            |_, inspect| {
                let packet = tos_query::source_dossier::execute_selected_source_navigation_descend(
                    &mut model,
                    self.bound,
                    inspect,
                    &request,
                    tos_query::source_dossier::DossierBudget {
                        inspect: self.budgets.inspect,
                        max_candidates: self.legacy.max_candidates,
                        max_work_steps: self.budgets.inspect.max_read_vm_steps,
                        block_size: self.legacy.block_size,
                    },
                    self.legacy.max_retained_bytes,
                )?;
                Ok(crate::knowledge::from_inspect(packet))
            },
        )
    }
    fn knowledge_search_legacy_available(&self) -> bool {
        true
    }
    fn knowledge_search_legacy(
        &self,
        request: tos_query::knowledge_legacy_search::LegacySearchRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'delivery>, AccessError> {
        let mut model = self.model.try_borrow_mut().map_err(|_| busy())?;
        let mut context = self.context.try_borrow_mut().map_err(|_| busy())?;
        context.prepare_operation(
            ReferenceMetadataOperation::LegacySearch,
            Arc::clone(&probe),
            |_, inspect| {
                crate::knowledge::execute_selected_legacy_search(
                    &mut model,
                    self.bound,
                    inspect,
                    &request,
                    self.legacy,
                    probe,
                )
            },
        )
    }
    fn knowledge_search_indexed_available(&self) -> bool {
        true
    }
    fn knowledge_search_indexed(
        &self,
        request: crate::IndexedSearchParams,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'delivery>, AccessError> {
        crate::knowledge::check_abort(&probe)?;
        let mut model = self.model.try_borrow_mut().map_err(|_| busy())?;
        let mut context = self.context.try_borrow_mut().map_err(|_| busy())?;
        context.prepare_operation(
            ReferenceMetadataOperation::IndexedSearch,
            Arc::clone(&probe),
            |_, authority| {
                let selected = tos_query::IndexedSearchV2Request {
                    query: request.query,
                    sources: request.sources,
                    kind_ids: request.kind_ids,
                    predicate_ids: request.predicate_ids,
                    limit: request.limit,
                };
                let normalized = selected
                    .clone()
                    .normalize(self.bound.selection(), self.bound)?;
                let initial = tos_query::SearchContinuationState::new(
                    self.bound.selection().clone(),
                    normalized,
                    authority.policy_binding().clone(),
                    self.bound,
                )?;
                let mut codec = crate::indexed_cursor::NativeIndexedCursorCodec::new(
                    initial,
                    self.bound.owner_receipt_id(),
                );
                let packet = tos_query::execute_indexed_search_page(
                    &mut model,
                    self.bound,
                    authority,
                    &mut codec,
                    selected,
                    request.cursor.as_deref(),
                    self.indexed,
                )?;
                crate::knowledge::check_abort(&probe)?;
                Ok(crate::knowledge::from_indexed_search(packet))
            },
        )
    }
    fn knowledge_available(&self, operation: KnowledgeOperation) -> bool {
        use KnowledgeOperation as K;
        if operation == K::EvidenceLens {
            return self.evidence.is_some()
                && (self.philosophy_original_available || self.corpus_original_available);
        }
        if operation == K::Dossier {
            return self.navigation_original_available;
        }
        if operation == K::PhilosophyAudit {
            return self.philosophy_original_available && self.philosophy_audit_available;
        }
        if operation.is_philosophy() {
            return self.philosophy_original_available;
        }
        if operation.is_corpus() {
            return self.corpus_original_available && self.corpus.is_some();
        }
        matches!(
            operation,
            K::ExplorationContracts
                | K::Catalog
                | K::Node
                | K::Relation
                | K::Temporal
                | K::Lens
                | K::Explore
                | K::Focus
                | K::StoredLens
                | K::Contracts
                | K::SearchCapabilities
                | K::Dossier
                | K::PhilosophyContracts
                | K::PhilosophyEpistemic
                | K::PhilosophyAudit
                | K::PhilosophyPacket
                | K::PhilosophyChronology
                | K::PhilosophySourceEvidence
                | K::PhilosophyConceptLineage
                | K::PhilosophyLens
                | K::PhilosophyStatus
                | K::PhilosophySearch
                | K::PhilosophyScaleManifest
                | K::PhilosophyScaleRows
                | K::PhilosophyNode
                | K::PhilosophyEdge
                | K::PhilosophyNeighborhood
                | K::PhilosophyPath
                | K::PhilosophyView
                | K::PhilosophyViews
                | K::PhilosophyLayers
                | K::PhilosophyClusters
                | K::PhilosophyReview
                | K::PhilosophySnapshot
                | K::PhilosophyUnresolved
                | K::CorpusStatus
                | K::CorpusSummary
                | K::CorpusGraphViews
                | K::CorpusSearch
                | K::CorpusResources
                | K::CorpusNode
                | K::CorpusRelationPack
                | K::CorpusGraphView
                | K::CorpusPacket
        )
    }
    fn reading_search_available(&self) -> bool {
        false
    }
    fn knowledge(
        &self,
        request: KnowledgeRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'delivery>, AccessError> {
        if let KnowledgeRequest::EvidenceLens(request) = &request {
            let source_available = match &request.mode {
                tos_query::philosophy_read::EvidenceMode::Philosophy => {
                    self.philosophy_original_available
                }
                tos_query::philosophy_read::EvidenceMode::Corpus => self.corpus_original_available,
            };
            if self.evidence.is_none() || !source_available {
                return Err(AccessError::new(
                    AccessErrorCode::Unavailable,
                    "selected evidence source is unavailable",
                ));
            }
        }
        let scope = operation(&request).ok_or_else(|| {
            AccessError::new(
                AccessErrorCode::Unavailable,
                "selected original input adapter absent",
            )
        })?;
        let mut model = self.model.try_borrow_mut().map_err(|_| busy())?;
        let mut context = self.context.try_borrow_mut().map_err(|_| busy())?;
        let mut checkpoints = self.checkpoints.try_borrow_mut().map_err(|_| busy())?;
        context.prepare_operation(scope, Arc::clone(&probe), |catalog, inspect| {
            let current_probe = crate::knowledge::combined_probe(
                Arc::clone(&probe),
                tos_query::InspectCurrentAuthority::abort_probe(inspect),
            );
            if matches!(request, KnowledgeRequest::PhilosophyAudit) {
                crate::knowledge::check_abort(&current_probe)?;
                let path = self
                    .view
                    .philosophy_audit_path()
                    .map_err(|_| busy())?
                    .to_str()
                    .ok_or_else(|| busy())?;
                let raw_result = self
                    .view
                    .read_philosophy_audit(self.budgets.inspect.max_payload_bytes);
                crate::knowledge::check_abort(&current_probe)?;
                let raw = raw_result.map_err(|_| busy())?;
                let packet = crate::knowledge::execute_selected_philosophy_audit(
                    &mut model,
                    self.bound,
                    inspect,
                    path,
                    raw.as_deref(),
                    tos_query::philosophy_read::PhilosophyReadBudget {
                        inspect: self.budgets.inspect,
                        max_work_steps: self.budgets.inspect.max_read_vm_steps,
                    },
                    probe,
                )?;
                return Ok(packet);
            }
            if let KnowledgeRequest::EvidenceLens(request) = &request {
                crate::knowledge::check_abort(&current_probe)?;
                let evidence = self.evidence.ok_or_else(|| {
                    AccessError::new(
                        AccessErrorCode::Unavailable,
                        "selected evidence projection unavailable",
                    )
                })?;
                let raw = evidence.raw();
                if raw.len() > self.budgets.inspect.max_payload_bytes {
                    return Err(AccessError::new(
                        AccessErrorCode::BudgetExceeded,
                        "selected evidence bytes exceed input budget",
                    ));
                }
                evidence.charge_work(raw.len() as u64).map_err(|_| busy())?;
                let packet = crate::knowledge::execute_selected_evidence(
                    &mut model,
                    self.bound,
                    inspect,
                    request,
                    raw,
                    self.corpus,
                    tos_query::philosophy_read::PhilosophyReadBudget {
                        inspect: self.budgets.inspect,
                        max_work_steps: self.budgets.inspect.max_read_vm_steps,
                    },
                    probe,
                )?;
                return Ok(packet);
            }
            if matches!(request, KnowledgeRequest::Contracts) {
                use tos_compiler::native_snapshot::SelectedKnowledgeRegistry as Registry;
                crate::knowledge::check_abort(&probe)?;
                let cap = self
                    .contracts
                    .max_registry_bytes
                    .min(self.contracts.max_input_bytes);
                let entity_result = self.view.read_registry(Registry::EntityTypes, cap);
                crate::knowledge::check_abort(&probe)?;
                let entity = entity_result.map_err(|_| {
                    AccessError::new(
                        AccessErrorCode::Unavailable,
                        "selected entity registry unavailable",
                    )
                })?;
                let remaining = self
                    .contracts
                    .max_input_bytes
                    .checked_sub(entity.len())
                    .ok_or_else(|| {
                        AccessError::new(
                            AccessErrorCode::BudgetExceeded,
                            "selected registry overlap budget exceeded",
                        )
                    })?;
                let relation_result = self.view.read_registry(
                    Registry::RelationTypes,
                    self.contracts.max_registry_bytes.min(remaining),
                );
                crate::knowledge::check_abort(&probe)?;
                let relation = relation_result.map_err(|_| {
                    AccessError::new(
                        AccessErrorCode::Unavailable,
                        "selected relation registry unavailable",
                    )
                })?;
                return crate::knowledge::execute_selected_knowledge_contracts(
                    &mut model,
                    self.bound,
                    inspect,
                    [&entity, &relation],
                    self.contracts,
                    self.budgets.inspect,
                    probe,
                );
            }
            if let KnowledgeRequest::Corpus(request) = &request {
                let corpus = self.corpus.ok_or_else(|| {
                    AccessError::new(
                        AccessErrorCode::Unavailable,
                        "selected corpus read context unavailable",
                    )
                })?;
                return crate::knowledge::execute_selected_corpus(
                    &mut model,
                    self.bound,
                    inspect,
                    corpus,
                    request,
                    tos_query::corpus_read::CorpusReadBudget {
                        inspect: self.budgets.inspect,
                        max_work_steps: self.budgets.inspect.max_read_vm_steps,
                    },
                    probe,
                );
            }
            if matches!(request, KnowledgeRequest::CorpusViewIds) {
                return crate::knowledge::execute_selected_corpus_view_ids(
                    &mut model,
                    self.bound,
                    inspect,
                    tos_query::corpus_read::CorpusReadBudget {
                        inspect: self.budgets.inspect,
                        max_work_steps: self.budgets.inspect.max_read_vm_steps,
                    },
                    probe,
                );
            }
            crate::knowledge::execute_selected_knowledge(
                &mut model,
                self.bound,
                catalog,
                inspect,
                &mut **checkpoints,
                request,
                self.budgets,
                probe,
            )
        })
    }
}
