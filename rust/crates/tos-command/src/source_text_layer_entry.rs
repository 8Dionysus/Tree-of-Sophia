//! One protected owner-local TextLayer create consumer. It records extraction
//! of an already acquired exact EPUB member; it does not download, OCR,
//! assess text, segment units, publish public content or grant admission.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, finish_creation_worker};
use crate::source_serialization::{
    capture_initial_text_layer, executable, instant, selected_components,
};
use crate::source_text_identity::selected_identity_snapshot;
use crate::source_text_layer_native::recheck_selected_inputs;
use crate::source_text_layer_native::{PreparedInitialLayerText, prepare_initial_text};
use crate::source_text_layer_payload::{
    PayloadIdentity, payload_root_pins, read_acquired_epub_member,
};
use crate::source_text_layer_proposal::{InitialLayerOutput, build_initial_layer};
use crate::source_text_owner::{OwnerTextContext, OwnerTextInitialLayerSelection};
use crate::source_text_private_store::publish_private_text;
use crate::source_text_private_store::{
    PrivateTextCustody, PrivateTextLocks, observe_private_text,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";
const LAYER_SCHEMA: &str = "ToS/contracts/source-text-layer.schema.json";
const ANCHOR_SCHEMA: &str = "ToS/contracts/source-anchor-v2.schema.json";
const EVENT_SCHEMA: &str = "ToS/contracts/provenance-event-v2.schema.json";
const MAX_CONTRACT: usize = 1_048_576;
const MAX_PACKAGE: usize = 12 * 1024 * 1024;
const INITIAL_FILES: [&str; 10] = [
    "source-text-layer.v1.json",
    "source-anchor.v2.json",
    "extraction-policy.json",
    "content.txt",
    "source-create-owner-configuration.json",
    "source-create-inputs.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];

pub struct NativeInitialTextLayerPreview {
    pub owner_configuration: String,
    pub expected_dependencies: String,
    pub source_path: String,
}

pub struct NativeInitialTextLayerResult {
    pub receipt: JsonValue,
    pub replayed: bool,
    pub grants_admission: bool,
}

struct PreparedInitial {
    context: OwnerTextContext,
    grant: OwnerTextInitialLayerSelection,
    publication: crate::source_creation_store::work_transaction::PublicationSnapshot,
    source: crate::source_sign_native::ResolvedInitialTextSource,
    output: InitialLayerOutput,
    member_sha256: Digest256,
    member_bytes: usize,
    member_identity: PayloadIdentity,
    contracts: BTreeMap<String, Digest256>,
    inventory: Digest256,
    software_rows: Vec<Value>,
    owner_configuration: String,
    dependencies: String,
}

pub(crate) fn line(value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    let mut raw = cmd::canonical(value)?;
    raw.push(b'\n');
    Ok(raw)
}

fn selected_contracts(
    context: &OwnerTextContext,
    worker: &CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Digest256>> {
    let mut contracts = BTreeMap::new();
    for name in [LAYER_SCHEMA, ANCHOR_SCHEMA, EVENT_SCHEMA] {
        let raw = context.read(name, MAX_CONTRACT, deadline, cancelled)?;
        let digest = Digest256::of_bytes(&raw);
        if worker.contract_digest(name) != Some(digest) {
            return Err(SourceCommandError::Conflict(
                "native Text selected contract changed",
            ));
        }
        contracts.insert(name.to_owned(), digest);
    }
    Ok(contracts)
}

fn selected_ids(grant: &OwnerTextInitialLayerSelection) -> SourceCommandResult<Vec<String>> {
    let ids = cmd::field(&grant.config, "identities")?;
    ["layer_id", "anchor_id", "passage_id", "provenance_event_id"]
        .iter()
        .map(|name| cmd::text(ids, name).map(str::to_owned))
        .collect()
}

fn selected_configuration(
    context: &OwnerTextContext,
    grant: &OwnerTextInitialLayerSelection,
    contracts: &BTreeMap<String, Digest256>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let contracts = cmd::object(
        contracts
            .iter()
            .map(|(name, digest)| (name.as_str(), cmd::string(&digest.to_prefixed())))
            .collect(),
    );
    let binding = cmd::object(vec![
        (
            "owner_configuration_bytes",
            cmd::string(&Digest256::of_bytes(&grant.raw).to_prefixed()),
        ),
        (
            "context",
            cmd::string(&context.snapshot(deadline, cancelled)?.to_prefixed()),
        ),
        ("contracts", contracts),
        (
            "payload_root_pins",
            payload_root_pins(grant, deadline, cancelled)?,
        ),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&binding)?).to_prefixed())
}

fn input_bytes(
    context: &OwnerTextContext,
    prepared: &PreparedInitialLayerText,
    software_rows: &[Value],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let mut implementation = BTreeMap::new();
    for row in software_rows {
        let path = row["artifact_ref"]
            .as_str()
            .ok_or(SourceCommandError::Invalid("native Text software ref"))?;
        let digest = row["artifact_sha256"]
            .as_str()
            .ok_or(SourceCommandError::Invalid("native Text software SHA"))?;
        if implementation
            .insert(path.to_owned(), format!("sha256:{digest}"))
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "native Text duplicate software ref",
            ));
        }
    }
    let parent: BTreeMap<_, _> = prepared
        .member
        .identity
        .parents
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                [value.0, value.1, u64::from(value.2), u64::from(value.3)],
            )
        })
        .collect();
    let inputs: Vec<_> = prepared
        .source
        .inputs
        .iter()
        .map(|input| json!([input.reference, input.category, input.raw_sha256.to_hex()]))
        .collect();
    let value = json!({
        "schema_version":"tos_native_construction_inputs_v1",
        "context":context.snapshot(deadline, cancelled)?.to_prefixed(),
        "inputs":inputs,
        "payload":{"identity":prepared.member.identity.file,"parents":parent},
        "implementation":implementation,
        "runtime":executable(deadline, cancelled)?.to_prefixed(),
    });
    let raw = serde_json::to_vec(&value)
        .map_err(|_| SourceCommandError::Invalid("native Text inputs JSON"))?;
    line(&cmd::parse(&raw)?)
}

