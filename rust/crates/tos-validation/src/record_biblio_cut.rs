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
    BoundedMemberSchemaEvidence, BoundedSchemaVerdict, GlobalIdFact, LinkUriFact,
    PathReferenceCheck, RecordFactBudget, RecordFamily, RecordFamilyReport, RecordGlobalJoin,
    RecordObservation, RecordRuleError, RecordSchema, RecordSink, TypedIdRefFact,
};
use crate::source_foundation_records::{
    SourceFoundationRecordFact, SourceFoundationRecordFactCollection,
    SourceFoundationRecordIdCarrier, SourceFoundationRecordsCursor, SourceFoundationRecordsIndex,
    SourceFoundationRecordsPageBudget, SourceFoundationRecordsStore,
};
use crate::{FormatProfile, SchemaResource};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1, SourcePresenceV1};

/// Borrowed member metadata supplied by an exact current source input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCutMemberMeta<'a> {
    pub path: &'a str,
    pub size_bytes: u64,
}

/// Adapter-reported coverage metadata for one current-member walk. This is
/// transport only: a completed kernel must independently compare its observed
/// stream and ask the adapter to verify its actual currentness fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCutInputCoverage {
    membership: SourceMembershipV1,
    member_count: u64,
    source_bytes_read: u64,
}

impl SourceCutInputCoverage {
    /// Call only after the adapter has reached its complete source EOF and
    /// verified the exact captured membership/currentness fence.
    pub fn after_verified_eof(
        membership: SourceMembershipV1,
        member_count: u64,
        source_bytes_read: u64,
    ) -> Self {
        Self {
            membership,
            member_count,
            source_bytes_read,
        }
    }

    pub fn membership(&self) -> SourceMembershipV1 {
        self.membership
    }

    pub fn member_count(&self) -> u64 {
        self.member_count
    }

    pub fn source_bytes_read(&self) -> u64 {
        self.source_bytes_read
    }
}

/// Private coverage for one complete, strict-descendant metadata range.
/// This is deliberately distinct from `SourceCutInputCoverage`: reaching this
/// range's end says nothing about members outside `directory/` and cannot
/// certify whole-source membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCutPrefixCoverage {
    directory: String,
    member_count: u64,
}

impl SourceCutPrefixCoverage {
    /// Mint only after the input adapter has reached the ordered end of this
    /// exact prefix range and checked its own full currentness fence.
    pub fn after_verified_prefix_eof(
        directory: &str,
        member_count: u64,
    ) -> Result<Self, ItemRefusal> {
        let parsed = RelativePath::parse(directory)
            .map_err(|_| ItemRefusal::Source("invalid current-member prefix".into()))?;
        if parsed.as_str() != directory {
            return Err(ItemRefusal::Source(
                "noncanonical current-member prefix".into(),
            ));
        }
        Ok(Self {
            directory: directory.to_owned(),
            member_count,
        })
    }

    pub fn directory(&self) -> &str {
        &self.directory
    }

    pub fn member_count(&self) -> u64 {
        self.member_count
    }
}

/// Bounded current-source access used by the additive streamed Records path.
/// The command adapter keeps raw member bytes borrowed inside each visitor and
/// retains its own opaque input identity; this interface creates no revision.
pub trait SourceCutInput {
    fn for_each_current_member_meta(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;

    /// Look up one member's size under the same currentness fence. The
    /// default retains full-walk semantics; indexed inputs may override it
    /// with a bounded exact lookup without claiming whole-source coverage.
    fn current_member_size(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<u64>, ItemRefusal> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(ItemRefusal::Source(
                "current-member point lookup cancelled".into(),
            ));
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Source("invalid current-member path".into()))?;
        let mut found = None;
        self.for_each_current_member_meta(deadline, cancelled, &mut |meta| {
            if meta.path == relative.as_str() && found.replace(meta.size_bytes).is_some() {
                return Err(ItemRefusal::Source(
                    "source input repeated a current member path".into(),
                ));
            }
            Ok(())
        })?;
        if cancelled.load(Ordering::Relaxed) {
            return Err(ItemRefusal::Source(
                "current-member point lookup cancelled".into(),
            ));
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        Ok(found)
    }

    /// Visit every strict descendant of one canonical relative directory and
    /// return private range coverage only after the range reaches EOF. The
    /// default is an honest full metadata walk; indexed adapters should
    /// override it with their bounded ordered prefix cursor and verify their
    /// exact full input fence both before and after that cursor.
    fn for_each_current_member_meta_under(
        &self,
        directory: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>) -> Result<(), ItemRefusal>,
    ) -> Result<SourceCutPrefixCoverage, ItemRefusal> {
        let parsed = RelativePath::parse(directory)
            .map_err(|_| ItemRefusal::Source("invalid current-member prefix".into()))?;
        if parsed.as_str() != directory {
            return Err(ItemRefusal::Source(
                "noncanonical current-member prefix".into(),
            ));
        }
        let prefix = format!("{directory}/");
        let mut member_count = 0u64;
        self.for_each_current_member_meta(deadline, cancelled, &mut |meta| {
            if cancelled.load(Ordering::Relaxed) {
                return Err(ItemRefusal::Source(
                    "current-member prefix walk cancelled".into(),
                ));
            }
            if Instant::now() >= deadline {
                return Err(ItemRefusal::Deadline);
            }
            if meta.path.starts_with(&prefix) {
                let member = RelativePath::parse(meta.path)
                    .map_err(|_| ItemRefusal::Source("invalid current-member path".into()))?;
                if member.as_str() != meta.path {
                    return Err(ItemRefusal::Source(
                        "noncanonical current-member path".into(),
                    ));
                }
                member_count = member_count.checked_add(1).ok_or(ItemRefusal::Budget)?;
                visit(meta)?;
            }
            Ok(())
        })?;
        SourceCutPrefixCoverage::after_verified_prefix_eof(directory, member_count)
    }

