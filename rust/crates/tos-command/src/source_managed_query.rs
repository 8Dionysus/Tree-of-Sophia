//! Existing catalog/navigation delivery inside the real managed source hold.
//! Source currentness adds to, and never replaces, the caller's original and
//! projection-carrier disclosure authority.

use crate::source_managed_selection::ManagedCurrentModelLease;
use std::sync::Arc;
use tos_compiler::ManagedSourceProofV1;
use tos_foundation::Digest256;
use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode};
use tos_query::{
    CatalogCurrentAuthority, CatalogDisclosureLease, CatalogDisclosureScope, CatalogError,
    CatalogErrorCode, IndexedDisclosureScope, InspectCurrentAuthority, InspectDisclosureLease,
    InspectedCarrier, ObservedInspectCarrier,
};

fn catalog_check(
    source: &ManagedCurrentModelLease<'_>,
    proof: &ManagedSourceProofV1,
) -> Result<(), CatalogError> {
    source
        .require_managed_basis(proof)
        .map_err(|_| CatalogError {
            code: CatalogErrorCode::StaleSelection,
            message: "managed source current hold changed",
        })
}
fn inspect_check(
    source: &ManagedCurrentModelLease<'_>,
    proof: &ManagedSourceProofV1,
) -> Result<(), SearchV2Error> {
    source
        .require_managed_basis(proof)
        .map_err(|_| SearchV2Error {
            code: SearchV2ErrorCode::StaleSelection,
            message: "managed source current hold changed",
        })
}

pub(crate) struct ManagedCatalogAuthority<'hold, 'source, 'inner> {
    pub inner: &'hold mut dyn CatalogCurrentAuthority<'inner>,
    pub source: &'hold ManagedCurrentModelLease<'source>,
    pub proof: &'hold ManagedSourceProofV1,
}
struct ManagedCatalogLease<'hold, 'source> {
    inner: Box<dyn CatalogDisclosureLease + 'hold>,
    source: &'hold ManagedCurrentModelLease<'source>,
    proof: &'hold ManagedSourceProofV1,
}
impl CatalogDisclosureLease for ManagedCatalogLease<'_, '_> {
    fn recheck(&mut self) -> Result<(), CatalogError> {
        self.inner.recheck()?;
        catalog_check(self.source, self.proof)
    }
}
impl<'hold, 'source: 'hold, 'inner: 'hold> CatalogCurrentAuthority<'hold>
    for ManagedCatalogAuthority<'hold, 'source, 'inner>
{
    fn authorize_managed_source_current(
        &mut self,
        proof: &ManagedSourceProofV1,
    ) -> Result<(), CatalogError> {
        catalog_check(self.source, proof)
    }
    fn abort_probe(&self) -> Option<Arc<dyn tos_query::AbortProbe>> {
        self.inner.abort_probe()
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.inner.policy_binding()
    }
    fn disclosure_scope(&self) -> CatalogDisclosureScope {
        self.inner.disclosure_scope()
    }
    fn check_selected(&mut self) -> Result<(), CatalogError> {
        catalog_check(self.source, self.proof)?;
        self.inner.check_selected()
    }
    fn authorize_current(&mut self, digest: Digest256) -> Result<(), CatalogError> {
        catalog_check(self.source, self.proof)?;
        self.inner.authorize_current(digest)
    }
    fn acquire_disclosure(
        &mut self,
        scope: &CatalogDisclosureScope,
        digest: Digest256,
    ) -> Result<Box<dyn CatalogDisclosureLease + 'hold>, CatalogError> {
        catalog_check(self.source, self.proof)?;
        let inner = self.inner.acquire_disclosure(scope, digest)?;
        catalog_check(self.source, self.proof)?;
        Ok(Box::new(ManagedCatalogLease {
            inner,
            source: self.source,
            proof: self.proof,
        }))
    }
}

