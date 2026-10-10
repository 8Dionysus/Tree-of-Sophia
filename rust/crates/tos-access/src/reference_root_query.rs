//! Reference-local metadata disclosure from the actual completed selected root.
//! This profile grants no managed publication, source text, rights or current use.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;
use tos_compiler::knowledge_stage::StageIsolation;
use tos_compiler::native_snapshot::{
    CompletedCaptureCarriers, CompletedNativeSnapshot, NativeColdOpenResourceHold,
};
use tos_compiler::{
    ColdOpenLimits, Error, NativeProcessLimits, PublicCapture, Result, VerifiedKnowledgeModel,
};
use tos_foundation::Digest256;
use tos_query::search_v2::{
    CurrentPolicyBinding, SearchSelectionBinding, SearchV2Error, SearchV2ErrorCode,
};
use tos_query::{
    BoundCmpKnowledge, CatalogCurrentAuthority, CatalogDisclosureLease, CatalogDisclosureScope,
    CatalogError, CatalogErrorCode, IndexedDisclosureScope, InspectCurrentAuthority,
    InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier,
};

/// Selected by the Access owner from its actual CLI operation, never wire policy strings.
#[derive(Clone, Copy, Debug)]
pub enum ReferenceMetadataOperation {
    Catalog,
    SourceDescend,
    PhilosophyAudit,
    KnowledgeHeader,
    CorpusHeader,
    Dossier,
    Temporal,
    Corpus(ReferenceOriginalOperation),
    Philosophy(ReferenceOriginalOperation),
    Evidence(ReferenceOriginalOperation),
    IndexedSearch,
    Contracts,
    Node,
    Relation,
    Exploration,
    Focus,
    StoredLens,
    Lens,
    LegacySearch,
    SearchCapabilities,
}
/// No public string constructor: source owner request types choose the exact scope.
#[derive(Clone, Copy, Debug)]
pub struct ReferenceOriginalOperation {
    id: &'static str,
    intended: &'static str,
}
impl ReferenceMetadataOperation {
    pub fn for_corpus(request: &tos_query::corpus_read::CorpusReadRequest) -> Self {
        Self::Corpus(ReferenceOriginalOperation {
            id: request.operation_id(),
            intended: tos_query::corpus_read::CORPUS_INTENDED_USE,
        })
    }
    pub fn for_philosophy(request: &tos_query::philosophy_read::PhilosophyReadRequest) -> Self {
        Self::Philosophy(ReferenceOriginalOperation {
            id: request.operation_id(),
            intended: tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE,
        })
    }
    pub fn for_evidence(request: &tos_query::philosophy_read::EvidenceRequest) -> Self {
        Self::Evidence(ReferenceOriginalOperation {
            id: tos_query::philosophy_read::EVIDENCE_OPERATION,
            intended: request.intended_use(),
        })
    }

    pub(crate) fn scope(self) -> (&'static str, &'static str) {
        match self {
            // Dedicated IDs of the selected header owner APIs; never Status scope.
            Self::KnowledgeHeader => (
                "tos_knowledge_header",
                "read_only_public_knowledge_header_v1",
            ),
            Self::CorpusHeader => (
                "tos_corpus_header",
                tos_query::corpus_read::CORPUS_INTENDED_USE,
            ),

            Self::PhilosophyAudit => (
                "tos_philosophy_graph_audit",
                tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE,
            ),
            Self::SourceDescend => (
                crate::common::OPERATION_ID,
                "read_only_public_metadata_navigation_v1",
            ),
            Self::Dossier => (
                tos_query::source_dossier::DOSSIER_OPERATION,
                tos_query::source_dossier::DOSSIER_INTENDED_USE,
            ),
            Self::Temporal => (
                tos_query::TEMPORAL_OPERATION,
                tos_query::TEMPORAL_INTENDED_USE,
            ),
            Self::Corpus(scope) | Self::Philosophy(scope) | Self::Evidence(scope) => {
                (scope.id, scope.intended)
            }
            Self::IndexedSearch => (
                tos_query::INDEXED_SEARCH_OPERATION_ID,
                tos_query::INDEXED_SEARCH_INTENDED_USE,
            ),
            Self::Catalog => (
                tos_query::CATALOG_OPERATION_ID,
                tos_query::CATALOG_INTENDED_USE,
            ),
            Self::Node => (
                tos_query::NODE_INSPECT_OPERATION,
                tos_query::INSPECT_INTENDED_USE,
            ),
            Self::Relation => (
                tos_query::RELATION_INSPECT_OPERATION,
                tos_query::INSPECT_INTENDED_USE,
            ),
            Self::Contracts => (
                tos_query::knowledge_contracts::KNOWLEDGE_CONTRACTS_OPERATION,
                tos_query::knowledge_contracts::KNOWLEDGE_CONTRACTS_INTENDED_USE,
            ),
            Self::Exploration => (
                tos_query::knowledge_exploration::EXPLORATION_OPERATION,
                tos_query::knowledge_exploration::EXPLORATION_INTENDED_USE,
            ),
            Self::Focus => (
                tos_query::knowledge_lens::FOCUS_OPERATION,
                tos_query::knowledge_lens::FOCUS_INTENDED_USE,
            ),
            Self::StoredLens => (
                tos_query::knowledge_lens::STORED_LENS_OPERATION,
                tos_query::knowledge_lens::STORED_LENS_INTENDED_USE,
            ),
            Self::Lens => (
                tos_query::knowledge_lens::LENS_OPERATION,
                tos_query::knowledge_lens::LENS_INTENDED_USE,
            ),
            Self::LegacySearch => (
                tos_query::knowledge_legacy_search::LEGACY_SEARCH_OPERATION,
                tos_query::knowledge_legacy_search::LEGACY_SEARCH_INTENDED_USE,
            ),
            Self::SearchCapabilities => (
                tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_OPERATION,
                tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_INTENDED_USE,
            ),
        }
    }
}

