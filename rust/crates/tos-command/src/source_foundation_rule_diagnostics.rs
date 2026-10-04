//! Diagnostic-v2 evaluation and ordered CLI issue assembly for the default
//! source-foundation districts. It preserves controls and owner gaps and does
//! not produce a whole-foundation validity verdict.

use super::foundation_cli::{SchemaPlacement, interleave_schema_findings};
use super::foundation_lab_resolution::{self, ResolvedFoundationLab};
use serde_json::Value;
use std::io::{self, Write};
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_validation::FormatProfile;
use tos_validation::executor::{
    ExactWorkerIdentity, SharedSchemaWorkerQuota, VerifiedWorkerImageHandle,
};
use tos_validation::source_cut::{
    CandidateCutSchemaDiagnostic, CandidateCutWorkerSchemaExecutor, CutPreparedSchemaProtocol,
};
use tos_validation::source_foundation_closure::{
    SourceFoundationClosureSchemaRequest, SourceFoundationClosureSchemaRequestStore,
};
use tos_validation::source_foundation_default_rules::{
    SourceFoundationDefaultRulesReport, SourceFoundationDefaultRulesStoredReport,
};
use tos_validation::source_foundation_discovery::{
    DiscoverySchemaRequestStore, Issue as DiscoveryIssue, SchemaRequest as DiscoverySchemaRequest,
};
use tos_validation::source_foundation_goldsets::SourceFoundationSchemaRequest as GoldsetSchemaRequest;
use tos_validation::source_foundation_labs::{
    SourceFoundationLab, SourceFoundationLabResult, SourceFoundationSchemaCheck,
};
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

/// Cost and evidence returned by the actual candidate diagnostic worker. The
/// stored owner report contains no Records payload; Records executions are
/// represented only by the worker's already-completed cumulative counters.
pub(crate) struct EvaluatedCandidateSourceFoundationRules<I> {
    pub owner_report: SourceFoundationDefaultRulesStoredReport<I>,
    pub diagnostics: Vec<CandidateCutSchemaDiagnostic<I>>,
    pub cost: CandidateSourceFoundationRuleDiagnosticsCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CandidateSourceFoundationRuleDiagnosticsCost {
    pub schema_check_count: usize,
    pub schema_input_instance_bytes: usize,
    pub schema_result_state_upper_bound_bytes: usize,
    pub typed_diagnostic_vector_state_upper_bound_bytes: usize,
    pub diagnostic_issue_count: usize,
    pub worker_schema_resource_bytes: u64,
    pub worker_request_bytes: u64,
    pub worker_response_bytes: u64,
    pub worker_cpu_micros: u64,
    pub cumulative_execution_count_before: usize,
    pub cumulative_execution_count_after: usize,
    pub peak_additional_state_upper_bound_bytes: usize,
}

/// A refused or incomplete candidate run keeps the real stored owner report
/// and every complete typed diagnostic already produced. It never introduces
/// a SourceRevision-shaped placeholder.
pub(crate) enum CandidateRuleDiagnosticsError<I> {
    Refused {
        owner_report: SourceFoundationDefaultRulesStoredReport<I>,
        diagnostics: Vec<CandidateCutSchemaDiagnostic<I>>,
        reason: &'static str,
    },
    Incomplete {
        owner_report: SourceFoundationDefaultRulesStoredReport<I>,
        diagnostics: Vec<CandidateCutSchemaDiagnostic<I>>,
        reason: &'static str,
    },
}

impl<I> CandidateRuleDiagnosticsError<I> {
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::Refused { reason, .. } | Self::Incomplete { reason, .. } => reason,
        }
    }
}

struct CandidateRuleStepFailure<I> {
    diagnostic: Option<CandidateCutSchemaDiagnostic<I>>,
    reason: &'static str,
    incomplete: bool,
}

struct CandidateRuleStepSuccess<I> {
    diagnostic: CandidateCutSchemaDiagnostic<I>,
    input_bytes: usize,
    diagnostic_state: usize,
    issue_count: usize,
    peak_state: usize,
}

const CANDIDATE_INSTANCE_WRITE_CHUNK_BYTES: usize = 64 * 1024;

struct CandidateInstanceWriter<'a, W> {
    inner: W,
    bytes: usize,
    maximum: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    failure: Option<&'static str>,
}

