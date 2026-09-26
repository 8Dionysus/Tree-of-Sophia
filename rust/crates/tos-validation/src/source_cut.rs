//! Actual source-carrier adapter for executable Item rules. Current member
//! paths and raw bytes come from an anchored CorpusCutReader; retained bases
//! stay in that reader and never become competing current record owners.
//! Carrier coverage is weaker than source-owner admission coverage.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

use crate::executor::{
    BoundedSchemaExecutor, ExactWorkerIdentity, ExecutionIdentity, ExecutorBudget, ExecutorFailure,
    ExecutorOutcome,
};
use crate::item_rules::{
    ItemFamilyReport, ItemLimits, ItemPayload, ItemRefusal, ItemRules, ItemSource,
};
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
}

/// Real disposable schema execution over resources read from the exact source
/// cut. This is a family execution receipt, not a trusted-source admission
/// ticket. Dedicated-worker descendant custody and host I/O interruption are
/// additional owner gates; the current executor guarantees parent liveness.
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
        self.contracts.get(contract).map(|(_, digest)| *digest)
    }

    pub fn receipts(&self) -> &[CutSchemaReceipt] {
        &self.receipts
    }
}

impl CutSchemaExecutor for CutWorkerSchemaExecutor {
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
        let uri = &self
            .contracts
            .get(contract)
            .ok_or_else(|| ItemRefusal::Unsupported(format!("missing source schema {contract}")))?
            .0;
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
            uri,
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
        });
        Ok(valid)
    }
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
fn store_error(error: impl std::fmt::Display) -> ItemRefusal {
    ItemRefusal::Source(error.to_string())
}
