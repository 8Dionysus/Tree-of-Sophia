//! Native managed Agent selected-model ownership. Descriptive compiler proofs
//! cannot issue this scoped current read lease or replace source/rights gates.

use crate::durable_adapter::{DurableError, DurablePgCoordinator};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation::{CreationPackage, ManagedSerializedCreation, SerializedCreation};
use crate::source_creation_store::CreationFilesystem;
use crate::source_creation_store::{CreationOwnerFence, active};
use crate::source_current_cut::{ManagedCurrentSourceCut, ManagedCurrentSourceGeneration};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_compiler::managed_source::CompletedManagedAgentProducer;
use tos_compiler::managed_source::{
    ManagedProducerProof, ManagedSourceDeltaV1, ManagedSourceProofV1, ManagedSourceProofV2,
};
use tos_compiler::{
    ColdOpenLimits, LinuxFsVerityCustody, NativeProcessLimits, VerifiedKnowledgeModel,
};
use tos_foundation::Digest256;
use tos_segment_store::{AuditedStoreRoot, SegmentStore};

/// Preserve the actual failing owner boundary rather than reducing a producer
/// or immutable-custody refusal to a permission flag.
#[derive(Debug)]
pub enum ManagedSelectionError {
    Source(SourceCommandError),
    Durable(DurableError),
    Compiler(tos_compiler::Error),
    Access(tos_access::AccessError),
}
pub type ManagedSelectionResult<T> = Result<T, ManagedSelectionError>;

/// Only the two sealed compiler proof variants can cross this owner bridge.
/// Constructors below always derive the generation from the private selected
/// capability; descriptive packets cannot manufacture a current read lease.
pub trait ManagedSelectedProof: ManagedProducerProof + PartialEq {
    fn from_selected(
        generation: &ManagedCurrentSourceGeneration,
        projection: Digest256,
        initial: (&str, &str, u64),
        delta: Option<ManagedSourceDeltaV1>,
    ) -> Result<Self, DurableError>;
    fn matches_selected(
        &self,
        generation: &ManagedCurrentSourceGeneration,
        projection: Digest256,
    ) -> Result<bool, DurableError>;
}
impl ManagedSelectedProof for ManagedSourceProofV1 {
    fn from_selected(
        g: &ManagedCurrentSourceGeneration,
        p: Digest256,
        i: (&str, &str, u64),
        delta: Option<ManagedSourceDeltaV1>,
    ) -> Result<Self, DurableError> {
        Ok(Self {
            schema: tos_compiler::managed_source::MANAGED_SOURCE_SCHEMA.into(),
            generation: DurablePgCoordinator::managed_model_generation_description(g, p)?,
            initial_export_source_revision: i.0.into(),
            initial_export_membership_sha256: i.1.into(),
            initial_export_members: i.2,
            delta,
        })
    }
    fn matches_selected(
        &self,
        g: &ManagedCurrentSourceGeneration,
        p: Digest256,
    ) -> Result<bool, DurableError> {
        Ok(self.generation == DurablePgCoordinator::managed_model_generation_description(g, p)?)
    }
}
impl ManagedSelectedProof for ManagedSourceProofV2 {
    fn from_selected(
        g: &ManagedCurrentSourceGeneration,
        p: Digest256,
        i: (&str, &str, u64),
        delta: Option<ManagedSourceDeltaV1>,
    ) -> Result<Self, DurableError> {
        Ok(Self {
            schema: tos_compiler::managed_source::MANAGED_SOURCE_V2_SCHEMA.into(),
            generation: DurablePgCoordinator::managed_model_generation_description_v2(g, p)?,
            initial_export_source_revision: i.0.into(),
            initial_export_membership_sha256: i.1.into(),
            initial_export_members: i.2,
            delta,
        })
    }
    fn matches_selected(
        &self,
        g: &ManagedCurrentSourceGeneration,
        p: Digest256,
    ) -> Result<bool, DurableError> {
        Ok(self.generation == DurablePgCoordinator::managed_model_generation_description_v2(g, p)?)
    }
}