/// Private construction occurs only in the completed-model/current-capture callback.
/// Permission is the Reference-local metadata profile; typed model custody independently
/// proves which rows belong to it. It is not a publication or withdrawal registry.
pub struct ReferenceRootMetadataHold<'view, 'capture> {
    view: &'view CompletedCaptureCarriers<'capture>,
    selection: SearchSelectionBinding,
    owner_receipt: String,
    policy: CurrentPolicyBinding,
    operation: ReferenceMetadataOperation,
    abort: Arc<dyn tos_query::AbortProbe>,
    corpus_root: Option<String>,
    philosophy_root: Option<String>,
    navigation_root: Option<String>,
}
struct ReferenceAbort {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl tos_query::AbortProbe for ReferenceAbort {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        if self.cancelled.load(Ordering::Acquire) {
            Some(tos_query::AbortReason::Cancelled)
        } else if Instant::now() >= self.deadline {
            Some(tos_query::AbortReason::DeadlineExceeded)
        } else {
            None
        }
    }
}
fn search_refusal() -> SearchV2Error {
    SearchV2Error {
        code: SearchV2ErrorCode::Unavailable,
        message: "selected Reference metadata currentness unavailable",
    }
}
fn catalog_refusal() -> CatalogError {
    CatalogError {
        code: CatalogErrorCode::PolicyBindingUnavailable,
        message: "selected Reference metadata currentness unavailable",
    }
}
impl<'view, 'capture> ReferenceRootMetadataHold<'view, 'capture> {
    pub(crate) fn navigation_packet(
        &self,
        body: Vec<u8>,
    ) -> std::result::Result<crate::PreparedPacket<'view>, crate::AccessError> {
        if !matches!(self.operation, ReferenceMetadataOperation::SourceDescend)
            || self.navigation_root.is_none()
        {
            return Err(search_refusal().into());
        }
        self.current()?;
        Ok(crate::PreparedPacket {
            body,
            fence: Box::new(ReferenceDisclosureLease {
                view: self.view,
                abort: Arc::clone(&self.abort),
            }),
        })
    }
    pub(crate) fn policy_binding(&self) -> &CurrentPolicyBinding {
        &self.policy
    }
    /// Direct Original carrier adapters must call this before actual delivery
    /// and after final flush while the same callback/view still lives.
    pub fn recheck_delivery(&self) -> Result<()> {
        self.active().map_err(delivery_error)?;
        let physical = self.view.verify_current();
        self.active().map_err(delivery_error)?;
        physical
    }

    // Selected SQLite/model bytes are immutable. Per-unit permission polls the
    // original clock/token; actual physical source proof stays at PRE/disclosure
    // acquisition/final flush, not repeated for every authenticated row.
    fn active(&self) -> std::result::Result<(), SearchV2Error> {
        match self.abort.reason() {
            Some(tos_query::AbortReason::Cancelled) => Err(SearchV2Error {
                code: SearchV2ErrorCode::Cancelled,
                message: "Reference metadata cancelled",
            }),
            Some(tos_query::AbortReason::DeadlineExceeded) => Err(SearchV2Error {
                code: SearchV2ErrorCode::DeadlineExceeded,
                message: "Reference metadata deadline",
            }),
            None => Ok(()),
        }
    }
    fn current(&self) -> std::result::Result<(), SearchV2Error> {
        self.active()?;
        let physical = self.view.verify_current();
        self.active()?;
        physical.map_err(|_| search_refusal())
    }
    fn indexed_scope(&self) -> IndexedDisclosureScope {
        let (operation, intended) = self.operation.scope();
        IndexedDisclosureScope {
            operation_id: operation.into(),
            carrier_layer: tos_query::CATALOG_CARRIER_LAYER.into(),
            intended_use: intended.into(),
            selected_model_receipt_id: self.owner_receipt.clone(),
            source_cut: self.selection.source_cut.clone(),
            through_commit_seq: self.selection.through_commit_seq,
            source_membership_root: self.selection.source_membership_root,
            descriptor_sha256: self.selection.vocabulary.descriptor_sha256,
            selected_index_sha256: self.selection.index_root_sha256,
            policy_issuer_ref: self.policy.issuer_ref.clone(),
            policy_receipt_id: self.policy.authorization_receipt_id.clone(),
            policy_scope: self.policy.scope.clone(),
            policy_epoch: self.policy.policy_epoch.clone(),
            withdrawal_generation: self.policy.withdrawal_generation.clone(),
        }
    }
    fn catalog_scope(&self) -> CatalogDisclosureScope {
        let scope = self.indexed_scope();
        CatalogDisclosureScope {
            operation_id: scope.operation_id,
            carrier_layer: scope.carrier_layer,
            intended_use: scope.intended_use,
            selected_model_receipt_id: scope.selected_model_receipt_id,
            source_cut: scope.source_cut,
            through_commit_seq: scope.through_commit_seq,
            source_membership_root: scope.source_membership_root,
            descriptor_sha256: scope.descriptor_sha256,
            selected_index_sha256: scope.selected_index_sha256,
            catalog_packet_sha256: self.selection.catalog_packet_sha256,
            policy_issuer_ref: scope.policy_issuer_ref,
            policy_receipt_id: scope.policy_receipt_id,
            policy_scope: scope.policy_scope,
            policy_epoch: scope.policy_epoch,
            withdrawal_generation: scope.withdrawal_generation,
        }
    }
    fn original(
        &self,
        root: &str,
        expected: Option<&str>,
        source_cut: &str,
        descriptor: &str,
        raw: &[u8],
        sha: Digest256,
    ) -> std::result::Result<(), SearchV2Error> {
        self.active()?;
        if expected != Some(root)
            || source_cut != self.selection.source_cut
            || descriptor != self.selection.vocabulary.descriptor_sha256.to_hex()
            || Digest256::of_bytes(raw) != sha
        {
            return Err(search_refusal());
        }
        Ok(())
    }
}
struct ReferenceDisclosureLease<'view, 'capture> {
    view: &'view CompletedCaptureCarriers<'capture>,
    abort: Arc<dyn tos_query::AbortProbe>,
}
impl crate::DisclosureFence for ReferenceDisclosureLease<'_, '_> {
    fn recheck(&mut self) -> std::result::Result<(), crate::AccessError> {
        self.recheck_selected().map_err(Into::into)
    }
}
impl ReferenceDisclosureLease<'_, '_> {
    fn recheck_selected(&self) -> std::result::Result<(), SearchV2Error> {
        let active = || match self.abort.reason() {
            Some(tos_query::AbortReason::Cancelled) => Err(SearchV2Error {
                code: SearchV2ErrorCode::Cancelled,
                message: "Reference metadata delivery cancelled",
            }),
            Some(tos_query::AbortReason::DeadlineExceeded) => Err(SearchV2Error {
                code: SearchV2ErrorCode::DeadlineExceeded,
                message: "Reference metadata delivery deadline",
            }),
            None => Ok(()),
        };
        active()?;
        let physical = self.view.verify_current();
        active()?;
        physical.map_err(|_| search_refusal())
    }
}
fn delivery_error(error: SearchV2Error) -> Error {
    let kind = match error.code {
        SearchV2ErrorCode::Cancelled => std::io::ErrorKind::Interrupted,
        SearchV2ErrorCode::DeadlineExceeded => std::io::ErrorKind::TimedOut,
        _ => std::io::ErrorKind::PermissionDenied,
    };
    Error::Io(std::io::Error::new(kind, error.message))
}
fn catalog_delivery_error(error: SearchV2Error) -> CatalogError {
    CatalogError {
        code: match error.code {
            SearchV2ErrorCode::Cancelled => CatalogErrorCode::Cancelled,
            SearchV2ErrorCode::DeadlineExceeded => CatalogErrorCode::DeadlineExceeded,
            _ => CatalogErrorCode::PolicyBindingUnavailable,
        },
        message: error.message,
    }
}
impl CatalogDisclosureLease for ReferenceDisclosureLease<'_, '_> {
    fn recheck(&mut self) -> std::result::Result<(), CatalogError> {
        self.recheck_selected().map_err(catalog_delivery_error)
    }
}
impl InspectDisclosureLease for ReferenceDisclosureLease<'_, '_> {
    fn recheck(&mut self) -> std::result::Result<(), SearchV2Error> {
        self.recheck_selected()
    }
}
impl<'delivery, 'view: 'delivery, 'capture> CatalogCurrentAuthority<'delivery>
    for ReferenceRootMetadataHold<'view, 'capture>
{
    fn abort_probe(&self) -> Option<Arc<dyn tos_query::AbortProbe>> {
        Some(self.abort.clone())
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> CatalogDisclosureScope {
        self.catalog_scope()
    }
    fn check_selected(&mut self) -> std::result::Result<(), CatalogError> {
        self.active().map_err(catalog_delivery_error)
    }
    fn authorize_current(&mut self, sha: Digest256) -> std::result::Result<(), CatalogError> {
        self.active().map_err(catalog_delivery_error)?;
        if !matches!(self.operation, ReferenceMetadataOperation::Catalog)
            || sha != self.selection.catalog_packet_sha256
        {
            return Err(catalog_refusal());
        }
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &CatalogDisclosureScope,
        sha: Digest256,
    ) -> std::result::Result<Box<dyn CatalogDisclosureLease + 'delivery>, CatalogError> {
        self.current().map_err(catalog_delivery_error)?;
        if scope != &self.catalog_scope() {
            return Err(catalog_refusal());
        }
        CatalogCurrentAuthority::authorize_current(self, sha)?;
        Ok(Box::new(ReferenceDisclosureLease {
            view: self.view,
            abort: self.abort.clone(),
        }))
    }
}
impl<'delivery, 'view: 'delivery, 'capture> InspectCurrentAuthority<'delivery>
    for ReferenceRootMetadataHold<'view, 'capture>
{
    fn abort_probe(&self) -> Option<Arc<dyn tos_query::AbortProbe>> {
        Some(self.abort.clone())
    }
    fn disclosure_metadata_state_upper_bound(&self) -> std::result::Result<usize, SearchV2Error> {
        use tos_foundation::OwnedState;
        let (operation, intended) = self.operation.scope();
        let mut bytes = self
            .policy
            .retained_state_bytes()
            .map_err(|_| SearchV2Error {
                code: SearchV2ErrorCode::BudgetExceeded,
                message: "Reference policy state overflow",
            })?
            .checked_add(std::mem::size_of::<IndexedDisclosureScope>())
            .ok_or(SearchV2Error {
                code: SearchV2ErrorCode::BudgetExceeded,
                message: "Reference disclosure state overflow",
            })?;
        for capacity in [
            operation.len(),
            intended.len(),
            tos_query::CATALOG_CARRIER_LAYER.len(),
            self.owner_receipt.capacity(),
            self.selection.source_cut.capacity(),
            self.policy.issuer_ref.capacity(),
            self.policy.authorization_receipt_id.capacity(),
            self.policy.scope.capacity(),
            self.policy.policy_epoch.capacity(),
            self.policy.withdrawal_generation.capacity(),
        ] {
            bytes = bytes.checked_add(capacity).ok_or(SearchV2Error {
                code: SearchV2ErrorCode::BudgetExceeded,
                message: "Reference disclosure state overflow",
            })?;
        }
        Ok(bytes)
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.indexed_scope()
    }
    fn check_selected(&mut self) -> std::result::Result<(), SearchV2Error> {
        self.active()
    }
    fn authorize_current(
        &mut self,
        _: &InspectedCarrier,
    ) -> std::result::Result<(), SearchV2Error> {
        self.active()
    }
    fn authorize_catalog_current(
        &mut self,
        sha: Digest256,
    ) -> std::result::Result<(), SearchV2Error> {
        self.active()?;
        if sha != self.selection.catalog_packet_sha256 {
            return Err(search_refusal());
        }
        Ok(())
    }
    fn authorize_registry_current(
        &mut self,
        id: &str,
        raw: &[u8],
        sha: Digest256,
    ) -> std::result::Result<(), SearchV2Error> {
        self.active()?;
        let matched = (id == self.selection.entity_registry_id
            && sha == self.selection.entity_registry_sha256)
            || (id == self.selection.relation_registry_id
                && sha == self.selection.relation_registry_sha256);
        if !matched || Digest256::of_bytes(raw) != sha {
            return Err(search_refusal());
        }
        Ok(())
    }
    fn authorize_corpus_original_current(
        &mut self,
        r: &tos_compiler::CorpusOriginalReceipt,
        _: tos_compiler::CorpusOriginalCollection,
        _: u64,
        raw: &[u8],
        sha: Digest256,
    ) -> std::result::Result<(), SearchV2Error> {
        self.original(
            &r.component_root_sha256,
            self.corpus_root.as_deref(),
            &r.source_cut,
            &r.descriptor_sha256,
            raw,
            sha,
        )
    }
    fn authorize_philosophy_original_current(
        &mut self,
        r: &tos_compiler::PhilosophyOriginalReceipt,
        _: tos_compiler::PhilosophyOriginalCollection,
        _: u64,
        raw: &[u8],
        sha: Digest256,
    ) -> std::result::Result<(), SearchV2Error> {
        self.original(
            &r.component_root_sha256,
            self.philosophy_root.as_deref(),
            &r.source_cut,
            &r.descriptor_sha256,
            raw,
            sha,
        )
    }
    fn authorize_navigation_original_current(
        &mut self,
        r: &tos_compiler::NavigationOriginalReceipt,
        _: i64,
        raw: &[u8],
        sha: Digest256,
    ) -> std::result::Result<(), SearchV2Error> {
        self.original(
            &r.component_root_sha256,
            self.navigation_root.as_deref(),
            &r.source_cut,
            &r.descriptor_sha256,
            raw,
            sha,
        )
    }
    fn authorize_corpus_view_identity_current(
        &mut self,
        r: &tos_compiler::CorpusOriginalReceipt,
        _: u64,
        _: Option<&str>,
        _: Digest256,
    ) -> std::result::Result<(), SearchV2Error> {
        self.active()?;
        if self.corpus_root.as_deref() != Some(r.component_root_sha256.as_str())
            || r.source_cut != self.selection.source_cut
            || r.descriptor_sha256 != self.selection.vocabulary.descriptor_sha256.to_hex()
        {
            return Err(search_refusal());
        }
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        _: &[ObservedInspectCarrier],
    ) -> std::result::Result<Box<dyn InspectDisclosureLease + 'delivery>, SearchV2Error> {
        self.current()?;
        if scope != &self.indexed_scope() {
            return Err(search_refusal());
        }
        Ok(Box::new(ReferenceDisclosureLease {
            view: self.view,
            abort: self.abort.clone(),
        }))
    }
}