fn dependency_digest(
    source_snapshot: &str,
    input_raw: &[u8],
    inventory: Digest256,
) -> SourceCommandResult<String> {
    let value = cmd::object(vec![
        ("source_snapshot", cmd::string(source_snapshot)),
        (
            "inputs",
            cmd::string(&Digest256::of_bytes(input_raw).to_prefixed()),
        ),
        ("identity_inventory", cmd::string(&inventory.to_prefixed())),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&value)?).to_prefixed())
}

pub(crate) fn checked_schema(
    worker: &mut CutWorkerSchemaExecutor,
    reference: &str,
    raw: &[u8],
    contract: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    match worker.check_reusing_scalar(reference, raw, contract, deadline, cancelled) {
        Ok(true) => Ok(()),
        Ok(false) => Err(SourceCommandError::Invalid("native Text selected schema")),
        Err(reason) => Err(SourceCommandError::SchemaExecution {
            path: reference.to_owned(),
            root: contract.to_owned(),
            reason,
        }),
    }
}

fn select_owner(
    context_path: &Path,
    grant_path: &Path,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    OwnerTextContext,
    crate::source_creation_store::work_transaction::PublicationSnapshot,
    OwnerTextInitialLayerSelection,
)> {
    active(deadline, cancelled)?;
    let schema_path = RelativePath::parse(CONTEXT_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("native Text context schema path"))?;
    let selected_schema = cut
        .read_member(
            cut.current().revision(),
            &schema_path,
            MAX_CONTRACT as u64,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("native Text selected context contract"))?
        .raw;
    let (context, _) =
        OwnerTextContext::select(context_path, &selected_schema, worker, deadline, cancelled)?;
    let publication = context.select_publication(deadline, cancelled)?;
    let grant = OwnerTextInitialLayerSelection::select(&context, grant_path, deadline, cancelled)?;
    Ok((context, publication, grant))
}