/// An actual complete producer and immutable model, bound to an owner-selected
/// source generation. Neither the model nor a descriptive proof is a public
/// authority constructor, and the raw model cannot escape a current read hold.
pub struct ManagedAgentSelectedParent<P: ManagedSelectedProof = ManagedSourceProofV1> {
    generation: ManagedCurrentSourceGeneration,
    proof: P,
    model: VerifiedKnowledgeModel<'static>,
    completed: CompletedManagedAgentProducer<P>,
}
impl<P: ManagedSelectedProof> ManagedAgentSelectedParent<P> {
    pub fn source_proof(&self) -> &P {
        &self.proof
    }
    pub fn generation(&self) -> &ManagedCurrentSourceGeneration {
        &self.generation
    }
    /// Descriptive immutable selection facts for independent read-scope
    /// construction. This receipt does not issue disclosure authorization.
    pub fn selection_expectation(&self) -> &tos_compiler::KnowledgeSelectedExpectation {
        self.completed.expectation()
    }
    pub fn navigation_original_receipt(&self) -> &tos_compiler::NavigationOriginalReceipt {
        self.completed.navigation_original()
    }
    /// The genuine initial complete catalogue root retained through this
    /// producer chain. Current catalogue inputs are independently observed.
    pub fn source_catalog_root_sha256(&self) -> &str {
        self.completed.source_catalog_root_sha256()
    }
    pub fn with_current_model<T>(
        &self,
        coordinator: &mut DurablePgCoordinator,
        store: &SegmentStore,
        filesystem: &CreationFilesystem,
        package: &ManagedSerializedCreation,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        seek: impl for<'seek> FnOnce(
            &'seek VerifiedKnowledgeModel<'seek>,
            &'seek ManagedCurrentModelLease<'seek, P>,
        ) -> ManagedSelectionResult<T>,
    ) -> ManagedSelectionResult<T> {
        self.with_current_package(
            coordinator,
            store,
            filesystem,
            CreationPackage::Managed(package),
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
            seek,
        )
    }
    fn with_current_package<T>(
        &self,
        coordinator: &mut DurablePgCoordinator,
        store: &SegmentStore,
        filesystem: &CreationFilesystem,
        package: CreationPackage<'_>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        seek: impl for<'seek> FnOnce(
            &'seek VerifiedKnowledgeModel<'seek>,
            &'seek ManagedCurrentModelLease<'seek, P>,
        ) -> ManagedSelectionResult<T>,
    ) -> ManagedSelectionResult<T> {
        coordinator
            .with_current_managed_model(
                store,
                &self.generation,
                &self.proof,
                filesystem,
                package,
                contract,
                rule_version,
                rights_version,
                job_id,
                job_fence,
                deadline,
                cancelled,
                |lease| {
                    self.model
                        .check_pin()
                        .map_err(ManagedSelectionError::Compiler)?;
                    let result = seek(&self.model, lease);
                    self.model
                        .check_pin()
                        .map_err(ManagedSelectionError::Compiler)?;
                    result
                },
            )
            .map_err(ManagedSelectionError::Durable)?
    }
}

fn open_completed_model<P: ManagedSelectedProof>(
    completed: &CompletedManagedAgentProducer<P>,
    process_limits: NativeProcessLimits,
    cold_limits: ColdOpenLimits,
) -> ManagedSelectionResult<VerifiedKnowledgeModel<'static>> {
    let measurement = tos_compiler::prepare_native_knowledge_artifact(
        completed.artifact_path(),
        completed.stage(),
    )
    .map_err(ManagedSelectionError::Compiler)?;
    let custody = LinuxFsVerityCustody::new(measurement, process_limits)
        .map_err(ManagedSelectionError::Compiler)?;
    let model = tos_compiler::open_selected_knowledge_model_owned(
        completed.artifact_path(),
        completed.expectation().clone(),
        Arc::new(custody),
        cold_limits,
    )
    .map_err(ManagedSelectionError::Compiler)?;
    if model.source_basis() != &completed.source_proof().basis() {
        return Err(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "managed completed model basis differs",
        )));
    }
    Ok(model)
}

/// Only the completed real catalog/navigation invocation can supply the
/// producer outcome. Its genuine export is consumed, so it cannot silently
/// stand for the next mutable managed generation. Cold artifact admission is
/// outside publication locks; the final source/current owner hold is real.
fn bind_completed_managed_agent_parent<P: ManagedSelectedProof>(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    export: ManagedCurrentSourceCut,
    completed: CompletedManagedAgentProducer<P>,
    filesystem: &CreationFilesystem,
    package: &SerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    process_limits: NativeProcessLimits,
    cold_limits: ColdOpenLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ManagedSelectionResult<ManagedAgentSelectedParent<P>> {
    active(deadline, cancelled).map_err(ManagedSelectionError::Source)?;
    let proof = completed.source_proof();
    let membership = export.membership();
    let revision = export.cut().current().revision();
    let projection = Digest256::from_hex(proof.inventory_projection()).map_err(|_| {
        ManagedSelectionError::Source(SourceCommandError::Invalid(
            "managed initial projection digest",
        ))
    })?;
    if proof.delta().is_some()
        || completed.successor_catalogue_input().is_some()
        || completed.created_record_input().is_some()
        || completed.created_forms_input().is_some()
        || completed.export_source_revision() != revision
        || completed.export_membership() != membership
        || proof.initial_export().0 != revision.0.to_hex()
        || proof.initial_export().1 != membership.digest.to_hex()
        || proof.initial_export().2 != membership.count
        || !proof
            .matches_selected(export.generation(), projection)
            .map_err(ManagedSelectionError::Durable)?
    {
        return Err(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "managed complete producer export differs",
        )));
    }
    let model = open_completed_model(&completed, process_limits, cold_limits)?;
    let parent = ManagedAgentSelectedParent {
        generation: export.into_generation(),
        proof: proof.clone(),
        model,
        completed,
    };
    parent.with_current_package(
        coordinator,
        store,
        filesystem,
        CreationPackage::V1(package),
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        deadline,
        cancelled,
        |_, lease| {
            lease
                .require_managed_basis(&parent.proof)
                .map_err(ManagedSelectionError::Source)
        },
    )?;
    Ok(parent)
}

