//! Whole-invocation owner join for the maintained native source-foundation
//! command. Each real owner runs in one serial ledger window; reports are
//! carried forward and consumed by the existing finalizer/output assembler.

use super::foundation_artifact_replay::{ArtifactReplayFailure, prepare_artifact_replay};
use super::foundation_bootstrap::{FoundationBootstrapError, FoundationBootstrapView};
use super::foundation_catalog::{
    self, EvaluatedPersistedCatalog, FoundationCatalogOutcome, PersistedCatalogEvaluationError,
};
use super::foundation_execution_limits::FoundationCutWorkerShape;
use super::foundation_execution_limits::{
    FoundationBudgetTicket, FoundationCatalogWorkerShape, FoundationCharge,
    FoundationExecutionLimits, FoundationPhaseReservation, FoundationPhaseUse,
    FoundationRemainingBudget, FoundationWindowKind, FoundationWorkerCpuUse,
};
use super::foundation_output::{self, SourceFoundationOutputOutcome};
use super::foundation_payload::FoundationPayloadSources;
use super::foundation_reader::FoundationRuleReadLimits;
use super::foundation_rule_diagnostics::SourceFoundationRuleDiagnosticsLimits;
use super::foundation_run::{
    self, EvaluatedFoundationDefault, FinalizedFoundationDefaultInputs, FoundationBiblioEvidence,
    FoundationDefaultReadError, FoundationFinalInputError,
};
use crate::source_command::SourceCommandError;
use crate::source_creation_store::DisposableCatalogTreeLimits;
use std::io;
use std::mem::size_of;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tos_foundation::Digest256;
use tos_validation::FormatProfile;
use tos_validation::executor::{
    BatchBudget, ExactWorkerIdentity, ExecutorBudget, MAX_WORKER_IMAGE_BYTES,
    SharedSchemaWorkerQuota, VerifiedWorkerImageHandle,
};
use tos_validation::item_rules::ItemRefusal;
use tos_validation::record_biblio_cut::{BiblioRecordExecutor, BiblioSchemaDiagnosticsLimits};
use tos_validation::source_cut::{
    CutSchemaDiagnosticsLimits, CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor,
    cut_schema_resource_preparation_state_upper_bound,
};
use tos_validation::source_foundation_discovery::SourcePhysicalFacts;
use tos_validation::source_foundation_records::SourceFoundationRecordsReport;
use tos_validation::source_foundation_schema::SourceFoundationSchemaLoadFailure;
use tos_validation::source_foundation_schema::{
    SourceFoundationSchemaLimits, SourceFoundationSchemaSet,
};

pub(crate) enum FoundationOrchestratorError {
    Bootstrap(FoundationBootstrapError),
    Command(SourceCommandError),
    Owner(ItemRefusal),
    Default(FoundationDefaultReadError),
    Final(FoundationFinalInputError),
    Replay(ArtifactReplayFailure),
    Catalog(tos_compiler::Error),
    Persisted(PersistedCatalogEvaluationError),
    Admission(io::Error),
    Incomplete(&'static str),
}

impl From<FoundationBootstrapError> for FoundationOrchestratorError {
    fn from(error: FoundationBootstrapError) -> Self {
        Self::Bootstrap(error)
    }
}

impl FoundationOrchestratorError {
    pub(crate) fn public_reason(&self) -> &'static str {
        match self {
            Self::Bootstrap(error) => match error {
                FoundationBootstrapError::Command(_) => {
                    "source-foundation bootstrap invocation refused"
                }
                FoundationBootstrapError::StageSelection(_) => {
                    "source-foundation bootstrap stage selection refused"
                }
                FoundationBootstrapError::IsolatedRoot(_) => {
                    "source-foundation bootstrap isolated root refused"
                }
                FoundationBootstrapError::RouteRoot(_) => {
                    "source-foundation bootstrap route root refused"
                }
                FoundationBootstrapError::Capture(_) => {
                    "source-foundation bootstrap capture refused"
                }
                FoundationBootstrapError::Selection(_) => {
                    "source-foundation bootstrap physical selection refused"
                }
                FoundationBootstrapError::Payload(_) => {
                    "source-foundation bootstrap payload snapshot refused"
                }
                FoundationBootstrapError::Physical(_) => {
                    "source-foundation bootstrap physical snapshot refused"
                }
                FoundationBootstrapError::Configuration(_) => {
                    "source-foundation bootstrap configuration refused"
                }
            },
            Self::Command(_) => "source-foundation invocation refused",
            Self::Owner(_) => "source-foundation owner phase refused",
            Self::Default(_) => "source-foundation default rules refused",
            Self::Final(_) => "source-foundation final custody refused",
            Self::Replay(_) => "source-foundation artifact replay refused",
            Self::Catalog(_) => "source-foundation catalog comparison refused",
            Self::Persisted(_) => "source-foundation persisted catalog refused",
            Self::Admission(_) => "source-foundation candidate index refused",
            Self::Incomplete(reason) => reason,
        }
    }
}

enum JoinedOutcome {
    Default(SourceFoundationOutputOutcome),
    Admission(crate::source_admission_index::Index),
}

type AdmissionReceiver<'receive> = dyn FnOnce(
        crate::source_admission_index::FreshRows,
        &mut crate::source_admission_index::SchemaCheck<'_>,
        FoundationPhaseReservation,
    ) -> io::Result<(crate::source_admission_index::Index, FoundationPhaseUse)>
    + 'receive;

pub(crate) fn evaluate<'work, 'cancel, 'signal>(
    view: FoundationBootstrapView<'work, 'cancel, 'signal>,
    payloads: FoundationPayloadSources<'work>,
) -> Result<SourceFoundationOutputOutcome, FoundationOrchestratorError> {
    match run(view, payloads, None)? {
        JoinedOutcome::Default(outcome) => Ok(outcome),
        JoinedOutcome::Admission(_) => Err(FoundationOrchestratorError::Incomplete(
            "ordinary foundation evaluation returned admission state",
        )),
    }
}

pub(crate) fn evaluate_admission<'work, 'receive, 'cancel, 'signal>(
    view: FoundationBootstrapView<'work, 'cancel, 'signal>,
    payloads: FoundationPayloadSources<'work>,
    receive: impl FnOnce(
        crate::source_admission_index::FreshRows,
        &mut crate::source_admission_index::SchemaCheck<'_>,
        FoundationPhaseReservation,
    ) -> io::Result<(crate::source_admission_index::Index, FoundationPhaseUse)>
    + 'receive,
) -> Result<crate::source_admission_index::Index, FoundationOrchestratorError> {
    match run(view, payloads, Some(Box::new(receive)))? {
        JoinedOutcome::Admission(index) => Ok(index),
        JoinedOutcome::Default(_) => Err(FoundationOrchestratorError::Incomplete(
            "candidate foundation did not produce a complete fresh catalog",
        )),
    }
}

fn failure_use(ticket: &FoundationBudgetTicket) -> FoundationPhaseUse {
    let remaining = ticket.remaining();
    FoundationPhaseUse {
        source_read_bytes: FoundationCharge::admitted_upper_bound(remaining.source_read_bytes),
        worker_wire_bytes: FoundationCharge::admitted_upper_bound(remaining.worker_wire_bytes),
        state_bytes: FoundationCharge::admitted_upper_bound(remaining.state_bytes),
        issue_count: FoundationCharge::admitted_upper_bound(remaining.issue_count),
        output_bytes: FoundationCharge::admitted_upper_bound(remaining.output_bytes),
        worker_cpu: FoundationWorkerCpuUse::AdmittedMicros(ticket.remaining_worker_cpu_micros()),
        tmpfs_bytes: FoundationCharge::admitted_upper_bound(remaining.tmpfs_bytes),
        tmpfs_inodes: FoundationCharge::admitted_upper_bound(remaining.tmpfs_inodes),
    }
}

fn open_window(
    execution_limits: &mut FoundationExecutionLimits,
    remaining_budget: &mut FoundationRemainingBudget<'_>,
    label: &'static str,
    kind: FoundationWindowKind,
    baseline: FoundationPhaseReservation,
) -> Result<FoundationBudgetTicket, FoundationOrchestratorError> {
    let ticket = remaining_budget
        .begin_window(label, baseline)
        .map_err(FoundationOrchestratorError::Command)?;
    if execution_limits.select_window(&ticket, kind).is_err() {
        let worst = failure_use(&ticket);
        let _ = remaining_budget.fail_window(ticket, worst);
        return Err(FoundationOrchestratorError::Incomplete(
            "source-foundation operation window refused",
        ));
    }
    Ok(ticket)
}

fn complete_window(
    execution_limits: &mut FoundationExecutionLimits,
    remaining_budget: &mut FoundationRemainingBudget<'_>,
    ticket: FoundationBudgetTicket,
    usage: FoundationPhaseUse,
) -> Result<(), FoundationOrchestratorError> {
    execution_limits
        .clear_window(&ticket)
        .map_err(FoundationOrchestratorError::Command)?;
    remaining_budget
        .complete_window(ticket, usage)
        .map_err(FoundationOrchestratorError::Command)
}

fn fail_window<T>(
    execution_limits: &mut FoundationExecutionLimits,
    remaining_budget: &mut FoundationRemainingBudget<'_>,
    ticket: FoundationBudgetTicket,
    error: FoundationOrchestratorError,
) -> Result<T, FoundationOrchestratorError> {
    let worst = failure_use(&ticket);
    let _ = execution_limits.clear_window(&ticket);
    let _ = remaining_budget.fail_window(ticket, worst);
    Err(error)
}

fn use_delta(
    before: tos_validation::executor::SharedSchemaWorkerQuotaUsage,
    after: tos_validation::executor::SharedSchemaWorkerQuotaUsage,
) -> Result<(u64, u64), FoundationOrchestratorError> {
    let cpu = after
        .worker_cpu_micros
        .checked_sub(before.worker_cpu_micros)
        .ok_or(FoundationOrchestratorError::Incomplete(
            "shared schema CPU counter regressed",
        ))?;
    let wire = after
        .worker_wire_bytes
        .checked_sub(before.worker_wire_bytes)
        .ok_or(FoundationOrchestratorError::Incomplete(
            "shared schema wire counter regressed",
        ))?;
    Ok((cpu, wire))
}

fn phase_use(
    read: u64,
    wire: u64,
    state: usize,
    issues: usize,
    cpu_micros: u64,
    tmpfs: u64,
    inodes: u64,
) -> FoundationPhaseUse {
    FoundationPhaseUse {
        source_read_bytes: FoundationCharge::measured(read),
        worker_wire_bytes: FoundationCharge::measured(wire),
        state_bytes: FoundationCharge::admitted_upper_bound(state),
        issue_count: FoundationCharge::measured(issues),
        output_bytes: FoundationCharge::measured(0),
        worker_cpu: FoundationWorkerCpuUse::MeasuredMicros(cpu_micros),
        tmpfs_bytes: FoundationCharge::admitted_upper_bound(tmpfs),
        tmpfs_inodes: FoundationCharge::admitted_upper_bound(inodes),
    }
}

fn bounded_usize(value: u64) -> Result<usize, FoundationOrchestratorError> {
    usize::try_from(value).map_err(|_| {
        FoundationOrchestratorError::Incomplete("foundation limit exceeds address space")
    })
}

