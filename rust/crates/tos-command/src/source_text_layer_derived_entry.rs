//! Protected owner-local TextLayer correction, normalization and recorded
//! source return. A selected grant and current source are required on every
//! prepare, publication and cold replay; a retained receipt is not authority.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, finish_creation_worker};
use crate::source_serialization::{
    capture_derived_text_layer, executable, instant, selected_components,
};
use crate::source_sign_native::{ResolvedDerivedTextSource, resolve_derived_owner_text_source};
use crate::source_text_identity::selected_identity_snapshot;
use crate::source_text_layer_derived_proposal::{DerivedLayerOutput, build_derived_layer};
use crate::source_text_layer_entry::{checked_schema, file_refs, line};
use crate::source_text_layer_native::recheck_selected_inputs;
use crate::source_text_layer_payload::{PayloadIdentity, verify_acquired_file};
use crate::source_text_owner::{OwnerTextContext, OwnerTextDerivedSelection};
use crate::source_text_owner_ocr;
use crate::source_text_private_store::{
    PrivateTextCustody, PrivateTextLocks, observe_private_text, publish_private_text,
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
const FILES: [&str; 9] = [
    "source-text-layer.v1.json",
    "derivation-policy.json",
    "content.txt",
    "source-create-owner-configuration.json",
    "source-create-inputs.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];
const OCR_FILES: [&str; 12] = [
    "source-text-layer.v1.json",
    "derivation-policy.json",
    "content.txt",
    "owner-ocr-receipt.json",
    "owner-ocr-signature.sigstore.json",
    "owner-ocr-signer.pub",
    "source-create-owner-configuration.json",
    "source-create-inputs.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];

fn expected_files(operation: &str) -> &'static [&'static str] {
    if operation.starts_with("text-layer.record-owner-") {
        &OCR_FILES
    } else {
        &FILES
    }
}

pub struct NativeDerivedTextLayerPreview {
    pub owner_configuration: String,
    pub expected_dependencies: String,
    pub source_path: String,
}
pub struct NativeDerivedTextLayerResult {
    pub receipt: JsonValue,
    pub replayed: bool,
    pub grants_admission: bool,
}

struct PreparedDerived {
    context: OwnerTextContext,
    grant: OwnerTextDerivedSelection,
    publication: crate::source_creation_store::work_transaction::PublicationSnapshot,
    source: ResolvedDerivedTextSource,
    output: DerivedLayerOutput,
    supplied_raw: Option<Vec<u8>>,
    file_identity: Option<PayloadIdentity>,
    contracts: BTreeMap<String, Digest256>,
    inventory: Digest256,
    software_rows: Vec<Value>,
    owner_configuration: String,
    dependencies: String,
    owner_verification_deadline: Instant,
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
    OwnerTextDerivedSelection,
)> {
    let schema_path = RelativePath::parse(CONTEXT_SCHEMA)
        .map_err(|_| SourceCommandError::Invalid("native derived context schema path"))?;
    let selected = cut
        .read_member(
            cut.current().revision(),
            &schema_path,
            MAX_CONTRACT as u64,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("native derived context contract"))?
        .raw;
    let (context, _) =
        OwnerTextContext::select(context_path, &selected, worker, deadline, cancelled)?;
    let publication = context.select_publication(deadline, cancelled)?;
    let grant = OwnerTextDerivedSelection::select(&context, grant_path, deadline, cancelled)?;
    Ok((context, publication, grant))
}

fn contracts(
    context: &OwnerTextContext,
    worker: &CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Digest256>> {
    let mut selected = BTreeMap::new();
    for name in [LAYER_SCHEMA, ANCHOR_SCHEMA, EVENT_SCHEMA] {
        let raw = context.read(name, MAX_CONTRACT, deadline, cancelled)?;
        let sha = Digest256::of_bytes(&raw);
        if worker.contract_digest(name) != Some(sha) {
            return Err(SourceCommandError::Conflict(
                "native derived selected contract changed",
            ));
        }
        selected.insert(name.to_owned(), sha);
    }
    Ok(selected)
}

fn selected_configuration(
    context: &OwnerTextContext,
    grant: &OwnerTextDerivedSelection,
    contracts: &BTreeMap<String, Digest256>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let contracts = cmd::object(
        contracts
            .iter()
            .map(|(name, sha)| (name.as_str(), cmd::string(&sha.to_prefixed())))
            .collect(),
    );
    let pins = if cmd::text(cmd::field(&grant.config, "input")?, "kind")? == "text_layer" {
        JsonValue::Null
    } else {
        crate::source_text_layer_payload::payload_root_pins_from_config(
            &grant.config,
            deadline,
            cancelled,
        )?
    };
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
        ("payload_root_pins", pins),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&binding)?).to_prefixed())
}

