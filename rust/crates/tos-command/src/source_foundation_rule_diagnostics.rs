//! Diagnostic-v2 evaluation and ordered CLI issue assembly for the default
//! source-foundation districts. It preserves controls and owner gaps and does
//! not produce a whole-foundation validity verdict.

use super::foundation_cli::{SchemaPlacement, interleave_schema_findings};
use super::foundation_lab_resolution::{self, ResolvedFoundationLab};
use serde_json::Value;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_validation::executor::{
    ExactWorkerIdentity, SharedSchemaWorkerQuota, VerifiedWorkerImageHandle,
};
use tos_validation::source_foundation_closure::SourceFoundationClosureSchemaRequest;
use tos_validation::source_foundation_default_rules::SourceFoundationDefaultRulesReport;
use tos_validation::source_foundation_discovery::{
    Issue as DiscoveryIssue, SchemaRequest as DiscoverySchemaRequest,
};
use tos_validation::source_foundation_goldsets::SourceFoundationSchemaRequest as GoldsetSchemaRequest;
use tos_validation::source_foundation_labs::{SourceFoundationLab, SourceFoundationLabResult};
use tos_validation::source_foundation_records::{
    SourceFoundationRecordsOwnerIssue, SourceFoundationRecordsSchemaCheck,
};
use tos_validation::source_foundation_schema::{
    SourceFoundationLegacySchemaInput, SourceFoundationMixedSchemaInput,
    SourceFoundationSchemaCheckReport, SourceFoundationSchemaFailure, SourceFoundationSchemaInput,
    SourceFoundationSchemaIssue, SourceFoundationSchemaLimits, SourceFoundationSchemaOutcome,
    SourceFoundationSchemaReport, SourceFoundationSchemaSet,
    evaluate_source_foundation_mixed_schema_checks,
    evaluate_source_foundation_mixed_schema_checks_with_shared_quota,
    evaluate_source_foundation_mixed_schema_checks_with_shared_quota_and_image,
};

/// Additional envelope for diagnostic assembly. `max_output_bytes` bounds
/// retained issue UTF-8 bytes; the CLI writer separately charges framing.
/// `max_state_bytes` is additional to the already-admitted owner report and
/// includes the returned wrapper's fixed headers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceFoundationRuleDiagnosticsLimits {
    pub max_issues: usize,
    pub max_output_bytes: usize,
    pub max_state_bytes: usize,
}

/// A resolved per-lab report keeps synthetic controls distinct from document
/// schema findings in the other districts.
pub(crate) struct ResolvedSourceFoundationLab {
    pub lab: SourceFoundationLab,
    pub report: Value,
    pub issues: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFoundationRuleDiagnosticsCost {
    pub schema_check_count: usize,
    pub schema_input_reference_bytes: usize,
    /// Peak bound for encoded instances, worker response, and evaluator-owned
    /// report structures while the same diagnostic-v2 worker is running.
    pub schema_evaluator_peak_state_upper_bound_bytes: usize,
    /// Retained schema report and structure upper bound.
    pub schema_report_state_upper_bound_bytes: usize,
    /// Retained resolved lab reports and interleaved issue streams.
    pub resolved_output_state_upper_bound_bytes: usize,
    pub issue_count: usize,
    pub issue_utf8_bytes: usize,
    pub peak_additional_state_upper_bound_bytes: usize,
}

/// Actual owner report remains available for following catalog and
/// bibliographic conditions. It retains source events, local controls, cost,
/// and exact coverage gaps; no completion field is synthesized here.
pub(crate) struct EvaluatedSourceFoundationRules {
    pub owner_report: SourceFoundationDefaultRulesReport,
    pub resolved_labs: Vec<ResolvedSourceFoundationLab>,
    pub records_issues: Vec<(String, String)>,
    pub goldset_issues: Vec<(String, String)>,
    pub discovery_issues: Vec<(String, String)>,
    pub closure_issues: Vec<(String, String)>,
    /// `None` is emitted only for a true zero-request run; it is not a fake
    /// empty successful diagnostic report.
    pub diagnostics: Option<SourceFoundationSchemaReport>,
    pub cost: SourceFoundationRuleDiagnosticsCost,
}

pub(crate) enum SourceFoundationRuleDiagnosticsError {
    Refused {
        owner_report: SourceFoundationDefaultRulesReport,
        diagnostics: Option<SourceFoundationSchemaReport>,
        reason: &'static str,
    },
    IncompleteSchema {
        owner_report: SourceFoundationDefaultRulesReport,
        report: SourceFoundationSchemaReport,
        reason: SourceFoundationSchemaFailure,
    },
}

impl SourceFoundationRuleDiagnosticsError {
    pub(crate) fn owner_report(&self) -> &SourceFoundationDefaultRulesReport {
        match self {
            Self::Refused { owner_report, .. } | Self::IncompleteSchema { owner_report, .. } => {
                owner_report
            }
        }
    }

    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::Refused { reason, .. } => reason,
            Self::IncompleteSchema { reason, .. } => match reason {
                SourceFoundationSchemaFailure::InvalidLimits => "foundation schema limits invalid",
                SourceFoundationSchemaFailure::Deadline => "foundation schema deadline",
                SourceFoundationSchemaFailure::Cancelled => "foundation schema cancelled",
                SourceFoundationSchemaFailure::InvalidLocation => {
                    "foundation schema location invalid"
                }
                SourceFoundationSchemaFailure::ContractNotSelected => {
                    "foundation schema contract not selected"
                }
                SourceFoundationSchemaFailure::InputProfileMismatch => {
                    "foundation schema input profile mismatch"
                }
                SourceFoundationSchemaFailure::CatalogSchemaSelection => {
                    "foundation schema selection invalid"
                }
                SourceFoundationSchemaFailure::InputBudget => "foundation schema input budget",
                SourceFoundationSchemaFailure::Worker(_) => "foundation schema worker failed",
                SourceFoundationSchemaFailure::IncompleteDiagnostic => {
                    "foundation diagnostic incomplete"
                }
                SourceFoundationSchemaFailure::TruncatedDiagnostics => {
                    "foundation diagnostic truncated"
                }
                SourceFoundationSchemaFailure::DiagnosticReportBudget => {
                    "foundation diagnostic report budget"
                }
                SourceFoundationSchemaFailure::DiagnosticBinding => {
                    "foundation diagnostic binding invalid"
                }
                SourceFoundationSchemaFailure::UnsupportedInputSemantics => {
                    "foundation schema input semantics unsupported"
                }
            },
        }
    }
}

struct Processed {
    resolved_labs: Vec<ResolvedSourceFoundationLab>,
    records_issues: Vec<(String, String)>,
    goldset_issues: Vec<(String, String)>,
    discovery_issues: Vec<(String, String)>,
    closure_issues: Vec<(String, String)>,
    cost: SourceFoundationRuleDiagnosticsCost,
}

