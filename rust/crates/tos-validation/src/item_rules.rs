//! Executable Item companion and payload mechanics from the source foundation
//! owner. This family has no admission/seal constructor. Complete source
//! enumeration, retained history and current rights remain separate owners.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde_json::Value;
use tos_foundation::Digest256;

const MANIFEST: &str = "ToS/contracts/source-item-manifest.schema.json";
const INVENTORY: &str = "ToS/contracts/source-resource-inventory.schema.json";
const RIGHTS: &str = "ToS/contracts/rights-record.schema.json";
const EVENT: &str = "ToS/contracts/provenance-event.schema.json";

#[derive(Debug, Clone, Copy)]
pub struct ItemLimits {
    pub max_member_bytes: usize,
    pub max_total_bytes: u64,
    pub max_state_bytes: usize,
    pub max_issues: usize,
    pub deadline: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemRefusal {
    Budget,
    /// A named rejected budget guard. None preserves an owner that did not
    /// supply a counter/limit, or arithmetic overflow, without inventing data.
    /// Uninstrumented guards retain Budget.
    BudgetCheck {
        check: &'static str,
        used: Option<u64>,
        limit: Option<u64>,
    },
    Executor(Box<ItemExecutorRefusal>),
    Deadline,
    Source(String),
    Unsupported(String),
}

impl ItemRefusal {
    /// Preserve the established coarse category at APIs that do not expose
    /// executor evidence. Foundation's typed receiver does not use this view.
    pub fn compatibility_category(self) -> Self {
        match self {
            Self::Executor(evidence) => match evidence.reason {
                crate::executor::ExecutorFailure::Timeout => Self::Deadline,
                crate::executor::ExecutorFailure::Cancelled => {
                    Self::Source("schema execution cancelled".into())
                }
                crate::executor::ExecutorFailure::InputBudget
                | crate::executor::ExecutorFailure::CpuLimit => Self::Budget,
                _ => Self::Unsupported(evidence.summary()),
            },
            other => other,
        }
    }
}

/// Bounded mechanical failure evidence; contains no instance, parser text or path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemExecutorRefusal {
    pub stage: &'static str,
    pub reason: crate::executor::ExecutorFailure,
    pub exchange: Option<crate::executor::ExchangeFailureContext>,
    /// Exact committed prefix before the failed attempt, never attempted work.
    pub quota: Option<crate::executor::SharedSchemaWorkerQuotaUsage>,
    /// Observed partial batch coverage. These units were not admitted as receipts.
    /// None means the refusal did not carry a batch checkpoint.
    pub batch_completed_count: Option<u64>,
}
impl ItemExecutorRefusal {
    pub fn summary(&self) -> String {
        // Stage and boundary are authored static guard names. Fingerprint them
        // so even future owner labels cannot disclose source paths.
        let stage = Digest256::of_bytes(self.stage.as_bytes()).to_hex();
        let boundary = self
            .exchange
            .map(|context| Digest256::of_bytes(context.boundary.as_bytes()).to_hex());
        format!(
            "executor stage={stage} reason={:?} boundary={boundary:?} exchange_reason={:?} natural_termination={:?} committed_quota_prefix={:?} batch_completed_count={:?}",
            self.reason,
            self.exchange.map(|context| context.failure),
            self.exchange
                .and_then(|context| context.natural_termination),
            self.quota,
            self.batch_completed_count
        )
    }
}

impl std::fmt::Display for ItemExecutorRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.summary())
    }
}
impl std::error::Error for ItemExecutorRefusal {}

pub(crate) fn executor_refusal(
    stage: &'static str,
    reason: crate::executor::ExecutorFailure,
    exchange: Option<crate::executor::ExchangeFailureContext>,
    quota: Option<&crate::executor::SharedSchemaWorkerQuota>,
) -> ItemRefusal {
    let observed = quota.and_then(|quota| quota.refusal_evidence());
    ItemRefusal::Executor(Box::new(ItemExecutorRefusal {
        stage,
        reason,
        exchange: exchange.or_else(|| observed.and_then(|(context, _)| context)),
        quota: observed.map(|(_, usage)| usage),
        batch_completed_count: None,
    }))
}

/// Preserve the source location of a guard that has no observed counter.
/// Only its fingerprint crosses the public refusal boundary.
#[macro_export]
macro_rules! item_budget_origin {
    () => {
        $crate::item_rules::ItemRefusal::BudgetCheck {
            check: concat!(module_path!(), ":", line!()),
            used: None,
            limit: None,
        }
    };
}

/// Custody performs streaming hashing against the pinned selected bytes.
/// Unavailable bytes preserve the metadata-only route; they do not count as
/// verified local fixity. A symlink/non-file is unavailable, never a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemPayload {
    Unavailable,
    File {
        byte_size: u64,
        sha256: String,
        excluded_from_source: bool,
    },
}