/// Run the compiler's actual full invocation on this private-issued export.
/// The callback can return only its non-constructible completed outcome; the
/// descriptive proof passed to it is rechecked against current owner state.
pub fn prepare_managed_agent_selected_parent<P: ManagedSelectedProof>(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    export: ManagedCurrentSourceCut,
    filesystem: &CreationFilesystem,
    package: &SerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    process_limits: NativeProcessLimits,
    cold_limits: ColdOpenLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    build: impl FnOnce(
        &ManagedCurrentSourceCut,
        P,
    ) -> tos_compiler::Result<CompletedManagedAgentProducer<P>>,
) -> ManagedSelectionResult<ManagedAgentSelectedParent<P>> {
    prepare_managed_agent_selected_parent_with_work(
        coordinator,
        store,
        export,
        filesystem,
        package,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        process_limits,
        cold_limits,
        deadline,
        cancelled,
        &mut crate::ManagedSourceWorkV1::default(),
        build,
    )
}

pub fn prepare_managed_agent_selected_parent_with_work<P: ManagedSelectedProof>(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    export: ManagedCurrentSourceCut,
    filesystem: &CreationFilesystem,
    package: &SerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    process_limits: NativeProcessLimits,
    cold_limits: ColdOpenLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    source_work: &mut crate::ManagedSourceWorkV1,
    build: impl FnOnce(
        &ManagedCurrentSourceCut,
        P,
    ) -> tos_compiler::Result<CompletedManagedAgentProducer<P>>,
) -> ManagedSelectionResult<ManagedAgentSelectedParent<P>> {
    let projection = coordinator
        .visit_managed_model_catalogue_with_work(
            export.generation(),
            deadline,
            cancelled,
            source_work,
            |_| Ok(()),
        )
        .map_err(ManagedSelectionError::Durable)?;
    let membership = export.membership();
    let revision = export.cut().current().revision().0.to_hex();
    let membership_digest = membership.digest.to_hex();
    let proof = P::from_selected(
        export.generation(),
        projection,
        (&revision, &membership_digest, membership.count),
        None,
    )
    .map_err(ManagedSelectionError::Durable)?;
    let completed = build(&export, proof).map_err(ManagedSelectionError::Compiler)?;
    bind_completed_managed_agent_parent(
        coordinator,
        store,
        export,
        completed,
        filesystem,
        package,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        process_limits,
        cold_limits,
        deadline,
        cancelled,
    )
}

