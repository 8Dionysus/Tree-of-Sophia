//! Executable Item companion and payload mechanics from the source foundation
//! owner. This family has no admission/seal constructor. Complete source
//! enumeration, retained history and current rights remain separate owners.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use serde_json::{Value, json};
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
    BudgetCheck { check: &'static str, used: Option<u64>, limit: Option<u64> },
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
    fn record_kind(&mut self, id: &str, deadline: Instant) -> Result<Option<String>, ItemRefusal>;
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
    metadata_bytes: u64,
    unavailable_payloads: u64,
    manifest_item_ids: BTreeSet<String>,
    event_ids: BTreeSet<String>,
    file_descriptors: BTreeMap<String, Value>,
}

impl ItemRules {
    pub fn new(limits: ItemLimits, require_local_payloads: bool) -> Self {
        Self {
            limits,
            require_local_payloads,
            issues: Vec::new(),
            state_bytes: 0,
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
        self.state_bytes = self
            .state_bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn issue(&mut self, path: &str, code: &'static str) -> Result<(), ItemRefusal> {
        if self.issues.len() >= self.limits.max_issues {
            return Err(ItemRefusal::Budget);
        }
        self.reserve(path.len() + code.len())?;
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
        let raw = source.metadata(path, self.limits.max_member_bytes, self.limits.deadline)?;
        self.check()?;
        if let Some(raw) = &raw {
            if raw.len() > self.limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
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
    // declared-record parser. Exact raw bytes still bind schema and fixity.
    fn object(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        contract: &str,
    ) -> Result<Option<(Value, Vec<u8>)>, ItemRefusal> {
        let Some(raw) = self.raw(source, path)? else {
            return Ok(None);
        };
        let Ok(value) = serde_json::from_slice::<Value>(&raw) else {
            self.issue(path, "invalid-json")?;
            return Ok(None);
        };
        if !value.is_object() {
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
        let mut refs: Vec<&Value> = ["source_refs", "source_record_refs", "receipt_refs"]
            .into_iter()
            .flat_map(|field| {
                value
                    .get(field)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .collect();
        for field in [
            "rights_ref",
            "provenance_ref",
            "forensic_report_ref",
            "resource_inventory_ref",
            "item_manifest_ref",
            "generated_from_manifest_ref",
        ] {
            if let Some(target) = value.get(field) {
                refs.push(target);
            }
        }
        for target in refs {
            let Some(target) = target.as_str() else {
                self.issue(path, "unresolved-source-ref")?;
                continue;
            };
            if target.starts_with("ToS/") {
                safe_path(target)?;
                if !source.exists(target, self.limits.deadline)? {
                    self.issue(path, "unresolved-source-ref")?;
                }
                self.check()?;
            }
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
            if source.record_kind(id, self.limits.deadline)?.as_deref() != Some(kind) {
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
        if !path.starts_with("ToS/source-witnesses/") || !path.ends_with("/item.manifest.json") {
            return Err(ItemRefusal::Unsupported("item-manifest-owner-path".into()));
        }
        let Some((manifest, _)) = self.object(source, path, MANIFEST)? else {
            return Ok(());
        };
        let directory = path.rsplit_once('/').unwrap().0;
        let item_id = &manifest["item_id"];
        self.require_kind(source, path, item_id, "item")?;
        self.require_kind(source, path, &manifest["embodiment_ref"], "edition")?;
        if let Some(id) = item_id.as_str() {
            if !self.manifest_item_ids.contains(id) {
                self.reserve(id.len())?;
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
            let expected: Vec<Value> = array(&manifest["payload_files"]).filter(|v| v.is_object()).map(|entry|
                json!({"file_id":entry["file_id"],"file_sha256":entry["sha256"],"media_type":entry["media_type"]})).collect();
            let actual: Vec<Value> = array(&inventory["files"]).filter(|v| v.is_object()).map(|entry|
                json!({"file_id":entry["file_id"],"file_sha256":entry["file_sha256"],"media_type":entry["media_type"]})).collect();
            if actual != expected {
                self.issue(inventory_path, "inventory-file-identity")?;
            }
            for entry in array(&inventory["files"]).filter(|v| v.is_object()) {
                self.check()?;
                let resources: Vec<&Value> = array(&entry["resources"]).collect();
                let mut ids = BTreeSet::new();
                for resource in resources.iter().filter(|v| v.is_object()) {
                    self.check()?;
                    if !ids.insert(resource["resource_id"].to_string()) {
                        self.issue(inventory_path, "duplicate-resource-id")?;
                    }
                }
                if entry["summary"].is_object()
                    && entry["summary"]["resource_count"].as_u64() != Some(resources.len() as u64)
                {
                    self.issue(inventory_path, "resource-count")?;
                }
            }
        }
        let rights_path = manifest["rights_ref"].as_str().unwrap_or("");
        let rights = if rights_path.is_empty() {
            None
        } else {
            self.object(source, rights_path, RIGHTS)?
        };
        if let Some((rights, _)) = &rights {
            if rights["scope_refs"].is_array()
                && !array(&rights["scope_refs"]).any(|scope| scope == item_id)
            {
                self.issue(rights_path, "rights-item-scope")?;
            }
            if rights["visibility"] != manifest["visibility"] {
                self.issue(rights_path, "rights-manifest-visibility")?;
            }
            let mut layer_ids = BTreeSet::new();
            for layer in array(&rights["layer_assessments"]).filter(|v| v.is_object()) {
                if let Some(id) = layer["layer_id"].as_str() {
                    if !layer_ids.insert(id) {
                        self.issue(rights_path, "duplicate-rights-layer-id")?;
                    }
                }
                self.source_refs(source, rights_path, layer)?;
            }
        }
        let provenance_path = manifest["provenance_ref"].as_str().unwrap_or("");
        let mut events = BTreeMap::new();
        if !provenance_path.is_empty() {
            if let Some(raw) = self.raw(source, provenance_path)? {
                let Ok(text) = std::str::from_utf8(&raw) else {
                    self.issue(provenance_path, "invalid-jsonl-utf8")?;
                    return Ok(());
                };
                // splitlines, like Python, does not turn the terminal LF into
                // a fictitious blank record. Interior blank lines do reject.
                for (i, line) in text.lines().enumerate() {
                    self.check()?;
                    let location = format!("{provenance_path}:{}", i + 1);
                    if line.trim().is_empty() {
                        self.issue(&location, "blank-jsonl-line")?;
                        continue;
                    }
                    let Ok(event) = serde_json::from_str::<Value>(line) else {
                        self.issue(&location, "invalid-jsonl")?;
                        continue;
                    };
                    if !event.is_object() {
                        self.issue(&location, "object-required")?;
                        continue;
                    }
                    if !source.schema(&location, line.as_bytes(), EVENT, self.limits.deadline)? {
                        self.issue(&location, "schema")?;
                    }
                    self.source_refs(source, &location, &event)?;
                    if let Some(id) = event["event_id"].as_str() {
                        if self.event_ids.contains(id) {
                            self.issue(&location, "duplicate-event-id")?;
                        } else {
                            self.reserve(id.len())?;
                            self.event_ids.insert(id.into());
                        }
                        events.insert(id.to_owned(), event);
                    }
                }
            }
        }
        if !manifest["acquisition_event_ref"]
            .as_str()
            .is_some_and(|id| events.contains_key(id))
        {
            self.issue(path, "acquisition-event-not-local")?;
        }
        if let Some((inventory, raw)) = &inventory {
            if let Some(event) = inventory["provenance_event_ref"]
                .as_str()
                .and_then(|id| events.get(id))
            {
                let expected = json!({"ref":inventory_path,"role":"tracked_text_free_resource_inventory","sha256":Digest256::of_bytes(raw).to_hex()});
                if !array(&event["outputs"]).any(|output| output == &expected) {
                    self.issue(inventory_path, "inventory-provenance-output")?;
                }
            } else {
                self.issue(inventory_path, "inventory-event-not-local")?;
            }
        }
        let mut expected_fixity = String::new();
        for entry in array(&manifest["payload_files"]).filter(|v| v.is_object()) {
            self.check()?;
            let sha = entry["sha256"].as_str().unwrap_or("None");
            let relative = entry["relative_path"].as_str().unwrap_or("None");
            expected_fixity.push_str(&format!("{sha}  {relative}\n"));
            if let (Some(file_id), Some(_)) = (entry["file_id"].as_str(), item_id.as_str()) {
                let descriptor = json!([entry["sha256"], entry["byte_size"], entry["media_type"]]);
                if self
                    .file_descriptors
                    .get(file_id)
                    .is_some_and(|previous| previous != &descriptor)
                {
                    self.issue(path, "file-identity-conflict")?;
                } else if !self.file_descriptors.contains_key(file_id) {
                    self.reserve(file_id.len() + descriptor.to_string().len())?;
                    self.file_descriptors.insert(file_id.into(), descriptor);
                }
                if file_id != format!("tos.file.sha256.{sha}") {
                    self.issue(path, "file-id-sha256")?;
                }
                if let Some((rights, _)) = &rights {
                    if rights["scope_refs"].is_array()
                        && !array(&rights["scope_refs"]).any(|scope| scope == file_id)
                    {
                        self.issue(rights_path, "rights-file-scope")?;
                    }
                }
            }
            if entry["relative_path"].as_str().is_none() {
                self.issue(path, "payload-path-string")?;
                continue;
            }
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
        }
        let fixity_path = format!("{directory}/fixity.sha256");
        if let Some(raw) = self.raw(source, &fixity_path)? {
            // Preserve the owner legacy conditional: an existing empty
            // companion is not compared here; schema/owner review may change
            // that separately. Missing still emits missing-companion.
            if !raw.is_empty() && raw != expected_fixity.as_bytes() {
                self.issue(&fixity_path, "fixity-manifest-drift")?;
            }
        }
        Ok(())
    }

    /// Invoke for every current native Item record after manifest traversal.
    pub fn inspect_item_record(
        &mut self,
        source: &mut impl ItemSource,
        path: &str,
        raw: &[u8],
    ) -> Result<(), ItemRefusal> {
        self.check()?;
        let item: Value = serde_json::from_slice(raw)
            .map_err(|_| ItemRefusal::Source("item-record-json".into()))?;
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
        fn record_kind(&mut self, id: &str, _: Instant) -> Result<Option<String>, ItemRefusal> {
            Ok(self.kinds.get(id).cloned())
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
