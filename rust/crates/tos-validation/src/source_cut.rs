//! Actual source-carrier adapter for executable Item rules. Current member
//! paths and raw bytes come from an anchored CorpusCutReader; retained bases
//! stay in that reader and never become competing current record owners.
//! Carrier coverage is weaker than source-owner admission coverage.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
    SoftwareComponentSelectionV1, SourceMembershipV1, StreamedCorpusCutReaderV1,
};

pub use crate::executor::LegacySelectedDiagnosticsLimits;
use crate::executor::{
    BatchBudget, BatchCoverageCheckpoint, BatchCoverageExpectation, BatchOutcome,
    BatchStreamBudget, BatchUnit, BatchUnitVerdict, DiagnosticsUnitInputMode, ExactWorkerIdentity,
    ExecutionIdentity, ExecutorBudget, ExecutorFailure, ExecutorOutcome, PreparedSchemaWorker,
    SchemaDiagnosticUnit, SchemaDiagnosticsCheckpoint, SchemaDiagnosticsExecutionCost,
    SchemaDiagnosticsOutcome, SharedSchemaWorkerQuota, VerifiedWorkerImageHandle,
    legacy_selected_child_address_space_required, schema_diagnostics,
    selected_legacy_diagnostics_batch_unit_digest,
};
use crate::item_rules::{
    ItemFamilyReport, ItemLimits, ItemPayload, ItemRefusal, ItemRules, ItemSource,
};
use crate::provenance_rules::{ProvenanceReport, ProvenanceRules, ProvenanceSource};
use crate::record_rules::RecordFamily;
use crate::{FormatProfile, SchemaBackendProbe, SchemaResource, published_value};

/// Exact owner schema executor, with separately enforced process custody.
/// An unknown profile/resource or incomplete execution must refuse.
pub trait CutSchemaExecutor {
    /// Exact immutable source revision bound by this executor, when its
    /// implementation can attest one. Adapters that cannot expose this
    /// identity must retain the default refusal value.
    fn selected_source_revision(&self) -> Option<SourceRevision> {
        None
    }

    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal>;