impl tos_query::IndexedDisclosureLease for ReferenceDisclosureLease<'_, '_> {
    fn recheck(&mut self) -> std::result::Result<(), SearchV2Error> {
        self.recheck_selected()
    }
}
impl<'delivery, 'view: 'delivery, 'capture> tos_query::ScopedIndexedKnowledgeAuthority<'delivery>
    for ReferenceRootMetadataHold<'view, 'capture>
{
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.indexed_scope()
    }
    fn check_selected(&mut self) -> std::result::Result<(), SearchV2Error> {
        self.active()
    }
    fn authorize_current(
        &mut self,
        _: &tos_query::search_candidate::SelectedSearchCandidate,
    ) -> std::result::Result<(), SearchV2Error> {
        self.active()?;
        if !matches!(self.operation, ReferenceMetadataOperation::IndexedSearch) {
            return Err(search_refusal());
        }
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        _: &[tos_query::ObservedSearchCandidate],
    ) -> std::result::Result<Box<dyn tos_query::IndexedDisclosureLease + 'delivery>, SearchV2Error>
    {
        self.current()?;
        if !matches!(self.operation, ReferenceMetadataOperation::IndexedSearch)
            || scope != &self.indexed_scope()
        {
            return Err(search_refusal());
        }
        Ok(Box::new(ReferenceDisclosureLease {
            view: self.view,
            abort: self.abort.clone(),
        }))
    }
}