pub(crate) struct ManagedInspectAuthority<'hold, 'source, 'inner> {
    pub inner: &'hold mut dyn InspectCurrentAuthority<'inner>,
    pub source: &'hold ManagedCurrentModelLease<'source>,
    pub proof: &'hold ManagedSourceProofV1,
}
struct ManagedInspectLease<'hold, 'source> {
    inner: Box<dyn InspectDisclosureLease + 'hold>,
    source: &'hold ManagedCurrentModelLease<'source>,
    proof: &'hold ManagedSourceProofV1,
}
impl InspectDisclosureLease for ManagedInspectLease<'_, '_> {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        self.inner.recheck()?;
        inspect_check(self.source, self.proof)
    }
}
impl<'hold, 'source: 'hold, 'inner: 'hold> InspectCurrentAuthority<'hold>
    for ManagedInspectAuthority<'hold, 'source, 'inner>
{
    fn authorize_managed_source_current(
        &mut self,
        proof: &ManagedSourceProofV1,
    ) -> Result<(), SearchV2Error> {
        inspect_check(self.source, proof)
    }
    fn abort_probe(&self) -> Option<Arc<dyn tos_query::AbortProbe>> {
        self.inner.abort_probe()
    }
    fn policy_binding(&self) -> CurrentPolicyBinding {
        self.inner.policy_binding()
    }
    fn disclosure_scope(&self) -> IndexedDisclosureScope {
        self.inner.disclosure_scope()
    }
    fn check_selected(&mut self) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner.check_selected()
    }
    fn authorize_current(&mut self, carrier: &InspectedCarrier) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner.authorize_current(carrier)
    }
    fn acquire_disclosure(
        &mut self,
        scope: &IndexedDisclosureScope,
        consulted: &[ObservedInspectCarrier],
    ) -> Result<Box<dyn InspectDisclosureLease + 'hold>, SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        let inner = self.inner.acquire_disclosure(scope, consulted)?;
        inspect_check(self.source, self.proof)?;
        Ok(Box::new(ManagedInspectLease {
            inner,
            source: self.source,
            proof: self.proof,
        }))
    }
    fn authorize_navigation_original_current(
        &mut self,
        receipt: &tos_compiler::NavigationOriginalReceipt,
        position: i64,
        raw: &[u8],
        digest: Digest256,
    ) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner
            .authorize_navigation_original_current(receipt, position, raw, digest)
    }
    fn authorize_registry_current(
        &mut self,
        path: &str,
        raw: &[u8],
        digest: Digest256,
    ) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner.authorize_registry_current(path, raw, digest)
    }
    fn authorize_catalog_current(&mut self, digest: Digest256) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner.authorize_catalog_current(digest)
    }
    fn authorize_corpus_original_current(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        collection: tos_compiler::CorpusOriginalCollection,
        position: u64,
        raw: &[u8],
        digest: Digest256,
    ) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner
            .authorize_corpus_original_current(receipt, collection, position, raw, digest)
    }
    fn authorize_corpus_view_identity_current(
        &mut self,
        receipt: &tos_compiler::CorpusOriginalReceipt,
        position: u64,
        id: Option<&str>,
        digest: Digest256,
    ) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner
            .authorize_corpus_view_identity_current(receipt, position, id, digest)
    }
    fn authorize_philosophy_original_current(
        &mut self,
        receipt: &tos_compiler::PhilosophyOriginalReceipt,
        collection: tos_compiler::PhilosophyOriginalCollection,
        position: u64,
        raw: &[u8],
        digest: Digest256,
    ) -> Result<(), SearchV2Error> {
        inspect_check(self.source, self.proof)?;
        self.inner
            .authorize_philosophy_original_current(receipt, collection, position, raw, digest)
    }
}

impl crate::source_managed_selection::ManagedAgentSelectedParent {
    /// Use the existing selected query and transport implementations. Every
    /// packet, original/carrier lease and warm model reader is dropped inside
    /// the held-current callback after the real output flush; only an exit code
    /// can return. This does not publish a model or authorize additional scopes.
    pub fn write_current_knowledge<'authority>(
        &self,
        coordinator: &mut crate::durable_adapter::DurablePgCoordinator,
        store: &tos_segment_store::SegmentStore,
        filesystem: &crate::source_creation_store::CreationFilesystem,
        package: &crate::source_creation::ManagedSerializedCreation,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        vocabulary: &tos_compiler::QueryVocabulary,
        descriptor_raw: &[u8],
        catalog: &mut dyn CatalogCurrentAuthority<'authority>,
        inspect: &mut dyn InspectCurrentAuthority<'authority>,
        checkpoints: &mut dyn tos_query::knowledge_exploration::ExplorationCheckpoints,
        request: tos_access::KnowledgeRequest,
        budgets: tos_access::knowledge::SelectedKnowledgeBudgets,
        profile: tos_access::AccessProfile,
        stdout: &mut dyn std::io::Write,
        stderr: &mut dyn std::io::Write,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> crate::source_managed_selection::ManagedSelectionResult<i32> {
        use crate::source_managed_selection::ManagedSelectionError;
        use tos_access::KnowledgeRequest;
        let open_steps = match &request {
            KnowledgeRequest::Catalog => budgets.catalog.max_open_vm_steps,
            KnowledgeRequest::Node { .. } | KnowledgeRequest::Relation { .. } => {
                budgets.inspect.max_open_vm_steps
            }
            _ => {
                return Err(ManagedSelectionError::Source(
                    crate::source_command::SourceCommandError::Unsupported(
                        "managed selected consumer is catalog and navigation node/relation only",
                    ),
                ));
            }
        };
        self.with_current_model(
            coordinator,
            store,
            filesystem,
            package,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
            |model, source| {
                let bound = tos_query::bind_managed_verified_knowledge(
                    model,
                    vocabulary,
                    descriptor_raw,
                    |proof| inspect_check(source, proof),
                )
                .map_err(|error| ManagedSelectionError::Access(error.into()))?;
                let mut reader = model
                    .fork_reader_with_vm_budget(open_steps)
                    .map_err(ManagedSelectionError::Compiler)?;
                let mut catalog = ManagedCatalogAuthority {
                    inner: catalog,
                    source,
                    proof: self.source_proof(),
                };
                let mut inspect = ManagedInspectAuthority {
                    inner: inspect,
                    source,
                    proof: self.source_proof(),
                };
                let packet = tos_access::checked_execute(profile.deadline_probe(), |probe| {
                    tos_access::knowledge::execute_selected_knowledge(
                        &mut reader,
                        &bound,
                        &mut catalog,
                        &mut inspect,
                        checkpoints,
                        request,
                        budgets,
                        probe,
                    )
                })
                .map_err(ManagedSelectionError::Access)?;
                Ok(tos_access::cli::write_packet(
                    packet, profile, stdout, stderr,
                ))
            },
        )
    }
}