fn prepare_selected(
    mut context: OwnerTextContext,
    publication: crate::source_creation_store::work_transaction::PublicationSnapshot,
    grant: OwnerTextInitialLayerSelection,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedInitial> {
    let contracts = selected_contracts(&context, worker, deadline, cancelled)?;
    let owner_configuration =
        selected_configuration(&context, &grant, &contracts, deadline, cancelled)?;
    let ids = selected_ids(&grant)?;
    let refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
    let inventory = selected_identity_snapshot(&context, &refs, exclude, deadline, cancelled)?;
    let prepared = prepare_initial_text(&mut context, worker, &grant, deadline, cancelled)?;
    let output = build_initial_layer(&grant, &prepared)?;
    let source_path = cmd::text(&grant.config, "source_path")?;
    let base = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native Text source parent"))?
        .0;
    checked_schema(
        worker,
        source_path,
        output
            .files
            .get("source-text-layer.v1.json")
            .ok_or(SourceCommandError::Invalid("native Text source output"))?,
        LAYER_SCHEMA,
        deadline,
        cancelled,
    )?;
    checked_schema(
        worker,
        &format!("{base}/source-anchor.v2.json"),
        output
            .files
            .get("source-anchor.v2.json")
            .ok_or(SourceCommandError::Invalid("native Text anchor output"))?,
        ANCHOR_SCHEMA,
        deadline,
        cancelled,
    )?;
    let software_rows = selected_components(software, components, deadline, cancelled)?;
    let input_raw = input_bytes(&context, &prepared, &software_rows, deadline, cancelled)?;
    let dependencies = dependency_digest(&prepared.source.input_snapshot, &input_raw, inventory)?;
    let member_sha256 = Digest256::of_bytes(&prepared.member.raw);
    let member_bytes = prepared.member.raw.len();
    let member_identity = prepared.member.identity.clone();
    let source = prepared.source;
    let mut output = output;
    output
        .files
        .insert("source-create-inputs.json".to_owned(), input_raw);
    if output
        .files
        .values()
        .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
        .is_none_or(|n| n > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native Text package byte budget",
        ));
    }
    context.verify_publication(&publication, deadline, cancelled)?;
    Ok(PreparedInitial {
        context,
        grant,
        publication,
        source,
        output,
        member_sha256,
        member_bytes,
        member_identity,
        contracts,
        inventory,
        software_rows,
        owner_configuration,
        dependencies,
    })
}

