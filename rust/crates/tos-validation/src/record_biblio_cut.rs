//! Source-cut composition of existing record rules and bounded schema plans.
//! Retained cuts retain their own registry and schema profile and never enter
//! current identity joins.
use crate::executor::{
    BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget, ExecutorFailure, ExecutorOutcome,
    SchemaDiagnosticUnit, SchemaDiagnosticsCheckpoint, SchemaDiagnosticsOutcome,
    SharedSchemaWorkerQuota, VerifiedWorkerImage, VerifiedWorkerImageHandle, schema_diagnostics,
};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::record_rules::{
    BoundedMemberSchemaEvidence, BoundedSchemaVerdict, PathReferenceCheck, RecordFactBudget,
    RecordFamily, RecordFamilyReport, RecordGlobalJoin, RecordObservation, RecordRuleError,
    RecordSchema, RecordSink,
};
use crate::{FormatProfile, SchemaResource};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1, SourcePresenceV1};

const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const REGISTRY_SCHEMA: &str = "ToS/contracts/semantic-entity-type-registry.schema.json";

/// Resource-plan execution reuses the same disposable Draft2020-12 worker;
/// common-schema plans do not fit a contract-path-only schema interface.
/// The exact image is retained through this operation's first deadline; each
/// varying schema plan retains its own digest. A deliberate closure change
/// finalizes/reaps the old child before the same operation-owned image starts
/// another closure, without resetting aggregate CPU, wall, wire or selector caps.
pub struct BiblioRecordExecutor {
    pub worker: ExactWorkerIdentity,
    pub budget: ExecutorBudget,
    pub profile: FormatProfile,
    pub max_executions: usize,
    executions: usize,
    image: Option<VerifiedWorkerImage>,
    operation_budget: BatchStreamBudget,
    diagnostics_v2: Option<BiblioSchemaDiagnosticsLimits>,
    shared_schema_worker_quota: Option<SharedSchemaWorkerQuota>,
    diagnostic_issues_used: usize,
    diagnostic_report_bytes_used: usize,
    diagnostic_state_bytes_used: usize,
    pending_schema_diagnostics: Vec<SourceCutSchemaDiagnostic>,
    finished: bool,
}

/// Caller-owned aggregate limits for the opt-in schema-diagnostics-v2 path.
/// State accounting conservatively sums each closure, unit copy, request and
/// response buffer, and retained report. Operation-v2 remains separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiblioSchemaDiagnosticsLimits {
    pub max_total_issues: usize,
    pub max_total_report_bytes: usize,
    pub max_total_state_bytes: usize,
}

/// Public projection of the exact bounded verdict consumed by RecordFamily.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCutSchemaVerdict {
    pub instance_sha256: Digest256,
    pub schema_set_digest: Digest256,
    pub format_profile: FormatProfile,
    pub root_uri: String,
    pub worker_protocol_id: String,
    pub worker_binary_digest: Digest256,
    pub valid: bool,
}

/// One complete structured schema result. `before_issue` is the number of
/// owner observations already emitted when this exact plan was evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCutSchemaDiagnostic {
    pub evaluation_ordinal: u64,
    pub path: String,
    pub before_issue: usize,
    pub verdict: SourceCutSchemaVerdict,
    pub checkpoint: SchemaDiagnosticsCheckpoint,
    pub unit: SchemaDiagnosticUnit,
    pub schema_resource_bytes: usize,
    pub schema_resource_buffer_bytes: usize,
    pub input_instance_bytes: usize,
    pub input_instance_buffer_bytes: usize,
    pub input_metadata_bytes: usize,
    pub request_bytes: usize,
    pub request_buffer_bytes: usize,
    pub response_bytes: usize,
    pub response_buffer_bytes: usize,
    pub worker_cpu_micros: u64,
    pub retained_state_bytes: usize,
    /// Conservative aggregate charge for the transient closure/request/output
    /// and the report retained in this row.
    pub accounted_state_bytes: usize,
}

impl BiblioRecordExecutor {
    pub fn new(
        worker: ExactWorkerIdentity,
        budget: ExecutorBudget,
        profile: FormatProfile,
        max_executions: usize,
    ) -> Self {
        let mut operation_budget = BatchStreamBudget::laboratory();
        operation_budget.max_chunks = max_executions as u64;
        operation_budget.max_total_units = max_executions as u64;
        operation_budget.batch.total_execution_wall = budget.execution_wall;
        operation_budget.batch.startup_wall = budget.execution_wall;
        operation_budget.batch.per_unit_wall = budget.execution_wall;
        operation_budget.batch.cleanup_grace = budget.cleanup_grace;
        operation_budget.operation_cpu_seconds = budget.cpu_seconds;
        operation_budget.operation_address_space_bytes = budget.address_space_bytes;
        Self {
            worker,
            budget,
            profile,
            max_executions,
            executions: 0,
            image: None,
            operation_budget,
            diagnostics_v2: None,
            shared_schema_worker_quota: None,
            diagnostic_issues_used: 0,
            diagnostic_report_bytes_used: 0,
            diagnostic_state_bytes_used: 0,
            pending_schema_diagnostics: Vec::new(),
            finished: false,
        }
    }

    /// Construct the record-plan executor over the caller operation's already
    /// verified sealed worker image. Schema closures remain local to this
    /// executor and continue to use the caller's profile and budgets.
    pub fn new_with_image(
        image: &VerifiedWorkerImageHandle,
        budget: ExecutorBudget,
        profile: FormatProfile,
        max_executions: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let bounded_deadline = deadline.min(image.operation_deadline());
        let mut executor = Self::new(image.identity().clone(), budget, profile, max_executions);
        executor.image = Some(
            VerifiedWorkerImage::from_handle(image, budget, bounded_deadline, cancelled).map_err(
                |reason| match reason {
                    ExecutorFailure::Timeout => ItemRefusal::Deadline,
                    ExecutorFailure::Cancelled => {
                        ItemRefusal::Source("record schema image preparation cancelled".into())
                    }
                    other => {
                        ItemRefusal::Unsupported(format!("record schema image reuse: {other:?}"))
                    }
                },
            )?,
        );
        Ok(executor)
    }

