//! Explicit managed-local projection composition. ReleaseStore is the holder;
//! neither CMP custody nor these callbacks grant raw source/payload access.
use crate::exploration_checkpoints::{CheckpointLimits, ProcessExplorationCheckpoints};
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
use tos_foundation::{Digest256, JsonLimits};
use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode};
use tos_query::{
    AbortProbe, BoundCmpKnowledge, CatalogCurrentAuthority, CatalogDisclosureLease,
    CatalogDisclosureScope, CatalogError, CatalogErrorCode, IndexedDisclosureScope,
    InspectCurrentAuthority, InspectDisclosureLease, InspectedCarrier, ObservedInspectCarrier,
};

pub struct ManagedLocalExecutor {
    release: Arc<ManagedRelease>,
    model: Mutex<VerifiedKnowledgeModel<'static>>,
    vocabulary: QueryVocabulary,
    descriptor: Vec<u8>,
    registries: [Vec<u8>; 2],
    original: Option<NavigationOriginalReceipt>,
    philosophy_original: Option<PhilosophyOriginalReceipt>,
    cold: ColdOpenLimits,
    profile: AccessProfile,
    checkpoints: ProcessExplorationCheckpoints,
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
        let release = ManagedRelease::open(root)?;
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
        cold_hold.recheck()?;
        Self::from_admitted_selection(
            release,
            model,
            selection.vocabulary().clone(),
            descriptor,
            [entity, relation],
            selection.producer().navigation_original.clone(),
            selection.producer().philosophy_original.clone(),
            selection.cold_limits(),
            profile,
        )
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
        cold: ColdOpenLimits,
        profile: AccessProfile,
    ) -> Result<Self, AccessError> {
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
            cold,
            profile,
            checkpoints,
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
}
fn intended(operation: O) -> &'static str {
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
    ) -> Result<PreparedPacket, AccessError> {
        Err(unavailable(
            "exact source owner is not selected by the release holder",
        ))
    }
    fn knowledge_available(&self, operation: O) -> bool {
        if operation.is_philosophy() {
            self.philosophy_original.is_some()
        } else {
            operation != O::Dossier || self.original.is_some()
        }
    }
    fn knowledge_search_legacy_available(&self) -> bool {
        true
    }
    fn knowledge_search_legacy(
        &self,
        request: tos_query::knowledge_legacy_search::LegacySearchRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
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
    ) -> Result<PreparedPacket, AccessError> {
        let cold = self
            .model
            .lock()
            .map_err(|_| unavailable("selected native model lock poisoned"))?;
        let mut model = cold
            .fork_reader_with_vm_budget(self.cold.max_vm_steps)
            .map_err(|_| unavailable("selected native reader unavailable"))?;
        drop(cold);
        let bound = tos_query::bind_verified_knowledge(&model, &self.vocabulary, &self.descriptor)?;
        let operation = request.operation();
        let mut inspect = Authority::new(self, &bound, operation.id(), intended(operation))?;
        let budgets = self.budgets();
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
        crate::knowledge::execute_selected_knowledge(
            &mut model,
            &bound,
            &mut catalog,
            &mut inspect,
            &mut checkpoints,
            request,
            budgets,
            probe,
        )
    }
}
struct Authority {
    policy: CurrentPolicyBinding,
    inspect: IndexedDisclosureScope,
    catalog: CatalogDisclosureScope,
    lease: Option<ReleaseLease>,
    registries: [(String, Digest256); 2],
    registry_grants: u8,
    original: Option<NavigationOriginalReceipt>,
    original_granted: bool,
    philosophy_original: Option<PhilosophyOriginalReceipt>,
    philosophy_granted: bool,
}
impl Authority {
    fn new(
        owner: &ManagedLocalExecutor,
        bound: &BoundCmpKnowledge<'_>,
        operation: &str,
        intended: &str,
    ) -> Result<Self, AccessError> {
        let lease = owner.release.acquire()?;
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
            lease: Some(lease),
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
        })
    }
    fn check(&mut self) -> Result<(), SearchV2Error> {
        self.lease
            .as_mut()
            .ok_or_else(|| query_error("release disclosure hold consumed"))?
            .check_hold()
            .map_err(|_| query_error("selected local release changed or revoked"))
    }
    fn take(&mut self) -> Result<ReleaseLease, SearchV2Error> {
        self.check()?;
        self.lease
            .take()
            .ok_or_else(|| query_error("release disclosure hold consumed"))
    }
}
impl InspectDisclosureLease for ReleaseLease {
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
impl InspectCurrentAuthority for Authority {
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
    ) -> Result<Box<dyn InspectDisclosureLease>, SearchV2Error> {
        if scope != &self.inspect
            || (scope.operation_id == O::Contracts.id() && self.registry_grants != 3)
            || (scope.operation_id == O::Dossier.id() && !self.original_granted)
            || (scope.intended_use == tos_query::philosophy_read::PHILOSOPHY_INTENDED_USE
                && !self.philosophy_granted)
        {
            return Err(query_error("selected local disclosure scope incomplete"));
        }
        Ok(Box::new(self.take()?))
    }
}
impl CatalogCurrentAuthority for Authority {
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
    ) -> Result<Box<dyn CatalogDisclosureLease>, CatalogError> {
        if scope != &self.catalog || sha != self.catalog.catalog_packet_sha256 {
            return Err(catalog_error("selected catalog binding changed"));
        }
        Ok(Box::new(self.take().map_err(|_| {
            catalog_error("selected local disclosure hold unavailable")
        })?))
    }
}