fn prepare(
    context_path: &Path,
    grant_path: &Path,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedInitial> {
    let (context, publication, grant) =
        select_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    prepare_selected(
        context,
        publication,
        grant,
        software,
        components,
        worker,
        exclude,
        deadline,
        cancelled,
    )
}

impl PreparedInitial {
    fn verify_stage_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        self.context.snapshot(deadline, cancelled)?;
        let now = OwnerTextInitialLayerSelection::select(
            &self.context,
            &self.grant.path,
            deadline,
            cancelled,
        )?;
        if now.raw != self.grant.raw || now.config != self.grant.config {
            return Err(SourceCommandError::Conflict(
                "native Text delegation changed",
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
        for (reference, digest) in &self.contracts {
            if Digest256::of_bytes(&self.context.read(
                reference,
                MAX_CONTRACT,
                deadline,
                cancelled,
            )?) != *digest
            {
                return Err(SourceCommandError::Conflict(
                    "native Text selected contract changed",
                ));
            }
        }
        if selected_configuration(
            &self.context,
            &self.grant,
            &self.contracts,
            deadline,
            cancelled,
        )? != self.owner_configuration
        {
            return Err(SourceCommandError::Conflict(
                "native Text configuration changed",
            ));
        }
        recheck_selected_inputs(&self.context, &self.source.inputs, deadline, cancelled)?;
        let ids = selected_ids(&self.grant)?;
        let refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
        if selected_identity_snapshot(&self.context, &refs, exclude, deadline, cancelled)?
            != self.inventory
        {
            return Err(SourceCommandError::Conflict(
                "native Text identity inventory changed",
            ));
        }
        let member = read_acquired_epub_member(
            &self.context,
            &self.grant,
            &self.source.payload_entry,
            deadline,
            cancelled,
        )?;
        if member.identity != self.member_identity
            || Digest256::of_bytes(&member.raw) != self.member_sha256
            || member.raw.len() != self.member_bytes
        {
            return Err(SourceCommandError::Conflict(
                "native Text acquired member changed",
            ));
        }
        if selected_components(software, components, deadline, cancelled)? != self.software_rows {
            return Err(SourceCommandError::Conflict(
                "native Text selected software changed",
            ));
        }
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        self.context.snapshot(deadline, cancelled)?;
        Ok(())
    }
}

/// Read-only prepare-create with a genuine selected cut, separate private
/// grant and exact source/software selection. Its digests are descriptive,
/// never a capability to publish a caller-supplied package.
pub fn prepare_initial_text_layer_from_captures(
    context_path: &Path,
    grant_path: &Path,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeInitialTextLayerPreview> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let prepared = prepare(
        context_path,
        grant_path,
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
    Ok(NativeInitialTextLayerPreview {
        owner_configuration: prepared.owner_configuration,
        expected_dependencies: prepared.dependencies,
        source_path: cmd::text(&prepared.grant.config, "source_path")?.to_owned(),
    })
}

fn request_create(
    request: &JsonValue,
    prepared: &PreparedInitial,
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
        ],
    )?;
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1"
        || cmd::text(request, "operation")? != "text-layer.create"
        || cmd::text(request, "command_id")?.is_empty()
        || cmd::text(request, "command_id")?.len() > 256
        || cmd::text(request, "expected_configuration")? != prepared.owner_configuration
        || original_dependencies
            && cmd::text(request, "expected_dependencies")? != prepared.dependencies
        || cmd::field(request, "expected_source")? != &JsonValue::Null
        || cmd::field(request, "expected_revision")? != &JsonValue::Null
    {
        return Err(SourceCommandError::Conflict(
            "native Text initial request differs",
        ));
    }
    Ok(())
}

pub(crate) fn file_refs<'a>(files: impl Iterator<Item = (&'a String, &'a Vec<u8>)>) -> JsonValue {
    cmd::object(
        files
            .map(|(name, raw)| {
                (
                    name.as_str(),
                    cmd::object(vec![
                        (
                            "sha256",
                            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                        ),
                        ("bytes", cmd::number(raw.len() as u64)),
                    ]),
                )
            })
            .collect(),
    )
}

fn same_value(left: &JsonValue, right: &JsonValue) -> SourceCommandResult<bool> {
    Ok(cmd::canonical(left)? == cmd::canonical(right)?)
}

fn verify_retained_initial(
    prepared: &PreparedInitial,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    active(deadline, cancelled)?;
    if files.len() != INITIAL_FILES.len()
        || files
            .keys()
            .any(|name| !INITIAL_FILES.contains(&name.as_str()))
        || prepared
            .output
            .files
            .iter()
            .any(|(name, expected)| files.get(name) != Some(expected))
        || files.get("source-create-request.json").map(Vec::as_slice)
            != Some(line(request)?.as_slice())
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained exact proposal differs",
        ));
    }
    let receipt_raw = files
        .get("source-create-receipt.json")
        .ok_or(SourceCommandError::Conflict("native Text receipt absent"))?;
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
        || !same_value(
            cmd::field(&receipt, "source")?,
            &cmd::reference(&prepared.output.layer, "layer_id", "layer_version")?,
        )?
        || !same_value(
            cmd::field(&receipt, "files")?,
            &file_refs(
                files
                    .iter()
                    .filter(|(name, _)| name.as_str() != "source-create-receipt.json"),
            ),
        )?
        || line(&receipt)? != *receipt_raw
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained receipt differs",
        ));
    }
    cmd::validate_instant(cmd::text(&receipt, "recorded_at")?)?;
    let event_raw = files
        .get("source-create-provenance.jsonl")
        .ok_or(SourceCommandError::Conflict("native Text event absent"))?;
    let event = cmd::parse(event_raw)?;
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native Text source home"))?
        .0;
    if cmd::text(&event, "event_id")?
        != cmd::text(
            cmd::field(&prepared.grant.config, "identities")?,
            "provenance_event_id",
        )?
        || cmd::text(cmd::field(&event, "record_binding")?, "manifest_ref")?
            != format!("{home}/source-create-receipt.json")
        || cmd::text(
            cmd::field(cmd::field(&event, "method")?, "procedure")?,
            "name",
        )? != "exact-native-text-layer-structural-extraction"
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained event identity differs",
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
        .map_err(|_| SourceCommandError::Invalid("native Text retained event JSON"))?;
    if !tos_validation::provenance_rules::semantic_issues(&decoded, 128, deadline)
        .map_err(|_| SourceCommandError::Invalid("native Text retained event semantics"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "native Text retained event semantics",
        ));
    }
    let method = cmd::field(&event, "method")?;
    let configuration = cmd::field(method, "configuration_binding")?;
    let environment = cmd::field(
        cmd::field(method, "environment")?,
        "environment_profile_binding",
    )?;
    let grant_raw =
        files
            .get("source-create-owner-configuration.json")
            .ok_or(SourceCommandError::Conflict(
                "native Text retained grant absent",
            ))?;
    let environment_raw =
        files
            .get("source-create-environment.json")
            .ok_or(SourceCommandError::Conflict(
                "native Text retained environment absent",
            ))?;
    if cmd::text(configuration, "ref")? != format!("{home}/source-create-owner-configuration.json")
        || cmd::text(configuration, "sha256")? != Digest256::of_bytes(grant_raw).to_hex()
        || cmd::text(environment, "ref")? != format!("{home}/source-create-environment.json")
        || cmd::text(environment, "sha256")? != Digest256::of_bytes(environment_raw).to_hex()
        || cmd::text(cmd::field(&event, "activity")?, "event_type")? != "native_extraction"
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained event binding differs",
        ));
    }
    let selected_software =
        decoded["method"]["software_components"]
            .as_array()
            .ok_or(SourceCommandError::Invalid(
                "native Text retained software rows",
            ))?;
    if selected_software.len() != prepared.software_rows.len() + 1
        || selected_software[..prepared.software_rows.len()] != prepared.software_rows[..]
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained software differs",
        ));
    }
    let construction = cmd::parse(files.get("source-create-inputs.json").ok_or(
        SourceCommandError::Conflict("native Text construction inputs absent"),
    )?)?;
    let runtime = cmd::text(&construction, "runtime")?;
    let runner = selected_software.last().ok_or(SourceCommandError::Invalid(
        "native Text retained runner absent",
    ))?;
    if runner["role"] != "serialization-runner"
        || runner["artifact_ref"] != "runtime:tos-native-executable"
        || format!(
            "sha256:{}",
            runner["artifact_sha256"]
                .as_str()
                .ok_or(SourceCommandError::Invalid(
                    "native Text retained runner digest"
                ))?
        ) != runtime
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained runner differs",
        ));
    }
    let environment_value = cmd::parse(environment_raw)?;
    if format!(
        "sha256:{}",
        cmd::text(&environment_value, "runtime_artifact_sha256")?
    ) != runtime
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained runtime differs",
        ));
    }
    let entities = cmd::field(&event, "entities")?;
    let mut observed = BTreeSet::new();
    let mut external_inputs = Vec::new();
    for group in ["inputs", "outputs", "byproducts"] {
        for entity in cmd::array(entities, group)? {
            active(deadline, cancelled)?;
            let reference = cmd::text(entity, "entity_ref")?;
            let Some(name) = reference.strip_prefix(&format!("{home}/")) else {
                if group != "inputs" {
                    return Err(SourceCommandError::Conflict(
                        "native Text external output entity",
                    ));
                }
                external_inputs.push((
                    reference.to_owned(),
                    cmd::text(entity, "sha256")?.to_owned(),
                    cmd::integer(entity, "size_bytes")?,
                ));
                continue;
            };
            let raw = files.get(name).ok_or(SourceCommandError::Conflict(
                "native Text retained event entity absent",
            ))?;
            if !observed.insert(name.to_owned())
                || cmd::integer(entity, "size_bytes")? != raw.len() as u64
                || cmd::text(entity, "sha256")? != Digest256::of_bytes(raw).to_hex()
            {
                return Err(SourceCommandError::Conflict(
                    "native Text retained entity bytes",
                ));
            }
        }
    }
    if observed
        != files
            .keys()
            .filter(|name| {
                name.as_str() != "source-create-provenance.jsonl"
                    && name.as_str() != "source-create-receipt.json"
            })
            .cloned()
            .collect()
    {
        return Err(SourceCommandError::Conflict(
            "native Text retained event closure",
        ));
    }
    let selected_inputs = capture_entities(prepared)?;
    let expected_external = selected_inputs
        .iter()
        .map(|row| {
            Ok((
                row["entity_ref"]
                    .as_str()
                    .ok_or(SourceCommandError::Invalid(
                        "native Text selected external ref",
                    ))?
                    .to_owned(),
                row["sha256"]
                    .as_str()
                    .ok_or(SourceCommandError::Invalid(
                        "native Text selected external SHA",
                    ))?
                    .to_owned(),
                row["size_bytes"]
                    .as_u64()
                    .ok_or(SourceCommandError::Invalid(
                        "native Text selected external size",
                    ))?,
            ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if external_inputs != expected_external {
        return Err(SourceCommandError::Conflict(
            "native Text retained external inputs differ",
        ));
    }
    Ok(receipt)
}

fn capture_entities(prepared: &PreparedInitial) -> SourceCommandResult<Vec<Value>> {
    let item_ref = cmd::text(
        cmd::field(&prepared.grant.config, "source_record_refs")?,
        "item",
    )?;
    let item_home = item_ref
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native Text Item home"))?
        .0;
    let relative = cmd::text(&prepared.source.payload_entry, "relative_path")?;
    let payload_ref = format!("{item_home}/{relative}");
    let scope = cmd::field(&prepared.grant.config, "source_scope")?;
    let member = cmd::field(&prepared.grant.config, "member")?;
    let at = instant()?;
    Ok(vec![
        json!({"entity_ref":payload_ref,"role":"exact-acquired-file",
            "sha256":cmd::text(scope,"file_sha256")?,"size_bytes":cmd::integer(&prepared.source.payload_entry,"byte_size")?,
            "media_type":"application/epub+zip","availability":"owner_local",
            "content_disclosure":"private_content","fixity_verified":true,"fixity_verified_at":at}),
        json!({"entity_ref":format!("{payload_ref}!/{}",cmd::text(member,"member_path")?),
            "role":"exact-container-member","sha256":prepared.member_sha256.to_hex(),
            "size_bytes":prepared.member_bytes,"media_type":"application/xhtml+xml",
            "availability":"owner_local","content_disclosure":"private_content",
            "fixity_verified":true,"fixity_verified_at":at}),
    ])
}

/// Whole initial owner-local publication. The caller cannot substitute a
/// prepared package: construction, native capture, schema FINAL, retained
/// receipt and guarded private no-replace publication are one entry.
pub fn execute_initial_text_layer_from_captures(
    context_path: &Path,
    grant_path: &Path,
    request: &JsonValue,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeInitialTextLayerResult> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let (context, publication, grant) =
        select_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    let selected_path = cmd::text(&grant.config, "source_path")?.to_owned();
    let target = context.private_new_package_target(&selected_path)?;
    let custody = observe_private_text(
        &context,
        &selected_path,
        request,
        &INITIAL_FILES,
        deadline,
        cancelled,
    )?;
    let exclude = match &custody {
        PrivateTextCustody::Published(_) => Some(target.as_path()),
        PrivateTextCustody::Absent | PrivateTextCustody::Pending(_) => None,
    };
    let mut prepared = prepare_selected(
        context,
        publication,
        grant,
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
    let source_path = cmd::text(&prepared.grant.config, "source_path")?.to_owned();
    match custody {
        PrivateTextCustody::Published(files) => {
            let receipt =
                verify_retained_initial(&prepared, request, &files, worker, deadline, cancelled)?;
            finish_creation_worker(worker, deadline, cancelled)?;
            let _locks = PrivateTextLocks::acquire(&prepared.context, deadline, cancelled)?;
            prepared.verify_stage_current(deadline, cancelled)?;
            let current = observe_private_text(
                &prepared.context,
                &source_path,
                request,
                &INITIAL_FILES,
                deadline,
                cancelled,
            )?;
            if !matches!(current, PrivateTextCustody::Published(ref bytes) if bytes == &files) {
                return Err(SourceCommandError::Conflict(
                    "native Text published replay changed",
                ));
            }
            prepared.verify_current(software, components, Some(&target), deadline, cancelled)?;
            return Ok(NativeInitialTextLayerResult {
                receipt,
                replayed: true,
                grants_admission: false,
            });
        }
        PrivateTextCustody::Pending(files) => {
            let receipt =
                verify_retained_initial(&prepared, request, &files, worker, deadline, cancelled)?;
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
            return Ok(NativeInitialTextLayerResult {
                receipt,
                replayed: true,
                grants_admission: false,
            });
        }
        PrivateTextCustody::Absent => (),
    }
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native Text source home"))?
        .0;
    let event_id = cmd::text(
        cmd::field(&prepared.grant.config, "identities")?,
        "provenance_event_id",
    )?;
    let entities = capture_entities(&prepared)?;
    let rights = cmd::field(
        cmd::field(&prepared.grant.config, "derivation_access")?,
        "rights_record_refs",
    )?;
    capture_initial_text_layer(
        request,
        event_id,
        home,
        &source_path,
        rights,
        entities,
        &mut prepared.output.files,
        software,
        components,
        deadline,
        cancelled,
    )?;
    checked_schema(
        worker,
        &format!("{home}/source-create-provenance.jsonl"),
        prepared
            .output
            .files
            .get("source-create-provenance.jsonl")
            .ok_or(SourceCommandError::Invalid("native Text provenance output"))?,
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
            cmd::reference(&prepared.output.layer, "layer_id", "layer_version")?,
        ),
        ("dependencies", cmd::string(&prepared.dependencies)),
        ("files", file_refs(prepared.output.files.iter())),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let receipt_raw = line(&receipt)?;
    prepared
        .output
        .files
        .insert("source-create-receipt.json".to_owned(), receipt_raw);
    if prepared
        .output
        .files
        .values()
        .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
        .is_none_or(|n| n > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native Text complete package byte budget",
        ));
    }
    finish_creation_worker(worker, deadline, cancelled)?;
    prepared.verify_current(software, components, None, deadline, cancelled)?;
    publish_private_text(
        &prepared.context,
        &source_path,
        request,
        &prepared.output.files,
        || prepared.verify_stage_current(deadline, cancelled),
        || prepared.verify_current(software, components, None, deadline, cancelled),
        None,
        deadline,
        cancelled,
    )?;
    Ok(NativeInitialTextLayerResult {
        receipt,
        replayed: false,
        grants_admission: false,
    })
}