enum ProcessError {
    Refused(&'static str),
}

/// Schedule every retained district request in maintained order and run the
/// same diagnostic-v2 evaluator used by the other foundation routes. Local
/// insertion ordinals remain district-local and the generic CLI interleaver
/// inserts full schema prose without boolean surrogates.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate(
    owner_report: SourceFoundationDefaultRulesReport,
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    schema_limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    limits: SourceFoundationRuleDiagnosticsLimits,
) -> Result<EvaluatedSourceFoundationRules, SourceFoundationRuleDiagnosticsError> {
    evaluate_inner(
        owner_report,
        schema_set,
        worker,
        schema_limits,
        deadline,
        cancelled,
        limits,
        None,
        None,
    )
}

/// Evaluate the same ordered diagnostic kernel while forwarding the caller's
/// invocation-wide quota to the diagnostics-v2 worker. The handle is cloned
/// once; no quota is constructed or reset in this district.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_with_shared_quota(
    owner_report: SourceFoundationDefaultRulesReport,
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    schema_limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    limits: SourceFoundationRuleDiagnosticsLimits,
    quota: &SharedSchemaWorkerQuota,
) -> Result<EvaluatedSourceFoundationRules, SourceFoundationRuleDiagnosticsError> {
    evaluate_inner(
        owner_report,
        schema_set,
        worker,
        schema_limits,
        deadline,
        cancelled,
        limits,
        Some(quota.clone()),
        None,
    )
}

/// Evaluate the same ordered diagnostics kernel with the caller's already
/// verified worker image. The handle remains borrowed across the exchange so
/// all foundation districts can share one sealed executable.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_with_shared_quota_and_image(
    owner_report: SourceFoundationDefaultRulesReport,
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    image: &VerifiedWorkerImageHandle,
    schema_limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    limits: SourceFoundationRuleDiagnosticsLimits,
    quota: &SharedSchemaWorkerQuota,
) -> Result<EvaluatedSourceFoundationRules, SourceFoundationRuleDiagnosticsError> {
    evaluate_inner(
        owner_report,
        schema_set,
        worker,
        schema_limits,
        deadline,
        cancelled,
        limits,
        Some(quota.clone()),
        Some(image),
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_inner(
    owner_report: SourceFoundationDefaultRulesReport,
    schema_set: &SourceFoundationSchemaSet,
    worker: &ExactWorkerIdentity,
    schema_limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    limits: SourceFoundationRuleDiagnosticsLimits,
    shared_schema_worker_quota: Option<SharedSchemaWorkerQuota>,
    worker_image: Option<&VerifiedWorkerImageHandle>,
) -> Result<EvaluatedSourceFoundationRules, SourceFoundationRuleDiagnosticsError> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        return Err(refused(
            owner_report,
            None,
            "foundation rule diagnostics deadline or cancellation",
        ));
    }
    if !schema_limits.validate() {
        return Err(refused(
            owner_report,
            None,
            "foundation rule diagnostics schema limits",
        ));
    }
    if owner_report.records.source_revision != schema_set.source_revision() {
        return Err(refused(
            owner_report,
            None,
            "foundation rule diagnostics source revision binding",
        ));
    }
    let count = match schema_count(&owner_report) {
        Ok(count) => count,
        Err(reason) => return Err(refused(owner_report, None, reason)),
    };
    if count != owner_report.cost.queued_schema_document_count {
        return Err(refused(
            owner_report,
            None,
            "foundation rule diagnostics queued schema count binding",
        ));
    }
    if count > schema_limits.max_checks {
        return Err(refused(
            owner_report,
            None,
            "foundation rule diagnostics schema count",
        ));
    }
    let input_reference_bytes =
        match count.checked_mul(size_of::<SourceFoundationMixedSchemaInput<'_>>()) {
            Some(bytes) => bytes,
            None => {
                return Err(refused(
                    owner_report,
                    None,
                    "foundation rule diagnostics input overflow",
                ));
            }
        };
    let evaluator_peak = match evaluator_peak_bound(count, schema_limits, input_reference_bytes) {
        Some(bytes) => bytes,
        None => {
            return Err(refused(
                owner_report,
                None,
                "foundation rule diagnostics state overflow",
            ));
        }
    };
    if evaluator_peak > limits.max_state_bytes {
        return Err(refused(
            owner_report,
            None,
            "foundation rule diagnostics evaluator state limit",
        ));
    }

    let diagnostics = if count == 0 {
        None
    } else {
        let mut checks = Vec::new();
        if checks.try_reserve_exact(count).is_err() {
            return Err(refused(
                owner_report,
                None,
                "foundation rule diagnostics input allocation",
            ));
        }
        append_lab_inputs(&owner_report, &mut checks);
        append_record_inputs(&owner_report, &mut checks);
        append_goldset_inputs(&owner_report, &mut checks);
        append_discovery_inputs(&owner_report, &mut checks);
        append_closure_inputs(&owner_report, &mut checks);
        if checks.len() != count {
            return Err(refused(
                owner_report,
                None,
                "foundation rule diagnostics input binding",
            ));
        }
        let outcome = match (shared_schema_worker_quota, worker_image) {
            (Some(quota), Some(image)) => {
                evaluate_source_foundation_mixed_schema_checks_with_shared_quota_and_image(
                    schema_set,
                    worker,
                    &checks,
                    schema_limits,
                    deadline,
                    cancelled,
                    &quota,
                    image,
                )
            }
            (Some(quota), None) => {
                evaluate_source_foundation_mixed_schema_checks_with_shared_quota(
                    schema_set,
                    worker,
                    &checks,
                    schema_limits,
                    deadline,
                    cancelled,
                    &quota,
                )
            }
            (None, _) => evaluate_source_foundation_mixed_schema_checks(
                schema_set,
                worker,
                &checks,
                schema_limits,
                deadline,
                cancelled,
            ),
        };
        drop(checks);
        match outcome {
            SourceFoundationSchemaOutcome::Complete(report) => Some(report),
            SourceFoundationSchemaOutcome::Incomplete { report, reason } => {
                return Err(SourceFoundationRuleDiagnosticsError::IncompleteSchema {
                    owner_report,
                    report,
                    reason,
                });
            }
        }
    };

    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        return Err(refused(
            owner_report,
            diagnostics,
            "foundation rule diagnostics deadline or cancellation",
        ));
    }

    let processed = process(
        &owner_report,
        diagnostics.as_ref(),
        count,
        input_reference_bytes,
        evaluator_peak,
        limits,
    );
    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        return Err(refused(
            owner_report,
            diagnostics,
            "foundation rule diagnostics deadline or cancellation",
        ));
    }
    let processed = match processed {
        Ok(processed) => processed,
        Err(ProcessError::Refused(reason)) => {
            return Err(refused(owner_report, diagnostics, reason));
        }
    };
    Ok(EvaluatedSourceFoundationRules {
        owner_report,
        resolved_labs: processed.resolved_labs,
        records_issues: processed.records_issues,
        goldset_issues: processed.goldset_issues,
        discovery_issues: processed.discovery_issues,
        closure_issues: processed.closure_issues,
        diagnostics,
        cost: processed.cost,
    })
}