/// One genuinely verified model/source context. Only operation scope changes;
/// all source/custody/work/deadline/token/resource holds remain original.
pub struct ReferenceMetadataContext<'view, 'capture> {
    hold: ReferenceRootMetadataHold<'view, 'capture>,
    scope_peak: std::cell::Cell<usize>,
    resource_scope_reservation: std::cell::Cell<Option<usize>>,
}
impl<'view, 'capture> ReferenceMetadataContext<'view, 'capture> {
    pub(crate) fn uses_capture_view(&self, view: &CompletedCaptureCarriers<'capture>) -> bool {
        std::ptr::eq(self.hold.view, view)
    }

    /// Forecast before any operation holder/abort allocation. The original
    /// Core ledger deducts this reservation before calling the query owner.
    pub(crate) fn reserve_resource_operation_scopes(
        &self,
    ) -> std::result::Result<usize, crate::AccessError> {
        use tos_foundation::OwnedState;
        let forecast = self
            .hold
            .retained_state_bytes()
            .and_then(|n| {
                n.checked_mul(3).ok_or_else(|| {
                    tos_foundation::FoundationError::new(
                        tos_foundation::FoundationErrorCode::BudgetExceeded,
                        "Reference scope forecast overflow",
                    )
                })
            })
            .and_then(|n| {
                tos_foundation::checked_state_add(n, crate::knowledge::combined_probe_state_bytes())
            })
            .map_err(|_| {
                crate::AccessError::new(
                    crate::AccessErrorCode::BudgetExceeded,
                    "Reference scope forecast overflow",
                )
            })?;
        self.resource_scope_reservation.set(Some(forecast));
        self.scope_peak.set(self.scope_peak.get().max(forecast));
        Ok(forecast)
    }