/// Continue the actual retained full producer only across the exact committed
/// Agent delta. The old immutable model is construction input, not a stale
/// current read grant. Complete catalogue and global reconstruction run outside
/// publication locks; the returned new parent requires final current fences.
pub fn prepare_managed_agent_selected_successor<P: ManagedSelectedProof>(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    parent: &ManagedAgentSelectedParent<P>,
    generation: ManagedCurrentSourceGeneration,
    prepare_id: &[u8],
    package: &ManagedSerializedCreation,
    filesystem: &CreationFilesystem,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    process_limits: NativeProcessLimits,
    cold_limits: ColdOpenLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    build: impl FnOnce(
        &CompletedManagedAgentProducer<P>,
        &VerifiedKnowledgeModel<'_>,
        P,
        &mut dyn FnMut(
            &mut dyn FnMut(&serde_json::Value) -> tos_compiler::Result<()>,
        ) -> tos_compiler::Result<String>,
        &str,
        &[u8],
        Option<&[u8]>,
    ) -> tos_compiler::Result<CompletedManagedAgentProducer<P>>,
) -> ManagedSelectionResult<ManagedAgentSelectedParent<P>> {
    use crate::source_command as cmd;
    use tos_compiler::managed_source::ManagedSourceDeltaV1;
    active(deadline, cancelled).map_err(ManagedSelectionError::Source)?;
    parent
        .model
        .check_pin()
        .map_err(ManagedSelectionError::Compiler)?;
    let receipt = coordinator
        .managed_model_delta_from_commit(
            store,
            &parent.generation,
            &generation,
            prepare_id,
            package,
            deadline,
            cancelled,
        )
        .map_err(ManagedSelectionError::Durable)?;
    let projection = coordinator
        .managed_model_projection_identity(&generation, deadline, cancelled)
        .map_err(ManagedSelectionError::Durable)?;
    let proof = P::from_selected(
        &generation,
        projection,
        parent.proof.initial_export(),
        Some(ManagedSourceDeltaV1 {
            parent_model_sha256: parent.model.selection().model_sha256.clone(),
            parent_model_size_bytes: parent.model.selection().model_size_bytes,
            parent_source_proof_sha256: parent
                .proof
                .root_sha256()
                .map_err(ManagedSelectionError::Compiler)?,
            parent_through_commit_seq: parent.generation.commit_seq(),
            committed_delta_sha256: receipt.delta_digest.to_hex(),
            committed_member_root_sha256: receipt.member_root.to_hex(),
        }),
    )
    .map_err(ManagedSelectionError::Durable)?;
    let config = cmd::parse(&package.prepared.context().configuration_raw)
        .map_err(ManagedSelectionError::Source)?;
    let record_path = cmd::text(&config, "source_path").map_err(ManagedSelectionError::Source)?;
    let (record_home, record_name) =
        record_path
            .rsplit_once('/')
            .ok_or(ManagedSelectionError::Source(SourceCommandError::Invalid(
                "managed committed source home",
            )))?;
    if record_home != package.prepared.home().as_str() || record_name.is_empty() {
        return Err(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "managed committed source home differs",
        )));
    }
    // The committed creation package owns home-relative file names. Full ToS
    // paths remain the producer's input identity, not package lookup keys.
    let record_raw = package
        .files()
        .get(record_name)
        .ok_or(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "managed committed source buffer absent",
        )))?;
    let forms_path = format!(
        "{}.human-forms.json",
        record_path
            .strip_suffix(".json")
            .ok_or(ManagedSelectionError::Source(SourceCommandError::Invalid(
                "managed source filename"
            )))?
    );
    let forms_name = forms_path.rsplit('/').next().unwrap();
    let forms_raw = package.files().get(forms_name).map(Vec::as_slice);
    let mut owner_failure = None;
    let mut observed_catalogue = None;
    let mut catalogue_called = false;
    let mut catalogue = |accept: &mut dyn FnMut(&serde_json::Value) -> tos_compiler::Result<()>| {
        if catalogue_called {
            return Err(tos_compiler::Error::Invalid(
                "managed catalogue callback already consumed",
            ));
        }
        catalogue_called = true;
        let mut producer_failure = None;
        let mut hash = tos_foundation::Digest256Hasher::new();
        let mut count = 0u64;
        let result =
            coordinator.visit_managed_model_catalogue(&generation, deadline, cancelled, |entry| {
                let raw = cmd::canonical(entry).map_err(DurableError::Source)?;
                let value: serde_json::Value = serde_json::from_slice(&raw)
                    .map_err(|_| DurableError::Invalid("managed catalogue transport codec"))?;
                if let Err(error) = accept(&value) {
                    producer_failure = Some(error);
                    return Err(DurableError::Refused("managed catalogue producer stopped"));
                }
                count = count.checked_add(1).ok_or(DurableError::Invalid(
                    "managed observed catalogue count overflow",
                ))?;
                hash.update(&raw);
                hash.update(b"\n");
                Ok(())
            });
        if let Some(error) = producer_failure {
            return Err(error);
        }
        match result {
            Ok(root) => {
                observed_catalogue = Some((hash.finalize(), count));
                Ok(root.to_hex())
            }
            Err(error) => {
                owner_failure = Some(error);
                Err(tos_compiler::Error::Invalid(
                    "managed catalogue current source unavailable",
                ))
            }
        }
    };
    let completed = build(
        &parent.completed,
        &parent.model,
        proof.clone(),
        &mut catalogue,
        record_path,
        record_raw,
        forms_raw,
    );
    drop(catalogue);
    if let Some(error) = owner_failure {
        return Err(ManagedSelectionError::Durable(error));
    }
    let completed = completed.map_err(ManagedSelectionError::Compiler)?;
    let (catalogue_sha256, catalogue_records) =
        observed_catalogue.ok_or(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "managed complete catalogue not observed",
        )))?;
    let catalogue_sha256 = format!("sha256:{}", catalogue_sha256.to_hex());
    let record_sha256 = Digest256::of_bytes(record_raw).to_hex();
    let forms_sha256 = forms_raw.map(|raw| Digest256::of_bytes(raw).to_hex());
    let expected_forms = forms_raw
        .zip(forms_sha256.as_ref())
        .map(|(raw, sha)| (forms_path.as_str(), sha.as_str(), raw.len() as u64));
    parent
        .model
        .check_pin()
        .map_err(ManagedSelectionError::Compiler)?;
    if completed.source_proof() != &proof
        || completed.export_source_revision() != parent.completed.export_source_revision()
        || completed.export_membership() != parent.completed.export_membership()
        || completed.successor_catalogue_input()
            != Some((catalogue_sha256.as_str(), catalogue_records))
        || completed.created_record_input()
            != Some((record_path, record_sha256.as_str(), record_raw.len() as u64))
        || completed.created_forms_input() != expected_forms
    {
        return Err(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "managed complete successor basis differs",
        )));
    }
    let model = open_completed_model(&completed, process_limits, cold_limits)?;
    let successor = ManagedAgentSelectedParent {
        generation,
        proof,
        model,
        completed,
    };
    successor.with_current_model(
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
        |_, lease| {
            lease
                .require_managed_basis(&successor.proof)
                .map_err(ManagedSelectionError::Source)
        },
    )?;
    Ok(successor)
}

/// One real immutable base shared across addressed successors. This is an
/// operation-owned handle, never a global cache. Historical handles retain
/// their exact manifest/version and base custody independently.
pub struct ManagedAgentOverlayParentV2 {
    base: Arc<VerifiedKnowledgeModel<'static>>,
    generation: ManagedCurrentSourceGeneration,
    manifest: tos_compiler::ManagedManifestV2,
    selected: crate::source_cohort::ManagedManifestSelectionV2,
}
impl ManagedAgentOverlayParentV2 {
    pub fn generation(&self) -> &ManagedCurrentSourceGeneration {
        &self.generation
    }
    pub fn manifest_digest(&self) -> Digest256 {
        self.selected.manifest_digest
    }
    pub fn model_version(&self) -> u64 {
        self.selected.model_version
    }
    pub fn source_binding(&self) -> &tos_compiler::ManagedOverlaySourceBindingV2 {
        self.manifest.source_binding()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn with_current_model<T>(
        &self,
        coordinator: &mut DurablePgCoordinator,
        store: &SegmentStore,
        filesystem: &CreationFilesystem,
        package: &ManagedSerializedCreation,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        manifest_limits: tos_compiler::ManagedManifestLimitsV2,
        deadline: Instant,
        cancelled: &AtomicBool,
        seek: impl for<'seek> FnOnce(
            &tos_compiler::ManagedOverlayReaderV2<'seek, 'static>,
        ) -> ManagedSelectionResult<T>,
    ) -> ManagedSelectionResult<(T, tos_segment_store::AuthenticatedTreeWorkV1)> {
        let (result, work) = coordinator
            .with_current_managed_manifest_v2(
                store,
                self.generation(),
                filesystem,
                CreationPackage::Managed(package),
                contract,
                rule_version,
                rights_version,
                job_id,
                job_fence,
                manifest_limits,
                self.selected,
                deadline,
                cancelled,
                |manifest| {
                    let reader = match tos_compiler::ManagedOverlayReaderV2::new(
                        &self.base,
                        manifest,
                        store,
                        self.generation().selected().audited_root(),
                    ) {
                        Ok(reader) => reader,
                        Err(error) => return Ok(Err(ManagedSelectionError::Compiler(error))),
                    };
                    Ok(seek(&reader))
                },
            )
            .map_err(ManagedSelectionError::Durable)?;
        Ok((result?, work))
    }
}