    fn with_current_member(
        &self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;

    fn for_each_current_member(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(SourceCutMemberMeta<'_>, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<SourceCutInputCoverage, ItemRefusal>;

    fn path_presence(
        &self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourcePresenceV1>, ItemRefusal>;

    fn verify_current_fence(
        &self,
        coverage: &SourceCutInputCoverage,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal>;
}

/// A current input whose opaque source identity can be compared with an
/// independently prepared candidate schema closure. Implementations keep
/// the identity type owned by their source adapter (for example a candidate
/// fence); it is never coerced into `SourceRevision`.
pub trait SourceCutInputWithIdentity<I: Eq>: SourceCutInput {
    fn input_identity(&self) -> &I;

    /// Explicit object-safe view avoids relying on trait-object upcasting in
    /// command adapters that also implement the identity-bearing subtrait.
    fn source_input(&self) -> &dyn SourceCutInput;
}

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

impl BiblioSchemaDiagnosticsLimits {
    /// Project caller ceilings into the same selected operation envelope.
    pub fn from_operation_ceilings(
        ceilings: Self,
        max_executions: usize,
        operation: BatchStreamBudget,
    ) -> Result<Self, ItemRefusal> {
        let issue_capacity =
            diagnostics_issue_capacity(max_executions, operation).ok_or(ItemRefusal::Budget)?;
        let issue_capacity = usize::try_from(issue_capacity).unwrap_or(usize::MAX);
        let wire_capacity = usize::try_from(operation.max_total_wire_bytes).unwrap_or(usize::MAX);
        let limits = Self {
            max_total_issues: ceilings.max_total_issues.min(issue_capacity),
            max_total_report_bytes: ceilings
                .max_total_report_bytes
                .min(schema_diagnostics::MAX_RESPONSE_BYTES)
                .min(wire_capacity),
            max_total_state_bytes: ceilings.max_total_state_bytes.min(wire_capacity),
        };
        if operation.validate().is_ok()
            && diagnostics_limits_valid(limits, max_executions, operation)
        {
            Ok(limits)
        } else {
            Err(ItemRefusal::Budget)
        }
    }
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

fn diagnostics_issue_capacity(max_executions: usize, operation: BatchStreamBudget) -> Option<u64> {
    u64::try_from(max_executions)
        .ok()
        .map(|maximum| {
            maximum
                .min(operation.max_chunks)
                .min(operation.max_total_units)
        })
        .and_then(|maximum| maximum.checked_mul(schema_diagnostics::MAX_ISSUES_PER_UNIT as u64))
}

fn diagnostics_limits_valid(
    limits: BiblioSchemaDiagnosticsLimits,
    max_executions: usize,
    operation: BatchStreamBudget,
) -> bool {
    let executable_capacity = diagnostics_issue_capacity(max_executions, operation);
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

/// Explicit caller-selected limit for RecordFamily and global join facts.
/// It is separate from retained-state and page bounds so spooling does not
/// inherit a RAM-derived corpus cardinality ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCutRecordFactBudget {
    pub max_facts: u64,
    pub max_encoded_bytes: u64,
}

/// Bounded summary from the opt-in current-only stored Records kernel.
/// Observation rows and sorted join facts remain in the caller's index and
/// are available only through its ordered bounded pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCutStreamedRecordSummary {
    current_membership: SourceMembershipV1,
    input_coverage: SourceCutInputCoverage,
    current_member_count: u64,
    current_source_bytes: u64,
    observation_count: u64,
    observation_digest: Digest256,
    global_issues: u64,
    family: SourceCutStreamedFamilySummary,
    usage: SourceCutRecordUsage,
}

impl SourceCutStreamedRecordSummary {
    pub fn current_membership(&self) -> &SourceMembershipV1 {
        &self.current_membership
    }

    /// Transport coverage from the fully streamed input pass. The source
    /// receiver may reuse it for a final fence check after its own Item reads;
    /// it does not replace the adapter's identity verification.
    pub fn input_coverage(&self) -> &SourceCutInputCoverage {
        &self.input_coverage
    }

    pub fn current_member_count(&self) -> u64 {
        self.current_member_count
    }

    pub fn current_source_bytes(&self) -> u64 {
        self.current_source_bytes
    }

    pub fn observation_count(&self) -> u64 {
        self.observation_count
    }

    pub fn observation_digest(&self) -> Digest256 {
        self.observation_digest
    }

    pub fn global_issues(&self) -> u64 {
        self.global_issues
    }

    pub fn family(&self) -> &SourceCutStreamedFamilySummary {
        &self.family
    }

    pub fn usage(&self) -> &SourceCutRecordUsage {
        &self.usage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCutStreamedFamilySummary {
    pub registry_version: String,
    pub registry_sha256: String,
    pub inspected_members: u64,
    pub issue_count: u64,
    pub emitted_member_fact_count: u64,
    pub emitted_member_fact_bytes: u64,
    pub enumerated_profile_count: usize,
    pub skipped_profile_count: usize,
    pub incomplete: bool,
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

#[derive(Clone)]
struct BiblioStoreHandle<'a>(Rc<RefCell<&'a mut (dyn SourceFoundationRecordsStore + 'a)>>);

impl<'a> BiblioStoreHandle<'a> {
    fn new(store: &'a mut (dyn SourceFoundationRecordsStore + 'a)) -> Self {
        Self(Rc::new(RefCell::new(store)))
    }

    fn with<T>(
        &self,
        action: impl FnOnce(&mut dyn SourceFoundationRecordsStore) -> Result<T, ItemRefusal>,
    ) -> Result<T, ItemRefusal> {
        let mut store = self.0.borrow_mut();
        action(&mut **store)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FactCoverageDigest {
    count: u64,
    xor: [u8; 32],
    sum: [u8; 32],
}

impl FactCoverageDigest {
    fn add(&mut self, digest: Digest256) -> Result<(), ItemRefusal> {
        self.count = self.count.checked_add(1).ok_or(ItemRefusal::Budget)?;
        for (slot, byte) in self.xor.iter_mut().zip(digest.as_bytes()) {
            *slot ^= *byte;
        }
        let mut carry = 0u16;
        for index in (0..32).rev() {
            let value = u16::from(self.sum[index])
                .checked_add(u16::from(digest.as_bytes()[index]))
                .and_then(|value| value.checked_add(carry))
                .ok_or(ItemRefusal::Budget)?;
            self.sum[index] = value as u8;
            carry = value >> 8;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderedRecordDigest {
    count: u64,
    digest: Digest256,
}

impl OrderedRecordDigest {
    fn new() -> Self {
        Self {
            count: 0,
            digest: Digest256::of_bytes(b"tos-biblio-record-observations-v1\0"),
        }
    }

    fn add(&mut self, ordinal: u64, row: &RecordObservation) -> Result<(), ItemRefusal> {
        if ordinal != self.count {
            return Err(ItemRefusal::Source(
                "Biblio observation ordinal is not contiguous".into(),
            ));
        }
        let leaf = hash_record_observation(ordinal, row)?;
        let next_count = self.count.checked_add(1).ok_or(ItemRefusal::Budget)?;
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-biblio-record-observation-sequence-v1\0");
        hash.update(self.digest.as_bytes());
        hash.update(&next_count.to_be_bytes());
        hash.update(leaf.as_bytes());
        self.digest = hash.finalize();
        self.count = next_count;
        Ok(())
    }
}

struct BiblioRecordProgress {
    failure: Option<ItemRefusal>,
    expected_observations: OrderedRecordDigest,
    expected_facts: [FactCoverageDigest; 4],
    issue_count: usize,
    live_page_state_bytes: usize,
    peak_state_bytes: usize,
}

impl BiblioRecordProgress {
    fn new(initial_state_bytes: usize) -> Self {
        Self {
            failure: None,
            expected_observations: OrderedRecordDigest::new(),
            expected_facts: [FactCoverageDigest::default(); 4],
            issue_count: 0,
            live_page_state_bytes: 0,
            peak_state_bytes: initial_state_bytes,
        }
    }

    fn fail(&mut self, refusal: ItemRefusal) {
        if self.failure.is_none() {
            self.failure = Some(refusal);
        }
    }
}

fn digest_frame(hash: &mut Digest256Hasher, bytes: &[u8]) -> Result<(), ItemRefusal> {
    let size = u64::try_from(bytes.len()).map_err(|_| ItemRefusal::Budget)?;
    hash.update(&size.to_be_bytes());
    hash.update(bytes);
    Ok(())
}

fn digest_text(hash: &mut Digest256Hasher, value: &str) -> Result<(), ItemRefusal> {
    digest_frame(hash, value.as_bytes())
}

fn hash_record_observation(
    ordinal: u64,
    row: &RecordObservation,
) -> Result<Digest256, ItemRefusal> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-biblio-record-observation-v1\0");
    hash.update(&ordinal.to_be_bytes());
    match row {
        RecordObservation::ExactPath { path, raw_sha256 } => {
            digest_text(&mut hash, "exact-path")?;
            digest_text(&mut hash, path)?;
            digest_text(&mut hash, raw_sha256)?;
        }
        RecordObservation::Registry {
            path,
            version,
            raw_sha256,
        } => {
            digest_text(&mut hash, "registry")?;
            digest_text(&mut hash, path)?;
            digest_text(&mut hash, version)?;
            digest_text(&mut hash, raw_sha256)?;
        }
        RecordObservation::Schema {
            path,
            uri,
            raw_sha256,
        } => {
            digest_text(&mut hash, "schema")?;
            digest_text(&mut hash, path)?;
            digest_text(&mut hash, uri)?;
            digest_text(&mut hash, raw_sha256)?;
        }
        RecordObservation::Profile {
            path,
            kind,
            profile_version,
            schema_version,
        } => {
            digest_text(&mut hash, "profile")?;
            digest_text(&mut hash, path)?;
            digest_text(&mut hash, kind)?;
            hash.update(&profile_version.to_be_bytes());
            digest_text(&mut hash, schema_version)?;
        }
        RecordObservation::Reference {
            from_path,
            target_path,
            check,
        } => {
            digest_text(&mut hash, "path-reference")?;
            digest_text(&mut hash, from_path)?;
            digest_text(&mut hash, target_path)?;
            digest_text(
                &mut hash,
                match check {
                    PathReferenceCheck::RepoExistsIfToS => "repo-exists-if-tos",
                    PathReferenceCheck::FileIfToS => "file-if-tos",
                },
            )?;
        }
        RecordObservation::RecordIdReference {
            from_path,
            target_id,
            expected_kind,
        } => {
            digest_text(&mut hash, "typed-id-reference")?;
            digest_text(&mut hash, from_path)?;
            digest_text(&mut hash, target_id)?;
            digest_text(&mut hash, expected_kind)?;
        }
        RecordObservation::LinkUriOwner { uri, id, path } => {
            digest_text(&mut hash, "link-uri-owner")?;
            digest_text(&mut hash, uri)?;
            digest_text(&mut hash, id)?;
            digest_text(&mut hash, path)?;
        }
        RecordObservation::IdOwner {
            id,
            kind,
            path,
            version,
            raw_sha256,
        } => {
            digest_text(&mut hash, "id-owner")?;
            digest_text(&mut hash, id)?;
            digest_text(&mut hash, kind)?;
            digest_text(&mut hash, path)?;
            hash.update(&version.to_be_bytes());
            digest_text(&mut hash, raw_sha256)?;
        }
        RecordObservation::IdKindOwner { kind, id, path } => {
            digest_text(&mut hash, "id-kind-owner")?;
            digest_text(&mut hash, kind)?;
            digest_text(&mut hash, id)?;
            digest_text(&mut hash, path)?;
        }
        RecordObservation::NativeReservation {
            id,
            packet_path,
            raw_sha256,
        } => {
            digest_text(&mut hash, "native-reservation")?;
            digest_text(&mut hash, id)?;
            digest_text(&mut hash, packet_path)?;
            digest_text(&mut hash, raw_sha256)?;
        }
        RecordObservation::Issue { path, code } => {
            digest_text(&mut hash, "issue")?;
            digest_text(&mut hash, path)?;
            digest_text(&mut hash, code)?;
        }
    }
    Ok(hash.finalize())
}

fn hash_record_fact(domain: &str, ordinal: u64, fields: &[&str]) -> Result<Digest256, ItemRefusal> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-biblio-record-fact-v1\0");
    digest_text(&mut hash, domain)?;
    hash.update(&ordinal.to_be_bytes());
    for field in fields {
        digest_text(&mut hash, field)?;
    }
    Ok(hash.finalize())
}

fn record_fact_slot(collection: SourceFoundationRecordFactCollection) -> Option<usize> {
    match collection {
        SourceFoundationRecordFactCollection::GlobalIdFacts => Some(0),
        SourceFoundationRecordFactCollection::LinkUriFacts => Some(1),
        SourceFoundationRecordFactCollection::TypedIdRefFacts => Some(2),
        SourceFoundationRecordFactCollection::PathReferenceFacts => Some(3),
        SourceFoundationRecordFactCollection::Observations => None,
    }
}

fn add_expected_fact(
    facts: &mut [FactCoverageDigest; 4],
    collection: SourceFoundationRecordFactCollection,
    digest: Digest256,
) -> Result<(), ItemRefusal> {
    let Some(slot) = record_fact_slot(collection) else {
        return Err(ItemRefusal::Source(
            "invalid Biblio fact digest collection".into(),
        ));
    };
    facts[slot].add(digest)
}

fn add_observation_fact_projections(
    progress: &mut BiblioRecordProgress,
    ordinal: u64,
    row: &RecordObservation,
) -> Result<(), ItemRefusal> {
    if let Some(fact) = row.clone().into_global_id_fact() {
        let carrier = match fact.carrier {
            crate::record_rules::IdCarrier::Standalone => "standalone",
            crate::record_rules::IdCarrier::NativePacket => "native-packet",
        };
        add_expected_fact(
            &mut progress.expected_facts,
            SourceFoundationRecordFactCollection::GlobalIdFacts,
            hash_record_fact(
                "global-id",
                ordinal,
                &[&fact.id, &fact.kind, &fact.path, carrier],
            )?,
        )?;
    }
    if let Some(fact) = row.clone().into_link_uri_fact() {
        add_expected_fact(
            &mut progress.expected_facts,
            SourceFoundationRecordFactCollection::LinkUriFacts,
            hash_record_fact("link-uri", ordinal, &[&fact.uri, &fact.id, &fact.path])?,
        )?;
    }
    if let Some(fact) = row.clone().into_typed_id_ref_fact() {
        add_expected_fact(
            &mut progress.expected_facts,
            SourceFoundationRecordFactCollection::TypedIdRefFacts,
            hash_record_fact(
                "typed-id-ref",
                ordinal,
                &[&fact.target_id, &fact.expected_kind, &fact.from_path],
            )?,
        )?;
    }
    if let RecordObservation::Reference {
        from_path,
        target_path,
        check,
    } = row
    {
        let check = match check {
            PathReferenceCheck::RepoExistsIfToS => "repo-exists-if-tos",
            PathReferenceCheck::FileIfToS => "file-if-tos",
        };
        add_expected_fact(
            &mut progress.expected_facts,
            SourceFoundationRecordFactCollection::PathReferenceFacts,
            hash_record_fact("path-reference", ordinal, &[from_path, target_path, check])?,
        )?;
    }
    Ok(())
}

struct StoredRecordObservationSink<'a> {
    store: BiblioStoreHandle<'a>,
    progress: Rc<RefCell<BiblioRecordProgress>>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    state_bytes: usize,
    max_state_bytes: usize,
    max_issues: usize,
    live_page_reservation: usize,
}

impl StoredRecordObservationSink<'_> {
    fn emit(&mut self, row: RecordObservation) -> Result<(), ItemRefusal> {
        check(self.deadline, self.cancelled)?;
        let row_state = record_observation_state_bytes(&row)?;
        let observed_state = self
            .state_bytes
            .checked_add(row_state)
            .ok_or(ItemRefusal::Budget)?;
        let projection_peak = observed_state
            .checked_add(row_state)
            .ok_or(ItemRefusal::Budget)?;
        let progress = self.progress.borrow();
        let page_state = progress.live_page_state_bytes;
        drop(progress);
        let live_observed = observed_state
            .checked_add(page_state)
            .ok_or(ItemRefusal::Budget)?;
        let live_projection_peak = projection_peak
            .checked_add(page_state)
            .ok_or(ItemRefusal::Budget)?;
        if live_projection_peak > self.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio stored observation transient state",
                used: Some(live_projection_peak as u64),
                limit: Some(self.max_state_bytes as u64),
            });
        }
        let mut progress = self.progress.borrow_mut();
        let ordinal = progress.expected_observations.count;
        if matches!(row, RecordObservation::Issue { .. }) && progress.issue_count >= self.max_issues
        {
            return Err(ItemRefusal::BudgetCheck {
                check: "record_issue_sink",
                used: progress
                    .issue_count
                    .checked_add(1)
                    .and_then(|value| u64::try_from(value).ok()),
                limit: u64::try_from(self.max_issues).ok(),
            });
        }
        self.store
            .with(|store| store.record_observation(ordinal, &row, self.deadline, self.cancelled))?;
        progress.peak_state_bytes = progress
            .peak_state_bytes
            .max(live_observed)
            .max(live_projection_peak);
        progress.expected_observations.add(ordinal, &row)?;
        add_observation_fact_projections(&mut progress, ordinal, &row)?;
        if matches!(row, RecordObservation::Issue { .. }) {
            progress.issue_count = progress
                .issue_count
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
        }
        Ok(())
    }

    fn store_current_record_first(
        &self,
        id: &str,
        record: &BiblioCurrentRecord,
    ) -> Result<(), ItemRefusal> {
        let decoded = crate::record_biblio_cut::decoded_state(&record.value)?;
        let temporary = self
            .state_bytes
            .checked_add(self.live_page_reservation)
            .ok_or(ItemRefusal::Budget)?
            .checked_add(std::mem::size_of::<BiblioCurrentRecord>())
            .and_then(|bytes| bytes.checked_add(id.len()))
            .and_then(|bytes| bytes.checked_add(record.path.len()))
            .and_then(|bytes| bytes.checked_add(record.kind.len()))
            .and_then(|bytes| bytes.checked_add(decoded))
            .ok_or(ItemRefusal::Budget)?;
        if temporary > self.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio stored record transient state",
                used: Some(temporary as u64),
                limit: Some(self.max_state_bytes as u64),
            });
        }
        let mut progress = self.progress.borrow_mut();
        progress.peak_state_bytes = progress.peak_state_bytes.max(temporary);
        drop(progress);
        self.store
            .with(|store| store.current_record_first(id, record))
    }
}

fn record_observation_state_bytes(row: &RecordObservation) -> Result<usize, ItemRefusal> {
    let payload = match row {
        RecordObservation::ExactPath { path, raw_sha256 } => {
            path.len().checked_add(raw_sha256.len())
        }
        RecordObservation::Registry {
            path,
            version,
            raw_sha256,
        } => path
            .len()
            .checked_add(version.len())
            .and_then(|bytes| bytes.checked_add(raw_sha256.len())),
        RecordObservation::Schema {
            path,
            uri,
            raw_sha256,
        } => path
            .len()
            .checked_add(uri.len())
            .and_then(|bytes| bytes.checked_add(raw_sha256.len())),
        RecordObservation::Profile {
            path,
            kind,
            schema_version,
            ..
        } => path
            .len()
            .checked_add(kind.len())
            .and_then(|bytes| bytes.checked_add(schema_version.len())),
        RecordObservation::Reference {
            from_path,
            target_path,
            ..
        } => from_path.len().checked_add(target_path.len()),
        RecordObservation::RecordIdReference {
            from_path,
            target_id,
            expected_kind,
        } => from_path
            .len()
            .checked_add(target_id.len())
            .and_then(|bytes| bytes.checked_add(expected_kind.len())),
        RecordObservation::LinkUriOwner { uri, id, path } => uri
            .len()
            .checked_add(id.len())
            .and_then(|bytes| bytes.checked_add(path.len())),
        RecordObservation::IdOwner {
            id,
            kind,
            path,
            raw_sha256,
            ..
        } => id
            .len()
            .checked_add(kind.len())
            .and_then(|bytes| bytes.checked_add(path.len()))
            .and_then(|bytes| bytes.checked_add(raw_sha256.len())),
        RecordObservation::IdKindOwner { kind, id, path } => kind
            .len()
            .checked_add(id.len())
            .and_then(|bytes| bytes.checked_add(path.len())),
        RecordObservation::NativeReservation {
            id,
            packet_path,
            raw_sha256,
        } => id
            .len()
            .checked_add(packet_path.len())
            .and_then(|bytes| bytes.checked_add(raw_sha256.len())),
        RecordObservation::Issue { path, code } => path.len().checked_add(code.len()),
    }
    .ok_or(ItemRefusal::Budget)?;
    std::mem::size_of::<RecordObservation>()
        .checked_add(payload)
        .ok_or(ItemRefusal::Budget)
}

impl RecordSink for StoredRecordObservationSink<'_> {
    fn push(&mut self, row: RecordObservation) -> Result<(), String> {
        self.emit(row).map_err(|error| {
            self.progress.borrow_mut().fail(error);
            "stored record observation refused".into()
        })
    }
}

fn check_stored_record_progress(
    progress: &Rc<RefCell<BiblioRecordProgress>>,
) -> Result<(), ItemRefusal> {
    if let Some(refusal) = progress.borrow().failure.clone() {
        return Err(refusal);
    }
    Ok(())
}

fn stored_record_rule_error(
    progress: &Rc<RefCell<BiblioRecordProgress>>,
    error: RecordRuleError,
) -> ItemRefusal {
    progress
        .borrow()
        .failure
        .clone()
        .unwrap_or_else(|| record_error(error))
}

#[derive(Default)]
struct RecordFactPageCoverage {
    observations: Option<OrderedRecordDigest>,
    facts: FactCoverageDigest,
}

struct RecordFactPageReader<'a> {
    store: BiblioStoreHandle<'a>,
    collection: SourceFoundationRecordFactCollection,
    page_budget: SourceFoundationRecordsPageBudget,
    total_state_limit: usize,
    base_state_bytes: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    progress: Rc<RefCell<BiblioRecordProgress>>,
    coverage: Rc<RefCell<RecordFactPageCoverage>>,
    cursor: Option<SourceFoundationRecordsCursor>,
    pending_cursor: Option<SourceFoundationRecordsCursor>,
    rows: std::vec::IntoIter<SourceFoundationRecordFact>,
    page_charge: usize,
    page_loaded: bool,
    done: bool,
    last_key: Option<String>,
    last_ordinal: Option<u64>,
}

fn reserve_record_fact_reader_state(
    sink: &mut StoredRecordObservationSink<'_>,
    page_budget: SourceFoundationRecordsPageBudget,
    reader_count: usize,
) -> Result<usize, ItemRefusal> {
    let cursor_reservation = page_budget
        .max_cursor_bytes
        .get()
        .checked_mul(2)
        .ok_or(ItemRefusal::Budget)?;
    let page_reservation = page_budget
        .max_state_bytes
        .get()
        .checked_mul(3)
        .ok_or(ItemRefusal::Budget)?;
    let per_reader = std::mem::size_of::<RecordFactPageReader<'static>>()
        .checked_add(cursor_reservation)
        .and_then(|bytes| bytes.checked_add(page_reservation))
        .ok_or(ItemRefusal::Budget)?;
    let reservation = per_reader
        .checked_mul(reader_count)
        .ok_or(ItemRefusal::Budget)?;
    let total = sink
        .state_bytes
        .checked_add(reservation)
        .ok_or(ItemRefusal::Budget)?;
    if total > sink.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio join readers, pages, and cursor overlap reservation",
            used: Some(total as u64),
            limit: Some(sink.max_state_bytes as u64),
        });
    }
    sink.live_page_reservation = reservation;
    let mut progress = sink.progress.borrow_mut();
    progress.live_page_state_bytes = reservation;
    progress.peak_state_bytes = progress.peak_state_bytes.max(total);
    Ok(sink
        .state_bytes
        .checked_add(reservation)
        .and_then(|bytes| bytes.checked_sub(per_reader))
        .ok_or(ItemRefusal::Budget)?)
}