    // Preparation stays inside the outer factory callback. The HTTP transport
    // owns the returned real lease through final flush; no new epoch or budget.
    pub(crate) fn prepare_operation<T>(
        &mut self,
        operation: ReferenceMetadataOperation,
        probe: Arc<dyn tos_query::AbortProbe>,
        consume: impl FnOnce(
            &mut ReferenceRootMetadataHold<'view, 'capture>,
            &mut ReferenceRootMetadataHold<'view, 'capture>,
            &mut ReferenceRootMetadataHold<'view, 'capture>,
        ) -> std::result::Result<T, crate::AccessError>,
    ) -> std::result::Result<T, crate::AccessError> {
        self.hold.active()?;
        self.hold.operation = operation;
        // Existing QRY takes two mutable authority slots. Both borrow the one
        // actual source epoch; only the related Catalog scope is distinct.
        if let Some(reserved) = self.resource_scope_reservation.get() {
            use tos_foundation::OwnedState;
            let forecast = self
                .hold
                .retained_state_bytes()
                .ok()
                .and_then(|n| n.checked_mul(3))
                .and_then(|n| n.checked_add(crate::knowledge::combined_probe_state_bytes()))
                .ok_or_else(|| {
                    crate::AccessError::new(
                        crate::AccessErrorCode::BudgetExceeded,
                        "Reference scope forecast overflow",
                    )
                })?;
            if forecast > reserved {
                return Err(crate::AccessError::new(
                    crate::AccessErrorCode::BudgetExceeded,
                    "Reference original scope reservation exceeded",
                ));
            }
        }
        let abort = crate::knowledge::combined_probe(Arc::clone(&self.hold.abort), Some(probe));
        let scoped = |operation| ReferenceRootMetadataHold {
            view: self.hold.view,
            selection: self.hold.selection.clone(),
            owner_receipt: self.hold.owner_receipt.clone(),
            policy: self.hold.policy.clone(),
            operation,
            abort: Arc::clone(&abort),
            corpus_root: self.hold.corpus_root.clone(),
            philosophy_root: self.hold.philosophy_root.clone(),
            navigation_root: self.hold.navigation_root.clone(),
        };
        let mut catalog = scoped(ReferenceMetadataOperation::Catalog);
        let mut inspect = scoped(operation);
        let mut indexed = scoped(operation);
        // These are three genuinely distinct String/binding clones. Record
        // actual capacities while they are live, then retain the observed peak
        // as a conservative reservation until the callback's final disclosure.
        use tos_foundation::{OwnedState, checked_state_add};
        let scope_bytes = checked_state_add(
            catalog.retained_state_bytes().map_err(|_| {
                crate::AccessError::new(
                    crate::AccessErrorCode::BudgetExceeded,
                    "Reference scope state overflow",
                )
            })?,
            inspect.retained_state_bytes().map_err(|_| {
                crate::AccessError::new(
                    crate::AccessErrorCode::BudgetExceeded,
                    "Reference scope state overflow",
                )
            })?,
        )
        .and_then(|n| checked_state_add(n, indexed.retained_state_bytes()?))
        .map_err(|_| {
            crate::AccessError::new(
                crate::AccessErrorCode::BudgetExceeded,
                "Reference scope state overflow",
            )
        })?;
        let scope_bytes = scope_bytes
            .checked_add(crate::knowledge::combined_probe_state_bytes())
            .ok_or_else(|| {
                crate::AccessError::new(
                    crate::AccessErrorCode::BudgetExceeded,
                    "Reference scope capacity overflow",
                )
            })?;
        if self
            .resource_scope_reservation
            .get()
            .is_some_and(|reserved| scope_bytes > reserved)
        {
            return Err(crate::AccessError::new(
                crate::AccessErrorCode::BudgetExceeded,
                "Reference original scope capacity reservation exceeded",
            ));
        }
        self.scope_peak.set(self.scope_peak.get().max(scope_bytes));
        let result = consume(&mut catalog, &mut inspect, &mut indexed);
        self.hold.active()?;
        result
    }