    /// Selects schema-diagnostics-v2 before the first plan executes. The
    /// legacy v1 default remains unchanged, and a plan is never checked by
    /// both protocols in one operation.
    pub fn enable_diagnostics_v2(
        &mut self,
        limits: BiblioSchemaDiagnosticsLimits,
    ) -> Result<(), ItemRefusal> {
        if !self.is_unused() {
            return Err(ItemRefusal::Unsupported(
                "record operation already started".into(),
            ));
        }
        if !diagnostics_limits_valid(limits, self.max_executions, self.operation_budget) {
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema diagnostics limits",
                used: None,
                limit: None,
            });
        }
        self.diagnostics_v2 = Some(limits);
        Ok(())
    }

    /// Attach the explicitly selected diagnostics-v2 record executor to one
    /// invocation-wide quota before the first record plan starts.
    pub fn set_shared_schema_worker_quota(
        &mut self,
        quota: SharedSchemaWorkerQuota,
    ) -> Result<(), ItemRefusal> {
        if self.diagnostics_v2.is_none()
            || !self.is_unused()
            || self.shared_schema_worker_quota.is_some()
        {
            return Err(ItemRefusal::Unsupported(
                "shared schema worker quota requires unused diagnostics-v2 record executor".into(),
            ));
        }
        quota.ensure_attachable().map_err(|_| {
            ItemRefusal::Unsupported("shared schema worker quota is not attachable".into())
        })?;
        self.shared_schema_worker_quota = Some(quota);
        Ok(())
    }

    pub fn set_operation_budget(&mut self, budget: BatchStreamBudget) -> Result<(), ItemRefusal> {
        if self.executions != 0 || self.finished {
            return Err(ItemRefusal::Unsupported(
                "record operation already started".into(),
            ));
        }
        budget.validate().map_err(|reason| {
            ItemRefusal::Unsupported(format!("record operation envelope: {reason:?}"))
        })?;
        if self
            .diagnostics_v2
            .is_some_and(|limits| !diagnostics_limits_valid(limits, self.max_executions, budget))
        {
            return Err(ItemRefusal::Unsupported(
                "record schema diagnostics exceed operation envelope".into(),
            ));
        }
        self.operation_budget = budget;
        Ok(())
    }
    pub(crate) fn is_unused(&self) -> bool {
        self.executions == 0 && !self.finished
    }

    pub(crate) fn operation_budget(&self) -> BatchStreamBudget {
        self.operation_budget
    }

    pub fn finish(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        if self.finished {
            return Err(ItemRefusal::Unsupported(
                "record operation finalized".into(),
            ));
        }
        self.finished = true;
        if self.image.is_none() {
            if let Err(reason) = check(deadline, cancelled) {
                if let Some(quota) = &self.shared_schema_worker_quota {
                    quota.poison();
                }
                return Err(reason);
            }
        }
        if let Some(image) = self.image.as_mut() {
            image
                .finish(deadline, cancelled)
                .map_err(|reason| match reason {
                    ExecutorFailure::Timeout => ItemRefusal::Deadline,
                    ExecutorFailure::Cancelled => {
                        ItemRefusal::Source("record operation cancelled".into())
                    }
                    other => ItemRefusal::Unsupported(format!(
                        "record operation finalization: {other:?}; original exchange: {:?}",
                        image.exchange_failure()
                    )),
                })?;
        }
        Ok(())
    }
    fn evaluate(
        &mut self,
        resources: &[SchemaResource],
        location: &str,
        root: &str,
        raw: &[u8],
        expected_set: Digest256,
        limits: ItemLimits,
        cancelled: &AtomicBool,
    ) -> Result<BoundedSchemaVerdict, ItemRefusal> {
        if self.diagnostics_v2.is_some() {
            let result = self.evaluate_diagnostics_v2(
                resources,
                location,
                root,
                raw,
                expected_set,
                limits,
                cancelled,
            );
            if result.is_err()
                && let Some(quota) = &self.shared_schema_worker_quota
            {
                quota.poison();
            }
            return result;
        }
        if self.finished {
            return Err(ItemRefusal::Unsupported(
                "record operation finalized".into(),
            ));
        }
        if let Some(image) = self.image.as_mut() {
            image
                .preflight(limits.deadline, cancelled)
                .map_err(|reason| match reason {
                    ExecutorFailure::Timeout => ItemRefusal::Deadline,
                    ExecutorFailure::Cancelled => {
                        ItemRefusal::Source("record operation cancelled".into())
                    }
                    other => ItemRefusal::Unsupported(format!(
                        "record operation refused: {other:?}; original exchange: {:?}",
                        image.exchange_failure()
                    )),
                })?;
        } else if cancelled.load(Ordering::Relaxed) || Instant::now() >= limits.deadline {
            self.finished = true;
            return Err(ItemRefusal::Deadline);
        }
        check(limits.deadline, cancelled)?;
        if self.executions >= self.max_executions {
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema executions",
                used: (self.executions as u64).checked_add(1),
                limit: Some(self.max_executions as u64),
            });
        }
        if raw.len() > limits.max_member_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema member bytes",
                used: Some(raw.len() as u64),
                limit: Some(limits.max_member_bytes as u64),
            });
        }
        self.executions += 1;
        let mut budget = self.budget;
        budget.execution_wall = budget.execution_wall.min(
            limits
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or(ItemRefusal::Deadline)?,
        );
        if self
            .image
            .as_ref()
            .is_some_and(|image| !image.matches(&self.worker))
        {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::WorkerIdentity);
            return Err(ItemRefusal::Source(
                "record operation worker identity changed".into(),
            ));
        }
        let started = Instant::now();
        if self.image.is_none() {
            self.image = Some(
                VerifiedWorkerImage::prepare(&self.worker, budget, limits.deadline, cancelled)
                    .map_err(|reason| match reason {
                        ExecutorFailure::Timeout => ItemRefusal::Deadline,
                        ExecutorFailure::Cancelled => {
                            ItemRefusal::Source("record schema cancelled".into())
                        }
                        other => ItemRefusal::Unsupported(format!(
                            "record worker preparation: {other:?}"
                        )),
                    })?,
            );
        }
        if self.executions == 1 {
            self.image
                .as_mut()
                .unwrap()
                .set_operation_budget(self.operation_budget)
                .map_err(|reason| {
                    ItemRefusal::Unsupported(format!("record operation envelope: {reason:?}"))
                })?;
        }
        budget.execution_wall = budget.execution_wall.saturating_sub(started.elapsed());
        if budget.execution_wall.is_zero() {
            return Err(ItemRefusal::Deadline);
        }
        let result = self.image.as_mut().unwrap().evaluate(
            resources,
            self.profile,
            root,
            raw,
            budget,
            limits.deadline,
            cancelled,
        );
        if matches!(
            &result,
            ExecutorOutcome::SchemaValid(_) | ExecutorOutcome::SchemaInvalid(_)
        ) {
            self.image
                .as_mut()
                .unwrap()
                .preflight(limits.deadline, cancelled)
                .map_err(|reason| match reason {
                    ExecutorFailure::Timeout => ItemRefusal::Deadline,
                    ExecutorFailure::Cancelled => {
                        ItemRefusal::Source("record operation cancelled".into())
                    }
                    other => ItemRefusal::Unsupported(format!(
                        "record operation refused: {other:?}; original exchange: {:?}",
                        self.image
                            .as_ref()
                            .and_then(VerifiedWorkerImage::exchange_failure)
                    )),
                })?;
        }
        let (identity, valid) = match result {
            ExecutorOutcome::SchemaValid(identity) => (identity, true),
            ExecutorOutcome::SchemaInvalid(identity) => (identity, false),
            ExecutorOutcome::Indeterminate {
                reason: ExecutorFailure::Timeout,
                ..
            } => return Err(ItemRefusal::Deadline),
            ExecutorOutcome::Indeterminate {
                reason: ExecutorFailure::Cancelled,
                ..
            } => return Err(ItemRefusal::Source("record schema cancelled".into())),
            other => {
                return Err(ItemRefusal::Unsupported(format!(
                    "record schema incomplete: {other:?}"
                )));
            }
        };
        if identity.worker_sha256 != self.worker.sha256
            || identity.instance_sha256 != Digest256::of_bytes(raw)
            || identity.schema_set_sha256 != expected_set
            || identity.profile != self.profile
        {
            return Err(ItemRefusal::Source(
                "record worker evidence mismatch".into(),
            ));
        }
        Ok(BoundedSchemaVerdict {
            instance_sha256: identity.instance_sha256,
            schema_set_digest: identity.schema_set_sha256,
            format_profile: identity.profile,
            root_uri: root.into(),
            worker_protocol_id: "tos_bounded_schema_worker_v1".into(),
            worker_binary_digest: identity.worker_sha256,
            valid,
        })
    }

    fn evaluate_diagnostics_v2(
        &mut self,
        resources: &[SchemaResource],
        location: &str,
        root: &str,
        raw: &[u8],
        expected_set: Digest256,
        limits: ItemLimits,
        cancelled: &AtomicBool,
    ) -> Result<BoundedSchemaVerdict, ItemRefusal> {
        if self.finished {
            return Err(ItemRefusal::Unsupported(
                "record operation finalized".into(),
            ));
        }
        if let Some(image) = self.image.as_mut() {
            image
                .preflight(limits.deadline, cancelled)
                .map_err(diagnostics_refusal)?;
        } else if cancelled.load(Ordering::Relaxed) || Instant::now() >= limits.deadline {
            self.finished = true;
            return Err(ItemRefusal::Deadline);
        }
        check(limits.deadline, cancelled)?;
        if self.executions >= self.max_executions {
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema executions",
                used: (self.executions as u64).checked_add(1),
                limit: Some(self.max_executions as u64),
            });
        }
        if raw.len() > limits.max_member_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema member bytes",
                used: Some(raw.len() as u64),
                limit: Some(limits.max_member_bytes as u64),
            });
        }
        if location.len() > 4096 || root.len() > 4096 {
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema diagnostic metadata bytes",
                used: Some(location.len().max(root.len()) as u64),
                limit: Some(4096),
            });
        }
        self.executions += 1;
        let mut budget = self.budget;
        budget.execution_wall = budget.execution_wall.min(
            limits
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or(ItemRefusal::Deadline)?,
        );
        if self
            .image
            .as_ref()
            .is_some_and(|image| !image.matches(&self.worker))
        {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::WorkerIdentity);
            return Err(ItemRefusal::Source(
                "record operation worker identity changed".into(),
            ));
        }
        let started = Instant::now();
        if self.image.is_none() {
            self.image = Some(
                VerifiedWorkerImage::prepare(&self.worker, budget, limits.deadline, cancelled)
                    .map_err(diagnostics_refusal)?,
            );
        }
        if self.executions == 1 {
            let image = self.image.as_mut().unwrap();
            image
                .set_operation_budget(self.operation_budget)
                .map_err(diagnostics_refusal)?;
            if let Some(quota) = &self.shared_schema_worker_quota {
                image
                    .set_shared_schema_worker_quota(quota.clone())
                    .map_err(diagnostics_refusal)?;
            }
        }
        budget.execution_wall = budget.execution_wall.saturating_sub(started.elapsed());
        if budget.execution_wall.is_zero() {
            return Err(ItemRefusal::Deadline);
        }
        let (outcome, cost) = self
            .image
            .as_mut()
            .unwrap()
            .evaluate_with_diagnostics(
                resources,
                self.profile,
                location,
                root,
                raw,
                budget,
                limits.deadline,
                cancelled,
            )
            .map_err(diagnostics_refusal)?;
        let (mut units, checkpoint) = match outcome {
            SchemaDiagnosticsOutcome::Complete { units, checkpoint } => (units, checkpoint),
            SchemaDiagnosticsOutcome::Incomplete { reason, .. } => {
                return Err(diagnostics_refusal(reason));
            }
        };
        if units.len() != 1 || checkpoint.completed_count != 1 {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::Protocol);
            return Err(ItemRefusal::Unsupported(
                "record schema diagnostics coverage incomplete".into(),
            ));
        }
        let unit = units.pop().unwrap();
        let report = &unit.report;
        let caps_sha256 = schema_diagnostics::Caps::CURRENT.digest();
        if unit.ordinal != 0
            || unit.relative_path != location
            || unit.root_uri != root
            || unit.raw_sha256 != Digest256::of_bytes(raw)
            || unit.member_id != "biblio-record-schema-unit"
            || checkpoint.worker_sha256 != self.worker.sha256
            || checkpoint.profile != self.profile
            || checkpoint.schema_set_sha256 != expected_set
            || checkpoint.caps_sha256 != caps_sha256
            || checkpoint.request_sha256 != report.request_sha256
            || report.worker_sha256 != self.worker.sha256
            || report.request_sha256 != checkpoint.request_sha256
            || report.unit_sha256 != unit.unit_sha256
            || report.schema_set_sha256 != expected_set
            || report.caps_sha256() != caps_sha256
            || !report.is_well_formed()
            || !matches!(
                report.status,
                schema_diagnostics::Status::Valid | schema_diagnostics::Status::Invalid
            )
            || cost.worker_cpu_micros.is_none()
        {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::Protocol);
            return Err(ItemRefusal::Unsupported(
                "record schema diagnostics binding incomplete".into(),
            ));
        }
        let diagnostic_limits = self.diagnostics_v2.ok_or_else(|| {
            ItemRefusal::Unsupported("record schema diagnostics mode missing".into())
        })?;
        let issue_count = report.issues.len();
        let Some(next_issue_count) = self
            .diagnostic_issues_used
            .checked_add(issue_count)
            .filter(|count| *count <= diagnostic_limits.max_total_issues)
        else {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::InputBudget);
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema diagnostic issues",
                used: self
                    .diagnostic_issues_used
                    .checked_add(issue_count)
                    .and_then(|count| u64::try_from(count).ok()),
                limit: u64::try_from(diagnostic_limits.max_total_issues).ok(),
            });
        };
        let Some(next_report_bytes) = self
            .diagnostic_report_bytes_used
            .checked_add(cost.response_bytes)
            .filter(|bytes| *bytes <= diagnostic_limits.max_total_report_bytes)
        else {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::InputBudget);
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema diagnostic report bytes",
                used: self
                    .diagnostic_report_bytes_used
                    .checked_add(cost.response_bytes)
                    .and_then(|bytes| u64::try_from(bytes).ok()),
                limit: u64::try_from(diagnostic_limits.max_total_report_bytes).ok(),
            });
        };
        let schema_resource_bytes =
            schema_resource_encoding_bytes(resources).ok_or(ItemRefusal::Budget)?;
        let retained_state_bytes =
            schema_diagnostic_state_bytes(location, root, &unit).ok_or(ItemRefusal::Budget)?;
        let input_metadata_bytes = "biblio-record-schema-unit"
            .len()
            .checked_add(location.len())
            .and_then(|bytes| bytes.checked_add(root.len()))
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or(ItemRefusal::Budget)?;
        let accounted_state_bytes = cost
            .schema_resource_buffer_bytes
            .checked_add(cost.input_instance_buffer_bytes)
            .and_then(|bytes| bytes.checked_add(cost.request_buffer_bytes))
            .and_then(|bytes| bytes.checked_add(cost.response_buffer_bytes))
            .and_then(|bytes| bytes.checked_add(input_metadata_bytes))
            .and_then(|bytes| bytes.checked_add(retained_state_bytes))
            .ok_or(ItemRefusal::Budget)?;
        let Some(next_state_bytes) = self
            .diagnostic_state_bytes_used
            .checked_add(accounted_state_bytes)
            .filter(|bytes| *bytes <= diagnostic_limits.max_total_state_bytes)
        else {
            self.image
                .as_mut()
                .unwrap()
                .poison(ExecutorFailure::InputBudget);
            return Err(ItemRefusal::BudgetCheck {
                check: "record schema diagnostic state bytes",
                used: self
                    .diagnostic_state_bytes_used
                    .checked_add(accounted_state_bytes)
                    .and_then(|bytes| u64::try_from(bytes).ok()),
                limit: u64::try_from(diagnostic_limits.max_total_state_bytes).ok(),
            });
        };
        let valid = report.status == schema_diagnostics::Status::Valid;
        let verdict = BoundedSchemaVerdict {
            instance_sha256: unit.raw_sha256,
            schema_set_digest: report.schema_set_sha256,
            format_profile: checkpoint.profile,
            root_uri: root.to_owned(),
            worker_protocol_id: "tos_schema_diagnostics_v2".into(),
            worker_binary_digest: checkpoint.worker_sha256,
            valid,
        };
        self.pending_schema_diagnostics
            .push(SourceCutSchemaDiagnostic {
                evaluation_ordinal: (self.executions - 1) as u64,
                path: location.to_owned(),
                before_issue: 0,
                verdict: SourceCutSchemaVerdict {
                    instance_sha256: verdict.instance_sha256,
                    schema_set_digest: verdict.schema_set_digest,
                    format_profile: verdict.format_profile,
                    root_uri: verdict.root_uri.clone(),
                    worker_protocol_id: verdict.worker_protocol_id.clone(),
                    worker_binary_digest: verdict.worker_binary_digest,
                    valid: verdict.valid,
                },
                checkpoint,
                schema_resource_buffer_bytes: cost.schema_resource_buffer_bytes,
                input_instance_bytes: raw.len(),
                input_instance_buffer_bytes: cost.input_instance_buffer_bytes,
                input_metadata_bytes,
                request_bytes: cost.request_bytes,
                request_buffer_bytes: cost.request_buffer_bytes,
                response_bytes: cost.response_bytes,
                response_buffer_bytes: cost.response_buffer_bytes,
                worker_cpu_micros: cost.worker_cpu_micros.unwrap(),
                schema_resource_bytes,
                retained_state_bytes,
                accounted_state_bytes,
                unit,
            });
        self.diagnostic_issues_used = next_issue_count;
        self.diagnostic_report_bytes_used = next_report_bytes;
        self.diagnostic_state_bytes_used = next_state_bytes;
        Ok(verdict)
    }

    fn take_schema_diagnostics(&mut self, before_issue: usize) -> Vec<SourceCutSchemaDiagnostic> {
        let mut diagnostics = std::mem::take(&mut self.pending_schema_diagnostics);
        for diagnostic in &mut diagnostics {
            diagnostic.before_issue = before_issue;
        }
        diagnostics
    }
}

