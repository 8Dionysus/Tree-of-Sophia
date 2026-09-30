//! Fixed protected Record-to-prepared Agent transport. Requests select no paths
//! or grants; the existing source kernel owns authentication and the transaction.
use super::prepared_transport::{DatabaseFence, active, bounded_limits, companion, failure, typed};
use super::{absolute, capped, digest, exact, selected_schema, text};
use crate::source_agent_publication::{
    NativeAgentExecution, bootstrap_reviewed_agent_execution_profile_transaction,
    publish_committed_agent_correction_with_precommit,
};
use crate::source_claim_publication::{
    ClaimPublicationLimits, ClaimPublicationProgress, ReviewedClaimProfileTransition,
};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use serde_json::{Value, json};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_compiler::{
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::{CatalogInputs, SourceOrderProfile},
    prepared_semantic_index::SemanticMaintenanceLimits,
    prepared_source_binding::PreparedSourceInputs,
    source_bibliographic::BibliographicLimits,
    source_witness_catalog::SourceCatalogLimits,
};
use tos_foundation::SourceRevision;
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::source_cut::CutSchemaExecutor;
const META: usize = 1_048_576;
// Bootstrap diagnostics retain only static kernel reasons or fixed categories.
// Dynamic source, IO and SQLite messages may contain selected source material.
fn bootstrap_failure(error: tos_compiler::Error) -> SourceCommandError {
    use tos_compiler::Error;
    let reason = match error {
        Error::Invalid(reason)
        | Error::PreparedUnsupported(reason)
        | Error::ManagedSourceUnsupported(reason)
        | Error::Budget(reason) => reason,
        Error::Io(_) => "Agent bootstrap IO",
        Error::Sql(_) | Error::SqlitePhase { .. } => "Agent bootstrap SQLite",
        Error::Source(_) => "Agent bootstrap source validation",
        Error::SqliteVmBudget { .. } => "Agent bootstrap SQLite VM budget",
    };
    SourceCommandError::Conflict(reason)
}
pub(super) fn run(
    invocation: &Value,
    request_raw: &[u8],
    store: &CorpusReader,
    current: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> SourceCommandResult<Value> {
    active(deadline, cancelled)?;
    if invocation.get("owner_context") != Some(&Value::Null)
        || invocation.get("assessment_schema_worker") != Some(&Value::Null)
    {
        return Err(SourceCommandError::Denied("Agent unused selectors"));
    }
    let request = serde_json::from_slice::<Value>(&cmd::canonical(&cmd::parse(request_raw)?)?)
        .map_err(failure)?;
    if text(&request, "action")? == "describe-agent-execution" {
        exact(&request, &["action"])?;
        if invocation.get("reviewed_execution_transition") != Some(&Value::Null) {
            return Err(SourceCommandError::Denied(
                "Agent describe carries no reviewed transition",
            ));
        }
        let execution = NativeAgentExecution::observe(deadline, cancelled).map_err(failure)?;
        let result = json!({"processor":execution.processor(),"declaration":execution.declaration(),"dependency_implementation":execution.dependency_implementation(),"agent_publication":execution.agent_publication()});
        execution.verify().map_err(failure)?;
        active(deadline, cancelled)?;
        return Ok(
            json!({"schema_version":"tos_local_native_agent_execution_v1","result":result,"grants_admission":false}),
        );
    }
    let bootstrap = text(&request, "action")? == "reviewed-agent-execution-bootstrap";
    let inspect = text(&request, "action")? == "inspect-agent-publication";
    if bootstrap {
        exact(&request, &["action"])?;
    } else {
        exact(&request, &["action", "recorded_at", "record_request"])?;
    }
    if !bootstrap && !inspect && text(&request, "action")? != "publish-agent-correction" {
        return Err(SourceCommandError::Invalid("Agent fixed action"));
    }
    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let (_filesystem, configuration_raw) =
        CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)?;
    let config = cmd::parse(&configuration_raw)?;
    crate::source_revisions::RevisionFamily::parse(cmd::text(&config, "schema_version")?)?;
    let root = absolute(cmd::text(&config, "source_root")?)?;
    let uid = rustix::process::getuid().as_raw();
    let budgets = &invocation["budgets"];
    let original = store
        .open_source_cut(
            SourceRevision(digest(text(invocation, "original_source_revision")?)?),
            CutReadLimits {
                max_revisions: capped(budgets, "max_revisions", 4)? as usize,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(failure)?;
    let limits = &invocation["publication_limits"];
    exact(
        limits,
        &[
            "operation",
            "publication",
            "catalog",
            "semantic",
            "bibliographic",
        ],
    )?;
    let op = &limits["operation"];
    exact(
        op,
        &[
            "max_nodes",
            "max_relations",
            "max_claims",
            "max_bytes",
            "max_row_bytes",
            "max_contexts",
            "max_vm_steps",
            "cow_target_bytes",
        ],
    )?;
    let operation = ClaimPublicationLimits {
        max_nodes: capped(op, "max_nodes", 4096)? as usize,
        max_relations: capped(op, "max_relations", 16384)? as usize,
        max_claims: capped(op, "max_claims", 512)? as usize,
        max_bytes: capped(op, "max_bytes", 16_777_216)? as usize,
        max_row_bytes: capped(op, "max_row_bytes", 8_388_608)? as usize,
        max_contexts: capped(op, "max_contexts", 4096)? as usize,
        max_vm_steps: capped(op, "max_vm_steps", 100_000_000)?,
        cow_target_bytes: capped(op, "cow_target_bytes", 262_144)? as usize,
    };
    let publication: PublicationLimits = bounded_limits(&limits["publication"])?;
    let catalog_limits: CatalogMaintenanceLimits = bounded_limits(&limits["catalog"])?;
    let semantic_limits: SemanticMaintenanceLimits = bounded_limits(&limits["semantic"])?;
    let bib = &limits["bibliographic"];
    exact(
        bib,
        &[
            "max_claim_cohort_rows",
            "max_claim_cohort_bytes",
            "max_output_rows",
            "max_output_bytes",
        ],
    )?;
    let bibliographic = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 2048,
            max_rows: 4096,
            max_file_bytes: 16_777_216,
            max_row_bytes: 1_048_576,
            max_contract_bytes: 4_194_304,
            max_output_row_bytes: 32_768,
        },
        max_claim_cohort_rows: capped(bib, "max_claim_cohort_rows", 512)? as usize,
        max_claim_cohort_bytes: capped(bib, "max_claim_cohort_bytes", 16_777_216)? as usize,
        max_output_rows: capped(bib, "max_output_rows", 16384)?,
        max_output_bytes: capped(bib, "max_output_bytes", 16_777_216)?,
        deadline,
    };
    let binding_raw = companion(
        invocation,
        "expected_binding_path",
        None,
        &root,
        uid,
        deadline,
        cancelled,
    )?;
    let binding = cmd::parse(&binding_raw)?;
    let source_raw = companion(
        invocation,
        "source_inputs_path",
        Some("source_inputs_sha256"),
        &root,
        uid,
        deadline,
        cancelled,
    )?;
    let source = PreparedSourceInputs::parse(&source_raw, publication).map_err(failure)?;
    let catalog_raw = companion(
        invocation,
        "catalog_path",
        Some("catalog_sha256"),
        &root,
        uid,
        deadline,
        cancelled,
    )?;
    let cv = serde_json::from_slice::<Value>(&cmd::canonical(&cmd::parse(&catalog_raw)?)?)
        .map_err(failure)?;
    exact(
        &cv,
        &[
            "header",
            "entity_registry",
            "relation_registry",
            "lenses",
            "source_order_profile",
        ],
    )?;
    let catalog = CatalogInputs {
        header: typed(&cv["header"])?,
        entity_registry: typed(&cv["entity_registry"])?,
        relation_registry: typed(&cv["relation_registry"])?,
        lenses: cv["lenses"]
            .as_array()
            .ok_or(SourceCommandError::Invalid("Agent catalog lenses"))?
            .iter()
            .map(typed)
            .collect::<SourceCommandResult<Vec<_>>>()?,
        source_order_profile: serde_json::from_value::<SourceOrderProfile>(
            cv["source_order_profile"].clone(),
        )
        .map_err(failure)?,
    };
    let descriptor = companion(
        invocation,
        "descriptor_path",
        Some("descriptor_sha256"),
        &root,
        uid,
        deadline,
        cancelled,
    )?;
    let vocabulary = tos_compiler::QueryVocabulary::parse(
        &descriptor,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
    )
    .map_err(failure)?;
    let (fence, db) = DatabaseFence::open(
        absolute(text(invocation, "prepared_database")?)?,
        &root,
        uid,
        publication.max_bytes,
        inspect,
    )?;
    if bootstrap {
        let review = &invocation["reviewed_execution_transition"];
        exact(
            review,
            &[
                "dependency_implementation_before",
                "declaration_before",
                "agent_publication_before",
                "claim_publication_before",
                "normalization_before",
                "normalization_after",
                "native_normalization_processor_sha256",
                "review_ref",
                "reviewed_after_agent_sha256",
            ],
        )?;
        let selected_review = ReviewedClaimProfileTransition {
            dependency_implementation_before: text(review, "dependency_implementation_before")?
                .to_owned(),
            declaration_before: text(review, "declaration_before")?.to_owned(),
            agent_publication_before: text(review, "agent_publication_before")?.to_owned(),
            claim_publication_before: match &review["claim_publication_before"] {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                _ => return Err(SourceCommandError::Invalid("Agent reviewed Claim profile")),
            },
            normalization_before: review["normalization_before"].clone(),
            normalization_after: review["normalization_after"].clone(),
            native_normalization_processor_sha256: text(
                review,
                "native_normalization_processor_sha256",
            )?
            .to_owned(),
            review_ref: text(review, "review_ref")?.to_owned(),
        };
        let execution = NativeAgentExecution::observe(deadline, cancelled).map_err(failure)?;
        let progress = ClaimPublicationProgress::install(
            &db,
            cancelled.clone(),
            deadline,
            operation.max_vm_steps,
        )
        .map_err(failure)?;
        let tx = db.unchecked_transaction().map_err(failure)?;
        let result = bootstrap_reviewed_agent_execution_profile_transaction(
            &tx,
            &binding,
            &source,
            &catalog,
            &execution,
            &selected_review,
            text(review, "reviewed_after_agent_sha256")?,
            &progress,
            operation,
            publication,
            catalog_limits,
            semantic_limits,
        )
        .map_err(bootstrap_failure)?;
        let (_, configuration_now) =
            CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)?;
        if configuration_now != configuration_raw {
            return Err(SourceCommandError::Conflict(
                "Agent bootstrap owner configuration changed",
            ));
        }
        execution.verify().map_err(failure)?;
        progress.verify(&tx).map_err(failure)?;
        active(deadline, cancelled)?;
        fence.verify()?;
        tx.commit().map_err(failure)?;
        return Ok(
            json!({"schema_version":"tos_local_native_agent_publication_result_v1", "result":result,"grants_admission":false}),
        );
    }
    if invocation.get("reviewed_execution_transition") != Some(&Value::Null) {
        return Err(SourceCommandError::Denied(
            "Agent unused reviewed transition",
        ));
    }
    let record_request = cmd::canonical(&typed(&request["record_request"])?)?;
    let context = super::revisions::context(
        &configuration_raw,
        &record_request,
        text(&request, "recorded_at")?,
        current,
        software,
        components,
        deadline,
        cancelled,
    )?;
    if inspect {
        let observation = crate::source_creation_store::revision_publication::observe_committed(
            &_filesystem,
            &context,
            current,
            &original,
            software,
            components,
            deadline,
            cancelled,
        )?;
        db.pragma_update(None, "query_only", true)
            .map_err(failure)?;
        let execution = NativeAgentExecution::observe(deadline, cancelled).map_err(failure)?;
        let progress = ClaimPublicationProgress::install(
            &db,
            cancelled.clone(),
            deadline,
            operation.max_vm_steps,
        )
        .map_err(failure)?;
        let tx = db.unchecked_transaction().map_err(failure)?;
        // These are candidates from persisted state, not an authority or receipt.
        // The complete paired/kernel proof below authenticates both selections.
        let mut statement = tx.prepare("SELECT binding,inputs FROM prepared_source_state WHERE singleton=1 AND typeof(binding)='text' AND typeof(inputs)='text' AND length(CAST(binding AS BLOB))<=?1 AND length(CAST(inputs AS BLOB))<=?1 LIMIT 2").map_err(failure)?;
        let mut rows = statement.query([META as u64]).map_err(failure)?;
        let row = rows
            .next()
            .map_err(failure)?
            .ok_or(SourceCommandError::Conflict(
                "Agent selected recovery state",
            ))?;
        let candidate_binding_raw: String = row.get(0).map_err(failure)?;
        let candidate_source_raw: String = row.get(1).map_err(failure)?;
        if rows.next().map_err(failure)?.is_some() {
            return Err(SourceCommandError::Conflict("Agent recovery singleton"));
        }
        drop(rows);
        drop(statement);
        let candidate_binding = cmd::parse(candidate_binding_raw.as_bytes())?;
        let candidate_source =
            PreparedSourceInputs::parse(candidate_source_raw.as_bytes(), publication)
                .map_err(failure)?;
        let candidate_header =
            crate::source_agent_publication_recovery::candidate_header(&tx, META)
                .map_err(failure)?;
        let candidate_catalog = CatalogInputs {
            header: candidate_header,
            ..catalog.clone()
        };
        let receipt = crate::source_agent_publication_recovery::reconcile(
            &tx,
            &progress,
            &observation,
            &candidate_binding,
            &candidate_source,
            &candidate_catalog,
            &execution,
            operation,
            publication,
            deadline,
            cancelled,
        )
        .map_err(failure)?;
        tx.rollback().map_err(failure)?;
        active(deadline, cancelled)?;
        fence.verify()?;
        observation.verify_current(deadline, cancelled)?;
        execution.verify().map_err(failure)?;
        return Ok(
            json!({"schema_version":"tos_local_native_agent_publication_result_v1","result":receipt,"grants_admission":false}),
        );
    }
    let mut original_worker = selected_schema(invocation, &original, deadline, cancelled)?;
    let mut current_worker = selected_schema(invocation, current, deadline, cancelled)?;
    let result = publish_committed_agent_correction_with_precommit(
        &db,
        &owner_path,
        &context,
        current,
        &original,
        software,
        components,
        &source,
        &binding,
        &catalog,
        &mut original_worker,
        &mut current_worker,
        &vocabulary,
        &descriptor,
        operation,
        bibliographic,
        publication,
        catalog_limits,
        semantic_limits,
        cancelled.clone(),
        &mut || {
            active(deadline, cancelled)
                .map_err(|e| tos_compiler::Error::Source(format!("{e:?}")))?;
            fence
                .verify()
                .map_err(|e| tos_compiler::Error::Source(format!("{e:?}")))
        },
    );
    match result {
        Ok(receipt) => Ok(
            json!({"schema_version":"tos_local_native_agent_publication_result_v1","result":receipt,"grants_admission":false}),
        ),
        Err(error) => {
            let _ = original_worker.finish(deadline, cancelled);
            let _ = current_worker.finish(deadline, cancelled);
            Err(failure(error))
        }
    }
}