    /// Borrow the already-held original request interruption owner.
    pub(crate) fn abort_probe(&self) -> Arc<dyn tos_query::AbortProbe> {
        self.hold.abort.clone()
    }

    pub fn with_operation(
        &mut self,
        operation: ReferenceMetadataOperation,
        consume: impl FnOnce(&mut ReferenceRootMetadataHold<'view, 'capture>) -> Result<()>,
    ) -> Result<()> {
        self.hold.active().map_err(delivery_error)?;
        self.hold.operation = operation;
        let result = consume(&mut self.hold);
        // Delivery has already taken place inside the callback; no rollback.
        self.hold.recheck_delivery()?;
        result
    }
}

/// The callback must complete actual transport delivery before returning. Existing
/// DisclosableCatalog/Inspect retain the borrowed currentness lease through flush.
/// No policy strings, arbitrary model/FD or standalone authorized JSON are accepted.
pub fn with_selected_metadata_context(
    capture: &PublicCapture,
    completed: &CompletedNativeSnapshot,
    isolation: &dyn StageIsolation,
    cold: ColdOpenLimits,
    process: NativeProcessLimits,
    working_ram_bytes: u64,
    resource_hold: &dyn NativeColdOpenResourceHold,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    consume: impl for<'model, 'view, 'capture> FnOnce(
        &mut VerifiedKnowledgeModel<'model>,
        &BoundCmpKnowledge<'_>,
        &mut ReferenceMetadataContext<'view, 'capture>,
        &'view CompletedCaptureCarriers<'capture>,
    ) -> Result<()>,
) -> Result<()> {
    completed.with_verified_model(
        capture,
        isolation,
        cold,
        process,
        working_ram_bytes,
        resource_hold,
        deadline,
        cancelled.as_ref(),
        |model, vocabulary, descriptor| {
            let bound = tos_query::bind_verified_knowledge(model, vocabulary, descriptor)
                .map_err(|_| Error::Invalid("Reference selected metadata model binding"))?;
            completed.with_capture_carriers(capture, |view| {
                view.verify_current()?;
                if bound.source_revision() != Some(completed.source_revision())
                    || view.source_revision() != Some(completed.source_revision())
                {
                    return Err(Error::Invalid("Reference selected metadata source profile"));
                }
                let policy = CurrentPolicyBinding {
                    scope: "reference_root_local_metadata_v1".into(),
                    issuer_ref: "tos-access/reference-root".into(),
                    authorization_receipt_id: format!(
                        "reference-metadata:{}",
                        bound.owner_receipt_id()
                    ),
                    policy_epoch: completed.source_revision().into(),
                    withdrawal_generation: "not-a-publication-registry".into(),
                };
                let hold = ReferenceRootMetadataHold {
                    view,
                    selection: bound.selection().clone(),
                    owner_receipt: bound.owner_receipt_id().into(),
                    policy,
                    operation: ReferenceMetadataOperation::Catalog,
                    abort: Arc::new(ReferenceAbort {
                        deadline,
                        cancelled: cancelled.clone(),
                    }),
                    corpus_root: if model.corpus_original_available() {
                        Some(
                            model
                                .corpus_original_receipt()?
                                .component_root_sha256
                                .clone(),
                        )
                    } else {
                        None
                    },
                    philosophy_root: if model.philosophy_original_available() {
                        Some(
                            model
                                .philosophy_original_receipt()?
                                .component_root_sha256
                                .clone(),
                        )
                    } else {
                        None
                    },
                    navigation_root: if model.navigation_original_available() {
                        Some(
                            model
                                .navigation_original_receipt()?
                                .component_root_sha256
                                .clone(),
                        )
                    } else {
                        None
                    },
                };
                let mut context = ReferenceMetadataContext {
                    hold,
                    scope_peak: std::cell::Cell::new(0),
                    resource_scope_reservation: std::cell::Cell::new(None),
                };
                let result = consume(model, &bound, &mut context, view);
                let final_fence = context.hold.recheck_delivery();
                final_fence?;
                result
            })
        },
    )
}