/// Owner adapter over an immutable current member namespace. `exists` must
/// preserve file-versus-directory semantics. Metadata reads and payload
/// hashing must enforce the supplied deadline themselves: this trait cannot
/// interrupt blocked I/O. Schema execution must use the named exact contract
/// and return Unsupported rather than silently selecting another profile.
pub trait ItemSource {
    /// Borrow the operation's original cancellation flag. Decoders poll this
    /// exact signal; adapters must not synthesize or reset a local token.
    fn cancellation_flag(&self) -> &AtomicBool;
    fn record_selection(
        &self,
    ) -> Option<Arc<crate::source_record_selection::SourceRecordSelection>> {
        None
    }
    fn metadata(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal>;
    fn exists(&mut self, path: &str, deadline: Instant) -> Result<bool, ItemRefusal>;
    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal>;
    fn payload(&mut self, path: &str, deadline: Instant) -> Result<ItemPayload, ItemRefusal>;
    fn record_kind(&mut self, id: &str, deadline: Instant) -> Result<Option<&str>, ItemRefusal>;
    /// Optional cooperative cancellation checkpoint for bounded owner loops.
    fn check_cancelled(&self) -> Result<(), ItemRefusal> {
        Ok(())
    }

    /// Optional compatibility seam for inventory bytes that the legacy
    /// Python JSON loader accepted but serde_json cannot represent. It is
    /// reached only after the ordinary finite decoder rejects an inventory;
    /// implementations must return a typed FND tree for an actual nonfinite
    /// value, never a substituted serde value.
    fn legacy_observed_inventory(
        &mut self,
        _path: &str,
        _raw: &[u8],
        _max_member_bytes: usize,
        _available_state_bytes: usize,
        _deadline: Instant,
    ) -> Result<Option<tos_foundation::JsonValue>, ItemRefusal> {
        Ok(None)
    }
}

enum ItemProvenanceRows<'s, 'a> {
    Full(std::iter::Enumerate<std::str::Lines<'a>>),
    Selected(crate::source_record_selection::SelectedRowCursor<'s, 'a>),
}
impl<'s, 'a> ItemProvenanceRows<'s, 'a> {
    fn next_checked(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Option<Result<(u64, &'a str), ItemRefusal>> {
        match self {
            Self::Full(rows) => rows.next().map(|(i, row)| Ok((i as u64 + 1, row))),
            Self::Selected(rows) => rows.next_checked(deadline, cancelled).map(|row| {
                let (line, raw, slot) = row?;
                if slot.kind != "provenance_event" {
                    return Err(ItemRefusal::Source(
                        "Item provenance selected slot kind".into(),
                    ));
                }
                let text = std::str::from_utf8(raw).map_err(|_| {
                    ItemRefusal::Source("Item provenance selected slot UTF8".into())
                })?;
                Ok((line, text))
            }),
        }
    }
}

enum ItemInventoryValue {
    Finite(Value),
    LegacyObserved(tos_foundation::JsonValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemIssue {
    pub path: String,
    pub code: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFamilyReport {
    pub issues: Vec<ItemIssue>,
    pub manifest_item_ids: BTreeSet<String>,
    pub metadata_bytes: u64,
    pub unavailable_payloads: u64,
    /// Logical retained-state counter charged by this ItemRules execution.
    /// This is an accounting upper bound, not a process-memory or RSS value.
    pub accounted_state_upper_bound_bytes: usize,
    /// Prefix-scan steps used by finite and observed inventory duplicate
    /// checks; globally capped by the caller's Item state limit.
    pub inventory_set_scan_steps: usize,
    /// Family-local execution is not proof of complete source membership.
    pub source_admission_complete: bool,
}

pub struct ItemRules {
    limits: ItemLimits,
    require_local_payloads: bool,
    issues: Vec<ItemIssue>,
    state_bytes: usize,
    live_bytes: usize,
    metadata_bytes: u64,
    unavailable_payloads: u64,
    manifest_item_ids: BTreeSet<String>,
    event_ids: BTreeSet<String>,
    file_descriptors: BTreeMap<String, [Value; 3]>,
    inventory_set_scan_steps: usize,
}

impl ItemRules {
    pub fn new(limits: ItemLimits, require_local_payloads: bool) -> Self {
        Self {
            limits,
            require_local_payloads,
            issues: Vec::new(),
            state_bytes: 0,
            live_bytes: 0,
            metadata_bytes: 0,
            unavailable_payloads: 0,
            manifest_item_ids: BTreeSet::new(),
            event_ids: BTreeSet::new(),
            file_descriptors: BTreeMap::new(),
            inventory_set_scan_steps: 0,
        }
    }

    fn check(&self) -> Result<(), ItemRefusal> {
        if Instant::now() >= self.limits.deadline {
            Err(ItemRefusal::Deadline)
        } else {
            Ok(())
        }
    }

    fn charge_inventory_set_scan_step(
        &mut self,
        source: &impl ItemSource,
    ) -> Result<(), ItemRefusal> {
        self.check()?;
        source.check_cancelled()?;
        let used = self
            .inventory_set_scan_steps
            .checked_add(1)
            .ok_or(crate::item_budget_origin!())?;
        if used > self.limits.max_state_bytes {
            return Err(ItemRefusal::BudgetCheck {
                check: "Item inventory set membership scan steps",
                used: Some(used as u64),
                limit: Some(self.limits.max_state_bytes as u64),
            });
        }
        self.inventory_set_scan_steps = used;
        Ok(())
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        self.check()?;
        let retained = self
            .state_bytes
            .checked_add(bytes)
            .ok_or(crate::item_budget_origin!())?;
        if retained
            .checked_add(self.live_bytes)
            .is_none_or(|total| total > self.limits.max_state_bytes)
        {
            return Err(crate::item_budget_origin!());
        }
        self.state_bytes = retained;
        Ok(())
    }

    fn available(&self) -> Result<usize, ItemRefusal> {
        self.limits
            .max_state_bytes
            .checked_sub(self.state_bytes)
            .and_then(|bytes| bytes.checked_sub(self.live_bytes))
            .ok_or(crate::item_budget_origin!())
    }

    fn admit_live(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        self.check()?;
        if bytes > self.available()? {
            return Err(crate::item_budget_origin!());
        }
        self.live_bytes = self
            .live_bytes
            .checked_add(bytes)
            .ok_or(crate::item_budget_origin!())?;
        Ok(())
    }

    fn release_raw(&mut self, raw: &[u8]) {
        self.live_bytes -= std::mem::size_of::<Vec<u8>>() + raw.len();
    }

    fn issue(&mut self, path: &str, code: &'static str) -> Result<(), ItemRefusal> {
        if self.issues.len() >= self.limits.max_issues {
            return Err(crate::item_budget_origin!());
        }
        self.reserve(path.len() + code.len() + std::mem::size_of::<ItemIssue>())?;
        self.issues.push(ItemIssue {
            path: path.into(),
            code,
        });
        Ok(())
    }

    fn raw(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        self.check()?;
        safe_path(path)?;
        let available = self
            .available()?
            .checked_sub(std::mem::size_of::<Vec<u8>>())
            .ok_or(crate::item_budget_origin!())?;
        let total_remaining = self
            .limits
            .max_total_bytes
            .checked_sub(self.metadata_bytes)
            .ok_or(crate::item_budget_origin!())?
            .min(usize::MAX as u64) as usize;
        // The adapter must enforce this cap while reading; the exact returned
        // length is admitted before any decoded representation is built.
        let raw = source.metadata(
            path,
            self.limits
                .max_member_bytes
                .min(available)
                .min(total_remaining),
            self.limits.deadline,
        )?;
        self.check()?;
        if let Some(raw) = &raw {
            if raw.len() > self.limits.max_member_bytes {
                return Err(crate::item_budget_origin!());
            }
            self.admit_live(std::mem::size_of::<Vec<u8>>() + raw.len())?;
            self.metadata_bytes = self
                .metadata_bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= self.limits.max_total_bytes)
                .ok_or(crate::item_budget_origin!())?;
        } else {
            self.issue(path, "missing-companion")?;
        }
        Ok(raw)
    }

    // Native Item Python loaders use ordinary json.loads. Keep that legacy
    // decoded-field profile here; do not silently substitute the strict
    // declared-record parser. The bounded legacy codec keeps syntax errors
    // separate from unsupported representations. Exact raw bytes still bind
    // schema and fixity after successful decode.
    fn object(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        contract: &str,
    ) -> Result<Option<(Value, Vec<u8>)>, ItemRefusal> {
        let Some(raw) = self.raw(source, path)? else {
            return Ok(None);
        };
        let available = self.available()?;
        let decoded = crate::validation_codec::bounded_legacy_item_decoded_state(
            &raw,
            item_json_limits(self.limits.max_member_bytes, available)?,
            available,
            self.limits.deadline,
            source.cancellation_flag(),
        );
        let (value, decoded_bytes) = match decoded {
            Ok(result) => result,
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                self.release_raw(&raw);
                drop(raw);
                self.issue(path, "invalid-json")?;
                return Ok(None);
            }
            Err(error @ ItemRefusal::Unsupported(_)) => return Err(error),
            Err(error) => {
                return Err(item_codec_refusal(
                    error,
                    raw.len(),
                    available,
                    self.limits.max_member_bytes,
                ));
            }
        };
        self.admit_live(decoded_bytes)?;
        if !value.is_object() {
            self.live_bytes -= decoded_bytes;
            self.release_raw(&raw);
            drop(value);
            drop(raw);
            self.issue(path, "object-required")?;
            return Ok(None);
        }
        if !source.schema(path, &raw, contract, self.limits.deadline)? {
            self.issue(path, "schema")?;
        }
        self.check()?;
        self.source_refs(source, path, &value)?;
        Ok(Some((value, raw)))
    }

    fn inventory_object(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
    ) -> Result<Option<(ItemInventoryValue, Vec<u8>)>, ItemRefusal> {
        let Some(raw) = self.raw(source, path)? else {
            return Ok(None);
        };
        let available = self.available()?;
        let decoded = crate::validation_codec::bounded_legacy_item_decoded_state(
            &raw,
            item_json_limits(self.limits.max_member_bytes, available)?,
            available,
            self.limits.deadline,
            source.cancellation_flag(),
        );
        let (value, decoded_bytes) = match decoded {
            Ok(result) => result,
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                let observed = source.legacy_observed_inventory(
                    path,
                    &raw,
                    self.limits.max_member_bytes,
                    available,
                    self.limits.deadline,
                )?;
                let Some(observed) = observed else {
                    self.release_raw(&raw);
                    drop(raw);
                    self.issue(path, "invalid-json")?;
                    return Ok(None);
                };
                let decoded_bytes = observed_json_retained_bytes(&observed)?;
                self.admit_live(decoded_bytes)?;
                if !source.schema(path, &raw, INVENTORY, self.limits.deadline)? {
                    self.issue(path, "schema")?;
                }
                if observed.as_object().is_none() {
                    self.live_bytes -= decoded_bytes;
                    self.release_raw(&raw);
                    drop(observed);
                    drop(raw);
                    self.issue(path, "object-required")?;
                    return Ok(None);
                }
                self.check()?;
                self.source_refs_observed(source, path, &observed)?;
                return Ok(Some((ItemInventoryValue::LegacyObserved(observed), raw)));
            }
            Err(error @ ItemRefusal::Unsupported(_)) => return Err(error),
            Err(error) => {
                return Err(item_codec_refusal(
                    error,
                    raw.len(),
                    available,
                    self.limits.max_member_bytes,
                ));
            }
        };
        self.admit_live(decoded_bytes)?;
        if !value.is_object() {
            self.live_bytes -= decoded_bytes;
            self.release_raw(&raw);
            drop(value);
            drop(raw);
            self.issue(path, "object-required")?;
            return Ok(None);
        }
        if !source.schema(path, &raw, INVENTORY, self.limits.deadline)? {
            self.issue(path, "schema")?;
        }
        self.check()?;
        self.source_refs(source, path, &value)?;
        Ok(Some((ItemInventoryValue::Finite(value), raw)))
    }