/// Cold preparation preserves the genuine full producer + admitted base.
/// Store candidate becomes durable before the same-owner PG model CAS. No
/// default V1 caller activates this route; schema enable/recognized backup
/// remains an explicit opt-in owner prerequisite.
#[allow(clippy::too_many_arguments)]
pub fn prepare_managed_agent_overlay_parent_v2<P: ManagedSelectedProof>(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    base: ManagedAgentSelectedParent<P>,
    catalog: &tos_compiler::VersionsCatalogEpochV2,
    filesystem: &CreationFilesystem,
    package: &SerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    manifest_limits: tos_compiler::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ManagedSelectionResult<(
    ManagedAgentOverlayParentV2,
    tos_compiler::CompletedManagedOverlayBaseV2,
    tos_segment_store::AuthenticatedTreeWorkV1,
)> {
    active(deadline, cancelled).map_err(ManagedSelectionError::Source)?;
    let source = DurablePgCoordinator::managed_overlay_binding_v2(&base.generation)
        .map_err(ManagedSelectionError::Durable)?;
    let prepared = tos_compiler::prepare_managed_overlay_base_v2(
        &base.completed,
        &base.model,
        catalog,
        source,
        store,
        base.generation.selected().audited_root(),
        tree_limits,
        manifest_limits,
        deadline,
        cancelled,
    )
    .map_err(ManagedSelectionError::Compiler)?;
    let (selected, cas_work) = coordinator
        .publish_managed_manifest_v2(
            store,
            &base.generation,
            filesystem,
            CreationPackage::V1(package),
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            None,
            prepared.manifest_digest(),
            None,
            manifest_limits,
            deadline,
            cancelled,
        )
        .map_err(ManagedSelectionError::Durable)?;
    let parent = ManagedAgentOverlayParentV2 {
        base: Arc::new(base.model),
        generation: base.generation,
        manifest: prepared.manifest().clone(),
        selected,
    };
    Ok((parent, prepared, cas_work))
}

/// Cold restoration supplies a genuinely newly admitted immutable base, not
/// a reconstructed CompletedProducer. Descriptive expected pointer facts are
/// checked against the actual PG row/history under current owner/source locks.
#[allow(clippy::too_many_arguments)]
pub fn recover_managed_agent_overlay_parent_v2(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    generation: ManagedCurrentSourceGeneration,
    base: VerifiedKnowledgeModel<'static>,
    expected_model_version: u64,
    expected_manifest_digest: Digest256,
    filesystem: &CreationFilesystem,
    package: &ManagedSerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    manifest_limits: tos_compiler::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ManagedSelectionResult<(
    ManagedAgentOverlayParentV2,
    tos_segment_store::AuthenticatedTreeWorkV1,
)> {
    if expected_model_version == 0 {
        return Err(ManagedSelectionError::Source(SourceCommandError::Invalid(
            "restored model version",
        )));
    }
    base.check_pin().map_err(ManagedSelectionError::Compiler)?;
    let selected = crate::source_cohort::ManagedManifestSelectionV2 {
        model_version: expected_model_version,
        manifest_digest: expected_manifest_digest,
    };
    let (checked, work) = coordinator
        .with_current_managed_manifest_v2(
            store,
            &generation,
            filesystem,
            CreationPackage::Managed(package),
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            manifest_limits,
            selected,
            deadline,
            cancelled,
            |manifest| match tos_compiler::ManagedOverlayReaderV2::new(
                &base,
                manifest,
                store,
                generation.selected().audited_root(),
            ) {
                Ok(_) => Ok(Ok(manifest.clone())),
                Err(error) => Ok(Err(ManagedSelectionError::Compiler(error))),
            },
        )
        .map_err(ManagedSelectionError::Durable)?;
    let manifest = checked?;
    base.check_pin().map_err(ManagedSelectionError::Compiler)?;
    Ok((
        ManagedAgentOverlayParentV2 {
            base: Arc::new(base),
            generation,
            manifest,
            selected,
        },
        work,
    ))
}

/// Rebind a genuinely restored immutable model to the actual new PG source
/// selection. The successful existing restore issues the opaque witness;
/// historical proof/base and tree roots are retained, not retagged. A failed
/// equality or changed source state refuses rather than cold-scanning silently.
#[allow(clippy::too_many_arguments)]
pub fn rebind_restored_managed_agent_overlay_v2(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    generation: ManagedCurrentSourceGeneration,
    base: VerifiedKnowledgeModel<'static>,
    old_manifest_digest: Digest256,
    witness: &crate::backup_recovery::VerifiedManagedModelRestoreV2,
    filesystem: &CreationFilesystem,
    package: &ManagedSerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    manifest_limits: tos_compiler::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> ManagedSelectionResult<(
    ManagedAgentOverlayParentV2,
    tos_compiler::CompletedManagedOverlayRecoveryV2,
    tos_segment_store::AuthenticatedTreeWorkV1,
)> {
    rebind_restored_managed_agent_overlay_v2_with_work(
        coordinator,
        store,
        generation,
        base,
        old_manifest_digest,
        witness,
        filesystem,
        package,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        manifest_limits,
        deadline,
        cancelled,
        &mut crate::AuditDeltaWork::default(),
    )
}

pub fn rebind_restored_managed_agent_overlay_v2_with_work(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    generation: ManagedCurrentSourceGeneration,
    base: VerifiedKnowledgeModel<'static>,
    old_manifest_digest: Digest256,
    witness: &crate::backup_recovery::VerifiedManagedModelRestoreV2,
    filesystem: &CreationFilesystem,
    package: &ManagedSerializedCreation,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    manifest_limits: tos_compiler::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
    audit_work: &mut crate::AuditDeltaWork,
) -> ManagedSelectionResult<(
    ManagedAgentOverlayParentV2,
    tos_compiler::CompletedManagedOverlayRecoveryV2,
    tos_segment_store::AuthenticatedTreeWorkV1,
)> {
    active(deadline, cancelled).map_err(ManagedSelectionError::Source)?;
    let source = DurablePgCoordinator::managed_overlay_binding_v2(&generation)
        .map_err(ManagedSelectionError::Durable)?;
    if witness.domain != source.domain
        || witness.new_database_oid != source.database_oid
        || !witness.manifests.contains(&old_manifest_digest.to_hex())
    {
        return Err(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "restored model witness actual source differs",
        )));
    }
    let audited = generation.selected().audited_root();
    let (previous, mut read_work) = tos_compiler::ManagedManifestV2::read_selected(
        store,
        audited,
        old_manifest_digest,
        manifest_limits,
        deadline,
        cancelled,
    )
    .map_err(ManagedSelectionError::Compiler)?;
    let recovery = tos_compiler::ManagedManifestRecoveryRebindV2 {
        parent_manifest_digest: old_manifest_digest.to_hex(),
        parent_source_binding_sha256: previous
            .source_binding_digest()
            .map_err(ManagedSelectionError::Compiler)?
            .to_hex(),
        old_generation_digest: previous.source_binding().selected_generation_digest.clone(),
        backup_receipt_sha256: witness.backup_receipt_sha256.clone(),
        restored_metadata_sha256: witness.metadata_sha256.clone(),
        store_inventory_sha256: witness.store_inventory_sha256.clone(),
        restored_cut_digest: witness.cut_digest.clone(),
        old_database_oid: witness.old_database_oid,
        new_database_oid: witness.new_database_oid,
        old_audit_generation: previous.source_binding().selected_audit_generation,
        new_audit_generation: source.selected_audit_generation,
    };
    let prepared = tos_compiler::prepare_managed_overlay_recovery_rebind_v2(
        &previous,
        old_manifest_digest,
        &base,
        source,
        recovery,
        store,
        audited,
        manifest_limits,
        deadline,
        cancelled,
    )
    .map_err(ManagedSelectionError::Compiler)?;
    let (selected, cas_work) = coordinator
        .publish_managed_manifest_v2_with_work(
            store,
            &generation,
            filesystem,
            CreationPackage::Managed(package),
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            None,
            prepared.manifest_digest(),
            Some(witness),
            manifest_limits,
            deadline,
            cancelled,
            audit_work,
        )
        .map_err(ManagedSelectionError::Durable)?;
    read_work.read_nodes = read_work
        .read_nodes
        .checked_add(cas_work.read_nodes)
        .ok_or(ManagedSelectionError::Source(SourceCommandError::Invalid(
            "recovery read nodes",
        )))?;
    read_work.read_bytes = read_work
        .read_bytes
        .checked_add(cas_work.read_bytes)
        .ok_or(ManagedSelectionError::Source(SourceCommandError::Invalid(
            "recovery read bytes",
        )))?;
    base.check_pin().map_err(ManagedSelectionError::Compiler)?;
    let parent = ManagedAgentOverlayParentV2 {
        base: Arc::new(base),
        generation,
        manifest: prepared.manifest().clone(),
        selected,
    };
    Ok((parent, prepared, read_work))
}

/// Genuine committed package + exact addressed source projection replace the
/// full catalogue visitor. A gap, deletion or unsupported member closure
/// refuses rather than scanning the old full representation invisibly.
#[allow(clippy::too_many_arguments)]
pub fn prepare_managed_agent_overlay_successor_v2(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    parent: &ManagedAgentOverlayParentV2,
    generation: ManagedCurrentSourceGeneration,
    prepare_id: &[u8],
    package: &ManagedSerializedCreation,
    filesystem: &CreationFilesystem,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    manifest_limits: tos_compiler::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
    build: impl FnOnce(
        &tos_compiler::ManagedManifestV2,
        Digest256,
        &VerifiedKnowledgeModel<'_>,
        tos_compiler::ManagedOverlaySourceBindingV2,
        tos_compiler::ManagedManifestDeltaV2,
        &[tos_compiler::ManagedOverlayChangedMemberV2<'_>],
        &serde_json::Value,
        &str,
        &[u8],
        Option<&[u8]>,
    ) -> tos_compiler::Result<tos_compiler::CompletedManagedOverlaySuccessorV2>,
) -> ManagedSelectionResult<(
    ManagedAgentOverlayParentV2,
    tos_compiler::CompletedManagedOverlaySuccessorV2,
    tos_segment_store::AuthenticatedTreeWorkV1,
)> {
    prepare_managed_agent_overlay_successor_v2_with_work(
        coordinator,
        store,
        parent,
        generation,
        prepare_id,
        package,
        filesystem,
        contract,
        rule_version,
        rights_version,
        job_id,
        job_fence,
        manifest_limits,
        deadline,
        cancelled,
        &mut crate::ManagedSourceWorkV1::default(),
        build,
    )
}

pub fn prepare_managed_agent_overlay_successor_v2_with_work(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    parent: &ManagedAgentOverlayParentV2,
    generation: ManagedCurrentSourceGeneration,
    prepare_id: &[u8],
    package: &ManagedSerializedCreation,
    filesystem: &CreationFilesystem,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    manifest_limits: tos_compiler::ManagedManifestLimitsV2,
    deadline: Instant,
    cancelled: &AtomicBool,
    source_work: &mut crate::ManagedSourceWorkV1,
    build: impl FnOnce(
        &tos_compiler::ManagedManifestV2,
        Digest256,
        &VerifiedKnowledgeModel<'_>,
        tos_compiler::ManagedOverlaySourceBindingV2,
        tos_compiler::ManagedManifestDeltaV2,
        &[tos_compiler::ManagedOverlayChangedMemberV2<'_>],
        &serde_json::Value,
        &str,
        &[u8],
        Option<&[u8]>,
    ) -> tos_compiler::Result<tos_compiler::CompletedManagedOverlaySuccessorV2>,
) -> ManagedSelectionResult<(
    ManagedAgentOverlayParentV2,
    tos_compiler::CompletedManagedOverlaySuccessorV2,
    tos_segment_store::AuthenticatedTreeWorkV1,
)> {
    use crate::source_command as cmd;
    active(deadline, cancelled).map_err(ManagedSelectionError::Source)?;
    parent
        .base
        .check_pin()
        .map_err(ManagedSelectionError::Compiler)?;
    let receipt = coordinator
        .managed_model_delta_from_commit(
            store,
            parent.generation(),
            &generation,
            prepare_id,
            package,
            deadline,
            cancelled,
        )
        .map_err(ManagedSelectionError::Durable)?;
    let source = DurablePgCoordinator::managed_overlay_binding_v2(&generation)
        .map_err(ManagedSelectionError::Durable)?;
    // Use compiler's canonical binding digest, not serde's field encounter order.
    let parent_binding = parent
        .manifest
        .source_binding_digest()
        .map_err(ManagedSelectionError::Compiler)?;
    let delta = tos_compiler::ManagedManifestDeltaV2 {
        parent_manifest_digest: parent.selected.manifest_digest.to_hex(),
        parent_source_binding_sha256: parent_binding.to_hex(),
        parent_through_commit_seq: parent.generation().commit_seq(),
        committed_delta_sha256: receipt.delta_digest.to_hex(),
        committed_member_root_sha256: receipt.member_root.to_hex(),
    };
    let config = cmd::parse(&package.prepared.context().configuration_raw)
        .map_err(ManagedSelectionError::Source)?;
    let record_path = cmd::text(&config, "source_path").map_err(ManagedSelectionError::Source)?;
    let (home, name) = record_path
        .rsplit_once('/')
        .ok_or(ManagedSelectionError::Source(SourceCommandError::Invalid(
            "overlay committed source home",
        )))?;
    if home != package.prepared.home().as_str() || name.is_empty() {
        return Err(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "overlay committed home differs",
        )));
    }
    let record_raw = package
        .files()
        .get(name)
        .ok_or(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "overlay committed source absent",
        )))?;
    let form_path = format!(
        "{}.human-forms.json",
        record_path
            .strip_suffix(".json")
            .ok_or(ManagedSelectionError::Source(SourceCommandError::Invalid(
                "overlay source filename"
            )))?
    );
    let forms_raw = package
        .files()
        .get(form_path.rsplit('/').next().unwrap())
        .map(Vec::as_slice);
    // The genuine delta above verifies COMPLETE registered projection/member
    // coverage. Deliver every supported changed member through the addressed
    // reader; auxiliary capture members cannot disappear at the compiler seam.
    let mut changed = Vec::new();
    let mut projection_bytes = 0usize;
    for change in CreationPackage::Managed(package).changes() {
        let raw = change
            .after
            .as_deref()
            .ok_or(ManagedSelectionError::Source(
                SourceCommandError::Unsupported("overlay requires initial Agent members"),
            ))?;
        let projection = coordinator
            .managed_model_addressed_projection_with_work(
                store,
                &generation,
                &change.path,
                deadline,
                cancelled,
                source_work,
            )
            .map_err(ManagedSelectionError::Durable)?
            .ok_or(ManagedSelectionError::Source(SourceCommandError::Conflict(
                "overlay committed projection absent",
            )))?;
        let encoded = cmd::canonical(&projection).map_err(ManagedSelectionError::Source)?;
        projection_bytes = projection_bytes
            .checked_add(encoded.len())
            .filter(|bytes| *bytes <= cmd::SELECTED_SOURCE_MAX_BYTES)
            .ok_or(ManagedSelectionError::Source(
                SourceCommandError::Unsupported(
                    "overlay changed projections exceed existing bounded package envelope",
                ),
            ))?;
        let projection = serde_json::from_slice(&encoded).map_err(|_| {
            ManagedSelectionError::Compiler(tos_compiler::Error::Invalid(
                "overlay projection transport",
            ))
        })?;
        changed.push(tos_compiler::ManagedOverlayChangedMemberV2 {
            path: change.path.as_str(),
            raw,
            projection,
        });
    }
    let projection = &changed
        .iter()
        .find(|member| member.path == record_path)
        .ok_or(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "overlay addressed record projection absent",
        )))?
        .projection;
    let prepared = build(
        &parent.manifest,
        parent.selected.manifest_digest,
        &parent.base,
        source.clone(),
        delta.clone(),
        &changed,
        projection,
        record_path,
        record_raw,
        forms_raw,
    )
    .map_err(ManagedSelectionError::Compiler)?;
    let record_sha = Digest256::of_bytes(record_raw).to_hex();
    let forms_sha = forms_raw.map(|raw| Digest256::of_bytes(raw).to_hex());
    let expected_forms = forms_raw
        .zip(forms_sha.as_ref())
        .map(|(raw, sha)| (form_path.as_str(), sha.as_str(), raw.len() as u64));
    let expected_inputs = changed
        .iter()
        .map(|member| {
            (
                member.path.to_owned(),
                Digest256::of_bytes(member.raw).to_hex(),
                member.raw.len() as u64,
            )
        })
        .collect::<Vec<_>>();
    if prepared.changed_member_inputs() != expected_inputs.as_slice()
        || prepared.manifest().delta() != Some(&delta)
        || prepared.created_record_input()
            != (record_path, record_sha.as_str(), record_raw.len() as u64)
        || prepared.created_forms_input() != expected_forms
        || prepared.manifest().source_binding() != &source
    {
        return Err(ManagedSelectionError::Source(SourceCommandError::Conflict(
            "overlay completed source differs",
        )));
    }
    prepared
        .manifest()
        .require_successor_of(&parent.manifest, parent.selected.manifest_digest)
        .map_err(ManagedSelectionError::Compiler)?;
    let (selected, cas_work) = coordinator
        .publish_managed_manifest_v2(
            store,
            &generation,
            filesystem,
            CreationPackage::Managed(package),
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            None,
            prepared.manifest_digest(),
            None,
            manifest_limits,
            deadline,
            cancelled,
        )
        .map_err(ManagedSelectionError::Durable)?;
    let successor = ManagedAgentOverlayParentV2 {
        base: parent.base.clone(),
        generation,
        manifest: prepared.manifest().clone(),
        selected,
    };
    parent
        .base
        .check_pin()
        .map_err(ManagedSelectionError::Compiler)?;
    Ok((successor, prepared, cas_work))
}

