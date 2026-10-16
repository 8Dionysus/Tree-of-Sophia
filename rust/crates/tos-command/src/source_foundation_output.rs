//! Bounded final output assembly for the maintained default source-foundation
//! route. This module preserves the evidence that produced the ordered CLI
//! findings; it never turns issue absence into source admission.

use super::foundation_catalog::{
    EvaluatedPersistedCatalog, FoundationCatalogOutcome, GeneratedCatalogObservation, Issue,
};
use super::foundation_payload::PhysicalPayloadCompletion;
use super::foundation_physical::PhysicalSourceCost;
use super::foundation_reader::FoundationRuleReadCost;
use super::foundation_rule_diagnostics::{
    EvaluatedSourceFoundationRules, SourceFoundationRuleDiagnosticsCost,
};
use super::foundation_run::{FinalizedFoundationDefaultInputs, FoundationBiblioEvidence};
use std::mem::size_of;
use tos_compiler::source_witness_catalog::ColdSourceCatalogReceipt;
use tos_validation::executor::schema_diagnostics::PathSegment;
use tos_validation::source_cut::CutSchemaDiagnostic;
use tos_validation::source_foundation_default_rules::SourceFoundationDefaultRulesReport;
use tos_validation::source_foundation_labs::SourceFoundationLab;
use tos_validation::source_foundation_records::SourceFoundationRecordsOwnerIssue;
use tos_validation::source_foundation_schema::{
    SourceFoundationSchemaReport, python_path_suffix, reason_prose,
};

/// The output envelope is additional to the already-admitted final inputs.
/// `max_output_bytes` covers issue location/message UTF-8 bytes; CLI framing
/// remains the writer's separate responsibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceFoundationOutputLimits {
    pub max_issues: usize,
    pub max_output_bytes: usize,
    pub max_state_bytes: usize,
}

/// Owner gaps stay as typed retained evidence. They are not converted to a
/// synthetic CLI finding or to an accepted empty result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceFoundationOutputGap {
    Labs,
    Records,
    Goldsets,
    Discovery,
    Closure,
    RuleDiagnostics,
    CatalogDiagnostic,
    PersistedCatalogDiagnostics,
    BibliographicClaims,
}

pub(crate) enum SourceFoundationOutputOutcome {
    /// Complete mechanics/evidence assembly. A nonempty `issues` list remains
    /// a normal negative CLI result; this variant is not a validity verdict.
    Complete(AssembledSourceFoundationOutput),
    /// At least one source owner gap or incomplete diagnostic remains. The
    /// exact reports and controls are retained intact for the caller.
    Incomplete {
        evidence: FinalizedFoundationDefaultInputs,
        gaps: Vec<SourceFoundationOutputGap>,
    },
    /// Output resource/binding refusal. No partial issue list escapes.
    Refused {
        evidence: FinalizedFoundationDefaultInputs,
        reason: &'static str,
    },
}

pub(crate) struct AssembledSourceFoundationOutput {
    /// Maintained order: Labs → Records → Goldsets → Discovery → Closure →
    /// generated catalog producer → persisted catalog tail.
    pub issues: Vec<Issue>,
    pub rules: SourceFoundationRulesOutputEvidence,
    pub bibliography: FoundationBiblioEvidence,
    pub catalog: SourceFoundationCatalogOutputEvidence,
    pub persisted_catalog: SourceFoundationPersistedCatalogOutputEvidence,
    pub custody: SourceFoundationCustodyOutputEvidence,
    pub cost: SourceFoundationOutputCost,
}

pub(crate) struct SourceFoundationRulesOutputEvidence {
    /// Includes source events, declarations, local controls, source bindings,
    /// typed findings and every original owner gap.
    pub owner_report: SourceFoundationDefaultRulesReport,
    /// Resolved per-lab reports remain separate from document diagnostics and
    /// synthetic control outcomes. Their issue rows move into `issues`.
    pub resolved_labs: Vec<ResolvedSourceFoundationLabOutputEvidence>,
    pub diagnostics: Option<SourceFoundationSchemaReport>,
    pub cost: SourceFoundationRuleDiagnosticsCost,
}

pub(crate) struct ResolvedSourceFoundationLabOutputEvidence {
    pub lab: SourceFoundationLab,
    pub report: serde_json::Value,
}