    fn inspect_finite_inventory(
        &mut self,
        source: &impl ItemSource,
        path: &str,
        manifest: &Value,
        inventory: &Value,
    ) -> Result<(), ItemRefusal> {
        if inventory["item_id"] != manifest["item_id"] {
            self.issue(path, "inventory-item-id")?;
        }
        let mut expected = array(&manifest["payload_files"]).filter(|v| v.is_object());
        let mut actual = array(&inventory["files"]).filter(|v| v.is_object());
        let same_files = loop {
            self.check()?;
            match (actual.next(), expected.next()) {
                (None, None) => break true,
                (Some(a), Some(e))
                    if a["file_id"] == e["file_id"]
                        && a["file_sha256"] == e["sha256"]
                        && a["media_type"] == e["media_type"] => {}
                _ => break false,
            }
        };
        if !same_files {
            self.issue(path, "inventory-file-identity")?;
        }
        for entry in array(&inventory["files"]).filter(|v| v.is_object()) {
            self.check()?;
            let resource_values = &entry["resources"];

            // Python's set construction fails for list/dict IDs. Detect those
            // before any membership scan so a partial duplicate result can
            // never appear as a complete family report.
            for resource in array(resource_values) {
                self.check()?;
                source.check_cancelled()?;
                if !resource.is_object() {
                    continue;
                }
                let id = resource.get("resource_id").unwrap_or(&Value::Null);
                if matches!(id, Value::Array(_) | Value::Object(_)) {
                    return Err(ItemRefusal::Unsupported(
                        "legacy inventory set membership has an unhashable resource_id".into(),
                    ));
                }
            }

            for (resource_index, resource) in array(resource_values).enumerate() {
                if !resource.is_object() {
                    continue;
                }
                self.check()?;
                source.check_cancelled()?;
                let id = resource.get("resource_id").unwrap_or(&Value::Null);
                let mut duplicate = false;
                for previous in array(resource_values).take(resource_index) {
                    self.charge_inventory_set_scan_step(source)?;
                    if !previous.is_object() {
                        continue;
                    }
                    let previous_id = previous.get("resource_id").unwrap_or(&Value::Null);
                    if native_json_set_members_equal(previous_id, id) {
                        duplicate = true;
                        break;
                    }
                }
                if duplicate {
                    self.issue(path, "duplicate-resource-id")?;
                }
            }
            if entry["summary"].is_object()
                && entry["summary"]["resource_count"].as_u64()
                    != Some(array(resource_values).count() as u64)
            {
                self.issue(path, "resource-count")?;
            }
        }
        Ok(())
    }