fn owner(error: ItemRefusal) -> FoundationOrchestratorError {
    FoundationOrchestratorError::Owner(error)
}

fn phase_amounts(
    usage: FoundationPhaseUse,
) -> Result<FoundationPhaseReservation, FoundationOrchestratorError> {
    let cpu = match usage.worker_cpu {
        FoundationWorkerCpuUse::MeasuredMicros(micros)
        | FoundationWorkerCpuUse::AdmittedMicros(micros) => micros,
        FoundationWorkerCpuUse::AdmittedSeconds(seconds) => seconds
            .checked_mul(1_000_000)
            .ok_or_else(|| incomplete("phase CPU accounting overflow"))?,
    };
    Ok(FoundationPhaseReservation {
        source_read_bytes: usage.source_read_bytes.amount,
        worker_wire_bytes: usage.worker_wire_bytes.amount,
        state_bytes: usage.state_bytes.amount,
        issue_count: usage.issue_count.amount,
        output_bytes: usage.output_bytes.amount,
        worker_cpu_seconds: 0,
        worker_cpu_micros: cpu,
        tmpfs_bytes: usage.tmpfs_bytes.amount,
        tmpfs_inodes: usage.tmpfs_inodes.amount,
    })
}

fn reservation_minus(
    available: FoundationPhaseReservation,
    used: FoundationPhaseReservation,
) -> Result<FoundationPhaseReservation, FoundationOrchestratorError> {
    let cpu = available
        .worker_cpu_micros
        .checked_sub(used.worker_cpu_micros)
        .ok_or_else(|| incomplete("phase CPU allowance underflow"))?;
    let seconds = cpu
        .checked_add(999_999)
        .ok_or_else(|| incomplete("phase CPU allowance overflow"))?
        / 1_000_000;
    Ok(FoundationPhaseReservation {
        source_read_bytes: available
            .source_read_bytes
            .checked_sub(used.source_read_bytes)
            .ok_or_else(|| incomplete("phase source-read allowance underflow"))?,
        worker_wire_bytes: available
            .worker_wire_bytes
            .checked_sub(used.worker_wire_bytes)
            .ok_or_else(|| incomplete("phase worker-wire allowance underflow"))?,
        state_bytes: available
            .state_bytes
            .checked_sub(used.state_bytes)
            .ok_or_else(|| incomplete("phase state allowance underflow"))?,
        issue_count: available
            .issue_count
            .checked_sub(used.issue_count)
            .ok_or_else(|| incomplete("phase issue allowance underflow"))?,
        output_bytes: available
            .output_bytes
            .checked_sub(used.output_bytes)
            .ok_or_else(|| incomplete("phase output allowance underflow"))?,
        worker_cpu_seconds: seconds,
        worker_cpu_micros: cpu,
        tmpfs_bytes: available
            .tmpfs_bytes
            .checked_sub(used.tmpfs_bytes)
            .ok_or_else(|| incomplete("phase tmpfs allowance underflow"))?,
        tmpfs_inodes: available
            .tmpfs_inodes
            .checked_sub(used.tmpfs_inodes)
            .ok_or_else(|| incomplete("phase inode allowance underflow"))?,
    })
}

fn issue_bytes(issues: &[(String, String)]) -> Result<usize, FoundationOrchestratorError> {
    issues.iter().try_fold(0usize, |total, (path, message)| {
        total
            .checked_add(path.len())
            .and_then(|bytes| bytes.checked_add(message.len()))
            .ok_or_else(|| incomplete("foundation issue byte count overflow"))
    })
}

fn incomplete(reason: &'static str) -> FoundationOrchestratorError {
    FoundationOrchestratorError::Incomplete(reason)
}

fn schema_load(_: SourceFoundationSchemaLoadFailure) -> FoundationOrchestratorError {
    incomplete("source-foundation selected schema set refused")
}

fn checked_add_usize(left: usize, right: usize) -> Result<usize, FoundationOrchestratorError> {
    left.checked_add(right)
        .ok_or_else(|| incomplete("source-foundation cost overflow"))
}

/// Worker adapters retain a cloned PathBuf identity alongside duplicated
/// FDs for the same sealed image. Count only the per-clone path allocation;
/// inline identity headers are already covered by the adapter/report bounds.
fn worker_identity_path_clone_bytes(
    worker: &ExactWorkerIdentity,
    copies: usize,
) -> Result<usize, FoundationOrchestratorError> {
    let path_capacity = worker.absolute_path.capacity();
    if path_capacity > 4096 {
        return Err(incomplete("worker identity path exceeds protected bound"));
    }
    path_capacity
        .checked_mul(copies)
        .ok_or_else(|| incomplete("worker identity path-copy accounting overflow"))
}

fn checked_add_u64(left: u64, right: u64) -> Result<u64, FoundationOrchestratorError> {
    left.checked_add(right)
        .ok_or_else(|| incomplete("source-foundation cost overflow"))
}

fn ticket_worker_budget(
    ticket: &FoundationBudgetTicket,
    deadline: Instant,
    address_space_bytes: u64,
) -> Result<ExecutorBudget, FoundationOrchestratorError> {
    let operation = ticket.operation_limits();
    let wall = deadline.saturating_duration_since(Instant::now());
    let cpu_seconds = operation
        .worker_cpu_seconds
        .min(ExecutorBudget::MAX_SCALAR_CPU_SECONDS);
    if wall.is_zero() || cpu_seconds == 0 || address_space_bytes < 64 * 1024 * 1024 {
        return Err(incomplete("source-foundation worker reservation exhausted"));
    }
    Ok(ExecutorBudget {
        execution_wall: wall.min(Duration::from_secs(3_600)),
        cleanup_grace: Duration::from_millis(200).min(wall),
        cpu_seconds,
        address_space_bytes,
    })
}

fn schema_limits_for_ticket(
    limits: &FoundationExecutionLimits,
    ticket: &FoundationBudgetTicket,
    selected_resources: usize,
    max_resource_bytes: usize,
    total_resource_bytes: usize,
    max_checks: usize,
) -> Result<SourceFoundationSchemaLimits, FoundationOrchestratorError> {
    let operation = ticket.operation_limits();
    let instance_cap = bounded_usize(operation.source_read_bytes)?
        .min(bounded_usize(limits.invocation_budgets.max_member_bytes)?)
        .min(tos_validation::executor::BatchBudget::MAX_RAW_BYTES);
    let total_instance = bounded_usize(operation.state_bytes as u64)?
        .min(bounded_usize(operation.source_read_bytes)?)
        .min(128 * 1024 * 1024);
    let batch = BatchBudget::laboratory();
    let chunks = max_checks
        .checked_add(batch.max_units - 1)
        .ok_or_else(|| incomplete("schema chunk limit overflow"))?
        / batch.max_units;
    limits
        .schema_limits(
            operation,
            operation,
            selected_resources,
            max_resource_bytes,
            total_resource_bytes,
            max_checks,
            chunks.max(1),
            instance_cap,
            total_instance,
            operation.issue_count,
            operation
                .output_bytes
                .min(tos_validation::executor::schema_diagnostics::MAX_RESPONSE_BYTES),
            batch,
        )
        .map_err(|_| incomplete("source-foundation schema reservation refused"))
}

fn cut_worker_shape(
    operation: FoundationPhaseReservation,
    max_checks: usize,
) -> FoundationCutWorkerShape {
    let batch = BatchBudget::laboratory();
    let units = u64::try_from(max_checks.max(1)).unwrap_or(u64::MAX - 1);
    let units = units.min(operation.worker_wire_bytes).min(u64::MAX - 1);
    FoundationCutWorkerShape {
        batch,
        max_chunks: units
            .saturating_add(batch.max_units as u64 - 1)
            .checked_div(batch.max_units as u64)
            .unwrap_or(1)
            .max(1),
        max_total_units: units.max(1),
        max_total_raw_bytes: operation
            .source_read_bytes
            .min(operation.worker_wire_bytes)
            .min(tos_validation::executor::BatchBudget::MAX_RAW_BYTES as u64)
            .max(1),
        aggregate_wire_upper_bound_bytes: operation.worker_wire_bytes,
        max_distinct_selectors: max_checks.max(1).min(1024),
    }
}

fn schema_state_upper_bound(
    bytes: usize,
    resources: usize,
) -> Result<usize, FoundationOrchestratorError> {
    bytes
        .checked_mul(8)
        .and_then(|state| {
            resources
                .checked_mul(512)
                .and_then(|rows| state.checked_add(rows))
        })
        .and_then(|state| state.checked_add(std::mem::size_of::<SourceFoundationSchemaSet>()))
        .ok_or_else(|| incomplete("source-foundation schema state overflow"))
}

fn records_read_bytes(
    records: &SourceFoundationRecordsReport,
    _schema_bytes: usize,
) -> Result<u64, FoundationOrchestratorError> {
    let cost = &records.cost;
    let record = cost
        .record_observed_read_bytes
        .ok_or_else(|| incomplete("source-foundation record read accounting incomplete"))?;
    checked_add_u64(record, cost.item_observed_read_bytes)
}

fn default_issue_count(evaluated: &EvaluatedFoundationDefault<'_>) -> usize {
    evaluated.rules.cost.issue_count
}

fn catalog_issue_count(
    outcome: &FoundationCatalogOutcome,
) -> Result<usize, FoundationOrchestratorError> {
    match outcome {
        FoundationCatalogOutcome::Complete(result) => Ok(result.issues.len()),
        FoundationCatalogOutcome::SchemaRejected {
            catalog_issues,
            diagnostic,
            ..
        } => checked_add_usize(catalog_issues.len(), diagnostic.report().issues.len()),
    }
}

fn catalog_read_bytes(
    outcome: &FoundationCatalogOutcome,
) -> Result<u64, FoundationOrchestratorError> {
    let (work, generated, recheck) = match outcome {
        FoundationCatalogOutcome::Complete(result) => (
            result.observed_plan_work_bytes,
            result.generated_read_bytes,
            result.source_recheck_read_bytes,
        ),
        FoundationCatalogOutcome::SchemaRejected {
            observed_plan_work_bytes,
            generated_read_bytes,
            source_recheck_read_bytes,
            ..
        } => (
            *observed_plan_work_bytes,
            *generated_read_bytes,
            *source_recheck_read_bytes,
        ),
    };
    checked_add_u64(
        work,
        checked_add_u64(
            u64::try_from(generated).map_err(|_| incomplete("catalog read cost range"))?,
            u64::try_from(recheck).map_err(|_| incomplete("catalog recheck cost range"))?,
        )?,
    )
}

fn catalog_retained_state(
    outcome: &FoundationCatalogOutcome,
) -> Result<usize, FoundationOrchestratorError> {
    match outcome {
        FoundationCatalogOutcome::Complete(result) => checked_add_usize(
            result.profiles.retained_state_bytes,
            result.generated_inputs.retained_state_bytes(),
        ),
        FoundationCatalogOutcome::SchemaRejected {
            profiles,
            generated_inputs,
            ..
        } => checked_add_usize(
            profiles.retained_state_bytes,
            generated_inputs
                .as_ref()
                .map_or(0, |inputs| inputs.retained_state_bytes()),
        ),
    }
}

