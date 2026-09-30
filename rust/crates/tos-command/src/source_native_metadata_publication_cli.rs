//! Fixed protected initial-Metadata-to-prepared transport. Requests select no paths
//! or grants; the existing source kernel owns authentication and the transaction.
use super::prepared_transport::{DatabaseFence, active, bounded_limits, companion, typed};
use super::{absolute, capped, digest, exact, selected_schema, text};
use crate::source_agent_publication::bootstrap_reviewed_agent_execution_profile_transaction;
use crate::source_claim_publication::{
    ClaimPublicationLimits, ClaimPublicationProgress, ReviewedClaimProfileTransition,
};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use crate::source_metadata_publication::{
    NativeMetadataExecution, ReviewedMetadataProfileTransition,
    publish_committed_initial_metadata_with_precommit,
};
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
use tos_foundation::{Digest256, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::source_cut::CutSchemaExecutor;
const META: usize = 1_048_576;
fn failure(e: impl std::fmt::Debug) -> SourceCommandError {
    let _ = e;
    SourceCommandError::Conflict("Metadata prepared publication transport")
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
        return Err(SourceCommandError::Denied("Metadata unused selectors"));
    }
    let request = serde_json::from_slice::<Value>(&cmd::canonical(&cmd::parse(request_raw)?)?)
        .map_err(failure)?;
    let action = text(&request, "action")?;
    if action != "publish-initial-metadata"
        && [
            "reviewed_metadata_transition",
            "expected_creation_receipt_sha256",
            "expected_creation_request_digest",
        ]
        .iter()
        .any(|key| invocation.get(*key) != Some(&Value::Null))
    {
        return Err(SourceCommandError::Denied(
            "Metadata unused publication selectors",
        ));
    }
    if text(&request, "action")? == "describe-metadata-execution" {
        exact(&request, &["action"])?;
        if invocation.get("reviewed_execution_transition") != Some(&Value::Null) {
            return Err(SourceCommandError::Denied(
                "Metadata describe carries no reviewed transition",
            ));
        }
        let execution = NativeMetadataExecution::observe(deadline, cancelled).map_err(failure)?;
        let result = json!({"processor":execution.processor(),"declaration":execution.common_execution().declaration(),"dependency_implementation":execution.common_execution().dependency_implementation(),"agent_publication":execution.common_execution().agent_publication(),"metadata_publication":execution.profile()});
        execution.common_execution().verify().map_err(failure)?;
        active(deadline, cancelled)?;
        return Ok(
            json!({"schema_version":"tos_local_native_metadata_execution_v1","result":result,"grants_admission":false}),
        );
    }
    let bootstrap = text(&request, "action")? == "reviewed-metadata-execution-bootstrap";
    if bootstrap {
        exact(&request, &["action"])?;
    } else {
        exact(&request, &["action", "recorded_at", "creation_request"])?;
    }
    if !bootstrap && text(&request, "action")? != "publish-initial-metadata" {
        return Err(SourceCommandError::Invalid("Metadata fixed action"));
    }
    let execution = NativeMetadataExecution::observe(deadline, cancelled).map_err(failure)?;
    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let (_filesystem, configuration_raw) =
        CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)?;
    let config = cmd::parse(&configuration_raw)?;
    let family =
        crate::source_creation::CreationFamily::parse(cmd::text(&config, "schema_version")?)?;
    if !matches!(
        family,
        crate::source_creation::CreationFamily::PublicProfile
            | crate::source_creation::CreationFamily::CorpusV1
            | crate::source_creation::CreationFamily::CorpusV2
    ) {
        return Err(SourceCommandError::Denied(
            "Metadata fixed creation owner family",
        ));
    }
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
            max_output_row_bytes: 1_048_576,
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
            .ok_or(SourceCommandError::Invalid("Metadata catalog lenses"))?
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
        false,
    )?;
    // Family-selected limits also bound SQLite scratch before either owned
    // write transaction starts. No on-disk temp store or dirty-cache spilling.
    db.execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cache_size=-8192; PRAGMA cache_spill=OFF")
        .map_err(failure)?;
    let page_size: u64 = db
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .map_err(failure)?;
    if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
        return Err(SourceCommandError::Invalid("Metadata SQLite page size"));
    }
    let max_pages = publication.max_bytes / page_size;
    if max_pages == 0 {
        return Err(SourceCommandError::Invalid("Metadata SQLite page ceiling"));
    }
    let selected_pages: u64 = db
        .query_row(&format!("PRAGMA max_page_count={max_pages}"), [], |r| {
            r.get(0)
        })
        .map_err(failure)?;
    if selected_pages > max_pages {
        return Err(SourceCommandError::Denied(
            "Metadata existing database exceeds selected page ceiling",
        ));
    }
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
                _ => {
                    return Err(SourceCommandError::Invalid(
                        "Metadata reviewed Claim profile",
                    ));
                }
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
            execution.common_execution(),
            &selected_review,
            text(review, "reviewed_after_agent_sha256")?,
            &progress,
            operation,
            publication,
            catalog_limits,
            semantic_limits,
        )
        .map_err(failure)?;
        let (_, configuration_now) =
            CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)?;
        if configuration_now != configuration_raw {
            return Err(SourceCommandError::Conflict(
                "Metadata bootstrap owner configuration changed",
            ));
        }
        execution.common_execution().verify().map_err(failure)?;
        progress.verify(&tx).map_err(failure)?;
        active(deadline, cancelled)?;
        fence.verify()?;
        tx.commit().map_err(failure)?;
        return Ok(
            json!({"schema_version":"tos_local_native_metadata_publication_result_v1", "result":result,"grants_admission":false}),
        );
    }
    if invocation.get("reviewed_execution_transition") != Some(&Value::Null) {
        return Err(SourceCommandError::Denied(
            "Metadata unused common reviewed transition",
        ));
    }
    let review = match &invocation["reviewed_metadata_transition"] {
        Value::Null => None,
        value => {
            exact(value, &["before_sha256", "after_sha256", "review_ref"])?;
            Some(ReviewedMetadataProfileTransition {
                before_sha256: text(value, "before_sha256")?.to_owned(),
                after_sha256: text(value, "after_sha256")?.to_owned(),
                review_ref: text(value, "review_ref")?.to_owned(),
            })
        }
    };
    let creation_request = cmd::canonical(&typed(&request["creation_request"])?)?;
    if cmd::text(&cmd::parse(&creation_request)?, "operation")? != "source.create" {
        return Err(SourceCommandError::Denied(
            "Metadata initial creation request required",
        ));
    }
    let original_context = super::revisions::context(
        &configuration_raw,
        &creation_request,
        text(&request, "recorded_at")?,
        &original,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let context = super::revisions::context(
        &configuration_raw,
        &creation_request,
        text(&request, "recorded_at")?,
        current,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let mut original_worker = selected_schema(invocation, &original, deadline, cancelled)?;
    let prepared = crate::source_creation::prepare_source_creation_from_captures(
        &original_context,
        &original,
        software,
        components,
        &mut original_worker,
        deadline,
        cancelled,
    )?;
    let retained = _filesystem
        .read_creation_retained(&prepared, deadline, cancelled)?
        .ok_or(SourceCommandError::Conflict(
            "Metadata genuine committed creation absent",
        ))?;
    let package = prepared.serialize_retained(
        software,
        components,
        &mut original_worker,
        &retained,
        deadline,
        cancelled,
    )?;
    crate::source_creation_store::finish_creation_worker(
        &mut original_worker,
        deadline,
        cancelled,
    )?;
    let mut current_worker = selected_schema(invocation, current, deadline, cancelled)?;
    let receipt = publish_committed_initial_metadata_with_precommit(
        &db,
        &owner_path,
        &package,
        &original,
        current,
        &context,
        software,
        components,
        digest(text(invocation, "expected_creation_receipt_sha256")?)?,
        digest(text(invocation, "expected_creation_request_digest")?)?,
        &source,
        &binding,
        &catalog,
        review.as_ref(),
        &mut current_worker,
        &vocabulary,
        &descriptor,
        &execution,
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
    )
    .map_err(failure)?;
    Ok(
        json!({"schema_version":"tos_local_native_metadata_publication_result_v1",
              "result":receipt,"grants_admission":false}),
    )
}