    /// An actual prior scalar execution may satisfy this repeated immutable
    /// schema request. Logical source reads/predicates remain caller-owned;
    /// a reuse hit is not a new execution receipt. Other executors stay fresh.
    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.check(path, raw, contract, deadline, cancelled)
    }

    fn schema_input_cost(
        &self,
        _path: &str,
        _raw: &[u8],
        _contract: &str,
        _ordinal: u64,
    ) -> Result<CutSchemaInputCost, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "schema input cost unavailable".into(),
        ))
    }

    fn set_operation_budget(&mut self, _budget: BatchStreamBudget) -> Result<(), ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "schema operation envelope unavailable".into(),
        ))
    }

    /// Attach this executor to one explicit diagnostics-v2 invocation quota.
    /// The default-v1 trait path does not participate in shared accounting.
    fn set_shared_schema_worker_quota(
        &mut self,
        _quota: SharedSchemaWorkerQuota,
    ) -> Result<(), ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "shared schema worker quota unavailable".into(),
        ))
    }

    fn finish(&mut self, _deadline: Instant, _cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "schema operation finalization unavailable".into(),
        ))
    }

    /// One bounded disposable invocation; no fallback to a weaker executor.
    fn check_batch(
        &mut self,
        _checks: &[CutSchemaCheck],
        _budget: BatchBudget,
        _deadline: Instant,
        _cancelled: &AtomicBool,
    ) -> Result<Vec<bool>, ItemRefusal> {
        Err(ItemRefusal::Unsupported(
            "schema batch execution unavailable".into(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct CutSchemaCheck {
    pub path: String,
    pub raw: Vec<u8>,
    pub contract: String,
}

/// Exact bounded batch framing costs over the selected closure; no execution or receipt.
#[derive(Debug, Clone)]
pub struct CutSchemaInputCost {
    pub decoded_instance_bytes: u64,
    pub unit_wire_bytes: u64,
    pub operation_wire_bytes: u64,
    pub frame_wire_bytes: u64,
    pub receipt_bytes: u64,
    pub remaining_receipts: u64,
    pub remaining_receipt_bytes: u64,
    pub selector: String,
}

#[derive(Debug, Clone, Copy)]
pub struct CutBatchBinding {
    pub checkpoint: BatchCoverageCheckpoint,
    pub ordinal: u64,
    pub unit_sha256: Digest256,
}

#[derive(Debug, Clone, Copy)]
pub struct CutWorkerLimits {
    pub max_receipts: usize,
    pub max_receipt_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct CutSchemaReceipt {
    pub path: String,
    pub contract: String,
    pub source_revision: SourceRevision,
    pub source_raw_sha256: Digest256,
    /// Native source loaders use decoded JSON. The strict transport worker
    /// evaluates this separately bound serialization, preserving last decoded
    /// field semantics without pretending it is the original source bytes.
    pub decoded_instance_sha256: Digest256,
    pub execution: ExecutionIdentity,
    pub valid: bool,
    /// Transport coverage of this finite invocation, not owner completeness.
    pub batch: Option<CutBatchBinding>,
}

#[path = "source_cut_receipt_spool.rs"]
mod receipt_spool;
pub use receipt_spool::{
    CutSchemaDiagnosticPage, CutSchemaReceiptPage, CutSchemaReceiptSpoolLimits,
    CutSchemaReceiptSpoolSummary, CutWorkerSchemaExecutorSpooling,
};

/// Bounded receipt range access used by operation reports. Rows are local to
/// the requested executor interval and never imply selected source membership.
pub trait CutSchemaReceiptRange {
    fn execution_binding(&self) -> CutExecutionBinding;
    fn source_revision(&self) -> SourceRevision;
    fn contract_digest(&self, contract: &str) -> Option<Digest256>;
    fn receipt_count(&self) -> usize;
    fn receipt_range_supported(&self) -> bool;
    fn receipt_page_limits(&self) -> (usize, usize);
    fn operation_budget(&self) -> BatchStreamBudget;
    fn receipt_limit_bytes(&self) -> usize;
    fn release_child(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal>;

    fn read_receipts_after(
        &mut self,
        after_ordinal: Option<u64>,
        max_rows: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaReceiptPage, ItemRefusal>;
}

/// Materialize only one caller-selected receipt interval. Each page is read
/// through the owner's bounded range surface and charged together with the
/// retained output slots; callers remain responsible for their final report
/// object's separate lifecycle and membership/currentness rules.
pub(crate) fn collect_schema_receipt_range<S: CutSchemaReceiptRange>(
    schemas: &mut S,
    start: usize,
    end: usize,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<CutSchemaReceipt>, ItemRefusal> {
    if !schemas.receipt_range_supported() || end < start || end > schemas.receipt_count() {
        return Err(ItemRefusal::Unsupported(
            "operation requires the legacy schema-receipt range".into(),
        ));
    }
    let count = end - start;
    let (page_rows, page_bytes) = schemas.receipt_page_limits();
    if count == 0 {
        return Ok(Vec::new());
    }
    if page_rows == 0 || page_bytes == 0 {
        return Err(ItemRefusal::Budget);
    }
    let output_slots = count
        .checked_mul(std::mem::size_of::<CutSchemaReceipt>())
        .ok_or(ItemRefusal::Budget)?;
    if output_slots > max_state_bytes {
        return Err(ItemRefusal::BudgetCheck {
            check: "operation schema receipt output slots",
            used: u64::try_from(output_slots).ok(),
            limit: u64::try_from(max_state_bytes).ok(),
        });
    }
    let mut receipts = Vec::new();
    receipts
        .try_reserve_exact(count)
        .map_err(|_| ItemRefusal::Budget)?;
    let mut ordinal = start;
    let mut after = start
        .checked_sub(1)
        .map(|value| u64::try_from(value).map_err(|_| ItemRefusal::Budget))
        .transpose()?;
    let mut retained_bytes = output_slots;
    while ordinal < end {
        check(deadline, cancelled)?;
        let mut rows = (end - ordinal).min(page_rows);
        let remaining = max_state_bytes
            .checked_sub(retained_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let max_bytes = page_bytes.min(remaining);
        let page = loop {
            match schemas.read_receipts_after(after, rows, max_bytes, deadline, cancelled) {
                Err(ItemRefusal::Budget) if rows > 1 => rows = rows.div_ceil(2),
                result => break result?,
            }
        };
        let page_bytes_used = page.encoded_bytes();
        retained_bytes = retained_bytes
            .checked_add(page_bytes_used)
            .filter(|bytes| *bytes <= max_state_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "operation schema receipt range state bytes",
                used: retained_bytes
                    .checked_add(page_bytes_used)
                    .and_then(|n| u64::try_from(n).ok()),
                limit: u64::try_from(max_state_bytes).ok(),
            })?;
        let rows = page.into_rows();
        if rows.is_empty() {
            return Err(ItemRefusal::Unsupported(
                "operation schema receipt range ended before its boundary".into(),
            ));
        }
        for (row_ordinal, receipt) in rows {
            if usize::try_from(row_ordinal).ok() != Some(ordinal) || ordinal >= end {
                return Err(ItemRefusal::Unsupported(
                    "operation schema receipt range ordinal changed".into(),
                ));
            }
            after = Some(row_ordinal);
            receipts.push(receipt);
            ordinal = ordinal.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }
    }
    check(deadline, cancelled)?;
    Ok(receipts)
}

/// Caller-owned aggregate ceilings for the opt-in source-cut diagnostics-v2
/// path. The legacy scalar and batch receipts remain a separate protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutSchemaDiagnosticsLimits {
    pub max_total_issues: usize,
    pub max_total_report_bytes: usize,
    pub max_total_state_bytes: usize,
}

/// One verified diagnostics-v2 result for an exact source-cut schema check.
/// Fields stay private so callers receive only evidence built after all
/// worker, closure, unit, report, caps, and source bindings were checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutSchemaDiagnostic {
    path: String,
    contract: String,
    source_revision: SourceRevision,
    source_raw_sha256: Digest256,
    decoded_instance_sha256: Digest256,
    aggregate_caps_sha256: Digest256,
    checkpoint: SchemaDiagnosticsCheckpoint,
    unit: SchemaDiagnosticUnit,
    schema_resource_bytes: usize,
    schema_resource_buffer_bytes: usize,
    input_instance_bytes: usize,
    input_instance_buffer_bytes: usize,
    input_metadata_bytes: usize,
    request_bytes: usize,
    request_buffer_bytes: usize,
    response_bytes: usize,
    response_buffer_bytes: usize,
    worker_cpu_micros: u64,
    retained_state_bytes: usize,
    accounted_state_bytes: usize,
}

impl CutSchemaDiagnostic {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn contract(&self) -> &str {
        &self.contract
    }
    pub fn source_revision(&self) -> SourceRevision {
        self.source_revision
    }
    pub fn source_raw_sha256(&self) -> Digest256 {
        self.source_raw_sha256
    }
    pub fn decoded_instance_sha256(&self) -> Digest256 {
        self.decoded_instance_sha256
    }
    pub fn aggregate_caps_sha256(&self) -> Digest256 {
        self.aggregate_caps_sha256
    }
    pub fn checkpoint(&self) -> &SchemaDiagnosticsCheckpoint {
        &self.checkpoint
    }
    pub fn unit(&self) -> &SchemaDiagnosticUnit {
        &self.unit
    }
    pub fn report(&self) -> &schema_diagnostics::Report {
        &self.unit.report
    }
    pub fn status(&self) -> schema_diagnostics::Status {
        self.unit.report.status
    }
    pub fn protocol_id(&self) -> &'static str {
        "tos_schema_diagnostics_v2"
    }
    pub fn is_valid(&self) -> bool {
        self.unit.report.is_valid()
    }
    pub fn is_invalid(&self) -> bool {
        self.unit.report.is_well_formed()
            && self.unit.report.status == schema_diagnostics::Status::Invalid
            && !self.unit.report.truncated
            && self.unit.report.failure == schema_diagnostics::Failure::None
            && self.unit.report.total_issue_count == self.unit.report.issues.len() as u64
    }
    pub fn schema_resource_bytes(&self) -> usize {
        self.schema_resource_bytes
    }
    pub fn schema_resource_buffer_bytes(&self) -> usize {
        self.schema_resource_buffer_bytes
    }
    pub fn input_instance_bytes(&self) -> usize {
        self.input_instance_bytes
    }
    pub fn input_instance_buffer_bytes(&self) -> usize {
        self.input_instance_buffer_bytes
    }
    pub fn input_metadata_bytes(&self) -> usize {
        self.input_metadata_bytes
    }
    pub fn request_bytes(&self) -> usize {
        self.request_bytes
    }
    /// Final encoded frame Vec capacity observed for this completed exchange.
    /// Selected Legacy admission separately counts three frame units to cover
    /// a transient overlap while the request Vec grows.
    pub fn request_buffer_bytes(&self) -> usize {
        self.request_buffer_bytes
    }
    pub fn response_bytes(&self) -> usize {
        self.response_bytes
    }
    pub fn response_buffer_bytes(&self) -> usize {
        self.response_buffer_bytes
    }
    pub fn worker_cpu_micros(&self) -> u64 {
        self.worker_cpu_micros
    }
    pub fn retained_state_bytes(&self) -> usize {
        self.retained_state_bytes
    }
    pub fn accounted_state_bytes(&self) -> usize {
        self.accounted_state_bytes
    }
}

/// Checked cumulative execution cost for complete diagnostics-v2 exchanges.
/// Schema resources are counted as bytes read into each worker request; request
/// and response fields are the measured wire byte counts, and CPU comes from
/// the worker's wait4 receipt. No v1 or incomplete exchange is represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutSchemaDiagnosticsCumulativeCost {
    completed_exchanges: u64,
    schema_resource_bytes: u64,
    request_bytes: u64,
    response_bytes: u64,
    worker_cpu_micros: u64,
}

impl CutSchemaDiagnosticsCumulativeCost {
    pub fn completed_exchanges(&self) -> u64 {
        self.completed_exchanges
    }
    pub fn schema_resource_bytes(&self) -> u64 {
        self.schema_resource_bytes
    }
    pub fn request_bytes(&self) -> u64 {
        self.request_bytes
    }
    pub fn response_bytes(&self) -> u64 {
        self.response_bytes
    }
    pub fn worker_cpu_micros(&self) -> u64 {
        self.worker_cpu_micros
    }

    fn checked_add_exchange(
        self,
        schema_resource_bytes: usize,
        request_bytes: usize,
        response_bytes: usize,
        worker_cpu_micros: u64,
    ) -> Option<Self> {
        Some(Self {
            completed_exchanges: self.completed_exchanges.checked_add(1)?,
            schema_resource_bytes: self
                .schema_resource_bytes
                .checked_add(u64::try_from(schema_resource_bytes).ok()?)?,
            request_bytes: self
                .request_bytes
                .checked_add(u64::try_from(request_bytes).ok()?)?,
            response_bytes: self
                .response_bytes
                .checked_add(u64::try_from(response_bytes).ok()?)?,
            worker_cpu_micros: self.worker_cpu_micros.checked_add(worker_cpu_micros)?,
        })
    }
}

/// Real disposable schema execution over resources read from the exact source
/// cut. This is a family execution receipt, not a trusted-source admission
/// ticket. Dedicated-worker descendant custody and host I/O interruption are
/// additional owner gates; the current executor guarantees parent liveness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutExecutionBinding {
    pub source_revision: SourceRevision,
    pub schema_profile: FormatProfile,
    pub schema_set_sha256: Digest256,
    pub worker_sha256: Digest256,
}

/// Logical controller-workspace upper bound for preparing the exact schema
/// resources selected from this cut. This reads authenticated member metadata
/// only; it does not read or parse payload bytes. It bounds the transient
/// constructor peak, including the full `SchemaBackendProbe::new` serde closure,
/// rather than retained closure state. The shared worker image is accounted by
/// its operation owner, and allocator bookkeeping/RSS remain governed by the
/// caller's physical host envelope.
pub fn cut_schema_resource_preparation_state_upper_bound(
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<usize, ItemRefusal> {
    let mut stats = SchemaResourcePreparationStats::default();

    for member in cut.current().members() {
        check(deadline, cancelled)?;
        let path = member.path.as_str();
        if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
            continue;
        }

        stats.add_schema(
            path,
            usize::try_from(member.size_bytes).map_err(|_| ItemRefusal::Budget)?,
        )?;
    }
    let upper_bound = stats.upper_bound()?;
    check(deadline, cancelled)?;
    Ok(upper_bound)
}

/// Shared resource-only accounting kernel for the source cut and the selected
/// worker's logical address-space admission. These bounds price constructor
/// workspace, not RSS or allocator bookkeeping.
#[derive(Default)]
struct SchemaResourcePreparationStats {
    count: usize,
    raw_bytes: usize,
    path_bytes: usize,
    largest_resource: usize,
    serde_workspace: usize,
}

impl SchemaResourcePreparationStats {
    fn add_schema(&mut self, path: &str, bytes: usize) -> Result<(), ItemRefusal> {
        if bytes > SchemaBackendProbe::MAX_RESOURCE_BYTES {
            return Err(ItemRefusal::Budget);
        }
        self.count = self
            .count
            .checked_add(1)
            .filter(|count| *count <= SchemaBackendProbe::MAX_RESOURCES)
            .ok_or(ItemRefusal::Budget)?;
        self.raw_bytes = self
            .raw_bytes
            .checked_add(bytes)
            .filter(|total| *total <= SchemaBackendProbe::MAX_TOTAL_BYTES)
            .ok_or(ItemRefusal::Budget)?;
        self.path_bytes = self
            .path_bytes
            .checked_add(path.len())
            .ok_or(ItemRefusal::Budget)?;
        self.largest_resource = self.largest_resource.max(bytes);
        self.serde_workspace = self
            .serde_workspace
            .checked_add(finite_serde_json_controller_workspace_upper_bound(bytes)?)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn upper_bound(&self) -> Result<usize, ItemRefusal> {
        if self.count == 0 {
            return Err(ItemRefusal::Budget);
        }
        let foundation_slot = std::mem::size_of::<tos_foundation::JsonValue>()
            .checked_add(std::mem::size_of::<(
                tos_foundation::JsonString,
                tos_foundation::JsonValue,
            )>())
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or(ItemRefusal::Budget)?;
        let foundation_workspace = self
            .largest_resource
            .checked_add(1)
            .and_then(|bytes| bytes.checked_mul(foundation_slot))
            .and_then(|bytes| bytes.checked_add(self.largest_resource.checked_mul(16)?))
            .and_then(|bytes| bytes.checked_add(65usize.checked_mul(foundation_slot)?))
            .ok_or(ItemRefusal::Budget)?;
        let closure_buffers = self
            .raw_bytes
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(self.path_bytes.checked_mul(8)?))
            .and_then(|bytes| bytes.checked_add(self.count.checked_mul(4096)?))
            .and_then(|bytes| bytes.checked_add(65536))
            .ok_or(ItemRefusal::Budget)?;
        self.serde_workspace
            .checked_add(foundation_workspace)
            .and_then(|bytes| bytes.checked_add(closure_buffers))
            .ok_or(ItemRefusal::Budget)
    }
}

pub struct CutWorkerSchemaExecutor {
    revision: SourceRevision,
    prepared: PreparedSchemaWorker,
    contracts: BTreeMap<String, (String, Digest256)>,
    schema_set_digest: Digest256,
    resource_preparation_state_upper_bound: usize,
    profile: FormatProfile,
    worker: ExactWorkerIdentity,
    budget: ExecutorBudget,
    limits: CutWorkerLimits,
    receipt_bytes: usize,
    receipt_count: usize,
    receipts: Vec<CutSchemaReceipt>,
    diagnostics_v2: Option<CutSchemaDiagnosticsLimits>,
    diagnostics_v2_controller_state_cap: Option<usize>,
    diagnostics_v2_legacy_raw_instance_limit: usize,
    diagnostics_v2_legacy_selected_limits: Option<LegacySelectedDiagnosticsLimits>,
    diagnostics_v2_shared_quota_attached: bool,
    diagnostics_v2_cost: Option<CutSchemaDiagnosticsCumulativeCost>,
    diagnostics_v2_cost_unknown: bool,
    diagnostic_executions: usize,
    diagnostic_issues_used: usize,
    diagnostic_report_bytes_used: usize,
    diagnostic_state_bytes_used: usize,
    pending_diagnostics: Vec<CutSchemaDiagnostic>,
    finished: bool,
    protocol_started: bool,
}

impl CutWorkerSchemaExecutor {
    pub fn from_cut(
        cut: &CorpusCutReader,
        profile: FormatProfile,
        worker: ExactWorkerIdentity,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        Self::from_cut_inner(
            cut, profile, worker, None, budget, limits, deadline, cancelled,
        )
    }

    /// Build this cut-local schema closure while reusing the exact sealed
    /// executable image already prepared by the caller operation.
    pub fn from_cut_with_image(
        cut: &CorpusCutReader,
        profile: FormatProfile,
        image: &VerifiedWorkerImageHandle,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        Self::from_cut_inner(
            cut,
            profile,
            image.identity().clone(),
            Some(image),
            budget,
            limits,
            deadline.min(image.operation_deadline()),
            cancelled,
        )
    }

    fn from_cut_inner(
        cut: &CorpusCutReader,
        profile: FormatProfile,
        worker: ExactWorkerIdentity,
        image: Option<&VerifiedWorkerImageHandle>,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let operation_origin = Instant::now();
        check(deadline, cancelled)?;
        if limits.max_receipts == 0
            || limits.max_receipts == usize::MAX
            || limits.max_receipt_bytes == 0
            || limits.max_receipt_bytes == usize::MAX
        {
            return Err(ItemRefusal::Budget);
        }
        let revision = cut.current().revision();
        let mut resources = Vec::new();
        let mut contracts = BTreeMap::new();
        let mut total_bytes = 0usize;
        // Derive schema membership from the exact anchored manifest and read
        // only those selected bytes. A narrow retirement must not scan an
        // unrelated surviving raw source while compiling its schema closure.
        for metadata in cut.current().members() {
            check(deadline, cancelled)?;
            let path = metadata.path.as_str();
            if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
                continue;
            }
            let member = cut
                .read_member(
                    revision,
                    &metadata.path,
                    SchemaBackendProbe::MAX_RESOURCE_BYTES as u64,
                    deadline,
                    cancelled,
                )
                .map_err(store_error)?;
            total_bytes = total_bytes
                .checked_add(member.raw.len())
                .filter(|n| *n <= SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or(ItemRefusal::Budget)?;
            if resources.len() >= SchemaBackendProbe::MAX_RESOURCES
                || member.raw.len() > SchemaBackendProbe::MAX_RESOURCE_BYTES
            {
                return Err(ItemRefusal::Budget);
            }
            let value = published_value(&member.raw, SchemaBackendProbe::MAX_RESOURCE_BYTES)
                .map_err(|error| {
                    ItemRefusal::Unsupported(format!("schema resource {path}: {error:?}"))
                })?;
            let uri = value["$id"]
                .as_str()
                .ok_or_else(|| ItemRefusal::Unsupported("schema resource ID".into()))?
                .to_owned();
            contracts.insert(
                path.to_owned(),
                (uri.clone(), Digest256::of_bytes(&member.raw)),
            );
            resources.push(SchemaResource {
                uri,
                raw: member.raw,
            });
        }
        Self::prepare_selected_resources(
            revision,
            contracts,
            resources,
            profile,
            worker,
            image,
            budget,
            limits,
            deadline,
            cancelled,
            operation_origin,
        )
    }

    /// Additive bounded manifest reader over the SAME selected schema kernel.
    /// The disk index is derived; exact source revision and schema bytes remain
    /// authenticated by the completed V1 selected-cut reader. Semantic source
    /// assessment and current disclosure authority belong to the caller.
    pub fn from_streamed_cut_with_image(
        cut: &StreamedCorpusCutReaderV1,
        profile: FormatProfile,
        image: &VerifiedWorkerImageHandle,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let deadline = deadline.min(image.operation_deadline());
        let worker = image.identity().clone();
        let image = Some(image);
        let operation_origin = Instant::now();
        check(deadline, cancelled)?;
        if limits.max_receipts == 0
            || limits.max_receipts == usize::MAX
            || limits.max_receipt_bytes == 0
            || limits.max_receipt_bytes == usize::MAX
        {
            return Err(ItemRefusal::Budget);
        }
        let revision = cut
            .revision_at(0)
            .map_err(store_error)?
            .ok_or_else(|| ItemRefusal::Source("streamed current revision absent".into()))?
            .revision;
        let mut resources = Vec::new();
        let mut contracts = BTreeMap::new();
        let mut total_bytes = 0usize;
        // Derive schema membership from the exact anchored manifest and read
        // only those selected bytes. A narrow retirement must not scan an
        // unrelated surviving raw source while compiling its schema closure.
        let mut after: Option<RelativePath> = None;
        while let Some(metadata) = cut
            .member_after(revision, after.as_ref())
            .map_err(store_error)?
        {
            after = Some(metadata.path.clone());
            check(deadline, cancelled)?;
            let path = metadata.path.as_str();
            if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
                continue;
            }
            let member = cut
                .read_member(
                    revision,
                    &metadata.path,
                    SchemaBackendProbe::MAX_RESOURCE_BYTES as u64,
                    deadline,
                    cancelled,
                )
                .map_err(store_error)?;
            total_bytes = total_bytes
                .checked_add(member.raw.len())
                .filter(|n| *n <= SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or(ItemRefusal::Budget)?;
            if resources.len() >= SchemaBackendProbe::MAX_RESOURCES
                || member.raw.len() > SchemaBackendProbe::MAX_RESOURCE_BYTES
            {
                return Err(ItemRefusal::Budget);
            }
            let value = published_value(&member.raw, SchemaBackendProbe::MAX_RESOURCE_BYTES)
                .map_err(|error| {
                    ItemRefusal::Unsupported(format!("schema resource {path}: {error:?}"))
                })?;
            let uri = value["$id"]
                .as_str()
                .ok_or_else(|| ItemRefusal::Unsupported("schema resource ID".into()))?
                .to_owned();
            contracts.insert(
                path.to_owned(),
                (uri.clone(), Digest256::of_bytes(&member.raw)),
            );
            resources.push(SchemaResource {
                uri,
                raw: member.raw,
            });
        }
        Self::prepare_selected_resources(
            revision,
            contracts,
            resources,
            profile,
            worker,
            image,
            budget,
            limits,
            deadline,
            cancelled,
            operation_origin,
        )
    }

    /// Prepare the same bounded schema engine from an explicitly selected live
    /// owner's resource closure. This labels checks with its source revision;
    /// it does not establish an immutable corpus cut or source admission.
    pub fn from_selected_resources(
        revision: SourceRevision,
        contracts: BTreeMap<String, (String, Digest256)>,
        resources: Vec<SchemaResource>,
        profile: FormatProfile,
        worker: ExactWorkerIdentity,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        Self::prepare_selected_resources(
            revision,
            contracts,
            resources,
            profile,
            worker,
            None,
            budget,
            limits,
            deadline,
            cancelled,
            Instant::now(),
        )
    }

    fn prepare_selected_resources(
        revision: SourceRevision,
        contracts: BTreeMap<String, (String, Digest256)>,
        resources: Vec<SchemaResource>,
        profile: FormatProfile,
        worker: ExactWorkerIdentity,
        image: Option<&VerifiedWorkerImageHandle>,
        budget: ExecutorBudget,
        limits: CutWorkerLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
        operation_origin: Instant,
    ) -> Result<Self, ItemRefusal> {
        check(deadline, cancelled)?;
        if limits.max_receipts == 0
            || limits.max_receipts == usize::MAX
            || limits.max_receipt_bytes == 0
            || limits.max_receipt_bytes == usize::MAX
        {
            return Err(ItemRefusal::Budget);
        }
        if resources.is_empty()
            || resources.len() > SchemaBackendProbe::MAX_RESOURCES
            || contracts.len() != resources.len()
        {
            return Err(ItemRefusal::Budget);
        }
        let mut resource_preparation = SchemaResourcePreparationStats::default();
        for (path, (uri, _)) in &contracts {
            let resource = resources
                .iter()
                .find(|resource| resource.uri == *uri)
                .ok_or(ItemRefusal::Budget)?;
            resource_preparation.add_schema(path, resource.raw.len())?;
        }
        let resource_preparation_state_upper_bound = resource_preparation.upper_bound()?;
        let mut total_bytes = 0usize;
        let mut selected_uris = std::collections::BTreeSet::new();
        for (path, (uri, digest)) in &contracts {
            check(deadline, cancelled)?;
            let relative = RelativePath::parse(path)
                .map_err(|_| ItemRefusal::Source("schema selected path".into()))?;
            if !relative.as_str().starts_with("ToS/contracts/")
                || !relative.as_str().ends_with(".schema.json")
                || !selected_uris.insert(uri.as_str())
            {
                return Err(ItemRefusal::Source("schema resource selection".into()));
            }
            let matching = resources
                .iter()
                .filter(|resource| resource.uri == *uri)
                .collect::<Vec<_>>();
            if matching.len() != 1
                || matching[0].raw.len() > SchemaBackendProbe::MAX_RESOURCE_BYTES
                || Digest256::of_bytes(&matching[0].raw) != *digest
            {
                return Err(ItemRefusal::Source("schema resource byte binding".into()));
            }
            total_bytes = total_bytes
                .checked_add(matching[0].raw.len())
                .filter(|n| *n <= SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or(ItemRefusal::Budget)?;
        }
        let schema_set_digest = SchemaBackendProbe::new(resources.clone(), profile)
            .map_err(|error| {
                ItemRefusal::Unsupported(format!("schema resource closure: {error:?}"))
            })?
            .schema_set_digest();
        check(deadline, cancelled)?;
        let prepared_result = match image {
            Some(image) => PreparedSchemaWorker::prepare_with_image(
                image, &resources, profile, budget, deadline, cancelled,
            ),
            None => PreparedSchemaWorker::prepare(
                &worker, &resources, profile, budget, deadline, cancelled,
            ),
        };
        let mut prepared = prepared_result.map_err(|reason| match reason {
            ExecutorFailure::Timeout => ItemRefusal::Deadline,
            ExecutorFailure::Cancelled => {
                ItemRefusal::Source("schema preparation cancelled".into())
            }
            other => ItemRefusal::Unsupported(format!("schema worker preparation: {other:?}")),
        })?;
        check(deadline, cancelled)?;
        prepared.set_operation_origin(operation_origin);
        // Preserve the existing declared receipt-count ceiling as the finite
        // default operation count; an explicit owner envelope may narrow it.
        let mut operation = prepared.operation_budget();
        operation.max_chunks = limits.max_receipts as u64;
        operation.max_total_units = limits.max_receipts as u64;
        prepared
            .set_operation_budget(operation)
            .map_err(operation_failure)?;
        Ok(Self {
            revision,
            prepared,
            contracts,
            schema_set_digest,
            resource_preparation_state_upper_bound,
            profile,
            worker,
            budget,
            limits,
            receipt_bytes: 0,
            receipt_count: 0,
            receipts: Vec::new(),
            diagnostics_v2: None,
            diagnostics_v2_controller_state_cap: None,
            diagnostics_v2_legacy_raw_instance_limit: SchemaBackendProbe::MAX_INSTANCE_BYTES,
            diagnostics_v2_legacy_selected_limits: None,
            diagnostics_v2_shared_quota_attached: false,
            diagnostics_v2_cost: None,
            diagnostics_v2_cost_unknown: false,
            diagnostic_executions: 0,
            diagnostic_issues_used: 0,
            diagnostic_report_bytes_used: 0,
            diagnostic_state_bytes_used: 0,
            pending_diagnostics: Vec::new(),
            finished: false,
            protocol_started: false,
        })
    }

    /// Configure the finite aggregate envelope before the first worker frame.
    pub fn set_operation_budget(&mut self, budget: BatchStreamBudget) -> Result<(), ItemRefusal> {
        if self.diagnostics_v2.is_some() {
            if self.protocol_started || self.finished {
                return Err(ItemRefusal::Unsupported(
                    "schema operation already started".into(),
                ));
            }
            if !cut_diagnostics_limits_valid(
                self.limits.max_receipts,
                self.diagnostics_v2.unwrap(),
                budget,
            ) {
                return Err(ItemRefusal::BudgetCheck {
                    check: "cut schema diagnostics operation envelope",
                    used: None,
                    limit: None,
                });
            }
        }
        self.prepared
            .set_operation_budget(budget)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })
    }

    /// Attach this unused diagnostics-v2 executor to one invocation-wide
    /// worker quota before its first request. Prior serial owners may already
    /// have committed usage to the same handle; v1/default execution remains
    /// separate.
    pub fn set_shared_schema_worker_quota(
        &mut self,
        quota: SharedSchemaWorkerQuota,
    ) -> Result<(), ItemRefusal> {
        if self.diagnostics_v2.is_none()
            || self.protocol_started
            || self.diagnostic_executions != 0
            || self.diagnostics_v2_controller_state_cap.is_some()
            || self.receipt_count != 0
            || self.finished
        {
            return Err(ItemRefusal::Unsupported(
                "shared schema worker quota requires unused diagnostics-v2 executor".into(),
            ));
        }
        self.prepared
            .set_shared_schema_worker_quota(quota)
            .map_err(operation_failure)?;
        self.diagnostics_v2_shared_quota_attached = true;
        Ok(())
    }

    /// Select diagnostics-v2 before any source schema check. v1 remains the
    /// default, and one operation can never check an instance in both modes.
    pub fn enable_diagnostics_v2(
        &mut self,
        limits: CutSchemaDiagnosticsLimits,
    ) -> Result<(), ItemRefusal> {
        if self.diagnostics_v2.is_some()
            || self.diagnostic_executions != 0
            || self.receipt_count != 0
            || self.finished
            || self.protocol_started
        {
            return Err(ItemRefusal::Unsupported(
                "schema operation already started".into(),
            ));
        }
        if !cut_diagnostics_limits_valid(
            self.limits.max_receipts,
            limits,
            self.prepared.operation_budget(),
        ) {
            return Err(ItemRefusal::BudgetCheck {
                check: "cut schema diagnostics limits",
                used: None,
                limit: None,
            });
        }
        self.diagnostics_v2 = Some(limits);
        self.diagnostics_v2_cost = Some(CutSchemaDiagnosticsCumulativeCost {
            completed_exchanges: 0,
            schema_resource_bytes: 0,
            request_bytes: 0,
            response_bytes: 0,
            worker_cpu_micros: 0,
        });
        Ok(())
    }

    /// Select the maximum exact source byte length accepted by the opt-in
    /// LegacyPythonObserved diagnostics-v2 scalar lane. Existing finite/v1
    /// input retains the independent one-MiB probe limit.
    pub fn set_diagnostics_v2_legacy_raw_instance_limit(
        &mut self,
        max_instance_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        let operation = self.prepared.operation_budget();
        if self.diagnostics_v2.is_none()
            || self.profile != FormatProfile::LegacyPythonObserved20260923
            || self.diagnostics_v2_controller_state_cap.is_some()
            || self.protocol_started
            || self.diagnostic_executions != 0
            || self.receipt_count != 0
            || self.finished
            || max_instance_bytes == 0
            || self.diagnostics_v2_legacy_selected_limits.is_some()
            || max_instance_bytes > BatchBudget::MAX_RAW_BYTES
            || max_instance_bytes > operation.batch.max_total_raw_bytes
            || u64::try_from(max_instance_bytes)
                .ok()
                .is_none_or(|bytes| bytes > operation.max_total_raw_bytes)
        {
            return Err(ItemRefusal::Unsupported(
                "legacy diagnostics-v2 input limit requires unused LegacyPythonObserved executor"
                    .into(),
            ));
        }
        self.diagnostics_v2_legacy_raw_instance_limit = max_instance_bytes;
        Ok(())
    }

    /// Bind the finite selected Legacy diagnostics-v2 sibling before attaching
    /// invocation quotas or a controller-state cap. Historical raw Legacy
    /// diagnostics remain on their original 300,000-visit mode.
    pub fn set_diagnostics_v2_legacy_selected_limits(
        &mut self,
        limits: LegacySelectedDiagnosticsLimits,
    ) -> Result<(), ItemRefusal> {
        let operation = self.prepared.operation_budget();
        if self.diagnostics_v2.is_none()
            || self.profile != FormatProfile::LegacyPythonObserved20260923
            || self.diagnostics_v2_legacy_selected_limits.is_some()
            || self.diagnostics_v2_legacy_raw_instance_limit
                != SchemaBackendProbe::MAX_INSTANCE_BYTES
            || self.diagnostics_v2_controller_state_cap.is_some()
            || self.diagnostics_v2_shared_quota_attached
            || self.protocol_started
            || self.diagnostic_executions != 0
            || self.receipt_count != 0
            || self.finished
            || limits.validate().is_err()
            || limits.max_instance_bytes > operation.batch.max_total_raw_bytes
            || u64::try_from(limits.max_instance_bytes)
                .ok()
                .is_none_or(|bytes| bytes > operation.max_total_raw_bytes)
        {
            return Err(ItemRefusal::Unsupported(
                "selected Legacy diagnostics-v2 limits require an unused LegacyPythonObserved executor"
                    .into(),
            ));
        }
        self.diagnostics_v2_legacy_selected_limits = Some(limits);
        Ok(())
    }

    /// Resolve an owner's exact raw schema bytes to the unique selected
    /// source path in this retained closure. The digest narrows the scan; the
    /// prepared worker's URI and raw resource bytes are compared as well.
    pub fn selected_contract_for_schema_raw(&self, schema_raw: &[u8]) -> Result<&str, ItemRefusal> {
        if schema_raw.is_empty() || schema_raw.len() > SchemaBackendProbe::MAX_RESOURCE_BYTES {
            return Err(ItemRefusal::Budget);
        }
        let digest = Digest256::of_bytes(schema_raw);
        let mut selected = None;
        for (path, (uri, selected_digest)) in &self.contracts {
            if *selected_digest != digest
                || !self.prepared.has_encoded_schema_resource(uri, schema_raw)
            {
                continue;
            }
            if selected.is_some() {
                return Err(ItemRefusal::Unsupported(
                    "source schema bytes select multiple contracts".into(),
                ));
            }
            selected = Some(path.as_str());
        }
        selected.ok_or_else(|| {
            ItemRefusal::Unsupported("source schema bytes are not in the selected closure".into())
        })
    }

    /// Allocation-free upper bound for one controller-side scalar check.
    /// `max_instance_bytes` is the caller's selected raw-member ceiling; the
    /// selected closure and any already retained rejection reports are read
    /// from this live executor. The exact retained encoded-closure capacity is
    /// included once, separately from its additional request-frame copy. This
    /// deliberately excludes the configured whole-operation state ceiling,
    /// which is not current live memory.
    pub fn diagnostics_v2_controller_state_upper_bound(
        &self,
        max_instance_bytes: usize,
        max_path_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        if self.diagnostics_v2.is_none() {
            return Err(ItemRefusal::Unsupported(
                "schema diagnostics v2 not selected".into(),
            ));
        }
        if max_instance_bytes == 0
            || max_instance_bytes > self.diagnostics_v2_instance_limit()
            || max_path_bytes == 0
            || max_path_bytes > 4096
        {
            return Err(ItemRefusal::Budget);
        }
        // Validate the exact retained closure without copying it. The
        // pre-callback owner may select a valid fragment URI whose rendered
        // form is longer than the base URI, so reserve the existing 4096-byte
        // selector and URI ceilings here; per-check admission below uses the
        // exact selected selector and rendered root URI lengths.
        self.prepared
            .max_encoded_schema_uri_bytes()
            .map_err(operation_failure)?;
        self.diagnostics_v2_controller_state_upper_bound_for_lengths(
            max_instance_bytes,
            max_path_bytes,
            4096,
            4096,
        )
    }

    /// Bind Host's reserved controller-state envelope before the first
    /// diagnostics-v2 check. This is a local admission cap; it does not alter
    /// the worker wire format or the existing worker resource limits.
    pub fn set_diagnostics_v2_controller_state_cap(
        &mut self,
        cap_bytes: usize,
    ) -> Result<(), ItemRefusal> {
        if self.diagnostics_v2.is_none()
            || cap_bytes == 0
            || self.diagnostics_v2_controller_state_cap.is_some()
            || self.protocol_started
            || self.diagnostic_executions != 0
            || self.finished
        {
            return Err(ItemRefusal::Unsupported(
                "controller-state cap requires unused diagnostics-v2 executor".into(),
            ));
        }
        self.diagnostics_v2_controller_state_cap = Some(cap_bytes);
        Ok(())
    }

    fn selected_contract_root_uri_bytes(&self, contract: &str) -> Option<usize> {
        let (base, fragment) = match contract.split_once('#') {
            Some((base, fragment))
                if !base.is_empty() && fragment.starts_with('/') && !fragment.contains('#') =>
            {
                (base, Some(fragment))
            }
            Some(_) => return None,
            None => (contract, None),
        };
        let base_bytes = self.contracts.get(base)?.0.len();
        fragment.map_or(Some(base_bytes), |suffix| {
            base_bytes.checked_add(1)?.checked_add(suffix.len())
        })
    }

    fn diagnostics_v2_uses_legacy_raw_input(&self) -> bool {
        self.profile == FormatProfile::LegacyPythonObserved20260923
    }

    fn diagnostics_v2_instance_limit(&self) -> usize {
        if let Some(limits) = self.diagnostics_v2_legacy_selected_limits {
            return limits.max_instance_bytes;
        }
        if self.diagnostics_v2_uses_legacy_raw_input() {
            self.diagnostics_v2_legacy_raw_instance_limit
        } else {
            SchemaBackendProbe::MAX_INSTANCE_BYTES
        }
    }

    fn diagnostics_v2_controller_state_upper_bound_for_lengths(
        &self,
        max_instance_bytes: usize,
        max_path_bytes: usize,
        max_contract_bytes: usize,
        root_uri_bytes: usize,
    ) -> Result<usize, ItemRefusal> {
        if max_instance_bytes == 0
            || max_instance_bytes > self.diagnostics_v2_instance_limit()
            || max_path_bytes == 0
            || max_path_bytes > 4096
            || max_contract_bytes == 0
            || max_contract_bytes > 4096
        {
            return Err(ItemRefusal::Budget);
        }
        if root_uri_bytes == 0 || root_uri_bytes > 4096 {
            return Err(ItemRefusal::Budget);
        }
        let member_id_bytes = "source-cut-schema-unit".len();
        let request_bytes = if self.diagnostics_v2_legacy_selected_limits.is_some() {
            self.prepared
                .diagnostics_v2_selected_legacy_request_frame_bytes_upper_bound(
                    member_id_bytes,
                    max_path_bytes,
                    root_uri_bytes,
                    max_instance_bytes,
                )
        } else {
            self.prepared
                .diagnostics_v2_request_frame_bytes_upper_bound(
                    member_id_bytes,
                    max_path_bytes,
                    root_uri_bytes,
                    max_instance_bytes,
                )
        }
        .map_err(operation_failure)?;
        let (request_buffer_bytes, response_buffer_bytes) = self
            .prepared
            .diagnostics_v2_request_response_bytes_upper_bound(request_bytes)
            .map_err(operation_failure)?;

        let pending_capacity_state = std::mem::size_of::<Vec<CutSchemaDiagnostic>>()
            .checked_add(
                self.pending_diagnostics
                    .capacity()
                    .checked_mul(std::mem::size_of::<CutSchemaDiagnostic>())
                    .ok_or(ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;
        let pending_state_bytes = self
            .pending_diagnostics
            .iter()
            .try_fold(pending_capacity_state, |total, diagnostic| {
                total.checked_add(cut_diagnostic_state_bytes(
                    &diagnostic.path,
                    &diagnostic.contract,
                    &diagnostic.unit,
                    &diagnostic.checkpoint,
                )?)
            })
            .ok_or(ItemRefusal::Budget)?;
        let retained_schema_closure_bytes = self.prepared.encoded_schema_resource_buffer_bytes();
        let issue_workspace = diagnostic_issue_workspace_upper_bound()?;
        let future_diagnostic_state = std::mem::size_of::<CutSchemaDiagnostic>()
            .checked_add(std::mem::size_of::<SchemaDiagnosticsCheckpoint>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<SchemaDiagnosticUnit>()))
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<BatchUnit>().checked_mul(3)?))
            .and_then(|bytes| bytes.checked_add(issue_workspace))
            .and_then(|bytes| bytes.checked_add(max_path_bytes.checked_mul(4)?))
            .and_then(|bytes| bytes.checked_add(max_contract_bytes))
            .and_then(|bytes| bytes.checked_add(root_uri_bytes.checked_mul(3)?))
            .and_then(|bytes| bytes.checked_add(member_id_bytes.checked_mul(3)?))
            .ok_or(ItemRefusal::Budget)?;
        let future_pending_capacity = self
            .pending_diagnostics
            .capacity()
            .max(1)
            .checked_mul(std::mem::size_of::<CutSchemaDiagnostic>())
            .ok_or(ItemRefusal::Budget)?;
        let input_instance_copies = max_instance_bytes
            .checked_mul(2)
            .ok_or(ItemRefusal::Budget)?;
        let request_buffer_multiplier = if self.diagnostics_v2_legacy_selected_limits.is_some() {
            3
        } else {
            2
        };
        let exchange_peak = request_buffer_bytes
            .checked_mul(request_buffer_multiplier)
            .and_then(|bytes| bytes.checked_add(response_buffer_bytes.checked_mul(2)?))
            .and_then(|bytes| bytes.checked_add(input_instance_copies))
            .and_then(|bytes| bytes.checked_add(future_diagnostic_state))
            .and_then(|bytes| bytes.checked_add(future_pending_capacity))
            .ok_or(ItemRefusal::Budget)?;
        let decode_peak = if self.diagnostics_v2_uses_legacy_raw_input() {
            0
        } else {
            finite_serde_json_controller_workspace_upper_bound(max_instance_bytes)?
                .checked_add(max_instance_bytes)
                .ok_or(ItemRefusal::Budget)?
        };
        pending_state_bytes
            .checked_add(retained_schema_closure_bytes)
            .and_then(|bytes| bytes.checked_add(exchange_peak.max(decode_peak)))
            .ok_or(ItemRefusal::Budget)
    }

    fn admit_diagnostics_v2_controller_state(
        &self,
        raw_bytes: usize,
        path_bytes: usize,
        contract: &str,
    ) -> Result<(), ItemRefusal> {
        let Some(cap_bytes) = self.diagnostics_v2_controller_state_cap else {
            return Ok(());
        };
        if raw_bytes > self.diagnostics_v2_instance_limit() {
            return Err(ItemRefusal::Budget);
        }
        let Some(root_uri_bytes) = self.selected_contract_root_uri_bytes(contract) else {
            // The existing input validator will return the specific static
            // contract-selection refusal without allocating a decoded value.
            return Ok(());
        };
        let required = self.diagnostics_v2_controller_state_upper_bound_for_lengths(
            raw_bytes,
            path_bytes,
            contract.len(),
            root_uri_bytes,
        )?;
        if required > cap_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "cut schema diagnostics controller state",
                used: u64::try_from(required).ok(),
                limit: u64::try_from(cap_bytes).ok(),
            });
        }
        Ok(())
    }

    fn admit_selected_legacy_child_address_space(
        &self,
        raw_bytes: usize,
        path_bytes: usize,
        contract: &str,
        limits: LegacySelectedDiagnosticsLimits,
    ) -> Result<(), ItemRefusal> {
        if raw_bytes > limits.max_instance_bytes {
            return Err(ItemRefusal::Budget);
        }
        let root_uri_bytes = self
            .selected_contract_root_uri_bytes(contract)
            .ok_or_else(|| ItemRefusal::Unsupported("schema contract selection".into()))?;
        let member_id_bytes = "source-cut-schema-unit".len();
        let request_bytes = self
            .prepared
            .diagnostics_v2_selected_legacy_request_frame_bytes_upper_bound(
                member_id_bytes,
                path_bytes,
                root_uri_bytes,
                raw_bytes,
            )
            .map_err(operation_failure)?;
        let response_bytes = self
            .prepared
            .diagnostics_v2_selected_legacy_response_buffer_bytes_upper_bound()
            .map_err(operation_failure)?;
        let required = legacy_selected_child_address_space_required(
            self.resource_preparation_state_upper_bound,
            request_bytes,
            response_bytes,
            member_id_bytes,
            path_bytes,
            root_uri_bytes,
            raw_bytes,
            self.prepared
                .worker_image_bytes()
                .map_err(operation_failure)?,
            limits,
        )
        .map_err(operation_failure)?;
        let operation = self.prepared.operation_budget();
        let child_limit = self
            .budget
            .address_space_bytes
            .min(operation.batch.address_space_bytes)
            .min(operation.operation_address_space_bytes);
        if required > child_limit {
            return Err(ItemRefusal::BudgetCheck {
                check: "cut schema diagnostics selected Legacy child address space",
                used: Some(required),
                limit: Some(child_limit),
            });
        }
        Ok(())
    }

    /// Diagnostics bridge for callbacks that receive exact selected schema
    /// bytes. Schema identity is resolved from those bytes before the ordinary
    /// same-kernel diagnostics-v2 execution.
    pub fn check_diagnostics_v2_for_schema_raw(
        &mut self,
        path: &str,
        raw: &[u8],
        schema_raw: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaDiagnostic, ItemRefusal> {
        self.prepared
            .preflight(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        let selected_contract = self.selected_contract_for_schema_raw(schema_raw)?;
        self.admit_diagnostics_v2_controller_state(raw.len(), path.len(), selected_contract)?;
        let contract = selected_contract.to_owned();
        self.check_diagnostics_v2(path, raw, &contract, deadline, cancelled)
    }

    /// Runs exactly one diagnostics-v2 unit through the already prepared
    /// closure and sealed image. The complete typed report is returned only
    /// after all bindings and aggregate caps are verified.
    pub fn check_diagnostics_v2(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaDiagnostic, ItemRefusal> {
        if self.diagnostics_v2.is_none() {
            return Err(ItemRefusal::Unsupported(
                "schema diagnostics v2 not selected".into(),
            ));
        }
        let result = self.check_diagnostics_v2_inner_with_selected_limits(
            path, raw, contract, deadline, cancelled, None, false,
        );
        if result.is_err() {
            self.diagnostics_v2_cost_unknown = true;
            self.prepared.poison(ExecutorFailure::Protocol);
        }
        result
    }

    /// Run one request-bound Legacy selected diagnostics-v2 unit using the
    /// exact cached contract reference already owned by this source cut.
    pub fn check_diagnostics_v2_legacy_selected(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaDiagnostic, ItemRefusal> {
        let limits = self.diagnostics_v2_legacy_selected_limits.ok_or_else(|| {
            ItemRefusal::Unsupported("selected Legacy diagnostics-v2 limits not selected".into())
        })?;
        if self.diagnostics_v2.is_none() {
            return Err(ItemRefusal::Unsupported(
                "schema diagnostics v2 not selected".into(),
            ));
        }
        let result = self.check_diagnostics_v2_inner_with_selected_limits(
            path,
            raw,
            contract,
            deadline,
            cancelled,
            Some(limits),
            false,
        );
        if result.is_err() {
            self.diagnostics_v2_cost_unknown = true;
            self.prepared.poison(ExecutorFailure::Protocol);
        }
        result
    }

    /// Retrieve the next actual complete invalid diagnostics report produced
    /// through the trait's bool projection. Valid v2 reports are not retained.
    pub fn take_schema_diagnostic_rejection(&mut self) -> Option<CutSchemaDiagnostic> {
        let index = self
            .pending_diagnostics
            .iter()
            .position(CutSchemaDiagnostic::is_invalid)?;
        Some(self.pending_diagnostics.remove(index))
    }

    /// Retrieve a fully authenticated non-verdict terminal retained only by
    /// the opt-in spool route. Ordinary diagnostics calls preserve their
    /// established refusal and poison behavior without queueing these reports.
    pub(crate) fn take_spooled_diagnostics_v2_status_refusal(
        &mut self,
    ) -> Option<CutSchemaDiagnostic> {
        let index = self
            .pending_diagnostics
            .iter()
            .position(|diagnostic| !diagnostic.is_valid() && !diagnostic.is_invalid())?;
        Some(self.pending_diagnostics.remove(index))
    }

    fn produce_diagnostics_v2_terminal(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaDiagnostic, ItemRefusal> {
        let diagnostic = if self.diagnostics_v2_legacy_selected_limits.is_some() {
            self.check_diagnostics_v2_legacy_selected(path, raw, contract, deadline, cancelled)?
        } else {
            self.check_diagnostics_v2(path, raw, contract, deadline, cancelled)?
        };
        if diagnostic.is_valid() || diagnostic.is_invalid() {
            return Ok(diagnostic);
        }
        self.prepared.poison(ExecutorFailure::Protocol);
        self.diagnostics_v2_cost_unknown = true;
        Err(ItemRefusal::Unsupported(
            "cut schema diagnostics status incomplete".into(),
        ))
    }

    /// Spool-only opt-in for retaining a fully authenticated non-verdict
    /// terminal. The ordinary public diagnostics API keeps its existing
    /// status refusal and poison behavior.
    fn produce_authenticated_diagnostics_v2_terminal_for_spool(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaDiagnostic, ItemRefusal> {
        let result = self.check_diagnostics_v2_inner_with_selected_limits(
            path,
            raw,
            contract,
            deadline,
            cancelled,
            self.diagnostics_v2_legacy_selected_limits,
            true,
        );
        if result.is_err() {
            self.diagnostics_v2_cost_unknown = true;
            self.prepared.poison(ExecutorFailure::Protocol);
        }
        result
    }

    fn retain_pending_diagnostic(
        &mut self,
        mut diagnostic: CutSchemaDiagnostic,
    ) -> Result<(), ItemRefusal> {
        self.precharge_pending_diagnostic(&mut diagnostic)?;
        if self.pending_diagnostics.len() >= self.pending_diagnostics.capacity() {
            self.prepared.poison(ExecutorFailure::Protocol);
            return Err(ItemRefusal::Unsupported(
                "cut schema diagnostics retained vector slot unavailable".into(),
            ));
        }
        self.pending_diagnostics.push(diagnostic);
        Ok(())
    }

    fn precharge_pending_diagnostic(
        &mut self,
        diagnostic: &mut CutSchemaDiagnostic,
    ) -> Result<(), ItemRefusal> {
        let limits = self
            .diagnostics_v2
            .ok_or_else(|| ItemRefusal::Unsupported("schema diagnostics v2 not selected".into()))?;
        let required_len = self
            .pending_diagnostics
            .len()
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        let previous_bytes = self
            .pending_diagnostics
            .capacity()
            .checked_mul(std::mem::size_of::<CutSchemaDiagnostic>())
            .ok_or(ItemRefusal::Budget)?;
        let will_grow = required_len > self.pending_diagnostics.capacity();
        let requested_capacity = if will_grow {
            required_len
        } else {
            self.pending_diagnostics.capacity()
        };
        let requested_capacity_bytes = requested_capacity
            .checked_mul(std::mem::size_of::<CutSchemaDiagnostic>())
            .ok_or(ItemRefusal::Budget)?;
        let transient_state_bytes = self
            .diagnostic_state_bytes_used
            .checked_add(if will_grow {
                requested_capacity_bytes
            } else {
                0
            })
            .ok_or(ItemRefusal::Budget)?;
        if transient_state_bytes > limits.max_total_state_bytes {
            self.prepared.poison(ExecutorFailure::InputBudget);
            return Err(ItemRefusal::BudgetCheck {
                check: "cut schema diagnostics retained vector allocation",
                used: u64::try_from(transient_state_bytes).ok(),
                limit: u64::try_from(limits.max_total_state_bytes).ok(),
            });
        }
        if self.pending_diagnostics.try_reserve_exact(1).is_err() {
            self.prepared.poison(ExecutorFailure::InputBudget);
            return Err(ItemRefusal::Budget);
        }
        let next_capacity_bytes = self
            .pending_diagnostics
            .capacity()
            .checked_mul(std::mem::size_of::<CutSchemaDiagnostic>())
            .ok_or(ItemRefusal::Budget)?;
        let Some(new_capacity_bytes) = next_capacity_bytes.checked_sub(previous_bytes) else {
            self.prepared.poison(ExecutorFailure::Protocol);
            return Err(ItemRefusal::Unsupported(
                "cut schema diagnostics retained vector capacity changed".into(),
            ));
        };
        let Some(next_state_bytes) = self
            .diagnostic_state_bytes_used
            .checked_add(new_capacity_bytes)
            .filter(|bytes| *bytes <= limits.max_total_state_bytes)
        else {
            self.prepared.poison(ExecutorFailure::InputBudget);
            self.pending_diagnostics = Vec::new();
            return Err(ItemRefusal::BudgetCheck {
                check: "cut schema diagnostics retained vector",
                used: self
                    .diagnostic_state_bytes_used
                    .checked_add(new_capacity_bytes)
                    .and_then(|bytes| u64::try_from(bytes).ok()),
                limit: u64::try_from(limits.max_total_state_bytes).ok(),
            });
        };
        diagnostic.accounted_state_bytes = diagnostic
            .accounted_state_bytes
            .checked_add(new_capacity_bytes)
            .ok_or(ItemRefusal::Budget)?;
        self.diagnostic_state_bytes_used = next_state_bytes;
        Ok(())
    }

    pub fn diagnostic_execution_count(&self) -> usize {
        self.diagnostic_executions
    }

    /// Return exact aggregate cost only while every attempted diagnostics-v2
    /// exchange has a complete validated receipt. v1 and failed/incomplete
    /// histories explicitly refuse because their actual cost is unavailable.
    pub fn diagnostics_v2_cumulative_cost(
        &self,
    ) -> Result<CutSchemaDiagnosticsCumulativeCost, ItemRefusal> {
        if self.diagnostics_v2.is_none() {
            return Err(ItemRefusal::Unsupported(
                "diagnostics-v2 cumulative cost is unavailable for v1".into(),
            ));
        }
        if self.diagnostics_v2_cost_unknown {
            return Err(ItemRefusal::Unsupported(
                "diagnostics-v2 cumulative cost is unknown after an incomplete exchange".into(),
            ));
        }
        self.diagnostics_v2_cost.ok_or_else(|| {
            ItemRefusal::Unsupported("diagnostics-v2 cumulative cost was not observed".into())
        })
    }

    fn check_diagnostics_v2_inner_with_selected_limits(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
        selected_limits: Option<LegacySelectedDiagnosticsLimits>,
        retain_nonverdict_terminal: bool,
    ) -> Result<CutSchemaDiagnostic, ItemRefusal> {
        self.prepared
            .preflight(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        let limits = self
            .diagnostics_v2
            .ok_or_else(|| ItemRefusal::Unsupported("schema diagnostics v2 not selected".into()))?;
        if selected_limits != self.diagnostics_v2_legacy_selected_limits
            || selected_limits.is_some_and(|selected| {
                self.profile != FormatProfile::LegacyPythonObserved20260923
                    || selected.max_instance_bytes > self.diagnostics_v2_instance_limit()
            })
        {
            self.prepared.poison(ExecutorFailure::Protocol);
            return Err(ItemRefusal::Unsupported(
                "selected Legacy diagnostics-v2 binding changed".into(),
            ));
        }
        let next_count = self
            .diagnostic_executions
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_receipts)
            .filter(|count| *count as u64 <= self.prepared.operation_budget().max_chunks)
            .filter(|count| *count as u64 <= self.prepared.operation_budget().max_total_units)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "cut schema diagnostics execution count",
                used: self
                    .diagnostic_executions
                    .checked_add(1)
                    .and_then(|n| u64::try_from(n).ok()),
                limit: Some(self.limits.max_receipts as u64),
            })?;
        self.admit_diagnostics_v2_controller_state(raw.len(), path.len(), contract)?;
        let input_mode = if selected_limits.is_some() {
            let selected = selected_limits.ok_or(ItemRefusal::Budget)?;
            if raw.len() > selected.max_instance_bytes {
                return Err(ItemRefusal::Budget);
            }
            DiagnosticsUnitInputMode::LegacyPythonObservedSelected
        } else if self.diagnostics_v2_uses_legacy_raw_input() {
            if raw.len() > self.diagnostics_v2_legacy_raw_instance_limit {
                return Err(ItemRefusal::Budget);
            }
            DiagnosticsUnitInputMode::LegacyPythonObserved
        } else {
            DiagnosticsUnitInputMode::FiniteJson
        };
        if let Some(selected) = selected_limits {
            self.admit_selected_legacy_child_address_space(
                raw.len(),
                path.len(),
                contract,
                selected,
            )?;
        }
        let (uri, worker_raw) = if matches!(
            input_mode,
            DiagnosticsUnitInputMode::LegacyPythonObserved
                | DiagnosticsUnitInputMode::LegacyPythonObservedSelected
        ) {
            (self.contract_root_uri(contract)?, raw.to_vec())
        } else {
            self.decoded_input(raw, contract)?
        };
        let expected_unit = BatchUnit {
            ordinal: 0,
            member_id: "source-cut-schema-unit".into(),
            relative_path: path.to_owned(),
            root_uri: uri,
            raw_instance: worker_raw,
        };
        let expected = if let Some(selected) = selected_limits {
            BatchCoverageExpectation::from_selected_legacy_diagnostics_units(
                std::slice::from_ref(&expected_unit),
                selected,
            )
        } else {
            BatchCoverageExpectation::from_diagnostics_units(
                std::slice::from_ref(&expected_unit),
                if input_mode == DiagnosticsUnitInputMode::LegacyPythonObserved {
                    crate::executor::DiagnosticsInputProfile::LegacyPythonObserved
                } else {
                    crate::executor::DiagnosticsInputProfile::FiniteJson
                },
                None,
            )
        }
        .map_err(|_| {
            self.prepared.poison(ExecutorFailure::CoverageMismatch);
            ItemRefusal::Budget
        })?;
        let expected_unit_sha = if let Some(selected) = selected_limits {
            selected_legacy_diagnostics_batch_unit_digest(&expected_unit, selected)
        } else {
            crate::executor::diagnostics_batch_unit_digest(&expected_unit, input_mode)
        }
        .map_err(|_| {
            self.prepared.poison(ExecutorFailure::CoverageMismatch);
            ItemRefusal::Budget
        })?;
        let decoded_sha256 = Digest256::of_bytes(&expected_unit.raw_instance);
        let source_raw_sha256 = Digest256::of_bytes(raw);
        let mut budget = self.budget;
        budget.execution_wall = budget.execution_wall.min(
            deadline
                .checked_duration_since(Instant::now())
                .ok_or(ItemRefusal::Deadline)?,
        );
        self.protocol_started = true;
        let execution = if input_mode == DiagnosticsUnitInputMode::LegacyPythonObservedSelected {
            self.prepared.evaluate_with_selected_legacy_diagnostics(
                "source-cut-schema-unit",
                path,
                &expected_unit.root_uri,
                &expected_unit.raw_instance,
                selected_limits.ok_or(ItemRefusal::Budget)?,
                self.resource_preparation_state_upper_bound,
                budget,
                deadline,
                cancelled,
            )
        } else if input_mode == DiagnosticsUnitInputMode::LegacyPythonObserved {
            self.prepared.evaluate_with_legacy_diagnostics(
                "source-cut-schema-unit",
                path,
                &expected_unit.root_uri,
                &expected_unit.raw_instance,
                budget,
                deadline,
                cancelled,
            )
        } else {
            self.prepared.evaluate_with_diagnostics(
                "source-cut-schema-unit",
                path,
                &expected_unit.root_uri,
                &expected_unit.raw_instance,
                budget,
                deadline,
                cancelled,
            )
        };
        let (outcome, cost) = execution
            .map_err(|reason| diagnostics_refusal(reason, self.prepared.exchange_failure()))?;
        let (mut units, checkpoint) = match outcome {
            SchemaDiagnosticsOutcome::Complete { units, checkpoint } => (units, checkpoint),
            SchemaDiagnosticsOutcome::Incomplete {
                reason, exchange, ..
            } => {
                return Err(diagnostics_refusal(reason, exchange));
            }
        };
        let unit = units.pop().ok_or_else(|| {
            self.prepared.poison(ExecutorFailure::CoverageMismatch);
            ItemRefusal::Unsupported("cut schema diagnostics coverage incomplete".into())
        })?;
        let report = &unit.report;
        let caps_sha256 = schema_diagnostics::Caps::CURRENT.digest();
        if units.len() != 0
            || checkpoint.completed_count != 1
            || checkpoint.worker_sha256 != self.worker.sha256
            || checkpoint.profile != self.profile
            || checkpoint.schema_set_sha256 != self.schema_set_digest
            || checkpoint.ordered_manifest_sha256 != expected.ordered_manifest_sha256
            || checkpoint.caps_sha256 != caps_sha256
            || checkpoint.request_sha256 != report.request_sha256
            || unit.ordinal != 0
            || unit.member_id != expected_unit.member_id
            || unit.relative_path != path
            || unit.root_uri != expected_unit.root_uri
            || unit.raw_sha256 != decoded_sha256
            || unit.unit_sha256 != expected_unit_sha
            || report.worker_sha256 != self.worker.sha256
            || report.request_sha256 != checkpoint.request_sha256
            || report.unit_sha256 != unit.unit_sha256
            || report.schema_set_sha256 != self.schema_set_digest
            || report.caps_sha256() != caps_sha256
            || !report.is_well_formed()
            || cost.worker_cpu_micros.is_none()
        {
            self.prepared.poison(ExecutorFailure::Protocol);
            return Err(ItemRefusal::Unsupported(
                "cut schema diagnostics binding incomplete".into(),
            ));
        }
        if !retain_nonverdict_terminal
            && !matches!(
                report.status,
                schema_diagnostics::Status::Valid | schema_diagnostics::Status::Invalid
            )
        {
            // The status and counts are observable only after the complete
            // worker/request/unit/report binding above has been authenticated.
            // Keep this fail-closed and expose only protocol enums and counts,
            // never worker-provided issue text or other free-form detail.
            self.prepared.poison(ExecutorFailure::Protocol);
            return Err(diagnostic_status_refusal(report));
        }
        let issue_count = report.issues.len();
        let next_issues = self
            .diagnostic_issues_used
            .checked_add(issue_count)
            .filter(|count| *count <= limits.max_total_issues)
            .ok_or_else(|| {
                self.prepared.poison(ExecutorFailure::InputBudget);
                ItemRefusal::BudgetCheck {
                    check: "cut schema diagnostics issues",
                    used: self
                        .diagnostic_issues_used
                        .checked_add(issue_count)
                        .and_then(|n| u64::try_from(n).ok()),
                    limit: u64::try_from(limits.max_total_issues).ok(),
                }
            })?;
        let next_report_bytes = self
            .diagnostic_report_bytes_used
            .checked_add(cost.response_bytes)
            .filter(|bytes| *bytes <= limits.max_total_report_bytes)
            .ok_or_else(|| {
                self.prepared.poison(ExecutorFailure::InputBudget);
                ItemRefusal::BudgetCheck {
                    check: "cut schema diagnostics report bytes",
                    used: self
                        .diagnostic_report_bytes_used
                        .checked_add(cost.response_bytes)
                        .and_then(|n| u64::try_from(n).ok()),
                    limit: u64::try_from(limits.max_total_report_bytes).ok(),
                }
            })?;
        let input_metadata_bytes = std::mem::size_of::<BatchUnit>()
            .checked_add(expected_unit.member_id.capacity())
            .and_then(|bytes| bytes.checked_add(expected_unit.relative_path.capacity()))
            .and_then(|bytes| bytes.checked_add(expected_unit.root_uri.capacity()))
            .and_then(|bytes| bytes.checked_add(path.len()))
            .and_then(|bytes| bytes.checked_add(contract.len()))
            .ok_or(ItemRefusal::Budget)?;
        let retained_path = path.to_owned();
        let retained_contract = contract.to_owned();
        let retained_state_bytes =
            cut_diagnostic_state_bytes(&retained_path, &retained_contract, &unit, &checkpoint)
                .ok_or(ItemRefusal::Budget)?;
        let closure_charge = if self.diagnostic_executions == 0 {
            cost.schema_resource_buffer_bytes
        } else {
            0
        };
        let input_buffer_bytes = expected_unit
            .raw_instance
            .capacity()
            .checked_add(cost.input_instance_buffer_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let accounted_state_bytes = closure_charge
            .checked_add(input_buffer_bytes)
            .and_then(|bytes| bytes.checked_add(cost.request_buffer_bytes))
            .and_then(|bytes| bytes.checked_add(cost.response_buffer_bytes))
            .and_then(|bytes| bytes.checked_add(input_metadata_bytes))
            .and_then(|bytes| bytes.checked_add(retained_state_bytes))
            .ok_or(ItemRefusal::Budget)?;
        let next_state_bytes = self
            .diagnostic_state_bytes_used
            .checked_add(accounted_state_bytes)
            .filter(|bytes| *bytes <= limits.max_total_state_bytes)
            .ok_or_else(|| {
                self.prepared.poison(ExecutorFailure::InputBudget);
                ItemRefusal::BudgetCheck {
                    check: "cut schema diagnostics state bytes",
                    used: self
                        .diagnostic_state_bytes_used
                        .checked_add(accounted_state_bytes)
                        .and_then(|n| u64::try_from(n).ok()),
                    limit: u64::try_from(limits.max_total_state_bytes).ok(),
                }
            })?;
        let worker_cpu_micros = cost.worker_cpu_micros.ok_or_else(|| {
            self.prepared.poison(ExecutorFailure::ResourceLimitUnknown);
            ItemRefusal::Unsupported("cut schema diagnostics cpu accounting unavailable".into())
        })?;
        let next_cumulative_cost = self.diagnostics_v2_cost.and_then(|previous| {
            previous.checked_add_exchange(
                cost.schema_resource_bytes,
                cost.request_bytes,
                cost.response_bytes,
                worker_cpu_micros,
            )
        });
        let Some(next_cumulative_cost) = next_cumulative_cost else {
            self.diagnostics_v2_cost_unknown = true;
            self.prepared.poison(ExecutorFailure::ResourceLimitUnknown);
            return Err(ItemRefusal::BudgetCheck {
                check: "cut schema diagnostics cumulative cost",
                used: None,
                limit: None,
            });
        };
        let aggregate_caps_sha256 = cut_diagnostics_caps_digest(
            self.limits,
            limits,
            self.prepared.operation_budget(),
            self.diagnostics_v2_controller_state_cap,
            (self.diagnostics_v2_legacy_selected_limits.is_none()
                && self.diagnostics_v2_uses_legacy_raw_input())
            .then_some(self.diagnostics_v2_legacy_raw_instance_limit),
            self.diagnostics_v2_legacy_selected_limits,
        );
        self.prepared
            .preflight(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        let diagnostic = CutSchemaDiagnostic {
            path: retained_path,
            contract: retained_contract,
            source_revision: self.revision,
            source_raw_sha256,
            decoded_instance_sha256: decoded_sha256,
            aggregate_caps_sha256,
            checkpoint,
            unit,
            schema_resource_bytes: cost.schema_resource_bytes,
            schema_resource_buffer_bytes: cost.schema_resource_buffer_bytes,
            input_instance_bytes: expected_unit.raw_instance.len(),
            input_instance_buffer_bytes: input_buffer_bytes,
            input_metadata_bytes,
            request_bytes: cost.request_bytes,
            request_buffer_bytes: cost.request_buffer_bytes,
            response_bytes: cost.response_bytes,
            response_buffer_bytes: cost.response_buffer_bytes,
            worker_cpu_micros,
            retained_state_bytes,
            accounted_state_bytes,
        };
        self.diagnostics_v2_cost = Some(next_cumulative_cost);
        self.diagnostic_executions = next_count;
        self.diagnostic_issues_used = next_issues;
        self.diagnostic_report_bytes_used = next_report_bytes;
        self.diagnostic_state_bytes_used = next_state_bytes;
        Ok(diagnostic)
    }

    /// Explicit reuse of an actual scalar execution within this immutable
    /// operation. A hit is not a new execution receipt or source observation.
    /// Callers still own every current byte read and predicate observation.
    pub fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        self.prepared
            .preflight(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        if self.diagnostics_v2.is_some() {
            return self.check(path, raw, contract, deadline, cancelled);
        }
        // Reapply the same selected-contract/fragment and raw/decoded admission
        // bounds even on a hit; a receipt cannot bypass current operation law.
        let (_, decoded) = self.decoded_input(raw, contract)?;
        let raw_sha = Digest256::of_bytes(raw);
        let decoded_sha = Digest256::of_bytes(&decoded);
        let hit = self
            .receipts
            .iter()
            .rev()
            .find(|receipt| {
                receipt.batch.is_none()
                    && receipt.path == path
                    && receipt.contract == contract
                    && receipt.source_revision == self.revision
                    && receipt.source_raw_sha256 == raw_sha
                    && receipt.decoded_instance_sha256 == decoded_sha
                    && receipt.execution.instance_sha256 == decoded_sha
                    && receipt.execution.schema_set_sha256 == self.schema_set_digest
                    && receipt.execution.profile == self.profile
                    && receipt.execution.worker_sha256 == self.worker.sha256
            })
            .map(|receipt| receipt.valid);
        drop(decoded);
        if let Some(valid) = hit {
            self.prepared
                .preflight(deadline, cancelled)
                .map_err(|reason| {
                    operation_failure_with_context(reason, self.prepared.exchange_failure())
                })?;
            return Ok(valid);
        }
        self.check(path, raw, contract, deadline, cancelled)
    }

    fn contract_root_uri(&self, contract: &str) -> Result<String, ItemRefusal> {
        let (base, fragment) = match contract.split_once('#') {
            Some((base, fragment))
                if !base.is_empty() && fragment.starts_with('/') && !fragment.contains('#') =>
            {
                (base, Some(fragment))
            }
            Some(_) => {
                return Err(ItemRefusal::Unsupported(
                    "invalid source schema fragment selector".into(),
                ));
            }
            None => (contract, None),
        };
        let base_uri = &self
            .contracts
            .get(base)
            .ok_or_else(|| ItemRefusal::Unsupported(format!("missing source schema {contract}")))?
            .0;
        let uri_bytes = fragment.map_or(Some(base_uri.len()), |suffix| {
            base_uri.len().checked_add(1)?.checked_add(suffix.len())
        });
        if uri_bytes.is_none_or(|bytes| bytes > 4096) {
            return Err(ItemRefusal::Budget);
        }
        Ok(fragment.map_or_else(|| base_uri.clone(), |f| format!("{base_uri}#{f}")))
    }

    fn decoded_input(&self, raw: &[u8], contract: &str) -> Result<(String, Vec<u8>), ItemRefusal> {
        if raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES {
            return Err(ItemRefusal::Budget);
        }
        let uri = self.contract_root_uri(contract)?;
        let decoded: serde_json::Value = serde_json::from_slice(raw).map_err(|_| {
            ItemRefusal::Unsupported("unsupported native decoded JSON representation".into())
        })?;
        let worker_raw = serde_json::to_vec(&decoded)
            .map_err(|_| ItemRefusal::Unsupported("native decoded JSON serialization".into()))?;
        if worker_raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES {
            return Err(ItemRefusal::Budget);
        }
        Ok((uri, worker_raw))
    }

    pub(crate) fn operation_budget(&self) -> BatchStreamBudget {
        self.prepared.operation_budget()
    }
    pub(crate) fn receipt_limit_bytes(&self) -> usize {
        self.limits.max_receipt_bytes
    }
    /// Known General owner phase barrier, preserving the original envelope.
    /// Poisoned operations never resume; this is not failure recovery.
    pub(crate) fn release_child(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        self.prepared
            .release_child(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })
    }

    pub fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        let base = contract.split_once('#').map_or(contract, |(base, _)| base);
        self.contracts.get(base).map(|(_, digest)| *digest)
    }

    pub fn receipts(&self) -> &[CutSchemaReceipt] {
        &self.receipts
    }

    pub fn receipt_count(&self) -> usize {
        self.receipt_count
    }

    /// Move this verified executor into the opt-in bounded receipt store.
    /// The caller-created auxiliary scope must share the caller's already
    /// admitted filesystem namespace, logical I/O and space ledgers, deadline,
    /// and cancellation lifetime; this conversion grants no quota itself.
    pub fn into_spooling(
        self,
        workspace_dir: std::fs::File,
        request: tos_source_store::PinnedSqliteAuxRequest,
        limits: CutSchemaReceiptSpoolLimits,
        deadline: Instant,
        cancelled: std::sync::Arc<AtomicBool>,
    ) -> Result<CutWorkerSchemaExecutorSpooling, ItemRefusal> {
        CutWorkerSchemaExecutorSpooling::new(
            self,
            workspace_dir,
            request,
            limits,
            deadline,
            cancelled,
        )
    }

    pub fn source_revision(&self) -> SourceRevision {
        self.revision
    }

    pub fn execution_binding(&self) -> CutExecutionBinding {
        CutExecutionBinding {
            source_revision: self.revision,
            schema_profile: self.profile,
            schema_set_sha256: self.schema_set_digest,
            worker_sha256: self.worker.sha256,
        }
    }
}

impl CutSchemaExecutor for CutWorkerSchemaExecutor {
    fn selected_source_revision(&self) -> Option<SourceRevision> {
        Some(CutWorkerSchemaExecutor::source_revision(self))
    }

    fn check_reusing_scalar(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        CutWorkerSchemaExecutor::check_reusing_scalar(
            self, path, raw, contract, deadline, cancelled,
        )
    }

    fn schema_input_cost(
        &self,
        path: &str,
        raw: &[u8],
        contract: &str,
        ordinal: u64,
    ) -> Result<CutSchemaInputCost, ItemRefusal> {
        if self.diagnostics_v2.is_some() {
            return Err(ItemRefusal::Unsupported(
                "schema diagnostics v2 input cost unavailable".into(),
            ));
        }
        let (uri, worker_raw) = self.decoded_input(raw, contract)?;
        // The exact same identity guard used by the transport manifest.
        let unit = BatchUnit {
            ordinal,
            member_id: ordinal.to_string(),
            relative_path: path.into(),
            root_uri: uri.clone(),
            raw_instance: worker_raw,
        };
        crate::executor::validate_batch_unit(&unit).map_err(|_| ItemRefusal::Budget)?;
        let (operation, frame, wire) = self
            .prepared
            .wire_cost(
                unit.member_id.len(),
                path.len(),
                uri.len(),
                unit.raw_instance.len(),
            )
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        let receipt = path
            .len()
            .checked_add(contract.len())
            .and_then(|n| n.checked_add(std::mem::size_of::<CutSchemaReceipt>()))
            .ok_or(ItemRefusal::Budget)?;
        Ok(CutSchemaInputCost {
            decoded_instance_bytes: unit.raw_instance.len() as u64,
            unit_wire_bytes: wire,
            operation_wire_bytes: operation,
            frame_wire_bytes: frame,
            receipt_bytes: receipt as u64,
            remaining_receipts: self.limits.max_receipts.saturating_sub(self.receipt_count) as u64,
            remaining_receipt_bytes: self
                .limits
                .max_receipt_bytes
                .saturating_sub(self.receipt_bytes) as u64,
            selector: uri,
        })
    }

    fn set_operation_budget(&mut self, budget: BatchStreamBudget) -> Result<(), ItemRefusal> {
        CutWorkerSchemaExecutor::set_operation_budget(self, budget)
    }

    fn set_shared_schema_worker_quota(
        &mut self,
        quota: SharedSchemaWorkerQuota,
    ) -> Result<(), ItemRefusal> {
        CutWorkerSchemaExecutor::set_shared_schema_worker_quota(self, quota)
    }

    fn finish(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
        self.prepared
            .finish(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        self.finished = true;
        Ok(())
    }

    fn check_batch(
        &mut self,
        checks: &[CutSchemaCheck],
        mut budget: BatchBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<bool>, ItemRefusal> {
        if self.diagnostics_v2.is_some() {
            self.prepared.poison_shared_schema_worker_quota();
            return Err(ItemRefusal::Unsupported(
                "schema diagnostics v2 batch execution unavailable".into(),
            ));
        }
        self.prepared
            .preflight(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        if budget.max_units == 0
            || budget.max_units > BatchBudget::MAX_UNITS
            || budget.max_total_raw_bytes == 0
            || budget.max_total_raw_bytes > BatchBudget::MAX_RAW_BYTES
            || checks.is_empty()
            || checks.len() > budget.max_units
            || self
                .receipt_count
                .checked_add(checks.len())
                .filter(|n| *n <= self.limits.max_receipts)
                .is_none()
        {
            return Err(ItemRefusal::Budget);
        }
        let mut next_bytes = self.receipt_bytes;
        let mut total_raw = 0usize;
        let mut units = Vec::with_capacity(checks.len());
        let mut raw_digests = Vec::with_capacity(checks.len());
        for (ordinal, input) in checks.iter().enumerate() {
            self.prepared
                .preflight(deadline, cancelled)
                .map_err(|reason| {
                    operation_failure_with_context(reason, self.prepared.exchange_failure())
                })?;
            next_bytes = input
                .path
                .len()
                .checked_add(input.contract.len())
                .and_then(|n| n.checked_add(std::mem::size_of::<CutSchemaReceipt>()))
                .and_then(|n| next_bytes.checked_add(n))
                .filter(|n| *n <= self.limits.max_receipt_bytes)
                .ok_or(ItemRefusal::Budget)?;
            let (uri, worker_raw) = self.decoded_input(&input.raw, &input.contract)?;
            total_raw = total_raw
                .checked_add(worker_raw.len())
                .filter(|n| *n <= budget.max_total_raw_bytes)
                .ok_or(ItemRefusal::Budget)?;
            raw_digests.push(Digest256::of_bytes(&input.raw));
            units.push(BatchUnit {
                ordinal: ordinal as u64,
                member_id: ordinal.to_string(),
                relative_path: input.path.clone(),
                root_uri: uri,
                raw_instance: worker_raw,
            });
        }
        let expected =
            BatchCoverageExpectation::from_units(&units).map_err(|_| ItemRefusal::Budget)?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(ItemRefusal::Deadline)?;
        budget.total_execution_wall = budget.total_execution_wall.min(remaining);
        budget.startup_wall = budget.startup_wall.min(budget.total_execution_wall);
        budget.per_unit_wall = budget.per_unit_wall.min(budget.total_execution_wall);
        self.protocol_started = true;
        let outcome = self
            .prepared
            .evaluate_batch(&units, expected, budget, deadline, cancelled);
        if matches!(&outcome, BatchOutcome::Complete { .. }) {
            self.prepared
                .preflight(deadline, cancelled)
                .map_err(|reason| {
                    operation_failure_with_context(reason, self.prepared.exchange_failure())
                })?;
        }
        let (receipts, checkpoint) = match outcome {
            BatchOutcome::Complete {
                receipts,
                checkpoint,
            } => (receipts, checkpoint),
            BatchOutcome::Incomplete {
                reason: ExecutorFailure::Timeout,
                ..
            } => return Err(ItemRefusal::Deadline),
            BatchOutcome::Incomplete {
                reason: ExecutorFailure::Cancelled,
                ..
            } => return Err(ItemRefusal::Source("schema execution cancelled".into())),
            other => {
                return Err(ItemRefusal::Unsupported(format!(
                    "schema batch execution incomplete: {other:?}"
                )));
            }
        };
        if checkpoint.worker_sha256 != self.worker.sha256
            || checkpoint.schema_set_sha256 != self.schema_set_digest
            || checkpoint.profile != self.profile
            || checkpoint.ordered_manifest_sha256 != expected.ordered_manifest_sha256
            || checkpoint.completed_count != expected.count
            || receipts.len() != checks.len()
        {
            return Err(ItemRefusal::Source("schema batch identity mismatch".into()));
        }
        let mut staged = Vec::with_capacity(checks.len());
        let mut verdicts = Vec::with_capacity(checks.len());
        for (ordinal, ((input, unit), receipt)) in
            checks.iter().zip(&units).zip(receipts).enumerate()
        {
            let decoded_digest = Digest256::of_bytes(&unit.raw_instance);
            if receipt.ordinal != ordinal as u64
                || receipt.member_id != unit.member_id
                || receipt.relative_path != input.path
                || receipt.root_uri != unit.root_uri
                || receipt.raw_sha256 != decoded_digest
            {
                return Err(ItemRefusal::Source(
                    "schema batch unit identity mismatch".into(),
                ));
            }
            let valid = match receipt.verdict {
                BatchUnitVerdict::SchemaValid => true,
                BatchUnitVerdict::SchemaInvalid => false,
                BatchUnitVerdict::InputRejected => {
                    return Err(ItemRefusal::Source("schema batch input rejected".into()));
                }
            };
            staged.push(CutSchemaReceipt {
                path: input.path.clone(),
                contract: input.contract.clone(),
                source_revision: self.revision,
                source_raw_sha256: raw_digests[ordinal],
                decoded_instance_sha256: decoded_digest,
                execution: ExecutionIdentity {
                    worker_sha256: checkpoint.worker_sha256,
                    request_sha256: checkpoint.request_sha256,
                    schema_set_sha256: checkpoint.schema_set_sha256,
                    instance_sha256: decoded_digest,
                    profile: checkpoint.profile,
                },
                valid,
                batch: Some(CutBatchBinding {
                    checkpoint,
                    ordinal: receipt.ordinal,
                    unit_sha256: receipt.unit_sha256,
                }),
            });
            verdicts.push(valid);
        }
        self.prepared
            .preflight(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        self.receipt_bytes = next_bytes;
        self.receipt_count = self
            .receipt_count
            .checked_add(staged.len())
            .ok_or(ItemRefusal::Budget)?;
        self.receipts.extend(staged);
        Ok(verdicts)
    }

    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        if self.diagnostics_v2.is_some() {
            let result = self
                .produce_diagnostics_v2_terminal(path, raw, contract, deadline, cancelled)
                .and_then(|diagnostic| {
                    if diagnostic.is_invalid() {
                        self.retain_pending_diagnostic(diagnostic)?;
                        Ok(false)
                    } else if diagnostic.is_valid() {
                        Ok(true)
                    } else {
                        self.diagnostics_v2_cost_unknown = true;
                        self.prepared.poison(ExecutorFailure::Protocol);
                        Err(ItemRefusal::Unsupported(
                            "cut schema diagnostics status incomplete".into(),
                        ))
                    }
                });
            if result.is_err() {
                self.diagnostics_v2_cost_unknown = true;
                self.prepared.poison_shared_schema_worker_quota();
            }
            return result;
        }
        self.prepared
            .preflight(deadline, cancelled)
            .map_err(|reason| {
                operation_failure_with_context(reason, self.prepared.exchange_failure())
            })?;
        if self.receipt_count >= self.limits.max_receipts {
            return Err(ItemRefusal::BudgetCheck {
                check: "Cut scalar receipt count",
                used: (self.receipt_count as u64).checked_add(1),
                limit: Some(self.limits.max_receipts as u64),
            });
        }
        let receipt_bytes = path
            .len()
            .checked_add(contract.len())
            .and_then(|n| n.checked_add(192))
            .ok_or(ItemRefusal::Budget)?;
        let attempted_bytes = self.receipt_bytes.checked_add(receipt_bytes);
        let next_bytes = attempted_bytes
            .filter(|n| *n <= self.limits.max_receipt_bytes)
            .ok_or(ItemRefusal::BudgetCheck {
                check: "Cut scalar receipt bytes",
                used: attempted_bytes.map(|n| n as u64),
                limit: Some(self.limits.max_receipt_bytes as u64),
            })?;
        let (uri, worker_raw) = self.decoded_input(raw, contract)?;
        let decoded_digest = Digest256::of_bytes(&worker_raw);
        let mut budget = self.budget;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(ItemRefusal::Deadline)?;
        budget.execution_wall = budget.execution_wall.min(remaining);
        self.protocol_started = true;
        let result = self
            .prepared
            .evaluate(&uri, &worker_raw, budget, deadline, cancelled);
        if matches!(
            &result,
            ExecutorOutcome::SchemaValid(_) | ExecutorOutcome::SchemaInvalid(_)
        ) {
            self.prepared
                .preflight(deadline, cancelled)
                .map_err(|reason| {
                    operation_failure_with_context(reason, self.prepared.exchange_failure())
                })?;
        }
        let (execution, valid) = match result {
            ExecutorOutcome::SchemaValid(identity) => (identity, true),
            ExecutorOutcome::SchemaInvalid(identity) => (identity, false),
            ExecutorOutcome::Indeterminate {
                reason: ExecutorFailure::Timeout,
                ..
            } => return Err(ItemRefusal::Deadline),
            ExecutorOutcome::Indeterminate {
                reason: ExecutorFailure::Cancelled,
                ..
            } => return Err(ItemRefusal::Source("schema execution cancelled".into())),
            other => {
                return Err(ItemRefusal::Unsupported(format!(
                    "schema execution incomplete: {other:?}"
                )));
            }
        };
        if execution.worker_sha256 != self.worker.sha256
            || execution.instance_sha256 != decoded_digest
            || execution.schema_set_sha256 != self.schema_set_digest
            || execution.profile != self.profile
        {
            return Err(ItemRefusal::Source(
                "schema execution identity mismatch".into(),
            ));
        }
        self.receipt_bytes = next_bytes;
        self.receipt_count = self
            .receipt_count
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        self.receipts.push(CutSchemaReceipt {
            path: path.into(),
            contract: contract.into(),
            source_revision: self.revision,
            source_raw_sha256: Digest256::of_bytes(raw),
            decoded_instance_sha256: decoded_digest,
            execution,
            valid,
            batch: None,
        });
        Ok(valid)
    }
}

impl CutSchemaReceiptRange for CutWorkerSchemaExecutor {
    fn execution_binding(&self) -> CutExecutionBinding {
        CutWorkerSchemaExecutor::execution_binding(self)
    }

    fn source_revision(&self) -> SourceRevision {
        CutWorkerSchemaExecutor::source_revision(self)
    }

    fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        CutWorkerSchemaExecutor::contract_digest(self, contract)
    }

    fn receipt_count(&self) -> usize {
        CutWorkerSchemaExecutor::receipt_count(self)
    }

    fn receipt_range_supported(&self) -> bool {
        self.diagnostics_v2.is_none()
    }

    fn receipt_page_limits(&self) -> (usize, usize) {
        (self.limits.max_receipts, self.limits.max_receipt_bytes)
    }

    fn operation_budget(&self) -> BatchStreamBudget {
        CutWorkerSchemaExecutor::operation_budget(self)
    }

    fn receipt_limit_bytes(&self) -> usize {
        CutWorkerSchemaExecutor::receipt_limit_bytes(self)
    }

    fn release_child(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        CutWorkerSchemaExecutor::release_child(self, deadline, cancelled)
    }

    fn read_receipts_after(
        &mut self,
        after_ordinal: Option<u64>,
        max_rows: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutSchemaReceiptPage, ItemRefusal> {
        check(deadline, cancelled)?;
        if self.diagnostics_v2.is_some()
            || max_rows == 0
            || max_bytes == 0
            || max_rows > self.limits.max_receipts
            || max_bytes > self.limits.max_receipt_bytes
        {
            return Err(ItemRefusal::Budget);
        }
        let start = after_ordinal
            .map(|ordinal| ordinal.checked_add(1).ok_or(ItemRefusal::Budget))
            .transpose()?
            .map(|ordinal| usize::try_from(ordinal).map_err(|_| ItemRefusal::Budget))
            .transpose()?
            .unwrap_or(0);
        if start > self.receipts.len() {
            return Err(ItemRefusal::Budget);
        }
        let take = max_rows.min(self.receipts.len().saturating_sub(start));
        let mut page_state_bytes = std::mem::size_of::<Vec<(u64, CutSchemaReceipt)>>();
        let mut encoded_bytes = 0usize;
        for receipt in &self.receipts[start..start + take] {
            check(deadline, cancelled)?;
            page_state_bytes = page_state_bytes
                .checked_add(std::mem::size_of::<(u64, CutSchemaReceipt)>())
                .and_then(|bytes| bytes.checked_add(receipt.path.len()))
                .and_then(|bytes| bytes.checked_add(receipt.contract.len()))
                .filter(|bytes| *bytes <= max_bytes)
                .ok_or(ItemRefusal::Budget)?;
            encoded_bytes = encoded_bytes
                .checked_add(
                    receipt_spool::receipt_codec::encoded_receipt_len(receipt)
                        .map_err(|_| ItemRefusal::Budget)?,
                )
                .ok_or(ItemRefusal::Budget)?;
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(take)
            .map_err(|_| ItemRefusal::Budget)?;
        for (index, receipt) in self.receipts[start..start + take].iter().enumerate() {
            check(deadline, cancelled)?;
            let ordinal = u64::try_from(start + index).map_err(|_| ItemRefusal::Budget)?;
            rows.push((ordinal, receipt.clone()));
        }
        check(deadline, cancelled)?;
        let next_after_ordinal = (start + take < self.receipts.len())
            .then(|| u64::try_from(start + take - 1).ok())
            .flatten();
        Ok(CutSchemaReceiptPage::from_rows(
            rows,
            next_after_ordinal,
            encoded_bytes,
        ))
    }
}

/// Actual source+software adapter to the existing provenance owner's current
/// and named archived-input rules. No retained revision is substituted for a
/// currently missing builder/schema, and no old schema gains current authority.
pub struct CutProvenanceSource<'a> {
    pub cut: &'a CorpusCutReader,
    pub software: &'a SoftwareCaptureReader,
    /// Exact bounded producer components from the already selected capture.
    /// This selection proves byte membership only, never producer authority.
    pub components: Option<&'a SoftwareComponentSelectionV1>,
    pub schemas: &'a mut CutWorkerSchemaExecutor,
    pub cancelled: &'a AtomicBool,
}

impl ProvenanceSource for CutProvenanceSource<'_> {
    fn current(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(deadline, self.cancelled)?;
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("provenance source path".into()))?;
        if self
            .components
            .is_some_and(|components| components.capture() != self.software.selection())
        {
            return Err(ItemRefusal::Unsupported(
                "provenance component capture differs".into(),
            ));
        }
        // Authored source locators retain their corpus owner even if a
        // software capture happens to contain a file at the same locator.
        if !path.starts_with("ToS/") {
            if let Some(components) = self
                .components
                .filter(|selection| selection.member(&relative).is_some())
            {
                return self
                    .software
                    .read_selected_component(
                        components,
                        &relative,
                        max_bytes as u64,
                        deadline,
                        self.cancelled,
                    )
                    .map(Some)
                    .map_err(store_error);
            }
        }
        if path.starts_with("scripts/") {
            return self
                .software
                .read_current(&relative, max_bytes as u64, deadline, self.cancelled)
                .map_err(store_error);
        }
        if !path.starts_with("ToS/") {
            return Err(ItemRefusal::Unsupported("provenance source owner".into()));
        }
        if self.cut.current().member(&relative).is_none() {
            return Ok(None);
        }
        self.cut
            .read_member(
                self.cut.current().revision(),
                &relative,
                max_bytes as u64,
                deadline,
                self.cancelled,
            )
            .map(|member| Some(member.raw))
            .map_err(store_error)
    }

    fn recorded_input(
        &mut self,
        path: &str,
        digest: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        let Ok(expected) = Digest256::from_hex(digest) else {
            return Ok(None);
        };
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("provenance component path".into()))?;
        if self
            .components
            .is_some_and(|selection| selection.capture() != self.software.selection())
        {
            return Err(ItemRefusal::Unsupported(
                "provenance component capture differs".into(),
            ));
        }
        let selected_component = !path.starts_with("ToS/")
            && self
                .components
                .is_some_and(|selection| selection.member(&relative).is_some());
        let historical_script = path
            .strip_prefix("scripts/")
            .and_then(|name| name.strip_suffix(".py"))
            .is_some_and(|stem| owner_basename(stem, b'_'));
        // Absence from the verified capture is sufficient for a historical
        // script lookup, including captures which no longer select scripts/.
        // Present bytes and selected executable components keep strict reads.
        let current = if historical_script
            && !selected_component
            && self.software.member(&relative).is_none()
        {
            check(deadline, self.cancelled)?;
            None
        } else {
            self.current(path, max_bytes, deadline)?
        };
        if let Some(raw) = current.as_ref() {
            if Digest256::of_bytes(raw) == expected {
                return Ok(current);
            }
        }
        if selected_component {
            return Ok(None);
        }
        let archive = if let Some(stem) = path
            .strip_prefix("scripts/")
            .and_then(|name| name.strip_suffix(".py"))
        {
            if !owner_basename(stem, b'_') {
                return Ok(None);
            }
            format!("ToS/research-packets/retained-builder-inputs/{stem}/{digest}.py")
        } else if let Some(stem) = path
            .strip_prefix("ToS/contracts/")
            .and_then(|name| name.strip_suffix(".schema.json"))
        {
            // Schema retirement is not part of the historical-script route.
            if current.is_none() || !owner_basename(stem, b'-') {
                return Ok(None);
            }
            format!("ToS/contracts/history/{digest}.json")
        } else {
            return Ok(None);
        };
        let Some(raw) = self.current(&archive, max_bytes.min(1_048_576), deadline)? else {
            return Ok(None);
        };
        if Digest256::of_bytes(&raw) != expected {
            return Ok(None);
        }
        if path.starts_with("ToS/contracts/") {
            let value: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| {
                ItemRefusal::Unsupported("recorded schema JSON representation".into())
            })?;
            if !value.is_object() || value["$id"] != format!("https://tree-of-sophia.local/{path}")
            {
                return Ok(None);
            }
        }
        Ok(Some(raw))
    }

    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        contract_digest: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        check(deadline, self.cancelled)?;
        if self.schemas.source_revision() != self.cut.current().revision() {
            return Err(ItemRefusal::Source(
                "provenance schema executor belongs to another source cut".into(),
            ));
        }
        let expected = Digest256::from_hex(contract_digest)
            .map_err(|_| ItemRefusal::Unsupported("provenance contract digest".into()))?;
        if self.schemas.contract_digest(contract) != Some(expected) {
            return Err(ItemRefusal::Source(
                "provenance current contract differs from pinned worker resources".into(),
            ));
        }
        self.schemas
            .check_reusing_scalar(path, raw, contract, deadline, self.cancelled)
    }
}

#[derive(Debug)]
pub struct SourceCutProvenanceReport {
    pub source_revision: SourceRevision,
    pub software_selection: SoftwareCaptureSelectionV1,
    pub provenance_family: ProvenanceReport,
}

pub fn inspect_provenance_lab_from_cut(
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    limits: ItemLimits,
    cancelled: &AtomicBool,
    schemas: &mut CutWorkerSchemaExecutor,
) -> Result<SourceCutProvenanceReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let mut source = CutProvenanceSource {
        cut,
        software,
        components: None,
        schemas,
        cancelled,
    };
    let mut rules = ProvenanceRules::new(limits);
    rules.inspect_lab(&mut source)?;
    check(limits.deadline, cancelled)?;
    Ok(SourceCutProvenanceReport {
        source_revision: cut.current().revision(),
        software_selection: software.selection().clone(),
        provenance_family: rules.finish(),
    })
}

