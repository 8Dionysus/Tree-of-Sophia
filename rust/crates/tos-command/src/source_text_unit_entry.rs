//! First TextUnit segmentation over one independently selected owner-local
//! TextLayer. The packet is a bounded proposal, never textual assessment or
//! public admission. The owner-only stage retains its exact original capture.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, finish_creation_worker};
use crate::source_serialization::{
    capture_first_text_unit, executable, instant, selected_components,
};
use crate::source_sign_native::{
    ResolvedOwnerTextLayer, resolve_owner_text_layer, resolve_owner_text_packet,
};
use crate::source_text_identity::selected_identity_snapshot;
use crate::source_text_layer_entry::{checked_schema, file_refs, line};
use crate::source_text_layer_native::recheck_selected_inputs;
use crate::source_text_owner::{OwnerTextContext, OwnerTextUnitSelection};
use crate::source_text_private_store::{
    PrivateTextCustody, PrivateTextLocks, observe_private_text, publish_flat_text,
    publish_private_text,
};
use crate::source_text_unit_proposal::build_text_unit_packet;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const PACKET_SCHEMA: &str = "ToS/contracts/source-text-unit-packet-v1.schema.json";
const BINDING_SCHEMA: &str = "ToS/contracts/native-text-layer-binding.schema.json";
const PACKET_BINDING_SCHEMA: &str = "ToS/contracts/native-text-unit-binding.schema.json";
const EVENT_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const MAX_CONTRACT: usize = 1_048_576;
const MAX_PACKAGE: usize = 12 * 1024 * 1024;
const FILES: [&str; 7] = [
    "source-text-unit.v1.json",
    "source-create-owner-configuration.json",
    "source-create-inputs.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];
const FLAT_FILES: [&str; 6] = [
    "source-text-unit.v1.json",
    "source-create-owner-configuration.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];

fn package_files(flat: bool) -> &'static [&'static str] {
    if flat { &FLAT_FILES } else { &FILES }
}

pub struct NativeFirstTextUnitPreview {
    pub owner_configuration: String,
    pub expected_dependencies: String,
    pub source_path: String,
}

pub struct NativeFirstTextUnitResult {
    pub receipt: JsonValue,
    pub replayed: bool,
    pub grants_admission: bool,
}

struct PreparedUnit {
    context: OwnerTextContext,
    grant: OwnerTextUnitSelection,
    publication: crate::source_creation_store::work_transaction::PublicationSnapshot,
    layer: ResolvedOwnerTextLayer,
    existing_packet: Option<JsonValue>,
    packet: JsonValue,
    files: BTreeMap<String, Vec<u8>>,
    contracts: BTreeMap<String, Digest256>,
    inventory: Digest256,
    software_rows: Vec<Value>,
    owner_configuration: String,
    dependencies: String,
}

fn selected_contracts(
    context: &OwnerTextContext,
    worker: &CutWorkerSchemaExecutor,
    packet_mode: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Digest256>> {
    let mut selected = BTreeMap::new();
    for name in [
        PACKET_SCHEMA,
        if packet_mode {
            PACKET_BINDING_SCHEMA
        } else {
            BINDING_SCHEMA
        },
        EVENT_SCHEMA,
    ] {
        let raw = context.read(name, MAX_CONTRACT, deadline, cancelled)?;
        let sha = Digest256::of_bytes(&raw);
        if worker.contract_digest(name) != Some(sha) {
            return Err(SourceCommandError::Conflict(
                "native TextUnit selected contract changed",
            ));
        }
        selected.insert(name.to_owned(), sha);
    }
    Ok(selected)
}

fn ids(grant: &OwnerTextUnitSelection) -> SourceCommandResult<Vec<String>> {
    let mut ids = [
        "packet_id",
        "scheme_id",
        "segmentation_id",
        "scope_anchor_ref",
        "provenance_event_id",
    ]
    .iter()
    .map(|name| cmd::text(&grant.config, name).map(str::to_owned))
    .collect::<SourceCommandResult<Vec<_>>>()?;
    for row in cmd::array(&grant.config, "unit_slots")? {
        ids.push(cmd::text(row, "unit_id")?.to_owned());
        ids.push(cmd::text(row, "anchor_ref")?.to_owned());
    }
    for row in cmd::array(&grant.config, "gap_anchor_refs")? {
        ids.push(
            row.as_str()
                .ok_or(SourceCommandError::Invalid("native TextUnit gap identity"))?
                .to_owned(),
        );
    }
    Ok(ids)
}

fn configuration(
    context: &OwnerTextContext,
    grant: &OwnerTextUnitSelection,
    contracts: &BTreeMap<String, Digest256>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let selected = cmd::object(
        contracts
            .iter()
            .map(|(name, sha)| (name.as_str(), cmd::string(&sha.to_prefixed())))
            .collect(),
    );
    let value = cmd::object(vec![
        (
            "owner_configuration_bytes",
            cmd::string(&Digest256::of_bytes(&grant.raw).to_prefixed()),
        ),
        (
            "context",
            cmd::string(&context.snapshot(deadline, cancelled)?.to_prefixed()),
        ),
        ("contracts", selected),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&value)?).to_prefixed())
}

fn selected_inputs(
    context: &OwnerTextContext,
    layer: &ResolvedOwnerTextLayer,
    software_rows: &[Value],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let implementation = software_rows
        .iter()
        .map(|row| {
            let name = row["artifact_ref"]
                .as_str()
                .ok_or(SourceCommandError::Invalid("native TextUnit software ref"))?;
            let sha = row["artifact_sha256"]
                .as_str()
                .ok_or(SourceCommandError::Invalid("native TextUnit software SHA"))?;
            Ok((name.to_owned(), format!("sha256:{sha}")))
        })
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    if implementation.len() != software_rows.len() {
        return Err(SourceCommandError::Invalid(
            "native TextUnit repeated software ref",
        ));
    }
    let inputs = layer
        .inputs
        .iter()
        .map(|row| json!([row.reference, row.category, row.raw_sha256.to_hex()]))
        .collect::<Vec<_>>();
    let value = json!({"schema_version":"tos_native_construction_inputs_v1",
        "context":context.snapshot(deadline, cancelled)?.to_prefixed(),
        "inputs":inputs,"implementation":implementation,
        "runtime":executable(deadline, cancelled)?.to_prefixed()});
    line(&cmd::parse(&serde_json::to_vec(&value).map_err(|_| {
        SourceCommandError::Invalid("native TextUnit inputs JSON")
    })?)?)
}

fn dependencies(
    layer_snapshot: &str,
    inventory: Digest256,
    inputs: &[u8],
) -> SourceCommandResult<String> {
    let native = cmd::object(vec![
        ("native_snapshot", cmd::string(layer_snapshot)),
        ("identity_snapshot", cmd::string(&inventory.to_prefixed())),
        (
            "implementation",
            cmd::field(&cmd::parse(inputs)?, "implementation")?.clone(),
        ),
    ]);
    let first = Digest256::of_bytes(&cmd::canonical(&native)?).to_prefixed();
    let value = cmd::object(vec![
        ("dependencies", cmd::string(&first)),
        (
            "exact_inputs",
            cmd::string(&Digest256::of_bytes(inputs).to_prefixed()),
        ),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&value)?).to_prefixed())
}