    fn inspect_observed_inventory(
        &mut self,
        source: &impl ItemSource,
        path: &str,
        manifest: &Value,
        inventory: &tos_foundation::JsonValue,
    ) -> Result<(), ItemRefusal> {
        if !legacy_json_native_equal(
            inventory.object_get("item_id").unwrap_or(&LEGACY_JSON_NULL),
            manifest.get("item_id").unwrap_or(&Value::Null),
        ) {
            self.issue(path, "inventory-item-id")?;
        }
        let expected = manifest
            .get("payload_files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|value| value.is_object());
        let actual = inventory
            .object_get("files")
            .and_then(tos_foundation::JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter(|value| value.as_object().is_some());
        let mut expected = expected;
        let mut actual = actual;
        let same_files = loop {
            self.check()?;
            match (actual.next(), expected.next()) {
                (None, None) => break true,
                (Some(actual), Some(expected))
                    if legacy_json_native_equal(
                        actual.object_get("file_id").unwrap_or(&LEGACY_JSON_NULL),
                        expected.get("file_id").unwrap_or(&Value::Null),
                    ) && legacy_json_native_equal(
                        actual
                            .object_get("file_sha256")
                            .unwrap_or(&LEGACY_JSON_NULL),
                        expected.get("sha256").unwrap_or(&Value::Null),
                    ) && legacy_json_native_equal(
                        actual.object_get("media_type").unwrap_or(&LEGACY_JSON_NULL),
                        expected.get("media_type").unwrap_or(&Value::Null),
                    ) => {}
                _ => break false,
            }
        };
        if !same_files {
            self.issue(path, "inventory-file-identity")?;
        }
        let files = inventory
            .object_get("files")
            .and_then(tos_foundation::JsonValue::as_array)
            .unwrap_or(&[]);
        for entry in files.iter().filter(|value| value.as_object().is_some()) {
            self.check()?;
            let resources = entry
                .object_get("resources")
                .and_then(tos_foundation::JsonValue::as_array)
                .unwrap_or(&[]);
            for resource in resources {
                self.check()?;
                source.check_cancelled()?;
                if resource.as_object().is_none() {
                    continue;
                }
                let id = resource
                    .object_get("resource_id")
                    .unwrap_or(&LEGACY_JSON_NULL);
                if matches!(
                    id,
                    tos_foundation::JsonValue::Array(_) | tos_foundation::JsonValue::Object(_)
                ) {
                    return Err(ItemRefusal::Unsupported(
                        "legacy inventory set membership has an unhashable resource_id".into(),
                    ));
                }
            }
            let mut duplicate = false;
            for (resource_index, resource) in resources.iter().enumerate() {
                if resource.as_object().is_none() {
                    continue;
                }
                self.check()?;
                source.check_cancelled()?;
                let id = resource
                    .object_get("resource_id")
                    .unwrap_or(&LEGACY_JSON_NULL);
                for previous in &resources[..resource_index] {
                    self.charge_inventory_set_scan_step(source)?;
                    if previous.as_object().is_none() {
                        continue;
                    }
                    let previous_id = previous
                        .object_get("resource_id")
                        .unwrap_or(&LEGACY_JSON_NULL);
                    if legacy_json_set_members_equal(previous_id, id) {
                        duplicate = true;
                        break;
                    }
                }
                if duplicate {
                    break;
                }
            }
            if duplicate {
                self.issue(path, "duplicate-resource-id")?;
            }
            if entry
                .object_get("summary")
                .and_then(tos_foundation::JsonValue::as_object)
                .is_some()
            {
                let expected_count = Value::from(resources.len() as u64);
                if !legacy_json_native_equal(
                    entry
                        .object_get("summary")
                        .and_then(|summary| summary.object_get("resource_count"))
                        .unwrap_or(&LEGACY_JSON_NULL),
                    &expected_count,
                ) {
                    self.issue(path, "resource-count")?;
                }
            }
        }
        Ok(())
    }

    fn source_refs(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        value: &Value,
    ) -> Result<(), ItemRefusal> {
        for field in ["source_refs", "source_record_refs", "receipt_refs"] {
            for target in array(&value[field]) {
                self.source_ref(source, path, target)?;
            }
        }
        for field in [
            "rights_ref",
            "provenance_ref",
            "forensic_report_ref",
            "resource_inventory_ref",
            "item_manifest_ref",
            "generated_from_manifest_ref",
        ] {
            if let Some(target) = value.get(field) {
                self.source_ref(source, path, target)?;
            }
        }
        Ok(())
    }

    fn source_refs_observed(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        value: &tos_foundation::JsonValue,
    ) -> Result<(), ItemRefusal> {
        for field in ["source_refs", "source_record_refs", "receipt_refs"] {
            if let Some(tos_foundation::JsonValue::Array(values)) = value.object_get(field) {
                for target in values {
                    self.source_ref_observed(source, path, target)?;
                }
            }
        }
        for field in [
            "rights_ref",
            "provenance_ref",
            "forensic_report_ref",
            "resource_inventory_ref",
            "item_manifest_ref",
            "generated_from_manifest_ref",
        ] {
            if let Some(target) = value.object_get(field) {
                self.source_ref_observed(source, path, target)?;
            }
        }
        Ok(())
    }

    fn source_ref_observed(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        target: &tos_foundation::JsonValue,
    ) -> Result<(), ItemRefusal> {
        let Some(target) = target.as_str() else {
            return self.issue(path, "unresolved-source-ref");
        };
        if target.starts_with("ToS/") {
            safe_path(target)?;
            if !source.exists(target, self.limits.deadline)? {
                self.issue(path, "unresolved-source-ref")?;
            }
            self.check()?;
        }
        Ok(())
    }

    fn source_ref(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        target: &Value,
    ) -> Result<(), ItemRefusal> {
        let Some(target) = target.as_str() else {
            return self.issue(path, "unresolved-source-ref");
        };
        if target.starts_with("ToS/") {
            safe_path(target)?;
            if !source.exists(target, self.limits.deadline)? {
                self.issue(path, "unresolved-source-ref")?;
            }
            self.check()?;
        }
        Ok(())
    }

    fn require_kind(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        id: &Value,
        kind: &str,
    ) -> Result<(), ItemRefusal> {
        if let Some(id) = id.as_str() {
            if source.record_kind(id, self.limits.deadline)? != Some(kind) {
                self.issue(path, "missing-or-wrong-record-kind")?;
            }
            self.check()?;
        }
        Ok(())
    }

    /// Invoke for every current item.manifest.json selected by the source
    /// owner, in the same immutable cut as companions and record endpoints.
    pub fn inspect_manifest(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
    ) -> Result<(), ItemRefusal> {
        let baseline = self.live_bytes;
        let result = self.inspect_manifest_inner(source, path);
        self.live_bytes = baseline;
        result
    }

    fn inspect_manifest_inner(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
    ) -> Result<(), ItemRefusal> {
        if !path.starts_with("ToS/source-witnesses/") || !path.ends_with("/item.manifest.json") {
            return Err(ItemRefusal::Unsupported("item-manifest-owner-path".into()));
        }
        let Some((manifest, manifest_raw)) = self.object(source, path, MANIFEST)? else {
            return Ok(());
        };
        self.release_raw(&manifest_raw);
        drop(manifest_raw);
        let directory = path.rsplit_once('/').unwrap().0;
        let item_id = &manifest["item_id"];
        self.require_kind(source, path, item_id, "item")?;
        self.require_kind(source, path, &manifest["embodiment_ref"], "edition")?;
        if let Some(id) = item_id.as_str() {
            if !self.manifest_item_ids.contains(id) {
                self.reserve(
                    id.len() + std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>(),
                )?;
                self.manifest_item_ids.insert(id.into());
            }
        }
        let inventory_path = manifest["resource_inventory_ref"].as_str().unwrap_or("");
        let inventory = if inventory_path.is_empty() {
            None
        } else {
            self.inventory_object(source, inventory_path)?
        };
        if let Some((inventory, _)) = &inventory {
            match inventory {
                ItemInventoryValue::Finite(inventory) => {
                    self.inspect_finite_inventory(source, inventory_path, &manifest, inventory)?;
                }
                ItemInventoryValue::LegacyObserved(inventory) => {
                    self.inspect_observed_inventory(source, inventory_path, &manifest, inventory)?;
                }
            }
        }
        let rights_path = manifest["rights_ref"].as_str().unwrap_or("");
        let rights = if rights_path.is_empty() {
            None
        } else {
            self.object(source, rights_path, RIGHTS)?
        };
        let rights = rights.map(|(value, raw)| {
            self.release_raw(&raw);
            value
        });
        if let Some(rights) = &rights {
            if rights["scope_refs"].is_array()
                && !array(&rights["scope_refs"]).any(|scope| scope == item_id)
            {
                self.issue(rights_path, "rights-item-scope")?;
            }
            if rights["visibility"] != manifest["visibility"] {
                self.issue(rights_path, "rights-manifest-visibility")?;
            }
            let baseline = self.live_bytes;
            self.admit_live(std::mem::size_of::<BTreeSet<&str>>())?;
            let mut layer_ids = BTreeSet::new();
            for layer in array(&rights["layer_assessments"]).filter(|v| v.is_object()) {
                if let Some(id) = layer["layer_id"].as_str() {
                    if layer_ids.contains(id) {
                        self.issue(rights_path, "duplicate-rights-layer-id")?;
                    } else {
                        self.admit_live(
                            std::mem::size_of::<&str>() + 3 * std::mem::size_of::<usize>(),
                        )?;
                        layer_ids.insert(id);
                    }
                }
                self.source_refs(source, rights_path, layer)?;
            }
            drop(layer_ids);
            self.live_bytes = baseline;
        }
        let provenance_path = manifest["provenance_ref"].as_str().unwrap_or("");
        let acquisition_ref = manifest["acquisition_event_ref"].as_str();
        let inventory_event_ref = inventory.as_ref().and_then(|(value, _)| match value {
            ItemInventoryValue::Finite(value) => value["provenance_event_ref"].as_str(),
            ItemInventoryValue::LegacyObserved(value) => value
                .object_get("provenance_event_ref")
                .and_then(tos_foundation::JsonValue::as_str),
        });
        let inventory_digest = if let Some((_, raw)) = &inventory {
            self.admit_live(std::mem::size_of::<String>() + 64)?;
            Some(Digest256::of_bytes(raw).to_hex())
        } else {
            None
        };
        let mut acquisition_local = false;
        // The last local event with this ID owns the output comparison, as in
        // the source validator's local_events_by_id map.
        let mut inventory_output_local = None;
        if !provenance_path.is_empty() {
            if let Some(raw) = self.raw(source, provenance_path)? {
                let selection = source.record_selection();
                let verified = if let Some(selection) = &selection {
                    for slot in selection.file_slots(provenance_path) {
                        let scratch = slot.verification_state_upper_bound()?;
                        if scratch > self.available()? {
                            return Err(crate::item_budget_origin!());
                        }
                    }
                    Some(selection.verify_file(
                        provenance_path,
                        &raw,
                        self.limits.deadline,
                        source.cancellation_flag(),
                    )?)
                } else {
                    None
                };
                let mut rows = if let Some(verified) = &verified {
                    ItemProvenanceRows::Selected(verified.row_cursor())
                } else {
                    let Ok(text) = std::str::from_utf8(&raw) else {
                        self.issue(provenance_path, "invalid-jsonl-utf8")?;
                        return Ok(());
                    };
                    ItemProvenanceRows::Full(text.lines().enumerate())
                };
                while let Some(row) =
                    rows.next_checked(self.limits.deadline, source.cancellation_flag())
                {
                    let (ordinal, line) = row?;
                    let baseline = self.live_bytes;
                    let line_result = (|| -> Result<(), ItemRefusal> {
                        self.check()?;
                        let digits = ordinal.ilog10() as usize + 1;
                        self.admit_live(
                            std::mem::size_of::<String>() + provenance_path.len() + 1 + digits,
                        )?;
                        let location = format!("{provenance_path}:{ordinal}");
                        if line.trim().is_empty() {
                            return self.issue(&location, "blank-jsonl-line");
                        }
                        let available = self.available()?;
                        let decoded = crate::validation_codec::bounded_legacy_item_decoded_state(
                            line.as_bytes(),
                            item_json_limits(self.limits.max_member_bytes, available)?,
                            available,
                            self.limits.deadline,
                            source.cancellation_flag(),
                        );
                        let (event, event_bytes) = match decoded {
                            Ok(result) => result,
                            Err(ItemRefusal::Source(reason))
                                if reason == "invalid finite native JSON" =>
                            {
                                return self.issue(&location, "invalid-jsonl");
                            }
                            Err(error @ ItemRefusal::Unsupported(_)) => return Err(error),
                            Err(error) => {
                                return Err(item_codec_refusal(
                                    error,
                                    line.len(),
                                    available,
                                    self.limits.max_member_bytes,
                                ));
                            }
                        };
                        self.admit_live(event_bytes)?;
                        if !event.is_object() {
                            return self.issue(&location, "object-required");
                        }
                        if !source.schema(
                            &location,
                            line.as_bytes(),
                            if selection.is_some()
                                && event["schema_version"] == "tos_provenance_event_v2"
                            {
                                "ToS/contracts/provenance-event-v2.schema.json"
                            } else {
                                EVENT
                            },
                            self.limits.deadline,
                        )? {
                            self.issue(&location, "schema")?;
                        }
                        self.source_refs(source, &location, &event)?;
                        if let Some(id) = event["event_id"].as_str() {
                            if acquisition_ref == Some(id) {
                                acquisition_local = true;
                            }
                            if inventory_event_ref == Some(id) {
                                let digest = inventory_digest.as_deref().unwrap_or("");
                                let mut matched = false;
                                for output in array(&event["outputs"]) {
                                    self.check()?;
                                    let Some(object) = output.as_object() else {
                                        continue;
                                    };
                                    if object.len() == 3
                                        && output["ref"] == inventory_path
                                        && output["role"] == "tracked_text_free_resource_inventory"
                                        && output["sha256"] == digest
                                    {
                                        matched = true;
                                        break;
                                    }
                                }
                                inventory_output_local = Some(matched);
                            }
                            if self.event_ids.contains(id) {
                                self.issue(&location, "duplicate-event-id")?;
                            } else {
                                self.reserve(
                                    id.len()
                                        + std::mem::size_of::<String>()
                                        + 3 * std::mem::size_of::<usize>(),
                                )?;
                                self.event_ids.insert(id.into());
                            }
                        }
                        Ok(())
                    })();
                    self.live_bytes = baseline;
                    line_result?;
                }
                self.release_raw(&raw);
            }
        }
        if !acquisition_local {
            self.issue(path, "acquisition-event-not-local")?;
        }
        if inventory.is_some() {
            match inventory_output_local {
                Some(false) => self.issue(inventory_path, "inventory-provenance-output")?,
                None => self.issue(inventory_path, "inventory-event-not-local")?,
                Some(true) => {}
            }
        }
        let scope_baseline = self.live_bytes;
        let mut rights_scopes = BTreeSet::new();
        if let Some(rights) = &rights {
            if rights["scope_refs"].is_array() {
                self.admit_live(std::mem::size_of::<BTreeSet<&str>>())?;
                for scope in array(&rights["scope_refs"]).filter_map(Value::as_str) {
                    self.check()?;
                    if !rights_scopes.contains(scope) {
                        self.admit_live(
                            std::mem::size_of::<&str>() + 3 * std::mem::size_of::<usize>(),
                        )?;
                        rights_scopes.insert(scope);
                    }
                }
            }
        }
        for entry in array(&manifest["payload_files"]).filter(|v| v.is_object()) {
            self.check()?;
            let sha = entry["sha256"].as_str().unwrap_or("None");
            let relative = entry["relative_path"].as_str().unwrap_or("None");
            if let (Some(file_id), Some(_)) = (entry["file_id"].as_str(), item_id.as_str()) {
                let descriptor = [&entry["sha256"], &entry["byte_size"], &entry["media_type"]];
                if self
                    .file_descriptors
                    .get(file_id)
                    .is_some_and(|previous| previous.iter().zip(descriptor).any(|(a, b)| a != b))
                {
                    self.issue(path, "file-identity-conflict")?;
                } else if !self.file_descriptors.contains_key(file_id) {
                    let bytes = descriptor.iter().try_fold(
                        file_id.len()
                            + std::mem::size_of::<String>()
                            + 3 * std::mem::size_of::<usize>(),
                        |size, value| {
                            size.checked_add(retained_value_bytes(value)?)
                                .ok_or(crate::item_budget_origin!())
                        },
                    )?;
                    self.reserve(bytes)?;
                    self.file_descriptors.insert(
                        file_id.into(),
                        [
                            descriptor[0].clone(),
                            descriptor[1].clone(),
                            descriptor[2].clone(),
                        ],
                    );
                }
                if file_id.strip_prefix("tos.file.sha256.") != Some(sha) {
                    self.issue(path, "file-id-sha256")?;
                }
                if let Some(rights) = &rights {
                    if rights["scope_refs"].is_array() && !rights_scopes.contains(file_id) {
                        self.issue(rights_path, "rights-file-scope")?;
                    }
                }
            }
            if entry["relative_path"].as_str().is_none() {
                self.issue(path, "payload-path-string")?;
                continue;
            }
            let payload_baseline = self.live_bytes;
            self.admit_live(std::mem::size_of::<String>() + directory.len() + 1 + relative.len())?;
            // The selected payload adapter returns one SHA-256 hex String at
            // most; keep its owned result live with the path and issue state.
            self.admit_live(
                std::mem::size_of::<ItemPayload>() + std::mem::size_of::<String>() + 64,
            )?;
            let payload_path = format!("{directory}/{relative}");
            safe_path(&payload_path)?;
            match source.payload(&payload_path, self.limits.deadline)? {
                ItemPayload::Unavailable => {
                    self.unavailable_payloads = self
                        .unavailable_payloads
                        .checked_add(1)
                        .ok_or(crate::item_budget_origin!())?;
                    if self.require_local_payloads {
                        self.issue(&payload_path, "required-payload-unavailable")?;
                    }
                }
                ItemPayload::File {
                    byte_size,
                    sha256,
                    excluded_from_source,
                } => {
                    if entry["byte_size"].as_u64() != Some(byte_size) {
                        self.issue(&payload_path, "payload-byte-size")?;
                    }
                    if sha256 != sha {
                        self.issue(&payload_path, "payload-sha256")?;
                    }
                    if !excluded_from_source {
                        self.issue(&payload_path, "payload-source-inclusion")?;
                    }
                }
            }
            self.check()?;
            drop(payload_path);
            self.live_bytes = payload_baseline;
        }
        drop(rights_scopes);
        self.live_bytes = scope_baseline;
        self.admit_live(std::mem::size_of::<String>() + directory.len() + "/fixity.sha256".len())?;
        let fixity_path = format!("{directory}/fixity.sha256");
        if let Some(raw) = self.raw(source, &fixity_path)? {
            // Preserve the owner legacy conditional: an existing empty
            // companion is not compared here; schema/owner review may change
            // that separately. Missing still emits missing-companion.
            if !raw.is_empty() && !self.fixity_matches(&raw, &manifest["payload_files"])? {
                self.issue(&fixity_path, "fixity-manifest-drift")?;
            }
        }
        Ok(())
    }

    /// Invoke for every current native Item record after manifest traversal.
    pub(crate) fn item_record_read_limit(&self, path: &str) -> Result<usize, ItemRefusal> {
        let header = std::mem::size_of::<Vec<u8>>()
            .checked_add(std::mem::size_of::<String>())
            .and_then(|n| n.checked_add(path.len()))
            .ok_or(crate::item_budget_origin!())?;
        Ok(self.limits.max_member_bytes.min(
            self.available()?
                .checked_sub(header)
                .ok_or(crate::item_budget_origin!())?,
        ))
    }

    /// Invoke for every current native Item record after manifest traversal.
    pub fn inspect_item_record(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        raw: &[u8],
    ) -> Result<(), ItemRefusal> {
        let baseline = self.live_bytes;
        let result = self.inspect_item_record_inner(source, path, raw);
        self.live_bytes = baseline;
        result
    }

    fn inspect_item_record_inner(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        raw: &[u8],
    ) -> Result<(), ItemRefusal> {
        self.check()?;
        if raw.len() > self.limits.max_member_bytes {
            return Err(crate::item_budget_origin!());
        }
        let header = std::mem::size_of::<Vec<u8>>()
            .checked_add(std::mem::size_of::<String>())
            .and_then(|n| n.checked_add(path.len()))
            .ok_or(crate::item_budget_origin!())?;
        self.admit_live(
            header
                .checked_add(raw.len())
                .ok_or(crate::item_budget_origin!())?,
        )?;
        let available = self.available()?;
        let decoded = crate::validation_codec::bounded_legacy_item_decoded_state(
            raw,
            item_json_limits(self.limits.max_member_bytes, available)?,
            available,
            self.limits.deadline,
            source.cancellation_flag(),
        );
        let (item, item_bytes) = match decoded {
            Ok(result) => result,
            Err(ItemRefusal::Source(reason)) if reason == "invalid finite native JSON" => {
                return Err(ItemRefusal::Source("item-record-json".into()));
            }
            Err(error @ ItemRefusal::Unsupported(_)) => return Err(error),
            Err(error) => {
                return Err(item_codec_refusal(
                    error,
                    raw.len(),
                    available,
                    self.limits.max_member_bytes,
                ));
            }
        };
        self.admit_live(item_bytes)?;
        if let Some(id) = item["record_id"].as_str() {
            if !self.manifest_item_ids.contains(id) {
                self.issue(path, "item-without-manifest")?;
            }
            if let Some(target) = item["item_manifest_ref"].as_str() {
                if let Some((manifest, _)) = self.object(source, target, MANIFEST)? {
                    if manifest["item_id"] != id {
                        self.issue(target, "manifest-item-record-id")?;
                    }
                }
            }
        }
        Ok(())
    }

    fn fixity_matches(&self, mut actual: &[u8], files: &Value) -> Result<bool, ItemRefusal> {
        for entry in array(files).filter(|value| value.is_object()) {
            self.check()?;
            let sha = entry["sha256"].as_str().unwrap_or("None");
            let relative = entry["relative_path"].as_str().unwrap_or("None");
            let Some(rest) = actual
                .strip_prefix(sha.as_bytes())
                .and_then(|rest| rest.strip_prefix(b"  "))
                .and_then(|rest| rest.strip_prefix(relative.as_bytes()))
                .and_then(|rest| rest.strip_prefix(b"\n"))
            else {
                return Ok(false);
            };
            actual = rest;
        }
        Ok(actual.is_empty())
    }

    pub fn finish(self) -> ItemFamilyReport {
        ItemFamilyReport {
            issues: self.issues,
            manifest_item_ids: self.manifest_item_ids,
            metadata_bytes: self.metadata_bytes,
            unavailable_payloads: self.unavailable_payloads,
            accounted_state_upper_bound_bytes: self.state_bytes,
            inventory_set_scan_steps: self.inventory_set_scan_steps,
            source_admission_complete: false,
        }
    }
}

fn array(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

const LEGACY_JSON_NULL: tos_foundation::JsonValue = tos_foundation::JsonValue::Null;

pub(crate) fn observed_json_retained_bytes(
    value: &tos_foundation::JsonValue,
) -> Result<usize, ItemRefusal> {
    fn vector_capacity_bytes(length: usize, cell: usize) -> Result<usize, ItemRefusal> {
        if length == 0 {
            return Ok(0);
        }
        let capacity = length
            .checked_mul(2)
            .ok_or(crate::item_budget_origin!())?
            .max(4);
        capacity
            .checked_mul(cell)
            .ok_or(crate::item_budget_origin!())
    }
    fn string_bytes(value: &tos_foundation::JsonString) -> Result<usize, ItemRefusal> {
        let units = vector_capacity_bytes(value.units().len(), std::mem::size_of::<u16>())?;
        let utf8 = value
            .as_str()
            .map(|text| {
                text.len()
                    .checked_mul(2)
                    .ok_or(crate::item_budget_origin!())
            })
            .transpose()?
            .unwrap_or_default();
        units.checked_add(utf8).ok_or(crate::item_budget_origin!())
    }
    fn visit(value: &tos_foundation::JsonValue) -> Result<usize, ItemRefusal> {
        let mut bytes = std::mem::size_of::<tos_foundation::JsonValue>();
        match value {
            tos_foundation::JsonValue::Null | tos_foundation::JsonValue::Bool(_) => {}
            tos_foundation::JsonValue::Number(number) => {
                bytes = bytes
                    .checked_add(
                        number
                            .lexeme
                            .len()
                            .checked_mul(2)
                            .ok_or(crate::item_budget_origin!())?,
                    )
                    .ok_or(crate::item_budget_origin!())?;
            }
            tos_foundation::JsonValue::String(text) => {
                bytes = bytes
                    .checked_add(string_bytes(text)?)
                    .ok_or(crate::item_budget_origin!())?;
            }
            tos_foundation::JsonValue::Array(values) => {
                bytes = bytes
                    .checked_add(vector_capacity_bytes(
                        values.len(),
                        std::mem::size_of::<tos_foundation::JsonValue>(),
                    )?)
                    .ok_or(crate::item_budget_origin!())?;
                for child in values {
                    bytes = bytes
                        .checked_add(visit(child)?)
                        .ok_or(crate::item_budget_origin!())?;
                }
            }
            tos_foundation::JsonValue::Object(entries) => {
                bytes =
                    bytes
                        .checked_add(vector_capacity_bytes(
                            entries.len(),
                            std::mem::size_of::<(
                                tos_foundation::JsonString,
                                tos_foundation::JsonValue,
                            )>(),
                        )?)
                        .ok_or(crate::item_budget_origin!())?;
                for (key, child) in entries {
                    bytes = bytes
                        .checked_add(string_bytes(key)?)
                        .and_then(|bytes| bytes.checked_add(visit(child).ok()?))
                        .ok_or(crate::item_budget_origin!())?;
                }
            }
        }
        Ok(bytes)
    }
    visit(value)
}

pub(crate) fn legacy_json_values_equal(
    left: &tos_foundation::JsonValue,
    right: &tos_foundation::JsonValue,
) -> bool {
    use tos_foundation::{JsonNumberKind as NumberKind, JsonValue as LegacyValue};
    match (left, right) {
        (LegacyValue::Null, LegacyValue::Null) => true,
        (LegacyValue::Bool(left), LegacyValue::Bool(right)) => left == right,
        (LegacyValue::Bool(value), LegacyValue::Number(number))
        | (LegacyValue::Number(number), LegacyValue::Bool(value)) => {
            legacy_number_equals_integer(number, if *value { "1" } else { "0" })
        }
        (LegacyValue::Number(left), LegacyValue::Number(right)) => match (left.kind, right.kind) {
            (NumberKind::Int, NumberKind::Int) => {
                integer_lexemes_equal(&left.lexeme, &right.lexeme)
            }
            (NumberKind::Int, NumberKind::Float) => right
                .as_python_float()
                .is_some_and(|value| integer_equals_float(&left.lexeme, value)),
            (NumberKind::Float, NumberKind::Int) => left
                .as_python_float()
                .is_some_and(|value| integer_equals_float(&right.lexeme, value)),
            (NumberKind::Float, NumberKind::Float) => left
                .as_python_float()
                .zip(right.as_python_float())
                .is_some_and(|(left, right)| left == right),
        },
        (LegacyValue::String(left), LegacyValue::String(right)) => left.units() == right.units(),
        (LegacyValue::Array(left), LegacyValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| legacy_json_values_equal(left, right))
        }
        (LegacyValue::Object(left), LegacyValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left_value)| {
                    right
                        .iter()
                        .find(|(right_key, _)| key.units() == right_key.units())
                        .is_some_and(|(_, right_value)| {
                            legacy_json_values_equal(left_value, right_value)
                        })
                })
        }
        _ => false,
    }
}

/// Python's JSON decoder returns its cached `NaN` constant object for each
/// literal. At the source owner's `set(resource_ids)` boundary those values
/// compare as the same set member through Python's identity fast path, even
/// though ordinary numeric equality remains non-reflexive.
pub(crate) fn legacy_json_set_members_equal(
    left: &tos_foundation::JsonValue,
    right: &tos_foundation::JsonValue,
) -> bool {
    fn is_cached_nan(value: &tos_foundation::JsonValue) -> bool {
        matches!(
            value,
            tos_foundation::JsonValue::Number(number)
                if number.kind == tos_foundation::JsonNumberKind::Float
                    && number.lexeme == "NaN"
        )
    }
    (is_cached_nan(left) && is_cached_nan(right)) || legacy_json_values_equal(left, right)
}

/// Python set membership for JSON-decoded hashable values. Arrays and objects
/// are rejected by the caller before this comparator runs.
pub(crate) fn native_json_set_members_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Bool(value), Value::Number(number))
        | (Value::Number(number), Value::Bool(value)) => {
            native_number_equals_integer(number, if *value { "1" } else { "0" })
        }
        (Value::Number(left), Value::Number(right)) => {
            let left_integer = left
                .as_i64()
                .map(|value| value.to_string())
                .or_else(|| left.as_u64().map(|value| value.to_string()));
            let right_integer = right
                .as_i64()
                .map(|value| value.to_string())
                .or_else(|| right.as_u64().map(|value| value.to_string()));
            match (left_integer, right_integer) {
                (Some(left), Some(right)) => integer_lexemes_equal(&left, &right),
                (Some(integer), None) => right
                    .as_f64()
                    .is_some_and(|float| integer_equals_float(&integer, float)),
                (None, Some(integer)) => left
                    .as_f64()
                    .is_some_and(|float| integer_equals_float(&integer, float)),
                (None, None) => left
                    .as_f64()
                    .zip(right.as_f64())
                    .is_some_and(|(left, right)| left == right),
            }
        }
        (Value::String(left), Value::String(right)) => left == right,
        _ => false,
    }
}