fn owner_basename(name: &str, punctuation: u8) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == punctuation)
}

/// Payload custody is outside source metadata membership. No ambient host
/// path is opened by this adapter. The custody owner hashes selected bytes.
pub trait CutPayloadReader {
    fn inspect(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<ItemPayload, ItemRefusal>;
}

/// Explicit source-owner metadata-only posture. The require-local profile
/// rejects these unavailable payloads in ItemRules instead of discovering
/// an unrelated checkout or local filesystem payload.
pub struct MetadataOnlyPayloads;
impl CutPayloadReader for MetadataOnlyPayloads {
    fn inspect(
        &mut self,
        _: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<ItemPayload, ItemRefusal> {
        check(deadline, cancelled)?;
        Ok(ItemPayload::Unavailable)
    }
}

#[derive(Debug)]
pub struct SourceCutItemReport {
    pub carrier_membership: SourceMembershipV1,
    pub item_family: ItemFamilyReport,
}

/// Run the complete current *carrier* through the record endpoint index and
/// every Item manifest and native Item record. The carrier's EOF verifies raw
/// ordered membership. This does not certify the carrier contains every
/// normative source profile or disposable derived catalog companion, and
/// never issues ValidationOutcome::MechanicallyValid.
pub fn inspect_items_from_cut(
    cut: &CorpusCutReader,
    limits: ItemLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_routes: &RecordFamily,
    schemas: &mut impl CutSchemaExecutor,
    payloads: &mut impl CutPayloadReader,
) -> Result<SourceCutItemReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let revision = cut.current().revision();
    let mut stream = cut.stream(revision).map_err(store_error)?;
    let mut kinds = BTreeMap::new();
    let mut manifests = Vec::new();
    let mut items = Vec::new();
    let mut index_bytes =
        2 * std::mem::size_of::<Vec<String>>() + std::mem::size_of::<BTreeMap<String, String>>();
    if index_bytes > limits.max_state_bytes {
        return Err(ItemRefusal::Budget);
    }
    let mut total_bytes = 0u64;
    while let Some(member) = stream
        .next_member(limits.deadline, cancelled)
        .map_err(store_error)?
    {
        check(limits.deadline, cancelled)?;
        total_bytes = total_bytes
            .checked_add(member.raw.len() as u64)
            .filter(|bytes| *bytes <= limits.max_total_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let path = member.path.as_str();
        if path.starts_with("ToS/source-witnesses/") && path.ends_with("/item.manifest.json") {
            reserve(
                &mut index_bytes,
                path.len() + std::mem::size_of::<String>(),
                limits.max_state_bytes,
            )?;
            manifests.push(member.path.clone());
        }
        // These are source-owned JSON record fields, never decoded manifest
        // keys or the weaker stable_ids index claims supplied by the carrier.
        // Native Item compatibility retains its ordinary decoded-field JSON
        // profile. Named strict declared-profile validation remains separate.
        if path.ends_with(".json") {
            if member.raw.len() > limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            if let Some(carrier) = record_routes
                .classify_current_member(path, &member.raw)
                .map_err(|error| ItemRefusal::Unsupported(format!("{error:?}")))?
            {
                let id = carrier.id.as_str();
                let kind = carrier.kind.as_str();
                reserve(
                    &mut index_bytes,
                    id.len()
                        + kind.len()
                        + std::mem::size_of::<(String, String)>()
                        + 3 * std::mem::size_of::<usize>(),
                    limits.max_state_bytes,
                )?;
                if kinds.insert(id.to_owned(), kind.to_owned()).is_some() {
                    return Err(ItemRefusal::Source("duplicate current record ID".into()));
                }
                if kind == "item" {
                    reserve(
                        &mut index_bytes,
                        path.len() + std::mem::size_of::<String>(),
                        limits.max_state_bytes,
                    )?;
                    items.push(member.path.clone());
                }
            }
        }
    }
    let membership = stream
        .coverage()
        .ok_or_else(|| ItemRefusal::Source("incomplete source carrier".into()))?;
    // Index state and rule state share one explicit logical allocation quota.
    let mut rule_limits = limits;
    rule_limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(index_bytes)
        .ok_or(ItemRefusal::Budget)?;
    let mut rules = ItemRules::new(rule_limits, require_local_payloads);
    let mut source = CutItemSource {
        cut,
        kinds: &kinds,
        cancelled,
        schemas,
        payloads,
    };
    for path in manifests {
        check(limits.deadline, cancelled)?;
        rules.inspect_manifest(&mut source, path.as_str())?;
    }
    for path in items {
        check(limits.deadline, cancelled)?;
        // The selected member reader materializes its raw Vec and path before
        // ItemRules can inspect it. Admit that live shape against the same
        // remaining family state before the read, not after allocation.
        let read_limit = rules.item_record_read_limit(path.as_str())?;
        let member = cut
            .read_member(
                revision,
                &path,
                read_limit as u64,
                limits.deadline,
                cancelled,
            )
            .map_err(store_error)?;
        rules.inspect_item_record(&mut source, path.as_str(), &member.raw)?;
    }
    check(limits.deadline, cancelled)?;
    Ok(SourceCutItemReport {
        carrier_membership: membership,
        item_family: rules.finish(),
    })
}

struct CutItemSource<'a, S, P> {
    cut: &'a CorpusCutReader,
    kinds: &'a BTreeMap<String, String>,
    cancelled: &'a AtomicBool,
    schemas: &'a mut S,
    payloads: &'a mut P,
}

impl<S: CutSchemaExecutor, P: CutPayloadReader> ItemSource for CutItemSource<'_, S, P> {
    fn cancellation_flag(&self) -> &AtomicBool {
        self.cancelled
    }