fn outcome_schema_execution_cost(
    outcome: &FoundationCatalogOutcome,
) -> tos_validation::source_cut::CutSchemaDiagnosticsCumulativeCost {
    match outcome {
        FoundationCatalogOutcome::Complete(result) => result.schema_execution_cost,
        FoundationCatalogOutcome::SchemaRejected {
            schema_execution_cost,
            ..
        } => *schema_execution_cost,
    }
}

fn catalog_profiles(
    outcome: &FoundationCatalogOutcome,
) -> Result<&[(String, String)], FoundationOrchestratorError> {
    let profiles = match outcome {
        FoundationCatalogOutcome::Complete(result) => &result.profiles,
        FoundationCatalogOutcome::SchemaRejected { profiles, .. } => profiles,
    };
    profiles
        .files()
        .map_err(|_| incomplete("catalog record-profile selection incomplete"))
}

fn run<'work, 'receive, 'cancel, 'signal>(
    view: FoundationBootstrapView<'work, 'cancel, 'signal>,
    mut payloads: FoundationPayloadSources<'work>,
    mut receive: Option<Box<AdmissionReceiver<'receive>>>,
) -> Result<JoinedOutcome, FoundationOrchestratorError> {
    let FoundationBootstrapView {
        clock: _,
        launch,
        invocation,
        execution_limits,
        remaining_budget,
        selected_roots,
        stage,
        isolated,
        sources,
        artifact_sources,
        captured,
        selection,
        physical,
        cancelled,
        cost,
        mut history,
    } = view;
    let deadline = invocation.deadline();
    remaining_budget
        .verify_invocation(invocation)
        .map_err(FoundationOrchestratorError::Command)?;
    if execution_limits.deadline != deadline
        || payloads.deadline() > deadline
        || physical.deadline() > deadline
        || sources.deadline() > deadline
        || artifact_sources
            .as_deref()
            .is_some_and(|root| root.deadline() > deadline)
    {
        return Err(incomplete(
            "source-foundation provider deadline exceeds invocation",
        ));
    }

    let budgets = invocation.budgets;
    let max_members = usize::try_from(budgets.max_current_members.min(usize::MAX as u64))
        .map_err(|_| incomplete("current-member limit range"))?;
    let mut schema_count = 0usize;
    let mut schema_total = 0usize;
    let mut schema_max = 0usize;
    for member in captured.cut().current().members() {
        if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
            return Err(incomplete("source-foundation schema census interrupted"));
        }
        let path = member.path.as_str();
        if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
            schema_count = schema_count
                .checked_add(1)
                .ok_or_else(|| incomplete("schema resource count overflow"))?;
            let bytes = usize::try_from(member.size_bytes)
                .map_err(|_| incomplete("schema resource size range"))?;
            schema_total = checked_add_usize(schema_total, bytes)?;
            schema_max = schema_max.max(bytes);
        }
    }
    let max_checks = max_members.min(65_536).max(1);
    let quota_caps = remaining_budget
        .remaining()
        .map_err(FoundationOrchestratorError::Command)?;
    let worker_quota = SharedSchemaWorkerQuota::new(
        remaining_budget
            .remaining_worker_cpu_micros()
            .map_err(FoundationOrchestratorError::Command)?,
        quota_caps.worker_wire_bytes,
        quota_caps.worker_wire_bytes.max(1),
    )
    .map_err(|_| incomplete("shared schema worker quota refused"))?;

    // Load the exact selected schema closure from the authenticated cut before
    // constructing Record or default-rule requests.
    let schema_ticket = open_window(
        execution_limits,
        remaining_budget,
        "schema-resource-load",
        FoundationWindowKind::Schema,
        FoundationPhaseReservation::default(),
    )?;
    let schema_limits = schema_limits_for_ticket(
        execution_limits,
        &schema_ticket,
        schema_count,
        schema_max,
        schema_total,
        max_checks,
    )?;
    let schema_preparation_state = match cut_schema_resource_preparation_state_upper_bound(
        captured.cut(),
        deadline,
        cancelled,
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                schema_ticket,
                owner(error),
            );
        }
    };
    if schema_preparation_state > schema_ticket.operation_limits().state_bytes {
        return fail_window(
            execution_limits,
            remaining_budget,
            schema_ticket,
            incomplete("schema closure preparation exceeds remaining state"),
        );
    }
    let schema_set = match SourceFoundationSchemaSet::from_cut(
        captured.cut(),
        FormatProfile::LegacyPythonObserved20260923,
        schema_limits,
        deadline,
        cancelled,
    ) {
        Ok(set) => set,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                schema_ticket,
                schema_load(error),
            );
        }
    };
    let loaded_schema_bytes = schema_set.schema_bytes();
    let schema_state =
        schema_state_upper_bound(loaded_schema_bytes, schema_set.schema_resource_count())?;
    if schema_state > schema_ticket.operation_limits().state_bytes {
        return fail_window(
            execution_limits,
            remaining_budget,
            schema_ticket,
            incomplete("selected schema closure exceeds retained-state budget"),
        );
    }
    complete_window(
        execution_limits,
        remaining_budget,
        schema_ticket,
        phase_use(
            // The bytes were read into the authenticated capture already.
            0,
            0,
            schema_state,
            0,
            0,
            0,
            0,
        ),
    )?;

    let current_ticket = open_window(
        execution_limits,
        remaining_budget,
        "records-and-bibliography",
        FoundationWindowKind::RecordsAndBibliography,
        FoundationPhaseReservation::default(),
    )?;
    let initial_operation = current_ticket.operation_limits();
    let current_paths = match foundation_run::captured_current_paths(
        captured,
        max_members,
        initial_operation.state_bytes,
        deadline,
        cancelled,
    ) {
        Ok(paths) => paths,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                FoundationOrchestratorError::Default(error),
            );
        }
    };
    let current_paths_state = current_paths.retained_state_upper_bound_bytes;
    if current_paths_state > initial_operation.state_bytes {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            incomplete("current-path snapshot exceeds budget"),
        );
    }
    let executor_budget = match ticket_worker_budget(
        &current_ticket,
        deadline,
        budgets.worker_address_space_bytes,
    ) {
        Ok(budget) => budget,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, current_ticket, error);
        }
    };
    // Reserve the donor's fixed sealed-image maximum before opening the
    // executable. The stack copy buffer and handle identity are included in
    // the simultaneous preparation-state preflight; successful completion
    // later charges the exact image bytes and retained handle upper bound.
    let image_workspace = 64 * 1024usize;
    let image_path_bytes = execution_limits.worker.absolute_path.capacity();
    let image_prepare_path_copy_state =
        match worker_identity_path_clone_bytes(&execution_limits.worker, 1) {
            Ok(bytes) => bytes,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, current_ticket, error);
            }
        };
    let image_handle_metadata =
        match size_of::<VerifiedWorkerImageHandle>().checked_add(image_path_bytes) {
            Some(bytes) => bytes,
            None => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    current_ticket,
                    incomplete("worker image handle-state overflow"),
                );
            }
        };
    // Record and Item each retain both a worker identity and a prepared
    // schema image identity. Their FDs refer to the shared memfd; only these
    // bounded path buffers are additional source-derived state.
    let records_identity_copy_state =
        match worker_identity_path_clone_bytes(&execution_limits.worker, 4) {
            Ok(bytes) => bytes,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, current_ticket, error);
            }
        };
    let image_peak_state = match usize::try_from(MAX_WORKER_IMAGE_BYTES)
        .ok()
        .and_then(|bytes| bytes.checked_add(image_workspace))
        .and_then(|bytes| bytes.checked_add(image_handle_metadata))
        .and_then(|bytes| bytes.checked_add(image_prepare_path_copy_state))
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("worker image preparation-state overflow"),
            );
        }
    };
    let image_operation = current_ticket.operation_limits();
    let image_peak_with_paths = match image_peak_state
        .checked_add(current_paths_state)
        .and_then(|bytes| bytes.checked_add(records_identity_copy_state))
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("worker image and current paths state overflow"),
            );
        }
    };
    if image_operation.source_read_bytes < MAX_WORKER_IMAGE_BYTES
        || image_operation.state_bytes < image_peak_with_paths
        || Instant::now() >= deadline
        || cancelled.load(Ordering::Acquire)
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            incomplete("verified worker image exceeds remaining source/state budget"),
        );
    }
    let worker_image = match VerifiedWorkerImageHandle::prepare(
        execution_limits.worker.clone(),
        executor_budget,
        deadline,
        cancelled,
    ) {
        Ok(image) => image,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("verified worker image preparation refused"),
            );
        }
    };
    let worker_image_read = match worker_image.image_bytes() {
        Ok(bytes) => bytes,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("verified worker image size unavailable"),
            );
        }
    };
    let worker_image_state = match worker_image.retained_state_bytes() {
        Ok(bytes) => match usize::try_from(bytes) {
            Ok(bytes) => bytes,
            Err(_) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    current_ticket,
                    incomplete("verified worker image retained-state range"),
                );
            }
        },
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("verified worker image retained-state unavailable"),
            );
        }
    };
    let current_live_use = match checked_add_usize(worker_image_state, current_paths_state)
        .and_then(|bytes| checked_add_usize(bytes, records_identity_copy_state))
    {
        Ok(state) => phase_amounts(phase_use(worker_image_read, 0, state, 0, 0, 0, 0)),
        Err(error) => Err(error),
    };
    let current_live_use = match current_live_use {
        Ok(usage) => usage,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, current_ticket, error);
        }
    };
    let operation = match reservation_minus(image_operation, current_live_use) {
        Ok(operation) => operation,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, current_ticket, error);
        }
    };
    let payload_state_ceiling = match payloads
        .cost()
        .peak_state_bytes
        .checked_add(operation.state_bytes)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("payload state allowance overflow"),
            );
        }
    };
    if let Err(error) = payloads.restrict_remaining_budget(
        operation.source_read_bytes,
        payload_state_ceiling,
        deadline,
        cancelled,
    ) {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            owner(error),
        );
    }
    let physical_state_ceiling = match physical
        .cost()
        .retained_state_bytes
        .checked_add(operation.state_bytes)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("physical state allowance overflow"),
            );
        }
    };
    if let Err(error) = physical.restrict_remaining_budget(
        usize::try_from(operation.source_read_bytes).unwrap_or(usize::MAX),
        physical_state_ceiling,
        deadline,
        cancelled,
    ) {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            owner(error),
        );
    }
    let schema_request_state = operation.state_bytes.max(1);
    let records_limits =
        match execution_limits.rolling_records_limits(operation, schema_request_state) {
            Ok(limits) => limits,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    current_ticket,
                    FoundationOrchestratorError::Command(error),
                );
            }
        };
    let worker_shape = cut_worker_shape(operation, max_checks);
    let record_stream = match execution_limits.cut_worker_stream_budget(operation, worker_shape) {
        Ok(stream) => stream,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                FoundationOrchestratorError::Command(error),
            );
        }
    };
    let worker_limits = CutWorkerLimits {
        max_receipts: max_checks,
        max_receipt_bytes: operation.state_bytes.min(1024 * 1024),
    };
    let mut record_executor = match BiblioRecordExecutor::new_with_image(
        &worker_image,
        executor_budget,
        FormatProfile::LegacyPythonObserved20260923,
        max_checks,
        deadline,
        cancelled,
    ) {
        Ok(executor) => executor,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                owner(error),
            );
        }
    };
    if record_executor.set_operation_budget(record_stream).is_err()
        || BiblioSchemaDiagnosticsLimits::from_operation_ceilings(
            BiblioSchemaDiagnosticsLimits {
                max_total_issues: operation.issue_count,
                max_total_report_bytes: operation.output_bytes,
                max_total_state_bytes: operation.state_bytes,
            },
            max_checks,
            record_stream,
        )
        .and_then(|limits| record_executor.enable_diagnostics_v2(limits))
        .is_err()
        || record_executor
            .set_shared_schema_worker_quota(worker_quota.clone())
            .is_err()
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            incomplete("record schema worker limits refused"),
        );
    }
    // The prepared closure's temporary DOMs are admitted before constructor allocation.
    // This workspace expires at constructor return; retained controller state
    // continues through its existing phase/H accounting, without a refund.
    if schema_preparation_state > operation.state_bytes {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            incomplete("Item schema preparation exceeds remaining state"),
        );
    }
    let mut item_schemas = match CutWorkerSchemaExecutor::from_cut_with_image(
        captured.cut(),
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        executor_budget,
        worker_limits,
        deadline,
        cancelled,
    ) {
        Ok(executor) => executor,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                owner(error),
            );
        }
    };
    let item_diag = CutSchemaDiagnosticsLimits {
        max_total_issues: operation.issue_count,
        max_total_report_bytes: operation.output_bytes,
        max_total_state_bytes: operation.state_bytes,
    };
    if item_schemas.set_operation_budget(record_stream).is_err()
        || CutSchemaDiagnosticsLimits::from_operation_ceilings(
            item_diag,
            worker_limits.max_receipts,
            record_stream,
        )
        .and_then(|limits| item_schemas.enable_diagnostics_v2(limits))
        .is_err()
        || item_schemas
            .set_shared_schema_worker_quota(worker_quota.clone())
            .is_err()
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            incomplete("Item schema worker limits refused"),
        );
    }
    let records = match foundation_run::prepare_records(
        captured,
        &mut payloads,
        &mut record_executor,
        &mut item_schemas,
        physical.facts(),
        launch.arguments.require_local_payloads,
        records_limits,
        cancelled,
    ) {
        Ok(report) => report,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                FoundationOrchestratorError::Default(error),
            );
        }
    };
    let records_source_bytes = match records_read_bytes(&records, loaded_schema_bytes) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, current_ticket, error);
        }
    };
    let Some(records_state) = records.cost.rolling_accounted_state_upper_bound_bytes else {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            incomplete("rolling Records state accounting unavailable"),
        );
    };
    let Some(records_issues) = records.cost.rolling_observed_issue_count else {
        return fail_window(
            execution_limits,
            remaining_budget,
            current_ticket,
            incomplete("rolling Records issue accounting unavailable"),
        );
    };
    let schema_usage_before = match worker_quota.usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                current_ticket,
                incomplete("record schema quota usage unavailable"),
            );
        }
    };
    let worker_delta = match use_delta(
        tos_validation::executor::SharedSchemaWorkerQuotaUsage {
            max_total_cpu_micros: schema_usage_before.max_total_cpu_micros,
            max_total_wire_bytes: schema_usage_before.max_total_wire_bytes,
            max_total_units: schema_usage_before.max_total_units,
            worker_cpu_micros: 0,
            worker_wire_bytes: 0,
            worker_units: 0,
        },
        schema_usage_before,
    ) {
        Ok(_) => (
            schema_usage_before.worker_cpu_micros,
            schema_usage_before.worker_wire_bytes,
        ),
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, current_ticket, error);
        }
    };
    let records_read = match checked_add_u64(records_source_bytes, worker_image_read) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, current_ticket, error);
        }
    };
    let records_retained = match checked_add_usize(records_state, current_paths_state)
        .and_then(|bytes| checked_add_usize(bytes, worker_image_state))
        .and_then(|bytes| checked_add_usize(bytes, records_identity_copy_state))
    {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, current_ticket, error);
        }
    };
    let records_usage = phase_use(
        records_read,
        worker_delta.1,
        records_retained,
        records_issues,
        worker_delta.0,
        0,
        0,
    );
    complete_window(
        execution_limits,
        remaining_budget,
        current_ticket,
        records_usage,
    )?;

    // Retained Artifact replay is conditional on an actual complete Records
    // family and keeps all selected root/history objects alive for default.
    let artifact_ticket = open_window(
        execution_limits,
        remaining_budget,
        "artifact-history-replay",
        FoundationWindowKind::ArtifactHistoryReplay,
        FoundationPhaseReservation::default(),
    )?;
    let artifact_operation = artifact_ticket.operation_limits();
    let artifact_item_limits = execution_limits
        .item_limits(artifact_operation)
        .map_err(FoundationOrchestratorError::Command)?;
    let readonly = invocation
        .budgets
        .readonly_record_limits(
            remaining_budget.charged().source_read_bytes,
            remaining_budget.charged().state_bytes,
        )
        .map_err(FoundationOrchestratorError::Command)?;
    let artifact_root = selected_roots.repo_root.join("ToS/source-witnesses");
    let artifact_before = worker_quota
        .usage()
        .map_err(|_| incomplete("schema quota unavailable"))?;
    let replay = match prepare_artifact_replay(
        captured,
        &current_paths.paths,
        &records,
        physical.facts(),
        &artifact_root,
        u64::from(invocation.uid()),
        &mut item_schemas,
        artifact_item_limits,
        readonly,
        cancelled,
    ) {
        Ok(replay) => replay,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                artifact_ticket,
                incomplete("artifact history or correction replay refused"),
            );
        }
    };
    let replay_cost = replay.cost();
    let replay_read = replay_cost
        .measured_source_read_bytes()
        .ok_or_else(|| incomplete("artifact replay read cost overflow"))?;
    let replay_state = replay_cost
        .retained_state_upper_bound_bytes()
        .ok_or_else(|| incomplete("artifact replay state cost overflow"))?;
    let artifact_after = worker_quota
        .usage()
        .map_err(|_| incomplete("schema quota unavailable"))?;
    let (artifact_cpu, artifact_wire) = use_delta(artifact_before, artifact_after)?;
    complete_window(
        execution_limits,
        remaining_budget,
        artifact_ticket,
        phase_use(
            replay_read,
            artifact_wire,
            replay_state,
            0,
            artifact_cpu,
            0,
            0,
        ),
    )?;

    let record_baseline = phase_amounts(records_usage)?;
    let default_ticket = open_window(
        execution_limits,
        remaining_budget,
        "default-rules-and-diagnostics",
        FoundationWindowKind::DefaultRules,
        record_baseline,
    )?;
    // Two retained identities belong to the layer Cut/native adapters; the
    // shared diagnostics bridge creates one temporary native identity at a
    // time while advancing its bounded chunks.
    let default_identity_copy_state =
        match worker_identity_path_clone_bytes(worker_image.identity(), 3) {
            Ok(bytes) => bytes,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, default_ticket, error);
            }
        };
    let mut default_operation = default_ticket.operation_limits();
    default_operation.state_bytes = match default_operation
        .state_bytes
        .checked_sub(default_identity_copy_state)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                default_ticket,
                incomplete("default worker identity state exceeds remaining budget"),
            );
        }
    };
    let default_free = match reservation_minus(
        default_ticket.remaining(),
        FoundationPhaseReservation {
            state_bytes: default_identity_copy_state,
            ..FoundationPhaseReservation::default()
        },
    ) {
        Ok(free) => free,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, default_ticket, error);
        }
    };
    let rule_aux_entry_bytes = size_of::<(String, Option<(Digest256, u64)>)>().saturating_add(64);
    let max_auxiliary_paths = if rule_aux_entry_bytes == 0 {
        0
    } else {
        max_members.min(default_operation.state_bytes / rule_aux_entry_bytes)
    };
    let reader_limits =
        match execution_limits.rule_read_limits(default_operation, max_auxiliary_paths) {
            Ok(limits) => limits,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    default_ticket,
                    FoundationOrchestratorError::Command(error),
                );
            }
        };
    let event_map_cap = default_operation
        .state_bytes
        .min(default_operation.output_bytes);
    let district_limits =
        match execution_limits.default_rules_limits(default_operation, event_map_cap) {
            Ok(limits) => limits,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    default_ticket,
                    FoundationOrchestratorError::Command(error),
                );
            }
        };
    let biblio_limits = match execution_limits.item_limits(default_operation) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                default_ticket,
                FoundationOrchestratorError::Command(error),
            );
        }
    };
    let default_schema_limits = match schema_limits_for_ticket(
        execution_limits,
        &default_ticket,
        schema_count,
        schema_max,
        schema_total,
        max_checks,
    ) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, default_ticket, error);
        }
    };
    let default_diagnostic_limits = CutSchemaDiagnosticsLimits {
        max_total_issues: default_operation.issue_count.max(1),
        max_total_report_bytes: default_operation.output_bytes.max(1),
        max_total_state_bytes: default_operation.state_bytes.max(1),
    };
    let executor_budget = match ticket_worker_budget(
        &default_ticket,
        deadline,
        budgets.worker_address_space_bytes,
    ) {
        Ok(budget) => budget,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, default_ticket, error);
        }
    };
    let layer_shape = cut_worker_shape(default_operation, max_checks);
    let layer_stream =
        match execution_limits.cut_worker_stream_budget(default_operation, layer_shape) {
            Ok(stream) => stream,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    default_ticket,
                    FoundationOrchestratorError::Command(error),
                );
            }
        };
    let layer_worker_limits = CutWorkerLimits {
        max_receipts: max_checks,
        max_receipt_bytes: default_operation.state_bytes.min(1024 * 1024),
    };
    // The prepared closure's temporary DOMs are admitted before constructor allocation.
    // This workspace expires at constructor return; retained controller state
    // continues through its existing phase/H accounting, without a refund.
    if schema_preparation_state > default_operation.state_bytes {
        return fail_window(
            execution_limits,
            remaining_budget,
            default_ticket,
            incomplete("layer schema preparation exceeds remaining state"),
        );
    }
    let mut layer_schemas = match CutWorkerSchemaExecutor::from_cut_with_image(
        captured.cut(),
        FormatProfile::LegacyPythonObserved20260923,
        &worker_image,
        executor_budget,
        layer_worker_limits,
        deadline,
        cancelled,
    ) {
        Ok(executor) => executor,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                default_ticket,
                owner(error),
            );
        }
    };
    if layer_schemas.set_operation_budget(layer_stream).is_err()
        || CutSchemaDiagnosticsLimits::from_operation_ceilings(
            default_diagnostic_limits,
            layer_worker_limits.max_receipts,
            layer_stream,
        )
        .and_then(|limits| layer_schemas.enable_diagnostics_v2(limits))
        .is_err()
        || layer_schemas
            .set_shared_schema_worker_quota(worker_quota.clone())
            .is_err()
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            default_ticket,
            incomplete("default layer schema worker limits refused"),
        );
    }
    let default_schema_usage_before = match worker_quota.usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                default_ticket,
                incomplete("default schema quota unavailable"),
            );
        }
    };
    payloads
        .restrict_remaining_budget(
            default_free.source_read_bytes,
            payloads
                .cost()
                .peak_state_bytes
                .checked_add(default_free.state_bytes)
                .ok_or_else(|| incomplete("payload state allowance overflow"))?,
            deadline,
            cancelled,
        )
        .map_err(owner)?;
    physical
        .restrict_remaining_budget(
            usize::try_from(default_free.source_read_bytes).unwrap_or(usize::MAX),
            physical
                .cost()
                .retained_state_bytes
                .checked_add(default_free.state_bytes)
                .ok_or_else(|| incomplete("physical state allowance overflow"))?,
            deadline,
            cancelled,
        )
        .map_err(owner)?;
    let histories = replay.histories();
    let artifact_replays = replay.artifact_replays();
    let physical_facts: &SourcePhysicalFacts = physical.facts();
    let evaluated = match foundation_run::evaluate_default(
        captured,
        &current_paths.paths,
        records,
        sources,
        &mut payloads,
        &mut record_executor,
        &mut item_schemas,
        &mut layer_schemas,
        physical_facts,
        histories,
        &artifact_replays,
        replay.invalid_schema_proofs(),
        launch.arguments.require_local_payloads,
        records_limits,
        biblio_limits,
        reader_limits,
        district_limits,
        &schema_set,
        worker_image.identity(),
        &worker_image,
        default_schema_limits,
        SourceFoundationRuleDiagnosticsLimits {
            max_issues: default_operation.issue_count.max(1),
            max_output_bytes: default_operation.output_bytes.max(1),
            max_state_bytes: default_operation.state_bytes.max(1),
        },
        &worker_quota,
        history.as_deref_mut(),
        cancelled,
    ) {
        Ok(evaluated) => evaluated,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                default_ticket,
                FoundationOrchestratorError::Default(error),
            );
        }
    };
    let default_schema_usage_after = match worker_quota.usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                default_ticket,
                incomplete("default schema quota usage unavailable"),
            );
        }
    };
    let (default_cpu, default_wire) =
        match use_delta(default_schema_usage_before, default_schema_usage_after) {
            Ok(delta) => delta,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, default_ticket, error);
            }
        };
    let record_state_baseline = evaluated
        .rules
        .owner_report
        .cost
        .records_state_reservation_upper_bound_bytes;
    let owner_state = match evaluated
        .rules
        .owner_report
        .cost
        .aggregate_state_reservation_bytes
        .checked_sub(record_state_baseline)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                default_ticket,
                incomplete("default owner state baseline regressed"),
            );
        }
    };
    let biblio_state = match &evaluated.bibliography {
        FoundationBiblioEvidence::Complete {
            state_reservation_bytes,
            ..
        } => *state_reservation_bytes,
        FoundationBiblioEvidence::NotSelected | FoundationBiblioEvidence::RecordsIncomplete => 0,
    };
    let biblio_read = match &evaluated.bibliography {
        FoundationBiblioEvidence::Complete { report, .. } => report.bytes_read,
        FoundationBiblioEvidence::NotSelected | FoundationBiblioEvidence::RecordsIncomplete => 0,
    };
    let biblio_issues = match &evaluated.bibliography {
        FoundationBiblioEvidence::Complete { report, .. } => report.shadow.issues.len(),
        FoundationBiblioEvidence::NotSelected | FoundationBiblioEvidence::RecordsIncomplete => 0,
    };
    let default_new_issues = evaluated
        .rules
        .cost
        .issue_count
        .checked_sub(records_issues)
        .and_then(|count| count.checked_add(biblio_issues));
    let Some(default_new_issues) = default_new_issues else {
        return fail_window(
            execution_limits,
            remaining_budget,
            default_ticket,
            incomplete("default issue baseline regressed"),
        );
    };
    let default_read = match checked_add_u64(
        evaluated.rules.owner_report.cost.later_counted_source_bytes,
        biblio_read,
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, default_ticket, error);
        }
    };
    let default_state = match checked_add_usize(
        owner_state,
        evaluated.rules.cost.peak_additional_state_upper_bound_bytes,
    )
    .and_then(|bytes| checked_add_usize(bytes, biblio_state))
    .and_then(|bytes| checked_add_usize(bytes, evaluated.reader_cost.auxiliary_state_bytes))
    .and_then(|bytes| checked_add_usize(bytes, default_identity_copy_state))
    {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, default_ticket, error);
        }
    };
    let default_usage = phase_use(
        default_read,
        default_wire,
        default_state,
        default_new_issues,
        default_cpu,
        0,
        0,
    );
    complete_window(
        execution_limits,
        remaining_budget,
        default_ticket,
        default_usage,
    )?;

    // Keep the compiler's existing planner and transfer kernel. Candidate
    // mode adds only its fresh disposable sink and callback; ordinary mode
    // compares the same selected source against its current generated files.
    let catalog_ticket = open_window(
        execution_limits,
        remaining_budget,
        "catalog-and-persisted-inputs",
        FoundationWindowKind::CatalogAndPersisted,
        FoundationPhaseReservation::default(),
    )?;
    // The catalog validator retains a Cut/native identity pair plus its
    // independent worker-pin identity for the lifetime of comparison.
    let catalog_identity_copy_state =
        match worker_identity_path_clone_bytes(worker_image.identity(), 3) {
            Ok(bytes) => bytes,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
            }
        };
    let mut catalog_operation = catalog_ticket.operation_limits();
    catalog_operation.state_bytes = match catalog_operation
        .state_bytes
        .checked_sub(catalog_identity_copy_state)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalog worker identity state exceeds remaining budget"),
            );
        }
    };
    let catalog_free = catalog_ticket.remaining();
    let mut catalog_members = 0usize;
    let mut largest_captured_member_bytes = 0u64;
    for member in captured.cut().current().members() {
        let path = member.path.as_str();
        if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalog source-shape census interrupted"),
            );
        }
        largest_captured_member_bytes = largest_captured_member_bytes.max(member.size_bytes);
        if path.starts_with("ToS/source-witnesses/") {
            catalog_members = catalog_members
                .checked_add(1)
                .ok_or_else(|| incomplete("catalog source-shape count overflow"))?;
        }
    }
    let max_catalog_members = catalog_members.min(4096).max(1);
    let max_catalog_files = max_members.min(u64::MAX as usize - 1).max(1) as u64;
    let max_catalog_rows = catalog_operation.source_read_bytes.min(u64::MAX - 1).max(1);
    let max_catalog_file_bytes = usize::try_from(
        budgets
            .max_member_bytes
            .min(16 * 1024 * 1024)
            .min(largest_captured_member_bytes.max(1))
            .min(catalog_operation.source_read_bytes)
            .min((usize::MAX - 1) as u64),
    )
    .map_err(|_| incomplete("catalog file cap range"))?;
    let max_catalog_row_bytes = max_catalog_file_bytes.min(1024 * 1024).max(1);
    let max_catalog_output_row_bytes = usize::try_from(
        catalog_operation
            .tmpfs_bytes
            .min((4 * 1024 * 1024) as u64)
            .min((usize::MAX - 1) as u64),
    )
    .map_err(|_| incomplete("catalog output row cap range"))?;
    let catalog_limits = match execution_limits.catalog_limits(
        max_catalog_files,
        max_catalog_rows,
        max_catalog_file_bytes,
        max_catalog_row_bytes,
        max_catalog_row_bytes,
        max_catalog_output_row_bytes,
    ) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                FoundationOrchestratorError::Command(error),
            );
        }
    };
    let catalog_source_limits = match execution_limits.catalog_input_limits(max_catalog_members) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                FoundationOrchestratorError::Command(error),
            );
        }
    };
    let catalog_read_limits = match execution_limits.read_limits(
        catalog_operation,
        max_members,
        catalog_operation.source_read_bytes,
    ) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                FoundationOrchestratorError::Command(error),
            );
        }
    };
    let max_claim_bytes = catalog_operation.state_bytes.min(128 * 1024 * 1024);
    // This is one Claim's simultaneous rendered cohort, not the whole catalog.
    // Keep the existing row/byte constructor invariant inside actual remaining
    // state; zero capacity must refuse rather than fabricate a positive slice.
    let max_claim_rows = (catalog_limits.max_rows.min(16_384) as usize)
        .min(max_claim_bytes / catalog_limits.max_output_row_bytes);
    if max_claim_rows == 0 {
        return fail_window(
            execution_limits,
            remaining_budget,
            catalog_ticket,
            incomplete("bibliographic Claim cohort exceeds remaining state"),
        );
    }
    let biblio_catalog_limits = match execution_limits.bibliography_limits(
        catalog_limits,
        max_claim_rows,
        max_claim_bytes,
        catalog_limits
            .max_rows
            .min(catalog_operation.output_bytes as u64)
            .max(1),
        catalog_operation
            .tmpfs_bytes
            .min(catalog_operation.output_bytes as u64)
            .max(1),
    ) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                FoundationOrchestratorError::Command(error),
            );
        }
    };
    let stage_limits = match execution_limits.stage_limits(
        max_catalog_rows,
        max_catalog_rows.min(1024) as usize,
        catalog_operation.tmpfs_bytes.min(64 * 1024 * 1024),
    ) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                FoundationOrchestratorError::Command(error),
            );
        }
    };
    let catalog_worker_shape = FoundationCatalogWorkerShape {
        batch: BatchBudget::laboratory(),
        max_chunks: max_checks
            .div_ceil(BatchBudget::laboratory().max_units)
            .max(1) as u64,
        max_total_units: u64::try_from(max_checks)
            .unwrap_or(u64::MAX - 1)
            .min(catalog_operation.worker_wire_bytes)
            .max(1),
        max_total_raw_bytes: catalog_operation
            .source_read_bytes
            .min(catalog_operation.worker_wire_bytes)
            .min(BatchBudget::MAX_RAW_BYTES as u64)
            .max(1),
        max_total_wire_bytes: catalog_operation.worker_wire_bytes,
        max_distinct_selectors: max_checks.max(1).min(1024),
        max_receipts: max_checks.max(1),
        max_receipt_bytes: catalog_operation.state_bytes.min(1024 * 1024).max(1),
    };
    let (catalog_executor, catalog_worker_limits, catalog_stream, catalog_diagnostic_limits) =
        match execution_limits.catalog_worker_limits(catalog_worker_shape) {
            Ok(limits) => limits,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    FoundationOrchestratorError::Command(error),
                );
            }
        };
    let candidate_path = isolated.path().join("source-foundation.sqlite");
    let catalog_before = match worker_quota.usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalog shared quota unavailable"),
            );
        }
    };
    let tmpfs_before = match stage.quota_usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalog stage quota observation unavailable"),
            );
        }
    };
    let candidate_mode = receive.is_some();
    let mut candidate_result = None;
    let mut fresh_candidate = None;
    let mut fresh_catalog_sources = None;
    let mut admission_index = None;
    let catalog_outcome = if candidate_mode {
        match foundation_catalog::compare_candidate(
            captured,
            sources,
            &candidate_path,
            stage,
            stage_limits,
            catalog_source_limits,
            catalog_read_limits,
            biblio_catalog_limits,
            &worker_image,
            catalog_executor,
            catalog_worker_limits,
            catalog_stream,
            catalog_diagnostic_limits,
            true,
            usize::try_from(max_catalog_files).unwrap_or(usize::MAX - 1),
            max_catalog_file_bytes,
            usize::try_from(
                catalog_operation
                    .source_read_bytes
                    .min(usize::MAX as u64 - 1),
            )
            .unwrap_or(usize::MAX - 1),
            usize::try_from(max_catalog_files).unwrap_or(usize::MAX - 1),
            catalog_operation.state_bytes,
            usize::try_from(
                catalog_operation
                    .source_read_bytes
                    .min(usize::MAX as u64 - 1),
            )
            .unwrap_or(usize::MAX - 1),
            &worker_quota,
            isolated,
            DisposableCatalogTreeLimits {
                max_total_bytes: usize::try_from(
                    catalog_operation.tmpfs_bytes.min(usize::MAX as u64 - 1),
                )
                .unwrap_or(usize::MAX - 1),
                max_file_bytes: max_catalog_output_row_bytes,
                max_files: usize::try_from(max_catalog_files).unwrap_or(usize::MAX - 1),
                max_state_bytes: catalog_operation.state_bytes,
                max_inodes: usize::try_from(max_catalog_files)
                    .unwrap_or(usize::MAX - 4)
                    .saturating_add(3),
            },
            cancelled,
        ) {
            Ok(result) => {
                let outcome = result.outcome;
                fresh_candidate = result.candidate;
                fresh_catalog_sources = result.fresh_sources;
                outcome
            }
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    FoundationOrchestratorError::Catalog(error),
                );
            }
        }
    } else {
        match foundation_catalog::compare(
            captured,
            sources,
            &candidate_path,
            stage,
            stage_limits,
            catalog_source_limits,
            catalog_read_limits,
            biblio_catalog_limits,
            &worker_image,
            catalog_executor,
            catalog_worker_limits,
            catalog_stream,
            catalog_diagnostic_limits,
            true,
            usize::try_from(max_catalog_files).unwrap_or(usize::MAX - 1),
            max_catalog_file_bytes,
            usize::try_from(
                catalog_operation
                    .source_read_bytes
                    .min(usize::MAX as u64 - 1),
            )
            .unwrap_or(usize::MAX - 1),
            usize::try_from(max_catalog_files).unwrap_or(usize::MAX - 1),
            catalog_operation.state_bytes,
            usize::try_from(
                catalog_operation
                    .source_read_bytes
                    .min(usize::MAX as u64 - 1),
            )
            .unwrap_or(usize::MAX - 1),
            &worker_quota,
            cancelled,
        ) {
            Ok(outcome) => outcome,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    FoundationOrchestratorError::Catalog(error),
                );
            }
        }
    };
    let catalog_read = match catalog_read_bytes(&catalog_outcome) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
        }
    };
    let catalog_issues = match catalog_issue_count(&catalog_outcome) {
        Ok(issues) => issues,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
        }
    };
    let mut catalog_state = match catalog_retained_state(&catalog_outcome) {
        Ok(state) => state,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
        }
    };
    catalog_state = match checked_add_usize(catalog_state, catalog_identity_copy_state) {
        Ok(state) => state,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
        }
    };
    let fresh_cost = fresh_candidate.as_ref().map(|candidate| candidate.cost());
    if let Some(fresh) = fresh_cost {
        catalog_state = match checked_add_usize(
            catalog_state,
            fresh.fresh_rows_retained_state_upper_bound_bytes,
        ) {
            Ok(state) => state,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
            }
        };
    }
    let catalog_read = match fresh_cost {
        Some(fresh) => match checked_add_u64(catalog_read, fresh.readback_bytes as u64) {
            Ok(bytes) => bytes,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
            }
        },
        None => catalog_read,
    };
    let candidate_schema_clean = match &catalog_outcome {
        FoundationCatalogOutcome::Complete(result) => result.issues.is_empty(),
        FoundationCatalogOutcome::SchemaRejected { .. } => false,
    };
    let owner_report = &evaluated.rules.owner_report;
    let default_clean = evaluated.rules.cost.issue_count == 0
        && owner_report.cost.direct_owner_issue_count == 0
        && owner_report.labs.unimplemented.is_empty()
        && owner_report
            .labs
            .results
            .iter()
            .all(|lab| lab.unimplemented.is_empty())
        && owner_report.records.unimplemented.is_empty()
        && owner_report.goldsets.coverage_gaps.is_empty()
        && owner_report.discovery.unsupported.is_empty()
        && owner_report.closure.unsupported.is_empty()
        && owner_report.cost.queued_schema_document_count
            == evaluated.rules.cost.schema_check_count
        && match (
            &evaluated.rules.diagnostics,
            owner_report.cost.queued_schema_document_count,
        ) {
            (None, 0) => true,
            (Some(report), count) if count > 0 => {
                report.is_complete() && report.expected_check_count == count
            }
            _ => false,
        }
        && match &evaluated.bibliography {
            FoundationBiblioEvidence::NotSelected => true,
            FoundationBiblioEvidence::RecordsIncomplete => false,
            FoundationBiblioEvidence::Complete { report, .. } => {
                !report.shadow.issue_sink_truncated && report.shadow.issues.is_empty()
            }
        };
    let catalog_quota_after = match worker_quota.usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalog schema quota usage unavailable"),
            );
        }
    };
    let (mut catalog_cpu, mut catalog_wire) = match use_delta(catalog_before, catalog_quota_after) {
        Ok(delta) => delta,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
        }
    };
    let catalog_pre_callback_use = phase_use(
        catalog_read,
        catalog_wire,
        catalog_state,
        catalog_issues,
        catalog_cpu,
        fresh_cost.map_or(0, |fresh| fresh.output_bytes as u64),
        fresh_cost.map_or(0, |fresh| fresh.created_inodes as u64),
    );
    if phase_amounts(catalog_pre_callback_use)?.source_read_bytes > catalog_free.source_read_bytes
        || catalog_state > catalog_free.state_bytes
        || catalog_issues > catalog_free.issue_count
        || catalog_wire > catalog_free.worker_wire_bytes
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            catalog_ticket,
            incomplete("catalog report exceeds the remaining invocation reservation"),
        );
    }

    if candidate_mode && default_clean && candidate_schema_clean {
        if let (Some(candidate), Some(receiver)) = (fresh_candidate.as_mut(), receive.take()) {
            if !candidate.eof_verified() {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("fresh catalog candidate EOF verification absent"),
                );
            }
            let rows = crate::source_admission_index::FreshRows {
                records: std::mem::take(&mut candidate.fresh_rows_mut().records),
                claims: std::mem::take(&mut candidate.fresh_rows_mut().claims),
                native_semantic: std::mem::take(&mut candidate.fresh_rows_mut().native_semantic),
            };
            let check_count = rows
                .records
                .len()
                .checked_add(rows.claims.len())
                .filter(|count| *count > 0 && *count <= max_checks)
                .ok_or_else(|| incomplete("candidate schema check count exceeds selected cap"))?;
            let callback_free =
                match reservation_minus(catalog_free, phase_amounts(catalog_pre_callback_use)?) {
                    Ok(free) => free,
                    Err(error) => {
                        return fail_window(
                            execution_limits,
                            remaining_budget,
                            catalog_ticket,
                            error,
                        );
                    }
                };
            let max_schema_instance = catalog_limits
                .max_output_row_bytes
                .min(BatchBudget::MAX_RAW_BYTES);
            if max_schema_instance == 0 {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("candidate schema instance cap is zero"),
                );
            }
            let mut callback_shape = cut_worker_shape(callback_free, check_count);
            callback_shape.max_total_raw_bytes = callback_shape
                .max_total_raw_bytes
                .min(max_schema_instance as u64)
                .max(1);
            callback_shape.aggregate_wire_upper_bound_bytes = callback_free.worker_wire_bytes;
            let callback_stream =
                match execution_limits.cut_worker_stream_budget(callback_free, callback_shape) {
                    Ok(stream) => stream,
                    Err(error) => {
                        return fail_window(
                            execution_limits,
                            remaining_budget,
                            catalog_ticket,
                            FoundationOrchestratorError::Command(error),
                        );
                    }
                };
            let callback_worker_budget = match ticket_worker_budget(
                &catalog_ticket,
                deadline,
                budgets.worker_address_space_bytes,
            ) {
                Ok(budget) => budget,
                Err(error) => {
                    return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
                }
            };
            // The callback's CutWorkerSchemaExecutor retains its own worker
            // identity and a native prepared-image identity. Its worker
            // duplicates the sealed FD but does not copy the executable.
            let callback_identity_copy_state =
                match worker_identity_path_clone_bytes(worker_image.identity(), 2) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        return fail_window(
                            execution_limits,
                            remaining_budget,
                            catalog_ticket,
                            error,
                        );
                    }
                };
            let callback_limits = CutWorkerLimits {
                max_receipts: check_count,
                max_receipt_bytes: catalog_limits.max_output_row_bytes,
            };
            // The prepared closure's temporary DOMs are admitted before constructor allocation.
            // This workspace expires at constructor return; retained controller state
            // continues through its existing phase/H accounting, without a refund.
            let callback_preparation_state =
                match schema_preparation_state.checked_add(callback_identity_copy_state) {
                    Some(bytes) => bytes,
                    None => {
                        return fail_window(
                            execution_limits,
                            remaining_budget,
                            catalog_ticket,
                            incomplete("admission callback schema preparation state overflow"),
                        );
                    }
                };
            if callback_preparation_state > callback_free.state_bytes {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("admission callback schema preparation exceeds remaining state"),
                );
            }
            let mut callback_schema = match CutWorkerSchemaExecutor::from_cut_with_image(
                captured.cut(),
                FormatProfile::LegacyPythonObserved20260923,
                &worker_image,
                callback_worker_budget,
                callback_limits,
                deadline,
                cancelled,
            ) {
                Ok(executor) => executor,
                Err(error) => {
                    return fail_window(
                        execution_limits,
                        remaining_budget,
                        catalog_ticket,
                        owner(error),
                    );
                }
            };
            let callback_report_bytes =
                usize::try_from(callback_free.worker_wire_bytes.min(usize::MAX as u64 - 1))
                    .unwrap_or(usize::MAX - 1);
            if callback_schema
                .set_operation_budget(callback_stream)
                .is_err()
                || CutSchemaDiagnosticsLimits::from_operation_ceilings(
                    CutSchemaDiagnosticsLimits {
                        max_total_issues: callback_free.issue_count.max(1),
                        max_total_report_bytes: callback_report_bytes.max(1),
                        max_total_state_bytes: callback_free.state_bytes.max(1),
                    },
                    callback_limits.max_receipts,
                    callback_stream,
                )
                .and_then(|limits| callback_schema.enable_diagnostics_v2(limits))
                .is_err()
                || callback_schema
                    .set_diagnostics_v2_legacy_raw_instance_limit(max_schema_instance)
                    .is_err()
                || callback_schema
                    .set_shared_schema_worker_quota(worker_quota.clone())
                    .is_err()
            {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("candidate schema callback worker setup refused"),
                );
            }
            let controller_state = match callback_schema
                .diagnostics_v2_controller_state_upper_bound(max_schema_instance, 4096)
            {
                Ok(state) => state,
                Err(error) => {
                    return fail_window(
                        execution_limits,
                        remaining_budget,
                        catalog_ticket,
                        owner(error),
                    );
                }
            };
            let callback_available_state = match callback_free
                .state_bytes
                .checked_sub(controller_state)
                .and_then(|state| state.checked_sub(callback_identity_copy_state))
            {
                Some(state) => state,
                None => {
                    return fail_window(
                        execution_limits,
                        remaining_budget,
                        catalog_ticket,
                        incomplete("candidate schema and worker state exceed remaining budget"),
                    );
                }
            };
            if callback_schema
                .set_diagnostics_v2_controller_state_cap(controller_state)
                .is_err()
            {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("candidate schema controller cap refused"),
                );
            }
            let mut schema_check = |path: &str,
                                    raw: &[u8],
                                    schema_raw: &[u8]|
             -> io::Result<Option<String>> {
                let diagnostic = callback_schema
                    .check_diagnostics_v2_for_schema_raw(path, raw, schema_raw, deadline, cancelled)
                    .map_err(|_| io::Error::other("selected candidate schema check refused"))?;
                if diagnostic.is_valid() {
                    Ok(None)
                } else if diagnostic.is_invalid() {
                    Ok(Some("selected candidate schema is invalid".to_owned()))
                } else {
                    Err(io::Error::other(
                        "selected candidate schema report incomplete",
                    ))
                }
            };
            let callback_reservation = FoundationPhaseReservation {
                state_bytes: callback_available_state,
                ..callback_free
            };
            let callback = receiver;
            let (index, callback_usage) =
                match callback(rows, &mut schema_check, callback_reservation) {
                    Ok(result) => result,
                    Err(error) => {
                        return fail_window(
                            execution_limits,
                            remaining_budget,
                            catalog_ticket,
                            FoundationOrchestratorError::Admission(error),
                        );
                    }
                };
            if !matches!(
                callback_usage.worker_cpu,
                FoundationWorkerCpuUse::MeasuredMicros(0)
            ) || callback_usage.worker_wire_bytes.amount != 0
            {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("candidate callback duplicated shared schema-worker accounting"),
                );
            }
            if callback_schema.finish(deadline, cancelled).is_err() {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("candidate schema callback worker finish refused"),
                );
            }
            let callback_quota_after = match worker_quota.usage() {
                Ok(usage) => usage,
                Err(_) => {
                    return fail_window(
                        execution_limits,
                        remaining_budget,
                        catalog_ticket,
                        incomplete("candidate schema quota usage unavailable"),
                    );
                }
            };
            let (catalog_cpu_after_callback, catalog_wire_after_callback) =
                match use_delta(catalog_before, callback_quota_after) {
                    Ok(delta) => delta,
                    Err(error) => {
                        return fail_window(
                            execution_limits,
                            remaining_budget,
                            catalog_ticket,
                            error,
                        );
                    }
                };
            catalog_cpu = catalog_cpu_after_callback;
            catalog_wire = catalog_wire_after_callback;
            let mut callback_amounts = match phase_amounts(callback_usage) {
                Ok(amounts) => amounts,
                Err(error) => {
                    return fail_window(execution_limits, remaining_budget, catalog_ticket, error);
                }
            };
            if callback_amounts.source_read_bytes > callback_reservation.source_read_bytes
                || callback_amounts.state_bytes > callback_reservation.state_bytes
                || callback_amounts.issue_count > callback_reservation.issue_count
                || callback_amounts.output_bytes > callback_reservation.output_bytes
                || callback_amounts.tmpfs_bytes > callback_reservation.tmpfs_bytes
                || callback_amounts.tmpfs_inodes > callback_reservation.tmpfs_inodes
            {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    catalog_ticket,
                    incomplete("candidate callback exceeded its live reservation"),
                );
            }
            callback_amounts.state_bytes =
                match callback_amounts.state_bytes.checked_add(controller_state) {
                    Some(state) => state,
                    None => {
                        return fail_window(
                            execution_limits,
                            remaining_budget,
                            catalog_ticket,
                            incomplete("candidate schema controller state overflow"),
                        );
                    }
                };
            callback_amounts.state_bytes = match callback_amounts
                .state_bytes
                .checked_add(callback_identity_copy_state)
            {
                Some(state) => state,
                None => {
                    return fail_window(
                        execution_limits,
                        remaining_budget,
                        catalog_ticket,
                        incomplete("candidate worker identity state overflow"),
                    );
                }
            };
            candidate_result = Some(callback_amounts);
            admission_index = Some(index);
        }
    }
    let tmpfs_after = match stage.quota_usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalog final stage quota observation unavailable"),
            );
        }
    };
    let tmpfs_bytes = tmpfs_after
        .used_bytes
        .saturating_sub(tmpfs_before.used_bytes);
    let tmpfs_inodes = tmpfs_after
        .used_inodes
        .saturating_sub(tmpfs_before.used_inodes);
    let catalog_usage = FoundationPhaseUse {
        tmpfs_bytes: FoundationCharge::measured(tmpfs_bytes),
        tmpfs_inodes: FoundationCharge::measured(tmpfs_inodes),
        ..catalog_pre_callback_use
    };
    let mut catalog_amounts = phase_amounts(catalog_usage)?;
    if let Some(callback_amounts) = candidate_result {
        catalog_amounts.source_read_bytes = checked_add_u64(
            catalog_amounts.source_read_bytes,
            callback_amounts.source_read_bytes,
        )?;
        catalog_amounts.state_bytes =
            checked_add_usize(catalog_amounts.state_bytes, callback_amounts.state_bytes)?;
        catalog_amounts.issue_count =
            checked_add_usize(catalog_amounts.issue_count, callback_amounts.issue_count)?;
        catalog_amounts.output_bytes =
            checked_add_usize(catalog_amounts.output_bytes, callback_amounts.output_bytes)?;
        catalog_amounts.tmpfs_bytes =
            checked_add_u64(catalog_amounts.tmpfs_bytes, callback_amounts.tmpfs_bytes)?;
        catalog_amounts.tmpfs_inodes =
            checked_add_u64(catalog_amounts.tmpfs_inodes, callback_amounts.tmpfs_inodes)?;
    }
    let catalog_usage = FoundationPhaseUse {
        source_read_bytes: FoundationCharge::measured(catalog_amounts.source_read_bytes),
        worker_wire_bytes: FoundationCharge::measured(catalog_wire),
        state_bytes: FoundationCharge::admitted_upper_bound(catalog_amounts.state_bytes),
        issue_count: FoundationCharge::measured(catalog_amounts.issue_count),
        output_bytes: FoundationCharge::measured(catalog_amounts.output_bytes),
        worker_cpu: FoundationWorkerCpuUse::MeasuredMicros(catalog_cpu),
        tmpfs_bytes: FoundationCharge::measured(catalog_amounts.tmpfs_bytes),
        tmpfs_inodes: FoundationCharge::measured(catalog_amounts.tmpfs_inodes),
    };
    complete_window(
        execution_limits,
        remaining_budget,
        catalog_ticket,
        catalog_usage,
    )?;

    let persisted_ticket = open_window(
        execution_limits,
        remaining_budget,
        "persisted-catalog-tail",
        FoundationWindowKind::CatalogAndPersisted,
        FoundationPhaseReservation::default(),
    )?;
    // The persisted schema wrapper uses one native prepared identity at a
    // time across its chunks; it releases each before the next one.
    let persisted_identity_copy_state =
        match worker_identity_path_clone_bytes(worker_image.identity(), 1) {
            Ok(bytes) => bytes,
            Err(error) => {
                return fail_window(execution_limits, remaining_budget, persisted_ticket, error);
            }
        };
    let mut persisted_operation = persisted_ticket.operation_limits();
    persisted_operation.state_bytes = match persisted_operation
        .state_bytes
        .checked_sub(persisted_identity_copy_state)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                persisted_ticket,
                incomplete("persisted worker identity state exceeds remaining budget"),
            );
        }
    };
    let extensions = match catalog_profiles(&catalog_outcome) {
        Ok(files) => files,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, persisted_ticket, error);
        }
    };
    let persisted_file_count = match extensions
        .len()
        .checked_add(10)
        .filter(|count| *count <= max_members)
    {
        Some(count) => count,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                persisted_ticket,
                incomplete("persisted catalog selected-file cap exceeded"),
            );
        }
    };
    let persisted_report = match foundation_catalog::inspect_persisted_catalog(
        sources,
        extensions,
        schema_set.catalog_entry_schema_present(),
        schema_set.catalog_claim_entry_schema_present(),
        max_catalog_file_bytes,
        usize::try_from(
            persisted_operation
                .source_read_bytes
                .min(usize::MAX as u64 - 1),
        )
        .unwrap_or(usize::MAX - 1),
        persisted_file_count,
        persisted_operation.state_bytes,
        persisted_operation.issue_count,
        max_checks,
        deadline,
        cancelled,
    ) {
        Ok(report) => report,
        Err(error) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                persisted_ticket,
                FoundationOrchestratorError::Catalog(error),
            );
        }
    };
    let persisted_read = persisted_report.generated_inputs.read_bytes();
    let persisted_schema_limits = match schema_limits_for_ticket(
        execution_limits,
        &persisted_ticket,
        schema_count,
        schema_max,
        schema_total,
        max_checks,
    ) {
        Ok(limits) => limits,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, persisted_ticket, error);
        }
    };
    let persisted_before = match worker_quota.usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                persisted_ticket,
                incomplete("persisted schema quota unavailable"),
            );
        }
    };
    let persisted = match foundation_catalog::evaluate_persisted_catalog_with_shared_quota_and_image(
        persisted_report,
        &schema_set,
        &worker_image,
        persisted_schema_limits,
        deadline,
        cancelled,
        persisted_operation.issue_count,
        persisted_operation.output_bytes,
        persisted_operation.state_bytes,
        &worker_quota,
    ) {
        Ok(report) => report,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                persisted_ticket,
                incomplete("persisted catalog schema evaluation refused"),
            );
        }
    };
    let persisted_after = match worker_quota.usage() {
        Ok(usage) => usage,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                persisted_ticket,
                incomplete("persisted schema quota usage unavailable"),
            );
        }
    };
    let (persisted_cpu, persisted_wire) = match use_delta(persisted_before, persisted_after) {
        Ok(delta) => delta,
        Err(error) => {
            return fail_window(execution_limits, remaining_budget, persisted_ticket, error);
        }
    };
    let persisted_issue_count = persisted.issues.len();
    let persisted_state = match persisted
        .generated_inputs
        .retained_state_bytes()
        .checked_add(persisted.parsed_input_state_bytes)
        .and_then(|state| state.checked_add(persisted.input_workspace_bytes))
        .and_then(|state| state.checked_add(size_of::<EvaluatedPersistedCatalog>()))
        .and_then(|state| {
            persisted
                .diagnostics
                .as_ref()
                .map_or(Some(state), |report| {
                    state
                        .checked_add(size_of::<
                            tos_validation::source_foundation_schema::SourceFoundationSchemaReport,
                        >())
                        .and_then(|value| value.checked_add(report.cost.estimated_report_bytes))
                })
        })
        .and_then(|state| state.checked_add(persisted_identity_copy_state))
    {
        Some(state) => state,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                persisted_ticket,
                incomplete("persisted state accounting overflow"),
            );
        }
    };
    let persisted_usage = phase_use(
        persisted_read as u64,
        persisted_wire,
        persisted_state,
        persisted_issue_count,
        persisted_cpu,
        0,
        0,
    );
    complete_window(
        execution_limits,
        remaining_budget,
        persisted_ticket,
        persisted_usage,
    )?;

    let final_ticket = open_window(
        execution_limits,
        remaining_budget,
        "final-custody-and-output",
        FoundationWindowKind::FinalCustodyAndOutput,
        FoundationPhaseReservation {
            issue_count: remaining_budget.charged().issue_count,
            ..FoundationPhaseReservation::default()
        },
    )?;
    let final_free = final_ticket.remaining();
    payloads
        .restrict_remaining_budget(
            final_free.source_read_bytes,
            payloads
                .cost()
                .peak_state_bytes
                .checked_add(final_free.state_bytes)
                .ok_or_else(|| incomplete("payload final state allowance overflow"))?,
            deadline,
            cancelled,
        )
        .map_err(owner)?;
    physical
        .restrict_remaining_budget(
            usize::try_from(final_free.source_read_bytes).unwrap_or(usize::MAX),
            physical
                .cost()
                .retained_state_bytes
                .checked_add(final_free.state_bytes)
                .ok_or_else(|| incomplete("physical final state allowance overflow"))?,
            deadline,
            cancelled,
        )
        .map_err(owner)?;
    if invocation.verify_before_source_eof().is_err() {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("invocation or executable changed before source EOF"),
        );
    }
    let invocation_delta = invocation
        .cost
        .invocation_read_bytes
        .checked_sub(cost.invocation_after_initial_read.invocation_read_bytes)
        .and_then(|bytes| {
            invocation
                .cost
                .self_image_read_bytes
                .checked_sub(cost.invocation_after_initial_read.self_image_read_bytes)
                .and_then(|self_bytes| bytes.checked_add(self_bytes))
        });
    let Some(invocation_delta) = invocation_delta else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("invocation EOF read cost regressed"),
        );
    };
    let final_read_limit = execution_limits
        .final_authored_member_read_bytes()
        .map_err(FoundationOrchestratorError::Command)?;
    const FINAL_CONTROL_RESERVE: u64 = 8192;
    if final_free.source_read_bytes < FINAL_CONTROL_RESERVE {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("final publication-control allowance exhausted"),
        );
    }
    // Reserve the exact selected-member byte census plus the fixed publication
    // control ceiling before any other final input provider can spend source
    // reads. The capture helper enforces this total before each member stream.
    let Some(captured_member_read_upper) =
        captured
            .observed_members()
            .try_fold(0usize, |sum, (_, member)| {
                usize::try_from(member.size_bytes)
                    .ok()
                    .and_then(|bytes| sum.checked_add(bytes))
            })
    else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("captured EOF member byte census overflow"),
        );
    };
    let Some(final_member_read_limit) = captured_member_read_upper
        .checked_add(usize::try_from(FINAL_CONTROL_RESERVE).unwrap_or(usize::MAX))
    else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("captured EOF control reservation overflow"),
        );
    };
    if final_member_read_limit > final_read_limit {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("captured EOF upper bound exceeds protected read limit"),
        );
    }
    let Some(after_invocation_read) = final_free.source_read_bytes.checked_sub(invocation_delta)
    else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("invocation EOF reads exceed final reservation"),
        );
    };
    let Ok(capture_read_reservation) = u64::try_from(final_member_read_limit) else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("captured EOF reservation exceeds platform size"),
        );
    };
    if capture_read_reservation > after_invocation_read {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("captured EOF reservation exceeds remaining source reads"),
        );
    }
    let history_before_final = history
        .as_deref()
        .map_or((0, 0), |provider| provider.usage());
    let Some(history_state_limit) = history_before_final.1.checked_add(final_free.state_bytes)
    else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("historical final state allowance overflow"),
        );
    };
    let Some(history_read_limit) = final_free
        .source_read_bytes
        .checked_sub(invocation_delta)
        .and_then(|bytes| bytes.checked_sub(capture_read_reservation))
    else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("historical final read allowance overflow"),
        );
    };
    if let Some(provider) = history.as_deref_mut() {
        if provider
            .recheck(history_read_limit, history_state_limit, deadline, cancelled)
            .is_err()
        {
            return fail_window(
                execution_limits,
                remaining_budget,
                final_ticket,
                incomplete("historical input final recheck refused"),
            );
        }
    }
    let history_after_final = history
        .as_deref()
        .map_or((0, 0), |provider| provider.usage());
    let history_final_read = history_after_final
        .0
        .checked_sub(history_before_final.0)
        .ok_or_else(|| incomplete("historical final read cost regressed"))?;
    let history_final_state = history_after_final
        .1
        .checked_sub(history_before_final.1)
        .ok_or_else(|| incomplete("historical final state cost regressed"))?;
    let Some(final_source_read_limit) = final_free
        .source_read_bytes
        .checked_sub(invocation_delta)
        .and_then(|bytes| bytes.checked_sub(history_final_read))
    else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("final source read allowance overflow"),
        );
    };
    let Some(final_state_limit) = final_free.state_bytes.checked_sub(history_final_state) else {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("historical final state exceeds reservation"),
        );
    };
    let final_read_limits = execution_limits
        .read_limits(
            final_ticket.operation_limits(),
            max_members,
            final_free.source_read_bytes,
        )
        .map_err(FoundationOrchestratorError::Command)?;
    let physical_before_final = physical.cost();
    let evaluated_reader_before = evaluated.reader_cost.bytes_read;
    let catalog_read_before_final = catalog_read_bytes(&catalog_outcome)?;
    let persisted_read_before_final = persisted.generated_inputs.read_bytes() as u64;
    let finalized = if let (Some(_), Some(fresh_sources)) =
        (admission_index.as_ref(), fresh_catalog_sources.as_mut())
    {
        match foundation_run::finalize_candidate_inputs(
            evaluated,
            catalog_outcome,
            persisted,
            captured,
            sources,
            artifact_sources,
            physical,
            payloads,
            final_read_limits,
            final_member_read_limit,
            final_source_read_limit,
            final_state_limit,
            deadline,
            cancelled,
            fresh_sources,
        ) {
            Ok(finalized) => finalized,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    final_ticket,
                    FoundationOrchestratorError::Final(error),
                );
            }
        }
    } else {
        match foundation_run::finalize_default_inputs(
            evaluated,
            catalog_outcome,
            persisted,
            captured,
            sources,
            artifact_sources,
            physical,
            payloads,
            final_read_limits,
            final_member_read_limit,
            final_source_read_limit,
            final_state_limit,
            deadline,
            cancelled,
        ) {
            Ok(finalized) => finalized,
            Err(error) => {
                return fail_window(
                    execution_limits,
                    remaining_budget,
                    final_ticket,
                    FoundationOrchestratorError::Final(error),
                );
            }
        }
    };
    let final_payload_state = finalized.payload_completion.cost.final_facts_state_bytes;
    let final_catalog_read = catalog_read_bytes(&finalized.catalog)?;
    let final_catalog_read_delta = final_catalog_read
        .checked_sub(catalog_read_before_final)
        .ok_or_else(|| incomplete("catalog final read cost regressed"))?;
    let final_persisted_read = finalized.persisted_catalog.generated_inputs.read_bytes() as u64;
    let final_persisted_read_delta = final_persisted_read
        .checked_sub(persisted_read_before_final)
        .ok_or_else(|| incomplete("persisted final read cost regressed"))?;
    let auxiliary_final_read = finalized
        .reader_cost
        .bytes_read
        .checked_sub(evaluated_reader_before)
        .ok_or_else(|| incomplete("auxiliary final read cost regressed"))?;
    let physical_final_read = finalized
        .physical_cost
        .bytes_read
        .checked_sub(physical_before_final.bytes_read)
        .ok_or_else(|| incomplete("physical final read cost regressed"))?;
    let payload_final_read = finalized.payload_completion.cost.final_bytes_read;
    let physical_final_state = finalized
        .physical_cost
        .retained_state_bytes
        .checked_sub(physical_before_final.retained_state_bytes)
        .ok_or_else(|| incomplete("physical final state cost regressed"))?;
    let final_source_read = invocation_delta
        .checked_add(finalized.final_authored_member_read_bytes as u64)
        .and_then(|bytes| bytes.checked_add(final_catalog_read_delta))
        .and_then(|bytes| bytes.checked_add(final_persisted_read_delta))
        .and_then(|bytes| bytes.checked_add(auxiliary_final_read))
        .and_then(|bytes| bytes.checked_add(physical_final_read as u64))
        .and_then(|bytes| bytes.checked_add(payload_final_read))
        .and_then(|bytes| bytes.checked_add(history_final_read))
        .ok_or_else(|| incomplete("final source-read accounting overflow"))?;
    if final_source_read > final_free.source_read_bytes {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("final source reads exceed remaining reservation"),
        );
    }
    let output_state_limit = match final_free
        .state_bytes
        .checked_sub(final_payload_state)
        .and_then(|state| state.checked_sub(history_final_state))
        .and_then(|state| state.checked_sub(physical_final_state))
    {
        Some(state) => state,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                final_ticket,
                incomplete("final payload facts exceed remaining state reservation"),
            );
        }
    };
    let output_limits = super::foundation_output::SourceFoundationOutputLimits {
        max_issues: final_ticket.operation_limits().issue_count,
        max_output_bytes: final_ticket
            .operation_limits()
            .output_bytes
            .min(usize::MAX - 1),
        max_state_bytes: output_state_limit,
    };
    let output = foundation_output::assemble_default(finalized, output_limits);
    let (output_state, output_bytes) = match &output {
        SourceFoundationOutputOutcome::Complete(assembled) => (
            assembled.cost.additional_state_upper_bound_bytes,
            assembled.cost.issue_utf8_bytes,
        ),
        SourceFoundationOutputOutcome::Incomplete { gaps, .. } => (
            size_of::<SourceFoundationOutputOutcome>()
                .checked_add(
                    gaps.capacity()
                        .saturating_mul(size_of::<foundation_output::SourceFoundationOutputGap>()),
                )
                .unwrap_or(usize::MAX),
            0,
        ),
        SourceFoundationOutputOutcome::Refused { .. } => {
            (size_of::<SourceFoundationOutputOutcome>(), 0)
        }
    };
    let total_output_state = match output_state
        .checked_add(final_payload_state)
        .and_then(|state| state.checked_add(history_final_state))
        .and_then(|state| state.checked_add(physical_final_state))
    {
        Some(state) => state,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                final_ticket,
                incomplete("final output state overflow"),
            );
        }
    };
    if total_output_state > final_free.state_bytes || output_bytes > output_limits.max_output_bytes
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            final_ticket,
            incomplete("final assembled output exceeds remaining reservation"),
        );
    }
    let output_use = FoundationPhaseUse {
        source_read_bytes: FoundationCharge::measured(final_source_read),
        worker_wire_bytes: FoundationCharge::measured(0),
        state_bytes: FoundationCharge::admitted_upper_bound(total_output_state),
        issue_count: FoundationCharge::measured(0),
        output_bytes: FoundationCharge::measured(output_bytes),
        worker_cpu: FoundationWorkerCpuUse::MeasuredMicros(0),
        tmpfs_bytes: FoundationCharge::measured(0),
        tmpfs_inodes: FoundationCharge::measured(0),
    };
    complete_window(execution_limits, remaining_budget, final_ticket, output_use)?;
    if let Some(index) = admission_index {
        match output {
            SourceFoundationOutputOutcome::Complete(assembled) if assembled.issues.is_empty() => {
                Ok(JoinedOutcome::Admission(index))
            }
            _ => Err(incomplete(
                "candidate foundation output remained incomplete",
            )),
        }
    } else {
        Ok(JoinedOutcome::Default(output))
    }
}