fn prepare(
    context_path: &Path,
    grant_path: &Path,
    request: &JsonValue,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedUnit> {
    active(deadline, cancelled)?;
    let schema = RelativePath::parse(CONTEXT_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("native TextUnit context schema"))?;
    let selected = cut
        .read_member(
            cut.current().revision(),
            &schema,
            MAX_CONTRACT as u64,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("native TextUnit context contract"))?;
    let (mut context, _) =
        OwnerTextContext::select(context_path, &selected.raw, worker, deadline, cancelled)?;
    let publication = context.select_publication(deadline, cancelled)?;
    let grant = OwnerTextUnitSelection::select(&context, grant_path, deadline, cancelled)?;
    let packet_mode =
        cmd::text(&grant.config, "schema_version")? == "tos_local_text_unit_create_owner_v1";
    let contracts = selected_contracts(&context, worker, packet_mode, deadline, cancelled)?;
    let owner_configuration = configuration(&context, &grant, &contracts, deadline, cancelled)?;
    let identities = ids(&grant)?;
    let refs = identities.iter().map(String::as_str).collect::<Vec<_>>();
    let inventory = selected_identity_snapshot(&context, &refs, exclude, deadline, cancelled)?;
    let (layer, existing_packet) = if packet_mode {
        let resolved = resolve_owner_text_packet(
            &mut context,
            worker,
            cmd::field(&grant.config, "source_binding")?,
            deadline,
            cancelled,
        )?;
        (
            ResolvedOwnerTextLayer {
                layer: resolved.layer,
                raw: resolved.raw,
                inputs: resolved.inputs,
                input_snapshot: resolved.input_snapshot,
            },
            Some(resolved.packet),
        )
    } else {
        (
            resolve_owner_text_layer(
                &mut context,
                worker,
                cmd::field(&grant.config, "source_binding")?,
                deadline,
                cancelled,
            )?,
            None,
        )
    };
    let text = std::str::from_utf8(&layer.raw)
        .map_err(|_| SourceCommandError::Invalid("native TextUnit exact UTF-8"))?;
    let (packet, packet_raw) = build_text_unit_packet(
        existing_packet.as_ref(),
        &layer.layer,
        cmd::field(&grant.config, "source_binding")?,
        text,
        &grant.config,
        request,
        deadline,
        cancelled,
    )?;
    let path = cmd::text(&grant.config, "source_path")?;
    checked_schema(
        worker,
        path,
        &packet_raw,
        PACKET_SCHEMA,
        deadline,
        cancelled,
    )?;
    let software_rows = selected_components(software, components, deadline, cancelled)?;
    let inputs = selected_inputs(&context, &layer, &software_rows, deadline, cancelled)?;
    let dependencies = if packet_mode {
        let value = cmd::object(vec![
            ("native_snapshot", cmd::string(&layer.input_snapshot)),
            ("identity_snapshot", cmd::string(&inventory.to_prefixed())),
            (
                "implementation",
                cmd::field(&cmd::parse(&inputs)?, "implementation")?.clone(),
            ),
        ]);
        Digest256::of_bytes(&cmd::canonical(&value)?).to_prefixed()
    } else {
        dependencies(&layer.input_snapshot, inventory, &inputs)?
    };
    let mut files = BTreeMap::new();
    files.insert("source-text-unit.v1.json".into(), packet_raw);
    files.insert(
        "source-create-owner-configuration.json".into(),
        grant.raw.clone(),
    );
    if !packet_mode {
        files.insert("source-create-inputs.json".into(), inputs);
    }
    if files
        .values()
        .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
        .is_none_or(|n| n > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native TextUnit prepared package budget",
        ));
    }
    context.verify_publication(&publication, deadline, cancelled)?;
    Ok(PreparedUnit {
        context,
        grant,
        publication,
        layer,
        existing_packet,
        packet,
        files,
        contracts,
        inventory,
        software_rows,
        owner_configuration,
        dependencies,
    })
}