    fn metadata(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(deadline, self.cancelled)?;
        let path = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source path".into()))?;
        if self.cut.current().member(&path).is_none() {
            return Ok(None);
        }
        self.cut
            .read_member(
                self.cut.current().revision(),
                &path,
                max_bytes as u64,
                deadline,
                self.cancelled,
            )
            .map(|member| Some(member.raw))
            .map_err(store_error)
    }
    fn exists(&mut self, path: &str, deadline: Instant) -> Result<bool, ItemRefusal> {
        check(deadline, self.cancelled)?;
        let path = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source path".into()))?;
        Ok(self
            .cut
            .presence(self.cut.current().revision(), &path)
            .is_some())
    }
    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        check(deadline, self.cancelled)?;
        self.schemas
            .check_reusing_scalar(path, raw, contract, deadline, self.cancelled)
    }
    fn payload(&mut self, path: &str, deadline: Instant) -> Result<ItemPayload, ItemRefusal> {
        check(deadline, self.cancelled)?;
        self.payloads.inspect(path, deadline, self.cancelled)
    }
    fn record_kind(&mut self, id: &str, deadline: Instant) -> Result<Option<&str>, ItemRefusal> {
        check(deadline, self.cancelled)?;
        Ok(self.kinds.get(id).map(String::as_str))
    }
}

