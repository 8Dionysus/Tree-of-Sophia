//! Shared extraction of the existing selected_wires synthetic test authority.
//! Custody/fence mechanics only; never an installed or production grant.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};
use tos_access::{KnowledgeOperation as O, KnowledgeRequest as R};
use tos_foundation::Digest256;
use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error};
use tos_query::{
    AbortProbe, BoundCmpKnowledge, CatalogCurrentAuthority, CatalogDisclosureLease,
    CatalogDisclosureScope, CatalogError, IndexedDisclosureScope, InspectCurrentAuthority,
    InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier,
};

// Resource limits for this fixture constructor, not policy/identity selectors.
// Borrow and check every variable-sized input before the first owned conversion.
fn check_constructor_inputs(bound: &BoundCmpKnowledge<'_>, operation: &str, intended: &str) {
    const MAX_INPUT_BYTES: usize = 16 * 1024;
    const MAX_CONSTRUCTOR_BYTES: usize = 256 * 1024;
    let selected = bound.selection();
    // Inline state plus a conservative 1024-byte allowance for all fixed literal
    // String copies (policy x3, carrier x2, catalog operation and intended use).
    let mut bytes = std::mem::size_of::<Authority>()
        .checked_add(1024)
        .expect("selected fixture constructor state overflow");
    for (value, copies) in [
        (operation, 1usize),
        (intended, 1),
        (bound.owner_receipt_id(), 2),
        (selected.source_cut.as_str(), 2),
        (selected.entity_registry_id.as_str(), 1),
        (selected.relation_registry_id.as_str(), 1),
    ] {
        assert!(
            value.len() <= MAX_INPUT_BYTES,
            "selected fixture constructor input limit"
        );
        bytes = value
            .len()
            .checked_mul(copies)
            .and_then(|n| bytes.checked_add(n))
            .expect("selected fixture constructor state overflow");
    }
    assert!(
        bytes <= MAX_CONSTRUCTOR_BYTES,
        "selected fixture constructor state limit"
    );
}