fn process(
    owner: &SourceFoundationDefaultRulesReport,
    diagnostics: Option<&SourceFoundationSchemaReport>,
    check_count: usize,
    input_reference_bytes: usize,
    evaluator_peak: usize,
    limits: SourceFoundationRuleDiagnosticsLimits,
) -> Result<Processed, ProcessError> {
    let mut schema_state = 0usize;
    if let Some(report) = diagnostics {
        if !report.is_complete() || report.checks.len() != check_count {
            return Err(ProcessError::Refused(
                "foundation diagnostic binding invalid",
            ));
        }
        schema_state = schema_report_state(report).ok_or(ProcessError::Refused(
            "foundation schema report state overflow",
        ))?;
    }
    if schema_state > limits.max_state_bytes {
        return Err(ProcessError::Refused(
            "foundation schema report state limit",
        ));
    }

    let lab_vector_state = owner
        .labs
        .results
        .len()
        .checked_mul(size_of::<ResolvedSourceFoundationLab>())
        .ok_or(ProcessError::Refused(
            "foundation resolved-lab vector state overflow",
        ))?;
    let output_header_state = size_of::<EvaluatedSourceFoundationRules>()
        .checked_sub(size_of::<SourceFoundationDefaultRulesReport>())
        .ok_or(ProcessError::Refused(
            "foundation resolved-output header state overflow",
        ))?;
    let mut retained_state = schema_state
        .checked_add(lab_vector_state)
        .and_then(|used| used.checked_add(output_header_state))
        .filter(|used| *used <= limits.max_state_bytes)
        .ok_or(ProcessError::Refused(
            "foundation resolved-lab vector state limit",
        ))?;
    let mut peak_state = evaluator_peak.max(retained_state);
    let mut issue_count = 0usize;
    let mut issue_bytes = 0usize;
    let mut next_check = 0usize;
    let mut resolved_labs = Vec::new();
    resolved_labs
        .try_reserve_exact(owner.labs.results.len())
        .map_err(|_| ProcessError::Refused("foundation resolved-lab allocation"))?;

    for lab in &owner.labs.results {
        let next = next_check
            .checked_add(lab.schema_checks.len())
            .ok_or(ProcessError::Refused(
                "foundation lab schema count overflow",
            ))?;
        let stage = resolve_lab(
            lab,
            diagnostics,
            next_check,
            limits,
            retained_state,
            issue_count,
            issue_bytes,
        )?;
        issue_count = issue_count
            .checked_add(stage.value.issues.len())
            .ok_or(ProcessError::Refused("foundation issue count overflow"))?;
        issue_bytes = issue_bytes
            .checked_add(
                issue_utf8_bytes(&stage.value.issues)
                    .ok_or(ProcessError::Refused("foundation issue byte overflow"))?,
            )
            .ok_or(ProcessError::Refused("foundation issue byte overflow"))?;
        if issue_count > limits.max_issues || issue_bytes > limits.max_output_bytes {
            return Err(ProcessError::Refused("foundation lab issue limits"));
        }
        retained_state =
            retained_state
                .checked_add(stage.retained_state)
                .ok_or(ProcessError::Refused(
                    "foundation lab retained state overflow",
                ))?;
        if retained_state > limits.max_state_bytes {
            return Err(ProcessError::Refused("foundation lab retained state limit"));
        }
        peak_state = peak_state.max(retained_state).max(stage.peak_state);
        resolved_labs.push(ResolvedSourceFoundationLab {
            lab: lab.lab,
            report: stage.value.report,
            issues: stage.value.issues,
        });
        next_check = next;
    }

    let records_start = next_check;
    let records_count = owner
        .records
        .schema_checks
        .iter()
        .filter(|check| record_has_schema_input(check))
        .count();
    next_check = next_check
        .checked_add(records_count)
        .ok_or(ProcessError::Refused(
            "foundation records schema count overflow",
        ))?;
    let records_issues =
        if let Some(report) = diagnostics {
            let placements =
                record_placements(
                    &owner.records.schema_checks,
                    records_start,
                    limits.max_state_bytes.checked_sub(retained_state).ok_or(
                        ProcessError::Refused("foundation records placement state limit"),
                    )?,
                )
                .map_err(ProcessError::Refused)?;
            let resolved = interleave(
                owner.records.ordered_issues.as_slice(),
                placements,
                report,
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation records state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(resolved.peak_state);
            resolved.issues
        } else {
            let resolved = direct_pairs(
                owner.records.ordered_issues.as_slice(),
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation records state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(retained_state);
            resolved.issues
        };

    let goldset_start = next_check;
    next_check = next_check
        .checked_add(owner.goldsets.schema_requests.len())
        .ok_or(ProcessError::Refused(
            "foundation gold-set schema count overflow",
        ))?;
    let goldset_issues =
        if let Some(report) = diagnostics {
            let placements =
                request_placements(
                    &owner.goldsets.schema_requests,
                    goldset_start,
                    "",
                    limits.max_state_bytes.checked_sub(retained_state).ok_or(
                        ProcessError::Refused("foundation gold-set placement state limit"),
                    )?,
                )
                .map_err(ProcessError::Refused)?;
            let resolved = interleave(
                owner.goldsets.ordered_issues.as_slice(),
                placements,
                report,
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation gold-set state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(resolved.peak_state);
            resolved.issues
        } else {
            let resolved = direct_pairs(
                owner.goldsets.ordered_issues.as_slice(),
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation gold-set state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(retained_state);
            resolved.issues
        };

    let discovery_start = next_check;
    next_check = next_check
        .checked_add(owner.discovery.schema_requests.len())
        .ok_or(ProcessError::Refused(
            "foundation discovery schema count overflow",
        ))?;
    let discovery_issues =
        if let Some(report) = diagnostics {
            let placements =
                request_placements(
                    &owner.discovery.schema_requests,
                    discovery_start,
                    "",
                    limits.max_state_bytes.checked_sub(retained_state).ok_or(
                        ProcessError::Refused("foundation discovery placement state limit"),
                    )?,
                )
                .map_err(ProcessError::Refused)?;
            let resolved = interleave(
                owner.discovery.issues.as_slice(),
                placements,
                report,
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation discovery state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(resolved.peak_state);
            resolved.issues
        } else {
            let resolved = direct_pairs(
                owner.discovery.issues.as_slice(),
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation discovery state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(retained_state);
            resolved.issues
        };

    let closure_start = next_check;
    next_check = next_check
        .checked_add(owner.closure.schema_requests.len())
        .ok_or(ProcessError::Refused(
            "foundation closure schema count overflow",
        ))?;
    if next_check != check_count {
        return Err(ProcessError::Refused("foundation schema district ordering"));
    }
    let closure_issues =
        if let Some(report) = diagnostics {
            let placements =
                request_placements(
                    &owner.closure.schema_requests,
                    closure_start,
                    "",
                    limits.max_state_bytes.checked_sub(retained_state).ok_or(
                        ProcessError::Refused("foundation closure placement state limit"),
                    )?,
                )
                .map_err(ProcessError::Refused)?;
            let resolved = interleave(
                owner.closure.issues.as_slice(),
                placements,
                report,
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation closure state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(resolved.peak_state);
            resolved.issues
        } else {
            let resolved = direct_pairs(
                owner.closure.issues.as_slice(),
                limits,
                retained_state,
                issue_count,
                issue_bytes,
            )?;
            retained_state = retained_state
                .checked_add(resolved.retained_state)
                .ok_or(ProcessError::Refused("foundation closure state overflow"))?;
            issue_count = resolved.issue_count;
            issue_bytes = resolved.issue_bytes;
            peak_state = peak_state.max(retained_state);
            resolved.issues
        };

    Ok(Processed {
        resolved_labs,
        records_issues,
        goldset_issues,
        discovery_issues,
        closure_issues,
        cost: SourceFoundationRuleDiagnosticsCost {
            schema_check_count: check_count,
            schema_input_reference_bytes: input_reference_bytes,
            schema_evaluator_peak_state_upper_bound_bytes: evaluator_peak,
            schema_report_state_upper_bound_bytes: schema_state,
            resolved_output_state_upper_bound_bytes: retained_state
                .checked_sub(schema_state)
                .ok_or(ProcessError::Refused(
                    "foundation resolved-output state underflow",
                ))?,
            issue_count,
            issue_utf8_bytes: issue_bytes,
            peak_additional_state_upper_bound_bytes: peak_state,
        },
    })
}

struct Interleaved {
    issues: Vec<(String, String)>,
    retained_state: usize,
    issue_count: usize,
    issue_bytes: usize,
    peak_state: usize,
}

trait DirectIssueSource {
    fn len(&self) -> usize;
    fn issue(&self, index: usize) -> Option<(&str, &str)>;
}

impl DirectIssueSource for [(String, String)] {
    fn len(&self) -> usize {
        <[(String, String)]>::len(self)
    }

    fn issue(&self, index: usize) -> Option<(&str, &str)> {
        self.get(index)
            .map(|(location, message)| (location.as_str(), message.as_str()))
    }
}

impl DirectIssueSource
    for [tos_validation::source_foundation_records::SourceFoundationRecordsIssue]
{
    fn len(&self) -> usize {
        <[tos_validation::source_foundation_records::SourceFoundationRecordsIssue]>::len(self)
    }

    fn issue(&self, index: usize) -> Option<(&str, &str)> {
        self.get(index)
            .map(|issue| (issue.location.as_str(), issue.message.as_str()))
    }
}

impl DirectIssueSource for [DiscoveryIssue] {
    fn len(&self) -> usize {
        <[DiscoveryIssue]>::len(self)
    }

    fn issue(&self, index: usize) -> Option<(&str, &str)> {
        self.get(index)
            .map(|issue| (issue.location.as_str(), issue.detail.as_str()))
    }
}

fn direct_issue_metrics<S: DirectIssueSource + ?Sized>(
    direct: &S,
) -> Result<(usize, usize, usize, usize), ProcessError> {
    let mut text_state = 0usize;
    let mut utf8_bytes = 0usize;
    for index in 0..direct.len() {
        let (location, message) = direct
            .issue(index)
            .ok_or(ProcessError::Refused("foundation direct issue binding"))?;
        text_state = text_state
            .checked_add(text_storage(location).ok_or(ProcessError::Refused(
                "foundation direct issue state overflow",
            ))?)
            .and_then(|n| n.checked_add(text_storage(message)?))
            .ok_or(ProcessError::Refused(
                "foundation direct issue state overflow",
            ))?;
        utf8_bytes = utf8_bytes
            .checked_add(location.len())
            .and_then(|n| n.checked_add(message.len()))
            .ok_or(ProcessError::Refused(
                "foundation direct issue byte overflow",
            ))?;
    }
    let copy_state = direct
        .len()
        .checked_mul(size_of::<(String, String)>())
        .and_then(|n| n.checked_add(text_state))
        .ok_or(ProcessError::Refused(
            "foundation direct issue state overflow",
        ))?;
    Ok((direct.len(), text_state, utf8_bytes, copy_state))
}

fn copy_direct_issues<S: DirectIssueSource + ?Sized>(
    direct: &S,
) -> Result<Vec<(String, String)>, ProcessError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(direct.len())
        .map_err(|_| ProcessError::Refused("foundation direct issue allocation"))?;
    for index in 0..direct.len() {
        let (location, message) = direct
            .issue(index)
            .ok_or(ProcessError::Refused("foundation direct issue binding"))?;
        let mut location_copy = String::new();
        location_copy
            .try_reserve_exact(location.len())
            .map_err(|_| ProcessError::Refused("foundation direct location allocation"))?;
        location_copy.push_str(location);
        let mut message_copy = String::new();
        message_copy
            .try_reserve_exact(message.len())
            .map_err(|_| ProcessError::Refused("foundation direct message allocation"))?;
        message_copy.push_str(message);
        output.push((location_copy, message_copy));
    }
    Ok(output)
}

fn resolve_lab(
    lab: &SourceFoundationLabResult,
    diagnostics: Option<&SourceFoundationSchemaReport>,
    first_check: usize,
    limits: SourceFoundationRuleDiagnosticsLimits,
    retained_state: usize,
    issue_count: usize,
    issue_bytes: usize,
) -> Result<ResolvedLabStage, ProcessError> {
    let report_state = value_state(&lab.report).ok_or(ProcessError::Refused(
        "foundation lab report state overflow",
    ))?;
    if let Some(diagnostics) = diagnostics {
        let clone_state = lab_clone_state(lab)
            .ok_or(ProcessError::Refused("foundation lab clone state overflow"))?;
        let margin = lab_resolution_margin(lab, diagnostics, first_check).ok_or(
            ProcessError::Refused("foundation lab resolution margin overflow"),
        )?;
        let available = limits
            .max_state_bytes
            .checked_sub(retained_state)
            .ok_or(ProcessError::Refused("foundation lab state limit"))?;
        let direct_text_state = issue_text_state(&lab.ordered_issues)
            .ok_or(ProcessError::Refused("foundation lab issue state overflow"))?;
        let expected_count = lab_resolution_issue_count(lab, diagnostics, first_check)
            .ok_or(ProcessError::Refused("foundation lab issue count overflow"))?;
        if issue_count
            .checked_add(expected_count)
            .is_none_or(|n| n > limits.max_issues)
        {
            return Err(ProcessError::Refused("foundation lab issue limit"));
        }
        let expected_bytes = lab_resolution_issue_bytes(lab, diagnostics, first_check)
            .ok_or(ProcessError::Refused("foundation lab issue byte overflow"))?;
        if issue_bytes
            .checked_add(expected_bytes)
            .is_none_or(|n| n > limits.max_output_bytes)
        {
            return Err(ProcessError::Refused("foundation lab issue byte limit"));
        }
        let clone_peak = retained_state
            .checked_add(clone_state)
            .ok_or(ProcessError::Refused("foundation lab clone peak overflow"))?;
        if clone_peak > limits.max_state_bytes {
            return Err(ProcessError::Refused("foundation lab clone state limit"));
        }
        let mut copy = lab.clone();
        // Keep gaps in the untouched owner report; this resolves only the
        // local schema controls and ordinary findings represented by DTOs.
        copy.unimplemented.clear();
        let clone_after_gap_drop = clone_state
            .checked_sub(
                text_list_heap_state(&lab.unimplemented)
                    .ok_or(ProcessError::Refused("foundation lab gap state overflow"))?,
            )
            .ok_or(ProcessError::Refused("foundation lab gap state overflow"))?;
        let result = foundation_lab_resolution::resolve(
            copy,
            diagnostics,
            first_check,
            limits.max_issues.saturating_sub(issue_count),
            limits.max_output_bytes.saturating_sub(issue_bytes),
            available
                .checked_sub(clone_after_gap_drop)
                .and_then(|n| n.checked_sub(margin))
                .ok_or(ProcessError::Refused(
                    "foundation lab resolution workspace limit",
                ))?,
        )
        .map_err(ProcessError::Refused)?;
        let actual_count = issue_count
            .checked_add(result.issues.len())
            .ok_or(ProcessError::Refused("foundation lab issue count overflow"))?;
        let actual_bytes = issue_bytes
            .checked_add(
                issue_utf8_bytes(&result.issues)
                    .ok_or(ProcessError::Refused("foundation lab issue bytes overflow"))?,
            )
            .ok_or(ProcessError::Refused("foundation lab issue bytes overflow"))?;
        if actual_count > limits.max_issues || actual_bytes > limits.max_output_bytes {
            return Err(ProcessError::Refused("foundation lab output limit"));
        }
        let retained = report_state
            .checked_add(direct_text_state)
            .and_then(|bytes| bytes.checked_add(result.additional_state_bytes))
            .and_then(|bytes| bytes.checked_add(margin))
            .ok_or(ProcessError::Refused(
                "foundation lab retained state overflow",
            ))?;
        if retained_state
            .checked_add(retained)
            .is_none_or(|used| used > limits.max_state_bytes)
        {
            return Err(ProcessError::Refused("foundation lab retained state limit"));
        }
        let peak = retained_state
            .checked_add(clone_after_gap_drop)
            .and_then(|n| n.checked_add(result.additional_state_bytes))
            .and_then(|n| n.checked_add(margin))
            .ok_or(ProcessError::Refused(
                "foundation lab resolution peak overflow",
            ))?;
        Ok(ResolvedLabStage {
            value: result,
            retained_state: retained,
            peak_state: clone_peak.max(peak),
        })
    } else {
        let direct_state = issue_pair_copy_state(&lab.ordered_issues).ok_or(
            ProcessError::Refused("foundation lab direct state overflow"),
        )?;
        let retained = report_state
            .checked_add(direct_state)
            .ok_or(ProcessError::Refused(
                "foundation lab retained state overflow",
            ))?;
        if retained_state
            .checked_add(retained)
            .is_none_or(|used| used > limits.max_state_bytes)
        {
            return Err(ProcessError::Refused(
                "foundation lab no-schema state limit",
            ));
        }
        let bytes = issue_utf8_bytes(&lab.ordered_issues).ok_or(ProcessError::Refused(
            "foundation lab output bytes overflow",
        ))?;
        if issue_count
            .checked_add(lab.ordered_issues.len())
            .is_none_or(|used| used > limits.max_issues)
            || issue_bytes
                .checked_add(bytes)
                .is_none_or(|used| used > limits.max_output_bytes)
        {
            return Err(ProcessError::Refused(
                "foundation lab no-schema issue limit",
            ));
        }
        let value = ResolvedFoundationLab {
            report: lab.report.clone(),
            issues: lab.ordered_issues.clone(),
            additional_state_bytes: retained,
        };
        Ok(ResolvedLabStage {
            value,
            retained_state: retained,
            peak_state: retained_state + retained,
        })
    }
}

struct ResolvedLabStage {
    value: ResolvedFoundationLab,
    retained_state: usize,
    peak_state: usize,
}

fn interleave<S: DirectIssueSource + ?Sized>(
    direct: &S,
    placements: Vec<SchemaPlacement<'_>>,
    diagnostics: &SourceFoundationSchemaReport,
    limits: SourceFoundationRuleDiagnosticsLimits,
    retained_state: usize,
    issue_count: usize,
    issue_bytes: usize,
) -> Result<Interleaved, ProcessError> {
    let placement_state = placements
        .len()
        .checked_mul(size_of::<SchemaPlacement<'_>>())
        .ok_or(ProcessError::Refused("foundation placement state overflow"))?;
    let (direct_count, direct_text, direct_bytes, direct_copy_state) =
        direct_issue_metrics(direct)?;
    let count = direct_count
        .checked_add(schema_issue_count(diagnostics, &placements)?)
        .ok_or(ProcessError::Refused("foundation issue count overflow"))?;
    let copied_bytes = schema_issue_bytes(diagnostics, &placements)?;
    let output_bytes = issue_bytes
        .checked_add(direct_bytes)
        .and_then(|n| n.checked_add(copied_bytes))
        .ok_or(ProcessError::Refused("foundation issue byte overflow"))?;
    let total_issues = issue_count
        .checked_add(count)
        .ok_or(ProcessError::Refused("foundation issue count overflow"))?;
    let allocation_margin = schema_issue_margin(diagnostics, &placements)?;
    let helper_state = count
        .checked_mul(size_of::<(String, String)>())
        .and_then(|n| n.checked_add(copied_bytes))
        .ok_or(ProcessError::Refused(
            "foundation interleave state overflow",
        ))?;
    if total_issues > limits.max_issues || output_bytes > limits.max_output_bytes {
        return Err(ProcessError::Refused("foundation interleave output limits"));
    }
    let peak = retained_state
        .checked_add(placement_state)
        .and_then(|n| n.checked_add(direct_copy_state))
        .and_then(|n| n.checked_add(helper_state))
        .and_then(|n| n.checked_add(allocation_margin))
        .ok_or(ProcessError::Refused("foundation interleave peak overflow"))?;
    if peak > limits.max_state_bytes {
        return Err(ProcessError::Refused("foundation interleave state limit"));
    }
    let direct_copy = copy_direct_issues(direct)?;
    let output = interleave_schema_findings(
        direct_copy,
        diagnostics,
        &placements,
        limits
            .max_issues
            .checked_sub(issue_count)
            .ok_or(ProcessError::Refused("foundation issue count limit"))?,
        limits
            .max_output_bytes
            .checked_sub(issue_bytes)
            .ok_or(ProcessError::Refused("foundation issue output limit"))?,
        limits
            .max_state_bytes
            .checked_sub(retained_state)
            .and_then(|n| n.checked_sub(placement_state))
            .and_then(|n| n.checked_sub(direct_copy_state))
            .and_then(|n| n.checked_sub(allocation_margin))
            .ok_or(ProcessError::Refused(
                "foundation interleave workspace limit",
            ))?,
    )
    .map_err(ProcessError::Refused)?;
    let next_count = issue_count
        .checked_add(output.len())
        .ok_or(ProcessError::Refused("foundation issue count overflow"))?;
    let next_bytes = issue_bytes
        .checked_add(
            issue_utf8_bytes(&output)
                .ok_or(ProcessError::Refused("foundation issue bytes overflow"))?,
        )
        .ok_or(ProcessError::Refused("foundation issue bytes overflow"))?;
    let retained = direct_text
        .checked_add(helper_state)
        .and_then(|n| n.checked_add(allocation_margin))
        .ok_or(ProcessError::Refused(
            "foundation interleave retained state overflow",
        ))?;
    Ok(Interleaved {
        issues: output,
        retained_state: retained,
        issue_count: next_count,
        issue_bytes: next_bytes,
        peak_state: peak,
    })
}

fn schema_count(owner: &SourceFoundationDefaultRulesReport) -> Result<usize, &'static str> {
    if !lab_aggregate_matches(&owner.labs) {
        return Err("foundation lab aggregate binding");
    }
    let lab_count = owner
        .labs
        .results
        .iter()
        .try_fold(0usize, |n, lab| n.checked_add(lab.schema_checks.len()))
        .ok_or("foundation schema count overflow")?;
    if lab_count != owner.labs.schema_checks.len() {
        return Err("foundation lab schema request binding");
    }
    let mut count = lab_count;
    for check in &owner.records.schema_checks {
        match (
            check.decoded_instance.is_some(),
            check.legacy_raw_instance.is_some(),
            check.owner_issue,
        ) {
            (true, false, None) | (false, true, None) => {
                count = count
                    .checked_add(1)
                    .ok_or("foundation schema count overflow")?
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
                count = count
                    .checked_add(1)
                    .ok_or("foundation schema count overflow")?
            }
            (
                false,
                true,
                Some(
                    SourceFoundationRecordsOwnerIssue::JsonRootMustBeObject
                    | SourceFoundationRecordsOwnerIssue::JsonlRecordMustBeObject,
                ),
            ) => {
                count = count
                    .checked_add(1)
                    .ok_or("foundation schema count overflow")?
            }
            (false, false, Some(_)) => {}
            _ => return Err("foundation record schema request binding"),
        }
    }
    for amount in [
        owner.goldsets.schema_requests.len(),
        owner.discovery.schema_requests.len(),
        owner.closure.schema_requests.len(),
    ] {
        count = count
            .checked_add(amount)
            .ok_or("foundation schema count overflow")?;
    }
    Ok(count)
}

fn record_has_schema_input(check: &SourceFoundationRecordsSchemaCheck) -> bool {
    check.decoded_instance.is_some() || check.legacy_raw_instance.is_some()
}

fn lab_aggregate_matches(
    labs: &tos_validation::source_foundation_labs::SourceFoundationLabsReport,
) -> bool {
    let mut aggregate_issues = labs.ordered_issues.iter();
    let mut aggregate_checks = labs.schema_checks.iter();
    let mut issue_base = 0usize;
    for lab in &labs.results {
        for issue in &lab.ordered_issues {
            if aggregate_issues.next() != Some(issue) {
                return false;
            }
        }
        for check in &lab.schema_checks {
            let Some(aggregate) = aggregate_checks.next() else {
                return false;
            };
            let Some(before_issue) = issue_base.checked_add(check.before_issue) else {
                return false;
            };
            if aggregate.before_issue != before_issue
                || aggregate.location != check.location
                || aggregate.contract != check.contract
                || aggregate.instance != check.instance
                || aggregate.negative_control != check.negative_control
                || aggregate.expected_valid != check.expected_valid
                || aggregate.expected_rejected != check.expected_rejected
                || aggregate.semantic_rejected != check.semantic_rejected
                || aggregate.report_slot != check.report_slot
                || aggregate.rejection_reasons_slot != check.rejection_reasons_slot
                || aggregate.schema_message_prefix != check.schema_message_prefix
                || aggregate.mismatch_message != check.mismatch_message
            {
                return false;
            }
        }
        let Some(next_issue_base) = issue_base.checked_add(lab.ordered_issues.len()) else {
            return false;
        };
        issue_base = next_issue_base;
    }
    aggregate_issues.next().is_none() && aggregate_checks.next().is_none()
}

fn evaluator_peak_bound(
    count: usize,
    limits: SourceFoundationSchemaLimits,
    input_bytes: usize,
) -> Option<usize> {
    if count == 0 {
        return Some(0);
    }
    let per_check = size_of::<SourceFoundationSchemaInput<'_>>()
        .checked_add(size_of::<SourceFoundationSchemaCheckReport>())?
        .checked_add(256)?;
    let issue_state = limits
        .max_total_issues
        .checked_mul(size_of::<SourceFoundationSchemaIssue>())?;
    input_bytes
        .checked_add(count.checked_mul(per_check)?)
        .and_then(|n| n.checked_add(issue_state))
        .and_then(|n| n.checked_add(limits.max_total_instance_bytes))
        .and_then(|n| n.checked_add(limits.max_total_report_bytes.checked_mul(2)?))
}

fn append_lab_inputs<'a>(
    owner: &'a SourceFoundationDefaultRulesReport,
    checks: &mut Vec<SourceFoundationMixedSchemaInput<'a>>,
) {
    for lab in &owner.labs.results {
        for check in &lab.schema_checks {
            checks.push(SourceFoundationMixedSchemaInput::Decoded(
                SourceFoundationSchemaInput {
                    location: &check.location,
                    contract: &check.contract,
                    decoded_instance: &check.instance,
                },
            ));
        }
    }
}

fn append_record_inputs<'a>(
    owner: &'a SourceFoundationDefaultRulesReport,
    checks: &mut Vec<SourceFoundationMixedSchemaInput<'a>>,
) {
    for check in &owner.records.schema_checks {
        if let Some(instance) = &check.decoded_instance {
            checks.push(SourceFoundationMixedSchemaInput::Decoded(
                SourceFoundationSchemaInput {
                    location: &check.location,
                    contract: &check.contract,
                    decoded_instance: instance,
                },
            ));
        } else if let Some(raw_instance) = &check.legacy_raw_instance {
            checks.push(SourceFoundationMixedSchemaInput::Legacy(
                SourceFoundationLegacySchemaInput {
                    location: &check.location,
                    contract: &check.contract,
                    raw_instance,
                },
            ));
        }
    }
}

fn append_goldset_inputs<'a>(
    owner: &'a SourceFoundationDefaultRulesReport,
    checks: &mut Vec<SourceFoundationMixedSchemaInput<'a>>,
) {
    for request in &owner.goldsets.schema_requests {
        push_request(request, checks);
    }
}

fn append_discovery_inputs<'a>(
    owner: &'a SourceFoundationDefaultRulesReport,
    checks: &mut Vec<SourceFoundationMixedSchemaInput<'a>>,
) {
    for request in &owner.discovery.schema_requests {
        push_request(request, checks);
    }
}

fn append_closure_inputs<'a>(
    owner: &'a SourceFoundationDefaultRulesReport,
    checks: &mut Vec<SourceFoundationMixedSchemaInput<'a>>,
) {
    for request in &owner.closure.schema_requests {
        push_request(request, checks);
    }
}

fn push_request<'a>(
    request: &'a impl SchemaRequestValue,
    checks: &mut Vec<SourceFoundationMixedSchemaInput<'a>>,
) {
    checks.push(SourceFoundationMixedSchemaInput::Decoded(
        SourceFoundationSchemaInput {
            location: request.location(),
            contract: request.contract(),
            decoded_instance: request.instance(),
        },
    ));
}