impl<W: Write> Write for CandidateInstanceWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancelled.load(Ordering::Relaxed) {
            self.failure = Some("candidate rule diagnostics cancelled");
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "candidate rule diagnostics cancelled",
            ));
        }
        if Instant::now() >= self.deadline {
            self.failure = Some("candidate rule diagnostics deadline");
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "candidate rule diagnostics deadline",
            ));
        }
        let next = self
            .bytes
            .checked_add(bytes.len())
            .filter(|used| *used <= self.maximum);
        let Some(next) = next else {
            self.failure = Some("candidate schema instance bound");
            return Err(io::Error::other("candidate schema instance bound"));
        };
        let chunk_bytes = bytes.len().min(CANDIDATE_INSTANCE_WRITE_CHUNK_BYTES);
        let written = self.inner.write(&bytes[..chunk_bytes])?;
        self.bytes = self
            .bytes
            .checked_add(written)
            .filter(|used| *used <= self.maximum)
            .ok_or_else(|| io::Error::other("candidate schema instance bound"))?;
        debug_assert!(self.bytes <= next);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn write_candidate_instance<W: Write>(
    value: &Value,
    writer: &mut CandidateInstanceWriter<'_, W>,
) -> Result<(), &'static str> {
    serde_json::to_writer(&mut *writer, value).map_err(|_| {
        writer
            .failure
            .unwrap_or("candidate schema instance encoding")
    })
}

fn candidate_instance_encoded_len(
    value: &Value,
    maximum: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<usize, &'static str> {
    let mut counter = CandidateInstanceWriter {
        inner: io::sink(),
        bytes: 0,
        maximum,
        deadline,
        cancelled,
        failure: None,
    };
    write_candidate_instance(value, &mut counter)?;
    Ok(counter.bytes)
}

fn encode_candidate_instance(
    value: &Value,
    encoded_len: usize,
    maximum: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, &'static str> {
    if encoded_len > maximum || encoded_len > max_state_bytes {
        return Err("candidate schema instance state limit");
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err("candidate rule diagnostics cancelled");
    }
    if Instant::now() >= deadline {
        return Err("candidate rule diagnostics deadline");
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(encoded_len)
        .map_err(|_| "candidate schema instance allocation")?;
    if raw.capacity() > maximum || raw.capacity() > max_state_bytes {
        return Err("candidate schema instance buffer capacity limit");
    }
    let mut writer = CandidateInstanceWriter {
        inner: &mut raw,
        bytes: 0,
        maximum,
        deadline,
        cancelled,
        failure: None,
    };
    write_candidate_instance(value, &mut writer)?;
    let written = writer.bytes;
    drop(writer);
    if raw.len() != encoded_len || written != encoded_len {
        return Err("candidate schema instance encoding changed");
    }
    Ok(raw)
}

fn candidate_request_count<I>(
    owner: &SourceFoundationDefaultRulesStoredReport<I>,
    discovery_schema_requests: &dyn DiscoverySchemaRequestStore,
    closure_schema_requests: &dyn SourceFoundationClosureSchemaRequestStore,
) -> Result<usize, &'static str> {
    if !lab_aggregate_matches(&owner.labs) {
        return Err("candidate stored lab schema request binding");
    }
    let request_store_cost = discovery_schema_requests.cost();
    let reported_spooled_count = owner
        .discovery
        .cost
        .candidate_discovery_schema_request_count;
    if request_store_cost.observation_rows != reported_spooled_count
        || (reported_spooled_count > 0 && !owner.discovery.schema_requests.is_empty())
    {
        return Err("candidate stored Discovery schema spool binding");
    }
    let spooled_count = usize::try_from(reported_spooled_count)
        .map_err(|_| "candidate stored Discovery schema spool count overflow")?;
    let closure_store_cost = closure_schema_requests.cost();
    let reported_closure_spooled_count = owner.closure.cost.candidate_schema_request_count;
    let closure_spooled_count = usize::try_from(reported_closure_spooled_count)
        .map_err(|_| "candidate stored Closure schema spool count overflow")?;
    let closure_total_count = usize::try_from(owner.closure.cost.schema_requests)
        .map_err(|_| "candidate stored Closure schema request count overflow")?;
    if closure_store_cost.observation_rows != reported_closure_spooled_count
        || (reported_closure_spooled_count > 0 && !owner.closure.schema_requests.is_empty())
        || closure_spooled_count.checked_add(owner.closure.schema_requests.len())
            != Some(closure_total_count)
    {
        return Err("candidate stored Closure schema spool binding");
    }
    let mut count = 0usize;
    for amount in [
        owner.labs.schema_checks.len(),
        owner.goldsets.schema_requests.len(),
        owner.discovery.schema_requests.len(),
        spooled_count,
        owner.closure.schema_requests.len(),
        closure_spooled_count,
    ] {
        count = count
            .checked_add(amount)
            .ok_or("candidate stored schema request count overflow")?;
    }
    for lab in &owner.labs.results {
        if !request_ordinals_valid(&lab.schema_checks, lab.ordered_issues.len()) {
            return Err("candidate stored lab schema ordinals invalid");
        }
    }
    if !request_ordinals_valid(
        &owner.goldsets.schema_requests,
        owner.goldsets.ordered_issues.len(),
    ) || !request_ordinals_valid(
        &owner.discovery.schema_requests,
        owner.discovery.issues.len(),
    ) || !request_ordinals_valid(&owner.closure.schema_requests, owner.closure.issues.len())
    {
        return Err("candidate stored schema ordinals invalid");
    }
    Ok(count)
}

