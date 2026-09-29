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
use tos_compiler::managed_source::ManagedSourceProofV1;
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

/// An actual complete producer and immutable model, bound to an owner-selected
/// source generation. Neither the model nor a descriptive proof is a public
/// authority constructor, and the raw model cannot escape a current read hold.
pub struct ManagedAgentSelectedParent {
    generation: ManagedCurrentSourceGeneration,
    proof: ManagedSourceProofV1,
    model: VerifiedKnowledgeModel<'static>,
    completed: CompletedManagedAgentProducer,
}
impl ManagedAgentSelectedParent {
    pub fn source_proof(&self) -> &ManagedSourceProofV1 {
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
            &'seek ManagedCurrentModelLease<'seek>,
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
            &'seek ManagedCurrentModelLease<'seek>,
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

fn open_completed_model(
    completed: &CompletedManagedAgentProducer,
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
    if model.source_basis().managed_source() != Some(completed.source_proof()) {
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
fn bind_completed_managed_agent_parent(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    export: ManagedCurrentSourceCut,
    completed: CompletedManagedAgentProducer,
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
) -> ManagedSelectionResult<ManagedAgentSelectedParent> {
    active(deadline, cancelled).map_err(ManagedSelectionError::Source)?;
    let proof = completed.source_proof();
    let membership = export.membership();
    let revision = export.cut().current().revision();
    let projection =
        Digest256::from_hex(&proof.generation.inventory_projection_sha256).map_err(|_| {
            ManagedSelectionError::Source(SourceCommandError::Invalid(
                "managed initial projection digest",
            ))
        })?;
    if proof.delta.is_some()
        || completed.successor_catalogue_input().is_some()
        || completed.created_record_input().is_some()
        || completed.created_forms_input().is_some()
        || completed.export_source_revision() != revision
        || completed.export_membership() != membership
        || proof.initial_export_source_revision != revision.0.to_hex()
        || proof.initial_export_membership_sha256 != membership.digest.to_hex()
        || proof.initial_export_members != membership.count
        || proof.generation
            != DurablePgCoordinator::managed_model_generation_description(
                export.generation(),
                projection,
            )
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
pub fn prepare_managed_agent_selected_parent(
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
        ManagedSourceProofV1,
    ) -> tos_compiler::Result<CompletedManagedAgentProducer>,
) -> ManagedSelectionResult<ManagedAgentSelectedParent> {
    let projection = coordinator
        .visit_managed_model_catalogue(export.generation(), deadline, cancelled, |_| Ok(()))
        .map_err(ManagedSelectionError::Durable)?;
    let membership = export.membership();
    let proof = ManagedSourceProofV1 {
        schema: tos_compiler::managed_source::MANAGED_SOURCE_SCHEMA.to_owned(),
        generation: DurablePgCoordinator::managed_model_generation_description(
            export.generation(),
            projection,
        ),
        initial_export_source_revision: export.cut().current().revision().0.to_hex(),
        initial_export_membership_sha256: membership.digest.to_hex(),
        initial_export_members: membership.count,
        delta: None,
    };
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
pub fn prepare_managed_agent_selected_successor(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    parent: &ManagedAgentSelectedParent,
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
        &CompletedManagedAgentProducer,
        &VerifiedKnowledgeModel<'_>,
        ManagedSourceProofV1,
        &mut dyn FnMut(
            &mut dyn FnMut(&serde_json::Value) -> tos_compiler::Result<()>,
        ) -> tos_compiler::Result<String>,
        &str,
        &[u8],
        Option<&[u8]>,
    ) -> tos_compiler::Result<CompletedManagedAgentProducer>,
) -> ManagedSelectionResult<ManagedAgentSelectedParent> {
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
    let proof = ManagedSourceProofV1 {
        schema: tos_compiler::managed_source::MANAGED_SOURCE_SCHEMA.to_owned(),
        generation: DurablePgCoordinator::managed_model_generation_description(
            &generation,
            projection,
        ),
        initial_export_source_revision: parent.proof.initial_export_source_revision.clone(),
        initial_export_membership_sha256: parent.proof.initial_export_membership_sha256.clone(),
        initial_export_members: parent.proof.initial_export_members,
        delta: Some(ManagedSourceDeltaV1 {
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
    };
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

/// Created only while the source owner holds current PG and filesystem fences.
/// Existing QRY authority callbacks compare their descriptive basis here in
/// addition to their normal original/carrier disclosure authorization.
pub struct ManagedCurrentModelLease<'a> {
    proof: &'a ManagedSourceProofV1,
    owner: &'a CreationOwnerFence<'a>,
    root: &'a AuditedStoreRoot,
    store: &'a SegmentStore,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl<'a> ManagedCurrentModelLease<'a> {
    pub(crate) fn under_held_fences(
        proof: &'a ManagedSourceProofV1,
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
    pub fn require_managed_basis(
        &self,
        supplied: &ManagedSourceProofV1,
    ) -> SourceCommandResult<()> {
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