fn diagnostics_limits_valid(
    limits: BiblioSchemaDiagnosticsLimits,
    max_executions: usize,
    operation: BatchStreamBudget,
) -> bool {
    let executable_capacity = u64::try_from(max_executions)
        .ok()
        .map(|maximum| {
            maximum
                .min(operation.max_chunks)
                .min(operation.max_total_units)
        })
        .and_then(|maximum| maximum.checked_mul(schema_diagnostics::MAX_ISSUES_PER_UNIT as u64));
    let wire_capacity = usize::try_from(operation.max_total_wire_bytes).unwrap_or(usize::MAX);
    limits.max_total_issues > 0
        && executable_capacity.is_some_and(|capacity| {
            u64::try_from(limits.max_total_issues).is_ok_and(|requested| requested <= capacity)
        })
        && limits.max_total_report_bytes > 0
        && limits.max_total_report_bytes <= schema_diagnostics::MAX_RESPONSE_BYTES
        && limits.max_total_report_bytes <= wire_capacity
        && limits.max_total_state_bytes > 0
        && limits.max_total_state_bytes <= wire_capacity
}

fn diagnostics_refusal(reason: ExecutorFailure) -> ItemRefusal {
    match reason {
        ExecutorFailure::Timeout => ItemRefusal::Deadline,
        ExecutorFailure::Cancelled => {
            ItemRefusal::Source("record schema diagnostics cancelled".into())
        }
        ExecutorFailure::InputBudget | ExecutorFailure::CpuLimit => ItemRefusal::Budget,
        _ => ItemRefusal::Unsupported("record schema diagnostics worker failed".into()),
    }
}

