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
            &mut dyn tos_query::ScopedIndexedKnowledgeAuthority<'hold>,
        ) -> Result<T, AccessError>,
    ) -> Result<T, AccessError>;
}

impl<'hold, 'view: 'hold, 'capture> SelectedQueryContext<'hold>
    for ReferenceMetadataContext<'view, 'capture>
{
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
            &mut dyn tos_query::ScopedIndexedKnowledgeAuthority<'hold>,
        ) -> Result<T, AccessError>,
    ) -> Result<T, AccessError> {
        ReferenceMetadataContext::prepare_operation(
            self,
            operation,
            probe,
            |catalog, inspect, indexed| consume(catalog, inspect, indexed),
        )
    }
}

/// The native owner supplies all limits and the same retained checkpoint store.
/// No field constructs a model, capture, authority, clock, or budget admission.
pub struct ReferenceQueryExecutor<'owner, 'model, 'bound, 'view, 'capture, 'evidence, C> {
    model: RefCell<&'owner mut VerifiedKnowledgeModel<'model>>,
    bound: &'owner BoundCmpKnowledge<'bound>,
    context: RefCell<&'owner mut C>,
    checkpoints: RefCell<&'owner mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints>,
    resource_query_state: std::cell::Cell<Option<usize>>,
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
    C: SelectedQueryContext<'owner>,
{
    /// Call inside the owner callback which supplies `context`, `view`, and any
    /// Evidence view. The socket must finish flushing before those callbacks
    /// return. RefCell guards serialize the existing mutable QRY session.
    /// State of the actual borrowed owners used by this executor, excluding
    /// aliases to outer capture/Evidence/corpus/checkpoint holders.
    pub(crate) fn retained_state_upper_bound(&self) -> Result<usize, AccessError>
    where
        C: tos_foundation::OwnedState,
    {
        use tos_foundation::{OwnedState, checked_state_add};
        let map = |_| {
            AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "Reference executor retained state overflow",
            )
        };
        let model = self.model.try_borrow().map_err(|_| {
            AccessError::new(AccessErrorCode::Unavailable, "Reference model is busy")
        })?;
        let context = self.context.try_borrow().map_err(|_| {
            AccessError::new(AccessErrorCode::Unavailable, "Reference context is busy")
        })?;
        let mut bytes = std::mem::size_of::<Self>();
        for amount in [
            model.retained_state_upper_bound().map_err(|_| map(()))?,
            self.bound
                .retained_state_upper_bound()
                .map_err(|_| map(()))?,
            (**context).retained_state_bytes().map_err(|_| map(()))?,
        ] {
            bytes = checked_state_add(bytes, amount).map_err(|_| map(()))?;
        }
        Ok(bytes)
    }