/// The same metadata authority can borrow either compiler-owned controlled
/// reader. The sidecar reader delegates receipts and currentness to its source.
trait ControlledReferenceModel {
    fn check_reference_pin(&self) -> tos_compiler::Result<()>;
    fn corpus_original_root(&self) -> Option<&str>;
    fn philosophy_original_root(&self) -> Option<&str>;
    fn navigation_original_root(&self) -> Option<&str>;
}
impl ControlledReferenceModel for tos_compiler::ControlledKnowledgeModel<'_, '_, '_> {
    fn check_reference_pin(&self) -> tos_compiler::Result<()> {
        self.check_pin()
    }
    fn corpus_original_root(&self) -> Option<&str> {
        self.corpus_original_receipt()
            .map(|r| r.component_root_sha256.as_str())
    }
    fn philosophy_original_root(&self) -> Option<&str> {
        self.philosophy_original_receipt()
            .map(|r| r.component_root_sha256.as_str())
    }
    fn navigation_original_root(&self) -> Option<&str> {
        self.navigation_original_receipt()
            .map(|r| r.component_root_sha256.as_str())
    }
}
impl ControlledReferenceModel for tos_compiler::ControlledSidecarModel<'_, '_, '_, '_> {
    fn check_reference_pin(&self) -> tos_compiler::Result<()> {
        self.check_pin()
    }
    fn corpus_original_root(&self) -> Option<&str> {
        self.corpus_original_receipt()
            .map(|r| r.component_root_sha256.as_str())
    }
    fn philosophy_original_root(&self) -> Option<&str> {
        self.philosophy_original_receipt()
            .map(|r| r.component_root_sha256.as_str())
    }
    fn navigation_original_root(&self) -> Option<&str> {
        self.navigation_original_receipt()
            .map(|r| r.component_root_sha256.as_str())
    }
}