fn reserve(total: &mut usize, bytes: usize, max: usize) -> Result<(), ItemRefusal> {
    *total = total
        .checked_add(bytes)
        .filter(|n| *n <= max)
        .ok_or(ItemRefusal::Budget)?;
    Ok(())
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source("source cut cancelled".into()));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}

fn cut_diagnostics_limits_valid(
    max_receipts: usize,
    limits: CutSchemaDiagnosticsLimits,
    operation: BatchStreamBudget,
) -> bool {
    let execution_capacity = u64::try_from(max_receipts)
        .ok()
        .map(|maximum| {
            maximum
                .min(operation.max_chunks)
                .min(operation.max_total_units)
        })
        .and_then(|maximum| maximum.checked_mul(schema_diagnostics::MAX_ISSUES_PER_UNIT as u64));
    let wire_capacity = usize::try_from(operation.max_total_wire_bytes).unwrap_or(usize::MAX);
    operation.validate().is_ok()
        && limits.max_total_issues > 0
        && execution_capacity.is_some_and(|capacity| {
            u64::try_from(limits.max_total_issues).is_ok_and(|requested| requested <= capacity)
        })
        && limits.max_total_report_bytes > 0
        && limits.max_total_report_bytes <= wire_capacity
        && limits.max_total_state_bytes > 0
        && limits.max_total_state_bytes <= wire_capacity
}

