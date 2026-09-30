//! Real committed Record -> prepared Agent publication entry. The source lock
//! is acquired before the DB transaction and retained through final commit.
use crate::{
    source_agent_publication_apply as apply,
    source_agent_publication_profile::NativeAgentExecution,
    source_claim_publication::{ClaimPublicationLimits, ClaimPublicationProgress},
    source_claim_publication_bytes as bytes,
    source_claim_publication_dependencies::Limits as DependencyLimits,
    source_claim_publication_normalize::{ClaimCandidateLimits, ClaimCandidateRegistries},
    source_claim_publication_roots::MutationLimits,
    source_command::CommandContext,
    source_creation_store::{CreationFilesystem, revision_publication::observe_committed},
};
use rusqlite::Connection;
use serde_json::Value;
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use tos_compiler::{
    Error, QueryVocabulary, Result, local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits, prepared_catalog_semantics::CatalogInputs,
    prepared_semantic_index::SemanticMaintenanceLimits,
    prepared_source_binding::PreparedSourceInputs, source_bibliographic::BibliographicLimits,
};
use tos_foundation::{JsonLimits, JsonValue, emit_value_preserved_json};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
const META: usize = 1_048_576;
fn encoded(value: &JsonValue) -> Result<Vec<u8>> {
    emit_value_preserved_json(
        value,
        JsonLimits::new(META, 128, 1_000_000, 4300)
            .map_err(|_| Error::Budget("Agent registry limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))
}

/// Publish one authenticated immediate Record successor using the selected
/// prepared predecessor. Execution-profile migration must be explicitly
/// reviewed beforehand. This function does not execute or undo source commands.
/// Errors before commit roll back the entire DB transaction; immutable COW
/// candidates remain unselected and belong to ordinary owner recovery.
#[allow(clippy::too_many_arguments)]
pub fn publish_committed_agent_correction(
    db: &Connection,
    owner_configuration: &Path,
    context: &CommandContext,
    current: &CorpusCutReader,
    original: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    before_source: &PreparedSourceInputs,
    expected_binding: &JsonValue,
    catalog: &CatalogInputs,
    original_worker: &mut CutWorkerSchemaExecutor,
    current_worker: &mut CutWorkerSchemaExecutor,
    vocabulary: &QueryVocabulary,
    descriptor: &[u8],
    operation: ClaimPublicationLimits,
    bibliographic: BibliographicLimits,
    publication: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    cancelled: Arc<AtomicBool>,
) -> Result<Value> {
    publish_committed_agent_correction_with_precommit(
        db,
        owner_configuration,
        context,
        current,
        original,
        software,
        components,
        before_source,
        expected_binding,
        catalog,
        original_worker,
        current_worker,
        vocabulary,
        descriptor,
        operation,
        bibliographic,
        publication,
        catalog_limits,
        semantic_limits,
        cancelled,
        &mut || Ok(()),
    )
}

/// Same guarded publication with a transport-owned final database fence.
#[allow(clippy::too_many_arguments)]
pub fn publish_committed_agent_correction_with_precommit(
    db: &Connection,
    owner_configuration: &Path,
    context: &CommandContext,
    current: &CorpusCutReader,
    original: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    before_source: &PreparedSourceInputs,
    expected_binding: &JsonValue,
    catalog: &CatalogInputs,
    original_worker: &mut CutWorkerSchemaExecutor,
    current_worker: &mut CutWorkerSchemaExecutor,
    vocabulary: &QueryVocabulary,
    descriptor: &[u8],
    operation: ClaimPublicationLimits,
    bibliographic: BibliographicLimits,
    publication: PublicationLimits,
    catalog_limits: CatalogMaintenanceLimits,
    semantic_limits: SemanticMaintenanceLimits,
    cancelled: Arc<AtomicBool>,
    precommit: &mut dyn FnMut() -> Result<()>,
) -> Result<Value> {
    operation.validate()?;
    publication.validate()?;
    catalog.validate()?;
    if !db.is_autocommit() || descriptor.len() > META {
        return Err(Error::Invalid(
            "Agent requires idle connection and bounded descriptor",
        ));
    }
    let deadline = bibliographic.deadline;
    let (fs, selected_configuration) = CreationFilesystem::select_protected_native_owner(
        owner_configuration,
        deadline,
        &cancelled,
    )
    .map_err(|e| Error::Source(format!("Agent protected owner: {e:?}")))?;
    if selected_configuration != context.configuration_raw {
        return Err(Error::Invalid(
            "Agent protected configuration differs from selected context",
        ));
    }
    let config = crate::source_command::parse(&selected_configuration)
        .map_err(|e| Error::Source(format!("Agent owner configuration: {e:?}")))?;
    let schema = crate::source_command::text(&config, "schema_version")
        .map_err(|e| Error::Source(format!("Agent owner schema: {e:?}")))?;
    crate::source_revisions::RevisionFamily::parse(schema)
        .map_err(|e| Error::Source(format!("Agent requires Record revision owner: {e:?}")))?;

    // Image hashing happens before taking the source mutex. Its descriptor is
    // kept and verified cheaply at the final commit, not hashed per row.
    let execution = NativeAgentExecution::observe(deadline, &cancelled)?;
    execution.verify_source(before_source, catalog)?;
    let entity = encoded(&catalog.entity_registry)?;
    let relation = encoded(&catalog.relation_registry)?;
    let header = bytes::parse(&encoded(&catalog.header)?, META)?;
    let normalization = header["normalization_binding"].clone();
    let registries = ClaimCandidateRegistries {
        entity_bytes: &entity,
        relation_bytes: &relation,
        descriptor_bytes: descriptor,
        vocabulary,
        expected_normalization_binding: &normalization,
    };
    let observation = observe_committed(
        &fs, context, current, original, software, components, deadline, &cancelled,
    )
    .map_err(|e| Error::Source(format!("Agent committed Record: {e:?}")))?;
    let progress =
        ClaimPublicationProgress::install(db, cancelled.clone(), deadline, operation.max_vm_steps)?;
    let tx = db.unchecked_transaction()?;
    let candidate_limits = ClaimCandidateLimits {
        max_nodes: operation.max_nodes,
        max_retained_nodes: operation.max_nodes,
        max_relations: operation.max_relations,
        max_traces: operation.max_claims,
        max_contexts: operation.max_contexts,
        max_row_bytes: operation.max_row_bytes,
        max_input_bytes: operation.max_bytes,
        max_output_bytes: operation.max_bytes,
    };
    let dependencies = DependencyLimits {
        max_claims: operation.max_claims,
        max_read_bytes: operation.max_bytes,
        max_writes: publication.max_mutations,
        ..DependencyLimits::default()
    };
    let mutation = MutationLimits {
        input: operation.max_bytes,
        decoded: operation.max_bytes,
        result: operation.max_bytes,
        ..MutationLimits::default()
    };
    let applied = apply::apply(
        &tx,
        &progress,
        &observation,
        before_source,
        expected_binding,
        catalog,
        original_worker,
        current_worker,
        bibliographic,
        mutation,
        candidate_limits,
        dependencies,
        &registries,
        publication,
        catalog_limits,
        semantic_limits,
        execution.declaration(),
        execution.dependency_implementation(),
        execution.processor(),
        operation.cow_target_bytes,
        &cancelled,
    )?;
    // Consume both real workers' FINAL receipts while rollback is still
    // possible. A transport/schema refusal must never follow a DB commit.
    original_worker
        .finish(deadline, &cancelled)
        .map_err(|e| Error::Source(format!("Agent original worker FINAL: {e:?}")))?;
    current_worker
        .finish(deadline, &cancelled)
        .map_err(|e| Error::Source(format!("Agent current worker FINAL: {e:?}")))?;
    execution.verify()?;
    applied.commit(tx, &progress, &observation, deadline, &cancelled, precommit)
}