fn schema_resource_encoding_bytes(resources: &[SchemaResource]) -> Option<usize> {
    resources.iter().try_fold(4usize, |total, resource| {
        total
            .checked_add(8)?
            .checked_add(resource.uri.len())?
            .checked_add(resource.raw.len())
    })
}

fn schema_diagnostic_state_bytes(
    location: &str,
    root_uri: &str,
    unit: &SchemaDiagnosticUnit,
) -> Option<usize> {
    fn add(total: &mut usize, bytes: usize) -> Option<()> {
        *total = total.checked_add(bytes)?;
        Some(())
    }
    fn path_bytes(path: &Vec<schema_diagnostics::PathSegment>) -> Option<usize> {
        path.iter().try_fold(
            path.capacity()
                .checked_mul(std::mem::size_of::<schema_diagnostics::PathSegment>())?,
            |total, segment| match segment {
                schema_diagnostics::PathSegment::Property(value) => {
                    total.checked_add(value.capacity())
                }
                schema_diagnostics::PathSegment::Index(_) => Some(total),
            },
        )
    }

    let mut total = std::mem::size_of::<SourceCutSchemaDiagnostic>()
        .checked_add(std::mem::size_of::<SourceCutSchemaVerdict>())?
        .checked_add(std::mem::size_of::<SchemaDiagnosticsCheckpoint>())?
        .checked_add(std::mem::size_of::<SchemaDiagnosticUnit>())?
        .checked_add(std::mem::size_of::<schema_diagnostics::Report>())?;
    add(&mut total, location.len())?;
    add(&mut total, root_uri.len())?;
    add(&mut total, "tos_schema_diagnostics_v2".len())?;
    add(&mut total, unit.member_id.capacity())?;
    add(&mut total, unit.relative_path.capacity())?;
    add(&mut total, unit.root_uri.capacity())?;
    add(
        &mut total,
        unit.report
            .issues
            .capacity()
            .checked_mul(std::mem::size_of::<schema_diagnostics::Issue>())?,
    )?;
    for issue in &unit.report.issues {
        add(&mut total, issue.schema_keyword.capacity())?;
        add(&mut total, path_bytes(&issue.instance_path)?)?;
        add(&mut total, path_bytes(&issue.schema_path)?)?;
    }
    Some(total)
}

#[derive(Debug, Clone)]
pub struct BiblioCurrentRecord {
    pub path: String,
    pub kind: String,
    pub value: serde_json::Value,
}

/// Bounded resource observations consumed by one complete current-and-retained
/// record inspection. The state field is the existing logical accounting
/// charge, not a process-memory or RSS measurement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCutRecordUsage {
    pub source_bytes_read: u64,
    pub accounted_state_upper_bound_bytes: usize,
    pub observed_issue_count: usize,
    /// Present only when this operation used diagnostics-v2. Its accounted
    /// state sum is already included in `accounted_state_upper_bound_bytes`.
    pub schema_diagnostics_v2: Option<SourceCutRecordSchemaCost>,
}

/// Exact sums from complete Biblio schema-diagnostics-v2 exchange rows. State
/// fields are subsets of `SourceCutRecordUsage::accounted_state_upper_bound_bytes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCutRecordSchemaCost {
    pub completed_exchanges: u64,
    pub issue_count: u64,
    pub schema_resource_bytes: u64,
    pub schema_resource_buffer_bytes: usize,
    pub input_instance_bytes: u64,
    pub input_instance_buffer_bytes: usize,
    pub input_metadata_bytes: usize,
    pub request_bytes: u64,
    pub request_buffer_bytes: usize,
    pub response_bytes: u64,
    pub response_buffer_bytes: usize,
    pub worker_cpu_micros: u64,
    pub retained_state_bytes: usize,
    pub accounted_state_bytes: usize,
}

pub struct SourceCutRecordReport {
    pub source_revision: SourceRevision,
    pub current_membership: SourceMembershipV1,
    pub retained_memberships: Vec<(SourceRevision, SourceMembershipV1)>,
    pub retained_record_profiles: Vec<RetainedRecordProfile>,
    pub records: BTreeMap<String, BiblioCurrentRecord>,
    pub observations: Vec<RecordObservation>,
    pub schema_diagnostics: Vec<SourceCutSchemaDiagnostic>,
    pub record_family: RecordFamilyReport,
    pub global_issues: u64,
    pub retained_profile_limits: Vec<String>,
    pub usage: SourceCutRecordUsage,
}

/// A historical profile is tied to one exact source revision. Its observations
/// cannot be interpreted as current ID owners or current reference targets.
pub struct RetainedRecordProfile {
    pub source_revision: SourceRevision,
    pub membership: SourceMembershipV1,
    pub observations: Vec<RecordObservation>,
    pub schema_diagnostics: Vec<SourceCutSchemaDiagnostic>,
    pub record_family: RecordFamilyReport,
    pub identity_version_issues: u64,
}