fn cut_diagnostics_caps_digest(
    worker_limits: CutWorkerLimits,
    diagnostics: CutSchemaDiagnosticsLimits,
    operation: BatchStreamBudget,
    controller_state_cap_bytes: Option<usize>,
    legacy_raw_instance_limit: Option<usize>,
    legacy_selected_limits: Option<LegacySelectedDiagnosticsLimits>,
) -> Digest256 {
    fn u64_bytes(value: usize) -> [u8; 8] {
        u64::try_from(value).unwrap_or(u64::MAX).to_be_bytes()
    }
    fn duration_bytes(value: std::time::Duration) -> [u8; 16] {
        value.as_nanos().to_be_bytes()
    }

    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-cut-schema-diagnostics-v2-caps\0");
    hash.update(&schema_diagnostics::PROTOCOL_VERSION.to_be_bytes());
    hash.update(schema_diagnostics::Caps::CURRENT.digest().as_bytes());
    hash.update(&u64_bytes(worker_limits.max_receipts));
    hash.update(&u64_bytes(worker_limits.max_receipt_bytes));
    hash.update(&u64_bytes(diagnostics.max_total_issues));
    hash.update(&u64_bytes(diagnostics.max_total_report_bytes));
    hash.update(&u64_bytes(diagnostics.max_total_state_bytes));
    hash.update(&[u8::from(controller_state_cap_bytes.is_some())]);
    hash.update(&u64_bytes(controller_state_cap_bytes.unwrap_or(0)));
    hash.update(&[u8::from(legacy_raw_instance_limit.is_some())]);
    hash.update(&u64_bytes(legacy_raw_instance_limit.unwrap_or(0)));
    hash.update(&operation.max_chunks.to_be_bytes());
    hash.update(&operation.max_total_units.to_be_bytes());
    hash.update(&operation.max_total_raw_bytes.to_be_bytes());
    hash.update(&duration_bytes(operation.total_execution_wall));
    hash.update(&operation.operation_cpu_seconds.to_be_bytes());
    hash.update(&operation.operation_address_space_bytes.to_be_bytes());
    hash.update(&operation.max_total_wire_bytes.to_be_bytes());
    hash.update(&u64_bytes(operation.max_distinct_selectors));
    hash.update(&duration_bytes(operation.batch.total_execution_wall));
    hash.update(&duration_bytes(operation.batch.startup_wall));
    hash.update(&duration_bytes(operation.batch.per_unit_wall));
    hash.update(&duration_bytes(operation.batch.cleanup_grace));
    hash.update(&operation.batch.cpu_seconds.to_be_bytes());
    hash.update(&operation.batch.address_space_bytes.to_be_bytes());
    hash.update(&u64_bytes(operation.batch.max_units));
    hash.update(&u64_bytes(operation.batch.max_total_raw_bytes));
    if let Some(limits) = legacy_selected_limits {
        hash.update(b"legacy-selected-diagnostics-v2-limits-v1\0");
        hash.update(&u64_bytes(limits.max_instance_bytes));
        hash.update(&limits.max_visits.to_be_bytes());
        hash.update(&limits.parser_state_bytes.to_be_bytes());
        hash.update(&limits.conversion_state_bytes.to_be_bytes());
        hash.update(&LegacySelectedDiagnosticsLimits::RUNTIME_HEADROOM_BYTES.to_be_bytes());
    }
    hash.finalize()
}