fn release_record_fact_reader_state(sink: &mut StoredRecordObservationSink<'_>) {
    sink.live_page_reservation = 0;
    sink.progress.borrow_mut().live_page_state_bytes = 0;
}

impl<'a> RecordFactPageReader<'a> {
    fn new(
        store: BiblioStoreHandle<'a>,
        collection: SourceFoundationRecordFactCollection,
        page_budget: SourceFoundationRecordsPageBudget,
        total_state_limit: usize,
        base_state_bytes: usize,
        deadline: Instant,
        cancelled: &'a AtomicBool,
        progress: Rc<RefCell<BiblioRecordProgress>>,
    ) -> Result<Self, ItemRefusal> {
        if base_state_bytes >= total_state_limit {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio fact cursor working state",
                used: Some(base_state_bytes as u64),
                limit: Some(total_state_limit as u64),
            });
        }
        Ok(Self {
            store,
            collection,
            page_budget,
            total_state_limit,
            base_state_bytes,
            deadline,
            cancelled,
            progress,
            coverage: Rc::new(RefCell::new(RecordFactPageCoverage::default())),
            cursor: None,
            pending_cursor: None,
            rows: Vec::new().into_iter(),
            page_charge: 0,
            page_loaded: false,
            done: false,
            last_key: None,
            last_ordinal: None,
        })
    }

    fn coverage(&self) -> Rc<RefCell<RecordFactPageCoverage>> {
        self.coverage.clone()
    }

    fn fetch_page(&mut self) -> Result<(), ItemRefusal> {
        check(self.deadline, self.cancelled)?;
        let cursor_bytes = self
            .cursor
            .as_ref()
            .map_or(0, |cursor| cursor.as_bytes().len());
        let last_key_bytes = self.last_key.as_ref().map_or(0, String::capacity);
        let fixed_bytes = std::mem::size_of::<Self>()
            .checked_add(cursor_bytes)
            .and_then(|bytes| bytes.checked_add(last_key_bytes))
            .and_then(|bytes| bytes.checked_add(self.page_budget.max_cursor_bytes.get()))
            .ok_or(ItemRefusal::Budget)?;
        let available_total = self
            .total_state_limit
            .checked_sub(self.base_state_bytes)
            .and_then(|bytes| bytes.checked_sub(fixed_bytes))
            .ok_or(ItemRefusal::Budget)?;
        let page_limit = available_total.min(self.page_budget.max_state_bytes.get());
        let Some(page_limit) = std::num::NonZeroUsize::new(page_limit) else {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio fact page retained state",
                used: Some(fixed_bytes as u64),
                limit: Some(self.total_state_limit as u64),
            });
        };
        let budget = SourceFoundationRecordsPageBudget {
            max_rows: self.page_budget.max_rows,
            max_state_bytes: page_limit,
            max_cursor_bytes: self.page_budget.max_cursor_bytes,
        };
        let page = self.store.with(|store| {
            SourceFoundationRecordsIndex::new(&*store).record_fact_page(
                self.collection,
                self.cursor.as_ref(),
                budget,
                self.deadline,
                self.cancelled,
            )
        })?;
        check(self.deadline, self.cancelled)?;
        self.page_charge = page.charged_state_bytes;
        self.rows = page.rows.into_iter();
        self.pending_cursor = page.next_cursor;
        self.page_loaded = true;
        let pending_cursor_bytes = self
            .pending_cursor
            .as_ref()
            .map_or(0, |cursor| cursor.as_bytes().len());
        if pending_cursor_bytes > self.page_budget.max_cursor_bytes.get() {
            return Err(ItemRefusal::Source(
                "Biblio fact page cursor exceeds its reservation".into(),
            ));
        }
        let total = self
            .base_state_bytes
            .checked_add(self.page_charge)
            .and_then(|bytes| bytes.checked_add(fixed_bytes))
            .ok_or(ItemRefusal::Budget)?;
        if total > self.total_state_limit {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio fact page retained state",
                used: Some(total as u64),
                limit: Some(self.total_state_limit as u64),
            });
        }
        let actual_total = self
            .base_state_bytes
            .checked_add(self.page_charge)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .and_then(|bytes| bytes.checked_add(cursor_bytes))
            .and_then(|bytes| bytes.checked_add(pending_cursor_bytes))
            .and_then(|bytes| bytes.checked_add(last_key_bytes))
            .ok_or(ItemRefusal::Budget)?;
        let mut progress = self.progress.borrow_mut();
        progress.peak_state_bytes = progress.peak_state_bytes.max(total);
        progress.peak_state_bytes = progress.peak_state_bytes.max(actual_total);
        Ok(())
    }

    fn next_fact(&mut self) -> Result<Option<SourceFoundationRecordFact>, ItemRefusal> {
        loop {
            if let Err(error) = check(self.deadline, self.cancelled) {
                self.progress.borrow_mut().fail(error.clone());
                return Err(error);
            }
            if self.done {
                return Ok(None);
            }
            if let Some(row) = self.rows.next() {
                if let Err(error) = self.accept_row(&row) {
                    self.progress.borrow_mut().fail(error.clone());
                    return Err(error);
                }
                if let Err(error) = check(self.deadline, self.cancelled) {
                    self.progress.borrow_mut().fail(error.clone());
                    return Err(error);
                }
                return Ok(Some(row));
            }
            if self.page_loaded {
                self.page_loaded = false;
                if let Some(cursor) = self.pending_cursor.take() {
                    self.cursor = Some(cursor);
                    self.page_charge = 0;
                    self.rows = Vec::new().into_iter();
                } else {
                    self.done = true;
                    return Ok(None);
                }
            }
            self.fetch_page().map_err(|error| {
                self.progress.borrow_mut().fail(error.clone());
                error
            })?;
        }
    }

    fn accept_row(&mut self, row: &SourceFoundationRecordFact) -> Result<(), ItemRefusal> {
        let (key, ordinal) = fact_sort_fields(self.collection, row)?;
        if let Some(key) = key {
            if self
                .last_key
                .as_deref()
                .is_some_and(|previous| previous > key)
                || (self.last_key.as_deref() == Some(key)
                    && self
                        .last_ordinal
                        .is_some_and(|previous| ordinal <= previous))
            {
                return Err(ItemRefusal::Source(
                    "Biblio stored fact cursor order changed".into(),
                ));
            }
            let old_key_capacity = self.last_key.as_ref().map_or(0, String::capacity);
            let requested_key_capacity = key.len();
            let cursor_bytes = self
                .cursor
                .as_ref()
                .map_or(0, |cursor| cursor.as_bytes().len())
                .checked_add(
                    self.pending_cursor
                        .as_ref()
                        .map_or(0, |cursor| cursor.as_bytes().len()),
                )
                .ok_or(ItemRefusal::Budget)?;
            let total = self
                .base_state_bytes
                .checked_add(self.page_charge)
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
                .and_then(|bytes| bytes.checked_add(cursor_bytes))
                .and_then(|bytes| bytes.checked_add(old_key_capacity))
                .and_then(|bytes| bytes.checked_add(requested_key_capacity))
                .ok_or(ItemRefusal::Budget)?;
            if total > self.total_state_limit {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio sorted-key cursor state",
                    used: Some(total as u64),
                    limit: Some(self.total_state_limit as u64),
                });
            }
            check(self.deadline, self.cancelled)?;
            let mut next_key = String::new();
            next_key
                .try_reserve_exact(key.len())
                .map_err(|_| ItemRefusal::Budget)?;
            let actual_total = self
                .base_state_bytes
                .checked_add(self.page_charge)
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
                .and_then(|bytes| bytes.checked_add(cursor_bytes))
                .and_then(|bytes| bytes.checked_add(old_key_capacity))
                .and_then(|bytes| bytes.checked_add(next_key.capacity()))
                .ok_or(ItemRefusal::Budget)?;
            if actual_total > self.total_state_limit {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio sorted-key allocation overlap",
                    used: Some(actual_total as u64),
                    limit: Some(self.total_state_limit as u64),
                });
            }
            next_key.push_str(key);
            check(self.deadline, self.cancelled)?;
            self.last_key = Some(next_key);
            self.last_ordinal = Some(ordinal);
            let mut progress = self.progress.borrow_mut();
            progress.peak_state_bytes = progress.peak_state_bytes.max(actual_total);
        } else {
            if self
                .last_ordinal
                .is_some_and(|previous| ordinal <= previous)
            {
                return Err(ItemRefusal::Source(
                    "Biblio source-order cursor ordinal changed".into(),
                ));
            }
            self.last_ordinal = Some(ordinal);
        }
        let mut coverage = self.coverage.borrow_mut();
        match row {
            SourceFoundationRecordFact::Observation(row) => {
                let ordered = coverage
                    .observations
                    .get_or_insert_with(OrderedRecordDigest::new);
                ordered.add(row.ordinal, &row.observation)?;
            }
            SourceFoundationRecordFact::GlobalId(fact) => {
                let tag = match fact.carrier {
                    SourceFoundationRecordIdCarrier::Standalone => "standalone",
                    SourceFoundationRecordIdCarrier::NativePacket => "native-packet",
                };
                coverage.facts.add(hash_record_fact(
                    "global-id",
                    fact.ordinal,
                    &[&fact.id, &fact.kind, &fact.path, tag],
                )?)?;
            }
            SourceFoundationRecordFact::LinkUri(fact) => {
                coverage.facts.add(hash_record_fact(
                    "link-uri",
                    fact.ordinal,
                    &[&fact.uri, &fact.id, &fact.path],
                )?)?;
            }
            SourceFoundationRecordFact::TypedIdRef(fact) => {
                coverage.facts.add(hash_record_fact(
                    "typed-id-ref",
                    fact.ordinal,
                    &[&fact.target_id, &fact.expected_kind, &fact.from_path],
                )?)?;
            }
            SourceFoundationRecordFact::PathReference(fact) => {
                coverage.facts.add(hash_record_fact(
                    "path-reference",
                    fact.ordinal,
                    &[
                        &fact.from_path,
                        &fact.target_path,
                        match fact.check {
                            PathReferenceCheck::RepoExistsIfToS => "repo-exists-if-tos",
                            PathReferenceCheck::FileIfToS => "file-if-tos",
                        },
                    ],
                )?)?;
            }
        }
        Ok(())
    }
}