#[derive(Default)]
pub(crate) struct Controls {
    pub(crate) revoked: AtomicBool,
    pub(crate) cancelled: AtomicBool,
    // Fixture-only withdrawal/cancellation immediately after preparation.
    pub(crate) after_prepare: AtomicU8,
    pub(crate) catalog_grants: AtomicUsize,
}
impl AbortProbe for Controls {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        self.cancelled
            .load(Ordering::SeqCst)
            .then_some(tos_query::AbortReason::Cancelled)
    }
}
struct Lease(Arc<AtomicUsize>, Arc<Controls>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl InspectDisclosureLease for Lease {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        use tos_query::search_v2::SearchV2ErrorCode as Code;
        let code = if self.1.revoked.load(Ordering::SeqCst) {
            Some(Code::StalePolicy)
        } else if self.1.cancelled.load(Ordering::SeqCst) {
            Some(Code::Cancelled)
        } else {
            None
        };
        match code {
            Some(code) => Err(SearchV2Error {
                code,
                message: "fixture disclosure withdrawn or cancelled",
            }),
            None => Ok(()),
        }
    }
}
impl CatalogDisclosureLease for Lease {
    fn recheck(&mut self) -> Result<(), CatalogError> {
        Ok(())
    }
}
pub(crate) struct Authority {
    policy: CurrentPolicyBinding,
    catalog: CatalogDisclosureScope,
    inspect: IndexedDisclosureScope,
    held: Arc<AtomicUsize>,
    controls: Arc<Controls>,
    registries: [(String, Digest256); 2],
    pub(crate) registry_grants: u8,
    philosophy_granted: bool,
    corpus_granted: bool,
}
impl Authority {
    pub(crate) fn new(
        bound: &BoundCmpKnowledge<'_>,
        request: &R,
        held: Arc<AtomicUsize>,
        controls: Arc<Controls>,
    ) -> Self {
        let intended = match request.operation() {
            op if op.is_corpus() => tos_query::corpus_read::CORPUS_INTENDED_USE,
            op if op.is_philosophy() => tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE,
            O::Dossier => tos_query::source_dossier::DOSSIER_INTENDED_USE,
            O::Explore => tos_query::knowledge_exploration::EXPLORATION_INTENDED_USE,
            O::Temporal => tos_query::TEMPORAL_INTENDED_USE,
            O::Lens => tos_query::knowledge_lens::LENS_INTENDED_USE,
            O::Focus => tos_query::knowledge_lens::FOCUS_INTENDED_USE,
            O::StoredLens => tos_query::knowledge_lens::STORED_LENS_INTENDED_USE,
            O::SearchCapabilities => {
                tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_INTENDED_USE
            }
            O::Contracts => tos_query::knowledge_contracts::KNOWLEDGE_CONTRACTS_INTENDED_USE,
            _ => "read_only_public_knowledge_inspect_v1",
        };
        Self::for_scope(bound, request.operation().id(), intended, held, controls)
    }
    pub(crate) fn for_scope(
        bound: &BoundCmpKnowledge<'_>,
        operation: &str,
        intended: &str,
        held: Arc<AtomicUsize>,
        controls: Arc<Controls>,
    ) -> Self {
        check_constructor_inputs(bound, operation, intended);
        let policy = CurrentPolicyBinding {
            scope: "synthetic-wire".into(),
            issuer_ref: "synthetic-issuer".into(),
            authorization_receipt_id: "synthetic-receipt".into(),
            policy_epoch: "synthetic-epoch".into(),
            withdrawal_generation: "synthetic-withdrawal".into(),
        };
        let selected = bound.selection();
        let inspect = IndexedDisclosureScope {
            operation_id: operation.into(),
            carrier_layer: "tos_knowledge_public_graph_projection_v1".into(),
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
            operation_id: "tos.knowledge.catalog".into(),
            carrier_layer: inspect.carrier_layer.clone(),
            intended_use: "read_only_public_knowledge_catalog_v1".into(),
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
        Self {
            policy,
            catalog,
            inspect,
            held,
            controls,
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
            philosophy_granted: false,
            corpus_granted: false,
        }
    }
    fn lease(&self) -> Lease {
        self.held.fetch_add(1, Ordering::SeqCst);
        Lease(Arc::clone(&self.held), Arc::clone(&self.controls))
    }
}
impl<'hold> CatalogCurrentAuthority<'hold> for Authority {
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> CatalogDisclosureScope {
        self.catalog.clone()
    }
    fn check_selected(&mut self) -> Result<(), CatalogError> {
        Ok(())
    }
    fn authorize_current(&mut self, sha: Digest256) -> Result<(), CatalogError> {
        assert_eq!(sha, self.catalog.catalog_packet_sha256);
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        _: &CatalogDisclosureScope,
        _: Digest256,
    ) -> Result<Box<dyn CatalogDisclosureLease + 'hold>, CatalogError> {
        Ok(Box::new(self.lease()))
    }
}
impl<'hold> InspectCurrentAuthority<'hold> for Authority {
    fn disclosure_metadata_state_upper_bound(&self) -> Result<usize, SearchV2Error> {
        use tos_foundation::OwnedState;
        let overflow = || SearchV2Error {
            code: tos_query::search_v2::SearchV2ErrorCode::BudgetExceeded,
            message: "selected fixture disclosure state overflow",
        };
        // Same accounting boundary as ReferenceRootMetadataHold: the owned
        // policy/scope copies returned below, including all inline digest roots.
        // Include the lease allocation and boxed handle; Arc targets stay shared.
        let mut bytes = self
            .policy
            .retained_state_bytes()
            .map_err(|_| overflow())?
            .checked_add(std::mem::size_of::<IndexedDisclosureScope>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Lease>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Box<dyn InspectDisclosureLease>>()))
            .ok_or_else(overflow)?;
        for capacity in [
            self.inspect.operation_id.capacity(),
            self.inspect.carrier_layer.capacity(),
            self.inspect.intended_use.capacity(),
            self.inspect.selected_model_receipt_id.capacity(),
            self.inspect.source_cut.capacity(),
            self.inspect.policy_issuer_ref.capacity(),
            self.inspect.policy_receipt_id.capacity(),
            self.inspect.policy_scope.capacity(),
            self.inspect.policy_epoch.capacity(),
            self.inspect.withdrawal_generation.capacity(),
        ] {
            bytes = bytes.checked_add(capacity).ok_or_else(overflow)?;
        }
        Ok(bytes)
    }
    fn authorize_corpus_view_identity_current(
        &mut self,
        _: &tos_compiler::CorpusOriginalReceipt,
        _: u64,
        _: Option<&str>,
        _: Digest256,
    ) -> Result<(), SearchV2Error> {
        InspectCurrentAuthority::check_selected(self)?;
        assert_eq!(self.inspect.operation_id, O::CorpusSummary.id());
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
        assert_eq!(
            self.inspect.intended_use,
            tos_query::corpus_read::CORPUS_INTENDED_USE
        );
        assert_eq!(
            receipt.descriptor_sha256,
            self.inspect.descriptor_sha256.to_hex()
        );
        assert_eq!(receipt.source_cut, self.inspect.source_cut);
        assert_eq!(
            receipt.membership_root,
            self.inspect.source_membership_root.to_hex()
        );
        assert_eq!(Digest256::of_bytes(raw), sha);
        match collection {
            tos_compiler::CorpusOriginalCollection::Header => {
                assert_eq!(ordinal, 0);
                assert_eq!(sha.to_hex(), receipt.header_sha256);
            }
            _ => assert!(
                receipt
                    .collections
                    .iter()
                    .any(|c| c.collection == collection.as_str() && ordinal < c.rows)
            ),
        }
        self.corpus_granted = true;
        Ok(()) // Fixture-only transport reference, never installed authority.
    }

    fn abort_probe(&self) -> Option<Arc<dyn AbortProbe>> {
        Some(self.controls.clone())
    }
    fn authorize_philosophy_original_current(
        &mut self,
        receipt: &tos_compiler::PhilosophyOriginalReceipt,
        collection: tos_compiler::PhilosophyOriginalCollection,
        ordinal: u64,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        use tos_compiler::PhilosophyOriginalCollection as C;
        assert_eq!(
            self.inspect.intended_use,
            tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE
        );
        assert_eq!(receipt.source_cut, self.inspect.source_cut);
        assert_eq!(
            receipt.descriptor_sha256,
            self.inspect.descriptor_sha256.to_hex()
        );
        assert_eq!(
            receipt.membership_root,
            self.inspect.source_membership_root.to_hex()
        );
        assert_eq!(Digest256::of_bytes(raw), sha);
        match collection {
            C::Header => {
                assert_eq!(ordinal, 0);
                assert_eq!(sha.to_hex(), receipt.header_sha256);
            }
            C::Nodes => assert!(ordinal < receipt.nodes),
            C::Edges => assert!(ordinal < receipt.edges),
        }
        self.philosophy_granted = true;
        Ok(()) // Fixture-only oracle; production compares the retained producer receipt.
    }
    fn authorize_navigation_original_current(
        &mut self,
        _: &tos_compiler::NavigationOriginalReceipt,
        ordinal: i64,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        assert_eq!(self.inspect.operation_id, O::Dossier.id());
        assert!(ordinal >= -1);
        assert_eq!(Digest256::of_bytes(raw), sha);
        Ok(()) // Test-only QRY reference, never the managed-local factory.
    }
    fn authorize_registry_current(
        &mut self,
        id: &str,
        raw: &[u8],
        sha: Digest256,
    ) -> Result<(), SearchV2Error> {
        assert_eq!(self.inspect.operation_id, O::Contracts.id());
        assert_eq!(Digest256::of_bytes(raw), sha);
        let at = self
            .registries
            .iter()
            .position(|(selected_id, selected_sha)| selected_id == id && *selected_sha == sha)
            .expect("grant exact selected original registry carrier");
        self.registry_grants |= 1 << at;
        Ok(())
    }
    fn authorize_catalog_current(&mut self, sha: Digest256) -> Result<(), SearchV2Error> {
        assert_eq!(self.inspect.operation_id, O::StoredLens.id());
        assert_eq!(sha, self.catalog.catalog_packet_sha256);
        self.controls.catalog_grants.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.policy.clone()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.inspect.clone()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        Ok(())
    }
    fn authorize_current(&mut self, _: &InspectedCarrier) -> Result<(), SearchV2Error> {
        Ok(())
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        observed: &[ObservedInspectCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
        if self.inspect.intended_use == tos_query::corpus_read::CORPUS_INTENDED_USE {
            assert!(self.corpus_granted);
            assert!(observed.is_empty());
        } else if self.inspect.intended_use == tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE {
            assert!(self.philosophy_granted);
            assert!(
                observed.is_empty(),
                "phi originals are separately granted under the same hold"
            );
        } else if self.inspect.operation_id == O::Contracts.id() {
            assert_eq!(
                self.registry_grants, 3,
                "one current hold must cover both selected registry grants"
            );
            assert!(
                observed.is_empty(),
                "registry carriers are separately granted, not graph rows"
            );
        } else if self.inspect.operation_id
            == tos_query::knowledge_legacy_search::SEARCH_CAPABILITIES_OPERATION
        {
            assert!(
                observed.is_empty(),
                "capabilities authenticates selected engine binding without disclosing graph rows"
            );
        } else if observed.is_empty()
            && self.inspect.operation_id == "tos.knowledge.temporal.list_claims"
            && self.inspect.intended_use == "read_only_public_knowledge_claim_list_v1"
        {
            // An empty selected suffix discloses no carrier. It still needs
            // the exact current TEST scope and the same revocable lease.
            if scope != &self.inspect
                || scope.policy_issuer_ref != self.policy.issuer_ref
                || scope.policy_receipt_id != self.policy.authorization_receipt_id
                || scope.policy_scope != self.policy.scope
                || scope.policy_epoch != self.policy.policy_epoch
                || scope.withdrawal_generation != self.policy.withdrawal_generation
            {
                return Err(SearchV2Error {
                    code: tos_query::search_v2::SearchV2ErrorCode::StalePolicy,
                    message: "empty list disclosure scope or policy changed",
                });
            }
            let mut lease = self.lease();
            InspectDisclosureLease::recheck(&mut lease)?;
            return Ok(Box::new(lease));
        } else {
            assert!(!observed.is_empty());
        }
        Ok(Box::new(self.lease()))
    }
}