    pub(crate) fn reserve_resource_query_state(&self, remaining_state_bytes: usize) {
        self.resource_query_state.set(Some(remaining_state_bytes));
    }

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
            resource_query_state: std::cell::Cell::new(None),
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
struct ReferenceHealthFence<'hold> {
    fences: [Option<Box<dyn crate::DisclosureFence + 'hold>>; 5],
    probe: Arc<dyn AbortProbe>,
}
impl crate::DisclosureFence for ReferenceHealthFence<'_> {
    fn recheck(&mut self) -> Result<(), AccessError> {
        crate::knowledge::check_abort(&self.probe)?;
        for fence in self.fences.iter_mut().flatten() {
            fence.recheck()?;
        }
        crate::knowledge::check_abort(&self.probe)
    }
}
struct ReferenceIndexedFence<'hold>(Box<dyn tos_query::IndexedDisclosureLease + 'hold>);
impl crate::DisclosureFence for ReferenceIndexedFence<'_> {
    fn recheck(&mut self) -> Result<(), AccessError> {
        self.0.recheck().map_err(Into::into)
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
            |_, inspect, _| {
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
    fn access_health_available(&self) -> bool {
        true
    }
    fn access_health(
        &self,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'delivery>, AccessError> {
        self.access_health_report(probe).map(|report| report.packet)
    }
    fn access_health_report(
        &self,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<crate::common::PreparedHealth<'delivery>, AccessError> {
        use crate::managed_local::{
            HealthChild, compose_selected_access_health, health_child_packet,
            partition_health_inspect_budget,
        };
        crate::knowledge::check_abort(&probe)?;
        let original = self.budgets.inspect;
        let mut budget = partition_health_inspect_budget(
            original,
            original.max_response_bytes,
            original.max_decoded_bytes,
            original.max_rows,
            original.max_read_vm_steps,
        )?;
        budget.max_open_vm_steps = original.max_open_vm_steps;
        let work_steps = original.max_read_vm_steps / 6;
        let mut model = self.model.try_borrow_mut().map_err(|_| busy())?;
        let mut context = self.context.try_borrow_mut().map_err(|_| busy())?;
        let mut fences: [Option<Box<dyn crate::DisclosureFence + 'delivery>>; 5] =
            std::array::from_fn(|_| None);
        let mut next_fence = 0usize;
        let (body, ok) = compose_selected_access_health(
            budget,
            original.max_response_bytes,
            original.json,
            &probe,
            |child, inspect, meter| {
                let corpus_request = match &child {
                    HealthChild::CorpusView(id) => {
                        tos_query::corpus_read::CorpusReadRequest::GraphView {
                            view_id: (*id).to_owned(),
                            limit: 1,
                        }
                    }
                    _ => tos_query::corpus_read::CorpusReadRequest::Status,
                };
                let philosophy_request = match &child {
                    HealthChild::PhilosophyView(id) => {
                        tos_query::philosophy_read::PhilosophyReadRequest::View {
                            view_id: (*id).to_owned(),
                            limit: 1,
                        }
                    }
                    _ => tos_query::philosophy_read::PhilosophyReadRequest::Views,
                };
                let operation = match &child {
                    HealthChild::CorpusSeed | HealthChild::CorpusView(_) => {
                        ReferenceMetadataOperation::for_corpus(&corpus_request)
                    }
                    HealthChild::PhilosophySeed | HealthChild::PhilosophyView(_) => {
                        ReferenceMetadataOperation::for_philosophy(&philosophy_request)
                    }
                    HealthChild::Knowledge => ReferenceMetadataOperation::Catalog,
                };
                let mut packet = context.prepare_operation(operation, Arc::clone(&probe), |catalog, authority, _| {
                    let packet = match child {
                        HealthChild::CorpusSeed => crate::knowledge::from_inspect(tos_query::corpus_read::execute_selected_corpus_health_seed_metered(&mut model, self.bound, authority, self.corpus.ok_or_else(|| AccessError::new(AccessErrorCode::Unavailable, "selected corpus source context unavailable"))?, tos_query::corpus_read::CorpusReadBudget { inspect, max_work_steps: work_steps }, meter)?),
                        HealthChild::CorpusView(_) => crate::knowledge::from_inspect(tos_query::corpus_read::execute_selected_corpus_metered(&mut model, self.bound, authority, self.corpus.ok_or_else(|| AccessError::new(AccessErrorCode::Unavailable, "selected corpus source context unavailable"))?, &corpus_request, tos_query::corpus_read::CorpusReadBudget { inspect, max_work_steps: work_steps }, meter)?),
                        HealthChild::PhilosophySeed => crate::knowledge::from_inspect(tos_query::philosophy_read::execute_selected_philosophy_health_seed_metered(&mut model, self.bound, authority, tos_query::philosophy_read::PhilosophyReadBudget { inspect, max_work_steps: work_steps }, meter)?),
                        HealthChild::PhilosophyView(_) => crate::knowledge::from_inspect(tos_query::philosophy_read::execute_selected_philosophy_metered(&mut model, self.bound, authority, &philosophy_request, tos_query::philosophy_read::PhilosophyReadBudget { inspect, max_work_steps: work_steps }, meter)?),
                        HealthChild::Knowledge => crate::knowledge::from_catalog(tos_query::execute_selected_knowledge_health_metadata(&mut model, self.bound, catalog, tos_query::CatalogBudget { max_open_vm_steps: original.max_open_vm_steps, max_read_vm_steps: inspect.max_read_vm_steps, max_packet_bytes: inspect.max_response_bytes, max_decoded_bytes: usize::try_from(inspect.max_decoded_bytes).unwrap_or(usize::MAX), json: inspect.json }, meter)?),
                    };
                    Ok(packet)
                })?;
                packet.fence.recheck()?;
                crate::knowledge::check_abort(&probe)?;
                let value = health_child_packet(&packet.body, inspect.json, meter)?;
                fences[next_fence] = Some(packet.fence);
                next_fence += 1;
                Ok(value)
            },
        )?;
        let mut fence = ReferenceHealthFence { fences, probe };
        crate::DisclosureFence::recheck(&mut fence)?;
        Ok(crate::common::PreparedHealth {
            ok,
            packet: PreparedPacket {
                body,
                fence: Box::new(fence),
            },
        })
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
            |_, inspect, _| {
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
            |_, _, authority| {
                let selected = tos_query::search_v2::IndexedSearchV2Request {
                    query: request.query,
                    sources: request.sources,
                    kind_ids: request.kind_ids,
                    predicate_ids: request.predicate_ids,
                    limit: request.limit,
                };
                let normalized = selected
                    .clone()
                    .normalize(self.bound.selection(), self.bound)?;
                let initial = tos_query::search_v2::SearchContinuationState::new(
                    self.bound.selection().clone(),
                    normalized,
                    authority.policy_binding().clone(),
                    self.bound,
                )?;
                let mut codec = crate::indexed_cursor::NativeIndexedCursorCodec::new(
                    initial,
                    self.bound.owner_receipt_id(),
                );
                let packet = tos_query::execute_scoped_indexed_search_page(
                    &mut model,
                    self.bound,
                    authority,
                    &mut codec,
                    selected,
                    request.cursor.as_deref(),
                    self.indexed,
                )?;
                crate::knowledge::check_abort(&probe)?;
                let (body, lease) = packet.into_parts();
                Ok(PreparedPacket {
                    body,
                    fence: Box::new(ReferenceIndexedFence(lease)),
                })
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
        context.prepare_operation(scope, Arc::clone(&probe), |catalog, inspect, _| {
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
                    Some(raw.as_slice()),
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
                if matches!(
                    request,
                    tos_query::corpus_read::CorpusReadRequest::GraphViews
                ) {
                    if let Some(remaining) = self.resource_query_state.get() {
                        return crate::knowledge::execute_selected_corpus_graph_views_with_state(
                            &mut model,
                            self.bound,
                            inspect,
                            corpus,
                            tos_query::corpus_read::CorpusReadBudget {
                                inspect: self.budgets.inspect,
                                max_work_steps: self.budgets.inspect.max_read_vm_steps,
                            },
                            probe,
                            remaining,
                        );
                    }
                }
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
