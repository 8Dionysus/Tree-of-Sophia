//! Whole-invocation owner join for the maintained native source-foundation
//! command. Each real owner runs in one serial ledger window; reports are
//! carried forward and consumed by the existing finalizer/output assembler.

use super::foundation_artifact_replay::{
    ArtifactReplayFailure, CandidateArtifactSchemaExecutor, prepare_artifact_replay,
};
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
use super::foundation_rule_diagnostics::{
    CandidateRuleDiagnosticsError, SourceFoundationRuleDiagnosticsError,
    SourceFoundationRuleDiagnosticsLimits,
};
use super::foundation_run::{
    self, EvaluatedFoundationDefault, FinalizedFoundationDefaultInputs, FoundationBiblioEvidence,
    FoundationDefaultReadError, FoundationFinalInputError,
};
use crate::source_command::SourceCommandError;
use crate::source_creation_store::DisposableCatalogTreeLimits;
use std::cell::RefCell;
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
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::record_biblio_cut::{BiblioRecordExecutor, BiblioSchemaDiagnosticsLimits};
use tos_validation::source_cut::{
    CutSchemaDiagnosticsLimits, CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor,
    cut_schema_resource_preparation_state_upper_bound,
    streamed_cut_schema_resource_preparation_state_upper_bound,
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
    OwnerAt(&'static str, ItemRefusal),
    Default(FoundationDefaultStage, FoundationDefaultReadError),
    Final(FoundationFinalInputError),
    Replay(ArtifactReplayFailure),
    Catalog(tos_compiler::Error),
    Persisted(PersistedCatalogEvaluationError),
    Admission(io::Error),
    Incomplete(&'static str),
}

const MAX_PUBLIC_BUDGET_REASON_BYTES: usize = 192;

// Reuse the completed, same-input Biblio owner result. Selected and generated
// coverage still binds the exact file, physical line and optional declared ID;
// neither route reruns schema work or synthesizes a missing Claim observation.
fn require_completed_biblio_claim(
    claims: &dyn tos_validation::source_foundation_default_rules::SourceFoundationDefaultClaims,
    path: &str,
    line: usize,
    file_sha256: Digest256,
    identity: Option<&str>,
) -> Result<(), ItemRefusal> {
    let mut observations = 0usize;
    claims.for_each_claim_at(path, line, &mut |_, claim| {
        if claim.path != path
            || claim.line != line
            || Digest256::from_hex(&claim.raw_sha256).ok() != Some(file_sha256)
            || identity.is_some_and(|id| {
                claim
                    .value
                    .get("claim_id")
                    .and_then(serde_json::Value::as_str)
                    != Some(id)
            })
        {
            return Err(ItemRefusal::Source(
                "Claim differs from completed Biblio owner".into(),
            ));
        }
        observations = observations
            .checked_add(1)
            .ok_or(tos_validation::item_budget_origin!())?;
        Ok(())
    })?;
    if observations != 1 {
        return Err(ItemRefusal::Source(
            "completed Biblio Claim coverage is missing or ambiguous".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum FoundationDefaultStage {
    CapturedCurrentPaths,
    PrepareRecords,
    EvaluateDefault,
}

impl From<FoundationBootstrapError> for FoundationOrchestratorError {
    fn from(error: FoundationBootstrapError) -> Self {
        Self::Bootstrap(error)
    }
}

impl FoundationOrchestratorError {
    pub(crate) fn public_reason(&self) -> String {
        if let Self::Catalog(error) = self {
            return crate::source_command::public_compiler_reason(error);
        }
        if let Self::Command(error)
        | Self::Bootstrap(
            FoundationBootstrapError::Command(error)
            | FoundationBootstrapError::Capture(error)
            | FoundationBootstrapError::IsolatedRoot(error),
        ) = self
        {
            return error.public_reason();
        }
        if let Self::Bootstrap(FoundationBootstrapError::RouteRoot(error)) = self {
            return crate::source_command::public_io_reason(error);
        }
        if let Self::Default(
            _,
            FoundationDefaultReadError::Owner(ItemRefusal::Executor(evidence)),
        ) = self
        {
            return evidence.summary();
        }
        if let Self::Admission(error) = self {
            if error.get_ref().is_some_and(|cause| {
                cause.is::<crate::source_command::SourceCommandError>()
                    || cause.is::<tos_validation::item_rules::ItemExecutorRefusal>()
            }) {
                return crate::source_command::public_io_reason(error);
            }
            let source_cause = error.to_string();
            if crate::source_admission_spooled_index::is_bounded_source_cause(&source_cause) {
                return source_cause;
            }
            if self.static_public_reason() == "source-foundation candidate index refused" {
                return crate::source_admission_spooled_index::bounded_source_cause(
                    "receiver-source",
                    "candidate-index-admission",
                    &source_cause,
                );
            }
        }
        let owner = match self {
            Self::Owner(error) => Some(("owner", error)),
            Self::OwnerAt(stage, error) => Some((*stage, error)),
            Self::Bootstrap(FoundationBootstrapError::Selection(error)) => {
                Some(("bootstrap-selection", error))
            }
            Self::Bootstrap(FoundationBootstrapError::Payload(error)) => {
                Some(("bootstrap-payload", error))
            }
            Self::Bootstrap(FoundationBootstrapError::Physical(error)) => {
                Some(("bootstrap-physical", error))
            }
            _ => None,
        };
        if let Some((stage, error)) = owner {
            if matches!(
                error,
                ItemRefusal::Source(_) | ItemRefusal::Unsupported(_) | ItemRefusal::Executor(_)
            ) || matches!(error, ItemRefusal::BudgetCheck { check, .. }
                    if !matches!(*check, "record_issue_sink" | "biblio_sink"))
            {
                return crate::source_admission_spooled_index::receiver_refusal(error.clone())
                    .to_string();
            }
            let kind = match error {
                ItemRefusal::Budget => "budget",
                ItemRefusal::BudgetCheck { .. } => "budget check",
                ItemRefusal::Deadline => "deadline",
                ItemRefusal::Source(_) => "source",
                ItemRefusal::Unsupported(_) => "unsupported",
                ItemRefusal::Executor(_) => "executor",
            };
            use std::fmt::Write as _;
            let mut reason = String::with_capacity(MAX_PUBLIC_BUDGET_REASON_BYTES);
            let formatted: std::fmt::Result = (|| {
                write!(reason, "source-foundation {stage}: owner {kind}")?;
                if let ItemRefusal::BudgetCheck { check, used, limit } = error
                    && matches!(*check, "record_issue_sink" | "biblio_sink")
                {
                    write!(reason, " {check} used=")?;
                    match used {
                        Some(value) => write!(reason, "{value}")?,
                        None => reason.push_str("unknown"),
                    }
                    reason.push_str(" limit=");
                    match limit {
                        Some(value) => write!(reason, "{value}")?,
                        None => reason.push_str("unknown"),
                    }
                }
                Ok(())
            })();
            if formatted.is_ok() && reason.len() <= MAX_PUBLIC_BUDGET_REASON_BYTES {
                return reason;
            }
        }
        if let Self::Default(
            stage,
            FoundationDefaultReadError::Owner(ItemRefusal::BudgetCheck { check, used, limit }),
        ) = self
        {
            if matches!(*check, "record_issue_sink" | "biblio_sink") {
                let stage = match stage {
                    FoundationDefaultStage::CapturedCurrentPaths => {
                        "source-foundation default captured current paths"
                    }
                    FoundationDefaultStage::PrepareRecords => {
                        "source-foundation default prepare records"
                    }
                    FoundationDefaultStage::EvaluateDefault => {
                        "source-foundation default evaluate default"
                    }
                };
                // The allow-listed labels and two optional u64 counters fit
                // this fixed buffer; the CLI separately charges emitted bytes.
                use std::fmt::Write as _;
                let mut reason = String::with_capacity(MAX_PUBLIC_BUDGET_REASON_BYTES);
                let formatted: std::fmt::Result = (|| {
                    write!(reason, "{stage}: owner budget check {check} used=")?;
                    match used {
                        Some(value) => write!(reason, "{value}")?,
                        None => reason.push_str("unknown"),
                    }
                    reason.push_str(" limit=");
                    match limit {
                        Some(value) => write!(reason, "{value}")?,
                        None => reason.push_str("unknown"),
                    }
                    Ok(())
                })();
                if formatted.is_ok() && reason.len() <= MAX_PUBLIC_BUDGET_REASON_BYTES {
                    return reason;
                }
            }
        }
        self.static_public_reason().to_owned()
    }

    fn static_public_reason(&self) -> &'static str {
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
            Self::Owner(_) | Self::OwnerAt(_, _) => "source-foundation owner phase refused",
            Self::Default(stage, error) => match (stage, error) {
                (stage, FoundationDefaultReadError::Owner(error @ ItemRefusal::Executor(_))) => {
                    Self::Default(
                        *stage,
                        FoundationDefaultReadError::Owner(error.clone().compatibility_category()),
                    )
                    .static_public_reason()
                }
                (
                    FoundationDefaultStage::CapturedCurrentPaths,
                    FoundationDefaultReadError::Owner(ItemRefusal::Budget),
                ) => "source-foundation default captured current paths: owner budget",
                (
                    FoundationDefaultStage::CapturedCurrentPaths,
                    FoundationDefaultReadError::Owner(ItemRefusal::BudgetCheck { .. }),
                ) => "source-foundation default captured current paths: owner budget check",
                (
                    FoundationDefaultStage::CapturedCurrentPaths,
                    FoundationDefaultReadError::Owner(ItemRefusal::Deadline),
                ) => "source-foundation default captured current paths: owner deadline",
                (
                    FoundationDefaultStage::CapturedCurrentPaths,
                    FoundationDefaultReadError::Owner(ItemRefusal::Source(_)),
                ) => "source-foundation default captured current paths: owner source",
                (
                    FoundationDefaultStage::CapturedCurrentPaths,
                    FoundationDefaultReadError::Owner(ItemRefusal::Unsupported(_)),
                ) => "source-foundation default captured current paths: owner unsupported",
                (
                    FoundationDefaultStage::CapturedCurrentPaths,
                    FoundationDefaultReadError::Diagnostics(
                        SourceFoundationRuleDiagnosticsError::Refused { .. },
                    ),
                ) => "source-foundation default captured current paths: diagnostics refused",
                (
                    FoundationDefaultStage::CapturedCurrentPaths,
                    FoundationDefaultReadError::Diagnostics(
                        SourceFoundationRuleDiagnosticsError::IncompleteSchema { .. },
                    ),
                ) => {
                    "source-foundation default captured current paths: diagnostics incomplete schema"
                }
                (
                    FoundationDefaultStage::PrepareRecords,
                    FoundationDefaultReadError::Owner(ItemRefusal::Budget),
                ) => "source-foundation default prepare records: owner budget",
                (
                    FoundationDefaultStage::PrepareRecords,
                    FoundationDefaultReadError::Owner(ItemRefusal::BudgetCheck { .. }),
                ) => "source-foundation default prepare records: owner budget check",
                (
                    FoundationDefaultStage::PrepareRecords,
                    FoundationDefaultReadError::Owner(ItemRefusal::Deadline),
                ) => "source-foundation default prepare records: owner deadline",
                (
                    FoundationDefaultStage::PrepareRecords,
                    FoundationDefaultReadError::Owner(ItemRefusal::Source(_)),
                ) => "source-foundation default prepare records: owner source",
                (
                    FoundationDefaultStage::PrepareRecords,
                    FoundationDefaultReadError::Owner(ItemRefusal::Unsupported(_)),
                ) => "source-foundation default prepare records: owner unsupported",
                (
                    FoundationDefaultStage::PrepareRecords,
                    FoundationDefaultReadError::Diagnostics(
                        SourceFoundationRuleDiagnosticsError::Refused { .. },
                    ),
                ) => "source-foundation default prepare records: diagnostics refused",
                (
                    FoundationDefaultStage::PrepareRecords,
                    FoundationDefaultReadError::Diagnostics(
                        SourceFoundationRuleDiagnosticsError::IncompleteSchema { .. },
                    ),
                ) => "source-foundation default prepare records: diagnostics incomplete schema",
                (
                    FoundationDefaultStage::EvaluateDefault,
                    FoundationDefaultReadError::Owner(ItemRefusal::Budget),
                ) => "source-foundation default evaluate default: owner budget",
                (
                    FoundationDefaultStage::EvaluateDefault,
                    FoundationDefaultReadError::Owner(ItemRefusal::BudgetCheck { .. }),
                ) => "source-foundation default evaluate default: owner budget check",
                (
                    FoundationDefaultStage::EvaluateDefault,
                    FoundationDefaultReadError::Owner(ItemRefusal::Deadline),
                ) => "source-foundation default evaluate default: owner deadline",
                (
                    FoundationDefaultStage::EvaluateDefault,
                    FoundationDefaultReadError::Owner(ItemRefusal::Source(_)),
                ) => "source-foundation default evaluate default: owner source",
                (
                    FoundationDefaultStage::EvaluateDefault,
                    FoundationDefaultReadError::Owner(ItemRefusal::Unsupported(_)),
                ) => "source-foundation default evaluate default: owner unsupported",
                (
                    FoundationDefaultStage::EvaluateDefault,
                    FoundationDefaultReadError::Diagnostics(
                        SourceFoundationRuleDiagnosticsError::Refused { .. },
                    ),
                ) => "source-foundation default evaluate default: diagnostics refused",
                (
                    FoundationDefaultStage::EvaluateDefault,
                    FoundationDefaultReadError::Diagnostics(
                        SourceFoundationRuleDiagnosticsError::IncompleteSchema { .. },
                    ),
                ) => "source-foundation default evaluate default: diagnostics incomplete schema",
            },
            Self::Final(_) => "source-foundation final custody refused",
            Self::Replay(_) => "source-foundation artifact replay refused",
            Self::Catalog(_) => "source-foundation catalog comparison refused",
            Self::Persisted(_) => "source-foundation persisted catalog refused",
            Self::Admission(error) => {
                // Return only known owner-authored static diagnostics. Unknown
                // errors may contain private paths or payloads and remain opaque.
                match error.to_string().as_str() {
                    "native source index SQLite scalar missing" => {
                        "native source index SQLite scalar missing"
                    }
                    "native source index SQLite column type refused" => {
                        "native source index SQLite column type refused"
                    }
                    "native source index SQLite query shape refused" => {
                        "native source index SQLite query shape refused"
                    }
                    "candidate spool is unusable" => "candidate spool is unusable",
                    "native source candidate membership did not reach its fenced EOF" => {
                        "native source candidate membership did not reach its fenced EOF"
                    }
                    "candidate spool per-row state exceeded" => {
                        "candidate spool per-row state exceeded"
                    }
                    "native source index profile is invalid" => {
                        "native source index profile is invalid"
                    }
                    "native source index shared-budget SQLite open refused" => {
                        "native source index shared-budget SQLite open refused"
                    }
                    "native source index SQLite ceiling is too small" => {
                        "native source index SQLite ceiling is too small"
                    }
                    "native source index SQLite policy changed" => {
                        "native source index SQLite policy changed"
                    }
                    "native source index row state exceeds profile" => {
                        "native source index row state exceeds profile"
                    }
                    "native source index SQLite busy" => "native source index SQLite busy",
                    "native source index SQLite out of memory" => {
                        "native source index SQLite out of memory"
                    }
                    "native source index SQLite read only" => {
                        "native source index SQLite read only"
                    }
                    "native source index SQLite I/O refused" => {
                        "native source index SQLite I/O refused"
                    }
                    "native source index SQLite corrupt" => "native source index SQLite corrupt",
                    "native source index SQLite full" => "native source index SQLite full",
                    "native source index SQLite open refused" => {
                        "native source index SQLite open refused"
                    }
                    "native source index SQLite schema changed" => {
                        "native source index SQLite schema changed"
                    }
                    "native source index SQLite operation refused" => {
                        "native source index SQLite operation refused"
                    }
                    "native source index storage refused" => "native source index storage refused",
                    "candidate Records/Item receiver budget refused" => {
                        "candidate Records/Item receiver budget refused"
                    }
                    "candidate Records/Item receiver budget check refused" => {
                        "candidate Records/Item receiver budget check refused"
                    }
                    "candidate Records/Item receiver deadline refused" => {
                        "candidate Records/Item receiver deadline refused"
                    }
                    "candidate Records/Item receiver source refused" => {
                        "candidate Records/Item receiver source refused"
                    }
                    "candidate Records/Item receiver unsupported" => {
                        "candidate Records/Item receiver unsupported"
                    }
                    _ => crate::source_admission_spooled_index::receiver_source_reason(
                        &error.to_string(),
                    )
                    .unwrap_or("source-foundation candidate index refused"),
                }
            }
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
    match run(view, payloads, None, None)? {
        JoinedOutcome::Default(outcome) => Ok(outcome),
        JoinedOutcome::Admission(_) => Err(FoundationOrchestratorError::Incomplete(
            "ordinary foundation evaluation returned admission state",
        )),
    }
}

pub(crate) type CatalogueObserver<'a> = dyn FnOnce(
        &mut tos_compiler::knowledge_stage::KnowledgeStage<'_>,
        &tos_compiler::source_witness_catalog::ColdSourceCatalogReceipt,
        tos_compiler::source_witness_catalog::SourceCatalogLimits,
        super::foundation_command::SourceFoundationCatalogueObservationOrigin,
    ) -> tos_compiler::Result<()>
    + 'a;

pub(crate) struct OwnedCatalogueObservation<'a> {
    pub limits: super::foundation_command::SourceFoundationCatalogueObservationLimits,
    pub callback: Box<CatalogueObserver<'a>>,
    pub retained_closure_state_bytes: usize,
}

pub(crate) fn evaluate_with_owned_catalogue_observation<'work, 'observe, 'cancel, 'signal>(
    view: FoundationBootstrapView<'work, 'cancel, 'signal>,
    payloads: FoundationPayloadSources<'work>,
    observation: OwnedCatalogueObservation<'observe>,
) -> Result<SourceFoundationOutputOutcome, FoundationOrchestratorError> {
    match run(view, payloads, None, Some(observation))? {
        JoinedOutcome::Default(outcome) => Ok(outcome),
        JoinedOutcome::Admission(_) => {
            Err(incomplete("catalogue observation returned admission state"))
        }
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
    match run(view, payloads, Some(Box::new(receive)), None)? {
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

/// Complete one candidate phase and adopt its original read/write/upper-bound
/// prefix only when Foundation retained both read classifications.
#[allow(clippy::too_many_arguments)]
fn complete_candidate_window(
    execution_limits: &mut FoundationExecutionLimits,
    remaining_budget: &mut FoundationRemainingBudget<'_>,
    ticket: FoundationBudgetTicket,
    usage: FoundationPhaseUse,
    io_before: tos_source_store::PinnedSqliteIoSnapshot,
    io_after: tos_source_store::PinnedSqliteIoSnapshot,
    adopted: &mut (u64, u64, u64),
    remaining_write_bytes: &mut u64,
    remaining_write: u64,
) -> Result<(), FoundationOrchestratorError> {
    let upper = io_after
        .read_upper_bound_attempted_bytes
        .checked_sub(io_before.read_upper_bound_attempted_bytes);
    if *adopted
        != (
            io_before.read_attempted_bytes,
            io_before.write_attempted_bytes,
            io_before.read_upper_bound_attempted_bytes,
        )
        || upper.is_none_or(|upper| upper > usage.source_read_bytes.amount)
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            ticket,
            FoundationOrchestratorError::Incomplete("candidate classified IO prefix differs"),
        );
    }
    let upper = upper.expect("checked upper-bound suffix");
    let measured_before = remaining_budget.measured_charged().source_read_bytes;
    let upper_before = remaining_budget.admitted_charged().source_read_bytes;
    let total_read = usage.source_read_bytes.amount;
    let completion = match execution_limits.clear_window(&ticket) {
        Ok(()) => remaining_budget
            .complete_window_with_source_read_upper_bound(ticket, usage, upper)
            .map_err(FoundationOrchestratorError::Command),
        Err(error) => {
            let worst = failure_use(&ticket);
            let _ = remaining_budget
                .fail_window_with_classified_source_reads(ticket, worst, total_read, upper);
            Err(FoundationOrchestratorError::Command(error))
        }
    };
    if remaining_budget
        .measured_charged()
        .source_read_bytes
        .checked_sub(measured_before)
        == Some(total_read - upper)
        && remaining_budget
            .admitted_charged()
            .source_read_bytes
            .checked_sub(upper_before)
            == Some(upper)
    {
        *adopted = (
            io_after.read_attempted_bytes,
            io_after.write_attempted_bytes,
            io_after.read_upper_bound_attempted_bytes,
        );
        *remaining_write_bytes = remaining_write;
    }
    completion
}

/// Close a failed candidate phase against the exact attempted-read prefix of
/// the shared original ledger. The adopted counters move only when measured
/// and admitted-upper-bound read amounts are both retained by Foundation.
#[allow(clippy::too_many_arguments)]
fn fail_candidate_window_with_classified_io<T>(
    execution_limits: &mut FoundationExecutionLimits,
    remaining_budget: &mut FoundationRemainingBudget<'_>,
    ticket: FoundationBudgetTicket,
    original_io: &tos_source_store::PinnedSqliteIoBudget,
    io_before: tos_source_store::PinnedSqliteIoSnapshot,
    external_reads: u64,
    adopted: &mut (u64, u64, u64),
    remaining_write_bytes: &mut u64,
    error: FoundationOrchestratorError,
) -> Result<T, FoundationOrchestratorError> {
    let io_after = original_io.snapshot();
    let Some(shared_read) = io_after
        .read_attempted_bytes
        .checked_sub(io_before.read_attempted_bytes)
    else {
        let worst = failure_use(&ticket);
        let _ = execution_limits.clear_window(&ticket);
        let _ = remaining_budget.fail_window(ticket, worst);
        return Err(error);
    };
    let Some(write_delta) = io_after
        .write_attempted_bytes
        .checked_sub(io_before.write_attempted_bytes)
    else {
        let worst = failure_use(&ticket);
        let _ = execution_limits.clear_window(&ticket);
        let _ = remaining_budget.fail_window(ticket, worst);
        return Err(error);
    };
    let Some(actual_read) = shared_read.checked_add(external_reads) else {
        let worst = failure_use(&ticket);
        let _ = execution_limits.clear_window(&ticket);
        let _ = remaining_budget.fail_window(ticket, worst);
        return Err(error);
    };
    let Some(upper) = io_after
        .read_upper_bound_attempted_bytes
        .checked_sub(io_before.read_upper_bound_attempted_bytes)
        .filter(|upper| *upper <= shared_read)
    else {
        let worst = failure_use(&ticket);
        let _ = execution_limits.clear_window(&ticket);
        let _ = remaining_budget.fail_window(ticket, worst);
        return Err(error);
    };
    let upper_before = remaining_budget.admitted_charged().source_read_bytes;
    let measured_before = remaining_budget.measured_charged().source_read_bytes;
    let worst = failure_use(&ticket);
    let _ = execution_limits.clear_window(&ticket);
    let _ = remaining_budget.fail_window_with_classified_source_reads(
        ticket,
        worst,
        actual_read,
        upper,
    );
    let measured_after = remaining_budget.measured_charged().source_read_bytes;
    let measured_delta = measured_after.checked_sub(measured_before);
    if measured_delta == Some(actual_read - upper)
        && remaining_budget
            .admitted_charged()
            .source_read_bytes
            .checked_sub(upper_before)
            == Some(upper)
    {
        // Both read classifications advance independently of write headroom.
        // A denied write must not make terminal accounting charge this
        // already-recorded read prefix a second time.
        adopted.0 = io_after.read_attempted_bytes;
        adopted.2 = io_after.read_upper_bound_attempted_bytes;
        if let Some(write_after) = remaining_write_bytes.checked_sub(write_delta) {
            adopted.1 = io_after.write_attempted_bytes;
            *remaining_write_bytes = write_after;
        }
    }
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

fn candidate_callback_refusal(stage: &'static str, error: ItemRefusal) -> io::Error {
    crate::source_admission_spooled_index::receiver_refusal(candidate_owner_refusal(stage, error))
}

fn candidate_owner_refusal(stage: &'static str, error: ItemRefusal) -> ItemRefusal {
    match error {
        ItemRefusal::Budget => ItemRefusal::BudgetCheck {
            check: stage,
            used: None,
            limit: None,
        },
        ItemRefusal::Source(reason)
            if !crate::source_admission_spooled_index::is_bounded_source_cause(&reason) =>
        {
            // The stage is a code-owned static label; parser text and paths
            // remain hashed. Preserve already bounded owner sites unchanged.
            let site = stage.replace(' ', "-");
            ItemRefusal::Source(crate::source_admission_spooled_index::bounded_source_cause(
                "receiver-source",
                &site,
                &reason,
            ))
        }
        other => other,
    }
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
    let max_checks =
        max_checks.min(tos_validation::source_foundation_schema::MAX_SOURCE_FOUNDATION_CHECKS);
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
            bounded_usize(operation.worker_wire_bytes)?
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
        // Record and cut diagnostics submit one unit per physical frame.
        // The batch capacity is an upper bound, not guaranteed occupancy.
        // Keep the finite unit envelope while admitting its scalar transport.
        max_chunks: units.max(1),
        max_total_units: units.max(1),
        max_total_raw_bytes: operation
            .source_read_bytes
            .min(operation.worker_wire_bytes)
            .max(1),
        aggregate_wire_upper_bound_bytes: operation.worker_wire_bytes,
        max_distinct_selectors: max_checks.max(1).min(1024),
    }
}

fn schema_state_upper_bound(
    bytes: usize,
    resources: usize,
    source_resource_metadata: usize,
) -> Result<usize, FoundationOrchestratorError> {
    bytes
        .checked_mul(8)
        .and_then(|state| {
            resources
                .checked_mul(512)
                .and_then(|rows| state.checked_add(rows))
        })
        .and_then(|state| state.checked_add(source_resource_metadata))
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

fn catalog_issue_count<B>(
    outcome: &FoundationCatalogOutcome<B>,
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

fn catalog_read_bytes<B, D>(
    outcome: &FoundationCatalogOutcome<B, D>,
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

fn catalog_retained_state<B, D>(
    outcome: &FoundationCatalogOutcome<B, D>,
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

fn outcome_schema_execution_cost<B, D>(
    outcome: &FoundationCatalogOutcome<B, D>,
) -> tos_validation::source_cut::CutSchemaDiagnosticsCumulativeCost {
    match outcome {
        FoundationCatalogOutcome::Complete(result) => result.schema_execution_cost,
        FoundationCatalogOutcome::SchemaRejected {
            schema_execution_cost,
            ..
        } => *schema_execution_cost,
    }
}

fn catalog_profiles<B, D>(
    outcome: &FoundationCatalogOutcome<B, D>,
) -> Result<&[(String, String)], FoundationOrchestratorError> {
    let profiles = match outcome {
        FoundationCatalogOutcome::Complete(result) => &result.profiles,
        FoundationCatalogOutcome::SchemaRejected { profiles, .. } => profiles,
    };
    profiles
        .files()
        .map_err(|_| incomplete("catalog record-profile selection incomplete"))
}

/// Select the actual candidate closure and move its bytes into the same
/// prepared worker kernel. Both the Records worker and the later native-index
/// callback worker use this path with one verified image and aggregate quota.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_candidate_schema_worker(
    view: &super::foundation_bootstrap::CandidateFoundationBootstrapView<'_, '_, '_, '_>,
    input: &crate::source_admission_candidate_records::CandidateRecordsInput<'_, '_>,
    image: &VerifiedWorkerImageHandle,
    quota: &SharedSchemaWorkerQuota,
    ticket: &FoundationBudgetTicket,
    schema_limits: SourceFoundationSchemaLimits,
    metadata_state_limit: usize,
    worker_limits: CutWorkerLimits,
    diagnostics: CutSchemaDiagnosticsLimits,
    operation: tos_validation::executor::BatchStreamBudget,
    held_state_bytes: usize,
    max_operation_state_bytes: usize,
) -> Result<
    tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        crate::source_admission_spooled_candidate::CandidateFence,
    >,
    FoundationOrchestratorError,
> {
    use tos_validation::record_biblio_cut::SourceCutInputWithIdentity;
    let deadline = view.invocation.deadline();
    let cancelled = view.cancelled;
    input
        .verify_invocation(deadline, cancelled)
        .map_err(owner)?;
    if input.input_identity() != view.input.input_identity() {
        return Err(incomplete(
            "candidate schema preparation source binding differs",
        ));
    }
    let image_state = image
        .retained_state_bytes()
        .map_err(|_| incomplete("candidate worker image state unavailable"))?;
    if held_state_bytes < bounded_usize(image_state)? {
        return Err(incomplete("candidate held worker image is unreserved"));
    }
    // Both initial Records and later native-index preparation use this helper.
    // Reserve only the selected descriptor/path geometry here; passing the
    // whole state ceiling would add that ceiling again to the overlap below.
    let selected_metadata_state =
        tos_validation::source_foundation_schema::CandidateSourceFoundationSchemaSet::<
            crate::source_admission_spooled_candidate::CandidateFence,
        >::source_resource_metadata_state_upper_bound(schema_limits.max_schema_resources)
        .ok_or_else(|| incomplete("candidate selected schema metadata state overflow"))?;
    let metadata_state_limit = metadata_state_limit.min(selected_metadata_state);
    let preparation = tos_validation::source_cut::source_foundation_candidate_schema_preparation_state_upper_bound(
        schema_limits, metadata_state_limit,
    ).map_err(owner)?;
    let required = held_state_bytes
        .checked_add(preparation)
        .and_then(|bytes| {
            bytes.checked_add(worker_identity_path_clone_bytes(image.identity(), 2).ok()?)
        })
        .and_then(|bytes| {
            bytes.checked_add(size_of::<
                tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
                    crate::source_admission_spooled_candidate::CandidateFence,
                >,
            >())
        })
        .ok_or_else(|| incomplete("candidate schema preparation overlap overflow"))?;
    input
        .require_callback_state(
            required,
            max_operation_state_bytes,
            "candidate schema constructor callback state",
        )
        .map_err(owner)?;
    let set =
        tos_validation::source_foundation_schema::CandidateSourceFoundationSchemaSet::from_input(
            input,
            FormatProfile::LegacyPythonObserved20260923,
            schema_limits,
            metadata_state_limit,
            deadline,
            cancelled,
        )
        .map_err(schema_load)?;
    let budget = ticket_worker_budget(
        ticket,
        deadline,
        view.invocation.budgets.worker_address_space_bytes,
    )?;
    let mut worker = tos_validation::source_cut::CandidateCutWorkerSchemaExecutor::from_schema_set(
        set,
        image,
        budget,
        worker_limits,
        deadline,
        cancelled,
    )
    .map_err(owner)?;
    let diagnostics = CutSchemaDiagnosticsLimits::from_operation_ceilings(
        diagnostics,
        worker_limits.max_receipts,
        operation,
    )
    .map_err(owner)?;
    worker.set_operation_budget(operation).map_err(owner)?;
    worker.enable_diagnostics_v2(diagnostics).map_err(owner)?;
    worker
        .set_shared_schema_worker_quota(quota.clone())
        .map_err(owner)?;
    let controller = worker
        .diagnostics_v2_controller_state_upper_bound(schema_limits.max_instance_bytes, 4096)
        .map_err(owner)?;
    // from_schema_set has consumed/dropped raw constructor and parser
    // workspace. Price the retained worker with the same bound charged by the
    // caller, plus its next controller exchange; constructor peak is separate.
    let retained_schema = schema_state_upper_bound(
        worker.schema_bytes(),
        worker.source_resource_count(),
        worker
            .source_resource_metadata_state_bytes()
            .ok_or_else(|| incomplete("candidate selected schema metadata state unavailable"))?,
    )?;
    let controller_overlap = held_state_bytes
        .checked_add(retained_schema)
        .and_then(|bytes| {
            bytes.checked_add(worker_identity_path_clone_bytes(image.identity(), 2).ok()?)
        })
        .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&worker)))
        .and_then(|bytes| bytes.checked_add(controller))
        .ok_or_else(|| incomplete("candidate schema controller overlap overflow"))?;
    input
        .require_callback_state(
            controller_overlap,
            max_operation_state_bytes,
            "candidate schema controller callback state",
        )
        .map_err(owner)?;
    worker
        .set_diagnostics_v2_controller_state_cap(controller)
        .map_err(owner)?;
    Ok(worker)
}

/// Run the maintained native identity/dependency/retirement kernel after the
/// fresh catalog has streamed its rows. Like the cold admission callback, this
/// uses a separately selected full schema worker under the same image/quota;
/// the catalog worker has already finished. No source view escapes this step.
pub(crate) fn build_candidate_native_index(
    index: &mut crate::source_admission_spooled_index::IndexSink<'_>,
    records: &crate::source_admission_spooled_index::CandidateRecordsReportVerified,
    schemas: &mut tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        crate::source_admission_spooled_candidate::CandidateFence,
    >,
    limits: crate::source_admission_index::IndexLimits,
    json: tos_foundation::JsonLimits,
    json_state_bytes: usize,
    deadline: Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<usize, FoundationOrchestratorError> {
    if *schemas.input_identity() != records.fence() || schemas.is_finished() {
        return Err(incomplete(
            "native candidate callback schema binding refused",
        ));
    }
    let retained = {
        let mut schema_check = |path: &str, raw: &[u8], schema_raw: &[u8]| {
            let diagnostic = schemas
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
        index
            .build(
                records,
                limits,
                json,
                json_state_bytes,
                &mut schema_check,
                deadline,
                cancelled,
            )
            .map_err(FoundationOrchestratorError::Admission)?
    };
    schemas.finish(deadline, cancelled).map_err(owner)?;
    if !schemas.is_finished() {
        return Err(incomplete(
            "native candidate callback worker EOF incomplete",
        ));
    }
    Ok(retained)
}

/// Post-native-index custody for the borrowed candidate route. The final schema
/// worker must already have stopped on the same candidate; the record worker is
/// stopped here before either physical provider rereads its observations. This helper
/// returns payload evidence only, never native admission completion. Authored,
/// catalog, historical and full candidate raw EOF custody remain caller duties.
pub(crate) fn finish_candidate_payload_and_physical<'candidate, 'host>(
    view: &mut super::foundation_bootstrap::CandidateFoundationBootstrapView<'_, '_, '_, '_>,
    candidate: &'candidate crate::source_admission_spooled_candidate::SpoolCandidate<'host>,
    input: &crate::source_admission_candidate_records::CandidateRecordsInput<'candidate, 'host>,
    original_io: &tos_source_store::PinnedSqliteIoBudget,
    mut payloads: FoundationPayloadSources<'_>,
    terminal_schemas: &tos_validation::source_cut::CandidateCutWorkerSchemaExecutor<
        crate::source_admission_spooled_candidate::CandidateFence,
    >,
    record_executor: &mut BiblioRecordExecutor,
    worker_quota: &SharedSchemaWorkerQuota,
    held: FoundationPhaseReservation,
    remaining_write_bytes: u64,
) -> Result<super::foundation_payload::PhysicalPayloadCompletion, FoundationOrchestratorError> {
    let deadline = view.invocation.deadline();
    let cancelled = view.cancelled;
    view.remaining_budget
        .verify_invocation(view.invocation)
        .map_err(FoundationOrchestratorError::Command)?;
    let input_identity: &dyn tos_validation::record_biblio_cut::SourceCutInputWithIdentity<
        crate::source_admission_spooled_candidate::CandidateFence,
    > = input;
    if terminal_schemas.input_identity() != view.input.input_identity()
        || !std::ptr::addr_eq(view.input, input_identity)
        || !candidate.shares_io_budget(original_io)
        || !input.shares_io_budget(original_io)
        || !view.original_io.shares_with(original_io)
        || !view.physical.shared_io_budget_matches(original_io)
        || !view.physical.forwards_physical_reads_to_shared_io()
        || !terminal_schemas.is_finished()
        || !candidate.matches_invocation(deadline, cancelled)
        || payloads.deadline() != deadline
        || view.physical.deadline() != deadline
    {
        return Err(incomplete("candidate post-worker custody binding refused"));
    }
    let payload_before = payloads.cost();
    let physical_before = view.physical.cost();
    let minimum_held = payload_before
        .peak_state_bytes
        .checked_add(physical_before.retained_state_bytes)
        .ok_or_else(|| incomplete("candidate final provider state overflow"))?;
    if held.state_bytes < minimum_held {
        return Err(incomplete("candidate final provider overlap is unreserved"));
    }
    let ticket = open_window(
        view.execution_limits,
        view.remaining_budget,
        "candidate-payload-physical-eof",
        FoundationWindowKind::FinalCustodyAndOutput,
        held,
    )?;
    let free = ticket.remaining();
    let io_before = original_io.snapshot();
    let mut known_external_reads = 0u64;
    let result = (|| {
        let history_before = view.history.as_deref().map(|history| history.usage());
        let history_shared_before = view
            .history
            .as_deref()
            .filter(|history| history.shared_io_budget_matches(original_io))
            .map(|history| history.shared_runtime_read_bytes_returned());
        let history_state_limit = history_before
            .map_or(0, |(_, state)| state)
            .checked_add(free.state_bytes)
            .ok_or_else(|| incomplete("candidate final history state allowance overflow"))?;
        if let Some(history) = view.history.as_deref_mut() {
            if history
                .recheck(
                    free.source_read_bytes,
                    history_state_limit,
                    deadline,
                    cancelled,
                )
                .is_err()
            {
                let (after_read, _) = history.usage();
                let partial_read = after_read
                    .checked_sub(history_before.map_or(0, |(read, _)| read))
                    .ok_or_else(|| incomplete("candidate partial history read regressed"))?;
                let partial_shared = history
                    .shared_io_budget_matches(original_io)
                    .then(|| history.shared_runtime_read_bytes_returned())
                    .zip(history_shared_before)
                    .map_or(Some(0), |(after, before)| after.checked_sub(before))
                    .filter(|bytes| *bytes <= partial_read)
                    .ok_or_else(|| incomplete("candidate partial history shared read differs"))?;
                known_external_reads = partial_read
                    .checked_sub(partial_shared)
                    .ok_or_else(|| incomplete("candidate partial history accounting differs"))?;
                return Err(incomplete(
                    "candidate final history custody recheck refused",
                ));
            }
        }
        let history_after = view.history.as_deref().map(|history| history.usage());
        let history_read = match (history_before, history_after) {
            (Some((before, _)), Some((after, _))) => after
                .checked_sub(before)
                .ok_or_else(|| incomplete("candidate final history read cost regressed"))?,
            _ => 0,
        };
        let history_state = match (history_before, history_after) {
            (Some((_, before)), Some((_, after))) => after
                .checked_sub(before)
                .ok_or_else(|| incomplete("candidate final history state cost regressed"))?,
            _ => 0,
        };
        let history_shared = match (
            history_shared_before,
            view.history
                .as_deref()
                .filter(|history| history.shared_io_budget_matches(original_io))
                .map(|history| history.shared_runtime_read_bytes_returned()),
        ) {
            (Some(before), Some(after)) => after
                .checked_sub(before)
                .filter(|bytes| *bytes <= history_read)
                .ok_or_else(|| incomplete("candidate final history shared-read cost differs"))?,
            (None, None) => 0,
            _ => return Err(incomplete("candidate final history IO identity changed")),
        };
        known_external_reads = history_read
            .checked_sub(history_shared)
            .ok_or_else(|| incomplete("candidate final history read accounting differs"))?;
        let io_after_history = original_io.snapshot();
        let shared_after_history = io_after_history
            .read_attempted_bytes
            .checked_sub(io_before.read_attempted_bytes)
            .ok_or_else(|| incomplete("candidate final history IO counter regressed"))?;
        if history_shared > shared_after_history {
            return Err(incomplete(
                "candidate final history share exceeds original IO",
            ));
        }
        let history_used = shared_after_history
            .checked_add(known_external_reads)
            .ok_or_else(|| incomplete("candidate final history read cost overflow"))?;
        let payload_headroom = free
            .source_read_bytes
            .checked_sub(history_used)
            .ok_or_else(|| incomplete("candidate final history exceeds read reservation"))?;
        candidate
            .restrict_remaining_io(payload_headroom, remaining_write_bytes)
            .map_err(FoundationOrchestratorError::Admission)?;

        // The payload reread is bounded by its actual initial stream. Its
        // forwarded returns are charged through the shared original ledger.
        let payload_recheck_bound = payload_before.initial_bytes_read;
        let quota_before = worker_quota
            .usage()
            .map_err(|_| incomplete("schema quota unavailable"))?;
        record_executor.finish(deadline, cancelled).map_err(owner)?;
        let quota_after = worker_quota
            .usage()
            .map_err(|_| incomplete("schema quota unavailable"))?;
        let (cpu, wire) = use_delta(quota_before, quota_after)?;
        payloads
            .restrict_remaining_read_budget(payload_recheck_bound, deadline, cancelled)
            .map_err(owner)?;
        let completion = payloads.finish_with_cost().map_err(owner)?;
        let payload_read = completion
            .cost
            .total_bytes_read
            .checked_sub(payload_before.total_bytes_read)
            .ok_or_else(|| incomplete("candidate payload final read cost regressed"))?;
        let payload_shared_read = completion
            .cost
            .shared_read_bytes_returned
            .checked_sub(payload_before.shared_read_bytes_returned)
            .ok_or_else(|| incomplete("candidate payload shared-read counter regressed"))?;
        let payload_unshared_read = payload_read
            .checked_sub(payload_shared_read)
            .ok_or_else(|| incomplete("candidate payload shared-read accounting differs"))?;
        known_external_reads = known_external_reads
            .checked_add(payload_unshared_read)
            .ok_or_else(|| incomplete("candidate payload external read cost overflow"))?;
        let io_after_payload = original_io.snapshot();
        let candidate_payload_read = io_after_payload
            .read_attempted_bytes
            .checked_sub(io_before.read_attempted_bytes)
            .ok_or_else(|| incomplete("candidate final IO read counter regressed"))?;
        let physical_headroom = free
            .source_read_bytes
            .checked_sub(candidate_payload_read)
            .and_then(|bytes| bytes.checked_sub(known_external_reads))
            .and_then(|bytes| bytes.checked_sub(payload_unshared_read))
            .ok_or_else(|| incomplete("candidate payload final reads exceed reservation"))?;
        let write_headroom = remaining_write_bytes
            .checked_sub(
                io_after_payload
                    .write_attempted_bytes
                    .checked_sub(io_before.write_attempted_bytes)
                    .ok_or_else(|| incomplete("candidate final IO write counter regressed"))?,
            )
            .ok_or_else(|| incomplete("candidate payload IO exceeds write reservation"))?;
        candidate
            .restrict_remaining_io(physical_headroom, write_headroom)
            .map_err(FoundationOrchestratorError::Admission)?;
        view.physical
            .restrict_remaining_read_budget(bounded_usize(physical_headroom)?, deadline, cancelled)
            .map_err(owner)?;
        if let Err(error) = view.physical.recheck_candidate(
            view.sources,
            view.artifact_sources.as_deref_mut(),
            input,
            view.coverage,
        ) {
            let partial = view.physical.cost();
            if let (Some(read), Some(shared)) = (
                partial.bytes_read.checked_sub(physical_before.bytes_read),
                partial
                    .shared_read_bytes_returned
                    .checked_sub(physical_before.shared_read_bytes_returned),
            ) {
                if let Some(unshared) = read.checked_sub(shared) {
                    let unshared = u64::try_from(unshared)
                        .map_err(|_| incomplete("candidate partial physical read range"))?;
                    known_external_reads = known_external_reads
                        .checked_add(unshared)
                        .ok_or_else(|| incomplete("candidate partial physical read overflow"))?;
                }
            }
            return Err(owner(error));
        }
        let physical_after = view.physical.cost();
        let physical_read = physical_after
            .bytes_read
            .checked_sub(physical_before.bytes_read)
            .ok_or_else(|| incomplete("candidate physical final read cost regressed"))?;
        let physical_shared_read = physical_after
            .shared_read_bytes_returned
            .checked_sub(physical_before.shared_read_bytes_returned)
            .ok_or_else(|| incomplete("candidate physical shared-read counter regressed"))?;
        let physical_unshared_read = physical_read
            .checked_sub(physical_shared_read)
            .ok_or_else(|| incomplete("candidate physical shared-read accounting differs"))?;
        known_external_reads = known_external_reads
            .checked_add(
                u64::try_from(physical_unshared_read)
                    .map_err(|_| incomplete("candidate physical external read range"))?,
            )
            .ok_or_else(|| incomplete("candidate physical external read overflow"))?;
        let physical_state = physical_after
            .retained_state_bytes
            .checked_sub(physical_before.retained_state_bytes)
            .ok_or_else(|| incomplete("candidate physical final state cost regressed"))?;
        let io_after = original_io.snapshot();
        let candidate_read = io_after
            .read_attempted_bytes
            .checked_sub(io_before.read_attempted_bytes)
            .ok_or_else(|| incomplete("candidate final IO read counter regressed"))?;
        let remaining_write = remaining_write_bytes
            .checked_sub(
                io_after
                    .write_attempted_bytes
                    .checked_sub(io_before.write_attempted_bytes)
                    .ok_or_else(|| incomplete("candidate final IO write counter regressed"))?,
            )
            .ok_or_else(|| incomplete("candidate final IO exceeds write reservation"))?;
        let read = candidate_read
            .checked_add(known_external_reads)
            .ok_or_else(|| incomplete("candidate final provider read cost overflow"))?;
        let state = completion
            .cost
            .final_facts_state_bytes
            .checked_add(physical_state)
            .and_then(|bytes| bytes.checked_add(history_state))
            .ok_or_else(|| incomplete("candidate final provider retained cost overflow"))?;
        Ok((
            completion,
            phase_use(read, wire, state, 0, cpu, 0, 0),
            remaining_write,
        ))
    })();
    let (completion, usage, remaining_write) = match result {
        Ok(result) => result,
        Err(error) => {
            return fail_candidate_window_with_classified_io(
                view.execution_limits,
                view.remaining_budget,
                ticket,
                original_io,
                io_before,
                known_external_reads,
                view.candidate_io_adopted,
                view.remaining_write_bytes,
                error,
            );
        }
    };
    complete_candidate_window(
        view.execution_limits,
        view.remaining_budget,
        ticket,
        usage,
        io_before,
        original_io.snapshot(),
        view.candidate_io_adopted,
        view.remaining_write_bytes,
        remaining_write,
    )?;
    let remaining = view
        .remaining_budget
        .remaining()
        .map_err(FoundationOrchestratorError::Command)?;
    candidate
        .restrict_remaining_io(remaining.source_read_bytes, remaining_write)
        .map_err(FoundationOrchestratorError::Admission)?;
    // Protected controls use their maintained parser/state/read envelope and
    // their own window, after the final candidate/pager IO delta is charged.
    // The outer owner adopts these terminal spool counters without another debit.
    super::foundation_bootstrap::verify_candidate_original_epoch_with_budget(
        view.sources,
        view.original_epoch,
        view.execution_limits,
        view.remaining_budget,
    )?;
    let remaining = view
        .remaining_budget
        .remaining()
        .map_err(FoundationOrchestratorError::Command)?;
    candidate
        .restrict_remaining_io(remaining.source_read_bytes, remaining_write)
        .map_err(FoundationOrchestratorError::Admission)?;
    Ok(completion)
}

/// Run the complete candidate route over the exact borrowed Records input.
/// The returned report is the opaque report sealed by `IndexSink`'s actual
/// Records receiver; this function creates no whole-admission token.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_spooled_admission<'work, 'input, 'candidate, 'host, 'cancel, 'signal>(
    mut view: super::foundation_bootstrap::CandidateFoundationBootstrapView<
        'work,
        'input,
        'cancel,
        'signal,
    >,
    mut payloads: FoundationPayloadSources<'work>,
    input: &'input crate::source_admission_candidate_records::CandidateRecordsInput<
        'candidate,
        'host,
    >,
    candidate: &'candidate crate::source_admission_spooled_candidate::SpoolCandidate<'host>,
    selected_index: crate::source_admission_spooled_index::SpoolIndexLimits,
    native_limits: crate::source_admission_index::IndexLimits,
    defaults_limits: crate::source_admission_spooled_index::SpoolIndexLimits,
    json: tos_foundation::JsonLimits,
    json_state_bytes: usize,
    callback_state_bytes: usize,
    base_declared_state_bytes: usize,
    scope: tos_validation::source_foundation_default_rules::SourceFoundationDefaultRuleScope,
) -> Result<
    (
        crate::source_admission_spooled_index::IndexSink<'candidate>,
        crate::source_admission_spooled_index::CandidateRecordsReportVerified,
    ),
    FoundationOrchestratorError,
> {
    use super::foundation_reader::{FoundationRuleReadLimits, FoundationRuleSource};
    use crate::source_admission_spooled_candidate::CandidateFence;
    use crate::source_admission_spooled_defaults::SpoolDefaultStore;
    use crate::source_admission_spooled_index::{
        CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES, CandidateRecordsReportVerified,
        IndexSink, SpoolIndexLimits,
    };
    use std::num::NonZeroUsize;
    use tos_validation::record_biblio_cut::{SourceCutInput, SourceCutInputWithIdentity};
    use tos_validation::source_foundation_records::SourceFoundationRecordsPageBudget;

    let owner = |error| FoundationOrchestratorError::OwnerAt("candidate entry", error);
    if matches!(scope, tos_validation::source_foundation_default_rules::SourceFoundationDefaultRuleScope::SelectedRecordClosure | tos_validation::source_foundation_default_rules::SourceFoundationDefaultRuleScope::SelectedGeneratedRecordClosure)
        != input.record_selection().is_some()
    {
        return Err(owner(tos_validation::item_rules::ItemRefusal::Source(
            "candidate record selection does not match declared validation scope".into(),
        )));
    }

    let deadline = view.invocation.deadline();
    let cancelled = view.cancelled;
    let budgets = view.invocation.budgets;
    let max_members = usize::try_from(budgets.max_current_members.min(usize::MAX as u64))
        .map_err(|_| incomplete("candidate current-member limit range"))?;
    let max_member_bytes = usize::try_from(budgets.max_member_bytes.min(usize::MAX as u64))
        .map_err(|_| incomplete("candidate member limit range"))?;
    let original_operation_state = view
        .remaining_budget
        .remaining()
        .map_err(FoundationOrchestratorError::Command)?
        .state_bytes;
    let callback_state_bytes = input
        .restrict_operation_state(callback_state_bytes, original_operation_state)
        .map_err(owner)?;
    let working_ram = usize::try_from(budgets.working_ram_bytes)
        .map_err(|_| incomplete("candidate working-RAM limit range"))?;

    let input_identity: &dyn SourceCutInputWithIdentity<CandidateFence> = input;
    if !std::ptr::addr_eq(view.input, input_identity)
        || !candidate.matches_invocation(deadline, cancelled)
        || !candidate.shares_io_budget(view.original_io)
        || !input.shares_io_budget(view.original_io)
        || !payloads.shared_io_budget_matches(view.original_io)
        || !payloads.forwards_payload_reads_to_shared_io()
        || !view.physical.shared_io_budget_matches(view.original_io)
        || !view.physical.forwards_physical_reads_to_shared_io()
        || payloads.deadline() != deadline
        || view.physical.deadline() != deadline
        || view
            .remaining_budget
            .verify_invocation(view.invocation)
            .is_err()
        || view.execution_limits.deadline != deadline
    {
        return Err(incomplete(
            "candidate runner input or shared-ledger binding refused",
        ));
    }
    let fence = *input.input_identity();
    if view.input.input_identity() != &fence
        || view.coverage.membership() != fence.membership
        || native_limits.max_edges == 0
        || native_limits.max_state_bytes == 0
        || json.max_bytes == 0
        || json_state_bytes == 0
        || callback_state_bytes == 0
        || callback_state_bytes > original_operation_state
    {
        return Err(incomplete("candidate runner profile binding refused"));
    }
    let io_entry = candidate
        .io_usage()
        .map_err(FoundationOrchestratorError::Admission)?;
    if *view.candidate_io_adopted
        != (
            io_entry.read_attempted_bytes,
            io_entry.write_attempted_bytes,
            io_entry.read_upper_bound_attempted_bytes,
        )
    {
        return Err(incomplete("candidate runner IO adoption differs"));
    }
    let borrowed_base = candidate
        .borrowed_base_declared_retained_state_bytes()
        .map_err(FoundationOrchestratorError::Admission)?;
    if borrowed_base
        .0
        .checked_add(borrowed_base.1)
        .filter(|bytes| *bytes == base_declared_state_bytes)
        .is_none()
    {
        return Err(incomplete("candidate borrowed-base declaration differs"));
    }
    let candidate_owned_state = candidate
        .own_retained_state_upper_bound_bytes()
        .map_err(FoundationOrchestratorError::Admission)?;
    let payload_initial = payloads.cost();
    let physical_initial = view.physical.cost();
    let fixed_declared = candidate_owned_state
        .checked_add(base_declared_state_bytes)
        .and_then(|state| state.checked_add(selected_index.cache_bytes))
        .and_then(|state| state.checked_add(defaults_limits.cache_bytes))
        .and_then(|state| state.checked_add(payload_initial.peak_state_bytes))
        .and_then(|state| state.checked_add(physical_initial.retained_state_bytes))
        .and_then(|state| {
            state.checked_add(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)
        })
        .ok_or_else(|| incomplete("candidate whole-run declared state overflow"))?;
    if fixed_declared > working_ram {
        return Err(incomplete("candidate declared working envelope exceeded"));
    }
    let minimal_callback = std::mem::size_of::<IndexSink<'candidate>>()
        .checked_add(std::mem::size_of::<SpoolDefaultStore<'candidate, 'host>>())
        .and_then(|bytes| {
            bytes.checked_add(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)
        })
        .ok_or_else(|| incomplete("candidate callback inline state overflow"))?;
    input
        .require_callback_state(
            minimal_callback,
            original_operation_state,
            "candidate runner baseline callback state",
        )
        .map_err(owner)?;

    let mut schema_count = 0usize;
    let mut schema_bytes = 0usize;
    let mut schema_max_bytes = 0usize;
    let mut catalog_members = 0usize;
    let mut catalog_member_bytes = 0u64;
    let mut largest_member = 0usize;

    let schema_ticket = open_window(
        view.execution_limits,
        view.remaining_budget,
        "candidate-schema-resource-load",
        FoundationWindowKind::Schema,
        FoundationPhaseReservation::default(),
    )?;
    let owner = |error| FoundationOrchestratorError::OwnerAt("candidate schema", error);
    let schema_operation = schema_ticket.operation_limits();
    let worker_quota = SharedSchemaWorkerQuota::new(
        view.remaining_budget
            .remaining_worker_cpu_micros()
            .map_err(FoundationOrchestratorError::Command)?,
        schema_ticket.remaining().worker_wire_bytes,
        schema_ticket.remaining().worker_wire_bytes.max(1),
    )
    .map_err(|_| incomplete("candidate shared schema quota refused"))?;
    let io_before_schema = candidate
        .io_usage()
        .map_err(FoundationOrchestratorError::Admission)?;
    let worker_image_max = MAX_WORKER_IMAGE_BYTES;
    // Preflight the worst-case external image read before preparation. No
    // candidate IO runs until it returns. Its shared physical ceiling only
    // narrows, so do not permanently spend the unused maximum here; apply the
    // verified actual image charge below before any candidate read resumes.
    schema_ticket
        .remaining()
        .source_read_bytes
        .checked_sub(worker_image_max)
        .ok_or_else(|| incomplete("candidate worker image exceeds source-read reservation"))?;
    let remaining_write_before_schema = *view.remaining_write_bytes;
    let image_budget =
        ticket_worker_budget(&schema_ticket, deadline, budgets.worker_address_space_bytes)?;
    let worker_image = VerifiedWorkerImageHandle::prepare(
        view.execution_limits.worker.clone(),
        image_budget,
        deadline,
        cancelled,
    )
    .map_err(|_| incomplete("candidate verified worker image refused"))?;
    let worker_image_read = worker_image
        .image_bytes()
        .map_err(|_| incomplete("candidate worker image byte count unavailable"))?;
    let worker_image_state = usize::try_from(
        worker_image
            .retained_state_bytes()
            .map_err(|_| incomplete("candidate worker image state unavailable"))?,
    )
    .map_err(|_| incomplete("candidate worker image state range"))?;
    let candidate_after_image_headroom = schema_ticket
        .remaining()
        .source_read_bytes
        .checked_sub(worker_image_read)
        .ok_or_else(|| incomplete("candidate worker image exceeds source reservation"))?;
    candidate
        .restrict_remaining_io(
            candidate_after_image_headroom,
            remaining_write_before_schema,
        )
        .map_err(FoundationOrchestratorError::Admission)?;

    input
        .verify_current_fence(view.coverage, deadline, cancelled)
        .map_err(owner)?;
    input
        .for_each_current_member_meta(deadline, cancelled, &mut |member| {
            let size = usize::try_from(member.size_bytes).map_err(|_| {
                ItemRefusal::Source("candidate member size exceeds address space".into())
            })?;
            if size > max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            largest_member = largest_member.max(size);
            if member.path.starts_with("ToS/contracts/") && member.path.ends_with(".schema.json") {
                schema_count = schema_count.checked_add(1).ok_or(ItemRefusal::Budget)?;
                schema_bytes = schema_bytes.checked_add(size).ok_or(ItemRefusal::Budget)?;
                schema_max_bytes = schema_max_bytes.max(size);
            }
            if member.path.starts_with("ToS/source-witnesses/") {
                catalog_members = catalog_members.checked_add(1).ok_or(ItemRefusal::Budget)?;
                catalog_member_bytes = catalog_member_bytes
                    .checked_add(member.size_bytes)
                    .ok_or(ItemRefusal::Budget)?;
            }
            if catalog_members > max_members {
                return Err(ItemRefusal::Budget);
            }
            Ok(())
        })
        .map_err(owner)?;
    input
        .verify_current_fence(view.coverage, deadline, cancelled)
        .map_err(owner)?;
    if schema_count == 0 || schema_bytes == 0 || schema_max_bytes == 0 || largest_member == 0 {
        return Err(incomplete("candidate selected schema closure is empty"));
    }
    // Resource preparation retains a finite report. Whole streamed execution
    // instead follows the selected member bound and original operation ledgers.
    // Every completed framed exchange costs at least one wire byte. This
    // original selected finite ceiling admits cardinality without a member
    // multiplicity guess; actual wire/raw/CPU ledgers remain independently held.
    let max_checks = usize::try_from(schema_operation.worker_wire_bytes)
        .ok()
        .filter(|n| *n > 0 && *n < usize::MAX)
        .ok_or_else(|| incomplete("candidate diagnostic whole-operation count range"))?;
    let schema_loader_checks =
        max_checks.min(tos_validation::source_foundation_schema::MAX_SOURCE_FOUNDATION_CHECKS);
    let schema_limits = schema_limits_for_ticket(
        view.execution_limits,
        &schema_ticket,
        schema_count,
        schema_max_bytes,
        schema_bytes,
        schema_loader_checks,
    )?;
    let mut first_worker_shape = cut_worker_shape(schema_operation, max_checks);
    // Scalar DiagnosticsV2 executes one framed exchange per check.
    first_worker_shape.max_chunks = first_worker_shape.max_total_units;
    let first_worker_stream = view
        .execution_limits
        .cut_worker_stream_budget(schema_operation, first_worker_shape)
        .map_err(FoundationOrchestratorError::Command)?;
    let first_worker_limits = CutWorkerLimits {
        max_receipts: max_checks,
        max_receipt_bytes: schema_operation.state_bytes.min(1024 * 1024).max(1),
    };
    // These are cumulative worker response bytes, already debited to the shared
    // wire quota. They are not bytes emitted in the final CLI result. Keep the
    // CLI output reservation independent, as in the captured catalog callback.
    let first_diagnostics = CutSchemaDiagnosticsLimits {
        max_total_issues: schema_operation.issue_count.max(1),
        max_total_report_bytes: bounded_usize(schema_operation.worker_wire_bytes)?.max(1),
        max_total_state_bytes: schema_operation.state_bytes.max(1),
    };
    let image_path_state = worker_identity_path_clone_bytes(worker_image.identity(), 4)?;
    // account_spooled_candidate_inner already charged candidate_owned_state
    // before this input received the remaining callback envelope. Keep that
    // baseline in whole-process/final held-state checks, but do not reserve it
    // again inside this remainder (including the later Records/native callbacks).
    // IndexSink and SpoolDefaultStore (and their caches/report) do not exist
    // until the Records phase below. They remain in callback_held there.
    let first_worker_held = base_declared_state_bytes
        .checked_add(worker_image_state)
        .and_then(|state| state.checked_add(image_path_state))
        .and_then(|state| state.checked_add(payload_initial.peak_state_bytes))
        .and_then(|state| state.checked_add(physical_initial.retained_state_bytes))
        .ok_or_else(|| incomplete("candidate schema worker held-state overflow"))?;
    if first_worker_held
        .checked_add(candidate_owned_state)
        .is_none_or(|state| state > working_ram)
    {
        return fail_window(
            view.execution_limits,
            view.remaining_budget,
            schema_ticket,
            incomplete("candidate schema worker exceeds declared working envelope"),
        );
    }
    let mut item_schemas = match prepare_candidate_schema_worker(
        &view,
        input,
        &worker_image,
        &worker_quota,
        &schema_ticket,
        schema_limits,
        schema_operation.state_bytes,
        first_worker_limits,
        first_diagnostics,
        first_worker_stream,
        first_worker_held,
        original_operation_state,
    ) {
        Ok(worker) => worker,
        Err(error) => {
            return fail_candidate_window_with_classified_io(
                view.execution_limits,
                view.remaining_budget,
                schema_ticket,
                view.original_io,
                io_before_schema,
                worker_image_read,
                view.candidate_io_adopted,
                view.remaining_write_bytes,
                error,
            );
        }
    };
    let schema_base_state = schema_state_upper_bound(
        item_schemas.schema_bytes(),
        item_schemas.source_resource_count(),
        item_schemas
            .source_resource_metadata_state_bytes()
            .ok_or_else(|| incomplete("candidate selected schema metadata state unavailable"))?,
    )?;
    let schema_controller = item_schemas
        .diagnostics_v2_controller_state_upper_bound(schema_limits.max_instance_bytes, 4096)
        .map_err(owner)?;
    let schema_state = schema_base_state
        .checked_add(schema_controller)
        .and_then(|state| state.checked_add(std::mem::size_of_val(&item_schemas)))
        .ok_or_else(|| incomplete("candidate schema retained-state overflow"))?;
    let io_after_schema = candidate
        .io_usage()
        .map_err(FoundationOrchestratorError::Admission)?;
    let schema_read_delta = io_after_schema
        .read_attempted_bytes
        .checked_sub(io_before_schema.read_attempted_bytes)
        .ok_or_else(|| incomplete("candidate schema IO read counter regressed"))?;
    let schema_write_delta = io_after_schema
        .write_attempted_bytes
        .checked_sub(io_before_schema.write_attempted_bytes)
        .ok_or_else(|| incomplete("candidate schema IO write counter regressed"))?;
    let remaining_write = remaining_write_before_schema
        .checked_sub(schema_write_delta)
        .ok_or_else(|| incomplete("candidate schema IO exceeded write reservation"))?;
    let schema_candidate_headroom = schema_ticket
        .remaining()
        .source_read_bytes
        .checked_sub(worker_image_read)
        .and_then(|bytes| bytes.checked_sub(schema_read_delta))
        .ok_or_else(|| incomplete("candidate schema IO exceeded source reservation"))?;
    candidate
        .restrict_remaining_io(schema_candidate_headroom, remaining_write)
        .map_err(FoundationOrchestratorError::Admission)?;
    let schema_charge = checked_add_usize(
        checked_add_usize(base_declared_state_bytes, worker_image_state)?,
        checked_add_usize(schema_state, image_path_state)?,
    )?;
    complete_candidate_window(
        view.execution_limits,
        view.remaining_budget,
        schema_ticket,
        phase_use(
            checked_add_u64(worker_image_read, schema_read_delta)?,
            0,
            schema_charge,
            0,
            0,
            0,
            0,
        ),
        io_before_schema,
        io_after_schema,
        view.candidate_io_adopted,
        view.remaining_write_bytes,
        remaining_write,
    )?;
    let remaining = view
        .remaining_budget
        .remaining()
        .map_err(FoundationOrchestratorError::Command)?;
    candidate
        .restrict_remaining_io(remaining.source_read_bytes, remaining_write)
        .map_err(FoundationOrchestratorError::Admission)?;

    let records_ticket = open_window(
        view.execution_limits,
        view.remaining_budget,
        "candidate-records-artifact-bibliography-defaults",
        FoundationWindowKind::RecordsAndBibliography,
        FoundationPhaseReservation::default(),
    )?;
    let owner = |error| FoundationOrchestratorError::OwnerAt("candidate records", error);
    let records_operation = records_ticket.operation_limits();
    if records_operation.source_read_bytes == 0
        || records_operation.state_bytes == 0
        || records_operation.issue_count == 0
    {
        return fail_window(
            view.execution_limits,
            view.remaining_budget,
            records_ticket,
            incomplete("candidate Records operation reservation exhausted"),
        );
    }
    let callback_held = base_declared_state_bytes
        .checked_add(selected_index.cache_bytes)
        .and_then(|state| state.checked_add(defaults_limits.cache_bytes))
        .and_then(|state| state.checked_add(worker_image_state))
        .and_then(|state| state.checked_add(image_path_state))
        .and_then(|state| state.checked_add(schema_state))
        .and_then(|state| state.checked_add(payloads.cost().peak_state_bytes))
        .and_then(|state| state.checked_add(view.physical.cost().retained_state_bytes))
        .and_then(|state| state.checked_add(std::mem::size_of::<BiblioRecordExecutor>()))
        .and_then(|state| state.checked_add(std::mem::size_of::<IndexSink<'candidate>>()))
        .and_then(|state| {
            state.checked_add(std::mem::size_of::<SpoolDefaultStore<'candidate, 'host>>())
        })
        .ok_or_else(|| incomplete("candidate dependent callback held-state overflow"))?;
    let binding_state =
        crate::source_admission_candidate_schema::candidate_schema_binding_additional_state_bytes(
            &item_schemas,
        )
        .map_err(owner)?;
    let callback_header_state = CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES
        .checked_add(std::mem::size_of::<std::sync::Arc<()>>())
        .and_then(|bytes| bytes.checked_add(binding_state))
        .ok_or_else(|| incomplete("candidate callback report-header state overflow"))?;
    let callback_workspace_state = callback_state_bytes
        .checked_sub(callback_held)
        .and_then(|state| state.checked_sub(callback_header_state))
        .ok_or_else(|| {
            incomplete("candidate simultaneous callback state exceeds its fixed reservation")
        })?
        .min(records_operation.state_bytes);
    if callback_state_bytes > working_ram || callback_workspace_state == 0 {
        return fail_window(
            view.execution_limits,
            view.remaining_budget,
            records_ticket,
            incomplete("candidate simultaneous callback state exceeds its fixed reservation"),
        );
    }
    input
        .require_callback_state(
            callback_state_bytes,
            original_operation_state,
            "candidate Records and Claim callback state",
        )
        .map_err(owner)?;

    let callback_result_state = std::mem::size_of::<(usize, u64)>();
    let records_callback_state = callback_workspace_state
        .checked_sub(callback_result_state)
        .ok_or_else(|| incomplete("candidate callback result header exceeds reservation"))?;
    let mut records_profile = records_operation;
    records_profile.state_bytes = records_callback_state;
    let records_schema_request_state = records_callback_state.max(1);
    let records_limits = view
        .execution_limits
        .rolling_records_limits(records_profile, records_schema_request_state)
        .map_err(FoundationOrchestratorError::Command)?;
    let records_max_checks = usize::try_from(records_profile.worker_wire_bytes)
        .ok()
        .filter(|n| *n > 0 && *n < usize::MAX)
        .ok_or_else(|| incomplete("candidate Records diagnostic count range"))?;
    let mut records_shape = cut_worker_shape(records_profile, records_max_checks);
    records_shape.max_chunks = records_shape.max_total_units;
    let records_stream = view
        .execution_limits
        .cut_worker_stream_budget(records_profile, records_shape)
        .map_err(FoundationOrchestratorError::Command)?;
    let record_executor_budget = ticket_worker_budget(
        &records_ticket,
        deadline,
        budgets.worker_address_space_bytes,
    )?;
    let mut record_executor = BiblioRecordExecutor::new_with_image(
        &worker_image,
        record_executor_budget,
        FormatProfile::LegacyPythonObserved20260923,
        records_max_checks,
        deadline,
        cancelled,
    )
    .map_err(owner)?;
    let record_diagnostic_ceilings = BiblioSchemaDiagnosticsLimits {
        max_total_issues: records_profile.issue_count.max(1),
        max_total_report_bytes: bounded_usize(records_profile.worker_wire_bytes)?.max(1),
        max_total_state_bytes: records_profile.state_bytes.max(1),
    };
    let record_diagnostic_limits = BiblioSchemaDiagnosticsLimits::from_operation_ceilings(
        record_diagnostic_ceilings,
        records_max_checks,
        records_stream,
    )
    .map_err(owner)?;
    record_executor
        .set_operation_budget(records_stream)
        .map_err(owner)?;
    record_executor
        .enable_diagnostics_v2(record_diagnostic_limits)
        .map_err(owner)?;
    record_executor
        .set_shared_schema_worker_quota(worker_quota.clone())
        .map_err(owner)?;

    let stored_operation_state = callback_workspace_state.min(defaults_limits.max_row_state_bytes);
    let stored_page_state = (stored_operation_state / 3).max(1);
    let page_state = NonZeroUsize::new(stored_page_state)
        .ok_or_else(|| incomplete("candidate stored page-state ceiling is zero"))?;
    let cursor_state = NonZeroUsize::new((stored_page_state / 4).min(64 * 1024).max(1))
        .ok_or_else(|| incomplete("candidate stored cursor-state ceiling is zero"))?;
    let max_page_rows = max_members
        .min(stored_page_state / std::mem::size_of::<String>().max(1))
        .max(1);
    let page_budget = SourceFoundationRecordsPageBudget {
        max_rows: NonZeroUsize::new(max_page_rows)
            .ok_or_else(|| incomplete("candidate stored page-row ceiling is zero"))?,
        max_state_bytes: page_state,
        max_cursor_bytes: cursor_state,
    };
    let max_scan_rows = records_profile.source_read_bytes.min(u64::MAX - 1).max(1);
    let stored_limits =
        tos_validation::source_foundation_default_rules::SourceFoundationDefaultStoredLimits {
            page_budget,
            max_scan_rows,
        };
    let item_limits = view
        .execution_limits
        .item_limits(records_profile)
        .map_err(FoundationOrchestratorError::Command)?;
    let biblio_query_rows = tos_validation::biblio_rules::biblio_query_row_operation_budget(
        usize::try_from(item_limits.max_total_bytes)
            .map_err(|_| incomplete("candidate bibliography query-byte range"))?,
    )
    .map_err(owner)?;
    let event_json_cap = item_limits.max_state_bytes.min(
        usize::try_from(item_limits.max_total_bytes.min(usize::MAX as u64)).unwrap_or(usize::MAX),
    );
    if event_json_cap < 2 {
        return fail_window(
            view.execution_limits,
            view.remaining_budget,
            records_ticket,
            incomplete("candidate stored event JSON ceiling is too small"),
        );
    }

    let io_before_records = view.original_io.snapshot();
    let quota_before_records = worker_quota
        .usage()
        .map_err(|_| incomplete("candidate schema quota unavailable"))?;
    candidate
        .restrict_remaining_io(
            records_ticket.remaining().source_read_bytes,
            remaining_write,
        )
        .map_err(FoundationOrchestratorError::Admission)?;
    payloads
        .restrict_remaining_budget(
            records_ticket.remaining().source_read_bytes,
            payloads
                .cost()
                .peak_state_bytes
                .checked_add(records_ticket.remaining().state_bytes)
                .ok_or_else(|| incomplete("candidate payload allowance overflow"))?,
            deadline,
            cancelled,
        )
        .map_err(owner)?;
    let mut index = IndexSink::open(candidate, selected_index)
        .map_err(FoundationOrchestratorError::Admission)?;
    let after_index_open = candidate
        .io_usage()
        .map_err(FoundationOrchestratorError::Admission)?;
    let index_open_read = after_index_open
        .read_attempted_bytes
        .checked_sub(io_before_records.read_attempted_bytes)
        .ok_or_else(|| incomplete("candidate index-open read counter regressed"))?;
    let index_open_write = after_index_open
        .write_attempted_bytes
        .checked_sub(io_before_records.write_attempted_bytes)
        .ok_or_else(|| incomplete("candidate index-open write counter regressed"))?;
    let write_after_index = remaining_write
        .checked_sub(index_open_write)
        .ok_or_else(|| incomplete("candidate index-open write reservation exceeded"))?;
    let read_after_index = records_ticket
        .remaining()
        .source_read_bytes
        .checked_sub(index_open_read)
        .ok_or_else(|| incomplete("candidate index-open read reservation exceeded"))?;
    candidate
        .restrict_remaining_io(read_after_index, write_after_index)
        .map_err(FoundationOrchestratorError::Admission)?;
    let mut defaults = SpoolDefaultStore::open(
        candidate,
        defaults_limits,
        biblio_query_rows,
        event_json_cap,
        stored_operation_state,
    )
    .map_err(FoundationOrchestratorError::Admission)?;

    let physical_facts = view.physical.facts();
    let callback_external_reads = std::cell::Cell::new(0u64);
    let (verified_records, (dependent_state, history_rule_reads)) = match index.with_candidate_records_report(
        input,
        &mut item_schemas,
        records_limits,
        view.launch.arguments.require_local_payloads,
        cancelled,
        &mut record_executor,
        physical_facts,
        &mut payloads,
        tos_validation::record_biblio_cut::SourceCutRecordFactBudget {
            max_facts: records_profile.source_read_bytes.min(u64::MAX - 1).max(1),
            max_encoded_bytes: records_profile.source_read_bytes.min(u64::MAX - 1).max(1),
        },
        page_budget,
        callback_held,
        original_operation_state,
        records_callback_state,
        |records, verified, schemas, record_executor, payload_reader| {
            let predicates = [
                verified.fence() != fence,
                verified.record_issue_count() != 0,
                verified.item_issue_count() != 0,
                records.input_identity() != &fence,
                records.source_membership() != &fence.membership,
                records.cost().selected_current_member_bytes != fence.source_bytes,
            ];
            let mask = predicates.into_iter().enumerate().fold(0u8, |mask, (bit, failed)| {
                mask | if failed { 1u8 << bit } else { 0 }
            });
            if mask != 0 {
                let mut site = format!("ri-{mask:x}-{:x}-{:x}",
                    verified.record_issue_count(), verified.item_issue_count());
                let mut reason = crate::source_admission_spooled_index::bounded_source_cause(
                    "receiver-source", &site, "candidate Records report is not clean and bound");
                if verified.record_issue_count() != 0 || verified.item_issue_count() != 0 {
                    use tos_validation::source_foundation_records::{
                        SourceFoundationRecordsCollection, SourceFoundationRecordsStoredFact,
                    };
                    let collection = if verified.record_issue_count() != 0 {
                        SourceFoundationRecordsCollection::OrderedIssues
                    } else {
                        SourceFoundationRecordsCollection::ItemIssues
                    };
                    let page = records.index().page(collection, None,
                        SourceFoundationRecordsPageBudget {
                            max_rows: NonZeroUsize::MIN,
                            ..page_budget
                        }, deadline, cancelled)
                        .map_err(crate::source_admission_spooled_index::receiver_refusal)?;
                    let first = page.rows.first().and_then(|row| match row {
                        SourceFoundationRecordsStoredFact::OrderedIssue(issue) =>
                            Some((issue.location.as_str(), issue.message.as_str())),
                        SourceFoundationRecordsStoredFact::ItemIssue(issue) =>
                            Some((issue.path.as_str(), issue.code)),
                        _ => None,
                    });
                    if let Some((location, message)) = first {
                        let digest = Digest256::of_bytes(location.as_bytes()).to_hex();
                        let issue_site = format!("{site}-p{}", &digest[..12]);
                        if issue_site.len() <= 40 { site = issue_site; }
                        reason = crate::source_admission_spooled_index::bounded_source_cause(
                            "receiver-source", &site, message);
                    }
                }
                if verified.record_issue_count() == 0 && verified.item_issue_count() != 0 {
                    use tos_validation::source_foundation_records::{
                        SourceFoundationRecordsCollection, SourceFoundationRecordsStoredFact,
                    };
                    let cap = 192usize; // Original Item histogram envelope; collections have their own bound.
                    let expected = usize::try_from(verified.item_issue_count())
                        .map_err(|_| io::Error::other("Item issue count does not fit"))?;
                    if expected > item_limits.max_issues {
                        return Err(io::Error::other("Item issue count exceeds original issue cap"));
                    }
                    // The histogram and formatting buffers coexist with one page and
                    // its old cursor, all inside the original page-state allowance.
                    let retained = expected.checked_mul(std::mem::size_of::<(&'static str, u64)>())
                        .and_then(|n| n.checked_add(std::mem::size_of::<Vec<(&'static str, u64)>>()))
                        .and_then(|n| n.checked_add(cap.checked_mul(6)?))
                        .and_then(|n| n.checked_add(std::mem::size_of::<tos_foundation::Digest256Hasher>()))
                        .and_then(|n| n.checked_add(page_budget.max_cursor_bytes.get()))
                        .ok_or_else(|| io::Error::other("Item histogram state overflow"))?;
                    let page_state = page_budget.max_state_bytes.get().checked_sub(retained)
                        .and_then(NonZeroUsize::new)
                        .ok_or_else(|| io::Error::other("Item histogram exceeds original page state"))?;
                    let mut histogram: Vec<(&'static str, u64)> = Vec::new();
                    histogram.try_reserve_exact(expected)
                        .map_err(|_| io::Error::other("Item histogram allocation refused"))?;
                    if histogram.capacity() > expected {
                        return Err(io::Error::other("Item histogram capacity exceeds charge"));
                    }
                    let mut cursor = None;
                    let mut observed = 0usize;
                    loop {
                        let page = records.index().page(
                            SourceFoundationRecordsCollection::ItemIssues, cursor.as_ref(),
                            SourceFoundationRecordsPageBudget {
                                max_rows: NonZeroUsize::MIN,
                                max_state_bytes: page_state,
                                ..page_budget
                            }, deadline, cancelled)
                            .map_err(crate::source_admission_spooled_index::receiver_refusal)?;
                        if page.rows.is_empty() && page.next_cursor.is_some() {
                            return Err(io::Error::other("Item issue page made no progress"));
                        }
                        for row in &page.rows {
                            let SourceFoundationRecordsStoredFact::ItemIssue(issue) = row else {
                                return Err(io::Error::other("Item issue page contains another fact"));
                            };
                            observed = observed.checked_add(1)
                                .filter(|n| *n <= expected)
                                .ok_or_else(|| io::Error::other("Item issue page count drift"))?;
                            if issue.code.is_empty() || issue.code.len() > cap ||
                                !issue.code.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
                                return Err(io::Error::other("Item issue code is not an owned public code"));
                            }
                            if let Some((_, count)) = histogram.iter_mut().find(|(code, _)| *code == issue.code) {
                                *count += 1;
                            } else { histogram.push((issue.code, 1)); }
                        }
                        cursor = page.next_cursor;
                        if cursor.is_none() { break; }
                    }
                    if observed != expected {
                        return Err(io::Error::other("Item issue EOF count drift"));
                    }
                    histogram.sort_unstable_by_key(|(code, _)| *code);
                    let mut hash = tos_foundation::Digest256Hasher::new();
                    for (code, count) in &histogram {
                        hash.update(code.as_bytes()); hash.update(&[0]); hash.update(&count.to_le_bytes());
                    }
                    let digest = hash.finalize().to_hex();
                    let mut summary = format!("{site} items={observed} codes={}", histogram.len());
                    let complete_bytes = histogram.iter().try_fold(summary.len(), |n, (code, count)| {
                        n.checked_add(code.len() + 2 + count.to_string().len())
                    }).ok_or_else(|| io::Error::other("Item histogram output overflow"))?;
                    if complete_bytes <= cap {
                        for (code, count) in &histogram {
                            use std::fmt::Write;
                            write!(&mut summary, " {code}={count}").map_err(io::Error::other)?;
                        }
                    } else {
                        // Full ordered histogram identity survives even when the
                        // finite public packet cannot carry every textual code.
                        summary.push_str(&format!(" hash={digest}"));
                        let mut shown = 0usize;
                        for (code, count) in &histogram {
                            let entry = format!(" {code}={count}");
                            let suffix = format!(" omitted={}", histogram.len() - shown);
                            if summary.len() + entry.len() + suffix.len() > cap { break; }
                            summary.push_str(&entry); shown += 1;
                        }
                        summary.push_str(&format!(" omitted={}", histogram.len() - shown));
                    }
                    if summary.len() > cap {
                        return Err(io::Error::other("Item histogram exceeds public reason cap"));
                    }
                    return Err(io::Error::other(crate::source_command::SourceCommandError::DeniedWithReason(summary)));
                }
                return Err(io::Error::other(reason));
            }
            let after_records = view.original_io.snapshot();
            let read_used = after_records
                .read_attempted_bytes
                .checked_sub(io_before_records.read_attempted_bytes)
                .ok_or_else(|| io::Error::other("candidate Records read counter regressed"))?;
            let write_used = after_records
                .write_attempted_bytes
                .checked_sub(io_before_records.write_attempted_bytes)
                .ok_or_else(|| io::Error::other("candidate Records write counter regressed"))?;
            let current_read = records_ticket
                .remaining()
                .source_read_bytes
                .checked_sub(read_used)
                .ok_or_else(|| io::Error::other("candidate Records read budget exceeded"))?;
            let current_write = remaining_write
                .checked_sub(write_used)
                .ok_or_else(|| io::Error::other("candidate Records write budget exceeded"))?;
            candidate
                .restrict_remaining_io(current_read, current_write)
                .map_err(|_| io::Error::other("candidate Records IO narrowing refused"))?;

            let artifact_limits = view
                .execution_limits
                .item_limits(FoundationPhaseReservation {
                    source_read_bytes: current_read,
                    worker_wire_bytes: records_ticket.remaining().worker_wire_bytes,
                    state_bytes: callback_workspace_state,
                    issue_count: records_ticket.remaining().issue_count,
                    output_bytes: records_ticket.remaining().output_bytes,
                    worker_cpu_micros: records_ticket.remaining().worker_cpu_micros,
                    worker_cpu_seconds: records_ticket.remaining().worker_cpu_seconds,
                    tmpfs_bytes: records_ticket.remaining().tmpfs_bytes,
                    tmpfs_inodes: records_ticket.remaining().tmpfs_inodes,
                })
                .map_err(|_| io::Error::other("candidate Artifact limits refused"))?;
            let source_root = view
                .selected_roots
                .repo_root
                .join("ToS/source-witnesses");
            let history_usage_before = view.history.as_deref().map(|history| history.usage());
            let history_shared_before = view
                .history
                .as_deref()
                .filter(|history| history.shared_io_budget_matches(view.original_io))
                .map(|history| history.shared_runtime_read_bytes_returned());
            let schema_worker = RefCell::new(schemas);
            let replay_result = super::foundation_artifact_replay::prepare_candidate_artifact_replay(
                input,
                view.coverage,
                records,
                &source_root,
                u64::from(view.invocation.uid()),
                &schema_worker,
                worker_quota.clone(),
                artifact_limits,
                page_budget,
                max_scan_rows.min(usize::MAX as u64) as usize,
                max_checks,
                cancelled,
            );
            let mut replay = match replay_result {
                Ok(replay) => replay,
                Err(error) => {
                    let history_after = view.history.as_deref().map(|history| history.usage());
                    let history_read = match (history_usage_before, history_after) {
                        (Some((before, _)), Some((after, _))) =>
                            after.checked_sub(before).unwrap_or(0),
                        _ => 0,
                    };
                    let history_shared = match (
                        history_shared_before,
                        view.history
                            .as_deref()
                            .filter(|history| history.shared_io_budget_matches(view.original_io))
                            .map(|history| history.shared_runtime_read_bytes_returned()),
                    ) {
                        (Some(before), Some(after)) => after.checked_sub(before).unwrap_or(0),
                        _ => 0,
                    };
                    let owner_reads = error
                        .cost
                        .native_history_source_read_bytes
                        .checked_add(error.cost.readonly.read_bytes);
                    if let Some(external) = owner_reads
                        .and_then(|reads| reads.checked_sub(history_shared.min(history_read)))
                    {
                        callback_external_reads.set(external);
                    }
                    return Err(io::Error::other(
                        crate::source_admission_spooled_index::bounded_source_cause(
                            "receiver-source",
                            "artifact-replay",
                            &format!("{:?}:{:?}", error.stage, error.class),
                        ),
                    ));
                }
            };
            let replay_cost = replay.cost();
            let mut replay_state = replay_cost
                .retained_state_upper_bound_bytes()
                .ok_or_else(|| io::Error::other("candidate Artifact retained cost overflow"))?;
            let replay_external_reads = replay_cost
                .native_history_source_read_bytes
                .checked_add(replay_cost.readonly.read_bytes)
                .ok_or_else(|| io::Error::other("candidate Artifact read cost overflow"))?;
            let after_replay = view.original_io.snapshot();
            let candidate_used = after_replay
                .read_attempted_bytes
                .checked_sub(io_before_records.read_attempted_bytes)
                .ok_or_else(|| io::Error::other("candidate Artifact read counter regressed"))?;
            let history_usage_after = view.history.as_deref().map(|history| history.usage());
            let history_read = match (history_usage_before, history_usage_after) {
                (Some((before, _)), Some((after, _))) => after
                    .checked_sub(before)
                    .ok_or_else(|| io::Error::other("candidate Artifact history cost regressed"))?,
                _ => 0,
            };
            let history_shared = match (
                history_shared_before,
                view.history
                    .as_deref()
                    .filter(|history| history.shared_io_budget_matches(view.original_io))
                    .map(|history| history.shared_runtime_read_bytes_returned()),
            ) {
                (Some(before), Some(after)) => after
                    .checked_sub(before)
                    .filter(|bytes| *bytes <= history_read)
                    .ok_or_else(|| io::Error::other("candidate Artifact history share differs"))?,
                (None, None) => 0,
                _ => return Err(io::Error::other("candidate Artifact history IO identity changed")),
            };
            if history_read != replay_cost.native_history_source_read_bytes
                || replay_cost.candidate_source_read_bytes > candidate_used
                || history_shared > candidate_used
            {
                return Err(io::Error::other("candidate Artifact history cost differs"));
            }
            let artifact_external_reads = replay_external_reads
                .checked_sub(history_shared)
                .ok_or_else(|| io::Error::other("candidate Artifact shared-read accounting differs"))?;
            callback_external_reads.set(artifact_external_reads);
            let after_replay_headroom = records_ticket
                .remaining()
                .source_read_bytes
                .checked_sub(candidate_used)
                .and_then(|bytes| bytes.checked_sub(artifact_external_reads))
                .ok_or_else(|| io::Error::other("candidate Artifact source budget exceeded"))?;
            candidate
                .restrict_remaining_io(after_replay_headroom, current_write)
                .map_err(|_| io::Error::other("candidate Artifact IO narrowing refused"))?;
            let biblio_operation = FoundationPhaseReservation {
                source_read_bytes: after_replay_headroom,
                worker_wire_bytes: records_ticket.remaining().worker_wire_bytes,
                state_bytes: callback_workspace_state.saturating_sub(replay_state),
                issue_count: records_ticket.remaining().issue_count,
                output_bytes: records_ticket.remaining().output_bytes,
                worker_cpu_micros: records_ticket.remaining().worker_cpu_micros,
                worker_cpu_seconds: records_ticket.remaining().worker_cpu_seconds,
                tmpfs_bytes: records_ticket.remaining().tmpfs_bytes,
                tmpfs_inodes: records_ticket.remaining().tmpfs_inodes,
            };
            let biblio_limits = view
                .execution_limits
                .item_limits(biblio_operation)
                .map_err(|_| io::Error::other("candidate Biblio limits refused"))?;
            let default_event_json_bytes = event_json_cap
                .min(biblio_limits.max_state_bytes)
                .min(usize::try_from(biblio_limits.max_total_bytes.min(usize::MAX as u64)).unwrap_or(usize::MAX));
            if default_event_json_bytes < 2 {
                return Err(io::Error::other("candidate Biblio event ceiling is too small"));
            }
            defaults
                .fold_record_events(
                    records,
                    stored_limits,
                    default_event_json_bytes,
                    stored_operation_state,
                    deadline,
                    cancelled,
                )
                .map_err(|error| candidate_callback_refusal("candidate default record event fold", error))?;
            let biblio_query_rows = tos_validation::biblio_rules::biblio_query_row_operation_budget(
                usize::try_from(biblio_limits.max_total_bytes)
                    .map_err(|_| io::Error::other("candidate Biblio query cap range"))?,
            )
            .map_err(|_| io::Error::other("candidate Biblio query budget refused"))?;
            let providers_result = defaults.with_providers(
                records,
                input,
                stored_limits,
                stored_operation_state,
                deadline,
                cancelled,
                |records_lookup,
                 paths,
                 default_events,
                 closure_links,
                 closure_schema_requests,
                 discovery_seen_ids,
                 discovery_run_summaries,
                 discovery_event_summaries,
                 discovery_schema_requests,
                 discovery_digest_cache,
                 biblio_sink| {
                    let mut biblio_schema_worker = schema_worker.borrow_mut();
                    let biblio_report = tos_validation::biblio_rules::inspect_bibliography_from_input_stored(
                        input,
                        view.coverage,
                        records,
                        records_lookup,
                        default_events.event_lookup(),
                        biblio_sink,
                        &mut **biblio_schema_worker,
                        biblio_limits,
                        cancelled,
                    ).map_err(|error| candidate_owner_refusal("candidate bibliography receiver", error))?;
                    drop(biblio_schema_worker);
                    let biblio_failures = [
                        biblio_report.input_identity() != &fence,
                        biblio_report.source_membership() != fence.membership,
                        !biblio_report.owner_predicates_complete(),
                        biblio_report.shadow().issue_sink_truncated,
                        !biblio_report.shadow().issues.is_empty(),
                    ];
                    let biblio_mask = biblio_failures.into_iter().enumerate()
                        .fold(0u8, |mask, (bit, failed)| mask | if failed { 1u8 << bit } else { 0 });
                    if biblio_mask != 0 {
                        // Preserve the failed predicates and first owner issue as
                        // bounded fingerprints; source locators stay private.
                        let issues = &biblio_report.shadow().issues;
                        let mut site = format!("bi-{biblio_mask:x}-{:x}", issues.len());
                        let mut cause = "candidate Biblio owner predicates are incomplete or invalid";
                        if let Some(issue) = issues.first() {
                            let location = Digest256::of_bytes(issue.location.as_bytes()).to_hex();
                            site.push_str(&format!("-p{}", &location[..12]));
                            cause = issue.code;
                        }
                        return Err(ItemRefusal::Source(
                            crate::source_admission_spooled_index::bounded_source_cause(
                                "receiver-source", &site, cause),
                        ));
                    }
                    let claims: &dyn tos_validation::source_foundation_default_rules::SourceFoundationDefaultClaims =
                        biblio_sink;
                    let biblio_state = biblio_report.accounted_state_upper_bound_bytes();
                    let available_after_biblio = biblio_operation
                        .state_bytes
                        .checked_sub(biblio_state)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let reader_member_bytes = max_member_bytes
                        .min(after_replay_headroom as usize)
                        .max(1)
                        .min(usize::try_from(biblio_limits.max_member_bytes)
                            .map_err(|_| tos_validation::item_budget_origin!())?);
                    // The same callback retains the Records header while the
                    // rule reader owns its header, copied member and auxiliary
                    // map. Project those simultaneous allocations from the one
                    // remaining state reservation before assigning the map cap.
                    let reader_retained_state = callback_held
                        .checked_add(callback_header_state)
                        .and_then(|state| state.checked_add(replay_state))
                        .and_then(|state| state.checked_add(biblio_state))
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let reader_auxiliary_state_bytes = available_after_biblio
                        .checked_sub(std::mem::size_of::<FoundationRuleSource<'_, '_>>())
                        .and_then(|state| state.checked_sub(reader_member_bytes))
                        .filter(|state| *state != 0)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let reader_limits = FoundationRuleReadLimits {
                        max_member_bytes: reader_member_bytes,
                        max_read_bytes: after_replay_headroom,
                        max_auxiliary_paths: max_members
                            .min(reader_auxiliary_state_bytes
                                / std::mem::size_of::<(String, Option<(Digest256, u64)>)>().max(1))
                            .max(1),
                        max_auxiliary_state_bytes: reader_auxiliary_state_bytes,
                        deadline,
                    };
                    let history_usage_before_rules =
                        view.history.as_deref().map(|history| history.usage());
                    let history_shared_before_rules = view
                        .history
                        .as_deref()
                        .filter(|history| history.shared_io_budget_matches(view.original_io))
                        .map(|history| history.shared_runtime_read_bytes_returned());
                    let worker_usage_before_rules = worker_quota
                        .usage()
                        .map_err(|_| tos_validation::item_budget_origin!())?;
                    let reader_io_before = view.original_io.snapshot();
                    if let Some(selection) = input.record_selection() {
                        let mut checked_claims = 0usize;
                        for member in selection.members().filter(|member|
                            selection.file_slots(&member.source_ref).iter().any(|slot| slot.kind == "claim"))
                        {
                            input.with_current_member(&member.source_ref, reader_member_bytes, deadline, cancelled,
                                &mut |meta, raw| {
                                    if meta.path != member.source_ref || meta.size_bytes != raw.len() as u64 {
                                        return Err(ItemRefusal::Source("selected Claim member custody differs".into()));
                                    }
                                    let verification_state = selection.file_slots(&member.source_ref).iter()
                                        .try_fold(0usize, |peak, slot| Ok::<_, ItemRefusal>(peak.max(slot.verification_state_upper_bound()?)))?;
                                    let held = reader_retained_state.checked_add(raw.len())
                                        .and_then(|state| state.checked_add(verification_state))
                                        .ok_or(tos_validation::item_budget_origin!())?;
                                    callback_state_bytes.checked_sub(held)
                                        .filter(|state| *state != 0).ok_or(tos_validation::item_budget_origin!())?;
                                    input.require_callback_state(callback_state_bytes, original_operation_state, "candidate Records and Claim callback state")?;
                                    let verified = selection.verify_file(&member.source_ref, raw, deadline, cancelled)?;
                                    let mut rows = verified.row_cursor();
                                    while let Some(row) = rows.next_checked(deadline, cancelled) {
                                        let (line, _bytes, slot) = row?;
                                        if slot.kind != "claim" { continue; }
                                        require_completed_biblio_claim(
                                            claims, &member.source_ref,
                                            usize::try_from(line).map_err(|_| tos_validation::item_budget_origin!())?,
                                            Digest256::from_hex(&slot.source.file_sha256)
                                                .map_err(|_| ItemRefusal::Source("selected Claim file digest invalid".into()))?,
                                            Some(slot.identity.as_str()),
                                        )?;
                                        checked_claims = checked_claims.checked_add(1)
                                            .ok_or(tos_validation::item_budget_origin!())?;
                                    }
                                    Ok(())
                                })?;
                        }
                        if checked_claims != selection.slots().filter(|slot| slot.kind == "claim").count() {
                            return Err(ItemRefusal::Source("selected Claim local owner coverage did not reach EOF".into()));
                        }
                    }
                    if let Some(generated) = input.generated_selection() {
                        // Verify generated geometry against the completed Biblio index,
                        // retaining physical traversal EOF and exact file/line binding.
                        let generated_coverage = input.for_each_current_member(deadline, cancelled,
                            &mut |meta, raw| {
                                if !generated.selects_claim_row(meta.path, 1)? { return Ok(()); }
                                let held = reader_retained_state.checked_add(raw.len())
                                    .ok_or(tos_validation::item_budget_origin!())?;
                                callback_state_bytes.checked_sub(held)
                                    .filter(|state| *state != 0).ok_or(tos_validation::item_budget_origin!())?;
                                input.require_callback_state(callback_state_bytes, original_operation_state, "candidate Records and Claim callback state")?;
                                let file_sha256 = Digest256::of_bytes(raw);
                                let mut checked_rows = 0u64;
                                for (line, _bytes) in tos_validation::source_record_selection::source_rows(raw) {
                                    if !generated.selects_claim_row(meta.path, line)? { continue; }
                                    require_completed_biblio_claim(
                                        claims, meta.path,
                                        usize::try_from(line).map_err(|_| tos_validation::item_budget_origin!())?,
                                        file_sha256, None,
                                    )?;
                                    checked_rows = checked_rows.checked_add(1)
                                        .ok_or(tos_validation::item_budget_origin!())?;
                                }
                                if checked_rows != 1 {
                                    return Err(ItemRefusal::Source("generated Claim local owner row coverage differs".into()));
                                }
                                Ok(())
                            })?;
                        if generated_coverage.membership() != fence.membership
                            || generated_coverage.member_count() != fence.membership.count
                            || generated_coverage.source_bytes_read() != fence.source_bytes
                        {
                            return Err(ItemRefusal::Source("generated Claim physical owner traversal is incomplete".into()));
                        }
                    }
                    let mut schema_executor =
                        CandidateArtifactSchemaExecutor::new(&schema_worker);
                    let mut rule_source = FoundationRuleSource::from_candidate(
                        input,
                        view.original_io,
                        view.sources,
                        &mut schema_executor,
                        payload_reader,
                        cancelled,
                        reader_limits,
                        reader_retained_state,
                        callback_state_bytes,
                        original_operation_state,
                    ).map_err(|error| candidate_owner_refusal("candidate default source preparation", error))?;
                    if let Some(history) = view.history.as_deref_mut() {
                        rule_source = rule_source.with_history(history)
                            .map_err(|error| candidate_owner_refusal("candidate default history binding", error))?;
                    }
                    let event_state_cap = biblio_operation
                        .state_bytes
                        .min(biblio_operation.output_bytes);
                    let default_rules_limits =
                        tos_validation::source_foundation_default_rules::SourceFoundationDefaultRulesLimits {
                            operation: {
                                let mut operation = biblio_limits;
                                operation.max_state_bytes = available_after_biblio;
                                operation.max_total_bytes = after_replay_headroom;
                                operation
                            },
                            max_event_map_bytes: event_state_cap.min(available_after_biblio),
                        };
                    let stored_report = tos_validation::source_foundation_default_rules::inspect_source_foundation_default_rules_from_input_stored_with_artifact_evidence_provider_and_seen_ids_and_run_summaries_and_event_summaries_and_schema_requests_and_digests_and_closure_links_and_closure_schema_requests(
                        &mut rule_source,
                        input,
                        view.coverage,
                        records,
                        records_lookup,
                        paths,
                        default_events,
                        claims,
                        physical_facts,
                        &mut replay,
                        discovery_seen_ids,
                        discovery_run_summaries,
                        discovery_event_summaries,
                        discovery_schema_requests,
                        discovery_digest_cache,
                        closure_links,
                        closure_schema_requests,
                        view.launch.arguments.require_local_payloads,
                        default_rules_limits,
                        stored_limits,
                        cancelled,
                        scope,
                    ).map_err(|error| candidate_owner_refusal("candidate default rules receiver", error))?;
                    let replay_cost_after_rules = replay.cost();
                    let evidence_peak_state = replay_cost_after_rules
                        .candidate_artifact_evidence_peak_state_bytes
                        .max(replay_cost_after_rules.candidate_record_index_state_bytes)
                        .max(
                            stored_report
                                .discovery
                                .cost
                                .candidate_artifact_evidence_peak_state_bytes,
                        )
                        .max(
                            stored_report
                                .discovery
                                .cost
                                .candidate_discovery_seen_ids_peak_workspace_state_bytes,
                        );
                    let evidence_peak_state = evidence_peak_state.max(
                        stored_report
                            .discovery
                            .cost
                            .candidate_discovery_run_summary_peak_workspace_state_bytes,
                    );
                    let evidence_peak_state = evidence_peak_state.max(
                        stored_report
                            .discovery
                            .cost
                            .candidate_discovery_event_summary_peak_workspace_state_bytes,
                    );
                    replay_state = replay_cost_after_rules
                        .retained_state_upper_bound_bytes()
                        .and_then(|state| state.checked_add(evidence_peak_state))
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let replay_had_skips = replay.has_skips();
                    let worker_usage_after_discovery = worker_quota
                        .usage()
                        .map_err(|_| tos_validation::item_budget_origin!())?;
                    let replay_worker_cpu = worker_usage_after_discovery
                        .worker_cpu_micros
                        .checked_sub(worker_usage_before_rules.worker_cpu_micros)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let replay_worker_wire = worker_usage_after_discovery
                        .worker_wire_bytes
                        .checked_sub(worker_usage_before_rules.worker_wire_bytes)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let replay_worker_units = worker_usage_after_discovery
                        .worker_units
                        .checked_sub(worker_usage_before_rules.worker_units)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    if replay_cost_after_rules.candidate_replay_worker_cpu_micros
                        > replay_worker_cpu
                        || replay_cost_after_rules.candidate_replay_worker_wire_bytes
                            > replay_worker_wire
                        || replay_cost_after_rules.candidate_replay_worker_units
                            > replay_worker_units
                    {
                        return Err(tos_validation::item_budget_origin!());
                    }
                    rule_source.recheck_auxiliary()
                        .map_err(|error| candidate_owner_refusal("candidate default auxiliary recheck", error))?;
                    let reader_cost = rule_source.cost();
                    drop(rule_source);
                    let reader_io_after = view.original_io.snapshot();
                    let history_usage_after_rules =
                        view.history.as_deref().map(|history| history.usage());
                    let history_rule_read = match (
                        history_usage_before_rules,
                        history_usage_after_rules,
                    ) {
                        (Some((before, _)), Some((after, _))) => {
                            after.checked_sub(before).ok_or(tos_validation::item_budget_origin!())?
                        }
                        _ => 0,
                    };
                    let history_shared_rule_read = match (
                        history_shared_before_rules,
                        view.history
                            .as_deref()
                            .filter(|history| history.shared_io_budget_matches(view.original_io))
                            .map(|history| history.shared_runtime_read_bytes_returned()),
                    ) {
                        (Some(before), Some(after)) => after
                            .checked_sub(before)
                            .filter(|bytes| *bytes <= history_rule_read)
                            .ok_or(tos_validation::item_budget_origin!())?,
                        (None, None) => 0,
                        _ => return Err(tos_validation::item_budget_origin!()),
                    };
                    let reader_shared_attempts = reader_io_after
                        .read_attempted_bytes
                        .checked_sub(reader_io_before.read_attempted_bytes)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let reader_external_reads = reader_cost
                        .bytes_read
                        .checked_sub(reader_cost.shared_read_bytes_returned)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    if history_rule_read
                        > reader_cost
                            .bytes_read
                            .checked_add(replay_cost_after_rules.native_history_source_read_bytes)
                            .ok_or(tos_validation::item_budget_origin!())?
                        || reader_cost.shared_read_bytes_returned > reader_shared_attempts
                    {
                        return Err(tos_validation::item_budget_origin!());
                    }
                    let provider_shared_history = history_shared_rule_read
                        .saturating_sub(reader_cost.shared_read_bytes_returned);
                    let replay_external_reads_after_rules = replay_cost_after_rules
                        .native_history_source_read_bytes
                        .checked_add(replay_cost_after_rules.readonly.read_bytes)
                        .and_then(|reads| reads.checked_sub(provider_shared_history))
                        .ok_or(tos_validation::item_budget_origin!())?;
                    callback_external_reads.set(
                        replay_external_reads_after_rules
                            .checked_add(reader_external_reads)
                            .ok_or(tos_validation::item_budget_origin!())?,
                    );
                    drop(schema_executor);
                    drop(replay);
                    let mut schemas = schema_worker.borrow_mut();
                    let owner_state = stored_report.cost.aggregate_state_reservation_bytes;
                    // The source reader, replay and replay schema adapter have
                    // ended above. Their peak remains evidence for the rules
                    // phase; those allocations do not coexist with diagnostics.
                    let common_evidence_state = biblio_state
                        .checked_add(owner_state)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let rules_evidence_peak = common_evidence_state
                        .checked_add(replay_state)
                        .and_then(|state| state.checked_add(reader_cost.auxiliary_state_bytes))
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let diagnostic_state_cap = callback_workspace_state
                        .checked_sub(common_evidence_state)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    if replay_had_skips {
                        return Err(ItemRefusal::Source(
                            "candidate Artifact evidence is incomplete".into(),
                        ));
                    }
                    let rule_diag_limits = SourceFoundationRuleDiagnosticsLimits {
                        max_issues: biblio_operation.issue_count.max(1),
                        max_output_bytes: biblio_operation.output_bytes.max(1),
                        max_state_bytes: diagnostic_state_cap.max(1),
                    };
                    let evaluated = super::foundation_rule_diagnostics::evaluate_candidate_stored_rules(
                        stored_report,
                        records,
                        page_budget,
                        discovery_schema_requests,
                        closure_schema_requests,
                        &mut **schemas,
                        schema_limits,
                        super::foundation_rule_diagnostics::CandidateRuleDiagnosticOperationLimits {
                            max_checks,
                            max_total_instance_bytes: usize::try_from(first_worker_stream.max_total_raw_bytes)
                                .map_err(|_| ItemRefusal::Budget)?,
                        },
                        deadline,
                        cancelled,
                        rule_diag_limits,
                    )
                    .map_err(|error| {
                        let (site, reason) = match error {
                            CandidateRuleDiagnosticsError::Refused { reason, .. } =>
                                ("default-refused", reason.to_owned()),
                            CandidateRuleDiagnosticsError::Incomplete { reason, .. } =>
                                ("default-incomplete", reason.to_owned()),
                        };
                        ItemRefusal::Source(
                            crate::source_admission_spooled_index::bounded_source_cause(
                                "receiver-source", site, &reason,
                            ),
                        )
                    })?;
                    let owner_report = &evaluated.owner_report;
                    if owner_report.scope != scope {
                        return Err(ItemRefusal::Source("candidate default profile binding differs".into()));
                    }

                    // Preserve every failed owned predicate, without printing private
                    // source paths, issue prose, or worker payloads. The first failed
                    // predicate carries exact observed/expected scalar counts.
                    let predicates = [
                        ("default direct owner issues", owner_report.cost.direct_owner_issue_count, 0),
                        ("default aggregate Labs coverage", owner_report.labs.as_ref().map_or(0, |labs| labs.unimplemented.len()), 0),
                        ("default per-lab coverage", owner_report.labs.as_ref().map_or(0, |labs| labs.results.iter().filter(|lab| !lab.unimplemented.is_empty()).count()), 0),
                        ("default Goldset coverage", owner_report.goldsets.as_ref().map_or(0, |goldsets| goldsets.coverage_gaps.len()), 0),
                        ("default Discovery coverage", owner_report.discovery.unsupported.len(), 0),
                        ("default Closure coverage", owner_report.closure.unsupported.len(), 0),
                        ("default queued execution count", evaluated.cost.schema_check_count, owner_report.cost.queued_schema_document_count),
                        ("default resolved schema issues", evaluated.semantic_failure_count().ok_or(ItemRefusal::Budget)?, 0),
                        ("default diagnostic live binding", usize::from(!evaluated.diagnostics_bound_to(&**schemas, scope)), 0),
                    ];
                    let mut failed_mask = 0u16;
                    let mut primary = None;
                    for (index, (label, observed, expected)) in predicates.into_iter().enumerate() {
                        if observed != expected {
                            failed_mask |= 1u16 << index;
                            primary.get_or_insert((label, observed, expected));
                        }
                    }
                    if let Some((label, observed, expected)) = primary {
                        let issue = owner_report.labs.as_ref().and_then(|labs| labs.ordered_issues.first()).map(|(_, text)| text.as_str())
                            .or_else(|| owner_report.goldsets.as_ref().and_then(|goldsets| goldsets.ordered_issues.first()).map(|(_, text)| text.as_str()))
                            .or_else(|| owner_report.discovery.issues.first().map(|issue| issue.detail.as_str()))
                            .or_else(|| owner_report.closure.issues.first().map(|(_, text)| text.as_str()))
                            .unwrap_or(label);
                        // 11 predicate bits and two 64-bit hex counters fit the
                        // existing 40-byte source-cause site bound exactly.
                        let mut site = format!("pr-{failed_mask:x}-{observed:x}-{expected:x}");
                        let mut cause = issue;
                        if owner_report.labs.as_ref().is_none_or(|labs| labs.ordered_issues.is_empty())
                            && owner_report.goldsets.as_ref().is_none_or(|goldsets| goldsets.ordered_issues.is_empty())
                        {
                            let finding = owner_report.discovery.issues.first()
                                .map(|finding| ("d", finding.code, finding.location.as_str()))
                                .or_else(|| owner_report.closure.issues.first()
                                    .map(|(path, _)| ("c", "closure", path.as_str())));
                            if let Some((district, code, path)) = finding {
                                let code = Digest256::of_bytes(code.as_bytes()).to_hex();
                                let path = Digest256::of_bytes(path.as_bytes()).to_hex();
                                let located = format!("{site}-{district}{}-p{}", &code[..12], &path[..12]);
                                if located.len() <= 40 {
                                    site = located;
                                }
                            }
                        }
                        if let Some(diagnostic) = evaluated.first_invalid()
                        {
                            let result = diagnostic.result();
                            let contract = Digest256::of_bytes(result.contract().as_bytes()).to_hex();
                            let reason = result.report().issues.first()
                                .map_or(0, |issue| issue.reason as u16);
                            let diagnostic_site = format!(
                                "{site}-s{:x}-r{reason:x}-c{}",
                                result.status() as u8, &contract[..12],
                            );
                            // The full source-path digest remains the cause, while
                            // this bounded navigation prefix names the selected
                            // contract and its owned structured reason code.
                            if diagnostic_site.len() <= 40 {
                                site = diagnostic_site;
                                cause = result.path();
                            }
                        }
                        // Return the bounded set from this same evaluation, not
                        // just its first finding. Fingerprints preserve source
                        // privacy while allowing the owner to correlate every
                        // path and reason against the selected local inputs.
                        use crate::source_admission_spooled_index::{
                            MAX_SOURCE_CAUSE_BYTES, MAX_SOURCE_CAUSES, bounded_source_cause,
                        };
                        let summary = bounded_source_cause("receiver-source", &site, cause);
                        let diagnostic_cap = MAX_SOURCE_CAUSE_BYTES.min(biblio_operation.output_bytes);
                        let diagnostic_state = evaluated.cost.peak_additional_state_upper_bound_bytes
                            .checked_add(diagnostic_cap + 1024)
                            .ok_or(tos_validation::item_budget_origin!())?;
                        if diagnostic_state > diagnostic_state_cap
                            || summary.len() + "source-causes:".len() > diagnostic_cap {
                            return Err(ItemRefusal::Source(summary));
                        }
                        let mut causes = String::with_capacity(diagnostic_cap);
                        causes.push_str("source-causes:");
                        causes.push_str(&summary);
                        let findings = owner_report.labs.iter()
                            .flat_map(|labs| labs.ordered_issues.iter().map(|(path, issue)| ("l", path.as_str(), issue.as_str())))
                            .chain(owner_report.goldsets.iter()
                                .flat_map(|goldsets| goldsets.ordered_issues.iter().map(|(path, issue)| ("g", path.as_str(), issue.as_str()))))
                            .chain(owner_report.discovery.issues.iter()
                                .map(|issue| ("d", issue.location.as_str(), issue.detail.as_str())))
                            .chain(owner_report.closure.issues.iter()
                                .map(|(path, issue)| ("c", path.as_str(), issue.as_str())));
                        for (district, path, issue) in findings.take(MAX_SOURCE_CAUSES - 1) {
                            let prefix = issue.split_once(':').map_or(issue, |(prefix, _)| prefix);
                            let prefix = Digest256::of_bytes(prefix.as_bytes()).to_hex();
                            let path = Digest256::of_bytes(path.as_bytes()).to_hex();
                            let site = format!("df-{district}-{}-p{}", &prefix[..12], &path[..12]);
                            let token = bounded_source_cause("receiver-source", &site, issue);
                            if causes.len().checked_add(1 + token.len())
                                .is_none_or(|bytes| bytes > diagnostic_cap) {
                                break;
                            }
                            causes.push('|');
                            causes.push_str(&token);
                        }
                        if causes.len() > diagnostic_cap {
                            return Err(tos_validation::item_budget_origin!());
                        }
                        return Err(ItemRefusal::Source(causes));
                    }
                    let _ordered_rule_observations = evaluated.ordered_observation_sha256();
                    let after_defaults = view.original_io.snapshot();
                    let candidate_read = after_defaults
                        .read_attempted_bytes
                        .checked_sub(io_before_records.read_attempted_bytes)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let total_candidate_and_external = candidate_read
                        .checked_add(callback_external_reads.get())
                        .ok_or(tos_validation::item_budget_origin!())?;
                    if total_candidate_and_external > records_ticket.remaining().source_read_bytes {
                        return Err(ItemRefusal::BudgetCheck {
                            check: "candidate dependent source read attempts and external returns",
                            used: Some(total_candidate_and_external),
                            limit: Some(records_ticket.remaining().source_read_bytes),
                        });
                    }
                    let diagnostic_evidence_peak = common_evidence_state
                        .checked_add(evaluated.cost.peak_additional_state_upper_bound_bytes)
                        .ok_or(tos_validation::item_budget_origin!())?;
                    let callback_evidence_state = rules_evidence_peak.max(diagnostic_evidence_peak);
                    if callback_evidence_state > callback_workspace_state {
                        return Err(ItemRefusal::BudgetCheck {
                            check: "candidate dependent evidence state upper bound",
                            used: u64::try_from(callback_evidence_state).ok(),
                            limit: u64::try_from(callback_workspace_state).ok(),
                        });
                    }
                    let _ = biblio_query_rows;
                    Ok((callback_evidence_state, callback_external_reads.get()))
                },
            );
            let dependent_evidence = providers_result
                .map_err(|error| candidate_callback_refusal("candidate default providers", error))?;
            let final_callback_io = view.original_io.snapshot();
            if final_callback_io.read_attempted_bytes
                .checked_sub(io_before_records.read_attempted_bytes)
                .is_none_or(|read| read > records_ticket.remaining().source_read_bytes)
                || final_callback_io.write_attempted_bytes
                    .checked_sub(io_before_records.write_attempted_bytes)
                    .is_none_or(|write| write > remaining_write)
            {
                return Err(io::Error::other("candidate dependent owners exceeded shared IO"));
            }
            Ok(dependent_evidence)
        },
    ) {
        Ok(result) => result,
        Err(error) => {
            return fail_candidate_window_with_classified_io(
                view.execution_limits,
                view.remaining_budget,
                records_ticket,
                view.original_io,
                io_before_records,
                callback_external_reads.get(),
                view.candidate_io_adopted,
                view.remaining_write_bytes,
                FoundationOrchestratorError::Admission(error),
            );
        }
    };

    let records_cost_state = verified_records
        .record_issue_count()
        .checked_add(verified_records.item_issue_count())
        .and_then(|issues| issues.checked_add(verified_records.manifest_item_id_count()))
        .and_then(|items| items.checked_mul(std::mem::size_of::<usize>()))
        .ok_or_else(|| incomplete("candidate Records report state overflow"))?;
    let io_after_records = view.original_io.snapshot();
    let records_read_delta = io_after_records
        .read_attempted_bytes
        .checked_sub(io_before_records.read_attempted_bytes)
        .ok_or_else(|| incomplete("candidate Records IO read counter regressed"))?;
    let records_write_delta = io_after_records
        .write_attempted_bytes
        .checked_sub(io_before_records.write_attempted_bytes)
        .ok_or_else(|| incomplete("candidate Records IO write counter regressed"))?;
    let remaining_write = remaining_write
        .checked_sub(records_write_delta)
        .ok_or_else(|| incomplete("candidate Records IO exceeded write reservation"))?;
    let quota_after_records = worker_quota
        .usage()
        .map_err(|_| incomplete("candidate schema quota unavailable"))?;
    let (records_cpu, records_wire) = use_delta(quota_before_records, quota_after_records)?;
    let defaults_event_state = defaults.default_event_cost().retained_state_bytes;
    let records_charge_state = selected_index
        .cache_bytes
        .checked_add(defaults_limits.cache_bytes)
        .and_then(|state| state.checked_add(defaults_event_state))
        .and_then(|state| state.checked_add(std::mem::size_of::<BiblioRecordExecutor>()))
        .and_then(|state| state.checked_add(std::mem::size_of::<IndexSink<'candidate>>()))
        .and_then(|state| {
            state.checked_add(std::mem::size_of::<SpoolDefaultStore<'candidate, 'host>>())
        })
        .and_then(|state| {
            state.checked_add(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)
        })
        .and_then(|state| state.checked_add(records_cost_state))
        .and_then(|state| state.checked_add(dependent_state))
        .ok_or_else(|| incomplete("candidate Records retained-state overflow"))?;
    let records_usage = phase_use(
        records_read_delta
            .checked_add(history_rule_reads)
            .ok_or_else(|| incomplete("candidate Records external read cost overflow"))?,
        records_wire,
        records_charge_state,
        0,
        records_cpu,
        0,
        0,
    );
    if phase_amounts(records_usage)?.state_bytes > records_ticket.remaining().state_bytes
        || records_read_delta
            .checked_add(history_rule_reads)
            .is_none_or(|read| read > records_ticket.remaining().source_read_bytes)
    {
        return fail_window(
            view.execution_limits,
            view.remaining_budget,
            records_ticket,
            incomplete("candidate Records and dependent-owner report exceeds reservation"),
        );
    }
    complete_candidate_window(
        view.execution_limits,
        view.remaining_budget,
        records_ticket,
        records_usage,
        io_before_records,
        io_after_records,
        view.candidate_io_adopted,
        view.remaining_write_bytes,
        remaining_write,
    )?;

    // Records/defaults output is already bound and fully charged. No later
    // phase reads its private SQLite projection. Close its sole connection
    // and scope before compiling the native schema closure; otherwise the
    // finished defaults cache needlessly overlaps that constructor workspace.
    drop(defaults);

    let catalog_ticket = open_window(
        view.execution_limits,
        view.remaining_budget,
        "candidate-catalog-and-persisted-inputs",
        FoundationWindowKind::CatalogAndPersisted,
        FoundationPhaseReservation::default(),
    )?;
    let owner = |error| FoundationOrchestratorError::OwnerAt("candidate catalogue", error);
    let catalog_operation = catalog_ticket.operation_limits();
    // The physical census below already reads the original candidate ledger.
    // Include it in this phase's observed prefix, before any source traversal.
    let io_before_catalog = view.original_io.snapshot();
    // The fresh tree and route reader retain their returned-byte counts on
    // both success and refusal; no unknown whole-invocation allowance is spent.
    let catalog_external_reads = std::cell::Cell::new(0);
    let catalog_attempt = (|| {
        let catalog_identity_state = worker_identity_path_clone_bytes(worker_image.identity(), 3)?;
        let catalog_state_cap = catalog_operation
            .state_bytes
            .checked_sub(catalog_identity_state)
            .ok_or_else(|| incomplete("candidate catalog worker identity exceeds state"))?;
        let mut largest_catalog_member = 0u64;
        let mut observed_catalog_members = 0usize;
        let mut observed_catalog_bytes = 0u64;
        input
            .for_each_current_member_meta(deadline, cancelled, &mut |member| {
                // This is the physical preparation envelope, not the semantic
                // SourceWitness census. The producer also reads declared grammar,
                // registries and native source members from this same held input.
                observed_catalog_members = observed_catalog_members
                    .checked_add(1)
                    .ok_or(tos_validation::item_budget_origin!())?;
                largest_catalog_member = largest_catalog_member.max(member.size_bytes);
                observed_catalog_bytes = observed_catalog_bytes
                    .checked_add(member.size_bytes)
                    .ok_or(tos_validation::item_budget_origin!())?;
                Ok(())
            })
            .map_err(owner)?;
        if observed_catalog_members == 0
            || observed_catalog_members > max_members
            || u64::try_from(observed_catalog_members).ok() != Some(view.coverage.member_count())
            || observed_catalog_bytes != fence.source_bytes
        {
            return Err(incomplete("candidate catalog current-source census refused"));
        }
        let max_catalog_members = observed_catalog_members.min(4096).max(1);
        let max_catalog_files = max_members.min((u64::MAX - 1) as usize).max(1) as u64;
        let max_catalog_rows = catalog_operation.source_read_bytes.min(u64::MAX - 1).max(1);
        let max_catalog_file_bytes = usize::try_from(
            budgets
                .max_member_bytes
                .min(16 * 1024 * 1024)
                .min(largest_catalog_member.max(1))
                .min(catalog_operation.source_read_bytes)
                .min((usize::MAX - 1) as u64),
        )
        .map_err(|_| incomplete("candidate catalog file cap range"))?;
        let max_catalog_row_bytes = max_catalog_file_bytes.min(1024 * 1024).max(1);
        // Contract closure is aggregate retained input, independent of any one
        // file/row. The authenticated physical census bounds all contract bytes.
        let max_catalog_contract_bytes = usize::try_from(
            observed_catalog_bytes
                .min(catalog_operation.source_read_bytes)
                .min(catalog_state_cap as u64)
                .min(
                    tos_compiler::source_witness_catalog::SourceCatalogLimits::MAX_CONTRACT_BYTES
                        as u64,
                ),
        )
        .map_err(|_| incomplete("candidate catalog contract cap range"))?;
        let max_catalog_output_row_bytes = usize::try_from(
            catalog_operation
                .tmpfs_bytes
                .min(4 * 1024 * 1024)
                .min((usize::MAX - 1) as u64),
        )
        .map_err(|_| incomplete("candidate catalog output row cap range"))?;
        let catalog_limits = view
            .execution_limits
            .catalog_limits(
                max_catalog_files,
                max_catalog_rows,
                max_catalog_file_bytes,
                max_catalog_row_bytes,
                max_catalog_contract_bytes,
                max_catalog_output_row_bytes,
            )
            .map_err(FoundationOrchestratorError::Command)?;
        let catalog_source_limits = view
            .execution_limits
            .catalog_input_limits(max_catalog_members)
            .map_err(FoundationOrchestratorError::Command)?;
        let catalog_read_limits = view
            .execution_limits
            .read_limits(
                catalog_operation,
                max_members,
                observed_catalog_bytes.min(catalog_operation.source_read_bytes),
            )
            .map_err(FoundationOrchestratorError::Command)?;
        let max_claim_bytes = catalog_state_cap.min(128 * 1024 * 1024);
        let max_claim_rows = (catalog_limits.max_rows.min(16_384) as usize)
            .min(max_claim_bytes / catalog_limits.max_output_row_bytes.max(1));
        if max_claim_rows == 0 {
            return Err(incomplete("candidate catalog Claim cohort exceeds state reservation"));
        }
        let biblio_catalog_limits = view
            .execution_limits
            .bibliography_limits(
                catalog_limits,
                max_claim_rows,
                max_claim_bytes,
                catalog_operation.tmpfs_bytes,
            )
            .map_err(FoundationOrchestratorError::Command)?;
        let stage_limits = view
            .execution_limits
            .stage_limits(
                max_catalog_rows,
                max_catalog_rows.min(1024) as usize,
                catalog_operation.tmpfs_bytes.min(64 * 1024 * 1024),
            )
            .map_err(FoundationOrchestratorError::Command)?;
        // This worker belongs to the catalog phase, whose reservation may be
        // smaller than the schema phase. Every count dimension uses its own wire
        // envelope rather than mixing receipts from one phase with units of another.
        let catalog_max_checks = bounded_usize(
            u64::try_from(max_checks)
                .unwrap_or(u64::MAX - 1)
                .min(catalog_operation.worker_wire_bytes),
        )?
        .max(1);
        let catalog_worker_shape = FoundationCatalogWorkerShape {
            batch: BatchBudget::laboratory(),
            max_chunks: catalog_max_checks
                .div_ceil(BatchBudget::laboratory().max_units)
                .max(1) as u64,
            max_total_units: u64::try_from(catalog_max_checks)
                .unwrap_or(u64::MAX - 1)
                .min(catalog_operation.worker_wire_bytes)
                .max(1),
            max_total_raw_bytes: catalog_operation
                .source_read_bytes
                .min(catalog_operation.worker_wire_bytes)
                .min(BatchBudget::MAX_RAW_BYTES as u64)
                .max(1),
            max_total_wire_bytes: catalog_operation.worker_wire_bytes,
            max_distinct_selectors: catalog_max_checks.max(1).min(1024),
            max_receipts: catalog_max_checks.max(1),
            max_receipt_bytes: catalog_state_cap.min(1024 * 1024).max(1),
        };
        let (catalog_executor, _catalog_cut_limits, _catalog_stream, _catalog_diagnostics) = view
            .execution_limits
            .catalog_worker_limits(catalog_worker_shape)
            .map_err(FoundationOrchestratorError::Command)?;
        // Fresh catalog files are staged bytes read back for comparison, not the
        // final CLI receipt. Both physical and cumulative read meters still apply.
        let max_generated_bytes = usize::try_from(
            catalog_operation
                .tmpfs_bytes
                .min(catalog_operation.source_read_bytes)
                .min((usize::MAX - 1) as u64),
        )
        .map_err(|_| incomplete("candidate generated catalog byte cap range"))?;
        let max_generated_files = max_catalog_files as usize;
        let tree_limits = DisposableCatalogTreeLimits {
            max_total_bytes: usize::try_from(
                catalog_operation.tmpfs_bytes.min((usize::MAX - 1) as u64),
            )
            .map_err(|_| incomplete("candidate catalog tmpfs byte range"))?,
            max_file_bytes: max_catalog_output_row_bytes,
            max_files: usize::try_from(max_catalog_files).unwrap_or(usize::MAX - 1),
            max_state_bytes: catalog_state_cap,
            max_inodes: usize::try_from(max_catalog_files)
                .unwrap_or(usize::MAX - 4)
                .saturating_add(4),
        };
        let candidate_catalog_path = view.isolated.path().join("source-foundation.sqlite");
        let quota_before_catalog = worker_quota
            .usage()
            .map_err(|_| incomplete("candidate catalog schema quota unavailable"))?;
        let validator = tos_compiler::source_witness_catalog::SourceCatalogValidator::from_candidate_prepared(
            worker_image.identity(),
            catalog_executor,
            cancelled,
            deadline,
            &fence,
            &mut item_schemas
                as &mut dyn tos_validation::source_foundation_records::SourceFoundationCandidateSchemaBinding<CandidateFence>,
        )
        .map_err(FoundationOrchestratorError::Catalog)?;
        let catalog_result = super::foundation_catalog::compare_spooled_candidate(
            input,
            view.coverage,
            view.original_epoch,
            view.sources,
            &candidate_catalog_path,
            view.stage,
            stage_limits,
            catalog_source_limits,
            biblio_catalog_limits,
            &validator,
            true,
            // Version resolution charges repeated reads, not distinct input members.
            bounded_usize(budgets.max_readonly_record_read_calls)?,
            // Version resolution charges cumulative reads, not the largest member.
            bounded_usize(catalog_operation.source_read_bytes)?,
            max_generated_bytes,
            max_generated_files,
            catalog_state_cap,
            view.isolated,
            tree_limits,
            &mut index,
            cancelled,
            &catalog_external_reads,
        )
        .map_err(FoundationOrchestratorError::Catalog)?;
        let complete_catalog = match &catalog_result.outcome {
            FoundationCatalogOutcome::Complete(result)
                if result.issues.is_empty() && result.profiles.files().is_ok() =>
            {
                result
            }
            FoundationCatalogOutcome::SchemaRejected {
                diagnostic,
                bibliographic_phase,
                ..
            } => {
                let result = diagnostic.result();
                let contract = Digest256::of_bytes(result.contract().as_bytes()).to_hex();
                let issue = result.report().issues.first();
                let reason = issue.map_or(0, |issue| issue.reason as u16);
                let mut location = tos_foundation::Digest256Hasher::new();
                if let Some(issue) = issue {
                    for segment in &issue.instance_path {
                        use tos_validation::executor::schema_diagnostics::PathSegment;
                        match segment {
                            PathSegment::Property(name) => {
                                location.update(b"p");
                                location.update(&(name.len() as u64).to_be_bytes());
                                location.update(name.as_bytes());
                            }
                            PathSegment::Index(index) => {
                                location.update(b"i");
                                location.update(&index.to_be_bytes());
                            }
                        }
                    }
                }
                let location = location.finalize().to_hex();
                let site = format!(
                    "cat-b{}-s{}-r{reason:x}-c{}-i{}",
                    u8::from(*bibliographic_phase),
                    result.status() as u8,
                    &contract[..12],
                    &location[..6],
                );
                return Err(owner(ItemRefusal::Source(
                        crate::source_admission_spooled_index::bounded_source_cause(
                            "receiver-source",
                            &site,
                            result.path(),
                        ),
                )));
            }
            FoundationCatalogOutcome::Complete(result) => {
                let refusal = if let Some((path, detail)) = result.issues.first() {
                    let path = Digest256::of_bytes(path.as_bytes()).to_hex();
                    owner(ItemRefusal::Source(
                        crate::source_admission_spooled_index::bounded_source_cause(
                            "receiver-source",
                            &format!("cat-issues-{:x}-p{}", result.issues.len(), &path[..12]),
                            detail,
                        ),
                    ))
                } else {
                    incomplete("candidate catalog record profile selection incomplete")
                };
                return Err(refusal);
            }
        };
        let fresh = catalog_result
            .candidate
            .as_ref()
            .ok_or_else(|| incomplete("candidate catalog produced no fresh sink"))?;
        if !fresh.eof_verified() || catalog_result.fresh_sources.is_none() {
            return Err(incomplete("candidate fresh catalog EOF or source evidence is missing"));
        }
        let catalog_state = catalog_retained_state(&catalog_result.outcome)?
            .checked_add(catalog_identity_state)
            .and_then(|state| {
                state.checked_add(fresh.cost().fresh_rows_retained_state_upper_bound_bytes)
            })
            .ok_or_else(|| incomplete("candidate catalog retained-state overflow"))?;
        let catalog_read = checked_add_u64(
            fresh.cost().readback_bytes as u64,
            u64::try_from(complete_catalog.generated_read_bytes)
                .map_err(|_| incomplete("candidate catalog generated read range"))?,
        )?;
        if catalog_external_reads.get() != catalog_read {
            return Err(incomplete("candidate catalog external read observation differs"));
        }
        let io_after_catalog = candidate
            .io_usage()
            .map_err(FoundationOrchestratorError::Admission)?;
        let catalog_candidate_read = io_after_catalog
            .read_attempted_bytes
            .checked_sub(io_before_catalog.read_attempted_bytes)
            .ok_or_else(|| incomplete("candidate catalog IO read counter regressed"))?;
        let catalog_write_delta = io_after_catalog
            .write_attempted_bytes
            .checked_sub(io_before_catalog.write_attempted_bytes)
            .ok_or_else(|| incomplete("candidate catalog IO write counter regressed"))?;
        let remaining_write = remaining_write
            .checked_sub(catalog_write_delta)
            .ok_or_else(|| incomplete("candidate catalog IO exceeded write reservation"))?;
        let catalog_source_use = checked_add_u64(catalog_candidate_read, catalog_read)?;
        let catalog_free = catalog_ticket.remaining();
        if catalog_source_use > catalog_free.source_read_bytes
            || catalog_state > catalog_free.state_bytes
            || complete_catalog.issues.len() > catalog_free.issue_count
            || fresh.cost().output_bytes as u64 > catalog_free.tmpfs_bytes
            || fresh.cost().created_inodes as u64 > catalog_free.tmpfs_inodes
        {
            return Err(incomplete("candidate catalog evidence exceeds the invocation reservation"));
        }
        let catalog_quota_after = worker_quota
            .usage()
            .map_err(|_| incomplete("candidate catalog schema quota unavailable"))?;
        let (catalog_cpu, catalog_wire) = use_delta(quota_before_catalog, catalog_quota_after)?;
        let tmpfs_before = view
            .stage
            .quota_usage()
            .map_err(|_| incomplete("candidate catalog stage quota unavailable"))?;
        let catalog_usage = phase_use(
            catalog_source_use,
            catalog_wire,
            catalog_state,
            complete_catalog.issues.len(),
            catalog_cpu,
            fresh.cost().output_bytes as u64,
            fresh.cost().created_inodes as u64,
        );
        let _ = tmpfs_before;
        drop(catalog_result);
        drop(validator);
        // The catalog owner already closes the borrowed executor before returning
        // complete output. Verify its terminal state; a second finish is refused.
        if !item_schemas.is_finished()
            || item_schemas.input_identity() != input.input_identity()
        {
            return Err(incomplete(
                "candidate catalog schema worker EOF is incomplete",
            ));
        }
        Ok((catalog_usage, io_after_catalog, remaining_write))
    })();
    let (catalog_usage, io_after_catalog, remaining_write) = match catalog_attempt {
        Ok(result) => result,
        Err(error) => {
            return fail_candidate_window_with_classified_io(
                view.execution_limits,
                view.remaining_budget,
                catalog_ticket,
                view.original_io,
                io_before_catalog,
                catalog_external_reads.get(),
                view.candidate_io_adopted,
                view.remaining_write_bytes,
                error,
            );
        }
    };
    complete_candidate_window(
        view.execution_limits,
        view.remaining_budget,
        catalog_ticket,
        catalog_usage,
        io_before_catalog,
        io_after_catalog,
        view.candidate_io_adopted,
        view.remaining_write_bytes,
        remaining_write,
    )?;
    // Catalog EOF and its candidate binding were verified above. Release the
    // completed schema closure before preparing the next worker; its controller
    // workspace must not overlap the native constructor. The cumulative ledger
    // retains the earlier charge, and final custody checks the native worker's
    // own candidate binding and successful EOF.
    drop(item_schemas);
    let remaining = view
        .remaining_budget
        .remaining()
        .map_err(FoundationOrchestratorError::Command)?;
    candidate
        .restrict_remaining_io(remaining.source_read_bytes, remaining_write)
        .map_err(FoundationOrchestratorError::Admission)?;

    let native_ticket = open_window(
        view.execution_limits,
        view.remaining_budget,
        "candidate-native-index-schema-and-callback",
        FoundationWindowKind::Schema,
        FoundationPhaseReservation::default(),
    )?;
    let native_operation = native_ticket.operation_limits();
    let native_max_checks = usize::try_from(native_operation.worker_wire_bytes)
        .ok()
        .filter(|n| *n > 0 && *n < usize::MAX)
        .ok_or_else(|| incomplete("candidate native diagnostic count range"))?;
    let native_schema_limits = schema_limits_for_ticket(
        view.execution_limits,
        &native_ticket,
        schema_count,
        schema_max_bytes,
        schema_bytes,
        schema_loader_checks,
    )?;
    let mut native_shape = cut_worker_shape(native_operation, native_max_checks);
    native_shape.max_chunks = native_shape.max_total_units;
    let native_stream = view
        .execution_limits
        .cut_worker_stream_budget(native_operation, native_shape)
        .map_err(FoundationOrchestratorError::Command)?;
    let native_worker_limits = CutWorkerLimits {
        max_receipts: native_max_checks,
        max_receipt_bytes: native_operation.state_bytes.min(1024 * 1024).max(1),
    };
    let native_diagnostics = CutSchemaDiagnosticsLimits {
        max_total_issues: native_operation.issue_count.max(1),
        max_total_report_bytes: bounded_usize(native_operation.worker_wire_bytes)?.max(1),
        max_total_state_bytes: native_operation.state_bytes.max(1),
    };
    let held_native = base_declared_state_bytes
        .checked_add(selected_index.cache_bytes)
        .and_then(|state| state.checked_add(worker_image_state))
        .and_then(|state| state.checked_add(image_path_state))
        .and_then(|state| state.checked_add(payloads.cost().peak_state_bytes))
        .and_then(|state| state.checked_add(view.physical.cost().retained_state_bytes))
        .and_then(|state| {
            state.checked_add(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)
        })
        .ok_or_else(|| incomplete("candidate native worker held-state overflow"))?;
    let io_before_native = candidate
        .io_usage()
        .map_err(FoundationOrchestratorError::Admission)?;
    let quota_before_native = worker_quota
        .usage()
        .map_err(|_| incomplete("candidate native-index quota unavailable"))?;
    candidate
        .restrict_remaining_io(native_ticket.remaining().source_read_bytes, remaining_write)
        .map_err(FoundationOrchestratorError::Admission)?;
    let mut native_schemas = match prepare_candidate_schema_worker(
        &view,
        input,
        &worker_image,
        &worker_quota,
        &native_ticket,
        native_schema_limits,
        native_operation.state_bytes,
        native_worker_limits,
        native_diagnostics,
        native_stream,
        held_native,
        original_operation_state,
    ) {
        Ok(worker) => worker,
        Err(error) => {
            return fail_candidate_window_with_classified_io(
                view.execution_limits,
                view.remaining_budget,
                native_ticket,
                view.original_io,
                io_before_native,
                0,
                view.candidate_io_adopted,
                view.remaining_write_bytes,
                error,
            );
        }
    };
    let native_index_state = build_candidate_native_index(
        &mut index,
        &verified_records,
        &mut native_schemas,
        native_limits,
        json,
        json_state_bytes,
        deadline,
        cancelled,
    )?;
    let native_schema_state = schema_state_upper_bound(
        native_schemas.schema_bytes(),
        native_schemas.source_resource_count(),
        native_schemas
            .source_resource_metadata_state_bytes()
            .ok_or_else(|| incomplete("candidate native schema metadata state unavailable"))?,
    )?;
    let native_io_after = candidate
        .io_usage()
        .map_err(FoundationOrchestratorError::Admission)?;
    let native_read_delta = native_io_after
        .read_attempted_bytes
        .checked_sub(io_before_native.read_attempted_bytes)
        .ok_or_else(|| incomplete("candidate native-index IO read counter regressed"))?;
    let native_write_delta = native_io_after
        .write_attempted_bytes
        .checked_sub(io_before_native.write_attempted_bytes)
        .ok_or_else(|| incomplete("candidate native-index IO write counter regressed"))?;
    let remaining_write = remaining_write
        .checked_sub(native_write_delta)
        .ok_or_else(|| incomplete("candidate native-index IO exceeded write reservation"))?;
    let native_quota_after = worker_quota
        .usage()
        .map_err(|_| incomplete("candidate native-index quota unavailable"))?;
    let (native_cpu, native_wire) = use_delta(quota_before_native, native_quota_after)?;
    let native_state = native_index_state
        .checked_add(native_schema_state)
        .and_then(|state| {
            state.checked_add(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)
        })
        .ok_or_else(|| incomplete("candidate native-index retained state overflow"))?;
    if native_read_delta > native_ticket.remaining().source_read_bytes
        || native_state > native_ticket.remaining().state_bytes
    {
        return fail_window(
            view.execution_limits,
            view.remaining_budget,
            native_ticket,
            incomplete("candidate native-index evidence exceeds reservation"),
        );
    }
    complete_candidate_window(
        view.execution_limits,
        view.remaining_budget,
        native_ticket,
        phase_use(
            native_read_delta,
            native_wire,
            native_state,
            0,
            native_cpu,
            0,
            0,
        ),
        io_before_native,
        native_io_after,
        view.candidate_io_adopted,
        view.remaining_write_bytes,
        remaining_write,
    )?;
    // build_candidate_native_index owns the successful worker close.
    if !native_schemas.is_finished() {
        return Err(incomplete(
            "candidate native-index worker EOF is incomplete",
        ));
    }
    let remaining = view
        .remaining_budget
        .remaining()
        .map_err(FoundationOrchestratorError::Command)?;
    candidate
        .restrict_remaining_io(remaining.source_read_bytes, remaining_write)
        .map_err(FoundationOrchestratorError::Admission)?;
    *view.remaining_write_bytes = remaining_write;

    let history_retained_state = view
        .history
        .as_deref()
        .map_or(0, |history| history.usage().1);
    let final_held_state = candidate_owned_state
        .checked_add(base_declared_state_bytes)
        .and_then(|state| state.checked_add(selected_index.cache_bytes))
        .and_then(|state| state.checked_add(worker_image_state))
        .and_then(|state| state.checked_add(image_path_state))
        .and_then(|state| state.checked_add(native_schema_state))
        .and_then(|state| state.checked_add(native_index_state))
        .and_then(|state| state.checked_add(records_cost_state))
        .and_then(|state| {
            state.checked_add(CANDIDATE_RECORDS_REPORT_RETAINED_STATE_UPPER_BOUND_BYTES)
        })
        .and_then(|state| state.checked_add(payloads.cost().peak_state_bytes))
        .and_then(|state| state.checked_add(view.physical.cost().retained_state_bytes))
        .and_then(|state| state.checked_add(history_retained_state))
        .and_then(|state| state.checked_add(std::mem::size_of::<IndexSink<'candidate>>()))
        .and_then(|state| state.checked_add(std::mem::size_of::<BiblioRecordExecutor>()))
        .and_then(|state| state.checked_add(std::mem::size_of_val(&native_schemas)))
        .and_then(|state| {
            state.checked_add(std::mem::size_of::<
                crate::source_foundation_admission::NativeAdmissionComplete,
            >())
        })
        .ok_or_else(|| incomplete("candidate final retained-state overflow"))?;
    if final_held_state > working_ram
        || final_held_state > view.remaining_budget.charged().state_bytes
    {
        return Err(incomplete(
            "candidate final retained state was not admitted",
        ));
    }
    let final_held = FoundationPhaseReservation {
        state_bytes: final_held_state,
        ..FoundationPhaseReservation::default()
    };
    let original_io = view.original_io;
    let _payload_completion = finish_candidate_payload_and_physical(
        &mut view,
        candidate,
        input,
        original_io,
        payloads,
        &native_schemas,
        &mut record_executor,
        &worker_quota,
        final_held,
        remaining_write,
    )?;
    let io_terminal = candidate
        .io_usage()
        .map_err(FoundationOrchestratorError::Admission)?;
    *view.candidate_io_adopted = (
        io_terminal.read_attempted_bytes,
        io_terminal.write_attempted_bytes,
        io_terminal.read_upper_bound_attempted_bytes,
    );
    *view.remaining_write_bytes = remaining_write
        .checked_sub(
            io_terminal
                .write_attempted_bytes
                .checked_sub(native_io_after.write_attempted_bytes)
                .ok_or_else(|| incomplete("candidate final write counter regressed"))?,
        )
        .ok_or_else(|| incomplete("candidate final write cap exceeded"))?;
    Ok((index, verified_records))
}

struct NativeAdmissionStateHeader;

fn run<'work, 'receive, 'observe, 'cancel, 'signal>(
    view: FoundationBootstrapView<'work, 'cancel, 'signal>,
    mut payloads: FoundationPayloadSources<'work>,
    mut receive: Option<Box<AdmissionReceiver<'receive>>>,
    mut observation: Option<OwnedCatalogueObservation<'observe>>,
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
    let catalogue_representation = invocation.catalogue_representation();
    let owned_cold = catalogue_representation
        == super::foundation_entry::FoundationCatalogueRepresentation::OwnedCold;
    if owned_cold && (observation.is_none() || receive.is_some()) {
        return Err(incomplete(
            "owned-cold catalogue requires observation without admission",
        ));
    }
    if owned_cold && captured.epoch().token().is_none() {
        return Err(incomplete(
            "owned-cold catalogue requires actual ready metadata epoch",
        ));
    }
    let catalogue_origin =
        super::foundation_command::SourceFoundationCatalogueObservationOrigin::new(
            catalogue_representation,
        );

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
    captured
        .for_each_member(deadline, cancelled, |member| {
            let path = member.path.as_str();
            if path.starts_with("ToS/contracts/") && path.ends_with(".schema.json") {
                schema_count = schema_count.checked_add(1).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "schema resource count overflow")
                })?;
                let bytes = usize::try_from(member.size_bytes).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "schema resource size range")
                })?;
                schema_total = schema_total.checked_add(bytes).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "schema resource byte overflow")
                })?;
                schema_max = schema_max.max(bytes);
            }
            Ok(())
        })
        .map_err(|_| incomplete("source-foundation schema census refused"))?;
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
    let preparation = match captured.streamed_cut() {
        Some(cut) => {
            streamed_cut_schema_resource_preparation_state_upper_bound(cut, deadline, cancelled)
        }
        None => {
            cut_schema_resource_preparation_state_upper_bound(captured.cut(), deadline, cancelled)
        }
    };
    let schema_preparation_state = match preparation {
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
    let selected_schema_set = match captured.streamed_cut() {
        Some(cut) => SourceFoundationSchemaSet::from_streamed_cut(
            cut,
            FormatProfile::LegacyPythonObserved20260923,
            schema_limits,
            schema_preparation_state,
            deadline,
            cancelled,
        ),
        None => SourceFoundationSchemaSet::from_cut(
            captured.cut(),
            FormatProfile::LegacyPythonObserved20260923,
            schema_limits,
            deadline,
            cancelled,
        ),
    };
    let schema_set = match selected_schema_set {
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
    let source_resource_metadata_state = schema_set
        .source_resource_metadata_state_bytes()
        .ok_or_else(|| incomplete("source-foundation selected resource state overflow"))?;
    let schema_state = schema_state_upper_bound(
        loaded_schema_bytes,
        schema_set.schema_resource_count(),
        source_resource_metadata_state,
    )?;
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
                FoundationOrchestratorError::Default(
                    FoundationDefaultStage::CapturedCurrentPaths,
                    error,
                ),
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
    if record_executor
        .enable_diagnostics_v2(BiblioSchemaDiagnosticsLimits {
            max_total_issues: operation.issue_count,
            max_total_report_bytes: bounded_usize(operation.worker_wire_bytes)?,
            max_total_state_bytes: operation.state_bytes,
        })
        .is_err()
        || record_executor
            .set_shared_schema_worker_quota(worker_quota.clone())
            .is_err()
        || record_executor.set_operation_budget(record_stream).is_err()
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
    let selected_worker = match captured.streamed_cut() {
        Some(cut) => CutWorkerSchemaExecutor::from_streamed_cut_with_image(
            cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            executor_budget,
            worker_limits,
            deadline,
            cancelled,
        ),
        None => CutWorkerSchemaExecutor::from_cut_with_image(
            captured.cut(),
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            executor_budget,
            worker_limits,
            deadline,
            cancelled,
        ),
    };
    let mut item_schemas = match selected_worker {
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
        max_total_report_bytes: bounded_usize(operation.worker_wire_bytes)?,
        max_total_state_bytes: operation.state_bytes,
    };
    if item_schemas.enable_diagnostics_v2(item_diag).is_err()
        || item_schemas
            .set_shared_schema_worker_quota(worker_quota.clone())
            .is_err()
        || item_schemas.set_operation_budget(record_stream).is_err()
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
                FoundationOrchestratorError::Default(FoundationDefaultStage::PrepareRecords, error),
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
        max_total_report_bytes: bounded_usize(default_operation.worker_wire_bytes)?.max(1),
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
    let selected_worker = match captured.streamed_cut() {
        Some(cut) => CutWorkerSchemaExecutor::from_streamed_cut_with_image(
            cut,
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            executor_budget,
            layer_worker_limits,
            deadline,
            cancelled,
        ),
        None => CutWorkerSchemaExecutor::from_cut_with_image(
            captured.cut(),
            FormatProfile::LegacyPythonObserved20260923,
            &worker_image,
            executor_budget,
            layer_worker_limits,
            deadline,
            cancelled,
        ),
    };
    let mut layer_schemas = match selected_worker {
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
    if layer_schemas
        .enable_diagnostics_v2(default_diagnostic_limits)
        .is_err()
        || layer_schemas
            .set_shared_schema_worker_quota(worker_quota.clone())
            .is_err()
        || layer_schemas.set_operation_budget(layer_stream).is_err()
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
                FoundationOrchestratorError::Default(
                    FoundationDefaultStage::EvaluateDefault,
                    error,
                ),
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
    let observation_limits = observation.as_ref().map(|o| o.limits).unwrap_or_default();
    catalog_operation.source_read_bytes = match catalog_operation
        .source_read_bytes
        .checked_sub(observation_limits.max_stage_read_bytes)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalogue observation read reservation exceeds remaining budget"),
            );
        }
    };
    catalog_operation.state_bytes = match catalog_operation
        .state_bytes
        .checked_sub(observation_limits.max_state_bytes)
    {
        Some(bytes) => bytes,
        None => {
            return fail_window(
                execution_limits,
                remaining_budget,
                catalog_ticket,
                incomplete("catalogue observation state reservation exceeds remaining budget"),
            );
        }
    };
    let catalog_free = catalog_ticket.remaining();
    let mut catalog_members = 0usize;
    let mut largest_captured_member_bytes = 0u64;
    if captured
        .for_each_member(deadline, cancelled, |member| {
            largest_captured_member_bytes = largest_captured_member_bytes.max(member.size_bytes);
            if member.path.as_str().starts_with("ToS/source-witnesses/") {
                catalog_members = catalog_members.checked_add(1).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "catalog source-shape count overflow",
                    )
                })?;
            }
            Ok(())
        })
        .is_err()
    {
        return fail_window(
            execution_limits,
            remaining_budget,
            catalog_ticket,
            incomplete("catalog source-shape census refused"),
        );
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
    let max_catalog_contract_bytes = usize::try_from(
        captured
            .source_bytes()
            .min(catalog_operation.source_read_bytes)
            .min(catalog_operation.state_bytes as u64)
            .min(
                tos_compiler::source_witness_catalog::SourceCatalogLimits::MAX_CONTRACT_BYTES
                    as u64,
            ),
    )
    .map_err(|_| incomplete("catalog contract cap range"))?;
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
        max_catalog_contract_bytes,
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
        catalog_operation.tmpfs_bytes,
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
    // This worker belongs to the catalog phase, whose reservation may be
    // smaller than the schema phase. Every count dimension uses its own wire
    // envelope rather than mixing receipts from one phase with units of another.
    let catalog_max_checks = bounded_usize(
        u64::try_from(max_checks)
            .unwrap_or(u64::MAX - 1)
            .min(catalog_operation.worker_wire_bytes),
    )?
    .max(1);
    let catalog_worker_shape = FoundationCatalogWorkerShape {
        batch: BatchBudget::laboratory(),
        max_chunks: catalog_max_checks
            .div_ceil(BatchBudget::laboratory().max_units)
            .max(1) as u64,
        max_total_units: u64::try_from(catalog_max_checks)
            .unwrap_or(u64::MAX - 1)
            .min(catalog_operation.worker_wire_bytes)
            .max(1),
        max_total_raw_bytes: catalog_operation
            .source_read_bytes
            .min(catalog_operation.worker_wire_bytes)
            .min(BatchBudget::MAX_RAW_BYTES as u64)
            .max(1),
        max_total_wire_bytes: catalog_operation.worker_wire_bytes,
        max_distinct_selectors: catalog_max_checks.max(1).min(1024),
        max_receipts: catalog_max_checks.max(1),
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
    let fresh_catalogue_mode = candidate_mode || owned_cold;
    let mut candidate_result = None;
    let mut fresh_candidate = None;
    let mut fresh_catalog_sources = None;
    let mut admission_index = None;
    let catalog_outcome = if fresh_catalogue_mode {
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
            // Version resolution charges cumulative reads, not the largest member.
            bounded_usize(catalog_operation.source_read_bytes)?,
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
                    .saturating_add(4),
            },
            None,
            cancelled,
            |stage, receipt, limits| {
                if let Some(observation) = observation.take() {
                    stage.with_raw_input_read_budget(
                        observation.limits.max_stage_read_bytes,
                        |stage| (observation.callback)(stage, receipt, limits, catalogue_origin),
                    )?;
                }
                Ok(())
            },
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
        match foundation_catalog::compare_with_owned_catalogue_observation(
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
            // Version resolution charges cumulative reads, not the largest member.
            bounded_usize(catalog_operation.source_read_bytes)?,
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
            |stage, receipt, limits| {
                if let Some(observation) = observation.take() {
                    stage.with_raw_input_read_budget(
                        observation.limits.max_stage_read_bytes,
                        |stage| (observation.callback)(stage, receipt, limits, catalogue_origin),
                    )?;
                }
                Ok(())
            },
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
    let catalog_read = match catalog_read_bytes(&catalog_outcome)
        .and_then(|bytes| checked_add_u64(bytes, observation_limits.max_stage_read_bytes))
    {
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
    catalog_state = match checked_add_usize(catalog_state, catalog_identity_copy_state)
        .and_then(|bytes| checked_add_usize(bytes, observation_limits.max_state_bytes))
    {
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
            let selected_worker = match captured.streamed_cut() {
                Some(cut) => CutWorkerSchemaExecutor::from_streamed_cut_with_image(
                    cut,
                    FormatProfile::LegacyPythonObserved20260923,
                    &worker_image,
                    callback_worker_budget,
                    callback_limits,
                    deadline,
                    cancelled,
                ),
                None => CutWorkerSchemaExecutor::from_cut_with_image(
                    captured.cut(),
                    FormatProfile::LegacyPythonObserved20260923,
                    &worker_image,
                    callback_worker_budget,
                    callback_limits,
                    deadline,
                    cancelled,
                ),
            };
            let mut callback_schema = match selected_worker {
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
                .enable_diagnostics_v2(CutSchemaDiagnosticsLimits {
                    max_total_issues: callback_free.issue_count.max(1),
                    max_total_report_bytes: callback_report_bytes.max(1),
                    max_total_state_bytes: callback_free.state_bytes.max(1),
                })
                .is_err()
                || callback_schema
                    .set_diagnostics_v2_legacy_raw_instance_limit(max_schema_instance)
                    .is_err()
                || callback_schema
                    .set_shared_schema_worker_quota(worker_quota.clone())
                    .is_err()
                || callback_schema
                    .set_operation_budget(callback_stream)
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
        source_read_bytes: if observation_limits.max_stage_read_bytes == 0 {
            FoundationCharge::measured(catalog_amounts.source_read_bytes)
        } else {
            FoundationCharge::admitted_upper_bound(catalog_amounts.source_read_bytes)
        },
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
    // Schema-check actual owned generated files, never invent a persisted-root
    // oracle. The same original tail reservation and worker quota apply.
    let generated_sources = if owned_cold {
        fresh_catalog_sources
            .as_mut()
            .ok_or_else(|| incomplete("owned-cold generated root absent"))?
    } else {
        &mut *sources
    };
    let persisted_report = match foundation_catalog::inspect_persisted_catalog(
        generated_sources,
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
    let captured_member_read_upper = match usize::try_from(captured.source_bytes()) {
        Ok(bytes) => bytes,
        Err(_) => {
            return fail_window(
                execution_limits,
                remaining_budget,
                final_ticket,
                incomplete("captured EOF member byte census overflow"),
            );
        }
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
    let finalized = if owned_cold {
        let fresh_sources = fresh_catalog_sources
            .as_mut()
            .ok_or_else(|| incomplete("owned-cold final generated root absent"))?;
        match foundation_run::finalize_owned_cold_inputs(
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
    } else if let (Some(_), Some(fresh_sources)) =
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