fn request_ordinals_valid<R: SchemaRequestValue>(requests: &[R], direct_issues: usize) -> bool {
    let mut prior = 0usize;
    for request in requests {
        let ordinal = request.before_issue();
        if ordinal < prior || ordinal > direct_issues {
            return false;
        }
        prior = ordinal;
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn check_candidate_rule_request<I: Copy + Eq>(
    request: &impl SchemaRequestValue,
    input_identity: I,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    expected_binding: tos_validation::source_cut::CutPreparedSchemaExecutionBinding,
    schema_limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    limits: SourceFoundationRuleDiagnosticsLimits,
    prior_diagnostic_state: usize,
    request_workspace_state: usize,
    diagnostic_vector_state: usize,
    input_bytes_used: usize,
    diagnostic_issue_count: usize,
    prior_peak_state: usize,
) -> Result<CandidateRuleStepSuccess<I>, CandidateRuleStepFailure<I>> {
    let fail = |reason, diagnostic| CandidateRuleStepFailure {
        diagnostic,
        reason,
        incomplete: false,
    };
    if cancelled.load(Ordering::Relaxed) {
        return Err(fail("candidate rule diagnostics cancelled", None));
    }
    if Instant::now() >= deadline {
        return Err(fail("candidate rule diagnostics deadline", None));
    }
    let location = request.location();
    let contract = request.contract();
    if location.is_empty() || location.len() > 4096 || contract.is_empty() || contract.len() > 4096
    {
        return Err(fail(
            "candidate rule diagnostics location or contract limit",
            None,
        ));
    }
    let base_state = prior_diagnostic_state
        .checked_add(request_workspace_state)
        .and_then(|bytes| bytes.checked_add(diagnostic_vector_state))
        .and_then(|bytes| {
            bytes.checked_add(size_of::<EvaluatedCandidateSourceFoundationRules<I>>())
        })
        .ok_or_else(|| fail("candidate rule diagnostics state overflow", None))?;
    let input_state_limit = limits
        .max_state_bytes
        .checked_sub(base_state)
        .ok_or_else(|| fail("candidate rule diagnostics state limit", None))?;
    let remaining_instance_bytes = schema_limits
        .max_total_instance_bytes
        .checked_sub(input_bytes_used)
        .ok_or_else(|| fail("candidate rule diagnostics aggregate input limit", None))?;
    let maximum_instance_bytes = schema_limits
        .max_instance_bytes
        .min(remaining_instance_bytes);
    let encoded_len = candidate_instance_encoded_len(
        request.instance(),
        maximum_instance_bytes,
        deadline,
        cancelled,
    )
    .map_err(|reason| CandidateRuleStepFailure {
        diagnostic: None,
        reason,
        incomplete: matches!(
            reason,
            "candidate rule diagnostics cancelled" | "candidate rule diagnostics deadline"
        ),
    })?;
    if encoded_len > input_state_limit {
        return Err(fail("candidate schema instance state limit", None));
    }
    let input_bytes = input_bytes_used
        .checked_add(encoded_len)
        .filter(|used| *used <= schema_limits.max_total_instance_bytes)
        .ok_or_else(|| fail("candidate rule diagnostics aggregate input limit", None))?;
    let worker_controller_state = worker
        .diagnostics_v2_controller_state_upper_bound(encoded_len, location.len())
        .map_err(|_| {
            fail(
                "candidate rule diagnostics controller state unavailable",
                None,
            )
        })?;
    let preallocation_peak = base_state
        .checked_add(encoded_len)
        .and_then(|bytes| bytes.checked_add(worker_controller_state))
        .ok_or_else(|| fail("candidate rule diagnostics controller state overflow", None))?;
    if preallocation_peak > limits.max_state_bytes {
        return Err(fail(
            "candidate rule diagnostics controller state limit",
            None,
        ));
    }
    let buffer_state_limit = input_state_limit
        .checked_sub(worker_controller_state)
        .ok_or_else(|| fail("candidate rule diagnostics controller state limit", None))?;
    let raw = encode_candidate_instance(
        request.instance(),
        encoded_len,
        maximum_instance_bytes,
        buffer_state_limit,
        deadline,
        cancelled,
    )
    .map_err(|reason| CandidateRuleStepFailure {
        diagnostic: None,
        reason,
        incomplete: matches!(
            reason,
            "candidate rule diagnostics cancelled" | "candidate rule diagnostics deadline"
        ),
    })?;
    let call_peak = base_state
        .checked_add(raw.capacity())
        .and_then(|bytes| bytes.checked_add(worker_controller_state))
        .ok_or_else(|| fail("candidate rule diagnostics controller state overflow", None))?;
    if call_peak > limits.max_state_bytes {
        return Err(fail(
            "candidate rule diagnostics actual buffer capacity exceeds state limit",
            None,
        ));
    }
    let diagnostic = worker
        .check_diagnostics_v2(location, &raw, contract, deadline, cancelled)
        .map_err(|_| fail("candidate rule diagnostics worker refused", None))?;
    if diagnostic.input_identity() != &input_identity
        || diagnostic.schema_set_sha256() != worker.schema_set_digest()
        || diagnostic.contract_selection_sha256() != worker.contract_selection_digest()
        || diagnostic.profile() != worker.profile()
        || diagnostic.profile() != FormatProfile::LegacyPythonObserved20260923
        || diagnostic.prepared_execution_binding() != expected_binding
        || diagnostic.result().path() != location
        || diagnostic.result().contract() != contract
    {
        return Err(fail(
            "candidate rule diagnostics result binding invalid",
            Some(diagnostic),
        ));
    }
    if !diagnostic.is_valid() && !diagnostic.is_invalid() {
        let reason = match (diagnostic.report().status, diagnostic.report().failure) {
            (
                tos_validation::executor::schema_diagnostics::Status::Indeterminate,
                tos_validation::executor::schema_diagnostics::Failure::UnsupportedInputSemantics,
            ) => "candidate rule diagnostics unsupported input semantics",
            (
                tos_validation::executor::schema_diagnostics::Status::Indeterminate,
                tos_validation::executor::schema_diagnostics::Failure::ValidatorRuntime,
            ) => "candidate rule diagnostics validator runtime indeterminate",
            _ => "candidate rule diagnostics incomplete",
        };
        return Err(CandidateRuleStepFailure {
            diagnostic: Some(diagnostic),
            reason,
            incomplete: true,
        });
    }
    let Some(next_issues) = diagnostic_issue_count
        .checked_add(diagnostic.report().issues.len())
        .filter(|count| *count <= schema_limits.max_total_issues)
    else {
        return Err(fail(
            "candidate rule diagnostics issue limit",
            Some(diagnostic),
        ));
    };
    let Some(returned_state) =
        prior_diagnostic_state.checked_add(diagnostic.accounted_state_bytes())
    else {
        return Err(fail(
            "candidate rule diagnostics result state overflow",
            Some(diagnostic),
        ));
    };
    let Some(peak) = returned_state
        .checked_add(request_workspace_state)
        .and_then(|bytes| bytes.checked_add(diagnostic_vector_state))
        .and_then(|bytes| {
            bytes.checked_add(size_of::<EvaluatedCandidateSourceFoundationRules<I>>())
        })
        .and_then(|bytes| bytes.checked_add(raw.capacity()))
        .and_then(|bytes| bytes.checked_add(worker_controller_state))
    else {
        return Err(fail(
            "candidate rule diagnostics peak state overflow",
            Some(diagnostic),
        ));
    };
    if peak > limits.max_state_bytes {
        return Err(fail(
            "candidate rule diagnostics result state limit",
            Some(diagnostic),
        ));
    }
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(CandidateRuleStepFailure {
            diagnostic: Some(diagnostic),
            reason: "candidate rule diagnostics ended at deadline or cancellation",
            incomplete: true,
        });
    }
    Ok(CandidateRuleStepSuccess {
        diagnostic,
        input_bytes,
        diagnostic_state: returned_state,
        issue_count: next_issues,
        peak_state: prior_peak_state
            .max(preallocation_peak)
            .max(call_peak)
            .max(peak),
    })
}

/// Evaluate only the stored default-rule request DTOs through the actual
/// candidate schema worker. The same worker instance and cumulative counters
/// that already cover candidate Records are continued without a quota reset.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_candidate_stored_rules<I: Copy + Eq>(
    mut owner_report: SourceFoundationDefaultRulesStoredReport<I>,
    discovery_schema_requests: &mut dyn DiscoverySchemaRequestStore,
    closure_schema_requests: &mut dyn SourceFoundationClosureSchemaRequestStore,
    schema_worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    schema_limits: SourceFoundationSchemaLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    limits: SourceFoundationRuleDiagnosticsLimits,
) -> Result<EvaluatedCandidateSourceFoundationRules<I>, CandidateRuleDiagnosticsError<I>> {
    let mut diagnostics = Vec::new();
    let request_count = match candidate_request_count(
        &owner_report,
        discovery_schema_requests,
        closure_schema_requests,
    ) {
        Ok(count) => count,
        Err(reason) => {
            return Err(CandidateRuleDiagnosticsError::Refused {
                owner_report,
                diagnostics,
                reason,
            });
        }
    };
    let input_identity = owner_report.input_identity;
    let worker_binding = schema_worker.prepared_execution_binding();
    if !schema_limits.validate()
        || limits.max_state_bytes == 0
        || schema_worker.input_identity() != &input_identity
        || schema_worker.profile() != FormatProfile::LegacyPythonObserved20260923
        || schema_worker.limits_sha256() != schema_limits.digest()
        || schema_worker.is_finished()
        || !matches!(
            worker_binding.protocol,
            CutPreparedSchemaProtocol::DiagnosticsV2 { .. }
        )
    {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule worker binding invalid",
        });
    }
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule diagnostics deadline or cancellation",
        });
    }
    if request_count > schema_limits.max_checks
        || owner_report.cost.queued_schema_document_count > schema_limits.max_checks
    {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule schema count limit",
        });
    }
    let before_count = match schema_worker.diagnostic_execution_count() {
        Ok(count) => count,
        Err(_) => {
            return Err(CandidateRuleDiagnosticsError::Refused {
                owner_report,
                diagnostics,
                reason: "candidate stored rule cumulative execution count unavailable",
            });
        }
    };
    let before_cost = match schema_worker.diagnostics_v2_cumulative_cost() {
        Ok(cost) => cost,
        Err(_) => {
            return Err(CandidateRuleDiagnosticsError::Refused {
                owner_report,
                diagnostics,
                reason: "candidate stored rule cumulative worker cost unavailable",
            });
        }
    };
    let expected_total = before_count.checked_add(request_count);
    if expected_total != Some(owner_report.cost.queued_schema_document_count)
        || usize::try_from(before_cost.completed_exchanges()).ok() != Some(before_count)
    {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule prior diagnostic count binding invalid",
        });
    }
    let requested_vector_state =
        match request_count.checked_mul(size_of::<CandidateCutSchemaDiagnostic<I>>()) {
            Some(bytes) if bytes <= limits.max_state_bytes => bytes,
            _ => {
                return Err(CandidateRuleDiagnosticsError::Refused {
                    owner_report,
                    diagnostics,
                    reason: "candidate stored rule diagnostic vector state limit",
                });
            }
        };
    if requested_vector_state
        .checked_add(size_of::<EvaluatedCandidateSourceFoundationRules<I>>())
        .is_none_or(|bytes| bytes > limits.max_state_bytes)
    {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule output header state limit",
        });
    }
    diagnostics.try_reserve_exact(request_count).map_err(|_| {
        CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics: Vec::new(),
            reason: "candidate stored rule diagnostic allocation",
        }
    })?;
    let Some(vector_state) = diagnostics
        .capacity()
        .checked_mul(size_of::<CandidateCutSchemaDiagnostic<I>>())
    else {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule diagnostic vector capacity limit",
        });
    };
    if vector_state > limits.max_state_bytes || vector_state < requested_vector_state {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule diagnostic vector capacity limit",
        });
    }
    let mut input_bytes = 0usize;
    let mut result_state = 0usize;
    let mut issue_count = 0usize;
    let mut peak_state =
        match vector_state.checked_add(size_of::<EvaluatedCandidateSourceFoundationRules<I>>()) {
            Some(bytes) if bytes <= limits.max_state_bytes => bytes,
            _ => {
                return Err(CandidateRuleDiagnosticsError::Refused {
                    owner_report,
                    diagnostics,
                    reason: "candidate stored rule output header state limit",
                });
            }
        };
    let mut failed = None;
    macro_rules! run_request {
        ($request:expr, $workspace_state:expr) => {
            match check_candidate_rule_request(
                $request,
                input_identity,
                schema_worker,
                worker_binding,
                schema_limits,
                deadline,
                cancelled,
                limits,
                result_state,
                $workspace_state,
                vector_state,
                input_bytes,
                issue_count,
                peak_state,
            ) {
                Ok(success) => {
                    input_bytes = success.input_bytes;
                    issue_count = success.issue_count;
                    result_state = success.diagnostic_state;
                    peak_state = success.peak_state;
                    diagnostics.push(success.diagnostic);
                }
                Err(failure) => {
                    if let Some(diagnostic) = failure.diagnostic {
                        diagnostics.push(diagnostic);
                    }
                    failed = Some((failure.reason, failure.incomplete));
                }
            }
        };
    }
    'requests: {
        for lab in &owner_report.labs.results {
            for request in &lab.schema_checks {
                run_request!(request, 0);
                if failed.is_some() {
                    break 'requests;
                }
            }
        }
        for request in &owner_report.goldsets.schema_requests {
            run_request!(request, 0);
            if failed.is_some() {
                break 'requests;
            }
        }
        for request in &owner_report.discovery.schema_requests {
            run_request!(request, 0);
            if failed.is_some() {
                break 'requests;
            }
        }
        let expected_spooled_requests = usize::try_from(
            owner_report
                .discovery
                .cost
                .candidate_discovery_schema_request_count,
        );
        let expected_spooled_requests = match expected_spooled_requests {
            Ok(count) => count,
            Err(_) => {
                failed = Some(("candidate Discovery schema spool count overflow", false));
                break 'requests;
            }
        };
        let mut discovery_eof = false;
        loop {
            if failed.is_some() {
                break;
            }
            let remaining_state = limits
                .max_state_bytes
                .checked_sub(result_state)
                .and_then(|bytes| bytes.checked_sub(vector_state))
                .and_then(|bytes| {
                    bytes.checked_sub(size_of::<EvaluatedCandidateSourceFoundationRules<I>>())
                });
            let Some(remaining_state) = remaining_state else {
                failed = Some(("candidate Discovery schema spool state limit", false));
                break;
            };
            match discovery_schema_requests.next_request(remaining_state) {
                Ok((Some(request), workspace_state)) => {
                    run_request!(&request, workspace_state);
                }
                Ok((None, workspace_state)) => {
                    let eof_peak = result_state
                        .checked_add(workspace_state)
                        .and_then(|bytes| bytes.checked_add(vector_state))
                        .and_then(|bytes| {
                            bytes.checked_add(size_of::<
                                    EvaluatedCandidateSourceFoundationRules<I>,
                                >())
                        });
                    match eof_peak {
                        Some(peak) if peak <= limits.max_state_bytes => {
                            peak_state = peak_state.max(peak);
                            discovery_eof = true;
                        }
                        _ => {
                            failed =
                                Some(("candidate Discovery schema spool EOF state limit", false));
                        }
                    }
                    break;
                }
                Err(_) => {
                    failed = Some((
                        "candidate Discovery schema spool read or EOF refused",
                        false,
                    ));
                    break;
                }
            }
            if failed.is_some() {
                break;
            }
        }
        let spool_cost = discovery_schema_requests.cost();
        if spool_cost.observation_rows
            != owner_report
                .discovery
                .cost
                .candidate_discovery_schema_request_count
            || usize::try_from(spool_cost.observation_rows).ok() != Some(expected_spooled_requests)
            || spool_cost.serialized_write_bytes
                != owner_report
                    .discovery
                    .cost
                    .candidate_discovery_schema_request_serialized_write_bytes
            || (failed.is_none()
                && (!discovery_eof
                    || (spool_cost.serialized_read_bytes == 0 && expected_spooled_requests > 0)))
        {
            failed = Some((
                "candidate Discovery schema spool count or EOF binding invalid",
                false,
            ));
        }
        owner_report
            .discovery
            .cost
            .candidate_discovery_schema_request_serialized_read_bytes =
            spool_cost.serialized_read_bytes;
        owner_report
            .discovery
            .cost
            .candidate_discovery_schema_request_peak_workspace_state_bytes = owner_report
            .discovery
            .cost
            .candidate_discovery_schema_request_peak_workspace_state_bytes
            .max(spool_cost.workspace_state_bytes);
        owner_report
            .discovery
            .cost
            .candidate_discovery_schema_request_scan_row_operations =
            spool_cost.scan_row_operations;
        if failed.is_some() {
            break 'requests;
        }
        let expected_closure_requests =
            usize::try_from(owner_report.closure.cost.candidate_schema_request_count);
        let expected_closure_requests = match expected_closure_requests {
            Ok(count) => count,
            Err(_) => {
                failed = Some(("candidate Closure schema spool count overflow", false));
                break 'requests;
            }
        };
        let mut closure_eof = false;
        loop {
            if failed.is_some() {
                break;
            }
            let remaining_state = limits
                .max_state_bytes
                .checked_sub(result_state)
                .and_then(|bytes| bytes.checked_sub(vector_state))
                .and_then(|bytes| {
                    bytes.checked_sub(size_of::<EvaluatedCandidateSourceFoundationRules<I>>())
                });
            let Some(remaining_state) = remaining_state else {
                failed = Some(("candidate Closure schema spool state limit", false));
                break;
            };
            match closure_schema_requests.next_request(remaining_state) {
                Ok((Some(request), workspace_state)) => {
                    run_request!(&request, workspace_state);
                }
                Ok((None, workspace_state)) => {
                    let eof_peak = result_state
                        .checked_add(workspace_state)
                        .and_then(|bytes| bytes.checked_add(vector_state))
                        .and_then(|bytes| {
                            bytes.checked_add(size_of::<
                                    EvaluatedCandidateSourceFoundationRules<I>,
                                >())
                        });
                    match eof_peak {
                        Some(peak) if peak <= limits.max_state_bytes => {
                            peak_state = peak_state.max(peak);
                            closure_eof = true;
                        }
                        _ => {
                            failed =
                                Some(("candidate Closure schema spool EOF state limit", false));
                        }
                    }
                    break;
                }
                Err(_) => {
                    failed = Some(("candidate Closure schema spool read or EOF refused", false));
                    break;
                }
            }
            if failed.is_some() {
                break;
            }
        }
        let closure_spool_cost = closure_schema_requests.cost();
        if closure_spool_cost.observation_rows
            != owner_report.closure.cost.candidate_schema_request_count
            || usize::try_from(closure_spool_cost.observation_rows).ok()
                != Some(expected_closure_requests)
            || closure_spool_cost.serialized_write_bytes
                != owner_report
                    .closure
                    .cost
                    .candidate_schema_request_serialized_write_bytes
            || (failed.is_none()
                && (!closure_eof
                    || (closure_spool_cost.serialized_read_bytes == 0
                        && expected_closure_requests > 0)))
        {
            failed = Some((
                "candidate Closure schema spool count or EOF binding invalid",
                false,
            ));
        }
        if failed.is_none() && closure_schema_requests.verify_finished().is_err() {
            failed = Some((
                "candidate Closure schema spool final drain binding invalid",
                false,
            ));
        }
        owner_report
            .closure
            .cost
            .candidate_schema_request_serialized_read_bytes =
            closure_spool_cost.serialized_read_bytes;
        owner_report
            .closure
            .cost
            .candidate_schema_request_peak_workspace_state_bytes = owner_report
            .closure
            .cost
            .candidate_schema_request_peak_workspace_state_bytes
            .max(closure_spool_cost.workspace_state_bytes);
        owner_report
            .closure
            .cost
            .candidate_schema_request_scan_row_operations = closure_spool_cost.scan_row_operations;
        if failed.is_some() {
            break 'requests;
        }
        for request in &owner_report.closure.schema_requests {
            run_request!(request, 0);
            if failed.is_some() {
                break 'requests;
            }
        }
    }
    if let Some((reason, incomplete)) = failed {
        return Err(if incomplete {
            CandidateRuleDiagnosticsError::Incomplete {
                owner_report,
                diagnostics,
                reason,
            }
        } else {
            CandidateRuleDiagnosticsError::Refused {
                owner_report,
                diagnostics,
                reason,
            }
        });
    }
    let after_count = match schema_worker.diagnostic_execution_count() {
        Ok(count) => count,
        Err(_) => {
            return Err(CandidateRuleDiagnosticsError::Refused {
                owner_report,
                diagnostics,
                reason: "candidate stored rule final execution count unavailable",
            });
        }
    };
    let after_cost = match schema_worker.diagnostics_v2_cumulative_cost() {
        Ok(cost) => cost,
        Err(_) => {
            return Err(CandidateRuleDiagnosticsError::Incomplete {
                owner_report,
                diagnostics,
                reason: "candidate stored rule final worker cost incomplete",
            });
        }
    };
    let delta_exchanges = after_cost
        .completed_exchanges()
        .checked_sub(before_cost.completed_exchanges());
    let delta_resource_bytes = after_cost
        .schema_resource_bytes()
        .checked_sub(before_cost.schema_resource_bytes());
    let delta_request_bytes = after_cost
        .request_bytes()
        .checked_sub(before_cost.request_bytes());
    let delta_response_bytes = after_cost
        .response_bytes()
        .checked_sub(before_cost.response_bytes());
    let delta_cpu_micros = after_cost
        .worker_cpu_micros()
        .checked_sub(before_cost.worker_cpu_micros());
    let expected_after = before_count.checked_add(request_count);
    if expected_after != Some(after_count)
        || u64::try_from(request_count).ok() != delta_exchanges
        || delta_resource_bytes.is_none()
        || delta_request_bytes.is_none()
        || delta_response_bytes.is_none()
        || delta_cpu_micros.is_none()
        || (request_count > 0
            && (delta_resource_bytes == Some(0)
                || delta_request_bytes == Some(0)
                || delta_response_bytes == Some(0)))
    {
        return Err(CandidateRuleDiagnosticsError::Incomplete {
            owner_report,
            diagnostics,
            reason: "candidate stored rule cumulative execution accounting invalid",
        });
    }
    let (
        Some(worker_schema_resource_bytes),
        Some(worker_request_bytes),
        Some(worker_response_bytes),
        Some(worker_cpu_micros),
    ) = (
        delta_resource_bytes,
        delta_request_bytes,
        delta_response_bytes,
        delta_cpu_micros,
    )
    else {
        return Err(CandidateRuleDiagnosticsError::Incomplete {
            owner_report,
            diagnostics,
            reason: "candidate stored rule cumulative execution counters regressed",
        });
    };
    let result_state_with_header = result_state.checked_add(vector_state).and_then(|bytes| {
        bytes.checked_add(size_of::<EvaluatedCandidateSourceFoundationRules<I>>())
    });
    if result_state_with_header.is_none_or(|bytes| bytes > limits.max_state_bytes)
        || peak_state > limits.max_state_bytes
    {
        return Err(CandidateRuleDiagnosticsError::Refused {
            owner_report,
            diagnostics,
            reason: "candidate stored rule retained diagnostic state limit",
        });
    }
    Ok(EvaluatedCandidateSourceFoundationRules {
        owner_report,
        diagnostics,
        cost: CandidateSourceFoundationRuleDiagnosticsCost {
            schema_check_count: request_count,
            schema_input_instance_bytes: input_bytes,
            schema_result_state_upper_bound_bytes: result_state,
            typed_diagnostic_vector_state_upper_bound_bytes: vector_state,
            diagnostic_issue_count: issue_count,
            worker_schema_resource_bytes,
            worker_request_bytes,
            worker_response_bytes,
            worker_cpu_micros,
            cumulative_execution_count_before: before_count,
            cumulative_execution_count_after: after_count,
            peak_additional_state_upper_bound_bytes: peak_state,
        },
    })
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

impl SchemaRequestValue for SourceFoundationSchemaCheck {
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
        &self.instance
    }
}

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
