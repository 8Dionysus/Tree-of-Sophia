//! Executable Item companion and payload mechanics from the source foundation
//! owner. This family has no admission/seal constructor. Complete source
//! enumeration, retained history and current rights remain separate owners.

use std::collections::{BTreeMap, BTreeSet};
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
    Deadline,
    Source(String),
    Unsupported(String),
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
        }
    }

    fn check(&self) -> Result<(), ItemRefusal> {
        if Instant::now() >= self.limits.deadline {
            Err(ItemRefusal::Deadline)
        } else {
            Ok(())
        }
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        self.check()?;
        let retained = self
            .state_bytes
            .checked_add(bytes)
            .ok_or(ItemRefusal::Budget)?;
        if retained
            .checked_add(self.live_bytes)
            .is_none_or(|total| total > self.limits.max_state_bytes)
        {
            return Err(ItemRefusal::Budget);
        }
        self.state_bytes = retained;
        Ok(())
    }

    fn available(&self) -> Result<usize, ItemRefusal> {
        self.limits
            .max_state_bytes
            .checked_sub(self.state_bytes)
            .and_then(|bytes| bytes.checked_sub(self.live_bytes))
            .ok_or(ItemRefusal::Budget)
    }

    fn admit_live(&mut self, bytes: usize) -> Result<(), ItemRefusal> {
        self.check()?;
        if bytes > self.available()? {
            return Err(ItemRefusal::Budget);
        }
        self.live_bytes = self
            .live_bytes
            .checked_add(bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn release_raw(&mut self, raw: &[u8]) {
        self.live_bytes -= std::mem::size_of::<Vec<u8>>() + raw.len();
    }

    fn issue(&mut self, path: &str, code: &'static str) -> Result<(), ItemRefusal> {
        if self.issues.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
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
            .ok_or(ItemRefusal::Budget)?;
        let total_remaining = self
            .limits
            .max_total_bytes
            .checked_sub(self.metadata_bytes)
            .ok_or(ItemRefusal::Budget)?
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
                return Err(ItemRefusal::Budget);
            }
            self.admit_live(std::mem::size_of::<Vec<u8>>() + raw.len())?;
            self.metadata_bytes = self
                .metadata_bytes
                .checked_add(raw.len() as u64)
                .filter(|n| *n <= self.limits.max_total_bytes)
                .ok_or(ItemRefusal::Budget)?;
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
        let decoded = crate::record_biblio_cut::bounded_legacy_decoded_state_with_limits(
            &raw,
            item_json_limits(self.limits.max_member_bytes, available)?,
            available,
            self.limits.deadline,
            &AtomicBool::new(false),
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
            self.object(source, inventory_path, INVENTORY)?
        };
        if let Some((inventory, _)) = &inventory {
            if inventory["item_id"] != *item_id {
                self.issue(inventory_path, "inventory-item-id")?;
            }
            if inventory["generated_from_manifest_ref"] != path {
                self.issue(inventory_path, "inventory-manifest-ref")?;
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
                self.issue(inventory_path, "inventory-file-identity")?;
            }
            for entry in array(&inventory["files"]).filter(|v| v.is_object()) {
                self.check()?;
                let resources = &entry["resources"];
                let baseline = self.live_bytes;
                self.admit_live(std::mem::size_of::<BTreeSet<String>>())?;
                let mut ids = BTreeSet::new();
                for resource in array(resources).filter(|v| v.is_object()) {
                    self.check()?;
                    let id = &resource["resource_id"];
                    let header = std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>();
                    let wire = crate::record_biblio_cut::decoded_wire_size(
                        id,
                        self.available()?
                            .checked_sub(header)
                            .ok_or(ItemRefusal::Budget)?,
                    )?;
                    let cost = header.checked_add(wire).ok_or(ItemRefusal::Budget)?;
                    self.admit_live(cost)?;
                    if !ids.insert(id.to_string()) {
                        self.live_bytes -= cost;
                        self.issue(inventory_path, "duplicate-resource-id")?;
                    }
                }
                if entry["summary"].is_object()
                    && entry["summary"]["resource_count"].as_u64()
                        != Some(array(resources).count() as u64)
                {
                    self.issue(inventory_path, "resource-count")?;
                }
                drop(ids);
                self.live_bytes = baseline;
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
        let inventory_event_ref = inventory
            .as_ref()
            .and_then(|(value, _)| value["provenance_event_ref"].as_str());
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
                let Ok(text) = std::str::from_utf8(&raw) else {
                    self.issue(provenance_path, "invalid-jsonl-utf8")?;
                    return Ok(());
                };
                // splitlines, like Python, does not turn the terminal LF into
                // a fictitious blank record. Interior blank lines do reject.
                for (i, line) in text.lines().enumerate() {
                    let baseline = self.live_bytes;
                    let line_result = (|| -> Result<(), ItemRefusal> {
                        self.check()?;
                        let ordinal = i + 1;
                        let digits = ordinal.ilog10() as usize + 1;
                        self.admit_live(
                            std::mem::size_of::<String>() + provenance_path.len() + 1 + digits,
                        )?;
                        let location = format!("{provenance_path}:{ordinal}");
                        if line.trim().is_empty() {
                            return self.issue(&location, "blank-jsonl-line");
                        }
                        let available = self.available()?;
                        let decoded =
                            crate::record_biblio_cut::bounded_legacy_decoded_state_with_limits(
                                line.as_bytes(),
                                item_json_limits(self.limits.max_member_bytes, available)?,
                                available,
                                self.limits.deadline,
                                &AtomicBool::new(false),
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
                            EVENT,
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
                                .ok_or(ItemRefusal::Budget)
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
            let payload_path = format!("{directory}/{relative}");
            safe_path(&payload_path)?;
            match source.payload(&payload_path, self.limits.deadline)? {
                ItemPayload::Unavailable => {
                    self.unavailable_payloads = self
                        .unavailable_payloads
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?;
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
            .ok_or(ItemRefusal::Budget)?;
        Ok(self.limits.max_member_bytes.min(
            self.available()?
                .checked_sub(header)
                .ok_or(ItemRefusal::Budget)?,
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
            return Err(ItemRefusal::Budget);
        }
        let header = std::mem::size_of::<Vec<u8>>()
            .checked_add(std::mem::size_of::<String>())
            .and_then(|n| n.checked_add(path.len()))
            .ok_or(ItemRefusal::Budget)?;
        self.admit_live(header.checked_add(raw.len()).ok_or(ItemRefusal::Budget)?)?;
        let available = self.available()?;
        let decoded = crate::record_biblio_cut::bounded_legacy_decoded_state_with_limits(
            raw,
            item_json_limits(self.limits.max_member_bytes, available)?,
            available,
            self.limits.deadline,
            &AtomicBool::new(false),
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
            source_admission_complete: false,
        }
    }
}

fn array(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn item_json_limits(
    max_bytes: usize,
    available: usize,
) -> Result<tos_foundation::JsonLimits, ItemRefusal> {
    tos_foundation::JsonLimits::new(max_bytes, 128, available.max(1), max_bytes.max(1))
        .map_err(|_| ItemRefusal::Budget)
}

fn item_codec_visits(raw_len: usize, available: usize) -> Result<usize, ItemRefusal> {
    let string_workspace = raw_len.checked_mul(5).ok_or(ItemRefusal::Budget)?;
    let visit_slot = std::mem::size_of::<tos_foundation::JsonValue>()
        + std::mem::size_of::<tos_foundation::JsonString>()
        + std::mem::size_of::<(Vec<u16>, usize)>()
        + std::mem::size_of::<std::collections::HashMap<Vec<u16>, usize>>();
    let remaining = available
        .checked_sub(string_workspace)
        .ok_or(ItemRefusal::Budget)?;
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
                .ok_or(ItemRefusal::Budget)
        })?,
        Value::Object(values) => values.iter().try_fold(0usize, |size, (key, child)| {
            let child_size = retained_value_bytes(child)?;
            let entry = std::mem::size_of::<String>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .and_then(|size| size.checked_add(key.len()))
                .and_then(|size| size.checked_add(child_size))
                .ok_or(ItemRefusal::Budget)?;
            size.checked_add(entry).ok_or(ItemRefusal::Budget)
        })?,
        _ => 0,
    };
    cell.checked_add(extra).ok_or(ItemRefusal::Budget)
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
            }
        }
    }

    impl ItemSource for Fixture {
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