pub(crate) fn legacy_json_native_equal(legacy: &tos_foundation::JsonValue, native: &Value) -> bool {
    use tos_foundation::{JsonNumberKind as NumberKind, JsonValue as LegacyValue};
    match (legacy, native) {
        (LegacyValue::Null, Value::Null) => true,
        (LegacyValue::Bool(left), Value::Bool(right)) => left == right,
        (LegacyValue::Bool(value), Value::Number(number)) => {
            native_number_equals_integer(number, if *value { "1" } else { "0" })
        }
        (LegacyValue::Number(number), Value::Bool(value)) => {
            legacy_number_equals_integer(number, if *value { "1" } else { "0" })
        }
        (LegacyValue::Number(left), Value::Number(right)) => match left.kind {
            NumberKind::Int => {
                if let Some(value) = right.as_i64() {
                    integer_lexemes_equal(&left.lexeme, &value.to_string())
                } else if let Some(value) = right.as_u64() {
                    integer_lexemes_equal(&left.lexeme, &value.to_string())
                } else {
                    right
                        .as_f64()
                        .is_some_and(|value| integer_equals_float(&left.lexeme, value))
                }
            }
            NumberKind::Float => {
                let Some(left) = left.as_python_float() else {
                    return false;
                };
                if let Some(value) = right.as_i64() {
                    integer_equals_float(&value.to_string(), left)
                } else if let Some(value) = right.as_u64() {
                    integer_equals_float(&value.to_string(), left)
                } else {
                    right.as_f64().is_some_and(|right| left == right)
                }
            }
        },
        (LegacyValue::String(left), Value::String(right)) => {
            left.as_str().is_some_and(|left| left == right)
        }
        (LegacyValue::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| legacy_json_native_equal(left, right))
        }
        (LegacyValue::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left_value)| {
                    key.as_str()
                        .and_then(|key| right.get(key))
                        .is_some_and(|right_value| {
                            legacy_json_native_equal(left_value, right_value)
                        })
                })
        }
        _ => false,
    }
}