fn selected_ids(grant: &OwnerTextDerivedSelection) -> SourceCommandResult<Vec<String>> {
    let ids = cmd::field(&grant.config, "identities")?;
    Ok(vec![
        cmd::text(ids, "layer_id")?.to_owned(),
        cmd::text(ids, "provenance_event_id")?.to_owned(),
    ])
}

fn input_bytes(
    context: &OwnerTextContext,
    source: &ResolvedDerivedTextSource,
    file_identity: &Option<PayloadIdentity>,
    software_rows: &[Value],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let mut implementation = BTreeMap::new();
    for row in software_rows {
        let name = row["artifact_ref"]
            .as_str()
            .ok_or(SourceCommandError::Invalid("native derived software ref"))?;
        let digest = row["artifact_sha256"]
            .as_str()
            .ok_or(SourceCommandError::Invalid(
                "native derived software digest",
            ))?;
        if implementation
            .insert(name.to_owned(), format!("sha256:{digest}"))
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "native derived repeated software ref",
            ));
        }
    }
    let inputs = source
        .inputs
        .iter()
        .map(|row| json!([row.reference, row.category, row.raw_sha256.to_hex()]))
        .collect::<Vec<_>>();
    let payload = file_identity.as_ref().map(|identity| {
        let parents: BTreeMap<_, _> = identity
            .parents
            .iter()
            .map(|(name, (dev, ino, mode, uid))| {
                (
                    name.clone(),
                    [*dev, *ino, u64::from(*mode), u64::from(*uid)],
                )
            })
            .collect();
        json!({"identity":identity.file,"parents":parents})
    });
    let value = json!({"schema_version":"tos_native_construction_inputs_v1",
        "context":context.snapshot(deadline,cancelled)?.to_prefixed(),
        "inputs":inputs,"payload":payload,"implementation":implementation,
        "runtime":executable(deadline,cancelled)?.to_prefixed()});
    line(&cmd::parse(&serde_json::to_vec(&value).map_err(|_| {
        SourceCommandError::Invalid("native derived input JSON")
    })?)?)
}

fn dependencies(
    source: &ResolvedDerivedTextSource,
    inputs: &[u8],
    inventory: Digest256,
) -> SourceCommandResult<String> {
    let value = cmd::object(vec![
        ("source_snapshot", cmd::string(&source.input_snapshot)),
        (
            "inputs",
            cmd::string(&Digest256::of_bytes(inputs).to_prefixed()),
        ),
        ("identity_inventory", cmd::string(&inventory.to_prefixed())),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&value)?).to_prefixed())
}