pub(crate) enum SourceFoundationCatalogOutputEvidence {
    Complete {
        catalog: ColdSourceCatalogReceipt,
        bibliographic_constructed: bool,
        generated_read_bytes: usize,
        generated_inputs: GeneratedCatalogObservation,
        source_recheck_read_bytes: usize,
        schema_execution_cost: tos_validation::source_cut::CutSchemaDiagnosticsCumulativeCost,
    },
    /// A complete, authenticated invalid schema report is retained as a
    /// negative finding. Its mapped prose is already present in `issues`.
    SchemaRejected {
        diagnostic: CutSchemaDiagnostic,
        bibliographic_phase: bool,
        generated_read_bytes: usize,
        generated_inputs: Option<GeneratedCatalogObservation>,
        source_recheck_read_bytes: usize,
        schema_execution_cost: tos_validation::source_cut::CutSchemaDiagnosticsCumulativeCost,
    },
}

pub(crate) struct SourceFoundationPersistedCatalogOutputEvidence {
    pub generated_inputs: GeneratedCatalogObservation,
    pub diagnostics: Option<SourceFoundationSchemaReport>,
    pub input_workspace_bytes: usize,
    pub parsed_input_state_bytes: usize,
}

pub(crate) struct SourceFoundationCustodyOutputEvidence {
    pub reader_cost: FoundationRuleReadCost,
    pub physical_cost: PhysicalSourceCost,
    pub payload_completion: PhysicalPayloadCompletion,
    pub final_authored_member_read_bytes: usize,
    pub route_operations_after_eof: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceFoundationOutputCost {
    pub issue_count: usize,
    pub issue_utf8_bytes: usize,
    /// Conservative additional live-state bound for issue vectors, schema
    /// presentation strings, lab evidence rows and output wrappers.
    pub additional_state_upper_bound_bytes: usize,
}

#[derive(Clone, Copy)]
struct Preflight {
    issue_count: usize,
    direct_issue_bytes: usize,
    schema_issue_count: usize,
    schema_bytes_upper: usize,
    schema_suffix_upper: usize,
    output_bytes_upper: usize,
    state_upper: usize,
}

/// Consume finalized custody evidence only after all owner gaps, schema
/// completeness/bindings, output bytes, issue count and added-state bounds
/// have been checked. Any incomplete/refused path returns the original
/// finalized inputs unchanged.
pub(crate) fn assemble_default(
    evidence: FinalizedFoundationDefaultInputs,
    limits: SourceFoundationOutputLimits,
) -> SourceFoundationOutputOutcome {
    let gaps = collect_gaps(&evidence);
    if !gaps.is_empty() {
        return incomplete(evidence, gaps, limits);
    }

    let Some(preflight) = preflight(&evidence, limits) else {
        return refused(evidence, "foundation output limits or bindings");
    };
    let mut issues = Vec::new();
    if issues.try_reserve_exact(preflight.issue_count).is_err() {
        return refused(evidence, "foundation output issue allocation");
    }
    let (mut mapped_schema, mapped_schema_bytes) =
        match map_catalog_diagnostic(&evidence, preflight) {
            Ok(mapped) => mapped,
            Err(reason) => return refused(evidence, reason),
        };
    let Some(issue_utf8_bytes) = preflight
        .direct_issue_bytes
        .checked_add(mapped_schema_bytes)
    else {
        return refused(evidence, "foundation output issue byte overflow");
    };
    let mut resolved_labs = Vec::new();
    if resolved_labs
        .try_reserve_exact(evidence.rules.resolved_labs.len())
        .is_err()
    {
        return refused(evidence, "foundation output lab evidence allocation");
    }

    let FinalizedFoundationDefaultInputs {
        rules,
        bibliography,
        catalog,
        persisted_catalog,
        reader_cost,
        physical_cost,
        payload_completion,
        final_authored_member_read_bytes,
        route_operations_after_eof,
    } = evidence;
    let EvaluatedSourceFoundationRules {
        owner_report,
        resolved_labs: district_labs,
        records_issues,
        goldset_issues,
        discovery_issues,
        closure_issues,
        diagnostics,
        cost: rules_cost,
    } = rules;

    for mut lab in district_labs {
        issues.append(&mut lab.issues);
        resolved_labs.push(ResolvedSourceFoundationLabOutputEvidence {
            lab: lab.lab,
            report: lab.report,
        });
    }
    issues.extend(records_issues);
    issues.extend(goldset_issues);
    issues.extend(discovery_issues);
    issues.extend(closure_issues);

    let catalog = match catalog {
        FoundationCatalogOutcome::Complete(result) => {
            issues.extend(result.issues);
            SourceFoundationCatalogOutputEvidence::Complete {
                catalog: result.catalog,
                bibliographic_constructed: result.bibliographic_constructed,
                generated_read_bytes: result.generated_read_bytes,
                generated_inputs: result.generated_inputs,
                source_recheck_read_bytes: result.source_recheck_read_bytes,
                schema_execution_cost: result.schema_execution_cost,
            }
        }
        FoundationCatalogOutcome::SchemaRejected {
            observed_plan_work_bytes: _,
            profiles: _,
            diagnostic,
            bibliographic_phase,
            mut catalog_issues,
            generated_read_bytes,
            generated_inputs,
            source_recheck_read_bytes,
            schema_execution_cost,
        } => {
            issues.append(&mut catalog_issues);
            issues.append(&mut mapped_schema);
            SourceFoundationCatalogOutputEvidence::SchemaRejected {
                diagnostic,
                bibliographic_phase,
                generated_read_bytes,
                generated_inputs,
                source_recheck_read_bytes,
                schema_execution_cost,
            }
        }
    };

    let SourceFoundationPersistedCatalogOutputEvidence {
        generated_inputs,
        diagnostics: persisted_diagnostics,
        input_workspace_bytes,
        parsed_input_state_bytes,
    } = {
        issues.extend(persisted_catalog.issues);
        SourceFoundationPersistedCatalogOutputEvidence {
            generated_inputs: persisted_catalog.generated_inputs,
            diagnostics: persisted_catalog.diagnostics,
            input_workspace_bytes: persisted_catalog.input_workspace_bytes,
            parsed_input_state_bytes: persisted_catalog.parsed_input_state_bytes,
        }
    };

    drop(mapped_schema);

    SourceFoundationOutputOutcome::Complete(AssembledSourceFoundationOutput {
        issues,
        bibliography,
        rules: SourceFoundationRulesOutputEvidence {
            owner_report,
            resolved_labs,
            diagnostics,
            cost: rules_cost,
        },
        catalog,
        persisted_catalog: SourceFoundationPersistedCatalogOutputEvidence {
            generated_inputs,
            diagnostics: persisted_diagnostics,
            input_workspace_bytes,
            parsed_input_state_bytes,
        },
        custody: SourceFoundationCustodyOutputEvidence {
            reader_cost,
            physical_cost,
            payload_completion,
            final_authored_member_read_bytes,
            route_operations_after_eof,
        },
        cost: SourceFoundationOutputCost {
            issue_count: preflight.issue_count,
            issue_utf8_bytes,
            additional_state_upper_bound_bytes: preflight.state_upper,
        },
    })
}

fn collect_gaps(evidence: &FinalizedFoundationDefaultInputs) -> Vec<SourceFoundationOutputGap> {
    let owner = &evidence.rules.owner_report;
    let mut gaps = Vec::new();
    // Exact Claim findings remain retained. Until their maintained CLI mapping
    // is connected, a negative/truncated report cannot become empty success.
    if match &evidence.bibliography {
        FoundationBiblioEvidence::NotSelected => false,
        FoundationBiblioEvidence::RecordsIncomplete => true,
        FoundationBiblioEvidence::Complete { report, .. } => {
            report.shadow.issue_sink_truncated || !report.shadow.issues.is_empty()
        }
    } {
        gaps.push(SourceFoundationOutputGap::BibliographicClaims);
    }
    if !owner.labs.unimplemented.is_empty()
        || owner
            .labs
            .results
            .iter()
            .any(|lab| !lab.unimplemented.is_empty())
    {
        gaps.push(SourceFoundationOutputGap::Labs);
    }
    if !owner.records.unimplemented.is_empty() {
        gaps.push(SourceFoundationOutputGap::Records);
    }
    if !owner.goldsets.coverage_gaps.is_empty() {
        gaps.push(SourceFoundationOutputGap::Goldsets);
    }
    if !owner.discovery.unsupported.is_empty() {
        gaps.push(SourceFoundationOutputGap::Discovery);
    }
    if !owner.closure.unsupported.is_empty() {
        gaps.push(SourceFoundationOutputGap::Closure);
    }
    if !owner_rule_bindings(&evidence.rules)
        || !rules_diagnostics_complete(&evidence.rules)
        || !resolved_lab_binding(&evidence.rules)
        || !rules_issue_cost_binding(&evidence.rules)
    {
        gaps.push(SourceFoundationOutputGap::RuleDiagnostics);
    }
    if matches!(
        &evidence.catalog,
        FoundationCatalogOutcome::SchemaRejected { diagnostic, .. }
            if !diagnostic.is_invalid() || diagnostic.report().issues.is_empty()
    ) {
        gaps.push(SourceFoundationOutputGap::CatalogDiagnostic);
    }
    if !persisted_diagnostics_complete(&evidence.persisted_catalog) {
        gaps.push(SourceFoundationOutputGap::PersistedCatalogDiagnostics);
    }
    gaps
}

fn incomplete(
    evidence: FinalizedFoundationDefaultInputs,
    gaps: Vec<SourceFoundationOutputGap>,
    limits: SourceFoundationOutputLimits,
) -> SourceFoundationOutputOutcome {
    let gap_state = gaps
        .capacity()
        .checked_mul(size_of::<SourceFoundationOutputGap>());
    let state =
        gap_state.and_then(|bytes| size_of::<SourceFoundationOutputOutcome>().checked_add(bytes));
    if state.is_none_or(|bytes| bytes > limits.max_state_bytes) {
        return refused(evidence, "foundation incomplete evidence state limit");
    }
    SourceFoundationOutputOutcome::Incomplete { evidence, gaps }
}

fn rules_diagnostics_complete(rules: &EvaluatedSourceFoundationRules) -> bool {
    let queued = rules.owner_report.cost.queued_schema_document_count;
    if rules.cost.schema_check_count != queued {
        return false;
    }
    match (&rules.diagnostics, queued) {
        (None, 0) => true,
        (Some(report), count) if count > 0 => {
            report.is_complete() && report.expected_check_count == count
        }
        _ => false,
    }
}

fn owner_rule_bindings(rules: &EvaluatedSourceFoundationRules) -> bool {
    let owner = &rules.owner_report;
    let direct_issue_count = owner
        .labs
        .ordered_issues
        .len()
        .checked_add(owner.records.ordered_issues.len())
        .and_then(|n| n.checked_add(owner.goldsets.ordered_issues.len()))
        .and_then(|n| n.checked_add(owner.discovery.issues.len()))
        .and_then(|n| n.checked_add(owner.closure.issues.len()));
    if direct_issue_count != Some(owner.cost.direct_owner_issue_count) {
        return false;
    }
    let Some(mut checks) = owner.labs.results.iter().try_fold(0usize, |count, lab| {
        count.checked_add(lab.schema_checks.len())
    }) else {
        return false;
    };
    if checks != owner.labs.schema_checks.len() {
        return false;
    }
    for check in &owner.records.schema_checks {
        match (
            check.decoded_instance.is_some(),
            check.legacy_raw_instance.is_some(),
            check.owner_issue,
        ) {
            (true, false, None) | (false, true, None) => {
                let Some(next) = checks.checked_add(1) else {
                    return false;
                };
                checks = next;
            }
            (
                true,
                false,
                Some(
                    SourceFoundationRecordsOwnerIssue::JsonRootMustBeObject
                    | SourceFoundationRecordsOwnerIssue::JsonlRecordMustBeObject,
                ),
            ) if check
                .decoded_instance
                .as_ref()
                .is_some_and(|value| !value.is_object()) =>
            {
                let Some(next) = checks.checked_add(1) else {
                    return false;
                };
                checks = next;
            }
            (
                false,
                true,
                Some(
                    SourceFoundationRecordsOwnerIssue::JsonRootMustBeObject
                    | SourceFoundationRecordsOwnerIssue::JsonlRecordMustBeObject,
                ),
            ) => {
                let Some(next) = checks.checked_add(1) else {
                    return false;
                };
                checks = next;
            }
            (false, false, Some(_)) => {}
            _ => return false,
        }
    }
    let Some(checks) = checks
        .checked_add(owner.goldsets.schema_requests.len())
        .and_then(|n| n.checked_add(owner.discovery.schema_requests.len()))
        .and_then(|n| n.checked_add(owner.closure.schema_requests.len()))
    else {
        return false;
    };
    checks == owner.cost.queued_schema_document_count
}

fn resolved_lab_binding(rules: &EvaluatedSourceFoundationRules) -> bool {
    rules.owner_report.labs.results.len() == rules.resolved_labs.len()
        && rules
            .owner_report
            .labs
            .results
            .iter()
            .zip(&rules.resolved_labs)
            .all(|(owner, resolved)| owner.lab == resolved.lab)
}

fn rules_issue_cost_binding(rules: &EvaluatedSourceFoundationRules) -> bool {
    let mut direct_count = 0usize;
    let mut direct_bytes = 0usize;
    for lab in &rules.resolved_labs {
        if !add_issue_stats(&mut direct_count, &mut direct_bytes, &lab.issues) {
            return false;
        }
    }
    for selected in [
        &rules.records_issues,
        &rules.goldset_issues,
        &rules.discovery_issues,
        &rules.closure_issues,
    ] {
        if !add_issue_stats(&mut direct_count, &mut direct_bytes, selected) {
            return false;
        }
    }
    direct_count == rules.cost.issue_count && direct_bytes == rules.cost.issue_utf8_bytes
}

fn persisted_diagnostics_complete(catalog: &EvaluatedPersistedCatalog) -> bool {
    match &catalog.diagnostics {
        None => catalog.input_workspace_bytes == 0,
        Some(report) => {
            catalog.input_workspace_bytes > 0
                && report.expected_check_count > 0
                && report.is_complete()
        }
    }
}

fn preflight(
    evidence: &FinalizedFoundationDefaultInputs,
    limits: SourceFoundationOutputLimits,
) -> Option<Preflight> {
    let mut issue_count = 0usize;
    let mut direct_issue_bytes = 0usize;
    for lab in &evidence.rules.resolved_labs {
        if !add_issue_stats(&mut issue_count, &mut direct_issue_bytes, &lab.issues) {
            return None;
        }
    }
    for selected in [
        &evidence.rules.records_issues,
        &evidence.rules.goldset_issues,
        &evidence.rules.discovery_issues,
        &evidence.rules.closure_issues,
    ] {
        if !add_issue_stats(&mut issue_count, &mut direct_issue_bytes, selected) {
            return None;
        }
    }
    let catalog_direct = match &evidence.catalog {
        FoundationCatalogOutcome::Complete(result) => &result.issues,
        FoundationCatalogOutcome::SchemaRejected { catalog_issues, .. } => catalog_issues,
    };
    if !add_issue_stats(&mut issue_count, &mut direct_issue_bytes, catalog_direct)
        || !add_issue_stats(
            &mut issue_count,
            &mut direct_issue_bytes,
            &evidence.persisted_catalog.issues,
        )
    {
        return None;
    }

    let (schema_issue_count, schema_bytes_upper, schema_suffix_upper) = match &evidence.catalog {
        FoundationCatalogOutcome::SchemaRejected { diagnostic, .. } => {
            cut_schema_output_bound(diagnostic)?
        }
        FoundationCatalogOutcome::Complete(_) => (0, 0, 0),
    };
    issue_count = issue_count.checked_add(schema_issue_count)?;
    let output_bytes_upper = direct_issue_bytes.checked_add(schema_bytes_upper)?;
    let output_slots = issue_count.checked_mul(size_of::<Issue>())?;
    let staged_schema_slots = schema_issue_count.checked_mul(size_of::<Issue>())?;
    let mapped_temporary = schema_suffix_upper.checked_mul(2)?.checked_add(16)?;
    let lab_slots = evidence
        .rules
        .resolved_labs
        .len()
        .checked_mul(size_of::<ResolvedSourceFoundationLabOutputEvidence>())?;
    let state_upper = output_slots
        .checked_add(staged_schema_slots)?
        .checked_add(schema_bytes_upper)?
        .checked_add(mapped_temporary)?
        .checked_add(lab_slots)?
        .checked_add(size_of::<AssembledSourceFoundationOutput>())?
        .checked_add(size_of::<SourceFoundationOutputOutcome>())?;

    if issue_count > limits.max_issues
        || output_bytes_upper > limits.max_output_bytes
        || state_upper > limits.max_state_bytes
    {
        return None;
    }
    Some(Preflight {
        issue_count,
        direct_issue_bytes,
        schema_issue_count,
        schema_bytes_upper,
        schema_suffix_upper,
        output_bytes_upper,
        state_upper,
    })
}

fn add_issue_stats(count: &mut usize, bytes: &mut usize, issues: &[Issue]) -> bool {
    let Some(next_count) = count.checked_add(issues.len()) else {
        return false;
    };
    let Some(next_bytes) = issues
        .iter()
        .try_fold(*bytes, |total, (location, message)| {
            total
                .checked_add(location.len())?
                .checked_add(message.len())
        })
    else {
        return false;
    };
    *count = next_count;
    *bytes = next_bytes;
    true
}

fn cut_schema_output_bound(diagnostic: &CutSchemaDiagnostic) -> Option<(usize, usize, usize)> {
    if !diagnostic.is_invalid() || diagnostic.report().issues.is_empty() {
        return None;
    }
    let mut bytes = 0usize;
    let mut suffix_max = 0usize;
    for issue in &diagnostic.report().issues {
        let suffix = path_suffix_upper_bound(&issue.instance_path)?;
        suffix_max = suffix_max.max(suffix);
        let message = issue
            .compatibility_text()
            .unwrap_or_else(|| reason_prose(issue.reason));
        bytes = bytes
            .checked_add(diagnostic.path().len())?
            .checked_add(suffix)?
            .checked_add(message.len())?;
    }
    Some((diagnostic.report().issues.len(), bytes, suffix_max))
}

fn path_suffix_upper_bound(path: &[PathSegment]) -> Option<usize> {
    path.iter().try_fold(0usize, |total, segment| {
        let segment_bytes = match segment {
            PathSegment::Property(value) => value.len().checked_mul(10)?.checked_add(4)?,
            PathSegment::Index(value) => decimal_digits(*value).checked_add(2)?,
        };
        total.checked_add(segment_bytes)
    })
}

fn decimal_digits(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn map_catalog_diagnostic(
    evidence: &FinalizedFoundationDefaultInputs,
    preflight: Preflight,
) -> Result<(Vec<Issue>, usize), &'static str> {
    let diagnostic = match &evidence.catalog {
        FoundationCatalogOutcome::SchemaRejected { diagnostic, .. } => diagnostic,
        FoundationCatalogOutcome::Complete(_) => return Ok((Vec::new(), 0)),
    };
    let mut mapped = Vec::new();
    mapped
        .try_reserve_exact(preflight.schema_issue_count)
        .map_err(|_| "foundation catalog diagnostic allocation")?;
    let mut bytes = 0usize;
    for issue in &diagnostic.report().issues {
        let suffix = python_path_suffix(&issue.instance_path);
        if suffix.len() > preflight.schema_suffix_upper {
            return Err("foundation catalog diagnostic path bound");
        }
        let location_bytes = diagnostic
            .path()
            .len()
            .checked_add(suffix.len())
            .ok_or("foundation catalog diagnostic byte overflow")?;
        let mut location = String::new();
        location
            .try_reserve_exact(location_bytes)
            .map_err(|_| "foundation catalog diagnostic location allocation")?;
        location.push_str(diagnostic.path());
        location.push_str(&suffix);
        let prose = issue
            .compatibility_text()
            .unwrap_or_else(|| reason_prose(issue.reason));
        let mut message = String::new();
        message
            .try_reserve_exact(prose.len())
            .map_err(|_| "foundation catalog diagnostic message allocation")?;
        message.push_str(prose);
        bytes = bytes
            .checked_add(location.len())
            .and_then(|n| n.checked_add(message.len()))
            .ok_or("foundation catalog diagnostic byte overflow")?;
        mapped.push((location, message));
    }
    if mapped.len() != preflight.schema_issue_count
        || bytes > preflight.schema_bytes_upper
        || preflight
            .direct_issue_bytes
            .checked_add(bytes)
            .is_none_or(|total| total > preflight.output_bytes_upper)
    {
        return Err("foundation catalog diagnostic output binding");
    }
    Ok((mapped, bytes))
}

fn refused(
    evidence: FinalizedFoundationDefaultInputs,
    reason: &'static str,
) -> SourceFoundationOutputOutcome {
    SourceFoundationOutputOutcome::Refused { evidence, reason }
}