impl PreparedUnit {
    fn verify_stage_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        self.context.snapshot(deadline, cancelled)?;
        let now =
            OwnerTextUnitSelection::select(&self.context, &self.grant.path, deadline, cancelled)?;
        if now.raw != self.grant.raw || now.config != self.grant.config {
            return Err(SourceCommandError::Conflict(
                "native TextUnit grant changed",
            ));
        }
        Ok(())
    }

    fn verify_current(
        &self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        exclude: Option<&Path>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify_stage_current(deadline, cancelled)?;
        for (name, digest) in &self.contracts {
            if Digest256::of_bytes(&self.context.read(name, MAX_CONTRACT, deadline, cancelled)?)
                != *digest
            {
                return Err(SourceCommandError::Conflict(
                    "native TextUnit contracts changed",
                ));
            }
        }
        if configuration(
            &self.context,
            &self.grant,
            &self.contracts,
            deadline,
            cancelled,
        )? != self.owner_configuration
        {
            return Err(SourceCommandError::Conflict(
                "native TextUnit configuration changed",
            ));
        }
        recheck_selected_inputs(&self.context, &self.layer.inputs, deadline, cancelled)?;
        let rep = cmd::field(&self.layer.layer, "representation")?;
        let current = self.context.read(
            cmd::text(rep, "content_ref")?,
            self.layer.raw.len(),
            deadline,
            cancelled,
        )?;
        if current != self.layer.raw {
            return Err(SourceCommandError::Conflict(
                "native TextUnit representation changed",
            ));
        }
        let identities = ids(&self.grant)?;
        let refs = identities.iter().map(String::as_str).collect::<Vec<_>>();
        if selected_identity_snapshot(&self.context, &refs, exclude, deadline, cancelled)?
            != self.inventory
        {
            return Err(SourceCommandError::Conflict(
                "native TextUnit identity changed",
            ));
        }
        if selected_components(software, components, deadline, cancelled)? != self.software_rows {
            return Err(SourceCommandError::Conflict(
                "native TextUnit software changed",
            ));
        }
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        Ok(())
    }
}

