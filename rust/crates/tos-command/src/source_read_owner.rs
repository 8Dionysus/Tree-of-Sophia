//! Maintained exact-source owner. Read-only selection carries no writer grant.
use crate::source_claim_publication_bytes as bytes;
use crate::source_claim_publication_roots::{MutationLimits, Roots, Snapshot};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_creation_store::source_read_filesystem::SourceReadFilesystem;
use crate::source_read_contract as wire;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};
use tos_compiler::prepared_source_binding::PreparedSourceInputs;
use tos_foundation::{Digest256, JsonLimits, JsonMode, JsonValue, SourceRevision};
pub use tos_validation::executor::ExactWorkerIdentity;
use tos_validation::{
    FormatProfile, SchemaResource,
    executor::ExecutorBudget,
    source_cut::{CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor},
};
#[derive(Clone, Copy)]
pub enum SourceReadOperation {
    Capabilities,
    Contract,
    Discover,
    Read,
}
pub struct SelectedSourceReadOwner {
    root: PathBuf,
    inputs: Arc<[u8]>,
    revision: String,
    inputs_sha256: String,
    worker: ExactWorkerIdentity,
    local: Option<PathBuf>,
    slots: Arc<AtomicUsize>,
}
struct Slot(Arc<AtomicUsize>);
impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
pub struct SourceReadPacket {
    body: Vec<u8>,
    fs: SourceReadFilesystem,
    roots: Roots,
    local_hold: Option<crate::source_native_text_read::LocalTextReadSelection>,
    _slot: Slot,
}
impl SourceReadPacket {
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn verify_current(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        self.roots
            .verify_retained_reads(deadline, cancelled)
            .map_err(project)?;
        if let Some(selection) = &self.local_hold {
            selection.verify(deadline, cancelled)?;
        }
        self.fs.verify_current(deadline, cancelled)
    }
}
fn project(_: tos_compiler::Error) -> Error {
    Error::Conflict("selected source projection refused")
}
fn typed(v: &Value) -> Result<JsonValue> {
    let raw = wire::canonical(v, wire::RESPONSE_BYTES)?;
    tos_foundation::parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(wire::RESPONSE_BYTES, 128, 1_000_000, 4096)
            .map_err(|_| Error::Invalid("source JSON limits"))?,
    )
    .map(|document| document.into_root())
    .map_err(|_| Error::Invalid("source JSON value"))
}
fn plain(v: &JsonValue) -> Result<Value> {
    let raw = tos_foundation::canonical_bytes_v1(
        v,
        tos_foundation::CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(wire::RESPONSE_BYTES, 128, 1_000_000, 4096)
            .map_err(|_| Error::Invalid("source JSON limits"))?,
    )
    .map_err(|_| Error::Invalid("source JSON output"))?;
    bytes::parse(&raw, wire::RESPONSE_BYTES).map_err(project)
}
impl SelectedSourceReadOwner {
    /// Canonical PreparedSourceInputs SHA-256, lowercase bare hex; whitespace-independent.
    pub fn source_inputs_sha256(&self) -> &str {
        &self.inputs_sha256
    }
    pub fn open(
        source_root: &Path,
        inputs_raw: &[u8],
        expected_revision: &str,
        worker: ExactWorkerIdentity,
        local_selection: Option<&Path>,
    ) -> Result<Self> {
        if !source_root.is_absolute()
            || !worker.absolute_path.is_absolute()
            || local_selection.is_some_and(|p| !p.is_absolute())
        {
            return Err(Error::Invalid("explicit source owner selection"));
        }
        let input = PreparedSourceInputs::parse(inputs_raw, Default::default()).map_err(project)?;
        if input.source_revision() != expected_revision {
            return Err(Error::Conflict(
                "source vector and selected prepared revision differ",
            ));
        }
        crate::source_agent_publication::require_vector(&input, 16_777_216).map_err(project)?;
        Ok(Self {
            root: source_root.to_owned(),
            inputs: Arc::from(inputs_raw),
            revision: expected_revision.to_owned(),
            inputs_sha256: input.digest().to_owned(),
            worker,
            local: local_selection.map(Path::to_owned),
            slots: Arc::new(AtomicUsize::new(0)),
        })
    }
    pub fn prepare(
        &self,
        operation: SourceReadOperation,
        request: &[u8],
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<SourceReadPacket> {
        self.slots
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 2).then_some(n + 1)
            })
            .map_err(|_| Error::Unsupported("source reader concurrency budget exhausted"))?;
        let slot = Slot(self.slots.clone());
        let mut fs =
            SourceReadFilesystem::open(&self.root, 4096, 67_108_864, deadline, &cancelled)?;
        let inputs =
            PreparedSourceInputs::parse(&self.inputs, Default::default()).map_err(project)?;
        // PreparedSourceInputs authenticates all retained root bytes/digests.
        // Namespace paths locate immutable addressed parts; they do not select
        // mutable projection pointer files or promise their continued presence.
        let full = crate::source_agent_publication::require_vector(&inputs, 16_777_216)
            .map_err(project)?;
        let catalog = inputs
            .roots()
            .get("source-catalog")
            .ok_or(Error::Invalid("selected source catalog absent"))?
            .clone();
        let snapshot = Snapshot::parse(
            catalog.root_bytes.clone(),
            catalog.namespace_path.clone().into(),
            &catalog.snapshot_sha256,
        )
        .map_err(project)?;
        let header = snapshot.manifest["header"].clone();
        let epoch = json!({"source_revision":self.revision,"catalog_root_sha256":catalog.snapshot_sha256,"catalog_namespace":header["catalog_namespace"],"source_publication":header["source_publication"]});
        wire::epoch(&epoch)?;
        if header["schema_version"] != "tos_source_catalog_projection_v2" {
            return Err(Error::Invalid("selected source catalog schema differs"));
        }
        if full["source_publication"] != epoch["source_publication"]["token"] {
            return Err(Error::Conflict(
                "selected source vector catalog publication differs",
            ));
        }
        let (token, generation) = fs.publication();
        if epoch["source_publication"]["token"].as_str() != token
            || epoch["source_publication"]["generation"].as_u64() != Some(generation)
        {
            return Err(Error::Conflict(
                "source publication differs from selected catalog",
            ));
        }
        let mut snapshots = BTreeMap::from([("source-catalog".to_owned(), snapshot)]);
        if let Some(root) = inputs.roots().get("authored-corpus") {
            snapshots.insert(
                "authored-corpus".to_owned(),
                Snapshot::parse(
                    root.root_bytes.clone(),
                    root.namespace_path.clone().into(),
                    &root.snapshot_sha256,
                )
                .map_err(project)?,
            );
        }
        let mut roots =
            Roots::new_retained(snapshots, MutationLimits::default()).map_err(project)?;
        let mut local_hold = None;
        let response = match operation {
            SourceReadOperation::Capabilities => {
                let mut representations = vec!["record", "native_public_unit"];
                if let Some(path) = &self.local {
                    let mut worker = self.worker(&mut fs, deadline, &cancelled)?;
                    let local = crate::source_native_text_read::LocalTextReadSelection::load(
                        path,
                        &self.root,
                        &mut worker,
                        deadline,
                        &cancelled,
                    );
                    worker.finish(deadline, &cancelled).map_err(|reason| {
                        Error::SchemaExecution {
                            path: "source.capabilities".into(),
                            root: "selected-schema".into(),
                            reason,
                        }
                    })?;
                    if let Ok(selection) = local {
                        if selection.verify(deadline, &cancelled).is_ok() {
                            representations.push("native_local_unit");
                            local_hold = Some(selection);
                        }
                    }
                }
                json!({"schema_version":"tos_source_read_capabilities_v1","available":true,"issuer":wire::ISSUER,"layers":{"metadata_record":header["record_families"],"claim_record":header.get("claim_count").is_some(),"source_slot":header.get("source_slot_count").is_some(),"authored_csv_record":inputs.roots().contains_key("authored-corpus")},"source_epoch":epoch,"binding":{"kind":"prepared-source-vector","catalog_root_sha256":catalog.snapshot_sha256,"source_inputs_sha256":inputs.digest()},"representations":representations,"limits":{"max_handle_bytes":wire::HANDLE_BYTES,"max_request_bytes":wire::REQUEST_BYTES,"max_record_bytes":wire::RECORD_BYTES,"max_response_bytes":wire::RESPONSE_BYTES},"authority":{"is_source":false,"writes_to_source":false,"grants_current_use":false,"native_text_payload":true,"note":"Metadata handles select records, not text permissions. native_public_unit requires unconditional public rights; native_local_unit additionally needs an explicitly selected current owner condition review and preserved notices. Neither route reads private payloads or authorizes external publication."}})
            }
            SourceReadOperation::Contract => contract(),
            _ => {
                let req = bytes::parse(request, wire::REQUEST_BYTES).map_err(project)?;
                self.dispatch(
                    operation,
                    &req,
                    &epoch,
                    &full,
                    &mut fs,
                    &mut roots,
                    &mut local_hold,
                    deadline,
                    &cancelled,
                )?
            }
        };
        let body = wire::canonical(&response, wire::RESPONSE_BYTES)?;
        let mut packet = SourceReadPacket {
            body,
            fs,
            roots,
            local_hold,
            _slot: slot,
        };
        packet.verify_current(deadline, &cancelled)?;
        Ok(packet)
    }
}
impl SelectedSourceReadOwner {
    fn dispatch(
        &self,
        op: SourceReadOperation,
        request: &Value,
        epoch: &Value,
        full: &Value,
        fs: &mut SourceReadFilesystem,
        roots: &mut Roots,
        local_hold: &mut Option<crate::source_native_text_read::LocalTextReadSelection>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        let (target, handle, representation) = match op {
            SourceReadOperation::Read => {
                wire::exact(request, &["handle", "representation"])?;
                wire::validate_handle(&request["handle"])?;
                let representation = wire::text(request, "representation")?;
                if !["record", "native_public_unit", "native_local_unit"].contains(&representation)
                {
                    return Err(Error::Invalid("source representation"));
                }
                (
                    request["handle"]["target"].clone(),
                    Some(request["handle"].clone()),
                    representation,
                )
            }
            SourceReadOperation::Discover => {
                let map = request
                    .as_object()
                    .ok_or(Error::Invalid("source discover request"))?;
                if map.len() != 1 {
                    return Err(Error::Invalid("source discover request"));
                }
                if let Some(t) = map.get("target") {
                    wire::target(t)?;
                    (t.clone(), None, "record")
                } else {
                    let selector = map
                        .get("selector")
                        .ok_or(Error::Invalid("source discover selector"))?;
                    match self.issue_target(selector, roots)? {
                        Some(t) => (t, None, "record"),
                        None => {
                            return Ok(
                                json!({"schema_version":"tos_source_handle_discovery_v1","status":"missing","reason":"owner-source-record-missing","target":null,"handle":null,"source_revision":self.revision,"content_revision":null,"provenance":null,"access":null,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false}),
                            );
                        }
                    }
                }
            }
            _ => return Err(Error::Invalid("source read operation")),
        };
        wire::target(&target)?;
        let discovery = handle.is_none();
        let mut response = if discovery {
            json!({"schema_version":"tos_source_handle_discovery_v1","status":"unsupported","reason":"owner-reader-not-configured","target":target,"handle":null,"source_revision":self.revision,"content_revision":target["content_revision"],"provenance":null,"access":null,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false})
        } else {
            json!({"schema_version":"tos_source_read_result_v1","status":"unsupported","reason":"owner-reader-not-configured","handle":handle,"source_revision":self.revision,"content_revision":target["content_revision"],"layer":target["layer"],"record_kind":match target["layer"].as_str(){Some("metadata_record")=>"metadata",Some("claim_record")=>"claim",Some("authored_csv_record")=>"authored_csv",_=>"source_slot"},"record_ref":target.get("record_ref"),"record":null,"provenance":null,"access":handle.as_ref().map(|h|&h["access"]),"grants_current_use":false,"performs_assessment":false,"writes_to_source":false})
        };
        if representation != "record" {
            response["schema_version"] = json!("tos_source_native_unit_read_result_v1");
            response["native_unit"] = Value::Null;
            response["text_access"] = Value::Null;
        }
        if handle.as_ref().is_some_and(|h| h["epoch"] != *epoch) {
            if representation != "record" {
                response["schema_version"] = json!("tos_source_native_unit_read_result_v1");
                response["native_unit"] = Value::Null;
                response["text_access"] = Value::Null;
            }
            response["status"] = json!("stale");
            response["reason"] = json!("source-epoch-differs");
            return Ok(response);
        }
        let selected = self.read_target(&target, epoch, full, fs, roots, deadline, cancelled);
        let (record, provenance, access, mut selected_worker) = match selected {
            Ok(v) => v,
            Err(error) => {
                let (status, reason) = closed_error(&error);
                response["status"] = json!(status);
                response["reason"] = json!(reason);
                response["access"] = Value::Null;
                return Ok(response);
            }
        };
        response["status"] = json!("available");
        response["provenance"] = provenance;
        if discovery && target["layer"] == "authored_csv_record" {
            if let Some(object) = response["provenance"].as_object_mut() {
                object.remove("raw_record");
            }
        }
        response["access"] = access.clone();
        if discovery {
            response["reason"] = json!("owner-issued-exact-source-handle");
            response["handle"] = wire::make_handle(epoch, &target, &access)?;
        } else {
            response["reason"] = json!("exact-owner-source-record");
            response["record"] = record.clone();
        }
        if representation != "record" {
            response["schema_version"] = json!("tos_source_native_unit_read_result_v1");
            response["record"] = Value::Null;
            response["native_unit"] = Value::Null;
            response["text_access"] = Value::Null;
            let Some(binding) = record
                .get("native_text_binding")
                .filter(|b| b.is_object() && target["layer"] == "metadata_record")
            else {
                response["status"] = json!("unsupported");
                response["reason"] = json!("no-exact-native-unit-binding");
                if let Some(mut worker) = selected_worker.take() {
                    worker.finish(deadline, cancelled).map_err(|reason| {
                        Error::SchemaExecution {
                            path: "source.read".into(),
                            root: "selected-schema".into(),
                            reason,
                        }
                    })?;
                }
                return Ok(response);
            };
            let local = representation == "native_local_unit";
            if local && self.local.is_none() {
                response["status"] = json!("unsupported");
                response["reason"] = json!("native-local-unit-owner-unconfigured");
                if let Some(mut worker) = selected_worker.take() {
                    worker.finish(deadline, cancelled).map_err(|reason| {
                        Error::SchemaExecution {
                            path: "source.read".into(),
                            root: "selected-schema".into(),
                            reason,
                        }
                    })?;
                }
                return Ok(response);
            }
            let mut worker = selected_worker
                .take()
                .ok_or(Error::Invalid("native text requires metadata worker"))?;
            if local {
                match crate::source_native_text_read::LocalTextReadSelection::load(
                    self.local.as_ref().unwrap(),
                    &self.root,
                    &mut worker,
                    deadline,
                    cancelled,
                ) {
                    Ok(selection) => *local_hold = Some(selection),
                    Err(_) => {
                        worker.finish(deadline, cancelled).map_err(|reason| {
                            Error::SchemaExecution {
                                path: "source.read".into(),
                                root: "selected-schema".into(),
                                reason,
                            }
                        })?;
                        response["status"] = json!("access-restricted");
                        response["reason"] = json!("native-unit-local-conditions-not-satisfied");
                        return Ok(response);
                    }
                }
            }
            let result = crate::source_native_text_read::read_native_unit(
                fs,
                &mut worker,
                &typed(binding)?,
                local_hold.as_ref(),
                65_536,
                deadline,
                cancelled,
            );
            worker
                .finish(deadline, cancelled)
                .map_err(|reason| Error::SchemaExecution {
                    path: "source.read".into(),
                    root: "native-text".into(),
                    reason,
                })?;
            match result {
                Ok(unit) => {
                    response["native_unit"] = plain(&unit)?;
                    response["reason"] = json!(if local {
                        "exact-owner-local-native-unit"
                    } else {
                        "exact-owner-public-native-unit"
                    });
                    response["text_access"] = if local {
                        json!({"scope":"local-native-unit","recorded_rights_verified":true,"conditional_rights":true,"grants_current_use":false,"external_publication_authorized":false})
                    } else {
                        json!({"scope":"public-native-unit","recorded_rights_verified":true,"conditional_rights":false,"grants_current_use":false})
                    };
                }
                Err(error) => {
                    response["status"] = json!(match error {
                        Error::Denied(_) => "access-restricted",
                        Error::Unsupported(_) => "over-budget",
                        _ => "corrupt",
                    });
                    response["reason"] = json!(if matches!(error, Error::Denied(_)) {
                        if local {
                            "native-unit-local-conditions-not-satisfied"
                        } else {
                            "native-unit-public-rights-not-satisfied"
                        }
                    } else if matches!(error, Error::Unsupported(_)) {
                        "native-unit-read-budget"
                    } else {
                        "native-unit-closure-not-verified"
                    });
                }
            }
        }
        if let Some(mut worker) = selected_worker {
            worker
                .finish(deadline, cancelled)
                .map_err(|reason| Error::SchemaExecution {
                    path: "source.read".into(),
                    root: "selected-schema".into(),
                    reason,
                })?;
        }
        Ok(response)
    }
    fn issue_target(&self, selector: &Value, roots: &mut Roots) -> Result<Option<Value>> {
        let layer = wire::text(selector, "layer")?;
        let target = match layer {
            "metadata_record" => {
                wire::exact(selector, &["layer", "record_type", "record_id"])?;
                let id = wire::text(selector, "record_id")?;
                let Some(row) = roots
                    .get("source-catalog", "records", id)
                    .map_err(project)?
                else {
                    return Ok(None);
                };
                if row["entry"]["record_type"] != selector["record_type"] {
                    return Err(Error::Conflict("source metadata kind differs"));
                }
                json!({"layer":layer,"record_type":selector["record_type"],"record_ref":row["source"]["record_ref"],"content_revision":row["source"]["record_ref"]["digest"]})
            }
            "claim_record" => {
                wire::exact(selector, &["layer", "claim_id"])?;
                let Some(row) = roots
                    .get(
                        "source-catalog",
                        "claims",
                        wire::text(selector, "claim_id")?,
                    )
                    .map_err(project)?
                else {
                    return Ok(None);
                };
                json!({"layer":layer,"record_ref":row["claim_ref"],"content_revision":row["claim_ref"]["digest"]})
            }
            "source_slot" => {
                wire::exact(selector, &["layer", "slot_kind", "identity"])?;
                let key = slot_key(
                    wire::text(selector, "slot_kind")?,
                    wire::text(selector, "identity")?,
                )?;
                let Some(row) = roots
                    .get("source-catalog", "source_slots", &key)
                    .map_err(project)?
                else {
                    return Ok(None);
                };
                json!({"layer":layer,"slot_kind":selector["slot_kind"],"identity":selector["identity"],"row_sha256":bytes::row_digest(&row,wire::RECORD_BYTES).map_err(project)?,"content_revision":format!("sha256:{}",wire::text(&row["source"],"canonical_sha256")?)})
            }
            "authored_csv_record" => {
                wire::exact(selector, &["layer", "pack_id", "edge_id"])?;
                let key = String::from_utf8(wire::canonical(
                    &json!([selector["pack_id"], selector["edge_id"]]),
                    4096,
                )?)
                .map_err(|_| Error::Invalid("CSV key"))?;
                let Some(row) = roots
                    .get("authored-corpus", "relations", &key)
                    .map_err(project)?
                else {
                    return Ok(None);
                };
                row["target"].clone()
            }
            _ => return Err(Error::Invalid("source selector layer")),
        };
        wire::target(&target)?;
        Ok(Some(target))
    }
    fn worker(
        &self,
        fs: &mut SourceReadFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<CutWorkerSchemaExecutor> {
        let mut contracts = BTreeMap::new();
        let mut resources = Vec::new();
        let mut total = 0usize;
        for (name, directory) in fs.list_directory("ToS/contracts", deadline, cancelled)? {
            if directory || !name.ends_with(".schema.json") {
                continue;
            }
            if resources.len() >= 512 {
                return Err(Error::Unsupported("source schema resource budget"));
            }
            let path = format!("ToS/contracts/{name}");
            let raw = fs.read(&path, 1_048_576, deadline, cancelled)?;
            total = total
                .checked_add(raw.len())
                .filter(|n| *n <= 8_388_608)
                .ok_or(Error::Unsupported("source schema byte budget"))?;
            let value = bytes::parse(&raw, 1_048_576).map_err(project)?;
            let uri = wire::text(&value, "$id")?.to_owned();
            contracts.insert(path, (uri.clone(), Digest256::of_bytes(&raw)));
            resources.push(SchemaResource { uri, raw });
        }
        let revision = SourceRevision(
            Digest256::from_hex(&self.revision).map_err(|_| Error::Invalid("source revision"))?,
        );
        let mut budget = ExecutorBudget::laboratory();
        budget.execution_wall = budget
            .execution_wall
            .min(deadline.saturating_duration_since(Instant::now()));
        CutWorkerSchemaExecutor::from_selected_resources(
            revision,
            contracts,
            resources,
            FormatProfile::LegacyPythonObserved20260923,
            self.worker.clone(),
            budget,
            CutWorkerLimits {
                max_receipts: 128,
                max_receipt_bytes: 2_097_152,
            },
            deadline,
            cancelled,
        )
        .map_err(|reason| Error::SchemaExecution {
            path: "source.read".into(),
            root: "selected-schema".into(),
            reason,
        })
    }
    fn read_target(
        &self,
        target: &Value,
        epoch: &Value,
        full: &Value,
        fs: &mut SourceReadFilesystem,
        roots: &mut Roots,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(Value, Value, Value, Option<CutWorkerSchemaExecutor>)> {
        let context = crate::source_read_layers::LayerContext {
            source_root: &self.root,
            full_revision: full,
            catalog_namespace: wire::text(epoch, "catalog_namespace")?,
            max_record_bytes: wire::RECORD_BYTES,
            deadline,
            cancelled,
        };
        match wire::text(target, "layer")? {
            "metadata_record" => {
                let row = roots
                    .get(
                        "source-catalog",
                        "records",
                        wire::text(&target["record_ref"], "id")?,
                    )
                    .map_err(project)?
                    .ok_or(Error::Unsupported("owner-source-record-missing"))?;
                if row["record_id"] != target["record_ref"]["id"]
                    || row["entry"]["record_type"] != target["record_type"]
                    || row["entry"]["source_record_ref"] != row["source"]["source_ref"]
                {
                    return Err(Error::Conflict("source catalog identities differ"));
                }
                let path = wire::text(&row["source"], "source_ref")?;
                let files = crate::source_revisions::collect_readonly_record_files(
                    fs, path, deadline, cancelled,
                )?;
                let current = files
                    .iter()
                    .find(|f| f.path.as_str() == path)
                    .ok_or(Error::Invalid("selected record absent"))?;
                if Digest256::of_bytes(&current.raw).to_hex()
                    != wire::text(&row["source"], "raw_sha256")?
                    || current.raw.len() as u64
                        != row["source"]["raw_bytes"]
                            .as_u64()
                            .ok_or(Error::Invalid("source record size"))?
                {
                    return Err(Error::Conflict("catalog current record bytes differ"));
                }
                let mut worker = self.worker(fs, deadline, cancelled)?;
                let input = crate::source_revisions::RecordVersionReadInput {
                    files: &files,
                    source_revision: SourceRevision(
                        Digest256::from_hex(&self.revision)
                            .map_err(|_| Error::Invalid("source revision"))?,
                    ),
                    effective_uid: fs.effective_uid() as u64,
                    schema_source_path: path,
                };
                let limits = tos_validation::item_rules::ItemLimits {
                    max_member_bytes: 8_388_608,
                    max_total_bytes: 67_108_864,
                    max_state_bytes: wire::RESPONSE_BYTES,
                    max_issues: 128,
                    deadline,
                };
                let result = crate::source_revisions::resolve_record_version_readonly(
                    &input,
                    path,
                    limits,
                    &typed(&target["record_ref"])?,
                    &mut worker,
                    deadline,
                    cancelled,
                )?;
                let record = plain(&result.record)?;
                let descriptor = plain(
                    result
                        .descriptor
                        .as_ref()
                        .ok_or(Error::Invalid("source descriptor absent"))?,
                )?;
                let visibility = wire::text(&descriptor, "source_scope")?;
                if record
                    .get("visibility")
                    .is_some_and(|v| v.as_str() != Some(visibility))
                {
                    return Err(Error::Denied(
                        "owner metadata descriptor visibility differs",
                    ));
                }
                wire::canonical(&record, wire::RECORD_BYTES)?;
                if descriptor["record_type"] != target["record_type"] {
                    return Err(Error::Conflict("owner metadata descriptor kind differs"));
                }
                let mut catalog = row["source"].clone();
                catalog["schema_version"] = json!("tos_source_catalog_address_v2");
                catalog["catalog_namespace"] = epoch["catalog_namespace"].clone();
                catalog["profile_id"] = json!("tos.source-catalog.public-records.v2");
                catalog["record_key"] = row["record_id"].clone();
                catalog["row_sha256"] =
                    json!(bytes::row_digest(&row, wire::RECORD_BYTES).map_err(project)?);
                let provenance = plain(&result.readonly_provenance(&typed(&catalog)?)?)?;
                Ok((
                    record,
                    provenance,
                    wire::access("public-metadata-record", visibility)?,
                    Some(worker),
                ))
            }
            "claim_record" => {
                let claim = roots
                    .get(
                        "source-catalog",
                        "claims",
                        wire::text(&target["record_ref"], "id")?,
                    )
                    .map_err(project)?
                    .ok_or(Error::Unsupported("owner-source-record-missing"))?;
                let slot = roots
                    .get(
                        "source-catalog",
                        "source_slots",
                        wire::text(&claim, "source_slot_key")?,
                    )
                    .map_err(project)?
                    .ok_or(Error::Invalid("Claim slot absent"))?;
                let read = crate::source_read_layers::read_current_claim(
                    fs,
                    &context,
                    &claim,
                    &slot,
                    &target["record_ref"],
                    &bytes::row_digest(&claim, wire::RECORD_BYTES).map_err(project)?,
                )?;
                let access = wire::access(
                    "public-claim-record",
                    wire::text(&read.record, "visibility")?,
                )?;
                Ok((read.record, read.provenance, access, None))
            }
            "source_slot" => {
                let kind = wire::text(target, "slot_kind")?;
                let id = wire::text(target, "identity")?;
                let slot = roots
                    .get("source-catalog", "source_slots", &slot_key(kind, id)?)
                    .map_err(project)?
                    .ok_or(Error::Unsupported("owner-source-record-missing"))?;
                let read = crate::source_read_layers::read_source_slot(
                    fs,
                    &context,
                    &slot,
                    kind,
                    id,
                    wire::text(target, "row_sha256")?,
                    wire::text(target, "content_revision")?,
                )?;
                // read_source_slot has authenticated the exact bytes and the
                // kind's declared publication policy. The slot binding has no
                // visibility field; unlabelled event/anchor slots are metadata.
                let visibility = match read.record.get("visibility") {
                    Some(Value::String(value)) => value.as_str(),
                    None if kind != "claim" => "public_metadata_only",
                    _ => return Err(Error::Denied("source slot payload visibility")),
                };
                let access = wire::access("public-source-slot-metadata", visibility)?;
                Ok((read.record, read.provenance, access, None))
            }
            "authored_csv_record" => {
                let key = String::from_utf8(wire::canonical(
                    &json!([target["pack_id"], target["edge_id"]]),
                    4096,
                )?)
                .map_err(|_| Error::Invalid("CSV key"))?;
                let row = roots
                    .get("authored-corpus", "relations", &key)
                    .map_err(project)?
                    .ok_or(Error::Unsupported("owner-source-record-missing"))?;
                let root = full["roots"]["authored-corpus"]["snapshot_sha256"]
                    .as_str()
                    .ok_or(Error::Invalid("authored root absent"))?;
                let read =
                    crate::source_read_layers::read_authored_csv(fs, &context, &row, target, root)?;
                Ok((
                    read.record,
                    read.provenance,
                    wire::access("public-authored-csv-record", "public_metadata_only")?,
                    None,
                ))
            }
            _ => Err(Error::Invalid("source target layer")),
        }
    }
}
fn closed_error(error: &Error) -> (&'static str, &'static str) {
    match error {
        Error::Conflict("source slot catalog row differs") => {
            ("stale", "source-slot-descriptor-differs")
        }
        Error::Conflict("Claim catalog exact ref differs") => {
            ("stale", "exact-version-digest-mismatch")
        }
        Error::Denied(_) => ("access-restricted", "owner-source-path-restricted"),
        Error::Unsupported("owner-source-record-missing") => {
            ("missing", "owner-source-record-missing")
        }
        Error::Unsupported("exact-version-not-retained") => {
            ("missing", "exact-version-not-retained")
        }
        Error::Conflict("exact-version-digest-mismatch") => {
            ("stale", "exact-version-digest-mismatch")
        }
        Error::Invalid(reason) if reason.contains("budget") => {
            ("over-budget", "source-read-budget")
        }
        Error::SchemaExecution {
            reason: tos_validation::item_rules::ItemRefusal::Budget,
            ..
        } => ("over-budget", "source-read-budget"),
        Error::SchemaExecution {
            reason: tos_validation::item_rules::ItemRefusal::BudgetCheck { .. },
            ..
        } => ("over-budget", "source-read-budget"),
        Error::Unsupported(_) => ("over-budget", "source-read-budget"),
        _ => ("corrupt", "owner-source-record-not-verified"),
    }
}
fn contract() -> Value {
    json!({"schema_version":"tos_source_read_contract_descriptor_v1","handle_schema":"tos_source_read_handle_v1","discovery_schema":"tos_source_handle_discovery_v1","result_schema":"tos_source_read_result_v1","supported_layers":["authored_csv_record","claim_record","metadata_record","source_slot"],"representations":["record","native_public_unit","native_local_unit"],"native_unit_result_schema":"tos_source_native_unit_read_result_v1","native_unit_owner_required":true,"discovery_inputs":["owner_target","typed_catalog_selector"],"statuses":["access-restricted","available","corrupt","missing","over-budget","stale","unsupported"],"authority":{"is_source":false,"writes_to_source":false,"grants_current_use":false,"native_text_payload":false,"path_or_range_requests":false,"latest_fallback":false,"target_origin":"owner-card-or-typed-catalog-resolver","rights_scope":"metadata-disclosure-only","native_unit_rights_scope":"separate-unconditional-public-owner-gate","native_local_unit_rights_scope":"separate-current-owner-condition-selection-with-notices"}})
}

fn slot_key(kind: &str, id: &str) -> Result<String> {
    String::from_utf8(wire::canonical(&json!([kind, id]), 8192)?)
        .map_err(|_| Error::Invalid("source slot key"))
}

/// Software-only discovery is available without selecting a source owner.
pub fn software_packet(operation: SourceReadOperation) -> Result<Vec<u8>> {
    let value = match operation {
        SourceReadOperation::Contract => contract(),
        SourceReadOperation::Capabilities => {
            json!({"schema_version":"tos_source_read_capabilities_v1","available":false,"issuer":wire::ISSUER,"layers":{"metadata_record":[],"claim_record":false,"source_slot":false,"authored_csv_record":false},"limits":{"max_handle_bytes":wire::HANDLE_BYTES,"max_request_bytes":wire::REQUEST_BYTES,"max_record_bytes":wire::RECORD_BYTES,"max_response_bytes":wire::RESPONSE_BYTES},"authority":{"is_source":false,"writes_to_source":false,"grants_current_use":false,"native_text_payload":false,"note":"Select an explicit source-owner binding; no path or latest fallback is available."},"source_epoch":null,"status":"unsupported","reason":"source-owner-reader-not-configured"})
        }
        _ => return Err(Error::Unsupported("source owner reader unconfigured")),
    };
    wire::canonical(&value, wire::RESPONSE_BYTES)
}