fn finite_serde_json_controller_workspace_upper_bound(
    max_input_bytes: usize,
) -> Result<usize, ItemRefusal> {
    // Keep the 768-byte per-entry allowance conservative even if workspace
    // feature unification enables `preserve_order`; it is not a BTreeMap-only
    // representation claim. Bound inline arrays, array Vec growth, decoded
    // strings/numbers, and the deserializer recursion stack from the caller's
    // bounded input size. This is logical controller workspace; allocator
    // bookkeeping and process RSS remain governed by the existing host limit.
    const SERDE_JSON_BTREE_NODE_BYTES_PER_ENTRY: usize = 768;
    const SERDE_JSON_ARRAY_BYTES_PER_INPUT_BYTE: usize = 64;
    const SERDE_JSON_MAX_DEPTH: usize = 128;

    let nodes = max_input_bytes.checked_add(1).ok_or(ItemRefusal::Budget)?;
    let array_workspace = nodes
        .checked_mul(SERDE_JSON_ARRAY_BYTES_PER_INPUT_BYTE)
        .ok_or(ItemRefusal::Budget)?;
    let map_entries = max_input_bytes / 5;
    let map_workspace = map_entries
        .checked_mul(SERDE_JSON_BTREE_NODE_BYTES_PER_ENTRY)
        .ok_or(ItemRefusal::Budget)?;
    let lexical_workspace = max_input_bytes.checked_mul(4).ok_or(ItemRefusal::Budget)?;
    let recursion_workspace = SERDE_JSON_MAX_DEPTH
        .checked_add(1)
        .and_then(|depth| depth.checked_mul(std::mem::size_of::<serde_json::Value>()))
        .ok_or(ItemRefusal::Budget)?;
    array_workspace
        .checked_add(map_workspace)
        .ok_or(ItemRefusal::Budget)?
        .checked_add(lexical_workspace)
        .and_then(|bytes| bytes.checked_add(recursion_workspace))
        .ok_or(ItemRefusal::Budget)
}