fn aggregate_schema_cost(
    current: &[SourceCutSchemaDiagnostic],
    retained: &[RetainedRecordProfile],
    executor: &BiblioRecordExecutor,
    initial_executions: usize,
    initial_diagnostic_issues: usize,
) -> Result<Option<SourceCutRecordSchemaCost>, ItemRefusal> {
    if executor.diagnostics_v2.is_none() {
        return Ok(None);
    }

    let expected_exchanges = executor
        .executions
        .checked_sub(initial_executions)
        .ok_or(ItemRefusal::Budget)?;
    let expected_issues = executor
        .diagnostic_issues_used
        .checked_sub(initial_diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    let mut total = SourceCutRecordSchemaCost {
        completed_exchanges: 0,
        issue_count: 0,
        schema_resource_bytes: 0,
        schema_resource_buffer_bytes: 0,
        input_instance_bytes: 0,
        input_instance_buffer_bytes: 0,
        input_metadata_bytes: 0,
        request_bytes: 0,
        request_buffer_bytes: 0,
        response_bytes: 0,
        response_buffer_bytes: 0,
        worker_cpu_micros: 0,
        retained_state_bytes: 0,
        accounted_state_bytes: 0,
    };
    let mut add = |diagnostic: &SourceCutSchemaDiagnostic| -> Result<(), ItemRefusal> {
        total.completed_exchanges = total
            .completed_exchanges
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        total.issue_count = total
            .issue_count
            .checked_add(
                u64::try_from(diagnostic.unit.report.issues.len())
                    .map_err(|_| ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;
        total.schema_resource_bytes = total
            .schema_resource_bytes
            .checked_add(
                u64::try_from(diagnostic.schema_resource_bytes).map_err(|_| ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;
        total.schema_resource_buffer_bytes = total
            .schema_resource_buffer_bytes
            .checked_add(diagnostic.schema_resource_buffer_bytes)
            .ok_or(ItemRefusal::Budget)?;
        total.input_instance_bytes = total
            .input_instance_bytes
            .checked_add(
                u64::try_from(diagnostic.input_instance_bytes).map_err(|_| ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;
        total.input_instance_buffer_bytes = total
            .input_instance_buffer_bytes
            .checked_add(diagnostic.input_instance_buffer_bytes)
            .ok_or(ItemRefusal::Budget)?;
        total.input_metadata_bytes = total
            .input_metadata_bytes
            .checked_add(diagnostic.input_metadata_bytes)
            .ok_or(ItemRefusal::Budget)?;
        total.request_bytes = total
            .request_bytes
            .checked_add(u64::try_from(diagnostic.request_bytes).map_err(|_| ItemRefusal::Budget)?)
            .ok_or(ItemRefusal::Budget)?;
        total.request_buffer_bytes = total
            .request_buffer_bytes
            .checked_add(diagnostic.request_buffer_bytes)
            .ok_or(ItemRefusal::Budget)?;
        total.response_bytes = total
            .response_bytes
            .checked_add(u64::try_from(diagnostic.response_bytes).map_err(|_| ItemRefusal::Budget)?)
            .ok_or(ItemRefusal::Budget)?;
        total.response_buffer_bytes = total
            .response_buffer_bytes
            .checked_add(diagnostic.response_buffer_bytes)
            .ok_or(ItemRefusal::Budget)?;
        total.worker_cpu_micros = total
            .worker_cpu_micros
            .checked_add(diagnostic.worker_cpu_micros)
            .ok_or(ItemRefusal::Budget)?;
        total.retained_state_bytes = total
            .retained_state_bytes
            .checked_add(diagnostic.retained_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        total.accounted_state_bytes = total
            .accounted_state_bytes
            .checked_add(diagnostic.accounted_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    };
    for diagnostic in current {
        add(diagnostic)?;
    }
    for profile in retained {
        for diagnostic in &profile.schema_diagnostics {
            add(diagnostic)?;
        }
    }
    if total.completed_exchanges
        != u64::try_from(expected_exchanges).map_err(|_| ItemRefusal::Budget)?
        || total.issue_count != u64::try_from(expected_issues).map_err(|_| ItemRefusal::Budget)?
    {
        return Err(ItemRefusal::Unsupported(
            "record schema diagnostics cost coverage incomplete".into(),
        ));
    }
    Ok(Some(total))
}

struct BoundedSink<'a> {
    rows: Vec<RecordObservation>,
    bytes: usize,
    cap: usize,
    issues: usize,
    max_issues: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
// The protected sink checks cancellation and deadline on each observation.
impl BoundedSink<'_> {
    fn emit(&mut self, row: RecordObservation) -> Result<(), RecordRuleError> {
        // The caller checks cancellation around each member and join. The sink
        // additionally checks deadline on each output, including long joins.
        if Instant::now() >= self.deadline {
            return Err(RecordRuleError::Sink {
                detail: "deadline".into(),
            });
        }
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(RecordRuleError::Sink {
                detail: "cancelled".into(),
            });
        }
        if matches!(row, RecordObservation::Issue { .. }) {
            if self.issues >= self.max_issues {
                return Err(RecordRuleError::Budget {
                    code: "record_issue_sink",
                });
            }
            self.issues += 1;
        }
        let cost = format!("{row:?}")
            .len()
            .checked_add(64)
            .ok_or(RecordRuleError::Budget {
                code: "biblio_sink",
            })?;
        self.bytes = self
            .bytes
            .checked_add(cost)
            .filter(|n| *n <= self.cap)
            .ok_or(RecordRuleError::Budget {
                code: "biblio_sink",
            })?;
        self.rows.push(row);
        Ok(())
    }
}

impl RecordSink for BoundedSink<'_> {
    fn push(&mut self, row: RecordObservation) -> Result<(), String> {
        self.emit(row).map_err(|error| match error {
            RecordRuleError::Budget { .. } => "budget".into(),
            RecordRuleError::Sink { detail } => detail,
            other => format!("{other:?}"),
        })
    }
}

fn retain_schema_diagnostics(
    executor: &mut BiblioRecordExecutor,
    before_issue: usize,
    output: &mut Vec<SourceCutSchemaDiagnostic>,
    state_bytes: &mut usize,
    max_state_bytes: usize,
) -> Result<(), ItemRefusal> {
    for diagnostic in executor.take_schema_diagnostics(before_issue) {
        reserve(
            state_bytes,
            diagnostic.accounted_state_bytes,
            max_state_bytes,
        )?;
        output.push(diagnostic);
    }
    Ok(())
}

fn refresh_sink_issue_limit(
    sink: &mut BoundedSink<'_>,
    total_issue_limit: usize,
    diagnostic_issues_used: usize,
) -> Result<(), ItemRefusal> {
    let record_issue_limit = total_issue_limit
        .checked_sub(diagnostic_issues_used)
        .ok_or(ItemRefusal::Budget)?;
    if sink.issues > record_issue_limit {
        return Err(ItemRefusal::BudgetCheck {
            check: "record issue sink after schema diagnostics",
            used: u64::try_from(sink.issues).ok(),
            limit: u64::try_from(record_issue_limit).ok(),
        });
    }
    sink.max_issues = record_issue_limit;
    Ok(())
}

pub fn inspect_records_from_cut(
    cut: &CorpusCutReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    executor: &mut BiblioRecordExecutor,
) -> Result<SourceCutRecordReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let initial_executions = executor.executions;
    let initial_diagnostic_issues = executor.diagnostic_issues_used;
    let mut used = 0u64;
    let registry = current(cut, REGISTRY, limits, cancelled, &mut used)?;
    let contract = current(cut, REGISTRY_SCHEMA, limits, cancelled, &mut used)?;
    let mut schemas = Vec::new();
    let mut state = registry
        .len()
        .checked_add(contract.len())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<SourceCutRecordUsage>()))
        .ok_or(ItemRefusal::Budget)?;
    if state > limits.max_state_bytes {
        return Err(ItemRefusal::Budget);
    }
    for metadata in cut.current().members() {
        check(limits.deadline, cancelled)?;
        let path = metadata.path.as_str();
        if path.starts_with("ToS/contracts/")
            && path.ends_with(".schema.json")
            && path != REGISTRY_SCHEMA
        {
            let raw = current(cut, path, limits, cancelled, &mut used)?;
            reserve(&mut state, path.len() + raw.len(), limits.max_state_bytes)?;
            schemas.push((path.to_owned(), raw));
        }
    }
    let fact_budget = RecordFactBudget {
        max_facts: limits.max_state_bytes as u64 / 32,
        max_encoded_bytes: limits.max_state_bytes as u64,
    };
    let (resources, root, set, _) =
        RecordFamily::registry_schema_plan(&contract, &registry, executor.profile)
            .map_err(record_error)?;
    let evidence = executor.evaluate(
        &resources, REGISTRY, &root, &registry, set, limits, cancelled,
    )?;
    let mut schema_diagnostics = executor.take_schema_diagnostics(0);
    for diagnostic in &schema_diagnostics {
        reserve(
            &mut state,
            diagnostic.accounted_state_bytes,
            limits.max_state_bytes,
        )?;
    }
    let registry_diagnostic_issues = executor
        .diagnostic_issues_used
        .checked_sub(initial_diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    let initial_record_issue_limit = limits
        .max_issues
        .checked_sub(registry_diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    let mut family = RecordFamily::new_with_bounded_registry(
        &registry,
        &contract,
        schemas.iter().map(|(path, raw)| RecordSchema { path, raw }),
        executor.profile,
        fact_budget,
        &evidence,
    )
    .map_err(record_error)?;
    let mut sink = BoundedSink {
        rows: Vec::new(),
        bytes: state,
        cap: limits.max_state_bytes,
        issues: 0,
        max_issues: initial_record_issue_limit,
        deadline: limits.deadline,
        cancelled,
    };
    family.emit_registry_read(&mut sink).map_err(record_error)?;
    let mut records = BTreeMap::new();
    let mut stream = cut.stream(cut.current().revision()).map_err(store_error)?;
    while let Some(member) = stream
        .next_member(limits.deadline, cancelled)
        .map_err(store_error)?
    {
        check(limits.deadline, cancelled)?;
        account(&mut used, member.raw.len(), limits.max_total_bytes)?;
        if member.raw.len() > limits.max_member_bytes {
            return Err(ItemRefusal::Budget);
        }
        let path = member.path.as_str();
        if !path.starts_with("ToS/source-witnesses/") || !path.ends_with(".json") {
            continue;
        }
        let basename = path.rsplit('/').next().unwrap_or("");
        let semantic = basename.starts_with("semantic-annotation") && basename.ends_with(".json");
        let carrier = family
            .classify_current_member(path, &member.raw)
            .map_err(record_error)?;
        if carrier.is_none() && !semantic {
            continue;
        }
        match family.member_schema_plan(path, &member.raw) {
            Ok(plan) => {
                let before_issue = sink.issues;
                let route = executor.evaluate(
                    &plan.resources,
                    path,
                    &plan.route_uri,
                    &member.raw,
                    plan.schema_set_digest,
                    limits,
                    cancelled,
                )?;
                retain_schema_diagnostics(
                    executor,
                    before_issue,
                    &mut schema_diagnostics,
                    &mut sink.bytes,
                    limits.max_state_bytes,
                )?;
                refresh_sink_issue_limit(
                    &mut sink,
                    limits.max_issues,
                    executor
                        .diagnostic_issues_used
                        .checked_sub(initial_diagnostic_issues)
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                let common = executor.evaluate(
                    &plan.resources,
                    path,
                    &plan.common_uri,
                    &member.raw,
                    plan.schema_set_digest,
                    limits,
                    cancelled,
                )?;
                retain_schema_diagnostics(
                    executor,
                    before_issue,
                    &mut schema_diagnostics,
                    &mut sink.bytes,
                    limits.max_state_bytes,
                )?;
                refresh_sink_issue_limit(
                    &mut sink,
                    limits.max_issues,
                    executor
                        .diagnostic_issues_used
                        .checked_sub(initial_diagnostic_issues)
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                family
                    .inspect_member_with_bounded_schema(
                        path,
                        &member.raw,
                        &BoundedMemberSchemaEvidence { route, common },
                        &mut sink,
                    )
                    .map_err(record_error)?;
            }
            Err(RecordRuleError::Unsupported {
                code: "unrecognized_record_basename",
                ..
            }) => {
                let plan = family
                    .native_schema_plan(path, &member.raw)
                    .map_err(record_error)?;
                let verdict = executor.evaluate(
                    &plan.resources,
                    path,
                    &plan.root_uri,
                    &member.raw,
                    plan.schema_set_digest,
                    limits,
                    cancelled,
                )?;
                retain_schema_diagnostics(
                    executor,
                    sink.issues,
                    &mut schema_diagnostics,
                    &mut sink.bytes,
                    limits.max_state_bytes,
                )?;
                refresh_sink_issue_limit(
                    &mut sink,
                    limits.max_issues,
                    executor
                        .diagnostic_issues_used
                        .checked_sub(initial_diagnostic_issues)
                        .ok_or(ItemRefusal::Budget)?,
                )?;
                family
                    .inspect_native_with_bounded_schema(path, &member.raw, &verdict, &mut sink)
                    .map_err(record_error)?;
            }
            Err(error) => return Err(record_error(error)),
        }
        if let Some(carrier) = carrier {
            // This is a source-authored identity index, never manifest IDs.
            reserve(
                &mut sink.bytes,
                path.len() + carrier.id.len() + carrier.kind.len() + member.raw.len() * 3,
                limits.max_state_bytes,
            )?;
            let value = serde_json::from_slice(&member.raw)
                .map_err(|_| ItemRefusal::Unsupported("record decoded representation".into()))?;
            records.entry(carrier.id).or_insert(BiblioCurrentRecord {
                path: path.into(),
                kind: carrier.kind,
                value,
            });
        }
    }
    let current_membership = stream
        .coverage()
        .ok_or_else(|| ItemRefusal::Source("record source EOF missing".into()))?;
    // Materialize sorted bounded facts from the protected sink. Larger corpora
    // need the owner's external-sort route rather than unbounded collections.
    let mut owners: Vec<_> = sink
        .rows
        .iter()
        .cloned()
        .filter_map(RecordObservation::into_global_id_fact)
        .collect();
    let mut uris: Vec<_> = sink
        .rows
        .iter()
        .cloned()
        .filter_map(RecordObservation::into_link_uri_fact)
        .collect();
    let mut refs: Vec<_> = sink
        .rows
        .iter()
        .cloned()
        .filter_map(RecordObservation::into_typed_id_ref_fact)
        .collect();
    reserve(
        &mut sink.bytes,
        owners
            .iter()
            .map(|r| r.id.len() + r.kind.len() + r.path.len() + 128)
            .sum::<usize>()
            + uris
                .iter()
                .map(|r| r.uri.len() + r.id.len() + r.path.len() + 128)
                .sum::<usize>()
            + refs
                .iter()
                .map(|r| r.target_id.len() + r.expected_kind.len() + r.from_path.len() + 128)
                .sum::<usize>(),
        limits.max_state_bytes,
    )?;
    owners.sort_by(|a, b| a.id.cmp(&b.id));
    uris.sort_by(|a, b| a.uri.cmp(&b.uri));
    refs.sort_by(|a, b| a.target_id.cmp(&b.target_id));
    let mut global_issues =
        RecordGlobalJoin::check_id_collisions(owners.clone(), fact_budget, &mut sink)
            .map_err(record_error)?;
    global_issues += RecordGlobalJoin::check_link_uri_collisions(uris, fact_budget, &mut sink)
        .map_err(record_error)?;
    global_issues += RecordGlobalJoin::check_typed_references(owners, refs, fact_budget, &mut sink)
        .map_err(record_error)?;
    let paths: Vec<_> = sink
        .rows
        .iter()
        .filter_map(|row| match row {
            RecordObservation::Reference {
                from_path,
                target_path,
                check,
            } => Some((from_path.clone(), target_path.clone(), *check)),
            _ => None,
        })
        .collect();
    for (from_path, target_path, mode) in paths {
        check(limits.deadline, cancelled)?;
        if !target_path.starts_with("ToS/") {
            continue;
        }
        let relative = RelativePath::parse(&target_path)
            .map_err(|_| ItemRefusal::Unsupported("record reference path".into()))?;
        let presence = cut.presence(cut.current().revision(), &relative);
        if presence.is_none()
            || (mode == PathReferenceCheck::FileIfToS && presence != Some(SourcePresenceV1::File))
        {
            sink.emit(RecordObservation::Issue {
                path: from_path,
                code: "record_path_reference_missing",
            })
            .map_err(record_error)?;
            global_issues += 1;
        }
    }
    // Current profile compilation is no longer needed during historical
    // validation; release its resource and route caches before the next one.
    let current_record_family = family.finish();
    drop(schemas);
    drop(registry);
    drop(contract);
    let mut retained_memberships = Vec::new();
    let mut retained_record_profiles = Vec::new();
    let mut historical_state = sink.bytes;
    let current_diagnostic_issues = executor
        .diagnostic_issues_used
        .checked_sub(initial_diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    let mut remaining_issues = limits
        .max_issues
        .checked_sub(sink.issues)
        .and_then(|remaining| remaining.checked_sub(current_diagnostic_issues))
        .ok_or(ItemRefusal::Budget)?;
    let mut record_issue_observations = sink.issues;
    let mut versions = BTreeMap::<(String, u64), String>::new();
    for row in &sink.rows {
        if let RecordObservation::IdOwner {
            id,
            version,
            raw_sha256,
            ..
        } = row
        {
            reserve(
                &mut historical_state,
                id.len() + raw_sha256.len() + 64,
                limits.max_state_bytes,
            )?;
            versions.insert((id.clone(), *version), raw_sha256.clone());
        }
    }
    for snapshot in cut.revisions().skip(1) {
        check(limits.deadline, cancelled)?;
        let history_diagnostic_start = executor.diagnostic_issues_used;
        let revision = snapshot.revision();
        let historical_registry =
            historical_required(cut, revision, REGISTRY, limits, cancelled, &mut used)?;
        let historical_contract =
            historical_required(cut, revision, REGISTRY_SCHEMA, limits, cancelled, &mut used)?;
        reserve(
            &mut historical_state,
            historical_registry.len() + historical_contract.len(),
            limits.max_state_bytes,
        )?;
        let mut historical_schemas = Vec::new();
        for metadata in snapshot.members() {
            check(limits.deadline, cancelled)?;
            let path = metadata.path.as_str();
            if path.starts_with("ToS/contracts/")
                && path.ends_with(".schema.json")
                && path != REGISTRY_SCHEMA
            {
                let raw = historical_required(cut, revision, path, limits, cancelled, &mut used)?;
                reserve(
                    &mut historical_state,
                    path.len() + raw.len(),
                    limits.max_state_bytes,
                )?;
                historical_schemas.push((path.to_owned(), raw));
            }
        }
        let (resources, root, set, _) = RecordFamily::registry_schema_plan(
            &historical_contract,
            &historical_registry,
            executor.profile,
        )
        .map_err(record_error)?;
        let evidence = executor.evaluate(
            &resources,
            REGISTRY,
            &root,
            &historical_registry,
            set,
            limits,
            cancelled,
        )?;
        let mut historical_schema_diagnostics = executor.take_schema_diagnostics(0);
        for diagnostic in &historical_schema_diagnostics {
            reserve(
                &mut historical_state,
                diagnostic.accounted_state_bytes,
                limits.max_state_bytes,
            )?;
        }
        let history_registry_diagnostic_issues = executor
            .diagnostic_issues_used
            .checked_sub(history_diagnostic_start)
            .ok_or(ItemRefusal::Budget)?;
        let history_record_issue_limit = remaining_issues
            .checked_sub(history_registry_diagnostic_issues)
            .ok_or(ItemRefusal::Budget)?;
        let mut historical_family = RecordFamily::new_with_bounded_registry(
            &historical_registry,
            &historical_contract,
            historical_schemas
                .iter()
                .map(|(path, raw)| RecordSchema { path, raw }),
            executor.profile,
            fact_budget,
            &evidence,
        )
        .map_err(record_error)?;
        // RecordFamily has its own selected resource copies. Keep no second
        // live copy of the retained registry/schema input during its stream.
        drop(resources);
        drop(root);
        drop(evidence);
        drop(historical_schemas);
        drop(historical_registry);
        drop(historical_contract);
        let mut historical_sink = BoundedSink {
            rows: Vec::new(),
            bytes: historical_state,
            cap: limits.max_state_bytes,
            issues: 0,
            max_issues: history_record_issue_limit,
            deadline: limits.deadline,
            cancelled,
        };
        historical_family
            .emit_registry_read(&mut historical_sink)
            .map_err(record_error)?;
        let mut history = cut.stream(snapshot.revision()).map_err(store_error)?;
        while let Some(member) = history
            .next_member(limits.deadline, cancelled)
            .map_err(store_error)?
        {
            account(&mut used, member.raw.len(), limits.max_total_bytes)?;
            check(limits.deadline, cancelled)?;
            if member.raw.len() > limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            let path = member.path.as_str();
            if !path.starts_with("ToS/source-witnesses/") || !path.ends_with(".json") {
                continue;
            }
            let basename = path.rsplit('/').next().unwrap_or("");
            let semantic =
                basename.starts_with("semantic-annotation") && basename.ends_with(".json");
            let carrier = historical_family
                .classify_current_member(path, &member.raw)
                .map_err(record_error)?;
            if carrier.is_none() && !semantic {
                continue;
            }
            match historical_family.member_schema_plan(path, &member.raw) {
                Ok(plan) => {
                    let before_issue = historical_sink.issues;
                    let route = executor.evaluate(
                        &plan.resources,
                        path,
                        &plan.route_uri,
                        &member.raw,
                        plan.schema_set_digest,
                        limits,
                        cancelled,
                    )?;
                    retain_schema_diagnostics(
                        executor,
                        before_issue,
                        &mut historical_schema_diagnostics,
                        &mut historical_sink.bytes,
                        limits.max_state_bytes,
                    )?;
                    refresh_sink_issue_limit(
                        &mut historical_sink,
                        remaining_issues,
                        executor
                            .diagnostic_issues_used
                            .checked_sub(history_diagnostic_start)
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    let common = executor.evaluate(
                        &plan.resources,
                        path,
                        &plan.common_uri,
                        &member.raw,
                        plan.schema_set_digest,
                        limits,
                        cancelled,
                    )?;
                    retain_schema_diagnostics(
                        executor,
                        before_issue,
                        &mut historical_schema_diagnostics,
                        &mut historical_sink.bytes,
                        limits.max_state_bytes,
                    )?;
                    refresh_sink_issue_limit(
                        &mut historical_sink,
                        remaining_issues,
                        executor
                            .diagnostic_issues_used
                            .checked_sub(history_diagnostic_start)
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    historical_family
                        .inspect_member_with_bounded_schema(
                            path,
                            &member.raw,
                            &BoundedMemberSchemaEvidence { route, common },
                            &mut historical_sink,
                        )
                        .map_err(record_error)?;
                }
                Err(RecordRuleError::Unsupported {
                    code: "unrecognized_record_basename",
                    ..
                }) => {
                    let plan = historical_family
                        .native_schema_plan(path, &member.raw)
                        .map_err(record_error)?;
                    let verdict = executor.evaluate(
                        &plan.resources,
                        path,
                        &plan.root_uri,
                        &member.raw,
                        plan.schema_set_digest,
                        limits,
                        cancelled,
                    )?;
                    retain_schema_diagnostics(
                        executor,
                        historical_sink.issues,
                        &mut historical_schema_diagnostics,
                        &mut historical_sink.bytes,
                        limits.max_state_bytes,
                    )?;
                    refresh_sink_issue_limit(
                        &mut historical_sink,
                        remaining_issues,
                        executor
                            .diagnostic_issues_used
                            .checked_sub(history_diagnostic_start)
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    historical_family
                        .inspect_native_with_bounded_schema(
                            path,
                            &member.raw,
                            &verdict,
                            &mut historical_sink,
                        )
                        .map_err(record_error)?;
                }
                Err(error) => return Err(record_error(error)),
            }
        }
        let membership = history
            .coverage()
            .ok_or_else(|| ItemRefusal::Source("retained record EOF missing".into()))?;
        let mut identity_version_issues = 0;
        for index in 0..historical_sink.rows.len() {
            let RecordObservation::IdOwner {
                id,
                version,
                raw_sha256,
                path,
                ..
            } = &historical_sink.rows[index]
            else {
                continue;
            };
            let key = (id.clone(), *version);
            if let Some(previous) = versions.get(&key) {
                if previous != raw_sha256 {
                    historical_sink
                        .emit(RecordObservation::Issue {
                            path: path.clone(),
                            code: "historical_identity_version_conflict",
                        })
                        .map_err(record_error)?;
                    identity_version_issues += 1;
                }
            } else {
                reserve(
                    &mut historical_sink.bytes,
                    id.len() + raw_sha256.len() + 64,
                    limits.max_state_bytes,
                )?;
                versions.insert(key, raw_sha256.clone());
            }
        }
        reserve(&mut historical_sink.bytes, 128, limits.max_state_bytes)?;
        historical_state = historical_sink.bytes;
        record_issue_observations = record_issue_observations
            .checked_add(historical_sink.issues)
            .ok_or(ItemRefusal::Budget)?;
        let history_diagnostic_issues = executor
            .diagnostic_issues_used
            .checked_sub(history_diagnostic_start)
            .ok_or(ItemRefusal::Budget)?;
        remaining_issues = remaining_issues
            .checked_sub(historical_sink.issues)
            .and_then(|remaining| remaining.checked_sub(history_diagnostic_issues))
            .ok_or(ItemRefusal::Budget)?;
        retained_memberships.push((revision, membership));
        retained_record_profiles.push(RetainedRecordProfile {
            source_revision: revision,
            membership,
            observations: historical_sink.rows,
            schema_diagnostics: historical_schema_diagnostics,
            record_family: historical_family.finish(),
            identity_version_issues,
        });
    }
    check(limits.deadline, cancelled)?;
    let diagnostic_issue_count = executor
        .diagnostic_issues_used
        .checked_sub(initial_diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    let observed_issue_count = record_issue_observations
        .checked_add(diagnostic_issue_count)
        .ok_or(ItemRefusal::Budget)?;
    let schema_diagnostics_v2 = aggregate_schema_cost(
        &schema_diagnostics,
        &retained_record_profiles,
        executor,
        initial_executions,
        initial_diagnostic_issues,
    )?;
    Ok(SourceCutRecordReport {
        source_revision: cut.current().revision(),
        current_membership,
        retained_memberships,
        retained_record_profiles,
        records,
        observations: sink.rows,
        schema_diagnostics,
        record_family: current_record_family,
        global_issues,
        retained_profile_limits: vec![
            "retained native compound lineage needs owner verification".into(),
        ],
        usage: SourceCutRecordUsage {
            source_bytes_read: used,
            accounted_state_upper_bound_bytes: historical_state,
            observed_issue_count,
            schema_diagnostics_v2,
        },
    })
}

fn historical_required(
    cut: &CorpusCutReader,
    revision: SourceRevision,
    path: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    used: &mut u64,
) -> Result<Vec<u8>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let relative = RelativePath::parse(path)
        .map_err(|_| ItemRefusal::Unsupported("historical profile source path".into()))?;
    if cut.presence(revision, &relative) != Some(SourcePresenceV1::File) {
        return Err(ItemRefusal::Unsupported(format!(
            "historical profile requires exact {path} in revision {revision:?}"
        )));
    }
    let member = cut
        .read_member(
            revision,
            &relative,
            limits.max_member_bytes as u64,
            limits.deadline,
            cancelled,
        )
        .map_err(store_error)?;
    account(used, member.raw.len(), limits.max_total_bytes)?;
    Ok(member.raw)
}

pub(crate) fn reserve(used: &mut usize, amount: usize, max: usize) -> Result<(), ItemRefusal> {
    let next = used.checked_add(amount);
    *used = next.filter(|n| *n <= max).ok_or(ItemRefusal::BudgetCheck {
        check: "record/bibliography logical state bytes",
        used: next.map(|n| n as u64),
        limit: Some(max as u64),
    })?;
    Ok(())
}
pub(crate) fn account(used: &mut u64, amount: usize, max: u64) -> Result<(), ItemRefusal> {
    let next = used.checked_add(amount as u64);
    *used = next.filter(|n| *n <= max).ok_or(ItemRefusal::BudgetCheck {
        check: "record/bibliography read bytes",
        used: next,
        limit: Some(max),
    })?;
    Ok(())
}
pub(crate) fn current(
    cut: &CorpusCutReader,
    path: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    used: &mut u64,
) -> Result<Vec<u8>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let relative = RelativePath::parse(path)
        .map_err(|_| ItemRefusal::Unsupported("bibliographic source path".into()))?;
    let member = cut
        .read_member(
            cut.current().revision(),
            &relative,
            limits.max_member_bytes as u64,
            limits.deadline,
            cancelled,
        )
        .map_err(store_error)?;
    account(used, member.raw.len(), limits.max_total_bytes)?;
    Ok(member.raw)
}
pub(crate) fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    use tos_source_store::StoreErrorCode;
    match error.code {
        StoreErrorCode::BudgetExceeded => ItemRefusal::Budget,
        StoreErrorCode::UnsupportedFormat | StoreErrorCode::UnsupportedPlatform => {
            ItemRefusal::Unsupported(error.to_string())
        }
        _ => ItemRefusal::Source(error.to_string()),
    }
}

pub(crate) use crate::validation_codec::{
    bounded_decoded_state, bounded_legacy_decoded_state, bounded_legacy_decoded_state_with_limits,
    bounded_legacy_item_decoded_state, bounded_ordered, check, decoded_state, decoded_wire_size,
    ordered_codec_state, ordered_emit_state, ordered_state, serialized_wire_size,
};
fn record_error(error: RecordRuleError) -> ItemRefusal {
    match error {
        RecordRuleError::Budget { code } => ItemRefusal::BudgetCheck {
            check: code,
            used: None,
            limit: None,
        },
        RecordRuleError::Sink { detail } if detail == "budget" => ItemRefusal::Budget,
        RecordRuleError::Sink { detail } if detail == "deadline" => ItemRefusal::Deadline,
        RecordRuleError::Sink { detail } if detail == "cancelled" => {
            ItemRefusal::Source("record family cancelled".into())
        }
        other => ItemRefusal::Unsupported(format!("{other:?}")),
    }
}

// Owned enum slot and owned string payloads; no debug serialization or allocator estimate.
pub(crate) fn predicate_state(read: &crate::PredicateRead) -> Result<usize, ItemRefusal> {
    use crate::PredicateRead::*;
    let strings: &[&str] = match read {
        ExactRecord {
            id,
            version,
            digest,
        } => &[id, version, digest],
        ExactPath { path, digest } => &[path, digest],
        ExactBytes { locator, digest } => &[locator, digest],
        IdentityKey { namespace, key, .. } | AbsentKey { namespace, key } => &[namespace, key],
        RefEndpoint {
            endpoint_type, id, ..
        } => &[endpoint_type, id],
        UniqueKey {
            namespace,
            key,
            owner,
        } => &[namespace, key, owner],
        Range {
            namespace,
            lower,
            upper,
            generation,
        } => &[namespace, lower, upper, generation],
        Prefix {
            namespace,
            prefix,
            generation,
        } => &[namespace, prefix, generation],
        ReverseRefs {
            target,
            relation,
            generation,
        } => &[target, relation, generation],
        Interval {
            scope, generation, ..
        } => &[scope, generation],
        SchemaResource { uri, digest } => &[uri, digest],
        Registry {
            uri,
            version,
            digest,
        } => &[uri, version, digest],
    };
    strings
        .iter()
        .try_fold(std::mem::size_of::<crate::PredicateRead>(), |sum, s| {
            sum.checked_add(s.len())
        })
        .ok_or(ItemRefusal::Budget)
}
