//! Actual source-carrier adapter for executable Item rules. Current member
//! paths and raw bytes come from an anchored CorpusCutReader; retained bases
//! stay in that reader and never become competing current record owners.
//! Carrier coverage is weaker than source-owner admission coverage.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
    SoftwareComponentSelectionV1, SourceMembershipV1,
};

use crate::executor::{
    BatchBudget, BatchCoverageCheckpoint, BatchCoverageExpectation, BatchOutcome, BatchUnit,
    BatchUnitVerdict, BoundedSchemaExecutor, ExactWorkerIdentity, ExecutionIdentity,
    ExecutorBudget, ExecutorFailure, ExecutorOutcome,
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
    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal>;

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

pub struct CutWorkerSchemaExecutor {
    revision: SourceRevision,
    resources: Vec<SchemaResource>,
    contracts: BTreeMap<String, (String, Digest256)>,
    schema_set_digest: Digest256,
    profile: FormatProfile,
    worker: ExactWorkerIdentity,
    budget: ExecutorBudget,
    limits: CutWorkerLimits,
    receipt_bytes: usize,
    receipts: Vec<CutSchemaReceipt>,
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
        check(deadline, cancelled)?;
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
        let schema_set_digest = SchemaBackendProbe::new(resources.clone(), profile)
            .map_err(|error| {
                ItemRefusal::Unsupported(format!("schema resource closure: {error:?}"))
            })?
            .schema_set_digest();
        check(deadline, cancelled)?;
        Ok(Self {
            revision,
            resources,
            contracts,
            schema_set_digest,
            profile,
            worker,
            budget,
            limits,
            receipt_bytes: 0,
            receipts: Vec::new(),
        })
    }

    pub fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        let base = contract.split_once('#').map_or(contract, |(base, _)| base);
        self.contracts.get(base).map(|(_, digest)| *digest)
    }

    pub fn receipts(&self) -> &[CutSchemaReceipt] {
        &self.receipts
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
    fn check_batch(
        &mut self,
        checks: &[CutSchemaCheck],
        mut budget: BatchBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<bool>, ItemRefusal> {
        check(deadline, cancelled)?;
        if budget.max_units == 0
            || budget.max_units > BatchBudget::MAX_UNITS
            || budget.max_total_raw_bytes == 0
            || budget.max_total_raw_bytes > BatchBudget::MAX_RAW_BYTES
            || checks.is_empty()
            || checks.len() > budget.max_units
            || self
                .receipts
                .len()
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
            check(deadline, cancelled)?;
            next_bytes = input
                .path
                .len()
                .checked_add(input.contract.len())
                .and_then(|n| n.checked_add(std::mem::size_of::<CutSchemaReceipt>()))
                .and_then(|n| next_bytes.checked_add(n))
                .filter(|n| *n <= self.limits.max_receipt_bytes)
                .ok_or(ItemRefusal::Budget)?;
            let (base, fragment) = match input.contract.split_once('#') {
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
                None => (input.contract.as_str(), None),
            };
            let base_uri = &self
                .contracts
                .get(base)
                .ok_or_else(|| {
                    ItemRefusal::Unsupported(format!("missing source schema {}", input.contract))
                })?
                .0;
            let uri = fragment.map_or_else(|| base_uri.clone(), |f| format!("{base_uri}#{f}"));
            if input.raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES {
                return Err(ItemRefusal::Budget);
            }
            let decoded: serde_json::Value = serde_json::from_slice(&input.raw).map_err(|_| {
                ItemRefusal::Unsupported("unsupported native decoded JSON representation".into())
            })?;
            let worker_raw = serde_json::to_vec(&decoded).map_err(|_| {
                ItemRefusal::Unsupported("native decoded JSON serialization".into())
            })?;
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
        let outcome = BoundedSchemaExecutor::evaluate_batch_cancellable(
            &self.worker,
            &self.resources,
            self.profile,
            units.clone(),
            expected,
            budget,
            cancelled,
        );
        check(deadline, cancelled)?;
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
        check(deadline, cancelled)?;
        self.receipt_bytes = next_bytes;
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
        check(deadline, cancelled)?;
        if self.receipts.len() >= self.limits.max_receipts {
            return Err(ItemRefusal::Budget);
        }
        let receipt_bytes = path
            .len()
            .checked_add(contract.len())
            .and_then(|n| n.checked_add(192))
            .ok_or(ItemRefusal::Budget)?;
        let next_bytes = self
            .receipt_bytes
            .checked_add(receipt_bytes)
            .filter(|n| *n <= self.limits.max_receipt_bytes)
            .ok_or(ItemRefusal::Budget)?;
        if raw.len() > SchemaBackendProbe::MAX_INSTANCE_BYTES {
            return Err(ItemRefusal::Budget);
        }
        // A source owner may execute a named subschema of one already selected
        // exact resource. The fragment never supplies a new file/resource or
        // another engine. Preserve the full selector in the worker request and
        // receipt, while its fixity stays bound to the base resource bytes.
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
        let uri = fragment.map_or_else(
            || base_uri.clone(),
            |fragment| format!("{base_uri}#{fragment}"),
        );
        let decoded: serde_json::Value = serde_json::from_slice(raw).map_err(|_| {
            ItemRefusal::Unsupported("unsupported native decoded JSON representation".into())
        })?;
        let worker_raw = serde_json::to_vec(&decoded)
            .map_err(|_| ItemRefusal::Unsupported("native decoded JSON serialization".into()))?;
        let decoded_digest = Digest256::of_bytes(&worker_raw);
        let mut budget = self.budget;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(ItemRefusal::Deadline)?;
        budget.execution_wall = budget.execution_wall.min(remaining);
        let result = BoundedSchemaExecutor::evaluate_cancellable(
            &self.worker,
            &self.resources,
            self.profile,
            &uri,
            &worker_raw,
            budget,
            cancelled,
        );
        check(deadline, cancelled)?;
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
        // The existing owner requires the current path to remain a file before
        // an exact original builder or schema capture may be consulted.
        let Some(current) = self.current(path, max_bytes, deadline)? else {
            return Ok(None);
        };
        if Digest256::of_bytes(&current) == expected {
            return Ok(Some(current));
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("provenance component path".into()))?;
        if !path.starts_with("ToS/")
            && self
                .components
                .is_some_and(|selection| selection.member(&relative).is_some())
        {
            // A native producer-run component is current-only. Its exact
            // selected digest cannot be replaced by an archived input.
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
            if !owner_basename(stem, b'-') {
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
            .check(path, raw, contract, deadline, self.cancelled)
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
    let mut index_bytes = 0usize;
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
            reserve(&mut index_bytes, path.len(), limits.max_state_bytes)?;
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
                    id.len() + kind.len(),
                    limits.max_state_bytes,
                )?;
                if kinds.insert(id.to_owned(), kind.to_owned()).is_some() {
                    return Err(ItemRefusal::Source("duplicate current record ID".into()));
                }
                if kind == "item" {
                    reserve(&mut index_bytes, path.len(), limits.max_state_bytes)?;
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
        let member = cut
            .read_member(
                revision,
                &path,
                limits.max_member_bytes as u64,
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
            .check(path, raw, contract, deadline, self.cancelled)
    }
    fn payload(&mut self, path: &str, deadline: Instant) -> Result<ItemPayload, ItemRefusal> {
        check(deadline, self.cancelled)?;
        self.payloads.inspect(path, deadline, self.cancelled)
    }
    fn record_kind(&mut self, id: &str, deadline: Instant) -> Result<Option<String>, ItemRefusal> {
        check(deadline, self.cancelled)?;
        Ok(self.kinds.get(id).cloned())
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