/// Same ReferenceRoot authority owner over either already admitted controlled
/// model/view. Holder reservation, model and query workspace remain same-owner.
pub(crate) fn with_controlled_metadata_context<'view, 'capture, M, H>(
    model: &mut M,
    bound: &BoundCmpKnowledge<'_>,
    view: &'view CompletedCaptureCarriers<'capture>,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    reserve_original: impl FnOnce(usize) -> Result<H>,
    consume: impl FnOnce(&mut M, &mut ReferenceMetadataContext<'view, 'capture>) -> Result<()>,
) -> Result<()>
where
    M: ControlledReferenceModel,
{
    use tos_foundation::OwnedState;
    model.check_reference_pin()?;
    view.verify_current()?;
    let revision = bound
        .require_source_revision()
        .map_err(|_| Error::Invalid("controlled Reference source revision absent"))?;
    if view.source_revision() != Some(revision) {
        return Err(Error::Invalid("controlled Reference source view differs"));
    }
    // This full carrier authority is issued only after all three original
    // receipts have passed the controlled cold owner, never from wire roots.
    let corpus_root = model.corpus_original_root().ok_or(Error::Invalid(
        "controlled Reference corpus original absent",
    ))?;
    let philosophy_root = model.philosophy_original_root().ok_or(Error::Invalid(
        "controlled Reference philosophy original absent",
    ))?;
    let navigation_root = model.navigation_original_root().ok_or(Error::Invalid(
        "controlled Reference navigation original absent",
    ))?;
    let strings = [
        "reference_root_local_metadata_v1",
        "tos-access/reference-root",
        "reference-metadata:",
        bound.owner_receipt_id(),
        bound.owner_receipt_id(),
        revision,
        "not-a-publication-registry",
        corpus_root,
        philosophy_root,
        navigation_root,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        sum.checked_add(value.len())
            .ok_or(Error::Budget("controlled Reference holder strings"))
    })?;
    let selection = bound
        .selection()
        .owned_heap_bytes()
        .map_err(|_| Error::Budget("controlled Reference selection forecast"))?;
    let forecast = std::mem::size_of::<ReferenceMetadataContext<'_, '_>>()
        .checked_add(selection)
        .and_then(|n| n.checked_add(strings))
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<ReferenceAbort>() + 2 * std::mem::size_of::<usize>())
        })
        // The one actual indexed disclosure lease and its Box remain live
        // through final transport delivery under this original reservation.
        .and_then(|n| n.checked_add(std::mem::size_of::<ReferenceDisclosureLease<'_, '_>>()))
        .and_then(|n| {
            n.checked_add(std::mem::size_of::<
                Box<dyn tos_query::IndexedDisclosureLease>,
            >())
        })
        .and_then(|n| n.checked_add(std::mem::size_of::<H>()))
        .and_then(|n| n.checked_add(std::mem::size_of_val(&consume)))
        .and_then(|n| n.checked_add(3 * std::mem::size_of::<Result<()>>()))
        .ok_or(Error::Budget("controlled Reference holder forecast"))?;
    let _original_hold = reserve_original(forecast)?;
    let authorization_bytes = "reference-metadata:"
        .len()
        .checked_add(bound.owner_receipt_id().len())
        .ok_or(Error::Budget("controlled Reference receipt capacity"))?;
    let mut authorization_receipt_id = String::with_capacity(authorization_bytes);
    authorization_receipt_id.push_str("reference-metadata:");
    authorization_receipt_id.push_str(bound.owner_receipt_id());
    let policy = CurrentPolicyBinding {
        scope: "reference_root_local_metadata_v1".into(),
        issuer_ref: "tos-access/reference-root".into(),
        authorization_receipt_id,
        policy_epoch: revision.into(),
        withdrawal_generation: "not-a-publication-registry".into(),
    };
    let hold = ReferenceRootMetadataHold {
        view,
        selection: bound.selection().clone(),
        owner_receipt: bound.owner_receipt_id().into(),
        policy,
        operation: ReferenceMetadataOperation::IndexedSearch,
        abort: Arc::new(ReferenceAbort {
            deadline,
            cancelled: Arc::clone(cancelled),
        }),
        corpus_root: Some(corpus_root.to_owned()),
        philosophy_root: Some(philosophy_root.to_owned()),
        navigation_root: Some(navigation_root.to_owned()),
    };
    let mut context = ReferenceMetadataContext {
        hold,
        scope_peak: std::cell::Cell::new(0),
        resource_scope_reservation: std::cell::Cell::new(None),
    };
    let result = consume(model, &mut context);
    context.hold.recheck_delivery()?;
    model.check_reference_pin()?;
    view.verify_current()?;
    result
}

/// Single-operation convenience delegates to the same admitted context.
pub fn with_selected_metadata(
    capture: &PublicCapture,
    completed: &CompletedNativeSnapshot,
    isolation: &dyn StageIsolation,
    cold: ColdOpenLimits,
    process: NativeProcessLimits,
    working_ram_bytes: u64,
    resource_hold: &dyn NativeColdOpenResourceHold,
    operation: ReferenceMetadataOperation,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
    consume: impl for<'model, 'view, 'capture> FnOnce(
        &mut VerifiedKnowledgeModel<'model>,
        &BoundCmpKnowledge<'_>,
        &mut ReferenceRootMetadataHold<'view, 'capture>,
        &'view CompletedCaptureCarriers<'capture>,
    ) -> Result<()>,
) -> Result<()> {
    with_selected_metadata_context(
        capture,
        completed,
        isolation,
        cold,
        process,
        working_ram_bytes,
        resource_hold,
        deadline,
        cancelled,
        |model, bound, context, view| {
            context.with_operation(operation, |hold| consume(model, bound, hold, view))
        },
    )
}

impl tos_foundation::OwnedState for ReferenceRootMetadataHold<'_, '_> {
    fn owned_heap_bytes(&self) -> tos_foundation::Result<usize> {
        use tos_foundation::{OwnedState, checked_state_add};
        let mut bytes = 0;
        for amount in [
            self.selection.owned_heap_bytes()?,
            self.owner_receipt.capacity(),
            self.policy.owned_heap_bytes()?,
            self.corpus_root.owned_heap_bytes()?,
            self.philosophy_root.owned_heap_bytes()?,
            self.navigation_root.owned_heap_bytes()?,
        ] {
            bytes = checked_state_add(bytes, amount)?;
        }
        Ok(bytes)
    }
}
impl tos_foundation::OwnedState for ReferenceMetadataContext<'_, '_> {
    fn owned_heap_bytes(&self) -> tos_foundation::Result<usize> {
        use tos_foundation::OwnedState;
        // Borrowed capture, cancellation and view aliases are priced once by
        // their actual outer owners; the single Reference abort payload is ours.
        tos_foundation::checked_state_add(self.hold.owned_heap_bytes()?, self.scope_peak.get())
            .and_then(|n| {
                tos_foundation::checked_state_add(n, std::mem::size_of::<ReferenceAbort>())
            })
    }
}
pub(crate) fn reference_lease_state_bytes() -> usize {
    std::mem::size_of::<ReferenceDisclosureLease<'_, '_>>()
}