fn fact_sort_fields<'a>(
    collection: SourceFoundationRecordFactCollection,
    row: &'a SourceFoundationRecordFact,
) -> Result<(Option<&'a str>, u64), ItemRefusal> {
    match (collection, row) {
        (
            SourceFoundationRecordFactCollection::Observations,
            SourceFoundationRecordFact::Observation(row),
        ) => Ok((None, row.ordinal)),
        (
            SourceFoundationRecordFactCollection::GlobalIdFacts,
            SourceFoundationRecordFact::GlobalId(row),
        ) => Ok((Some(&row.id), row.ordinal)),
        (
            SourceFoundationRecordFactCollection::LinkUriFacts,
            SourceFoundationRecordFact::LinkUri(row),
        ) => Ok((Some(&row.uri), row.ordinal)),
        (
            SourceFoundationRecordFactCollection::TypedIdRefFacts,
            SourceFoundationRecordFact::TypedIdRef(row),
        ) => Ok((Some(&row.target_id), row.ordinal)),
        (
            SourceFoundationRecordFactCollection::PathReferenceFacts,
            SourceFoundationRecordFact::PathReference(row),
        ) => Ok((None, row.ordinal)),
        _ => Err(ItemRefusal::Source(
            "Biblio fact page has the wrong collection".into(),
        )),
    }
}

struct GlobalIdPageIterator<'a>(RecordFactPageReader<'a>);

impl Iterator for GlobalIdPageIterator<'_> {
    type Item = GlobalIdFact;
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next_fact() {
            Ok(Some(SourceFoundationRecordFact::GlobalId(row))) => {
                let (_, fact): (u64, GlobalIdFact) = row.into();
                Some(fact)
            }
            Ok(None) => None,
            _ => None,
        }
    }
}

struct LinkUriPageIterator<'a>(RecordFactPageReader<'a>);

impl Iterator for LinkUriPageIterator<'_> {
    type Item = LinkUriFact;
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next_fact() {
            Ok(Some(SourceFoundationRecordFact::LinkUri(row))) => {
                let (_, fact): (u64, LinkUriFact) = row.into();
                Some(fact)
            }
            Ok(None) => None,
            _ => None,
        }
    }
}

struct TypedIdRefPageIterator<'a>(RecordFactPageReader<'a>);

impl Iterator for TypedIdRefPageIterator<'_> {
    type Item = TypedIdRefFact;
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next_fact() {
            Ok(Some(SourceFoundationRecordFact::TypedIdRef(row))) => {
                let (_, fact): (u64, TypedIdRefFact) = row.into();
                Some(fact)
            }
            Ok(None) => None,
            _ => None,
        }
    }
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
    let mut total = empty_schema_cost();
    for diagnostic in current {
        add_schema_diagnostic_cost(&mut total, diagnostic)?;
    }
    for profile in retained {
        for diagnostic in &profile.schema_diagnostics {
            add_schema_diagnostic_cost(&mut total, diagnostic)?;
        }
    }
    finish_schema_cost(
        total,
        executor,
        initial_executions,
        initial_diagnostic_issues,
    )
    .map(Some)
}

fn empty_schema_cost() -> SourceCutRecordSchemaCost {
    SourceCutRecordSchemaCost {
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
    }
}

fn add_schema_diagnostic_cost(
    total: &mut SourceCutRecordSchemaCost,
    diagnostic: &SourceCutSchemaDiagnostic,
) -> Result<(), ItemRefusal> {
    total.completed_exchanges = total
        .completed_exchanges
        .checked_add(1)
        .ok_or(ItemRefusal::Budget)?;
    total.issue_count = total
        .issue_count
        .checked_add(
            u64::try_from(diagnostic.unit.report.issues.len()).map_err(|_| ItemRefusal::Budget)?,
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
}

fn finish_schema_cost(
    total: SourceCutRecordSchemaCost,
    executor: &BiblioRecordExecutor,
    initial_executions: usize,
    initial_diagnostic_issues: usize,
) -> Result<SourceCutRecordSchemaCost, ItemRefusal> {
    let expected_exchanges = executor
        .executions
        .checked_sub(initial_executions)
        .ok_or(ItemRefusal::Budget)?;
    let expected_issues = executor
        .diagnostic_issues_used
        .checked_sub(initial_diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    if total.completed_exchanges
        != u64::try_from(expected_exchanges).map_err(|_| ItemRefusal::Budget)?
        || total.issue_count != u64::try_from(expected_issues).map_err(|_| ItemRefusal::Budget)?
    {
        return Err(ItemRefusal::Unsupported(
            "record schema diagnostics cost coverage incomplete".into(),
        ));
    }
    Ok(total)
}

#[derive(Clone, Copy)]
struct SinkBudgetFailure {
    check: &'static str,
    used: Option<u64>,
    limit: Option<u64>,
}

impl SinkBudgetFailure {
    fn encode(self) -> String {
        let used = self
            .used
            .map_or_else(|| "-".to_owned(), |value| value.to_string());
        let limit = self
            .limit
            .map_or_else(|| "-".to_owned(), |value| value.to_string());
        format!("budget-check|{}|{used}|{limit}", self.check)
    }
}

struct BoundedSink<'a> {
    rows: Vec<RecordObservation>,
    bytes: usize,
    cap: usize,
    issues: usize,
    max_issues: usize,
    budget_failure: Option<SinkBudgetFailure>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
// The protected sink checks cancellation and deadline on each observation.
impl BoundedSink<'_> {
    fn budget_failure(
        &mut self,
        check: &'static str,
        used: Option<usize>,
        limit: usize,
    ) -> RecordRuleError {
        self.budget_failure = Some(SinkBudgetFailure {
            check,
            used: used.and_then(|value| u64::try_from(value).ok()),
            limit: u64::try_from(limit).ok(),
        });
        RecordRuleError::Budget { code: check }
    }

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
                let used = self.issues.checked_add(1);
                let limit = self.max_issues;
                return Err(self.budget_failure("record_issue_sink", used, limit));
            }
            self.issues += 1;
        }
        let Some(cost) = format!("{row:?}").len().checked_add(64) else {
            let limit = self.cap;
            return Err(self.budget_failure("biblio_sink", None, limit));
        };
        let Some(next_bytes) = self.bytes.checked_add(cost) else {
            let limit = self.cap;
            return Err(self.budget_failure("biblio_sink", None, limit));
        };
        if next_bytes > self.cap {
            let limit = self.cap;
            return Err(self.budget_failure("biblio_sink", Some(next_bytes), limit));
        }
        self.bytes = next_bytes;
        self.rows.push(row);
        Ok(())
    }
}