fn request_create(
    request: &JsonValue,
    prepared: &PreparedUnit,
    original_dependencies: bool,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        request,
        &[
            "schema_version",
            "operation",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "spans",
            "excluded_gaps",
        ],
    )?;
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1"
        || cmd::text(request, "operation")? != "text-unit.create"
        || cmd::text(request, "command_id")?.is_empty()
        || cmd::text(request, "command_id")?.len() > 256
        || cmd::text(request, "expected_configuration")? != prepared.owner_configuration
        || original_dependencies
            && cmd::text(request, "expected_dependencies")? != prepared.dependencies
        || cmd::field(request, "expected_source")? != &JsonValue::Null
        || cmd::field(request, "expected_revision")? != &JsonValue::Null
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit creation request changed",
        ));
    }
    Ok(())
}

fn capture_inputs(
    prepared: &PreparedUnit,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<Value>> {
    active(deadline, cancelled)?;
    let target = cmd::field(
        cmd::field(&prepared.grant.config, "source_binding")?,
        "text_layer",
    )?;
    let layer_ref = cmd::text(target, "record_ref")?;
    let representation = cmd::field(&prepared.layer.layer, "representation")?;
    let content_ref = cmd::text(representation, "content_ref")?;
    let selected = prepared
        .layer
        .inputs
        .iter()
        .find(|input| input.reference == layer_ref)
        .ok_or(SourceCommandError::Conflict(
            "native TextUnit selected layer missing",
        ))?;
    let layer_raw = prepared
        .context
        .read(layer_ref, 1_048_576, deadline, cancelled)?;
    if Digest256::of_bytes(&layer_raw) != selected.raw_sha256 {
        return Err(SourceCommandError::Conflict(
            "native TextUnit selected layer changed",
        ));
    }
    let at = instant()?;
    let mut entities = vec![
        json!({"entity_ref":layer_ref,"role":"verified-text-layer",
            "sha256":selected.raw_sha256.to_hex(),"size_bytes":layer_raw.len(),
            "media_type":"application/json","availability":"owner_local",
            "content_disclosure":"private_content","fixity_verified":true,"fixity_verified_at":at}),
        json!({"entity_ref":content_ref,"role":"verified-exact-representation",
            "sha256":Digest256::of_bytes(&prepared.layer.raw).to_hex(),
            "size_bytes":prepared.layer.raw.len(),
            "media_type":cmd::text(representation,"media_type")?,
            "availability":"owner_local","content_disclosure":"private_content",
            "fixity_verified":true,"fixity_verified_at":at}),
    ];
    if prepared.existing_packet.is_some() {
        let packet_ref = cmd::text(
            cmd::field(&prepared.grant.config, "source_binding")?,
            "packet_ref",
        )?;
        let selected = prepared
            .layer
            .inputs
            .iter()
            .find(|input| input.reference == packet_ref)
            .ok_or(SourceCommandError::Conflict(
                "native TextUnit selected packet missing",
            ))?;
        let raw = prepared
            .context
            .read(packet_ref, 1_048_576, deadline, cancelled)?;
        if Digest256::of_bytes(&raw) != selected.raw_sha256 {
            return Err(SourceCommandError::Conflict(
                "native TextUnit selected packet changed",
            ));
        }
        entities.insert(0,json!({"entity_ref":packet_ref,"role":"verified-native-packet",
            "sha256":selected.raw_sha256.to_hex(),"size_bytes":raw.len(),
            "media_type":"application/json","availability":"owner_local",
            "content_disclosure":"private_content","fixity_verified":true,"fixity_verified_at":instant()?}));
    }
    Ok(entities)
}

/// This entry only reports selected preparation; it cannot publish caller
/// bytes or grant private reading in a later invocation.
pub fn prepare_first_text_unit_from_captures(
    context_path: &Path,
    grant_path: &Path,
    request: &JsonValue,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeFirstTextUnitPreview> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let prepared = prepare(
        context_path,
        grant_path,
        request,
        cut,
        software,
        components,
        worker,
        None,
        deadline,
        cancelled,
    )?;
    finish_creation_worker(worker, deadline, cancelled)?;
    prepared.verify_current(software, components, None, deadline, cancelled)?;
    Ok(NativeFirstTextUnitPreview {
        owner_configuration: prepared.owner_configuration,
        expected_dependencies: prepared.dependencies,
        source_path: cmd::text(&prepared.grant.config, "source_path")?.to_owned(),
    })
}

fn verify_retained(
    prepared: &PreparedUnit,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    active(deadline, cancelled)?;
    let expected = package_files(prepared.existing_packet.is_some());
    if files.len() != expected.len()
        || files.keys().any(|name| !expected.contains(&name.as_str()))
        || prepared
            .files
            .iter()
            .any(|(name, raw)| files.get(name) != Some(raw))
        || files.get("source-create-request.json").map(Vec::as_slice)
            != Some(line(request)?.as_slice())
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit retained files differ",
        ));
    }
    let receipt_raw =
        files
            .get("source-create-receipt.json")
            .ok_or(SourceCommandError::Conflict(
                "native TextUnit receipt absent",
            ))?;
    let receipt = cmd::parse(receipt_raw)?;
    cmd::exact_keys(
        &receipt,
        &[
            "schema_version",
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "source_path",
            "source",
            "dependencies",
            "files",
            "grants_admission",
        ],
    )?;
    let source_path = cmd::text(&prepared.grant.config, "source_path")?;
    let packet_ref = cmd::reference(&prepared.packet, "packet_id", "packet_version")?;
    if cmd::text(&receipt, "schema_version")? != "tos_local_source_create_receipt_v1"
        || cmd::field(&receipt, "command_id")? != cmd::field(request, "command_id")?
        || cmd::text(&receipt, "request_digest")?
            != Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed()
        || cmd::field(&receipt, "principal_id")?
            != cmd::field(&prepared.grant.config, "principal_id")?
        || cmd::field(&receipt, "authority_ref")?
            != cmd::field(&prepared.grant.config, "authority_ref")?
        || cmd::text(&receipt, "owner_configuration")? != prepared.owner_configuration
        || cmd::text(&receipt, "source_path")? != source_path
        || cmd::text(&receipt, "dependencies")? != cmd::text(request, "expected_dependencies")?
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
        || cmd::canonical(cmd::field(&receipt, "source")?)? != cmd::canonical(&packet_ref)?
        || cmd::canonical(cmd::field(&receipt, "files")?)?
            != cmd::canonical(&file_refs(
                files
                    .iter()
                    .filter(|(name, _)| name.as_str() != "source-create-receipt.json"),
            ))?
        || line(&receipt)? != *receipt_raw
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit retained receipt differs",
        ));
    }
    cmd::validate_instant(cmd::text(&receipt, "recorded_at")?)?;
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native TextUnit home"))?
        .0;
    let event_raw = files
        .get("source-create-provenance.jsonl")
        .ok_or(SourceCommandError::Conflict("native TextUnit event absent"))?;
    let event = cmd::parse(event_raw)?;
    if cmd::text(&event, "event_id")? != cmd::text(&prepared.grant.config, "provenance_event_id")?
        || cmd::text(cmd::field(&event, "record_binding")?, "manifest_ref")?
            != format!("{home}/source-create-receipt.json")
        || cmd::text(
            cmd::field(cmd::field(&event, "method")?, "procedure")?,
            "name",
        )? != if prepared.existing_packet.is_some() {
            "exact-native-text-unit-construction"
        } else {
            "exact-native-first-text-unit-segmentation"
        }
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit event binding differs",
        ));
    }
    checked_schema(
        worker,
        &format!("{home}/source-create-provenance.jsonl"),
        event_raw,
        EVENT_SCHEMA,
        deadline,
        cancelled,
    )?;
    let decoded: Value = serde_json::from_slice(event_raw)
        .map_err(|_| SourceCommandError::Invalid("native TextUnit retained event JSON"))?;
    if !tos_validation::provenance_rules::semantic_issues(&decoded, 128, deadline)
        .map_err(|_| SourceCommandError::Invalid("native TextUnit event semantics"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "native TextUnit event semantics",
        ));
    }
    let method = cmd::field(&event, "method")?;
    let config = cmd::field(method, "configuration_binding")?;
    let environment = cmd::field(
        cmd::field(method, "environment")?,
        "environment_profile_binding",
    )?;
    let grant_raw =
        files
            .get("source-create-owner-configuration.json")
            .ok_or(SourceCommandError::Conflict(
                "native TextUnit retained grant absent",
            ))?;
    let env_raw =
        files
            .get("source-create-environment.json")
            .ok_or(SourceCommandError::Conflict(
                "native TextUnit retained environment absent",
            ))?;
    if cmd::text(config, "ref")? != format!("{home}/source-create-owner-configuration.json")
        || cmd::text(config, "sha256")? != Digest256::of_bytes(grant_raw).to_hex()
        || cmd::text(environment, "ref")? != format!("{home}/source-create-environment.json")
        || cmd::text(environment, "sha256")? != Digest256::of_bytes(env_raw).to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit environment differs",
        ));
    }
    let rows = decoded["method"]["software_components"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("native TextUnit software rows"))?;
    if rows.len() != prepared.software_rows.len() + 1
        || rows[..prepared.software_rows.len()] != prepared.software_rows[..]
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit software differs",
        ));
    }
    let inputs = files
        .get("source-create-inputs.json")
        .map(|raw| cmd::parse(raw))
        .transpose()?;
    let environment_value = cmd::parse(env_raw)?;
    let runtime = if let Some(inputs) = &inputs {
        cmd::text(inputs, "runtime")?.to_owned()
    } else {
        format!(
            "sha256:{}",
            cmd::text(&environment_value, "runtime_artifact_sha256")?
        )
    };
    if rows
        .last()
        .and_then(|row| row["artifact_sha256"].as_str())
        .is_none_or(|sha| format!("sha256:{sha}") != runtime)
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit runner differs",
        ));
    }
    let mut internal = std::collections::BTreeSet::new();
    let mut external = Vec::new();
    for group in ["inputs", "outputs", "byproducts"] {
        for entity in cmd::array(cmd::field(&event, "entities")?, group)? {
            active(deadline, cancelled)?;
            let reference = cmd::text(entity, "entity_ref")?;
            if let Some(name) = reference.strip_prefix(&format!("{home}/")) {
                let raw = files.get(name).ok_or(SourceCommandError::Conflict(
                    "native TextUnit entity absent",
                ))?;
                if !internal.insert(name.to_owned())
                    || cmd::integer(entity, "size_bytes")? != raw.len() as u64
                    || cmd::text(entity, "sha256")? != Digest256::of_bytes(raw).to_hex()
                {
                    return Err(SourceCommandError::Conflict(
                        "native TextUnit entity bytes differ",
                    ));
                }
            } else if group == "inputs" {
                external.push((
                    reference.to_owned(),
                    cmd::text(entity, "sha256")?.to_owned(),
                    cmd::integer(entity, "size_bytes")?,
                ));
            } else {
                return Err(SourceCommandError::Conflict(
                    "native TextUnit external output",
                ));
            }
        }
    }
    if internal
        != files
            .keys()
            .filter(|name| {
                ![
                    "source-create-provenance.jsonl",
                    "source-create-receipt.json",
                ]
                .contains(&name.as_str())
            })
            .cloned()
            .collect()
    {
        return Err(SourceCommandError::Conflict(
            "native TextUnit event closure differs",
        ));
    }
    let expected = capture_inputs(prepared, deadline, cancelled)?
        .into_iter()
        .map(|row| {
            Ok((
                row["entity_ref"]
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("native TextUnit external ref"))?
                    .to_owned(),
                row["sha256"]
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("native TextUnit external SHA"))?
                    .to_owned(),
                row["size_bytes"]
                    .as_u64()
                    .ok_or(SourceCommandError::Invalid("native TextUnit external size"))?,
            ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if external != expected {
        return Err(SourceCommandError::Conflict(
            "native TextUnit external inputs changed",
        ));
    }
    Ok(receipt)
}

/// One whole protected initial segmentation. The public request contains
/// spans only; this entry selects the layer, builds all bytes, observes native
/// provenance, reaches worker FINAL and uses the owner-held durable stage.
pub fn execute_first_text_unit_from_captures(
    context_path: &Path,
    grant_path: &Path,
    request: &JsonValue,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeFirstTextUnitResult> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    active(deadline, cancelled)?;
    let schema = RelativePath::parse(CONTEXT_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("native TextUnit context contract path"))?;
    let selected = cut
        .read_member(
            cut.current().revision(),
            &schema,
            MAX_CONTRACT as u64,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("native TextUnit context contract"))?;
    let (context, _) =
        OwnerTextContext::select(context_path, &selected.raw, worker, deadline, cancelled)?;
    let grant = OwnerTextUnitSelection::select(&context, grant_path, deadline, cancelled)?;
    let source_path = cmd::text(&grant.config, "source_path")?.to_owned();
    let target = context.private_new_package_target(&source_path)?;
    let flat = cmd::text(&grant.config, "schema_version")? == "tos_local_text_unit_create_owner_v1";
    let selected_files = package_files(flat);
    let custody = observe_private_text(
        &context,
        &source_path,
        request,
        selected_files,
        deadline,
        cancelled,
    )?;
    let exclude = match &custody {
        PrivateTextCustody::Published(_) => Some(target.as_path()),
        PrivateTextCustody::Pending(_) | PrivateTextCustody::Absent => None,
    };
    // `prepare` repeats source selection from this exact cut. That repeated
    // metadata/context read is a current check, not an issuer shortcut.
    let mut prepared = prepare(
        context_path,
        grant_path,
        request,
        cut,
        software,
        components,
        worker,
        exclude,
        deadline,
        cancelled,
    )?;
    request_create(
        request,
        &prepared,
        matches!(&custody, PrivateTextCustody::Absent),
    )?;
    match custody {
        PrivateTextCustody::Published(files) => {
            let receipt = verify_retained(&prepared, request, &files, worker, deadline, cancelled)?;
            finish_creation_worker(worker, deadline, cancelled)?;
            let _locks = PrivateTextLocks::acquire(&prepared.context, deadline, cancelled)?;
            prepared.verify_stage_current(deadline, cancelled)?;
            let current = observe_private_text(
                &prepared.context,
                &source_path,
                request,
                selected_files,
                deadline,
                cancelled,
            )?;
            if !matches!(current, PrivateTextCustody::Published(ref raw) if raw == &files) {
                return Err(SourceCommandError::Conflict(
                    "native TextUnit published replay changed",
                ));
            }
            prepared.verify_current(software, components, Some(&target), deadline, cancelled)?;
            return Ok(NativeFirstTextUnitResult {
                receipt,
                replayed: true,
                grants_admission: false,
            });
        }
        PrivateTextCustody::Pending(files) => {
            if flat {
                return Err(SourceCommandError::Conflict(
                    "native TextUnit flat stage is not a retained plan",
                ));
            }
            let receipt = verify_retained(&prepared, request, &files, worker, deadline, cancelled)?;
            finish_creation_worker(worker, deadline, cancelled)?;
            prepared.verify_current(software, components, None, deadline, cancelled)?;
            publish_private_text(
                &prepared.context,
                &source_path,
                request,
                &files,
                || prepared.verify_stage_current(deadline, cancelled),
                || prepared.verify_current(software, components, None, deadline, cancelled),
                None,
                deadline,
                cancelled,
            )?;
            return Ok(NativeFirstTextUnitResult {
                receipt,
                replayed: true,
                grants_admission: false,
            });
        }
        PrivateTextCustody::Absent => (),
    }
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native TextUnit home"))?
        .0;
    let inputs = capture_inputs(&prepared, deadline, cancelled)?;
    let rep = cmd::field(&prepared.layer.layer, "representation")?;
    let rights = cmd::field(rep, "rights_record_refs")?;
    capture_first_text_unit(
        !flat,
        request,
        cmd::text(&prepared.grant.config, "provenance_event_id")?,
        home,
        &source_path,
        rights,
        inputs,
        &mut prepared.files,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let event_raw = prepared
        .files
        .get("source-create-provenance.jsonl")
        .ok_or(SourceCommandError::Invalid("native TextUnit event output"))?;
    checked_schema(
        worker,
        &format!("{home}/source-create-provenance.jsonl"),
        event_raw,
        EVENT_SCHEMA,
        deadline,
        cancelled,
    )?;
    let receipt = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_source_create_receipt_v1"),
        ),
        ("command_id", cmd::field(request, "command_id")?.clone()),
        (
            "request_digest",
            cmd::string(&Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed()),
        ),
        (
            "principal_id",
            cmd::field(&prepared.grant.config, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(&prepared.grant.config, "authority_ref")?.clone(),
        ),
        (
            "owner_configuration",
            cmd::string(&prepared.owner_configuration),
        ),
        ("recorded_at", cmd::string(&instant()?)),
        ("source_path", cmd::string(&source_path)),
        (
            "source",
            cmd::reference(&prepared.packet, "packet_id", "packet_version")?,
        ),
        ("dependencies", cmd::string(&prepared.dependencies)),
        ("files", file_refs(prepared.files.iter())),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    prepared
        .files
        .insert("source-create-receipt.json".into(), line(&receipt)?);
    if prepared
        .files
        .values()
        .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
        .is_none_or(|n| n > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native TextUnit complete package budget",
        ));
    }
    finish_creation_worker(worker, deadline, cancelled)?;
    prepared.verify_current(software, components, None, deadline, cancelled)?;
    if flat {
        publish_flat_text(
            &prepared.context,
            &source_path,
            request,
            &prepared.files,
            || prepared.verify_stage_current(deadline, cancelled),
            || prepared.verify_current(software, components, None, deadline, cancelled),
            None,
            deadline,
            cancelled,
        )?;
    } else {
        publish_private_text(
            &prepared.context,
            &source_path,
            request,
            &prepared.files,
            || prepared.verify_stage_current(deadline, cancelled),
            || prepared.verify_current(software, components, None, deadline, cancelled),
            deadline,
            cancelled,
        )?;
    }
    Ok(NativeFirstTextUnitResult {
        receipt,
        replayed: false,
        grants_admission: false,
    })
}