fn native_number_equals_integer(number: &serde_json::Number, integer: &str) -> bool {
    if let Some(value) = number.as_i64() {
        integer_lexemes_equal(&value.to_string(), integer)
    } else if let Some(value) = number.as_u64() {
        integer_lexemes_equal(&value.to_string(), integer)
    } else {
        number
            .as_f64()
            .is_some_and(|value| integer_equals_float(integer, value))
    }
}

fn legacy_number_equals_integer(number: &tos_foundation::JsonNumber, integer: &str) -> bool {
    match number.kind {
        tos_foundation::JsonNumberKind::Int => integer_lexemes_equal(&number.lexeme, integer),
        tos_foundation::JsonNumberKind::Float => number
            .as_python_float()
            .is_some_and(|value| integer_equals_float(integer, value)),
    }
}

fn integer_lexemes_equal(left: &str, right: &str) -> bool {
    fn parts(value: &str) -> (bool, &str) {
        let negative = value.starts_with('-');
        let digits = value.strip_prefix('-').unwrap_or(value);
        let digits = digits.trim_start_matches('0');
        (
            negative && !digits.is_empty(),
            if digits.is_empty() { "0" } else { digits },
        )
    }
    let (left_negative, left_digits) = parts(left);
    let (right_negative, right_digits) = parts(right);
    left_negative == right_negative && left_digits == right_digits
}