fn prepare_selected(
    mut context: OwnerTextContext,
    publication: crate::source_creation_store::work_transaction::PublicationSnapshot,
    grant: OwnerTextDerivedSelection,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    retained: Option<&BTreeMap<String, Vec<u8>>>,
    exclude: Option<&Path>,
    owner_verification_deadline: Instant,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedDerived> {
    let contracts = contracts(&context, worker, deadline, cancelled)?;
    let owner_configuration =
        selected_configuration(&context, &grant, &contracts, deadline, cancelled)?;
    let refs = selected_ids(&grant)?;
    let inventory = selected_identity_snapshot(
        &context,
        &refs.iter().map(String::as_str).collect::<Vec<_>>(),
        exclude,
        deadline,
        cancelled,
    )?;
    let source = resolve_derived_owner_text_source(
        &mut context,
        worker,
        &grant.config,
        deadline,
        cancelled,
    )?;
    let supplied = cmd::text(cmd::field(&grant.config, "input")?, "kind")? != "text_layer";
    let mut owner_evidence = None;
    let (supplied_raw, file_identity) = if supplied {
        let identity = verify_acquired_file(
            &context,
            &grant.config,
            &source.payload_entry,
            deadline,
            cancelled,
        )?;
        let material = cmd::field(&grant.config, "material")?;
        if grant.operation.starts_with("text-layer.record-owner-") {
            let page = grant.operation == "text-layer.record-owner-page-ocr";
            let scope = cmd::field(&grant.config, "source_scope")?;
            let language = cmd::text(&grant.config, "language")?;
            let (raw, evidence) = if let Some(files) = retained {
                let raw = files
                    .get("content.txt")
                    .ok_or(SourceCommandError::Conflict(
                        "native owner OCR retained content absent",
                    ))?
                    .clone();
                if raw.len() as u64 != cmd::integer(material, "byte_size")?
                    || Digest256::of_bytes(&raw).to_hex() != cmd::text(material, "content_sha256")?
                    || std::str::from_utf8(&raw).is_err()
                {
                    return Err(SourceCommandError::Conflict(
                        "native owner OCR retained content differs",
                    ));
                }
                let mut evidence = BTreeMap::new();
                for name in [
                    "owner-ocr-receipt.json",
                    "owner-ocr-signature.sigstore.json",
                    "owner-ocr-signer.pub",
                ] {
                    evidence.insert(
                        name.to_owned(),
                        files
                            .get(name)
                            .ok_or(SourceCommandError::Conflict(
                                "native owner OCR retained evidence absent",
                            ))?
                            .clone(),
                    );
                }
                (raw, evidence)
            } else {
                let verified = source_text_owner_ocr::verify_initial(
                    material,
                    scope,
                    language,
                    page,
                    context.account_uid(),
                    owner_verification_deadline,
                    cancelled,
                )?;
                (verified.content, verified.evidence)
            };
            owner_evidence = Some(evidence);
            (Some(raw), Some(identity))
        } else {
            cmd::exact_keys(
                material,
                &[
                    "authority_ref",
                    "expires_at",
                    "content_ref",
                    "content_sha256",
                    "byte_size",
                    "access_allowed",
                    "reported_maker",
                    "provider_execution",
                ],
            )?;
            cmd::validate_expiry(cmd::text(material, "expires_at")?, &instant()?)?;
            if cmd::text(material, "authority_ref")?.trim().is_empty()
                || cmd::field(material, "access_allowed")? != &JsonValue::Bool(true)
                || cmd::text(material, "provider_execution")? != "not_observed"
                || !(1..=131_072).contains(&cmd::integer(material, "byte_size")?)
            {
                return Err(SourceCommandError::Denied(
                    "native derived supplied material grant",
                ));
            }
            let ref_name = cmd::text(material, "content_ref")?;
            if !ref_name.starts_with("ToS/source-witnesses/owner-local/")
                || ref_name.starts_with(
                    cmd::text(&grant.config, "source_path")?
                        .rsplit_once('/')
                        .ok_or(SourceCommandError::Invalid("native derived output home"))?
                        .0,
                )
            {
                return Err(SourceCommandError::Denied(
                    "native derived separate supplied content",
                ));
            }
            let raw = context.read(ref_name, 131_072, deadline, cancelled)?;
            if raw.len() as u64 != cmd::integer(material, "byte_size")?
                || Digest256::of_bytes(&raw).to_hex() != cmd::text(material, "content_sha256")?
                || std::str::from_utf8(&raw).is_err()
            {
                return Err(SourceCommandError::Conflict(
                    "native derived supplied content differs",
                ));
            }
            (Some(raw), Some(identity))
        }
    } else {
        (None, None)
    };
    let predecessor_text = source
        .predecessor_raw
        .as_deref()
        .map(|raw| {
            std::str::from_utf8(raw)
                .map_err(|_| SourceCommandError::Invalid("native derived predecessor UTF-8"))
        })
        .transpose()?;
    let supplied_text = supplied_raw
        .as_deref()
        .map(|raw| {
            std::str::from_utf8(raw)
                .map_err(|_| SourceCommandError::Invalid("native derived supplied UTF-8"))
        })
        .transpose()?;
    let output = build_derived_layer(
        &grant.config,
        &source.source_binding,
        source.predecessor.as_ref(),
        predecessor_text,
        supplied_text,
        deadline,
        cancelled,
    )?;
    let source_path = cmd::text(&grant.config, "source_path")?;
    checked_schema(
        worker,
        source_path,
        output
            .files
            .get("source-text-layer.v1.json")
            .ok_or(SourceCommandError::Invalid("native derived layer output"))?,
        LAYER_SCHEMA,
        deadline,
        cancelled,
    )?;
    let software_rows = selected_components(software, components, deadline, cancelled)?;
    let inputs = input_bytes(
        &context,
        &source,
        &file_identity,
        &software_rows,
        deadline,
        cancelled,
    )?;
    let dependencies = dependencies(&source, &inputs, inventory)?;
    let mut output = output;
    if let Some(evidence) = owner_evidence {
        output.files.extend(evidence);
    }
    output
        .files
        .insert("source-create-inputs.json".into(), inputs);
    if output
        .files
        .values()
        .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
        .is_none_or(|n| n > MAX_PACKAGE)
    {
        return Err(SourceCommandError::Unsupported(
            "native derived package byte budget",
        ));
    }
    context.verify_publication(&publication, deadline, cancelled)?;
    Ok(PreparedDerived {
        context,
        grant,
        publication,
        source,
        output,
        supplied_raw,
        file_identity,
        contracts,
        inventory,
        software_rows,
        owner_configuration,
        dependencies,
        owner_verification_deadline,
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
    owner_verification_deadline: Instant,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedDerived> {
    let (context, publication, grant) =
        select_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    prepare_selected(
        context,
        publication,
        grant,
        software,
        components,
        worker,
        None,
        exclude,
        owner_verification_deadline,
        deadline,
        cancelled,
    )
}

impl PreparedDerived {
    fn verify_stage_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        self.context.snapshot(deadline, cancelled)?;
        let now = OwnerTextDerivedSelection::select(
            &self.context,
            &self.grant.path,
            deadline,
            cancelled,
        )?;
        if now.raw != self.grant.raw || now.config != self.grant.config {
            return Err(SourceCommandError::Conflict(
                "native derived delegation changed",
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
        for (name, sha) in &self.contracts {
            if Digest256::of_bytes(&self.context.read(name, MAX_CONTRACT, deadline, cancelled)?)
                != *sha
            {
                return Err(SourceCommandError::Conflict(
                    "native derived selected contract changed",
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
                "native derived configuration changed",
            ));
        }
        recheck_selected_inputs(&self.context, &self.source.inputs, deadline, cancelled)?;
        let refs = selected_ids(&self.grant)?;
        if selected_identity_snapshot(
            &self.context,
            &refs.iter().map(String::as_str).collect::<Vec<_>>(),
            exclude,
            deadline,
            cancelled,
        )? != self.inventory
        {
            return Err(SourceCommandError::Conflict(
                "native derived identity inventory changed",
            ));
        }
        if let Some(identity) = &self.file_identity {
            let actual = verify_acquired_file(
                &self.context,
                &self.grant.config,
                &self.source.payload_entry,
                deadline,
                cancelled,
            )?;
            if &actual != identity {
                return Err(SourceCommandError::Conflict(
                    "native derived acquired File changed",
                ));
            }
        }
        if let Some(raw) = &self.supplied_raw {
            let material = cmd::field(&self.grant.config, "material")?;
            if self.grant.operation.starts_with("text-layer.record-owner-") {
                if raw.len() as u64 != cmd::integer(material, "byte_size")?
                    || Digest256::of_bytes(raw).to_hex() != cmd::text(material, "content_sha256")?
                {
                    return Err(SourceCommandError::Conflict(
                        "native owner OCR retained bytes differ from grant",
                    ));
                }
            } else {
                let current = self.context.read(
                    cmd::text(material, "content_ref")?,
                    131_072,
                    deadline,
                    cancelled,
                )?;
                if &current != raw {
                    return Err(SourceCommandError::Conflict(
                        "native derived supplied bytes changed",
                    ));
                }
            }
        }
        if selected_components(software, components, deadline, cancelled)? != self.software_rows {
            return Err(SourceCommandError::Conflict(
                "native derived selected software changed",
            ));
        }
        self.context
            .verify_publication(&self.publication, deadline, cancelled)?;
        self.context.snapshot(deadline, cancelled)?;
        Ok(())
    }
}

pub fn prepare_derived_text_layer_from_captures(
    context_path: &Path,
    grant_path: &Path,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeDerivedTextLayerPreview> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let owner_verification_deadline = deadline.min(Instant::now() + Duration::from_secs(30));
    let prepared = prepare(
        context_path,
        grant_path,
        cut,
        software,
        components,
        worker,
        None,
        owner_verification_deadline,
        deadline,
        cancelled,
    )?;
    finish_creation_worker(worker, deadline, cancelled)?;
    prepared.verify_current(software, components, None, deadline, cancelled)?;
    Ok(NativeDerivedTextLayerPreview {
        owner_configuration: prepared.owner_configuration,
        expected_dependencies: prepared.dependencies,
        source_path: cmd::text(&prepared.grant.config, "source_path")?.to_owned(),
    })
}

fn request_create(
    request: &JsonValue,
    prepared: &PreparedDerived,
    original: bool,
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
        || cmd::text(request, "operation")? != prepared.grant.operation
        || cmd::text(request, "command_id")?.is_empty()
        || cmd::text(request, "command_id")?.len() > 256
        || cmd::text(request, "expected_configuration")? != prepared.owner_configuration
        || original && cmd::text(request, "expected_dependencies")? != prepared.dependencies
        || cmd::field(request, "expected_source")? != &JsonValue::Null
        || cmd::field(request, "expected_revision")? != &JsonValue::Null
    {
        return Err(SourceCommandError::Conflict(
            "native derived request differs",
        ));
    }
    Ok(())
}

fn verify_retained(
    prepared: &PreparedDerived,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    evidence_path: Option<&Path>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let expected = expected_files(&prepared.grant.operation);
    if files.len() != expected.len()
        || files.keys().any(|name| !expected.contains(&name.as_str()))
        || prepared
            .output
            .files
            .iter()
            .any(|(name, raw)| files.get(name) != Some(raw))
        || files.get("source-create-request.json").map(Vec::as_slice)
            != Some(line(request)?.as_slice())
    {
        return Err(SourceCommandError::Conflict(
            "native derived retained package differs",
        ));
    }
    if prepared
        .grant
        .operation
        .starts_with("text-layer.record-owner-")
    {
        let material = cmd::field(&prepared.grant.config, "material")?;
        for (name, key) in [
            ("owner-ocr-receipt.json", "receipt_sha256"),
            ("owner-ocr-signature.sigstore.json", "signature_sha256"),
            ("owner-ocr-signer.pub", "public_key_sha256"),
        ] {
            let raw = files.get(name).ok_or(SourceCommandError::Conflict(
                "native owner OCR copied evidence absent",
            ))?;
            if Digest256::of_bytes(raw).to_hex() != cmd::text(material, key)? {
                return Err(SourceCommandError::Conflict(
                    "native owner OCR copied evidence differs",
                ));
            }
        }
        if let Some(evidence_path) = evidence_path {
            source_text_owner_ocr::verify_record(
                material,
                cmd::field(&prepared.grant.config, "source_scope")?,
                cmd::text(&prepared.grant.config, "language")?,
                prepared.grant.operation == "text-layer.record-owner-page-ocr",
                evidence_path,
                files,
                prepared.context.account_uid(),
                prepared.owner_verification_deadline,
                cancelled,
            )?;
        }
    }
    let receipt_raw =
        files
            .get("source-create-receipt.json")
            .ok_or(SourceCommandError::Conflict(
                "native derived receipt absent",
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
        || cmd::canonical(cmd::field(&receipt, "source")?)?
            != cmd::canonical(&cmd::reference(
                &prepared.output.layer,
                "layer_id",
                "layer_version",
            )?)?
        || cmd::canonical(cmd::field(&receipt, "files")?)?
            != cmd::canonical(&file_refs(
                files
                    .iter()
                    .filter(|(name, _)| name.as_str() != "source-create-receipt.json"),
            ))?
        || line(&receipt)? != *receipt_raw
    {
        return Err(SourceCommandError::Conflict(
            "native derived retained receipt differs",
        ));
    }
    cmd::validate_instant(cmd::text(&receipt, "recorded_at")?)?;
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native derived home"))?
        .0;
    let event_raw = files
        .get("source-create-provenance.jsonl")
        .ok_or(SourceCommandError::Conflict("native derived event absent"))?;
    let event = cmd::parse(event_raw)?;
    let procedure = format!(
        "exact-native-text-layer-{}",
        prepared
            .grant
            .operation
            .strip_prefix("text-layer.")
            .ok_or(SourceCommandError::Invalid("native derived operation"))?
    );
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
        )? != procedure
    {
        return Err(SourceCommandError::Conflict(
            "native derived event identity differs",
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
        .map_err(|_| SourceCommandError::Invalid("native derived event JSON"))?;
    if !tos_validation::provenance_rules::semantic_issues(&decoded, 128, deadline)
        .map_err(|_| SourceCommandError::Invalid("native derived event semantics"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid(
            "native derived event semantics",
        ));
    }
    let grant_raw =
        files
            .get("source-create-owner-configuration.json")
            .ok_or(SourceCommandError::Conflict(
                "native derived retained grant",
            ))?;
    let env_raw =
        files
            .get("source-create-environment.json")
            .ok_or(SourceCommandError::Conflict(
                "native derived retained environment",
            ))?;
    let method = cmd::field(&event, "method")?;
    let configuration = cmd::field(method, "configuration_binding")?;
    let environment = cmd::field(
        cmd::field(method, "environment")?,
        "environment_profile_binding",
    )?;
    let visibility = cmd::field(&event, "rights_and_visibility")?;
    let expected_event = if matches!(
        prepared.grant.operation.as_str(),
        "text-layer.record-transcription"
            | "text-layer.record-ocr"
            | "text-layer.record-owner-ocr"
            | "text-layer.record-owner-page-ocr"
    ) {
        "annotation"
    } else if prepared.grant.operation == "text-layer.normalize" {
        "normalization"
    } else {
        "correction"
    };
    if cmd::text(configuration, "ref")? != format!("{home}/source-create-owner-configuration.json")
        || cmd::text(environment, "ref")? != format!("{home}/source-create-environment.json")
        || cmd::text(cmd::field(&event, "activity")?, "event_type")? != expected_event
        || cmd::canonical(cmd::field(visibility, "rights_record_bindings")?)?
            != cmd::canonical(cmd::field(
                cmd::field(&prepared.grant.config, "derivation_access")?,
                "rights_record_refs",
            )?)?
        || cmd::text(visibility, "content_visibility")? != "local_only"
        || cmd::field(visibility, "publication_authorized")? != &JsonValue::Bool(false)
        || cmd::text(configuration, "sha256")? != Digest256::of_bytes(grant_raw).to_hex()
        || cmd::text(environment, "sha256")? != Digest256::of_bytes(env_raw).to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "native derived retained capture binding",
        ));
    }
    let selected = decoded["method"]["software_components"]
        .as_array()
        .ok_or(SourceCommandError::Invalid("native derived software rows"))?;
    if selected.len() != prepared.software_rows.len() + 1
        || selected[..prepared.software_rows.len()] != prepared.software_rows[..]
    {
        return Err(SourceCommandError::Conflict(
            "native derived retained software differs",
        ));
    }
    let inputs = cmd::parse(
        files
            .get("source-create-inputs.json")
            .ok_or(SourceCommandError::Conflict("native derived inputs absent"))?,
    )?;
    let runtime = cmd::text(&inputs, "runtime")?;
    let runner = selected
        .last()
        .ok_or(SourceCommandError::Invalid("native derived runner absent"))?;
    if runner["role"] != "serialization-runner"
        || runner["artifact_ref"] != "runtime:tos-native-executable"
        || format!(
            "sha256:{}",
            runner["artifact_sha256"]
                .as_str()
                .ok_or(SourceCommandError::Invalid("native derived runner SHA"))?
        ) != runtime
        || format!(
            "sha256:{}",
            cmd::text(&cmd::parse(env_raw)?, "runtime_artifact_sha256")?
        ) != runtime
    {
        return Err(SourceCommandError::Conflict(
            "native derived runtime differs",
        ));
    }
    let entities = cmd::field(&event, "entities")?;
    let expected_external = capture_entities(prepared)?
        .into_iter()
        .filter(|row| !row.0.starts_with(&format!("{home}/")))
        .collect::<Vec<_>>();
    let mut observed = BTreeSet::new();
    let mut external = Vec::new();
    for group in ["inputs", "outputs", "byproducts"] {
        for row in cmd::array(entities, group)? {
            active(deadline, cancelled)?;
            let reference = cmd::text(row, "entity_ref")?;
            if let Some(name) = reference.strip_prefix(&format!("{home}/")) {
                let extra_owner_input = prepared
                    .grant
                    .operation
                    .starts_with("text-layer.record-owner-")
                    && matches!(name, "content.txt" | "owner-ocr-receipt.json")
                    && group == "inputs";
                let expected_group = match name {
                    "source-create-request.json" => "inputs",
                    "source-create-environment.json" => "byproducts",
                    _ => "outputs",
                };
                let raw = files.get(name).ok_or(SourceCommandError::Conflict(
                    "native derived local entity absent",
                ))?;
                if group != expected_group && !extra_owner_input
                    || !observed.insert((group.to_owned(), name.to_owned()))
                    || cmd::integer(row, "size_bytes")? != raw.len() as u64
                    || cmd::text(row, "sha256")? != Digest256::of_bytes(raw).to_hex()
                    || extra_owner_input
                        && (cmd::text(row, "role")?
                            != if name == "content.txt" {
                                "authenticated-owner-ocr-result"
                            } else {
                                "authenticated-owner-ocr-execution-receipt"
                            }
                            || cmd::field(row, "fixity_verified")? != &JsonValue::Bool(true))
                {
                    return Err(SourceCommandError::Conflict(
                        "native derived local entity differs",
                    ));
                }
            } else if group == "inputs" {
                if cmd::text(row, "availability")? != "owner_local"
                    || cmd::text(row, "content_disclosure")? != "private_content"
                    || cmd::field(row, "fixity_verified")? != &JsonValue::Bool(true)
                {
                    return Err(SourceCommandError::Conflict(
                        "native derived external input posture",
                    ));
                }
                external.push((
                    reference.to_owned(),
                    cmd::text(row, "sha256")?.to_owned(),
                    cmd::integer(row, "size_bytes")?,
                    cmd::text(row, "role")?.to_owned(),
                    cmd::text(row, "media_type")?.to_owned(),
                ));
            } else {
                return Err(SourceCommandError::Conflict(
                    "native derived external output",
                ));
            }
        }
    }
    let mut expected_local = BTreeSet::new();
    for name in files.keys().filter(|name| {
        !matches!(
            name.as_str(),
            "source-create-provenance.jsonl" | "source-create-receipt.json"
        )
    }) {
        let group = match name.as_str() {
            "source-create-request.json" => "inputs",
            "source-create-environment.json" => "byproducts",
            _ => "outputs",
        };
        expected_local.insert((group.to_owned(), name.clone()));
    }
    if prepared
        .grant
        .operation
        .starts_with("text-layer.record-owner-")
    {
        for name in ["content.txt", "owner-ocr-receipt.json"] {
            expected_local.insert(("inputs".to_owned(), name.to_owned()));
        }
    }
    if observed != expected_local || external != expected_external {
        return Err(SourceCommandError::Conflict(
            "native derived event entity closure",
        ));
    }
    Ok(receipt)
}

fn capture_entities(
    prepared: &PreparedDerived,
) -> SourceCommandResult<Vec<(String, String, u64, String, String)>> {
    let scope = cmd::field(&prepared.grant.config, "source_scope")?;
    let refs = cmd::field(&prepared.grant.config, "source_record_refs")?;
    let item = cmd::text(refs, "item")?;
    let item_home = item
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("native derived Item home"))?
        .0;
    let relative = cmd::text(&prepared.source.payload_entry, "relative_path")?;
    let mut rows = Vec::new();
    if prepared.file_identity.is_some() {
        rows.push((
            format!("{item_home}/{relative}"),
            cmd::text(scope, "file_sha256")?.to_owned(),
            cmd::integer(&prepared.source.payload_entry, "byte_size")?,
            "exact-source-file-not-provider-input-execution".into(),
            cmd::text(&prepared.source.payload_entry, "media_type")?.to_owned(),
        ));
    }
    if let Some(prior) = &prepared.source.predecessor {
        let target = cmd::field(cmd::field(&prepared.grant.config, "input")?, "binding")?;
        let target = cmd::field(target, "text_layer")?;
        rows.push((
            cmd::text(target, "record_ref")?.to_owned(),
            cmd::text(target, "record_sha256")?.to_owned(),
            prepared
                .source
                .predecessor_record_raw
                .as_ref()
                .ok_or(SourceCommandError::Conflict(
                    "native derived predecessor raw absent",
                ))?
                .len() as u64,
            "exact-predecessor-record".into(),
            "application/json".into(),
        ));
        rows.push((
            cmd::text(cmd::field(prior, "representation")?, "content_ref")?.to_owned(),
            cmd::text(cmd::field(prior, "representation")?, "content_sha256")?.to_owned(),
            prepared
                .source
                .predecessor_raw
                .as_ref()
                .map_or(0, |raw| raw.len() as u64),
            "exact-predecessor-representation".into(),
            "text/plain; charset=utf-8".into(),
        ));
    }
    if let Some(raw) = &prepared.supplied_raw {
        if prepared
            .grant
            .operation
            .starts_with("text-layer.record-owner-")
        {
            let home = cmd::text(&prepared.grant.config, "source_path")?
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("native owner OCR output home"))?
                .0;
            rows.push((
                format!("{home}/content.txt"),
                Digest256::of_bytes(raw).to_hex(),
                raw.len() as u64,
                "authenticated-owner-ocr-result".into(),
                "text/plain; charset=utf-8".into(),
            ));
            let receipt = prepared.output.files.get("owner-ocr-receipt.json").ok_or(
                SourceCommandError::Conflict("native owner OCR copied receipt absent"),
            )?;
            rows.push((
                format!("{home}/owner-ocr-receipt.json"),
                Digest256::of_bytes(receipt).to_hex(),
                receipt.len() as u64,
                "authenticated-owner-ocr-execution-receipt".into(),
                "application/json".into(),
            ));
        } else {
            let material = cmd::field(&prepared.grant.config, "material")?;
            let method = cmd::text(cmd::field(&prepared.grant.config, "policy")?, "method")?;
            rows.push((
                cmd::text(material, "content_ref")?.to_owned(),
                Digest256::of_bytes(raw).to_hex(),
                raw.len() as u64,
                format!("supplied-unverified-{method}-result"),
                "text/plain; charset=utf-8".into(),
            ));
        }
    }
    Ok(rows)
}

pub fn execute_derived_text_layer_from_captures(
    context_path: &Path,
    grant_path: &Path,
    request: &JsonValue,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeDerivedTextLayerResult> {
    let deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let owner_verification_deadline = deadline.min(Instant::now() + Duration::from_secs(30));
    let (context, publication, grant) =
        select_owner(context_path, grant_path, cut, worker, deadline, cancelled)?;
    let path = cmd::text(&grant.config, "source_path")?.to_owned();
    let target = context.private_new_package_target(&path)?;
    let expected = expected_files(&grant.operation);
    let custody = observe_private_text(&context, &path, request, expected, deadline, cancelled)?;
    let exclude = if matches!(custody, PrivateTextCustody::Published(_)) {
        Some(target.as_path())
    } else {
        None
    };
    let mut prepared = prepare_selected(
        context,
        publication,
        grant,
        software,
        components,
        worker,
        match &custody {
            PrivateTextCustody::Absent => None,
            PrivateTextCustody::Published(files) | PrivateTextCustody::Pending(files) => {
                Some(files)
            }
        },
        exclude,
        owner_verification_deadline,
        deadline,
        cancelled,
    )?;
    request_create(
        request,
        &prepared,
        matches!(custody, PrivateTextCustody::Absent),
    )?;
    match custody {
        PrivateTextCustody::Published(files) => {
            let receipt = verify_retained(
                &prepared,
                request,
                &files,
                Some(&target),
                worker,
                deadline,
                cancelled,
            )?;
            finish_creation_worker(worker, deadline, cancelled)?;
            let _locks = PrivateTextLocks::acquire(&prepared.context, deadline, cancelled)?;
            prepared.verify_stage_current(deadline, cancelled)?;
            if !matches!(observe_private_text(&prepared.context,&path,request,expected,deadline,cancelled)?,
                PrivateTextCustody::Published(ref current) if current==&files)
            {
                return Err(SourceCommandError::Conflict(
                    "native derived retained package changed",
                ));
            }
            prepared.verify_current(software, components, Some(&target), deadline, cancelled)?;
            Ok(NativeDerivedTextLayerResult {
                receipt,
                replayed: true,
                grants_admission: false,
            })
        }
        PrivateTextCustody::Pending(files) => {
            let receipt = verify_retained(
                &prepared, request, &files, None, worker, deadline, cancelled,
            )?;
            finish_creation_worker(worker, deadline, cancelled)?;
            prepared.verify_current(software, components, None, deadline, cancelled)?;
            let owner_ocr = prepared
                .grant
                .operation
                .starts_with("text-layer.record-owner-");
            let mut staged = |stage: &Path| {
                source_text_owner_ocr::verify_record(
                    cmd::field(&prepared.grant.config, "material")?,
                    cmd::field(&prepared.grant.config, "source_scope")?,
                    cmd::text(&prepared.grant.config, "language")?,
                    prepared.grant.operation == "text-layer.record-owner-page-ocr",
                    stage,
                    &files,
                    prepared.context.account_uid(),
                    prepared.owner_verification_deadline,
                    cancelled,
                )
            };
            publish_private_text(
                &prepared.context,
                &path,
                request,
                &files,
                || prepared.verify_stage_current(deadline, cancelled),
                || prepared.verify_current(software, components, None, deadline, cancelled),
                if owner_ocr { Some(&mut staged) } else { None },
                deadline,
                cancelled,
            )?;
            Ok(NativeDerivedTextLayerResult {
                receipt,
                replayed: true,
                grants_admission: false,
            })
        }
        PrivateTextCustody::Absent => {
            let home = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("native derived home"))?
                .0;
            let event_id = cmd::text(
                cmd::field(&prepared.grant.config, "identities")?,
                "provenance_event_id",
            )?;
            let rights = cmd::field(
                cmd::field(&prepared.grant.config, "derivation_access")?,
                "rights_record_refs",
            )?;
            let observed_at = instant()?;
            let entities = capture_entities(&prepared)?.into_iter().map(|(reference,sha,size,role,media)|json!({
                "entity_ref":reference,"role":role,"sha256":sha,"size_bytes":size,
                "media_type":media,"availability":"owner_local","content_disclosure":"private_content",
                "fixity_verified":true,"fixity_verified_at":observed_at})).collect();
            capture_derived_text_layer(
                &prepared.grant.operation,
                request,
                event_id,
                home,
                &path,
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
                    .ok_or(SourceCommandError::Invalid("native derived event output"))?,
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
                ("source_path", cmd::string(&path)),
                (
                    "source",
                    cmd::reference(&prepared.output.layer, "layer_id", "layer_version")?,
                ),
                ("dependencies", cmd::string(&prepared.dependencies)),
                ("files", file_refs(prepared.output.files.iter())),
                ("grants_admission", JsonValue::Bool(false)),
            ]);
            let receipt_raw = line(&receipt)?;
            drop(receipt);
            let receipt = cmd::parse(&receipt_raw)?;
            prepared
                .output
                .files
                .insert("source-create-receipt.json".into(), receipt_raw);
            if prepared
                .output
                .files
                .values()
                .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
                .is_none_or(|n| n > MAX_PACKAGE)
            {
                return Err(SourceCommandError::Unsupported(
                    "native derived complete package budget",
                ));
            }
            finish_creation_worker(worker, deadline, cancelled)?;
            prepared.verify_current(software, components, None, deadline, cancelled)?;
            let owner_ocr = prepared
                .grant
                .operation
                .starts_with("text-layer.record-owner-");
            let mut staged = |stage: &Path| {
                source_text_owner_ocr::verify_record(
                    cmd::field(&prepared.grant.config, "material")?,
                    cmd::field(&prepared.grant.config, "source_scope")?,
                    cmd::text(&prepared.grant.config, "language")?,
                    prepared.grant.operation == "text-layer.record-owner-page-ocr",
                    stage,
                    &prepared.output.files,
                    prepared.context.account_uid(),
                    prepared.owner_verification_deadline,
                    cancelled,
                )
            };
            publish_private_text(
                &prepared.context,
                &path,
                request,
                &prepared.output.files,
                || prepared.verify_stage_current(deadline, cancelled),
                || prepared.verify_current(software, components, None, deadline, cancelled),
                if owner_ocr { Some(&mut staged) } else { None },
                deadline,
                cancelled,
            )?;
            Ok(NativeDerivedTextLayerResult {
                receipt,
                replayed: false,
                grants_admission: false,
            })
        }
    }
}