trait SchemaRequestValue {
    fn before_issue(&self) -> usize;
    fn location(&self) -> &str;
    fn contract(&self) -> &str;
    fn instance(&self) -> &Value;
}

macro_rules! impl_schema_request {
    ($ty:ty) => {
        impl SchemaRequestValue for $ty {
            fn before_issue(&self) -> usize {
                self.before_issue
            }
            fn location(&self) -> &str {
                &self.location
            }
            fn contract(&self) -> &str {
                &self.contract
            }
            fn instance(&self) -> &Value {
                &self.document
            }
        }
    };
}

impl_schema_request!(GoldsetSchemaRequest);
impl_schema_request!(DiscoverySchemaRequest);
impl_schema_request!(SourceFoundationClosureSchemaRequest);

fn record_placements<'a>(
    checks: &'a [SourceFoundationRecordsSchemaCheck],
    first: usize,
    available_state: usize,
) -> Result<Vec<SchemaPlacement<'a>>, &'static str> {
    let count = checks
        .iter()
        .filter(|check| record_has_schema_input(check))
        .count();
    if count
        .checked_mul(size_of::<SchemaPlacement<'_>>())
        .is_none_or(|bytes| bytes > available_state)
    {
        return Err("foundation records placement state limit");
    }
    let mut placements = Vec::new();
    placements
        .try_reserve_exact(count)
        .map_err(|_| "foundation records placement allocation")?;
    let mut index = first;
    for check in checks {
        if !record_has_schema_input(check) {
            continue;
        }
        placements.push(SchemaPlacement {
            before_issue: check.before_issue,
            check_index: index,
            location: &check.location,
            contract: &check.contract,
            prefix: "",
        });
        index = index
            .checked_add(1)
            .ok_or("foundation records schema index overflow")?;
    }
    Ok(placements)
}

fn request_placements<'a>(
    requests: &'a [impl SchemaRequestValue],
    first: usize,
    prefix: &'static str,
    available_state: usize,
) -> Result<Vec<SchemaPlacement<'a>>, &'static str> {
    if requests
        .len()
        .checked_mul(size_of::<SchemaPlacement<'_>>())
        .is_none_or(|bytes| bytes > available_state)
    {
        return Err("foundation schema placement state limit");
    }
    let mut placements = Vec::new();
    placements
        .try_reserve_exact(requests.len())
        .map_err(|_| "foundation schema placement allocation")?;
    for (offset, request) in requests.iter().enumerate() {
        placements.push(SchemaPlacement {
            before_issue: request.before_issue(),
            check_index: first
                .checked_add(offset)
                .ok_or("foundation schema index overflow")?,
            location: request.location(),
            contract: request.contract(),
            prefix,
        });
    }
    Ok(placements)
}

fn schema_issue_count(
    report: &SourceFoundationSchemaReport,
    placements: &[SchemaPlacement<'_>],
) -> Result<usize, ProcessError> {
    placements.iter().try_fold(0usize, |count, placement| {
        let check = report
            .checks
            .get(placement.check_index)
            .ok_or(ProcessError::Refused("foundation schema placement absent"))?;
        if check.location != placement.location || check.contract != placement.contract {
            return Err(ProcessError::Refused("foundation schema placement binding"));
        }
        count
            .checked_add(check.issues.len())
            .ok_or(ProcessError::Refused(
                "foundation schema issue count overflow",
            ))
    })
}

fn schema_issue_bytes(
    report: &SourceFoundationSchemaReport,
    placements: &[SchemaPlacement<'_>],
) -> Result<usize, ProcessError> {
    placements.iter().try_fold(0usize, |total, placement| {
        let check = report
            .checks
            .get(placement.check_index)
            .ok_or(ProcessError::Refused("foundation schema placement absent"))?;
        check.issues.iter().try_fold(total, |n, issue| {
            n.checked_add(issue.location.len())
                .and_then(|n| n.checked_add(placement.prefix.len()))
                .and_then(|n| n.checked_add(issue.message.len()))
                .ok_or(ProcessError::Refused(
                    "foundation schema issue byte overflow",
                ))
        })
    })
}

fn schema_issue_margin(
    report: &SourceFoundationSchemaReport,
    placements: &[SchemaPlacement<'_>],
) -> Result<usize, ProcessError> {
    let strings = placements.iter().try_fold(0usize, |total, placement| {
        let check = report
            .checks
            .get(placement.check_index)
            .ok_or(ProcessError::Refused("foundation schema placement absent"))?;
        total
            .checked_add(
                check
                    .issues
                    .len()
                    .checked_mul(2)
                    .ok_or(ProcessError::Refused(
                        "foundation schema string count overflow",
                    ))?,
            )
            .ok_or(ProcessError::Refused(
                "foundation schema string count overflow",
            ))
    })?;
    strings.checked_mul(32).ok_or(ProcessError::Refused(
        "foundation schema string margin overflow",
    ))
}

fn direct_pairs<S: DirectIssueSource + ?Sized>(
    direct: &S,
    limits: SourceFoundationRuleDiagnosticsLimits,
    retained: usize,
    count: usize,
    bytes: usize,
) -> Result<Interleaved, ProcessError> {
    let (direct_count, _text_state, direct_bytes, state) = direct_issue_metrics(direct)?;
    let issue_count = count
        .checked_add(direct_count)
        .ok_or(ProcessError::Refused("foundation issue count overflow"))?;
    let issue_bytes = bytes
        .checked_add(direct_bytes)
        .ok_or(ProcessError::Refused("foundation issue output overflow"))?;
    if issue_count > limits.max_issues
        || issue_bytes > limits.max_output_bytes
        || retained
            .checked_add(state)
            .is_none_or(|used| used > limits.max_state_bytes)
    {
        return Err(ProcessError::Refused("foundation direct issue limits"));
    }
    let output = copy_direct_issues(direct)?;
    Ok(Interleaved {
        issues: output,
        retained_state: state,
        issue_count,
        issue_bytes,
        peak_state: retained + state,
    })
}

fn issue_utf8_bytes(issues: &[(String, String)]) -> Option<usize> {
    issues
        .iter()
        .try_fold(0usize, |total, (location, message)| {
            total
                .checked_add(location.len())?
                .checked_add(message.len())
        })
}

fn issue_text_state(issues: &[(String, String)]) -> Option<usize> {
    issues
        .iter()
        .try_fold(0usize, |total, (location, message)| {
            total
                .checked_add(text_storage(location)?)?
                .checked_add(text_storage(message)?)
        })
}

fn issue_pair_copy_state(issues: &[(String, String)]) -> Option<usize> {
    issues
        .len()
        .checked_mul(size_of::<(String, String)>())?
        .checked_add(issue_text_state(issues)?)
}

fn text_storage(text: &str) -> Option<usize> {
    text.len().checked_mul(2)?.checked_add(32)
}

fn text_list_heap_state(texts: &[String]) -> Option<usize> {
    texts
        .iter()
        .try_fold(0usize, |n, text| n.checked_add(text_storage(text)?))
}

fn value_state(value: &Value) -> Option<usize> {
    fn walk(value: &Value, total: &mut usize, depth: usize) -> Option<()> {
        if depth > 128 {
            return None;
        }
        *total = total.checked_add(size_of::<Value>())?;
        match value {
            Value::String(text) => *total = total.checked_add(text_storage(text)?)?,
            Value::Array(values) => {
                *total = total.checked_add(
                    values
                        .len()
                        .checked_mul(size_of::<Value>().checked_mul(2)?.checked_add(64)?)?,
                )?;
                for value in values {
                    walk(value, total, depth + 1)?;
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    *total = total.checked_add(text_storage(key)?.checked_add(160)?)?;
                    walk(value, total, depth + 1)?;
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
        Some(())
    }
    let mut total = 0usize;
    walk(value, &mut total, 0)?;
    Some(total)
}

fn lab_clone_state(lab: &SourceFoundationLabResult) -> Option<usize> {
    let mut total = size_of::<SourceFoundationLabResult>()
        .checked_add(value_state(&lab.report)?)?
        .checked_add(issue_pair_copy_state(&lab.ordered_issues)?)?
        .checked_add(lab.unimplemented.iter().try_fold(
            lab.unimplemented.len().checked_mul(size_of::<String>())?,
            |n, text| n.checked_add(text_storage(text)?),
        )?)?
        .checked_add(lab.schema_checks.len().checked_mul(size_of::<
            tos_validation::source_foundation_labs::SourceFoundationSchemaCheck,
        >())?)?;
    for check in &lab.schema_checks {
        for text in [
            Some(check.location.as_str()),
            Some(check.contract.as_str()),
            check.negative_control.as_deref(),
            check.report_slot.as_deref(),
            check.rejection_reasons_slot.as_deref(),
            check.schema_message_prefix.as_deref(),
            check.mismatch_message.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.checked_add(text_storage(text)?)?;
        }
        total = total.checked_add(value_state(&check.instance)?)?;
    }
    Some(total)
}

fn lab_resolution_margin(
    lab: &SourceFoundationLabResult,
    report: &SourceFoundationSchemaReport,
    first: usize,
) -> Option<usize> {
    let checks = report
        .checks
        .get(first..first.checked_add(lab.schema_checks.len())?)?;
    let strings =
        lab.schema_checks
            .iter()
            .zip(checks)
            .try_fold(0usize, |total, (request, check)| {
                let mut count = 0usize;
                if request.negative_control.is_none() {
                    count = count.checked_add(check.issues.len().checked_mul(2)?)?;
                }
                if request.mismatch_message.is_some() {
                    count = count.checked_add(2)?;
                }
                if request.rejection_reasons_slot.is_some() {
                    count = count.checked_add(check.issues.len())?;
                }
                if request.negative_control.is_some() && request.report_slot.is_some() {
                    count = count.checked_add(1)?;
                }
                total.checked_add(count)
            })?;
    strings.checked_mul(32)
}

fn lab_resolution_issue_count(
    lab: &SourceFoundationLabResult,
    report: &SourceFoundationSchemaReport,
    first: usize,
) -> Option<usize> {
    let checks = report
        .checks
        .get(first..first.checked_add(lab.schema_checks.len())?)?;
    lab.schema_checks.iter().zip(checks).try_fold(
        lab.ordered_issues.len(),
        |mut count, (request, check)| {
            if request.negative_control.is_none() {
                count = count.checked_add(check.issues.len())?;
            }
            let valid = check.diagnostic.is_valid();
            let rejected = !valid || request.semantic_rejected.unwrap_or(false);
            let mismatch = if request.negative_control.is_some() {
                request
                    .expected_rejected
                    .is_some_and(|expected| expected != rejected)
            } else {
                request
                    .expected_valid
                    .is_some_and(|expected| expected != valid)
            };
            count.checked_add(usize::from(mismatch))
        },
    )
}

fn lab_resolution_issue_bytes(
    lab: &SourceFoundationLabResult,
    report: &SourceFoundationSchemaReport,
    first: usize,
) -> Option<usize> {
    let checks = report
        .checks
        .get(first..first.checked_add(lab.schema_checks.len())?)?;
    lab.schema_checks.iter().zip(checks).try_fold(
        issue_utf8_bytes(&lab.ordered_issues)?,
        |bytes, (request, check)| {
            let valid = check.diagnostic.is_valid();
            let rejected = !valid || request.semantic_rejected.unwrap_or(false);
            let mismatch = if request.negative_control.is_some() {
                request
                    .expected_rejected
                    .is_some_and(|expected| expected != rejected)
            } else {
                request
                    .expected_valid
                    .is_some_and(|expected| expected != valid)
            };
            let mut next = bytes;
            if request.negative_control.is_none() {
                for issue in &check.issues {
                    next = next
                        .checked_add(issue.location.len())?
                        .checked_add(
                            request
                                .schema_message_prefix
                                .as_ref()
                                .map_or(0, String::len),
                        )?
                        .checked_add(issue.message.len())?;
                }
            }
            if mismatch {
                next = next
                    .checked_add(request.location.len())?
                    .checked_add(request.mismatch_message.as_ref()?.len())?;
            }
            Some(next)
        },
    )
}

fn schema_report_state(report: &SourceFoundationSchemaReport) -> Option<usize> {
    let issues = report
        .checks
        .iter()
        .try_fold(0usize, |count, check| count.checked_add(check.issues.len()))?;
    let structure = size_of::<SourceFoundationSchemaReport>()
        .checked_add(report.checkpoints.len().checked_mul(128)?)?
        .checked_add(
            report
                .checks
                .len()
                .checked_mul(size_of::<SourceFoundationSchemaCheckReport>())?,
        )?
        .checked_add(issues.checked_mul(size_of::<SourceFoundationSchemaIssue>())?)?;
    report
        .cost
        .metadata_bytes
        .checked_add(report.cost.estimated_report_bytes)?
        .checked_add(structure)
}

fn refused(
    owner_report: SourceFoundationDefaultRulesReport,
    diagnostics: Option<SourceFoundationSchemaReport>,
    reason: &'static str,
) -> SourceFoundationRuleDiagnosticsError {
    SourceFoundationRuleDiagnosticsError::Refused {
        owner_report,
        diagnostics,
        reason,
    }
}