/// Created only while the source owner holds current PG and filesystem fences.
/// Existing QRY authority callbacks compare their descriptive basis here in
/// addition to their normal original/carrier disclosure authorization.
pub struct ManagedCurrentModelLease<'a, P: ManagedSelectedProof = ManagedSourceProofV1> {
    proof: &'a P,
    owner: &'a CreationOwnerFence<'a>,
    root: &'a AuditedStoreRoot,
    store: &'a SegmentStore,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl<'a, P: ManagedSelectedProof> ManagedCurrentModelLease<'a, P> {
    pub(crate) fn under_held_fences(
        proof: &'a P,
        owner: &'a CreationOwnerFence<'a>,
        root: &'a AuditedStoreRoot,
        store: &'a SegmentStore,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self {
            proof,
            owner,
            root,
            store,
            deadline,
            cancelled,
        }
    }

    /// Recheck inside the existing held read boundary. No nested PG transaction
    /// or recursive corpus flock: the caller's short shared locks stay held.
    pub fn require_managed_basis(&self, supplied: &P) -> SourceCommandResult<()> {
        active(self.deadline, self.cancelled)?;
        if supplied != self.proof {
            return Err(SourceCommandError::Conflict(
                "managed selected model source basis differs",
            ));
        }
        self.owner.verify_current(self.deadline, self.cancelled)?;
        self.root.require_store(self.store).map_err(|_| {
            SourceCommandError::Conflict("managed selected model source pin changed")
        })?;
        active(self.deadline, self.cancelled)
    }
}

pub type ManagedCurrentModelLeaseV2<'a> = ManagedCurrentModelLease<'a, ManagedSourceProofV2>;
impl ManagedCurrentModelLease<'_, ManagedSourceProofV2> {
    pub fn require_managed_v2_basis(
        &self,
        supplied: &ManagedSourceProofV2,
    ) -> SourceCommandResult<()> {
        self.require_managed_basis(supplied)
    }
}