impl RecordSink for BoundedSink<'_> {
    fn push(&mut self, row: RecordObservation) -> Result<(), String> {
        self.emit(row).map_err(|error| match error {
            RecordRuleError::Budget { .. } => self
                .budget_failure
                .take()
                .map(SinkBudgetFailure::encode)
                .unwrap_or_else(|| "budget".into()),
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

fn retain_stored_schema_diagnostics(
    executor: &mut BiblioRecordExecutor,
    before_issue: usize,
    accumulated_cost: &mut SourceCutRecordSchemaCost,
    sink: &mut StoredRecordObservationSink<'_>,
) -> Result<(), ItemRefusal> {
    let diagnostics = executor.take_schema_diagnostics(before_issue);
    let vector_state_bytes = std::mem::size_of::<Vec<SourceCutSchemaDiagnostic>>()
        .checked_add(
            diagnostics
                .capacity()
                .checked_mul(std::mem::size_of::<SourceCutSchemaDiagnostic>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    let mut batch_state_bytes = 0usize;
    for diagnostic in &diagnostics {
        batch_state_bytes = batch_state_bytes
            .checked_add(diagnostic.accounted_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
    }
    let transient_state_bytes = sink
        .state_bytes
        .checked_add(sink.live_page_reservation)
        .and_then(|bytes| bytes.checked_add(vector_state_bytes))
        .and_then(|bytes| bytes.checked_add(batch_state_bytes))
        .ok_or(ItemRefusal::Budget)?;
    if transient_state_bytes > sink.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio terminal schema diagnostics batch state",
            used: Some(transient_state_bytes as u64),
            limit: Some(sink.max_state_bytes as u64),
        });
    }
    {
        let mut progress = sink.progress.borrow_mut();
        progress.peak_state_bytes = progress.peak_state_bytes.max(transient_state_bytes);
    }
    for diagnostic in diagnostics {
        check(sink.deadline, sink.cancelled)?;
        sink.store.with(|store| {
            store.record_schema_diagnostic(
                diagnostic.evaluation_ordinal,
                &diagnostic,
                sink.deadline,
                sink.cancelled,
            )
        })?;
        add_schema_diagnostic_cost(accumulated_cost, &diagnostic)?;
        check(sink.deadline, sink.cancelled)?;
    }
    Ok(())
}

fn refresh_stored_sink_issue_limit(
    sink: &mut StoredRecordObservationSink<'_>,
    total_issue_limit: usize,
    diagnostic_issues_used: usize,
) -> Result<(), ItemRefusal> {
    let record_issue_limit = total_issue_limit
        .checked_sub(diagnostic_issues_used)
        .ok_or(ItemRefusal::Budget)?;
    if sink.progress.borrow().issue_count > record_issue_limit {
        return Err(ItemRefusal::BudgetCheck {
            check: "stored record issues after schema diagnostics",
            used: u64::try_from(sink.progress.borrow().issue_count).ok(),
            limit: u64::try_from(record_issue_limit).ok(),
        });
    }
    sink.max_issues = record_issue_limit;
    Ok(())
}

fn current_input_member(
    input: &dyn SourceCutInput,
    path: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    used: &mut u64,
    retained_state_bytes: usize,
) -> Result<Vec<u8>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let relative_path_state = std::mem::size_of::<RelativePath>()
        .checked_add(path.len())
        .ok_or(ItemRefusal::Budget)?;
    let path_parse_peak = retained_state_bytes
        .checked_add(relative_path_state)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Option<Vec<u8>>>()))
        .ok_or(ItemRefusal::Budget)?;
    if path_parse_peak > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio current-member path parse state",
            used: Some(path_parse_peak as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    RelativePath::parse(path)
        .map_err(|_| ItemRefusal::Source("Biblio current-member path is invalid".into()))?;
    let mut raw = None;
    input.with_current_member(
        path,
        limits.max_member_bytes,
        limits.deadline,
        cancelled,
        &mut |meta, bytes| {
            check(limits.deadline, cancelled)?;
            if raw.is_some()
                || meta.path != path
                || bytes.len() > limits.max_member_bytes
                || u64::try_from(bytes.len()).ok() != Some(meta.size_bytes)
            {
                return Err(ItemRefusal::Source(
                    "Biblio current-member adapter returned an inexact member".into(),
                ));
            }
            let overlap = bytes
                .len()
                .checked_mul(2)
                .and_then(|both| both.checked_add(std::mem::size_of::<Vec<u8>>()))
                .and_then(|both| both.checked_add(std::mem::size_of::<Option<Vec<u8>>>()))
                .and_then(|both| both.checked_add(relative_path_state))
                .and_then(|both| both.checked_add(retained_state_bytes))
                .ok_or(ItemRefusal::Budget)?;
            if overlap > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio borrowed and owned source member overlap",
                    used: Some(overlap as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            account(used, bytes.len(), limits.max_total_bytes)?;
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| ItemRefusal::Budget)?;
            let actual_overlap = owned
                .capacity()
                .checked_add(bytes.len())
                .and_then(|both| both.checked_add(std::mem::size_of::<Vec<u8>>()))
                .and_then(|both| both.checked_add(std::mem::size_of::<Option<Vec<u8>>>()))
                .and_then(|both| both.checked_add(relative_path_state))
                .and_then(|both| both.checked_add(retained_state_bytes))
                .ok_or(ItemRefusal::Budget)?;
            if actual_overlap > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio borrowed and owned source member capacity",
                    used: Some(actual_overlap as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            owned.extend_from_slice(bytes);
            check(limits.deadline, cancelled)?;
            raw = Some(owned);
            Ok(())
        },
    )?;
    check(limits.deadline, cancelled)?;
    raw.ok_or_else(|| {
        ItemRefusal::Source(format!("Biblio adapter omitted required member: {path}"))
    })
}

fn verify_fact_page_coverage(
    progress: &BiblioRecordProgress,
    collection: SourceFoundationRecordFactCollection,
    actual: &RecordFactPageCoverage,
) -> Result<(), ItemRefusal> {
    let slot = record_fact_slot(collection).ok_or_else(|| {
        ItemRefusal::Source("Biblio fact coverage requested for ordered observations".into())
    })?;
    if actual.facts != progress.expected_facts[slot] {
        return Err(ItemRefusal::Source(format!(
            "Biblio {:?} fact cursor coverage differs from emitted observations",
            collection
        )));
    }
    Ok(())
}

/// Current-only Records kernel for a caller-authenticated input and indexed
/// store. It reuses the existing registry, schema, classification, RecordFamily
/// and RecordGlobalJoin predicates while keeping observations and joins in
/// bounded storage pages. Input coverage returned by `SourceCutInput` is only
/// checked transport; the adapter remains responsible for authenticating its
/// own captured identity and complete membership digest.
pub fn inspect_records_from_input_stored(
    input: &dyn SourceCutInput,
    store: &mut dyn SourceFoundationRecordsStore,
    limits: ItemLimits,
    fact_budget: SourceCutRecordFactBudget,
    page_budget: SourceFoundationRecordsPageBudget,
    cancelled: &AtomicBool,
    executor: &mut BiblioRecordExecutor,
) -> Result<SourceCutStreamedRecordSummary, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if fact_budget.max_facts == 0 || fact_budget.max_encoded_bytes == 0 {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio caller fact budget",
            used: Some(0),
            limit: Some(fact_budget.max_facts.min(fact_budget.max_encoded_bytes)),
        });
    }
    let fact_budget = RecordFactBudget {
        max_facts: fact_budget.max_facts,
        max_encoded_bytes: fact_budget.max_encoded_bytes,
    };
    let store_handle = BiblioStoreHandle::new(store);
    let initial_executions = executor.executions;
    let initial_diagnostic_issues = executor.diagnostic_issues_used;
    let mut used = 0u64;
    let fixed_header_state = std::mem::size_of::<SourceCutRecordUsage>()
        .checked_add(std::mem::size_of::<SourceCutStreamedRecordSummary>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<SourceCutSchemaDiagnostic>>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>()))
        .ok_or(ItemRefusal::Budget)?;
    let registry = current_input_member(
        input,
        REGISTRY,
        limits,
        cancelled,
        &mut used,
        fixed_header_state,
    )?;
    let registry_retained_state = fixed_header_state
        .checked_add(registry.len())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<u8>>()))
        .ok_or(ItemRefusal::Budget)?;
    let contract = current_input_member(
        input,
        REGISTRY_SCHEMA,
        limits,
        cancelled,
        &mut used,
        registry_retained_state,
    )?;
    let mut base_state = registry
        .len()
        .checked_add(contract.len())
        .and_then(|bytes| bytes.checked_add(fixed_header_state))
        .and_then(|bytes| {
            std::mem::size_of::<Vec<u8>>()
                .checked_mul(2)
                .and_then(|headers| bytes.checked_add(headers))
        })
        .ok_or(ItemRefusal::Budget)?;
    if base_state > limits.max_state_bytes {
        return Err(ItemRefusal::Budget);
    }

    let mut schema_paths = Vec::<String>::new();
    let mut metadata_members = 0u64;
    let mut metadata_bytes = 0u64;
    let mut metadata_state = std::mem::size_of::<Vec<String>>()
        .checked_add(std::mem::size_of::<String>())
        .ok_or(ItemRefusal::Budget)?;
    let mut schema_path_bytes = 0usize;
    let mut previous_metadata_path = String::new();
    input.for_each_current_member_meta(limits.deadline, cancelled, &mut |meta| {
        check(limits.deadline, cancelled)?;
        let path_parse_state = std::mem::size_of::<RelativePath>()
            .checked_add(meta.path.len())
            .ok_or(ItemRefusal::Budget)?;
        let path_parse_peak = base_state
            .checked_add(metadata_state)
            .and_then(|bytes| bytes.checked_add(previous_metadata_path.capacity()))
            .and_then(|bytes| bytes.checked_add(path_parse_state))
            .ok_or(ItemRefusal::Budget)?;
        if path_parse_peak > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio metadata path parse precharge",
                used: Some(path_parse_peak as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        RelativePath::parse(meta.path).map_err(|_| {
            ItemRefusal::Source("Biblio metadata contained an invalid relative path".into())
        })?;
        if !previous_metadata_path.is_empty() && meta.path <= previous_metadata_path.as_str() {
            return Err(ItemRefusal::Source(
                "Biblio source metadata paths are not strictly ordered".into(),
            ));
        }
        metadata_members = metadata_members.checked_add(1).ok_or(ItemRefusal::Budget)?;
        metadata_bytes = metadata_bytes
            .checked_add(meta.size_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let previous_capacity = previous_metadata_path.capacity();
        let previous_requested_capacity = previous_capacity.max(meta.path.len());
        let previous_overlap_capacity = if previous_requested_capacity > previous_capacity {
            previous_capacity
                .checked_add(previous_requested_capacity)
                .ok_or(ItemRefusal::Budget)?
        } else {
            previous_capacity
        };
        if meta.path.starts_with("ToS/contracts/")
            && meta.path.ends_with(".schema.json")
            && meta.path != REGISTRY_SCHEMA
        {
            let next_len = schema_paths
                .len()
                .checked_add(1)
                .ok_or(ItemRefusal::Budget)?;
            let old_capacity = schema_paths.capacity();
            let requested_capacity = old_capacity.max(next_len);
            let requested_slots = requested_capacity
                .checked_mul(std::mem::size_of::<String>())
                .ok_or(ItemRefusal::Budget)?;
            let vector_reallocation_overlap = if requested_capacity > old_capacity {
                requested_slots
            } else {
                0
            };
            let local_schema_path = std::mem::size_of::<String>()
                .checked_add(meta.path.len())
                .ok_or(ItemRefusal::Budget)?;
            let metadata_peak = base_state
                .checked_add(metadata_state)
                .and_then(|bytes| bytes.checked_add(vector_reallocation_overlap))
                .and_then(|bytes| bytes.checked_add(local_schema_path))
                .and_then(|bytes| bytes.checked_add(previous_overlap_capacity))
                .ok_or(ItemRefusal::Budget)?;
            if metadata_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio source metadata state",
                    used: Some(metadata_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            schema_paths
                .try_reserve_exact(1)
                .map_err(|_| ItemRefusal::Budget)?;
            let next_capacity = schema_paths.capacity();
            let actual_metadata_state = std::mem::size_of::<Vec<String>>()
                .checked_add(std::mem::size_of::<String>())
                .and_then(|bytes| {
                    bytes.checked_add(next_capacity.checked_mul(std::mem::size_of::<String>())?)
                })
                .and_then(|bytes| bytes.checked_add(schema_path_bytes))
                .ok_or(ItemRefusal::Budget)?;
            let actual_vector_overlap = if next_capacity > old_capacity {
                next_capacity
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or(ItemRefusal::Budget)?
            } else {
                0
            };
            let actual_peak = base_state
                .checked_add(metadata_state)
                .and_then(|bytes| bytes.checked_add(actual_vector_overlap))
                .and_then(|bytes| bytes.checked_add(local_schema_path))
                .and_then(|bytes| bytes.checked_add(previous_overlap_capacity))
                .ok_or(ItemRefusal::Budget)?;
            if actual_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio source metadata vector capacity",
                    used: Some(actual_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            previous_metadata_path.clear();
            previous_metadata_path
                .try_reserve_exact(meta.path.len())
                .map_err(|_| ItemRefusal::Budget)?;
            let previous_actual_overlap = if previous_metadata_path.capacity() > previous_capacity {
                previous_capacity
                    .checked_add(previous_metadata_path.capacity())
                    .ok_or(ItemRefusal::Budget)?
            } else {
                previous_metadata_path.capacity()
            };
            let actual_peak = base_state
                .checked_add(actual_metadata_state)
                .and_then(|bytes| bytes.checked_add(local_schema_path))
                .and_then(|bytes| bytes.checked_add(previous_actual_overlap))
                .ok_or(ItemRefusal::Budget)?;
            if actual_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio source metadata simultaneous path state",
                    used: Some(actual_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            let mut path = String::new();
            path.try_reserve_exact(meta.path.len())
                .map_err(|_| ItemRefusal::Budget)?;
            path.push_str(meta.path);
            previous_metadata_path.push_str(meta.path);
            schema_paths.push(path);
            schema_path_bytes = schema_path_bytes
                .checked_add(meta.path.len())
                .ok_or(ItemRefusal::Budget)?;
            metadata_state = actual_metadata_state
                .checked_add(meta.path.len())
                .ok_or(ItemRefusal::Budget)?;
        } else {
            let metadata_peak = base_state
                .checked_add(metadata_state)
                .and_then(|bytes| bytes.checked_add(previous_overlap_capacity))
                .ok_or(ItemRefusal::Budget)?;
            if metadata_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio source metadata last-path precharge",
                    used: Some(metadata_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            previous_metadata_path.clear();
            previous_metadata_path
                .try_reserve_exact(meta.path.len())
                .map_err(|_| ItemRefusal::Budget)?;
            let actual_peak = base_state
                .checked_add(metadata_state)
                .and_then(|bytes| bytes.checked_add(previous_metadata_path.capacity()))
                .ok_or(ItemRefusal::Budget)?;
            if actual_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio source metadata last-path state",
                    used: Some(actual_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            previous_metadata_path.push_str(meta.path);
        }
        Ok(())
    })?;
    check(limits.deadline, cancelled)?;

    drop(previous_metadata_path);
    metadata_state = metadata_state
        .checked_sub(std::mem::size_of::<String>())
        .ok_or(ItemRefusal::Budget)?;
    let mut schemas = Vec::<(String, Vec<u8>)>::new();
    let schema_slots = schema_paths
        .len()
        .checked_mul(std::mem::size_of::<(String, Vec<u8>)>())
        .ok_or(ItemRefusal::Budget)?;
    let mut schemas_state = std::mem::size_of::<Vec<(String, Vec<u8>)>>()
        .checked_add(schema_slots)
        .ok_or(ItemRefusal::Budget)?;
    let schema_preallocation_peak = base_state
        .checked_add(metadata_state)
        .and_then(|bytes| bytes.checked_add(schemas_state))
        .ok_or(ItemRefusal::Budget)?;
    if schema_preallocation_peak > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio schema row capacity precharge",
            used: Some(schema_preallocation_peak as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    schemas
        .try_reserve_exact(schema_paths.len())
        .map_err(|_| ItemRefusal::Budget)?;
    schemas_state = std::mem::size_of::<Vec<(String, Vec<u8>)>>()
        .checked_add(
            schemas
                .capacity()
                .checked_mul(std::mem::size_of::<(String, Vec<u8>)>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    let actual_preallocation_peak = base_state
        .checked_add(metadata_state)
        .and_then(|bytes| bytes.checked_add(schemas_state))
        .ok_or(ItemRefusal::Budget)?;
    if actual_preallocation_peak > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio schema row capacity",
            used: Some(actual_preallocation_peak as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }
    for path in schema_paths.iter() {
        check(limits.deadline, cancelled)?;
        let path_clone_state = std::mem::size_of::<String>()
            .checked_add(path.len())
            .ok_or(ItemRefusal::Budget)?;
        let path_clone_peak = base_state
            .checked_add(metadata_state)
            .and_then(|bytes| bytes.checked_add(schemas_state))
            .and_then(|bytes| bytes.checked_add(path_clone_state))
            .ok_or(ItemRefusal::Budget)?;
        if path_clone_peak > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio source schema path clone precharge",
                used: Some(path_clone_peak as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        let mut owned_path = String::new();
        owned_path
            .try_reserve_exact(path.len())
            .map_err(|_| ItemRefusal::Budget)?;
        owned_path.push_str(path);
        let raw = current_input_member(input, path, limits, cancelled, &mut used, path_clone_peak)?;
        schemas_state = schemas_state
            .checked_add(path.len())
            .and_then(|bytes| bytes.checked_add(raw.len()))
            .ok_or(ItemRefusal::Budget)?;
        let schema_profile_state = base_state
            .checked_add(metadata_state)
            .and_then(|bytes| bytes.checked_add(schemas_state))
            .ok_or(ItemRefusal::Budget)?;
        if schema_profile_state > limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "Biblio source schema profile state",
                used: Some(schema_profile_state as u64),
                limit: Some(limits.max_state_bytes as u64),
            });
        }
        schemas.push((owned_path, raw));
    }
    drop(schema_paths);
    base_state = base_state
        .checked_add(schemas_state)
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<StoredRecordObservationSink<'static>>())
        })
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<BiblioRecordProgress>()))
        .ok_or(ItemRefusal::Budget)?;
    if base_state > limits.max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio current profile state",
            used: Some(base_state as u64),
            limit: Some(limits.max_state_bytes as u64),
        });
    }

    let (resources, root, set, _) =
        RecordFamily::registry_schema_plan(&contract, &registry, executor.profile)
            .map_err(record_error)?;
    let evidence = executor.evaluate(
        &resources, REGISTRY, &root, &registry, set, limits, cancelled,
    )?;
    let mut schema_cost = empty_schema_cost();
    let progress = Rc::new(RefCell::new(BiblioRecordProgress::new(base_state)));
    let mut sink = StoredRecordObservationSink {
        store: store_handle.clone(),
        progress: progress.clone(),
        deadline: limits.deadline,
        cancelled,
        state_bytes: base_state,
        max_state_bytes: limits.max_state_bytes,
        max_issues: limits.max_issues,
        live_page_reservation: 0,
    };
    retain_stored_schema_diagnostics(executor, 0, &mut schema_cost, &mut sink)?;
    refresh_stored_sink_issue_limit(
        &mut sink,
        limits.max_issues,
        executor
            .diagnostic_issues_used
            .checked_sub(initial_diagnostic_issues)
            .ok_or(ItemRefusal::Budget)?,
    )?;
    let mut family = RecordFamily::new_with_bounded_registry(
        &registry,
        &contract,
        schemas.iter().map(|(path, raw)| RecordSchema { path, raw }),
        executor.profile,
        fact_budget,
        &evidence,
    )
    .map_err(record_error)?;
    family
        .emit_registry_read(&mut sink)
        .map_err(|error| stored_record_rule_error(&progress, error))?;
    check_stored_record_progress(&progress)?;

    let mut scan_members = 0u64;
    let mut scan_bytes = 0u64;
    let mut previous_path = String::new();
    let coverage =
        input.for_each_current_member(limits.deadline, cancelled, &mut |meta, raw| {
            check(limits.deadline, cancelled)?;
            let raw_len = u64::try_from(raw.len()).map_err(|_| ItemRefusal::Budget)?;
            let path_parse_state = std::mem::size_of::<RelativePath>()
                .checked_add(meta.path.len())
                .ok_or(ItemRefusal::Budget)?;
            let path_parse_peak = base_state
                .checked_add(raw.len())
                .and_then(|bytes| bytes.checked_add(previous_path.capacity()))
                .and_then(|bytes| bytes.checked_add(path_parse_state))
                .ok_or(ItemRefusal::Budget)?;
            if path_parse_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio streamed path parse and member precharge",
                    used: Some(path_parse_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            RelativePath::parse(meta.path).map_err(|_| {
                ItemRefusal::Source("Biblio stream contained an invalid relative path".into())
            })?;
            if raw.len() > limits.max_member_bytes || raw_len != meta.size_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio current stream member bytes",
                    used: Some(raw_len),
                    limit: Some(meta.size_bytes.min(limits.max_member_bytes as u64)),
                });
            }
            if !previous_path.is_empty() && meta.path <= previous_path.as_str() {
                return Err(ItemRefusal::Source(
                    "Biblio current stream paths are not strictly ordered".into(),
                ));
            }
            scan_members = scan_members.checked_add(1).ok_or(ItemRefusal::Budget)?;
            scan_bytes = scan_bytes.checked_add(raw_len).ok_or(ItemRefusal::Budget)?;
            account(&mut used, raw.len(), limits.max_total_bytes)?;
            let old_path_capacity = previous_path.capacity();
            let requested_path_capacity = old_path_capacity.max(meta.path.len());
            let path_overlap_capacity = if requested_path_capacity > old_path_capacity {
                old_path_capacity
                    .checked_add(requested_path_capacity)
                    .ok_or(ItemRefusal::Budget)?
            } else {
                old_path_capacity
            };
            let path_peak = base_state
                .checked_add(raw.len())
                .and_then(|bytes| bytes.checked_add(path_overlap_capacity))
                .ok_or(ItemRefusal::Budget)?;
            if path_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio stream borrowed member and prior-path overlap",
                    used: Some(path_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            previous_path.clear();
            previous_path
                .try_reserve_exact(meta.path.len())
                .map_err(|_| ItemRefusal::Budget)?;
            let actual_path_peak = base_state
                .checked_add(raw.len())
                .and_then(|bytes| bytes.checked_add(previous_path.capacity()))
                .ok_or(ItemRefusal::Budget)?;
            if actual_path_peak > limits.max_state_bytes {
                return Err(ItemRefusal::BudgetCheck {
                    check: "Biblio stream path buffer state",
                    used: Some(actual_path_peak as u64),
                    limit: Some(limits.max_state_bytes as u64),
                });
            }
            previous_path.push_str(meta.path);
            {
                let mut progress = progress.borrow_mut();
                progress.peak_state_bytes = progress.peak_state_bytes.max(actual_path_peak);
            }
            sink.state_bytes = actual_path_peak;
            let path = meta.path;
            if !path.starts_with("ToS/source-witnesses/") || !path.ends_with(".json") {
                sink.state_bytes = base_state;
                return Ok(());
            }
            let basename = path.rsplit('/').next().unwrap_or("");
            let semantic =
                basename.starts_with("semantic-annotation") && basename.ends_with(".json");
            let carrier = family
                .classify_current_member(path, raw)
                .map_err(record_error)?;
            if carrier.is_none() && !semantic {
                sink.state_bytes = base_state;
                return Ok(());
            }
            match family.member_schema_plan(path, raw) {
                Ok(plan) => {
                    let before_issue = progress.borrow().issue_count;
                    let route = executor.evaluate(
                        &plan.resources,
                        path,
                        &plan.route_uri,
                        raw,
                        plan.schema_set_digest,
                        limits,
                        cancelled,
                    )?;
                    retain_stored_schema_diagnostics(
                        executor,
                        before_issue,
                        &mut schema_cost,
                        &mut sink,
                    )?;
                    refresh_stored_sink_issue_limit(
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
                        raw,
                        plan.schema_set_digest,
                        limits,
                        cancelled,
                    )?;
                    retain_stored_schema_diagnostics(
                        executor,
                        before_issue,
                        &mut schema_cost,
                        &mut sink,
                    )?;
                    refresh_stored_sink_issue_limit(
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
                            raw,
                            &BoundedMemberSchemaEvidence { route, common },
                            &mut sink,
                        )
                        .map_err(|error| stored_record_rule_error(&progress, error))?;
                    check_stored_record_progress(&progress)?;
                }
                Err(RecordRuleError::Unsupported {
                    code: "unrecognized_record_basename",
                    ..
                }) => {
                    let plan = family.native_schema_plan(path, raw).map_err(record_error)?;
                    let verdict = executor.evaluate(
                        &plan.resources,
                        path,
                        &plan.root_uri,
                        raw,
                        plan.schema_set_digest,
                        limits,
                        cancelled,
                    )?;
                    let before_issue = progress.borrow().issue_count;
                    retain_stored_schema_diagnostics(
                        executor,
                        before_issue,
                        &mut schema_cost,
                        &mut sink,
                    )?;
                    refresh_stored_sink_issue_limit(
                        &mut sink,
                        limits.max_issues,
                        executor
                            .diagnostic_issues_used
                            .checked_sub(initial_diagnostic_issues)
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    family
                        .inspect_native_with_bounded_schema(path, raw, &verdict, &mut sink)
                        .map_err(|error| stored_record_rule_error(&progress, error))?;
                    check_stored_record_progress(&progress)?;
                }
                Err(error) => return Err(record_error(error)),
            }
            if let Some(carrier) = carrier {
                let value: serde_json::Value = serde_json::from_slice(raw).map_err(|_| {
                    ItemRefusal::Unsupported("record decoded representation".into())
                })?;
                let record = BiblioCurrentRecord {
                    path: path.into(),
                    kind: carrier.kind,
                    value,
                };
                sink.store_current_record_first(&carrier.id, &record)?;
            }
            sink.state_bytes = base_state;
            check_stored_record_progress(&progress)
        })?;
    check(limits.deadline, cancelled)?;
    drop(previous_path);
    if scan_members != metadata_members
        || scan_bytes != metadata_bytes
        || coverage.member_count() != scan_members
        || coverage.source_bytes_read() != scan_bytes
    {
        return Err(ItemRefusal::Source(
            "Biblio current member coverage differs from metadata and observed stream".into(),
        ));
    }
    input.verify_current_fence(&coverage, limits.deadline, cancelled)?;
    check_stored_record_progress(&progress)?;
    let membership = coverage.membership();

    let mut global_issues = 0u64;
    let id_reader_base = reserve_record_fact_reader_state(&mut sink, page_budget, 1)?;
    let id_reader = RecordFactPageReader::new(
        store_handle.clone(),
        SourceFoundationRecordFactCollection::GlobalIdFacts,
        page_budget,
        limits.max_state_bytes,
        id_reader_base,
        limits.deadline,
        cancelled,
        progress.clone(),
    )?;
    let id_coverage = id_reader.coverage();
    global_issues = global_issues
        .checked_add(
            RecordGlobalJoin::check_id_collisions(
                GlobalIdPageIterator(id_reader),
                fact_budget,
                &mut sink,
            )
            .map_err(|error| stored_record_rule_error(&progress, error))?,
        )
        .ok_or(ItemRefusal::Budget)?;
    check_stored_record_progress(&progress)?;
    verify_fact_page_coverage(
        &progress.borrow(),
        SourceFoundationRecordFactCollection::GlobalIdFacts,
        &id_coverage.borrow(),
    )?;
    release_record_fact_reader_state(&mut sink);

    let uri_reader_base = reserve_record_fact_reader_state(&mut sink, page_budget, 1)?;
    let uri_reader = RecordFactPageReader::new(
        store_handle.clone(),
        SourceFoundationRecordFactCollection::LinkUriFacts,
        page_budget,
        limits.max_state_bytes,
        uri_reader_base,
        limits.deadline,
        cancelled,
        progress.clone(),
    )?;
    let uri_coverage = uri_reader.coverage();
    global_issues = global_issues
        .checked_add(
            RecordGlobalJoin::check_link_uri_collisions(
                LinkUriPageIterator(uri_reader),
                fact_budget,
                &mut sink,
            )
            .map_err(|error| stored_record_rule_error(&progress, error))?,
        )
        .ok_or(ItemRefusal::Budget)?;
    check_stored_record_progress(&progress)?;
    verify_fact_page_coverage(
        &progress.borrow(),
        SourceFoundationRecordFactCollection::LinkUriFacts,
        &uri_coverage.borrow(),
    )?;
    release_record_fact_reader_state(&mut sink);

    let reference_reader_base = reserve_record_fact_reader_state(&mut sink, page_budget, 2)?;
    let owner_reader = RecordFactPageReader::new(
        store_handle.clone(),
        SourceFoundationRecordFactCollection::GlobalIdFacts,
        page_budget,
        limits.max_state_bytes,
        reference_reader_base,
        limits.deadline,
        cancelled,
        progress.clone(),
    )?;
    let owner_coverage = owner_reader.coverage();
    let reference_reader = RecordFactPageReader::new(
        store_handle.clone(),
        SourceFoundationRecordFactCollection::TypedIdRefFacts,
        page_budget,
        limits.max_state_bytes,
        reference_reader_base,
        limits.deadline,
        cancelled,
        progress.clone(),
    )?;
    let reference_coverage = reference_reader.coverage();
    global_issues = global_issues
        .checked_add(
            RecordGlobalJoin::check_typed_references(
                GlobalIdPageIterator(owner_reader),
                TypedIdRefPageIterator(reference_reader),
                fact_budget,
                &mut sink,
            )
            .map_err(|error| stored_record_rule_error(&progress, error))?,
        )
        .ok_or(ItemRefusal::Budget)?;
    check_stored_record_progress(&progress)?;
    verify_fact_page_coverage(
        &progress.borrow(),
        SourceFoundationRecordFactCollection::GlobalIdFacts,
        &owner_coverage.borrow(),
    )?;
    verify_fact_page_coverage(
        &progress.borrow(),
        SourceFoundationRecordFactCollection::TypedIdRefFacts,
        &reference_coverage.borrow(),
    )?;
    release_record_fact_reader_state(&mut sink);

    let path_reader_base = reserve_record_fact_reader_state(&mut sink, page_budget, 1)?;
    let path_reader = RecordFactPageReader::new(
        store_handle.clone(),
        SourceFoundationRecordFactCollection::PathReferenceFacts,
        page_budget,
        limits.max_state_bytes,
        path_reader_base,
        limits.deadline,
        cancelled,
        progress.clone(),
    )?;
    let path_coverage = path_reader.coverage();
    let mut path_reader = path_reader;
    while let Some(row) = path_reader.next_fact().map_err(|error| {
        progress.borrow_mut().fail(error.clone());
        error
    })? {
        let SourceFoundationRecordFact::PathReference(row) = row else {
            return Err(ItemRefusal::Source(
                "Biblio path fact collection changed".into(),
            ));
        };
        check(limits.deadline, cancelled)?;
        if !row.target_path.starts_with("ToS/") {
            continue;
        }
        let relative = RelativePath::parse(&row.target_path)
            .map_err(|_| ItemRefusal::Unsupported("record reference path".into()))?;
        let presence = input.path_presence(relative.as_str(), limits.deadline, cancelled)?;
        if presence.is_none()
            || (row.check == PathReferenceCheck::FileIfToS
                && presence != Some(SourcePresenceV1::File))
        {
            sink.emit(RecordObservation::Issue {
                path: row.from_path,
                code: "record_path_reference_missing",
            })?;
            global_issues = global_issues.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
    }
    check_stored_record_progress(&progress)?;
    verify_fact_page_coverage(
        &progress.borrow(),
        SourceFoundationRecordFactCollection::PathReferenceFacts,
        &path_coverage.borrow(),
    )?;
    drop(path_reader);
    release_record_fact_reader_state(&mut sink);

    let observation_reader_base = reserve_record_fact_reader_state(&mut sink, page_budget, 1)?;
    let observation_reader = RecordFactPageReader::new(
        store_handle.clone(),
        SourceFoundationRecordFactCollection::Observations,
        page_budget,
        limits.max_state_bytes,
        observation_reader_base,
        limits.deadline,
        cancelled,
        progress.clone(),
    )?;
    let observation_coverage = observation_reader.coverage();
    let mut observation_reader = observation_reader;
    while observation_reader
        .next_fact()
        .map_err(|error| {
            progress.borrow_mut().fail(error.clone());
            error
        })?
        .is_some()
    {}
    check_stored_record_progress(&progress)?;
    let actual_observations = observation_coverage
        .borrow()
        .observations
        .unwrap_or_else(OrderedRecordDigest::new);
    if actual_observations != progress.borrow().expected_observations {
        return Err(ItemRefusal::Source(
            "Biblio observation cursor count or ordered digest differs from emitted rows".into(),
        ));
    }
    drop(observation_reader);
    release_record_fact_reader_state(&mut sink);
    input.verify_current_fence(&coverage, limits.deadline, cancelled)?;
    check(limits.deadline, cancelled)?;

    let family_report = family.finish();
    let family_summary = SourceCutStreamedFamilySummary {
        registry_version: family_report.registry_version,
        registry_sha256: family_report.registry_sha256,
        inspected_members: family_report.inspected_members,
        issue_count: family_report.issue_count,
        emitted_member_fact_count: family_report.emitted_member_fact_count,
        emitted_member_fact_bytes: family_report.emitted_member_fact_bytes,
        enumerated_profile_count: family_report.enumerated_profile_ids.len(),
        skipped_profile_count: family_report.skipped_profile_ids.len(),
        incomplete: family_report.incomplete,
    };
    drop(schemas);
    drop(registry);
    drop(contract);
    let diagnostic_issues = executor
        .diagnostic_issues_used
        .checked_sub(initial_diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    let observed_issue_count = progress
        .borrow()
        .issue_count
        .checked_add(diagnostic_issues)
        .ok_or(ItemRefusal::Budget)?;
    if observed_issue_count > limits.max_issues {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio final issue count",
            used: Some(observed_issue_count as u64),
            limit: Some(limits.max_issues as u64),
        });
    }
    let schema_diagnostics_v2 = if executor.diagnostics_v2.is_some() {
        Some(finish_schema_cost(
            schema_cost,
            executor,
            initial_executions,
            initial_diagnostic_issues,
        )?)
    } else {
        None
    };
    let progress = progress.borrow();
    Ok(SourceCutStreamedRecordSummary {
        current_membership: membership,
        input_coverage: coverage,
        current_member_count: scan_members,
        current_source_bytes: scan_bytes,
        observation_count: progress.expected_observations.count,
        observation_digest: progress.expected_observations.digest,
        global_issues,
        family: family_summary,
        usage: SourceCutRecordUsage {
            source_bytes_read: used,
            accounted_state_upper_bound_bytes: progress.peak_state_bytes,
            observed_issue_count,
            schema_diagnostics_v2,
        },
    })
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
        budget_failure: None,
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
            budget_failure: None,
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

pub(crate) fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source("bibliography cancelled".into()));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
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
pub(crate) fn source_store_cause(error: &tos_source_store::StoreError) -> String {
    // StoreError.detail is owner-authored &'static str; its optional raw IO
    // source may contain paths, so only the typed code and detail digest travel.
    let code = match &error.source {
        Some(source) => format!("{:?}-{:?}", error.code, source.kind()),
        None => format!("{:?}", error.code),
    };
    format!(
        "source-cause:source-store:{code}:{}",
        tos_foundation::Digest256::of_bytes(error.detail.as_bytes()).to_hex()
    )
}
pub(crate) fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    use tos_source_store::StoreErrorCode;
    match error.code {
        StoreErrorCode::BudgetExceeded => ItemRefusal::Budget,
        StoreErrorCode::UnsupportedFormat | StoreErrorCode::UnsupportedPlatform => {
            ItemRefusal::Unsupported(error.to_string())
        }
        _ => ItemRefusal::Source(source_store_cause(&error)),
    }
}

