//! Existing catalog/navigation delivery inside the real managed source hold.
//! Source currentness adds to, and never replaces, the caller's original and
//! projection-carrier disclosure authority.

use crate::source_managed_selection::{ManagedCurrentModelLease, ManagedSelectedProof};
use std::sync::Arc;
use tos_compiler::{ManagedProducerProof, ManagedSourceProofV1, ManagedSourceProofV2};
use tos_foundation::Digest256;
use tos_query::search_v2::{CurrentPolicyBinding, SearchV2Error, SearchV2ErrorCode};
use tos_query::{
    CatalogCurrentAuthority, CatalogDisclosureLease, CatalogDisclosureScope, CatalogError,
    CatalogErrorCode, IndexedDisclosureScope, InspectCurrentAuthority, InspectDisclosureLease,
    InspectedCarrier, ObservedInspectCarrier,
};

fn catalog_check<P: ManagedSelectedProof>(
    source: &ManagedCurrentModelLease<'_, P>,
    proof: &P,
) -> Result<(), CatalogError> {
    source
        .require_managed_basis(proof)
        .map_err(|_| CatalogError {
            code: CatalogErrorCode::StaleSelection,
            message: "managed source current hold changed",
        })
}
fn inspect_check<P: ManagedSelectedProof>(
    source: &ManagedCurrentModelLease<'_, P>,
    proof: &P,
) -> Result<(), SearchV2Error> {
    source
        .require_managed_basis(proof)
        .map_err(|_| SearchV2Error {
            code: SearchV2ErrorCode::StaleSelection,
            message: "managed source current hold changed",
        })
}

pub(crate) struct ManagedCatalogAuthority<'hold, 'source, 'inner, P: ManagedSelectedProof> {
    pub inner: &'hold mut dyn CatalogCurrentAuthority<'inner>,
    pub source: &'hold ManagedCurrentModelLease<'source, P>,
    pub proof: &'hold P,
}
struct ManagedCatalogLease<'hold, 'source, P: ManagedSelectedProof> {
    inner: Box<dyn CatalogDisclosureLease + 'hold>,
    source: &'hold ManagedCurrentModelLease<'source, P>,
    proof: &'hold P,
}
impl<P: ManagedSelectedProof> CatalogDisclosureLease for ManagedCatalogLease<'_, '_, P> {
    fn recheck(&mut self) -> Result<(), CatalogError> {
        self.inner.recheck()?;
        catalog_check(self.source, self.proof)
    }
}
impl<'hold, 'source: 'hold, 'inner: 'hold, P: ManagedSelectedProof> CatalogCurrentAuthority<'hold>
    for ManagedCatalogAuthority<'hold, 'source, 'inner, P>
{
    fn authorize_managed_source_current(
        &mut self,
        proof: &ManagedSourceProofV1,
    ) -> Result<(), CatalogError> {
        if self.proof.basis() != proof.basis() {
            return Err(CatalogError {
                code: CatalogErrorCode::StaleSelection,
                message: "managed source proof version/identity differs",
            });
        }
        catalog_check(self.source, self.proof)
    }
    fn authorize_managed_source_v2_current(
        &mut self,
        proof: &ManagedSourceProofV2,
    ) -> Result<(), CatalogError> {
        if self.proof.basis() != proof.basis() {
            return Err(CatalogError {
                code: CatalogErrorCode::StaleSelection,
                message: "managed source proof version/identity differs",
            });
        }
        catalog_check(self.source, self.proof)
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

pub(crate) struct ManagedInspectAuthority<'hold, 'source, 'inner, P: ManagedSelectedProof> {
    pub inner: &'hold mut dyn InspectCurrentAuthority<'inner>,
    pub source: &'hold ManagedCurrentModelLease<'source, P>,
    pub proof: &'hold P,
}
struct ManagedInspectLease<'hold, 'source, P: ManagedSelectedProof> {
    inner: Box<dyn InspectDisclosureLease + 'hold>,
    source: &'hold ManagedCurrentModelLease<'source, P>,
    proof: &'hold P,
}
impl<P: ManagedSelectedProof> InspectDisclosureLease for ManagedInspectLease<'_, '_, P> {
    fn recheck(&mut self) -> Result<(), SearchV2Error> {
        self.inner.recheck()?;
        inspect_check(self.source, self.proof)
    }
}
impl<'hold, 'source: 'hold, 'inner: 'hold, P: ManagedSelectedProof> InspectCurrentAuthority<'hold>
    for ManagedInspectAuthority<'hold, 'source, 'inner, P>
{
    fn authorize_managed_source_current(
        &mut self,
        proof: &ManagedSourceProofV1,
    ) -> Result<(), SearchV2Error> {
        if self.proof.basis() != proof.basis() {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::StaleSelection,
                message: "managed source proof version/identity differs",
            });
        }
        inspect_check(self.source, self.proof)
    }
    fn authorize_managed_source_v2_current(
        &mut self,
        proof: &ManagedSourceProofV2,
    ) -> Result<(), SearchV2Error> {
        if self.proof.basis() != proof.basis() {
            return Err(SearchV2Error {
                code: SearchV2ErrorCode::StaleSelection,
                message: "managed source proof version/identity differs",
            });
        }
        inspect_check(self.source, self.proof)
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

impl<P: ManagedSelectedProof> crate::source_managed_selection::ManagedAgentSelectedParent<P> {
    /// Bind the genuine selected query under the current source hold. The
    /// transport owner executes and flushes inside `consume`; reader, bound
    /// selection and disclosure leases cannot escape that callback. Callback
    /// errors retain their original type without adding a transport dependency.
    #[allow(clippy::too_many_arguments)]
    pub fn with_current_knowledge<'authority, 'vocabulary, T, E>(
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
        vocabulary: &'vocabulary tos_compiler::QueryVocabulary,
        descriptor_raw: &[u8],
        catalog: &mut dyn CatalogCurrentAuthority<'authority>,
        inspect: &mut dyn InspectCurrentAuthority<'authority>,
        open_steps: u64,
        deadline: std::time::Instant,
        cancelled: &std::sync::atomic::AtomicBool,
        consume: impl for<'hold> FnOnce(
            &mut tos_compiler::VerifiedKnowledgeModel<'hold>,
            &tos_query::BoundCmpKnowledge<'vocabulary>,
            &mut dyn CatalogCurrentAuthority<'hold>,
            &mut dyn InspectCurrentAuthority<'hold>,
        ) -> Result<T, E>,
    ) -> crate::source_managed_selection::ManagedSelectionResult<Result<T, E>> {
        use crate::source_managed_selection::ManagedSelectionError;
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
                let bound = tos_query::bind_managed_proof_verified_knowledge(
                    model,
                    vocabulary,
                    descriptor_raw,
                    self.source_proof(),
                    |proof| inspect_check(source, proof),
                )
                .map_err(ManagedSelectionError::Query)?;
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
                Ok(consume(&mut reader, &bound, &mut catalog, &mut inspect))
            },
        )
    }
}