fn integer_equals_float(integer: &str, value: f64) -> bool {
    if !value.is_finite() || value.fract() != 0.0 {
        return false;
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0 && value != 0.0;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let (mut significand, exponent) = if exponent_bits == 0 {
        (fraction, 1 - 1023 - 52)
    } else {
        ((1u64 << 52) | fraction, exponent_bits - 1023 - 52)
    };
    if significand == 0 {
        return integer_lexemes_equal(integer, "0");
    }
    if exponent < 0 {
        let shift = (-exponent) as u32;
        if shift >= u64::BITS || significand % (1u64 << shift) != 0 {
            return false;
        }
        significand /= 1u64 << shift;
    }
    let mut digits = [0u8; 310];
    let mut length = 0usize;
    let mut remaining = significand;
    while remaining != 0 {
        digits[length] = (remaining % 10) as u8;
        remaining /= 10;
        length += 1;
    }
    for _ in 0..exponent.max(0) {
        let mut carry = 0u16;
        for digit in &mut digits[..length] {
            let doubled = u16::from(*digit) * 2 + carry;
            *digit = (doubled % 10) as u8;
            carry = doubled / 10;
        }
        while carry != 0 {
            if length >= digits.len() {
                return false;
            }
            digits[length] = (carry % 10) as u8;
            carry /= 10;
            length += 1;
        }
    }
    let integer_negative = integer.starts_with('-');
    let integer_digits = integer
        .strip_prefix('-')
        .unwrap_or(integer)
        .trim_start_matches('0');
    let integer_digits = if integer_digits.is_empty() {
        "0"
    } else {
        integer_digits
    };
    if integer_negative != negative || integer_digits.len() != length {
        return false;
    }
    integer_digits
        .bytes()
        .rev()
        .zip(&digits[..length])
        .all(|(byte, digit)| byte == b'0' + *digit)
}

fn item_json_limits(
    max_bytes: usize,
    available: usize,
) -> Result<tos_foundation::JsonLimits, ItemRefusal> {
    tos_foundation::JsonLimits::new(max_bytes, 128, available.max(1), max_bytes.max(1))
        .map_err(|_| crate::item_budget_origin!())
}

fn item_codec_visits(raw_len: usize, available: usize) -> Result<usize, ItemRefusal> {
    let string_workspace = raw_len.checked_mul(5).ok_or(crate::item_budget_origin!())?;
    let visit_slot = std::mem::size_of::<tos_foundation::JsonValue>()
        + std::mem::size_of::<tos_foundation::JsonString>()
        + std::mem::size_of::<(Vec<u16>, usize)>()
        + std::mem::size_of::<std::collections::HashMap<Vec<u16>, usize>>();
    let remaining = available
        .checked_sub(string_workspace)
        .ok_or(crate::item_budget_origin!())?;
    Ok((remaining / visit_slot).min(available.max(1)))
}

fn item_codec_refusal(
    error: ItemRefusal,
    raw_len: usize,
    available: usize,
    max_member_bytes: usize,
) -> ItemRefusal {
    if !matches!(
        &error,
        ItemRefusal::BudgetCheck {
            check: "strict JSON codec bytes/depth/visits/integer",
            ..
        }
    ) {
        return error;
    }
    // This is the existing bounded legacy helper's combined FND guard. Its
    // depth/visit/integer profile can reject input the direct serde Item route
    // accepted, so it is an explicit capability gap, never malformed source.
    let visits = item_codec_visits(raw_len, available).unwrap_or(0);
    ItemRefusal::Unsupported(format!(
        "Item bounded legacy JSON profile: max_depth=128 max_visits={visits} max_integer_digits={}",
        max_member_bytes.max(1)
    ))
}

// Charge persistent descriptor values before cloning them into the cross-Item
// index. Include each Value cell, string bytes, and collection entries; the
// source parser's temporary tree is separately bounded by member bytes.
fn retained_value_bytes(value: &Value) -> Result<usize, ItemRefusal> {
    let cell = std::mem::size_of::<Value>();
    let extra = match value {
        Value::String(text) => text.len(),
        Value::Number(number) => number.as_str().len(),
        Value::Array(values) => values.iter().try_fold(0usize, |size, child| {
            size.checked_add(retained_value_bytes(child)?)
                .ok_or(crate::item_budget_origin!())
        })?,
        Value::Object(values) => values.iter().try_fold(0usize, |size, (key, child)| {
            let child_size = retained_value_bytes(child)?;
            let entry = std::mem::size_of::<String>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .and_then(|size| size.checked_add(key.len()))
                .and_then(|size| size.checked_add(child_size))
                .ok_or(crate::item_budget_origin!())?;
            size.checked_add(entry).ok_or(crate::item_budget_origin!())
        })?,
        _ => 0,
    };
    cell.checked_add(extra).ok_or(crate::item_budget_origin!())
}

fn safe_path(path: &str) -> Result<(), ItemRefusal> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(['\\', '\0'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part == ".git")
        || path
            .split('/')
            .next()
            .is_some_and(|part| part.contains(':'))
    {
        return Err(ItemRefusal::Unsupported("unsafe-source-path".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FormatProfile, SchemaBackendProbe, SchemaResource};
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::Duration;

    const ITEM: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1884-part-3/editions/chemnitz-schmeitzner-1884-part-3/items/dta-sbb-corrected-tei-p5";

    struct Fixture {
        root: PathBuf,
        members: BTreeMap<String, Vec<u8>>,
        kinds: BTreeMap<String, String>,
        schema: SchemaBackendProbe,
        cancelled: AtomicBool,
    }

    impl Fixture {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
            let mut members = BTreeMap::new();
            for basename in [
                "item.manifest.json",
                "rights.json",
                "provenance.jsonl",
                "resource-inventory.json",
                "fixity.sha256",
            ] {
                let path = format!("{ITEM}/{basename}");
                members.insert(path.clone(), std::fs::read(root.join(path)).unwrap());
            }
            let manifest: Value =
                serde_json::from_slice(&members[&format!("{ITEM}/item.manifest.json")]).unwrap();
            let kinds = [
                (manifest["item_id"].as_str().unwrap().into(), "item".into()),
                (
                    manifest["embodiment_ref"].as_str().unwrap().into(),
                    "edition".into(),
                ),
            ]
            .into_iter()
            .collect();
            let resources = [MANIFEST, INVENTORY, RIGHTS, EVENT]
                .into_iter()
                .map(|path| {
                    let raw = std::fs::read(root.join(path)).unwrap();
                    let value: Value = serde_json::from_slice(&raw).unwrap();
                    SchemaResource {
                        uri: value["$id"].as_str().unwrap().into(),
                        raw,
                    }
                });
            let schema =
                SchemaBackendProbe::new(resources, FormatProfile::LegacyPythonObserved20260923)
                    .unwrap();
            Self {
                root,
                members,
                kinds,
                schema,
                cancelled: AtomicBool::new(false),
            }
        }
    }

    impl ItemSource for Fixture {
        fn cancellation_flag(&self) -> &AtomicBool {
            &self.cancelled
        }

        fn metadata(
            &mut self,
            path: &str,
            _: usize,
            _: Instant,
        ) -> Result<Option<Vec<u8>>, ItemRefusal> {
            Ok(self.members.get(path).cloned())
        }
        fn exists(&mut self, path: &str, _: Instant) -> Result<bool, ItemRefusal> {
            Ok(self.members.contains_key(path) || self.root.join(path).exists())
        }
        fn schema(
            &mut self,
            _: &str,
            raw: &[u8],
            contract: &str,
            _: Instant,
        ) -> Result<bool, ItemRefusal> {
            let value =
                serde_json::from_slice(raw).map_err(|_| ItemRefusal::Source("json".into()))?;
            self.schema
                .is_valid(&format!("https://tree-of-sophia.local/{contract}"), &value)
                .map_err(|error| ItemRefusal::Unsupported(format!("{error:?}")))
        }
        fn payload(&mut self, _: &str, _: Instant) -> Result<ItemPayload, ItemRefusal> {
            Ok(ItemPayload::Unavailable)
        }
        fn record_kind(&mut self, id: &str, _: Instant) -> Result<Option<&str>, ItemRefusal> {
            Ok(self.kinds.get(id).map(String::as_str))
        }
    }

    fn rules(required: bool) -> ItemRules {
        ItemRules::new(
            ItemLimits {
                max_member_bytes: 1_048_576,
                max_total_bytes: 16 * 1_048_576,
                max_state_bytes: 1_048_576,
                max_issues: 64,
                deadline: Instant::now() + Duration::from_secs(30),
            },
            required,
        )
    }

    #[test]
    fn source_item_companions_and_metadata_only_boundary() {
        let path = format!("{ITEM}/item.manifest.json");
        let mut fixture = Fixture::new();
        let mut good = rules(false);
        good.inspect_manifest(&mut fixture, &path).unwrap();
        let report = good.finish();
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        assert_eq!(report.unavailable_payloads, 1);
        assert!(!report.source_admission_complete);

        let mut required = rules(true);
        required.inspect_manifest(&mut fixture, &path).unwrap();
        assert!(
            required
                .finish()
                .issues
                .iter()
                .any(|issue| issue.code == "required-payload-unavailable")
        );

        let inventory_path = format!("{ITEM}/resource-inventory.json");
        let mut inventory: Value =
            serde_json::from_slice(&fixture.members[&inventory_path]).unwrap();
        inventory["files"][0]["summary"]["resource_count"] = json!(999);
        fixture
            .members
            .insert(inventory_path, serde_json::to_vec(&inventory).unwrap());
        let mut drift = rules(false);
        drift.inspect_manifest(&mut fixture, &path).unwrap();
        let report = drift.finish();
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "resource-count")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "inventory-provenance-output")
        );

        let mut duplicate = rules(false);
        let mut fixture = Fixture::new();
        duplicate.inspect_manifest(&mut fixture, &path).unwrap();
        duplicate.inspect_manifest(&mut fixture, &path).unwrap();
        assert!(
            duplicate
                .finish()
                .issues
                .iter()
                .any(|issue| issue.code == "duplicate-event-id")
        );
    }
}