pub(crate) fn decoded_wire_size(
    value: &serde_json::Value,
    limit: usize,
) -> Result<usize, ItemRefusal> {
    serialized_wire_size(limit, |writer| serde_json::to_writer(writer, value))
}
pub(crate) fn serialized_wire_size(
    limit: usize,
    write: impl FnOnce(&mut dyn std::io::Write) -> serde_json::Result<()>,
) -> Result<usize, ItemRefusal> {
    struct Counter {
        bytes: usize,
        limit: usize,
        exhausted: bool,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self.bytes.checked_add(bytes.len());
            if next.is_none_or(|n| n > self.limit) {
                self.exhausted = true;
                return Err(std::io::Error::other("logical serialization budget"));
            }
            self.bytes = next.unwrap();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter {
        bytes: 0,
        limit,
        exhausted: false,
    };
    let outcome = write(&mut counter);
    if counter.exhausted {
        return Err(ItemRefusal::BudgetCheck {
            check: "decoded JSON serialization bytes",
            used: None,
            limit: Some(limit as u64),
        });
    }
    outcome.map_err(|_| ItemRefusal::Unsupported("decoded JSON serialization".into()))?;
    Ok(counter.bytes)
}

// Logical state counts Rust value slots and their retained string/byte payloads.
// BTree/HashMap allocator nodes, buckets, alignment and allocator rounding are
// deliberately not claimed as RSS. Codec byte/depth/visit limits bound parsing
// separately; callers must count each simultaneously retained representation.
pub(crate) fn decoded_state(value: &serde_json::Value) -> Result<usize, ItemRefusal> {
    fn heap(value: &serde_json::Value) -> Option<usize> {
        use serde_json::Value;
        match value {
            Value::Number(n) => Some(n.as_str().len()),
            Value::String(s) => Some(s.len()),
            Value::Array(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<Value>())?,
                |sum, item| sum.checked_add(heap(item)?),
            ),
            Value::Object(items) => items.iter().try_fold(
                items
                    .len()
                    .checked_mul(std::mem::size_of::<(String, Value)>())?,
                |sum, (key, item)| sum.checked_add(key.len())?.checked_add(heap(item)?),
            ),
            _ => Some(0),
        }
    }
    std::mem::size_of::<serde_json::Value>()
        .checked_add(heap(value).ok_or(ItemRefusal::Budget)?)
        .ok_or(ItemRefusal::Budget)
}
pub(crate) fn ordered_state(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
    fn string(s: &tos_foundation::JsonString) -> Option<usize> {
        s.units()
            .len()
            .checked_mul(std::mem::size_of::<u16>())?
            .checked_add(s.as_str().map_or(0, str::len))
    }
    fn heap(value: &tos_foundation::JsonValue) -> Option<usize> {
        use tos_foundation::JsonValue;
        match value {
            JsonValue::Number(n) => Some(n.lexeme.len()),
            JsonValue::String(s) => string(s),
            JsonValue::Array(items) => items.iter().try_fold(
                items.len().checked_mul(std::mem::size_of::<JsonValue>())?,
                |sum, item| sum.checked_add(heap(item)?),
            ),
            JsonValue::Object(items) => items.iter().try_fold(
                items
                    .len()
                    .checked_mul(std::mem::size_of::<(tos_foundation::JsonString, JsonValue)>())?,
                |sum, (key, item)| sum.checked_add(string(key)?)?.checked_add(heap(item)?),
            ),
            _ => Some(0),
        }
    }
    std::mem::size_of::<tos_foundation::JsonValue>()
        .checked_add(heap(value).ok_or(ItemRefusal::Budget)?)
        .ok_or(ItemRefusal::Budget)
}
// Peak logical strict-parser tree plus duplicate-key index slots/payloads.
// Ancestor object indexes can coexist; summing the actual object indexes is a
// structural upper bound, independent of corpus size or serialized multipliers.
pub(crate) fn ordered_codec_state(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
    fn indexes(value: &tos_foundation::JsonValue) -> Option<usize> {
        use tos_foundation::JsonValue;
        match value {
            JsonValue::Object(items) => items.iter().try_fold(
                std::mem::size_of::<std::collections::HashMap<Vec<u16>, usize>>(),
                |n, (key, value)| {
                    n.checked_add(std::mem::size_of::<(Vec<u16>, usize)>())?
                        .checked_add(key.units().len().checked_mul(std::mem::size_of::<u16>())?)?
                        .checked_add(indexes(value)?)
                },
            ),
            JsonValue::Array(items) => items
                .iter()
                .try_fold(0usize, |n, v| n.checked_add(indexes(v)?)),
            _ => Some(0),
        }
    }
    ordered_state(value)?
        .checked_add(indexes(value).ok_or(ItemRefusal::Budget)?)
        .ok_or(ItemRefusal::Budget)
}
// Canonical emission keeps borrowed duplicate-key and sorted-entry indexes.
// Nested object indexes can coexist; this prices their logical slots, without
// cloning keys or interpreting allocator buckets as retained payloads.
pub(crate) fn ordered_emit_state(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
    use tos_foundation::{JsonString, JsonValue};
    fn indexes(value: &JsonValue) -> Option<usize> {
        match value {
            JsonValue::Object(items) => items.iter().try_fold(
                std::mem::size_of::<std::collections::HashSet<&Vec<u16>>>()
                    + std::mem::size_of::<Vec<&(JsonString, JsonValue)>>()
                    + items.len().checked_mul(
                        std::mem::size_of::<&Vec<u16>>()
                            + std::mem::size_of::<&(JsonString, JsonValue)>(),
                    )?,
                |n, (_, value)| n.checked_add(indexes(value)?),
            ),
            JsonValue::Array(items) => items
                .iter()
                .try_fold(0usize, |n, value| n.checked_add(indexes(value)?)),
            _ => Some(0),
        }
    }
    indexes(value).ok_or(ItemRefusal::Budget)
}
pub(crate) fn bounded_ordered(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<tos_foundation::JsonValue, ItemRefusal> {
    bounded_ordered_mode(
        raw,
        limits,
        available,
        deadline,
        cancelled,
        tos_foundation::JsonMode::PublishedStrict,
        false,
        false,
    )
}
fn bounded_ordered_mode(
    raw: &[u8],
    mut limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    mode: tos_foundation::JsonMode,
    syntax_as_source: bool,
    incremental_state: bool,
) -> Result<tos_foundation::JsonValue, ItemRefusal> {
    check(deadline, cancelled)?;
    // During Foundation parsing a decoded key may retain UTF16 both in the
    // ordered tree and the duplicate-key index, plus its cached UTF8. Their
    // total lengths cannot exceed two UTF16 copies and one UTF8 copy of input.
    // Each value visit can own one value slot, one key, one index entry and one
    // object index header. These are logical slots, not hash bucket/RSS bounds.
    if !incremental_state {
        let strings = raw
            .len()
            .checked_mul(2 * std::mem::size_of::<u16>() + std::mem::size_of::<u8>())
            .ok_or(ItemRefusal::Budget)?;
        let slot = std::mem::size_of::<tos_foundation::JsonValue>()
            + std::mem::size_of::<tos_foundation::JsonString>()
            + std::mem::size_of::<(Vec<u16>, usize)>()
            + std::mem::size_of::<std::collections::HashMap<Vec<u16>, usize>>();
        let remaining = available
            .checked_sub(strings)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "strict JSON logical string workspace",
                used: Some(strings as u64),
                limit: Some(available as u64),
            })?;
        limits.max_visits = limits.max_visits.min(remaining / slot);
        if limits.max_visits == 0 {
            return Err(ItemRefusal::BudgetCheck {
                check: "strict JSON logical node workspace",
                used: Some(slot as u64),
                limit: Some(remaining as u64),
            });
        }
    }
    let parsed = if incremental_state {
        tos_foundation::parse_json_with_state_budget(raw, mode, limits, available)
    } else {
        tos_foundation::parse_json(raw, mode, limits)
    };
    let result = parsed
        .map_err(|e| {
            if e.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                if incremental_state && e.detail == "JSON parser state budget exceeded" {
                    ItemRefusal::BudgetCheck {
                        check: "Item JSON parser workspace",
                        used: None,
                        limit: Some(available as u64),
                    }
                } else {
                    ItemRefusal::BudgetCheck {
                        check: "strict JSON codec bytes/depth/visits/integer",
                        used: None,
                        limit: None,
                    }
                }
            } else if syntax_as_source
                && matches!(
                    e.code,
                    tos_foundation::FoundationErrorCode::InvalidUtf8
                        | tos_foundation::FoundationErrorCode::InvalidJson
                        | tos_foundation::FoundationErrorCode::InvalidUnicodeScalar
                        | tos_foundation::FoundationErrorCode::InvalidNumber
                        | tos_foundation::FoundationErrorCode::NonfiniteFloat
                )
            {
                ItemRefusal::Source("invalid finite native JSON".into())
            } else {
                ItemRefusal::Unsupported(format!("strict JSON: {e:?}"))
            }
        })?
        .into_root();
    check(deadline, cancelled)?;
    let state = ordered_state(&result)?;
    if state > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "strict JSON retained ordered state",
            used: Some(state as u64),
            limit: Some(available as u64),
        });
    }
    Ok(result)
}
// Existing legacy JSON consumers keep last-key-wins; this bounds the same
// codec workspace without imposing PublishedStrict duplicate-key semantics.
// Integer text was bounded only by the member bytes in that serde route.
pub(crate) fn bounded_legacy_decoded_state(
    raw: &[u8],
    max_bytes: usize,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    let limits =
        tos_foundation::JsonLimits::new(max_bytes, 128, available.max(1), max_bytes.max(1))
            .map_err(|_| ItemRefusal::Budget)?;
    bounded_legacy_decoded_state_inner(raw, limits, available, deadline, cancelled, false, false)
}
// The named source-layer caller has the original native decoded-field JSON
// profile: malformed finite JSON is an issue, while a valid value outside
// serde's representable scalar strings remains explicitly unsupported.
pub(crate) fn bounded_legacy_decoded_state_with_limits(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    bounded_legacy_decoded_state_inner(raw, limits, available, deadline, cancelled, true, false)
}
// Only the actual Item legacy route uses incremental parser workspace.
// Other native/selected-layer profiles preserve their existing admission.
pub(crate) fn bounded_legacy_item_decoded_state(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    bounded_legacy_decoded_state_inner(raw, limits, available, deadline, cancelled, true, true)
}
fn bounded_legacy_decoded_state_inner(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    malformed_as_source: bool,
    incremental_state: bool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    drop(bounded_ordered_mode(
        raw,
        limits,
        available,
        deadline,
        cancelled,
        tos_foundation::JsonMode::RequestLastWins,
        malformed_as_source,
        incremental_state,
    )?);
    let value = serde_json::from_slice(raw)
        .map_err(|_| ItemRefusal::Unsupported("decoded JSON representation".into()))?;
    let state = decoded_state(&value)?;
    if state > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "legacy JSON retained decoded state",
            used: Some(state as u64),
            limit: Some(available as u64),
        });
    }
    check(deadline, cancelled)?;
    Ok((value, state))
}
pub(crate) fn bounded_decoded_state(
    raw: &[u8],
    limits: tos_foundation::JsonLimits,
    available: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(serde_json::Value, usize), ItemRefusal> {
    // Strict validation and its duplicate-key index finish before decoding;
    // there is no simultaneous retained Foundation and serde tree here.
    drop(bounded_ordered(
        raw, limits, available, deadline, cancelled,
    )?);
    let value = serde_json::from_slice(raw)
        .map_err(|_| ItemRefusal::Unsupported("decoded JSON representation".into()))?;
    let state = decoded_state(&value)?;
    if state > available {
        return Err(ItemRefusal::BudgetCheck {
            check: "strict JSON retained decoded state",
            used: Some(state as u64),
            limit: Some(available as u64),
        });
    }
    check(deadline, cancelled)?;
    Ok((value, state))
}
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
        RecordRuleError::Sink { detail } => decode_sink_budget(&detail).unwrap_or_else(|| {
            ItemRefusal::Unsupported(format!("{:?}", RecordRuleError::Sink { detail }))
        }),
        other => ItemRefusal::Unsupported(format!("{other:?}")),
    }
}

fn decode_sink_budget(detail: &str) -> Option<ItemRefusal> {
    fn counter(value: &str) -> Option<Option<u64>> {
        if value == "-" {
            Some(None)
        } else {
            value.parse().ok().map(Some)
        }
    }

    let mut fields = detail.strip_prefix("budget-check|")?.split('|');
    let check = match fields.next()? {
        "record_issue_sink" => "record_issue_sink",
        "biblio_sink" => "biblio_sink",
        _ => return None,
    };
    let used = counter(fields.next()?)?;
    let limit = counter(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }
    Some(ItemRefusal::BudgetCheck { check, used, limit })
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