fn diagnostic_issue_workspace_upper_bound() -> Result<usize, ItemRefusal> {
    let caps = schema_diagnostics::Caps::CURRENT;
    let issue_count = caps.max_issues_per_unit as usize;
    let path_segments = caps.max_path_segments as usize;
    let one_issue = std::mem::size_of::<schema_diagnostics::Issue>();
    let segment_slots = issue_count
        .checked_mul(2)
        .and_then(|count| count.checked_mul(path_segments))
        .and_then(|count| count.checked_mul(std::mem::size_of::<schema_diagnostics::PathSegment>()))
        .ok_or(ItemRefusal::Budget)?;
    let path_text = (caps.max_report_bytes_per_unit as usize)
        .checked_mul(2)
        .ok_or(ItemRefusal::Budget)?;
    std::mem::size_of::<schema_diagnostics::Report>()
        .checked_add(
            issue_count
                .checked_mul(one_issue)
                .ok_or(ItemRefusal::Budget)?,
        )
        .and_then(|bytes| bytes.checked_add(segment_slots))
        .and_then(|bytes| bytes.checked_add(path_text))
        .ok_or(ItemRefusal::Budget)
}

fn cut_diagnostic_state_bytes(
    path: &String,
    contract: &String,
    unit: &SchemaDiagnosticUnit,
    _checkpoint: &SchemaDiagnosticsCheckpoint,
) -> Option<usize> {
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

    let mut total = std::mem::size_of::<CutSchemaDiagnostic>()
        .checked_add(std::mem::size_of::<SchemaDiagnosticsCheckpoint>())?
        .checked_add(std::mem::size_of::<SchemaDiagnosticUnit>())?
        .checked_add(std::mem::size_of::<schema_diagnostics::Report>())?
        .checked_add(path.capacity())?
        .checked_add(contract.capacity())?
        .checked_add(unit.member_id.capacity())?
        .checked_add(unit.relative_path.capacity())?
        .checked_add(unit.root_uri.capacity())?
        .checked_add(
            unit.report
                .issues
                .capacity()
                .checked_mul(std::mem::size_of::<schema_diagnostics::Issue>())?,
        )?;
    for issue in &unit.report.issues {
        total = total
            .checked_add(issue.schema_keyword.capacity())?
            .checked_add(path_bytes(&issue.instance_path)?)?
            .checked_add(path_bytes(&issue.schema_path)?)?;
    }
    Some(total)
}

fn diagnostics_refusal(
    reason: ExecutorFailure,
    _exchange: Option<crate::executor::ExchangeFailureContext>,
) -> ItemRefusal {
    match reason {
        ExecutorFailure::Timeout => ItemRefusal::Deadline,
        ExecutorFailure::Cancelled => ItemRefusal::Source("schema diagnostics cancelled".into()),
        ExecutorFailure::InputBudget | ExecutorFailure::CpuLimit => ItemRefusal::Budget,
        _ => ItemRefusal::Unsupported("schema diagnostics worker failed".into()),
    }
}

fn diagnostic_status_refusal(report: &schema_diagnostics::Report) -> ItemRefusal {
    let status = match report.status {
        schema_diagnostics::Status::Valid => "valid",
        schema_diagnostics::Status::Invalid => "invalid",
        schema_diagnostics::Status::Truncated => "truncated",
        schema_diagnostics::Status::InputRejected => "input_rejected",
        schema_diagnostics::Status::Indeterminate => "indeterminate",
    };
    let failure = match report.failure {
        schema_diagnostics::Failure::None => "none",
        schema_diagnostics::Failure::InvalidJson => "invalid_json",
        schema_diagnostics::Failure::InputBudget => "input_budget",
        schema_diagnostics::Failure::ValidatorRuntime => "validator_runtime",
        schema_diagnostics::Failure::UnsupportedInputSemantics => "unsupported_input_semantics",
    };
    ItemRefusal::Unsupported(format!(
        concat!(
            "cut schema diagnostics status={} failure={} ",
            "returned_issues={} total_issues={} truncated={}"
        ),
        status,
        failure,
        report.issues.len(),
        report.total_issue_count,
        report.truncated,
    ))
}

fn store_error(error: tos_source_store::StoreError) -> ItemRefusal {
    use tos_source_store::StoreErrorCode;
    match error.code {
        StoreErrorCode::BudgetExceeded => ItemRefusal::Budget,
        StoreErrorCode::UnsupportedFormat | StoreErrorCode::UnsupportedPlatform => {
            ItemRefusal::Unsupported(error.to_string())
        }
        _ => ItemRefusal::Source(error.to_string()),
    }
}

fn operation_failure(reason: ExecutorFailure) -> ItemRefusal {
    operation_failure_with_context(reason, None)
}
fn operation_failure_with_context(
    reason: ExecutorFailure,
    exchange: Option<crate::executor::ExchangeFailureContext>,
) -> ItemRefusal {
    match reason {
        ExecutorFailure::Timeout => ItemRefusal::Deadline,
        ExecutorFailure::Cancelled => ItemRefusal::Source("schema operation cancelled".into()),
        other => ItemRefusal::Unsupported(format!(
            "schema operation refused: {other:?}; original exchange: {exchange:?}"
        )),
    }
}

#[cfg(test)]
mod diagnostic_status_tests {
    use super::*;

    #[test]
    fn status_refusal_contains_only_authenticated_protocol_summary_fields() {
        let worker_sha256 = Digest256::of_bytes(b"worker");
        let request_sha256 = Digest256::of_bytes(b"request");
        let unit_sha256 = Digest256::of_bytes(b"unit");
        let schema_set_sha256 = Digest256::of_bytes(b"schema");
        let caps = schema_diagnostics::Caps::CURRENT;
        let status = schema_diagnostics::Status::Indeterminate;
        let failure = schema_diagnostics::Failure::ValidatorRuntime;
        let total_issue_count = 1;
        let truncated = false;
        let issues = vec![schema_diagnostics::Issue {
            instance_path: vec![schema_diagnostics::PathSegment::Property(
                "private-instance-detail".into(),
            )],
            schema_keyword: "type".into(),
            reason: schema_diagnostics::Reason::Type,
            schema_path: Vec::new(),
            compatibility_text: None,
        }];
        let issues_sha256 = schema_diagnostics::issues_digest(&issues).unwrap();
        let report_sha256 = schema_diagnostics::report_digest(
            worker_sha256,
            request_sha256,
            unit_sha256,
            schema_set_sha256,
            caps,
            status,
            failure,
            total_issue_count,
            truncated,
            issues_sha256,
        );
        let report = schema_diagnostics::Report {
            protocol_version: schema_diagnostics::PROTOCOL_VERSION,
            worker_sha256,
            request_sha256,
            unit_sha256,
            schema_set_sha256,
            caps,
            status,
            failure,
            total_issue_count,
            truncated,
            issues_sha256,
            report_sha256,
            issues,
        };
        assert!(report.is_well_formed());

        let ItemRefusal::Unsupported(message) = diagnostic_status_refusal(&report) else {
            panic!("status refusals use the unsupported route");
        };
        assert_eq!(
            message,
            "cut schema diagnostics status=indeterminate failure=validator_runtime returned_issues=1 total_issues=1 truncated=false"
        );
        assert!(!message.contains("private-instance-detail"));
    }
}

/// Logical preparation upper bound for the exact cut-selected schema closure.
/// Reads only authentic member metadata. This is not allocator/RSS admission;
/// the caller retains its independent operation and physical host envelope.
pub fn cut_schema_preparation_state_upper_bound(
    cut: &CorpusCutReader,
    worker: &std::path::Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<usize, ItemRefusal> {
    let mut count = 0usize;
    let mut raw = 0usize;
    let mut paths = 0usize;
    let mut largest = 0usize;
    let mut serde_workspace = 0usize;
    for member in cut.current().members() {
        check(deadline, cancelled)?;
        let path = member.path.as_str();
        if !path.starts_with("ToS/contracts/") || !path.ends_with(".schema.json") {
            continue;
        }
        let bytes = usize::try_from(member.size_bytes).map_err(|_| ItemRefusal::Budget)?;
        if bytes > SchemaBackendProbe::MAX_RESOURCE_BYTES {
            return Err(ItemRefusal::Budget);
        }
        count = count
            .checked_add(1)
            .filter(|n| *n <= SchemaBackendProbe::MAX_RESOURCES)
            .ok_or(ItemRefusal::Budget)?;
        raw = raw
            .checked_add(bytes)
            .filter(|n| *n <= SchemaBackendProbe::MAX_TOTAL_BYTES)
            .ok_or(ItemRefusal::Budget)?;
        paths = paths.checked_add(path.len()).ok_or(ItemRefusal::Budget)?;
        largest = largest.max(bytes);
        // Probe::new retains all converted serde trees simultaneously.
        serde_workspace = serde_workspace
            .checked_add(finite_serde_json_controller_workspace_upper_bound(bytes)?)
            .ok_or(ItemRefusal::Budget)?;
    }
    if count == 0 {
        return Err(ItemRefusal::Budget);
    }
    // PublishedStrict's one live resource tree and its conversion overlap.
    // Vec capacities < 2*length; each input byte bounds a node and a key slot.
    let foundation_slot = std::mem::size_of::<tos_foundation::JsonValue>()
        .checked_add(std::mem::size_of::<(
            tos_foundation::JsonString,
            tos_foundation::JsonValue,
        )>())
        .and_then(|n| n.checked_mul(2))
        .ok_or(ItemRefusal::Budget)?;
    let foundation_workspace = largest
        .checked_add(1)
        .and_then(|n| n.checked_mul(foundation_slot))
        .and_then(|n| n.checked_add(largest.checked_mul(16)?))
        .and_then(|n| n.checked_add(65usize.checked_mul(foundation_slot)?))
        .ok_or(ItemRefusal::Budget)?;
    // Original+cloned raw buffers, extracted URI (bounded by raw), contract/
    // URI maps, encoded resource Vec growth and temporary framing buffers.
    let closure_buffers = raw
        .checked_mul(16)
        .and_then(|n| n.checked_add(paths.checked_mul(8)?))
        .and_then(|n| n.checked_add(count.checked_mul(4096)?))
        .and_then(|n| n.checked_add(65536))
        .ok_or(ItemRefusal::Budget)?;
    // Sealed worker backing plus transient verification image/buffer. The two
    // schema/worker phases do not overlap completely; their sum is conservative.
    let identity_workspace = worker
        .as_os_str()
        .as_encoded_bytes()
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_mul(16))
        .and_then(|n| {
            n.checked_add(16 * std::mem::size_of::<crate::executor::ExactWorkerIdentity>())
        })
        .ok_or(ItemRefusal::Budget)?;
    let image_workspace = usize::try_from(crate::executor::MAX_WORKER_IMAGE_BYTES)
        .map_err(|_| ItemRefusal::Budget)?
        .checked_mul(2)
        .and_then(|n| n.checked_add(65536))
        .and_then(|n| n.checked_add(identity_workspace))
        .ok_or(ItemRefusal::Budget)?;
    serde_workspace
        .checked_add(foundation_workspace)
        .and_then(|n| n.checked_add(closure_buffers))
        .and_then(|n| n.checked_add(image_workspace))
        .ok_or(ItemRefusal::Budget)
}
