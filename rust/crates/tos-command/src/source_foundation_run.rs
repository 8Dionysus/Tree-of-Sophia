//! Default district execution over the actual authenticated CMD capture.
//! This phase retains findings and diagnostics; catalog comparison, physical
//! finalization, capture/epoch EOF checks and CLI verdict remain caller work.

use super::foundation_capture::FoundationCapturedCut;
use super::foundation_catalog::{
    EvaluatedPersistedCatalog, FoundationCatalogOutcome, GeneratedCatalogObservation,
};
use super::foundation_payload::FoundationPayloadSources;
use super::foundation_payload::PhysicalPayloadCompletion;
use super::foundation_physical::{FoundationPhysicalSnapshot, PhysicalSourceCost};
use super::foundation_reader::{
    FoundationAuxiliaryCustody, FoundationRuleReadCost, FoundationRuleReadLimits,
    FoundationRuleSource,
};
use super::foundation_rule_diagnostics::{
    EvaluatedSourceFoundationRules, SourceFoundationRuleDiagnosticsError,
    SourceFoundationRuleDiagnosticsLimits,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_ops_mechanics_plan::route_cards::RouteSources;
use tos_source_store::ReadLimits;
use tos_validation::biblio_rules::{SourceCutBiblioReport, inspect_bibliography_from_cut};
use tos_validation::executor::{ExactWorkerIdentity, VerifiedWorkerImageHandle};
use tos_validation::native_compound::NativeRecordHistoryReadObservation;
use tos_validation::record_biblio_cut::BiblioRecordExecutor;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use tos_validation::source_foundation_default_rules::{
    SourceFoundationDefaultRulesLimits,
    inspect_source_foundation_default_rules_with_invalid_artifact_proofs,
};
use tos_validation::source_foundation_discovery::{
    ArtifactCorrectionReplayMap, CurrentArtifactInvalidSchemaProofs, SourcePhysicalFacts,
};
use tos_validation::source_foundation_records::{
    SourceFoundationRecordKernelOutcome, SourceFoundationRecordsLimits,
    SourceFoundationRecordsReport, inspect_source_foundation_records_from_cut_rolling,
};
use tos_validation::source_foundation_schema::{
    SourceFoundationSchemaLimits, SourceFoundationSchemaSet,
};

pub(crate) enum FoundationDefaultReadError {
    Owner(tos_validation::item_rules::ItemRefusal),
    Diagnostics(SourceFoundationRuleDiagnosticsError),
}

/// The complete path sequence belongs to the authenticated capture, rather
/// than the physical selector's smaller set of paths. Its retained logical
/// allocation bound is charged while the later owner reports borrow it.
pub(crate) struct FoundationCurrentPaths {
    pub paths: Vec<String>,
    pub retained_state_upper_bound_bytes: usize,
}

pub(crate) fn captured_current_paths(
    captured: &FoundationCapturedCut,
    max_members: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<FoundationCurrentPaths, FoundationDefaultReadError> {
    let active = || {
        crate::source_creation_store::active(deadline, cancelled).map_err(|_| {
            FoundationDefaultReadError::Owner(if cancelled.load(Ordering::Relaxed) {
                tos_validation::item_rules::ItemRefusal::Source(
                    "foundation invocation cancelled".into(),
                )
            } else {
                tos_validation::item_rules::ItemRefusal::Deadline
            })
        })
    };
    active()?;
    let mut count = 0usize;
    let mut state_bytes = std::mem::size_of::<FoundationCurrentPaths>();
    for (path, _) in captured.observed_members() {
        active()?;
        count = count
            .checked_add(1)
            .filter(|count| *count <= max_members)
            .ok_or(FoundationDefaultReadError::Owner(
                tos_validation::item_rules::ItemRefusal::Budget,
            ))?;
        state_bytes = state_bytes
            .checked_add(std::mem::size_of::<String>())
            .and_then(|bytes| bytes.checked_add(path.len()))
            .filter(|bytes| *bytes <= max_state_bytes)
            .ok_or(FoundationDefaultReadError::Owner(
                tos_validation::item_rules::ItemRefusal::Budget,
            ))?;
    }
    if state_bytes > max_state_bytes {
        return Err(FoundationDefaultReadError::Owner(
            tos_validation::item_rules::ItemRefusal::Budget,
        ));
    }
    let mut paths = Vec::new();
    paths.try_reserve_exact(count).map_err(|_| {
        FoundationDefaultReadError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
    })?;
    for (path, _) in captured.observed_members() {
        active()?;
        paths.push(path.to_owned());
    }
    Ok(FoundationCurrentPaths {
        paths,
        retained_state_upper_bound_bytes: state_bytes,
    })
}

pub(crate) struct EvaluatedFoundationDefault<'cancel> {
    pub rules: EvaluatedSourceFoundationRules,
    pub bibliography: FoundationBiblioEvidence,
    /// Includes auxiliary EOF rereads; overlaps the assembler's counted source
    /// reads and must not be added twice to those subset measurements.
    pub reader_cost: FoundationRuleReadCost,
    /// Must survive every later worker and be rechecked before the CLI verdict.
    pub auxiliary_custody: FoundationAuxiliaryCustody<'cancel>,
}

/// The genuine conditional Claim traversal shares the completed Records scan.
/// Its state ceiling is reserved before later districts; it is not measured RSS.
pub(crate) enum FoundationBiblioEvidence {
    NotSelected,
    RecordsIncomplete,
    Complete {
        report: SourceCutBiblioReport,
        state_reservation_bytes: usize,
    },
}

/// Execute the single Records scan before conditional retained Artifact
/// reconstruction. The caller keeps this exact report alive during replay and
/// then moves it into `evaluate_default`; neither phase reconstructs a second
/// current-record view.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_records(
    captured: &FoundationCapturedCut,
    payloads: &mut FoundationPayloadSources<'_>,
    record_executor: &mut BiblioRecordExecutor,
    item_schemas: &mut CutWorkerSchemaExecutor,
    physical: &SourcePhysicalFacts,
    require_local_payloads: bool,
    limits: SourceFoundationRecordsLimits,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationRecordsReport, FoundationDefaultReadError> {
    inspect_source_foundation_records_from_cut_rolling(
        captured.cut(),
        captured.revision(),
        captured.membership(),
        limits,
        require_local_payloads,
        cancelled,
        record_executor,
        item_schemas,
        physical,
        payloads,
    )
    .map_err(FoundationDefaultReadError::Owner)
}

pub(crate) enum FoundationFinalInputError {
    Owner(tos_validation::item_rules::ItemRefusal),
    Catalog(tos_compiler::Error),
    Capture(crate::source_command::SourceCommandError),
}

/// Custody completion only. Owner coverage gaps and schema rejection remain in
/// the returned reports and must be resolved before formatting a CLI verdict.
pub(crate) struct FinalizedFoundationDefaultInputs {
    pub rules: EvaluatedSourceFoundationRules,
    pub bibliography: FoundationBiblioEvidence,
    pub catalog: FoundationCatalogOutcome,
    pub persisted_catalog: EvaluatedPersistedCatalog,
    pub reader_cost: FoundationRuleReadCost,
    pub physical_cost: PhysicalSourceCost,
    pub payload_completion: PhysicalPayloadCompletion,
    pub final_authored_member_read_bytes: usize,
    /// Existing shared lookup allowance, sampled after capture/epoch EOF.
    /// Caller must construct auxiliary roots with `new_until_related`.
    pub route_operations_after_eof: usize,
}

/// Invoke after the last schema/catalog/bibliographic worker has terminated;
/// no dependent worker may be started between this function and CLI output.
/// Every selected namespace keeps its original budgets and whole deadline.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_default_inputs(
    evaluated: EvaluatedFoundationDefault<'_>,
    catalog: FoundationCatalogOutcome,
    persisted_catalog: EvaluatedPersistedCatalog,
    captured: &FoundationCapturedCut,
    sources: &mut RouteSources,
    artifact_sources: Option<&mut RouteSources>,
    physical: &mut FoundationPhysicalSnapshot<'_, '_>,
    payloads: FoundationPayloadSources<'_>,
    read_limits: ReadLimits,
    max_final_authored_member_read_bytes: usize,
    max_final_source_read_bytes: u64,
    max_final_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<FinalizedFoundationDefaultInputs, FoundationFinalInputError> {
    finalize_default_inputs_inner(
        evaluated,
        catalog,
        persisted_catalog,
        captured,
        sources,
        artifact_sources,
        physical,
        payloads,
        read_limits,
        max_final_authored_member_read_bytes,
        max_final_source_read_bytes,
        max_final_state_bytes,
        deadline,
        cancelled,
        None,
    )
}

/// Candidate catalogs are generated in their disposable held namespace;
/// grammar custody remains on the original primary root.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_candidate_inputs(
    evaluated: EvaluatedFoundationDefault<'_>,
    catalog: FoundationCatalogOutcome,
    persisted_catalog: EvaluatedPersistedCatalog,
    captured: &FoundationCapturedCut,
    sources: &mut RouteSources,
    artifact_sources: Option<&mut RouteSources>,
    physical: &mut FoundationPhysicalSnapshot<'_, '_>,
    payloads: FoundationPayloadSources<'_>,
    read_limits: ReadLimits,
    max_final_authored_member_read_bytes: usize,
    max_final_source_read_bytes: u64,
    max_final_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    fresh_catalog_sources: &mut RouteSources,
) -> Result<FinalizedFoundationDefaultInputs, FoundationFinalInputError> {
    if captured.cost().candidate_copy_read_bytes.is_none() {
        return Err(FoundationFinalInputError::Capture(
            crate::source_command::SourceCommandError::Denied(
                "candidate finalization requires candidate capture",
            ),
        ));
    }
    finalize_default_inputs_inner(
        evaluated,
        catalog,
        persisted_catalog,
        captured,
        sources,
        artifact_sources,
        physical,
        payloads,
        read_limits,
        max_final_authored_member_read_bytes,
        max_final_source_read_bytes,
        max_final_state_bytes,
        deadline,
        cancelled,
        Some(fresh_catalog_sources),
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_default_inputs_inner(
    evaluated: EvaluatedFoundationDefault<'_>,
    mut catalog: FoundationCatalogOutcome,
    mut persisted_catalog: EvaluatedPersistedCatalog,
    captured: &FoundationCapturedCut,
    sources: &mut RouteSources,
    artifact_sources: Option<&mut RouteSources>,
    physical: &mut FoundationPhysicalSnapshot<'_, '_>,
    mut payloads: FoundationPayloadSources<'_>,
    read_limits: ReadLimits,
    max_final_authored_member_read_bytes: usize,
    max_final_source_read_bytes: u64,
    max_final_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    fresh_catalog_sources: Option<&mut RouteSources>,
) -> Result<FinalizedFoundationDefaultInputs, FoundationFinalInputError> {
    crate::source_creation_store::active(deadline, cancelled)
        .map_err(FoundationFinalInputError::Capture)?;
    let EvaluatedFoundationDefault {
        rules,
        bibliography,
        mut auxiliary_custody,
        ..
    } = evaluated;
    auxiliary_custody
        .verify_context(deadline, cancelled)
        .map_err(FoundationFinalInputError::Owner)?;
    if auxiliary_custody.deadline() > deadline
        || physical.deadline() > deadline
        || payloads.deadline() > deadline
        || sources.deadline() > deadline
        || artifact_sources
            .as_deref()
            .is_some_and(|root| root.deadline() > deadline)
    {
        return Err(FoundationFinalInputError::Owner(
            tos_validation::item_rules::ItemRefusal::Source(
                "foundation final input deadline exceeds whole invocation".into(),
            ),
        ));
    }
    let capture_reserved = u64::try_from(max_final_authored_member_read_bytes).map_err(|_| {
        FoundationFinalInputError::Capture(crate::source_command::SourceCommandError::Denied(
            "foundation final capture allowance exceeds platform size",
        ))
    })?;
    let mut remaining_reads = max_final_source_read_bytes
        .checked_sub(capture_reserved)
        .ok_or_else(|| {
            FoundationFinalInputError::Capture(crate::source_command::SourceCommandError::Denied(
                "final capture reservation exceeds remaining source reads",
            ))
        })?;

    let payload_before = payloads.cost();
    let payload_state_cap = payload_before
        .peak_state_bytes
        .checked_add(max_final_state_bytes)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    payloads
        .restrict_remaining_budget(remaining_reads, payload_state_cap, deadline, cancelled)
        .map_err(FoundationFinalInputError::Owner)?;
    let payload_completion = payloads
        .finish_with_cost()
        .map_err(FoundationFinalInputError::Owner)?;
    let payload_final_read = payload_completion
        .cost
        .final_bytes_read
        .checked_sub(payload_before.final_bytes_read)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    remaining_reads = remaining_reads
        .checked_sub(payload_final_read)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    let mut remaining_state = max_final_state_bytes
        .checked_sub(payload_completion.cost.final_facts_state_bytes)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;

    let physical_before = physical.cost();
    let physical_state_cap = physical_before
        .retained_state_bytes
        .checked_add(remaining_state)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    physical
        .restrict_remaining_budget(
            usize::try_from(remaining_reads).unwrap_or(usize::MAX),
            physical_state_cap,
            deadline,
            cancelled,
        )
        .map_err(FoundationFinalInputError::Owner)?;
    physical
        .recheck(sources, artifact_sources)
        .map_err(FoundationFinalInputError::Owner)?;
    let physical_cost = physical.cost();
    let physical_final_read = physical_cost
        .bytes_read
        .checked_sub(physical_before.bytes_read)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    remaining_reads = remaining_reads
        .checked_sub(physical_final_read)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    let physical_final_state = physical_cost
        .retained_state_bytes
        .checked_sub(physical_before.retained_state_bytes)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    remaining_state = remaining_state
        .checked_sub(physical_final_state)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;

    let reader_before = auxiliary_custody.bytes_read();
    let auxiliary_state_cap = auxiliary_custody
        .retained_state_bytes()
        .checked_add(remaining_state)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    let reader_cost = auxiliary_custody
        .recheck_until_with_limits(
            sources,
            remaining_reads,
            auxiliary_state_cap,
            deadline,
            cancelled,
        )
        .map_err(FoundationFinalInputError::Owner)?;
    let reader_final_read = reader_cost
        .bytes_read
        .checked_sub(reader_before)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    remaining_reads = remaining_reads
        .checked_sub(reader_final_read)
        .ok_or_else(|| {
            FoundationFinalInputError::Owner(tos_validation::item_rules::ItemRefusal::Budget)
        })?;
    {
        let generated_sources = fresh_catalog_sources.unwrap_or(&mut *sources);
        if generated_sources.deadline() > deadline {
            return Err(FoundationFinalInputError::Capture(
                crate::source_command::SourceCommandError::Denied(
                    "catalog finalization deadline exceeds invocation",
                ),
            ));
        }
        match &mut catalog {
            FoundationCatalogOutcome::Complete(result) => recheck_generated_with_remaining(
                &mut result.generated_inputs,
                generated_sources,
                &mut remaining_reads,
                deadline,
                cancelled,
            )
            .map_err(FoundationFinalInputError::Catalog)?,
            FoundationCatalogOutcome::SchemaRejected {
                generated_inputs, ..
            } => {
                if let Some(selected) = generated_inputs {
                    recheck_generated_with_remaining(
                        selected,
                        generated_sources,
                        &mut remaining_reads,
                        deadline,
                        cancelled,
                    )
                    .map_err(FoundationFinalInputError::Catalog)?;
                }
            }
        }
        match &mut catalog {
            FoundationCatalogOutcome::Complete(result) => {
                result.generated_read_bytes = result.generated_inputs.read_bytes();
            }
            FoundationCatalogOutcome::SchemaRejected {
                generated_inputs,
                generated_read_bytes,
                ..
            } => {
                if let Some(selected) = generated_inputs {
                    *generated_read_bytes = selected.read_bytes();
                }
            }
        }
        recheck_generated_with_remaining(
            &mut persisted_catalog.generated_inputs,
            generated_sources,
            &mut remaining_reads,
            deadline,
            cancelled,
        )
        .map_err(FoundationFinalInputError::Catalog)?;
    }
    let final_authored_member_read_bytes = if captured.cost().candidate_copy_read_bytes.is_some() {
        captured.recheck_candidate_transport_with_control_budget(
            sources,
            max_final_authored_member_read_bytes,
            deadline,
            cancelled,
        )
    } else {
        captured.recheck_with_control_budget(
            sources,
            read_limits,
            max_final_authored_member_read_bytes,
            deadline,
            cancelled,
        )
    }
    .map_err(FoundationFinalInputError::Capture)?;
    if u64::try_from(final_authored_member_read_bytes).unwrap_or(u64::MAX) > capture_reserved {
        return Err(FoundationFinalInputError::Capture(
            crate::source_command::SourceCommandError::Denied(
                "final authored read exceeded its reserved allowance",
            ),
        ));
    }
    Ok(FinalizedFoundationDefaultInputs {
        rules,
        bibliography,
        catalog,
        persisted_catalog,
        reader_cost,
        physical_cost,
        payload_completion,
        final_authored_member_read_bytes,
        route_operations_after_eof: sources.operation_count(),
    })
}

fn recheck_generated_with_remaining(
    observation: &mut GeneratedCatalogObservation,
    sources: &mut RouteSources,
    remaining_reads: &mut u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> tos_compiler::Result<()> {
    let before = observation.read_bytes();
    observation.restrict_remaining_read_budget(
        usize::try_from(*remaining_reads)
            .map_err(|_| tos_compiler::Error::Budget("generated final read allowance"))?,
        deadline,
        cancelled,
    )?;
    observation.recheck(sources, deadline, cancelled)?;
    let delta = observation
        .read_bytes()
        .checked_sub(before)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(tos_compiler::Error::Budget(
            "generated final read accounting",
        ))?;
    *remaining_reads = remaining_reads
        .checked_sub(delta)
        .ok_or(tos_compiler::Error::Budget(
            "generated final read allowance",
        ))?;
    Ok(())
}

/// Consume genuine selected owner inputs in maintained district order. The
/// native-history map belongs only to conditional Discovery consumers; no
/// unconditional current-record history requirement is introduced here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_default<'cancel>(
    captured: &FoundationCapturedCut,
    current_paths: &[String],
    records: SourceFoundationRecordsReport,
    sources: &mut RouteSources,
    payloads: &mut FoundationPayloadSources<'_>,
    record_executor: &mut BiblioRecordExecutor,
    item_schemas: &mut CutWorkerSchemaExecutor,
    layer_schemas: &mut dyn CutSchemaExecutor,
    physical: &SourcePhysicalFacts,
    histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    invalid_artifact_proofs: &CurrentArtifactInvalidSchemaProofs<'_>,
    require_local_payloads: bool,
    record_limits: SourceFoundationRecordsLimits,
    mut bibliography_limits: tos_validation::item_rules::ItemLimits,
    reader_limits: FoundationRuleReadLimits,
    mut district_limits: SourceFoundationDefaultRulesLimits,
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    worker_image: &VerifiedWorkerImageHandle,
    schema_limits: SourceFoundationSchemaLimits,
    diagnostic_limits: SourceFoundationRuleDiagnosticsLimits,
    quota: &tos_validation::executor::SharedSchemaWorkerQuota,
    history: Option<&mut (dyn super::foundation_reader::FoundationHistoricalEvidence + 'static)>,
    cancelled: &'cancel AtomicBool,
) -> Result<EvaluatedFoundationDefault<'cancel>, FoundationDefaultReadError> {
    let deadline = district_limits.operation.deadline;
    if record_limits.operation.deadline > deadline
        || record_limits.records.deadline > deadline
        || record_limits.items.deadline > deadline
        || reader_limits.deadline > deadline
        || bibliography_limits.deadline > deadline
    {
        return Err(FoundationDefaultReadError::Owner(
            tos_validation::item_rules::ItemRefusal::Source(
                "foundation district deadline exceeds whole invocation".into(),
            ),
        ));
    }
    if schema_set.source_revision() != captured.revision() {
        return Err(FoundationDefaultReadError::Owner(
            tos_validation::item_rules::ItemRefusal::Source(
                "foundation schema resources differ from captured source revision".into(),
            ),
        ));
    }
    let mut selected_members = captured.cut().current().members();
    for path in current_paths {
        if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
            return Err(FoundationDefaultReadError::Owner(
                if cancelled.load(Ordering::Relaxed) {
                    tos_validation::item_rules::ItemRefusal::Source(
                        "foundation invocation cancelled".into(),
                    )
                } else {
                    tos_validation::item_rules::ItemRefusal::Deadline
                },
            ));
        }
        if selected_members
            .next()
            .is_none_or(|member| member.path.as_str() != path)
        {
            return Err(FoundationDefaultReadError::Owner(
                tos_validation::item_rules::ItemRefusal::Source(
                    "foundation current paths differ from captured membership".into(),
                ),
            ));
        }
    }
    if selected_members.next().is_some() {
        return Err(FoundationDefaultReadError::Owner(
            tos_validation::item_rules::ItemRefusal::Source(
                "foundation current paths differ from captured membership".into(),
            ),
        ));
    }
    if records.source_revision != captured.revision()
        || records.source_membership != captured.membership()
    {
        return Err(FoundationDefaultReadError::Owner(
            tos_validation::item_rules::ItemRefusal::Source(
                "foundation Records report differs from captured source".into(),
            ),
        ));
    }
    let bibliography_selected =
        tos_validation::source_foundation_closure::source_foundation_requires_bibliographic(
            current_paths,
            &records.used_declared_profile_kinds,
        );
    let bibliography = if !bibliography_selected {
        FoundationBiblioEvidence::NotSelected
    } else if let SourceFoundationRecordKernelOutcome::Complete(record_report) = &records.records {
        let record_read_reservation = records
            .cost
            .record_observed_read_bytes
            .unwrap_or(records.cost.record_read_limit_bytes)
            .checked_add(records.cost.item_observed_read_bytes)
            .and_then(|bytes| bytes.checked_add(records.cost.item_schema_resource_bytes))
            .ok_or(FoundationDefaultReadError::Owner(
                tos_validation::item_rules::ItemRefusal::Budget,
            ))?;
        let rolling = records.cost.rolling_accounted_state_upper_bound_bytes;
        if let Some(record_state) = rolling {
            bibliography_limits.max_state_bytes = bibliography_limits.max_state_bytes.min(
                district_limits
                    .operation
                    .max_state_bytes
                    .checked_sub(record_state)
                    .ok_or(FoundationDefaultReadError::Owner(
                        tos_validation::item_rules::ItemRefusal::Budget,
                    ))?,
            );
            bibliography_limits.max_total_bytes = bibliography_limits.max_total_bytes.min(
                district_limits
                    .operation
                    .max_total_bytes
                    .checked_sub(record_read_reservation)
                    .ok_or(FoundationDefaultReadError::Owner(
                        tos_validation::item_rules::ItemRefusal::Budget,
                    ))?,
            );
            let record_issues = records.cost.rolling_observed_issue_count.ok_or(
                FoundationDefaultReadError::Owner(tos_validation::item_rules::ItemRefusal::Budget),
            )?;
            bibliography_limits.max_issues = bibliography_limits.max_issues.min(
                district_limits
                    .operation
                    .max_issues
                    .checked_sub(record_issues)
                    .ok_or(FoundationDefaultReadError::Owner(
                        tos_validation::item_rules::ItemRefusal::Budget,
                    ))?,
            );
        } else {
            if records
                .cost
                .operation_state_limit_bytes
                .checked_add(bibliography_limits.max_state_bytes)
                .is_none_or(|bytes| bytes > district_limits.operation.max_state_bytes)
                || record_read_reservation
                    .checked_add(bibliography_limits.max_total_bytes)
                    .is_none_or(|bytes| bytes > district_limits.operation.max_total_bytes)
                || record_limits
                    .operation
                    .max_issues
                    .checked_add(bibliography_limits.max_issues)
                    .is_none_or(|issues| issues > district_limits.operation.max_issues)
            {
                return Err(FoundationDefaultReadError::Owner(
                    tos_validation::item_rules::ItemRefusal::Budget,
                ));
            }
        }
        let report = inspect_bibliography_from_cut(
            captured.cut(),
            record_report,
            bibliography_limits,
            cancelled,
            item_schemas,
        )
        .map_err(FoundationDefaultReadError::Owner)?;
        let bibliography_retained_state = if rolling.is_some() {
            report.accounted_state_upper_bound_bytes
        } else {
            bibliography_limits.max_state_bytes
        };
        district_limits.operation.max_total_bytes = district_limits
            .operation
            .max_total_bytes
            .checked_sub(report.bytes_read)
            .ok_or(FoundationDefaultReadError::Owner(
                tos_validation::item_rules::ItemRefusal::Budget,
            ))?;
        district_limits.operation.max_state_bytes = district_limits
            .operation
            .max_state_bytes
            .checked_sub(bibliography_retained_state)
            .ok_or(FoundationDefaultReadError::Owner(
                tos_validation::item_rules::ItemRefusal::Budget,
            ))?;
        district_limits.operation.max_issues = district_limits
            .operation
            .max_issues
            .checked_sub(report.shadow.issues.len())
            .ok_or(FoundationDefaultReadError::Owner(
                tos_validation::item_rules::ItemRefusal::Budget,
            ))?;
        FoundationBiblioEvidence::Complete {
            report,
            state_reservation_bytes: bibliography_retained_state,
        }
    } else {
        FoundationBiblioEvidence::RecordsIncomplete
    };
    let claims = match &bibliography {
        FoundationBiblioEvidence::Complete { report, .. } => report.claims.as_slice(),
        FoundationBiblioEvidence::NotSelected | FoundationBiblioEvidence::RecordsIncomplete => &[],
    };
    let mut source = FoundationRuleSource::new(
        captured.cut(),
        sources,
        layer_schemas,
        payloads,
        cancelled,
        reader_limits,
    )
    .map_err(FoundationDefaultReadError::Owner)?;
    if let Some(history) = history {
        source = source
            .with_history(history)
            .map_err(FoundationDefaultReadError::Owner)?;
    }
    let report = inspect_source_foundation_default_rules_with_invalid_artifact_proofs(
        &mut source,
        captured.cut(),
        current_paths,
        records,
        histories,
        claims,
        physical,
        artifact_replays,
        invalid_artifact_proofs,
        require_local_payloads,
        district_limits,
    )
    .map_err(FoundationDefaultReadError::Owner)?;
    let rules = super::foundation_rule_diagnostics::evaluate_with_shared_quota_and_image(
        report,
        schema_set,
        worker,
        worker_image,
        schema_limits,
        deadline,
        cancelled,
        diagnostic_limits,
        quota,
    )
    .map_err(FoundationDefaultReadError::Diagnostics)?;
    source
        .recheck_auxiliary()
        .map_err(FoundationDefaultReadError::Owner)?;
    let reader_cost = source.cost();
    let auxiliary_custody = source.into_auxiliary_custody();
    record_executor
        .finish(deadline, cancelled)
        .map_err(FoundationDefaultReadError::Owner)?;
    item_schemas
        .finish(deadline, cancelled)
        .map_err(FoundationDefaultReadError::Owner)?;
    layer_schemas
        .finish(deadline, cancelled)
        .map_err(FoundationDefaultReadError::Owner)?;
    Ok(EvaluatedFoundationDefault {
        rules,
        bibliography,
        reader_cost,
        auxiliary_custody,
    })
}
