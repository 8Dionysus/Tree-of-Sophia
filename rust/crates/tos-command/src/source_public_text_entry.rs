//! Whole public project Text construction. The supplied-carrier assembly here
//! follows the independent public authority gate; it is not an initial private
//! layer grant or a visibility conversion.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_public_text_proposal::{build_public_layer, build_public_text_unit_packet};
use crate::source_text_layer_entry::line;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};

fn value(v: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&crate::source_command::canonical(v)?)
        .map_err(|_| SourceCommandError::Invalid("public Text JSON"))
}
fn foundation(v: &Value) -> SourceCommandResult<JsonValue> {
    crate::source_command::parse(
        &serde_json::to_vec(v).map_err(|_| SourceCommandError::Invalid("public Text JSON"))?,
    )
}

pub(crate) struct PublicTextAssembly {
    pub(crate) packet: JsonValue,
    pub(crate) bindings: JsonValue,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
}

/// Assemble both immutable carriers only after the caller has authenticated
/// the source, authority, identity inventory and current protected selection.
fn assemble(
    config: &JsonValue,
    authority: &JsonValue,
    plan: &JsonValue,
    original: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PublicTextAssembly> {
    let c = value(config)?;
    let a = value(authority)?;
    let source_path = c["source_path"]
        .as_str()
        .ok_or(SourceCommandError::Invalid("public Text target"))?;
    let base = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("public Text target"))?
        .0;
    let plan_bytes = line(plan)?;
    let refs = foundation(&json!({
        "layer_ref":format!("{base}/source-text-layer.v1.json"),
        "anchor_ref":format!("{base}/source-anchor.v2.json"),
        "content_ref":format!("{base}/content.txt"),
        "policy_ref":format!("{base}/extraction-policy.json"),
        "configuration_ref":format!("{base}/construction-plan.json"),
        "configuration_sha256":Digest256::of_bytes(&plan_bytes).to_hex(),
        "source_ref":c["source"]["ref"]
    }))?;
    let text = std::str::from_utf8(original)
        .map_err(|_| SourceCommandError::Invalid("public Text strict UTF-8"))?;
    let built = build_public_layer(text, config, &refs, deadline, cancelled)?;
    if built.content.len()
        > c["limits"]["max_output_bytes"]
            .as_u64()
            .ok_or(SourceCommandError::Invalid("public Text output cap"))? as usize
        || a["output_scope"]["content_sha256"] != Digest256::of_bytes(&built.content).to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "public Text authorized output differs",
        ));
    }
    let layer_bytes = line(&built.layer)?;
    let layer_binding = json!({"schema_version":"tos_native_text_layer_binding_v1","text_layer":{
        "record_ref":format!("{base}/source-text-layer.v1.json"),"record_sha256":Digest256::of_bytes(&layer_bytes).to_hex(),
        "layer_id":c["identities"]["layer_id"],"layer_version":1},"source_record_refs":c["source_record_refs"]});
    let exact = std::str::from_utf8(&built.content)
        .map_err(|_| SourceCommandError::Invalid("public Text strict UTF-8"))?;
    let mut unit_config = c["identities"].clone();
    let unit_object = unit_config
        .as_object_mut()
        .ok_or(SourceCommandError::Invalid("public Text identities"))?;
    for name in ["scheme", "method"] {
        unit_object.insert(name.into(), c["unit_proposal"][name].clone());
    }
    unit_object.insert("principal_id".into(), c["principal_id"].clone());
    unit_object.insert("allowed_text_scope".into(), json!({"start":0,"end":exact.chars().count(),"position_unit":"unicode_code_point","interval":"half_open"}));
    let (packet, packet_bytes) = build_public_text_unit_packet(
        &built.layer,
        &foundation(&layer_binding)?,
        exact,
        &foundation(&unit_config)?,
        &foundation(&c["unit_proposal"])?,
        deadline,
        cancelled,
    )?;
    let p = value(&packet)?;
    let bindings = p["units"].as_array().ok_or(SourceCommandError::Invalid("public Text units"))?.iter().map(|unit| json!({
        "schema_version":"tos_native_text_unit_binding_v1","packet_ref":source_path,"packet_sha256":Digest256::of_bytes(&packet_bytes).to_hex(),
        "packet_id":c["identities"]["packet_id"],"packet_version":1,"segmentation_id":c["identities"]["segmentation_id"],"segmentation_version":1,
        "unit_id":unit["unit_id"],"unit_version":1,"ordered_anchor_refs":unit["ordered_anchor_refs"],"text_layer":layer_binding["text_layer"],"source_record_refs":c["source_record_refs"]
    })).collect::<Vec<_>>();
    let bindings =
        foundation(&json!({"schema_version":"tos_native_unit_bindings_v1","bindings":bindings}))?;
    let files = BTreeMap::from([
        ("source-text-layer.v1.json".into(), layer_bytes),
        ("source-anchor.v2.json".into(), line(&built.anchor)?),
        ("content.txt".into(), built.content),
        ("extraction-policy.json".into(), line(&built.policy)?),
        ("construction-plan.json".into(), plan_bytes),
        ("source-text-unit.v1.json".into(), packet_bytes),
        ("native-bindings.json".into(), line(&bindings)?),
    ]);
    Ok(PublicTextAssembly {
        packet,
        bindings,
        files,
    })
}

use crate::source_command as cmd;
use crate::source_public_text_owner::{PublicNativeTextSelection, PublicTextInputCache};
use crate::source_serialization::{
    capture_public_project_text, executable, instant, selected_components,
};
use crate::source_sign_native::{
    NativeReadKind, NativeReadScope, SignNativeRead, resolve_bindings,
    resolve_public_text_authority,
};
use crate::source_text_identity::selected_public_identity_snapshot;
use crate::source_text_layer_entry::{checked_schema, file_refs};
use crate::source_text_private_store::{
    PrivateTextCustody, PrivateTextLocks, observe_public_text, public_recovery_request,
    publish_public_text,
};
use std::path::Path;
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

const FILES: [&str; 12] = [
    "source-text-layer.v1.json",
    "source-anchor.v2.json",
    "content.txt",
    "extraction-policy.json",
    "construction-plan.json",
    "source-text-unit.v1.json",
    "native-bindings.json",
    "source-create-inputs.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];

struct Reader<'a> {
    selection: &'a PublicNativeTextSelection,
    cache: &'a mut PublicTextInputCache,
    overlay: Option<(&'a str, &'a BTreeMap<String, Vec<u8>>)>,
}
impl SignNativeRead for Reader<'_> {
    fn read(
        &mut self,
        reference: &str,
        _: NativeReadKind,
        max: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if let Some((home, files)) = self.overlay {
            if let Some(name) = reference
                .strip_prefix(home)
                .and_then(|tail| tail.strip_prefix('/'))
            {
                if let Some(raw) = files.get(name) {
                    if raw.len() > max {
                        return Err(SourceCommandError::Unsupported(
                            "public Text proposed read cap",
                        ));
                    }
                    return Ok(raw.clone());
                }
            }
        }
        self.cache
            .read(self.selection, reference, None, max, deadline, cancelled)
    }
    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.cache
            .verify_current(self.selection, deadline, cancelled)
    }
    fn owner_local(&self, _: &str) -> SourceCommandResult<bool> {
        Ok(false)
    }
}

struct Prepared {
    assembly: PublicTextAssembly,
    cache: PublicTextInputCache,
    dependencies: String,
    inventory: Digest256,
    software_rows: Vec<Value>,
    runtime: Digest256,
}
fn identities(config: &JsonValue) -> SourceCommandResult<Vec<String>> {
    let ids = cmd::field(config, "identities")?;
    let mut result = Vec::new();
    for key in [
        "layer_id",
        "anchor_id",
        "passage_id",
        "provenance_event_id",
        "packet_id",
        "scheme_id",
        "segmentation_id",
        "scope_anchor_ref",
    ] {
        result.push(cmd::text(ids, key)?.to_owned());
    }
    for slot in cmd::array(ids, "unit_slots")? {
        for key in ["unit_id", "anchor_ref"] {
            result.push(cmd::text(slot, key)?.to_owned());
        }
    }
    for id in cmd::array(ids, "gap_anchor_refs")? {
        result.push(
            id.as_str()
                .ok_or(SourceCommandError::Invalid("public Text gap identity"))?
                .to_owned(),
        );
    }
    Ok(result)
}
fn inventory(
    selection: &PublicNativeTextSelection,
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
    let ids = identities(&selection.config)?;
    selected_public_identity_snapshot(
        selection,
        &ids.iter().map(String::as_str).collect::<Vec<_>>(),
        exclude,
        deadline,
        cancelled,
    )
}
fn prepare(
    selection: &PublicNativeTextSelection,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    exclude: Option<&Path>,
    retained_inventory: Option<Digest256>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Prepared> {
    let mut cache = PublicTextInputCache::new();
    let authority = resolve_public_text_authority(
        &mut Reader {
            selection,
            cache: &mut cache,
            overlay: None,
        },
        worker,
        &selection.config,
        deadline,
        cancelled,
    )?;
    let live_inventory = inventory(selection, exclude, deadline, cancelled)?;
    let inventory = retained_inventory.unwrap_or(live_inventory);
    let source = cmd::field(&selection.config, "source")?;
    // Affirmative authority and identity gates have completed before this read.
    let original = cache.read(
        selection,
        cmd::text(source, "ref")?,
        Some(cmd::text(source, "sha256")?),
        usize::try_from(cmd::integer(
            cmd::field(&selection.config, "limits")?,
            "max_source_bytes",
        )?)
        .map_err(|_| SourceCommandError::Invalid("public Text source budget"))?,
        deadline,
        cancelled,
    )?;
    if original.len() as u64 != cmd::integer(source, "byte_size")? {
        return Err(SourceCommandError::Conflict(
            "public Text original byte count",
        ));
    }
    let mut assembly = assemble(
        &selection.config,
        &authority.authority,
        &selection.public_plan()?,
        &original,
        deadline,
        cancelled,
    )?;
    let source_path = cmd::text(&selection.config, "source_path")?;
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("public Text package"))?
        .0;
    let rows = cmd::array(&assembly.bindings, "bindings")?;
    let closure = resolve_bindings(
        &mut Reader {
            selection,
            cache: &mut cache,
            overlay: Some((home, &assembly.files)),
        },
        worker,
        rows,
        NativeReadScope::PublicContent,
        deadline,
        cancelled,
    )?;
    if closure.summaries.iter().any(|summary| {
        cmd::field(summary, "public_content_available").ok() != Some(&JsonValue::Bool(true))
    }) {
        return Err(SourceCommandError::Denied(
            "public Text genuine public closure",
        ));
    }
    let software_rows = selected_components(software, components, deadline, cancelled)?;
    let runtime = executable(deadline, cancelled)?;
    let input_digests = cache
        .files
        .iter()
        .map(|(name, bytes)| (name.clone(), json!(Digest256::of_bytes(bytes).to_hex())))
        .collect::<serde_json::Map<_, _>>();
    // Native producer bytes are authenticated through the software capture,
    // distinct from maintained Python reference fixtures.
    let captured = foundation(
        &json!({"schema_version":"tos_public_native_construction_inputs_v1","inputs":input_digests,"identity_inventory":inventory.to_prefixed(),"runtime":runtime.to_prefixed(),"implementation":software_rows,"source_files_verified":true,"original_payload_opened":false,"source_authorship_authenticated":false,"assessment_performed":false}),
    )?;
    let dependencies = Digest256::of_bytes(&cmd::canonical(&captured)?).to_prefixed();
    assembly
        .files
        .insert("source-create-inputs.json".into(), line(&captured)?);
    cache.verify_current(selection, deadline, cancelled)?;
    Ok(Prepared {
        assembly,
        cache,
        dependencies,
        inventory: live_inventory,
        software_rows,
        runtime,
    })
}
impl Prepared {
    fn current(
        &self,
        selection: &PublicNativeTextSelection,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        exclude: Option<&Path>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.cache.verify_current(selection, deadline, cancelled)?;
        if inventory(selection, exclude, deadline, cancelled)? != self.inventory
            || selected_components(software, components, deadline, cancelled)? != self.software_rows
            || executable(deadline, cancelled)? != self.runtime
        {
            return Err(SourceCommandError::Conflict(
                "public Text current identity or implementation",
            ));
        }
        Ok(())
    }
}
fn check_request(
    selection: &PublicNativeTextSelection,
    request: &JsonValue,
    dependencies: &str,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        request,
        &[
            "schema_version",
            "operation",
            "command_id",
            "expected_configuration",
            "expected_dependencies",
            "expected_source",
            "expected_revision",
        ],
    )?;
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1"
        || cmd::text(request, "operation")? != "native-text.create"
        || cmd::text(request, "command_id")?.is_empty()
        || cmd::text(request, "command_id")?.chars().count() > 256
    {
        return Err(SourceCommandError::Invalid(
            "public Text retained command request",
        ));
    }
    if cmd::text(request, "expected_configuration")?
        != selection.configuration_digest().to_prefixed()
        || cmd::text(request, "expected_dependencies")? != dependencies
        || cmd::field(request, "expected_source")? != &JsonValue::Null
        || cmd::field(request, "expected_revision")? != &JsonValue::Null
    {
        return Err(SourceCommandError::Conflict(
            "public Text exact prepared request",
        ));
    }
    Ok(())
}
fn verify_retained(
    selection: &PublicNativeTextSelection,
    request: &JsonValue,
    files: &BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    exclude: Option<&Path>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, JsonValue)> {
    if files.len() != FILES.len() || FILES.iter().any(|name| !files.contains_key(*name)) {
        return Err(SourceCommandError::Conflict(
            "public Text retained file closure",
        ));
    }
    let receipt = cmd::parse(&files["source-create-receipt.json"])?;
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
    let captured = cmd::parse(&files["source-create-inputs.json"])?;
    let retained = Digest256::from_prefixed(cmd::text(&captured, "identity_inventory")?)
        .map_err(|_| SourceCommandError::Invalid("public Text retained inventory"))?;
    let prepared = prepare(
        selection,
        software,
        components,
        worker,
        exclude,
        Some(retained),
        deadline,
        cancelled,
    )?;
    check_request(selection, request, &prepared.dependencies)?;
    if cmd::text(&receipt, "schema_version")? != "tos_local_source_create_receipt_v1"
        || cmd::field(&receipt, "command_id")? != cmd::field(request, "command_id")?
        || cmd::text(&receipt, "request_digest")?
            != Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed()
        || cmd::field(&receipt, "principal_id")? != cmd::field(&selection.config, "principal_id")?
        || cmd::field(&receipt, "authority_ref")? != cmd::field(&selection.config, "authority_ref")?
        || cmd::text(&receipt, "owner_configuration")?
            != selection.configuration_digest().to_prefixed()
        || cmd::field(&receipt, "source_path")? != cmd::field(&selection.config, "source_path")?
        || cmd::field(&receipt, "source")?
            != &cmd::reference(&prepared.assembly.packet, "packet_id", "packet_version")?
        || cmd::text(&receipt, "dependencies")? != prepared.dependencies
        || cmd::field(&receipt, "grants_admission")? != &JsonValue::Bool(false)
        || cmd::canonical(cmd::field(&receipt, "files")?)?
            != cmd::canonical(&file_refs(
                files
                    .iter()
                    .filter(|(name, _)| name.as_str() != "source-create-receipt.json"),
            ))?
        || cmd::canonical(&cmd::parse(&files["source-create-request.json"])?)?
            != cmd::canonical(request)?
        || prepared
            .assembly
            .files
            .iter()
            .any(|(name, bytes)| files.get(name) != Some(bytes))
    {
        return Err(SourceCommandError::Conflict(
            "public Text retained exact request or output",
        ));
    }
    // Retained timestamp and environment are execution evidence, not rebuilt.
    cmd::validate_instant(cmd::text(&receipt, "recorded_at")?)?;
    if line(&receipt)? != files["source-create-receipt.json"] {
        return Err(SourceCommandError::Conflict(
            "public Text retained receipt bytes",
        ));
    }
    let source_path = cmd::text(&selection.config, "source_path")?;
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("public Text retained home"))?
        .0;
    let mut rebuilt = prepared.assembly.files.clone();
    crate::source_serialization::restore_creation_capture(
        request,
        cmd::text(
            cmd::field(&selection.config, "identities")?,
            "provenance_event_id",
        )?,
        home,
        &mut rebuilt,
        files,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let event = cmd::parse(&files["source-create-provenance.jsonl"])?;
    if cmd::field(&event, "event_id")?
        != cmd::field(
            cmd::field(&selection.config, "identities")?,
            "provenance_event_id",
        )?
    {
        return Err(SourceCommandError::Conflict(
            "public Text retained event identity",
        ));
    }
    if cmd::text(
        cmd::field(cmd::field(&event, "method")?, "procedure")?,
        "name",
    )? != "tos.project-authored.utf8-range.v1"
    {
        return Err(SourceCommandError::Conflict(
            "public Text retained procedure",
        ));
    }
    let decoded = value(&event)?;
    if !tos_validation::provenance_rules::semantic_issues(&decoded, 128, deadline)
        .map_err(|_| SourceCommandError::Invalid("public Text event semantics"))?
        .is_empty()
    {
        return Err(SourceCommandError::Invalid("public Text event semantics"));
    }
    checked_schema(
        worker,
        "source-create-provenance.jsonl",
        &files["source-create-provenance.jsonl"],
        "ToS/contracts/provenance-event-v2.schema.json",
        deadline,
        cancelled,
    )?;
    Ok((receipt, prepared.assembly.bindings))
}

/// The complete public project Text route. The caller supplies the selected
/// original schema worker and authenticated producer capture; no build occurs.
pub(crate) fn run(
    selection: &PublicNativeTextSelection,
    request: &JsonValue,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let max_seconds = cmd::integer(cmd::field(&selection.config, "limits")?, "max_seconds")?;
    let deadline = deadline.min(
        Instant::now()
            .checked_add(std::time::Duration::from_secs(max_seconds))
            .ok_or(SourceCommandError::Invalid(
                "public Text execution deadline",
            ))?,
    );
    let operation = cmd::text(request, "operation")?;
    let config = value(&selection.config)?;
    let source_path = cmd::text(&selection.config, "source_path")?;
    let configuration = selection.configuration_digest().to_prefixed();
    selection.verify_current(deadline, cancelled)?;
    if operation == "describe" {
        return foundation(
            &json!({"status":"ready","configuration":configuration,"source_path":source_path,"operation":"native-text.create","grants_admission":false,"external_publication_authorized":false}),
        );
    }
    if operation == "prepare-create" {
        let p = prepare(
            selection, software, components, worker, None, None, deadline, cancelled,
        )?;
        return foundation(
            &json!({"configuration":configuration,"dependencies":p.dependencies,"source":value(&cmd::reference(&p.assembly.packet,"packet_id","packet_version")?)?,"expected_source":null,"expected_revision":null,"native_bindings":value(cmd::field(&p.assembly.bindings,"bindings")?)?,"grants_admission":false}),
        );
    }
    let id = cmd::text(request, "command_id")?;
    if id.is_empty() || id.chars().count() > 256 {
        return Err(SourceCommandError::Invalid("public Text command identity"));
    }
    let target = selection.new_package_target(source_path)?;
    if operation == "inspect-recovery" {
        let Some((original, files)) =
            public_recovery_request(selection, source_path, id, deadline, cancelled)?
        else {
            return foundation(&json!({"status":"no_retained_control","grants_admission":false}));
        };
        let committed = match observe_public_text(
            selection,
            source_path,
            &original,
            &FILES,
            deadline,
            cancelled,
        )? {
            PrivateTextCustody::Published(bytes) => {
                if bytes != files {
                    return Err(SourceCommandError::Conflict(
                        "public Text installed recovery differs",
                    ));
                }
                true
            }
            _ => false,
        };
        verify_retained(
            selection,
            &original,
            &files,
            software,
            components,
            worker,
            if committed {
                Some(target.as_path())
            } else {
                None
            },
            deadline,
            cancelled,
        )?;
        return foundation(
            &json!({"status":if committed {"committed"} else {"retained_plan"},"resume_operation":"native-text.create","file_count":files.len(),"grants_admission":false}),
        );
    }
    if operation != "native-text.create" {
        return Err(SourceCommandError::Denied("public Text selected operation"));
    }
    let locks = PrivateTextLocks::acquire(selection, deadline, cancelled)?;
    let result = |receipt: &JsonValue,
                  bindings: &JsonValue,
                  replayed: bool|
     -> SourceCommandResult<JsonValue> {
        foundation(
            &json!({"status":if replayed {"replayed"} else {"created"},"source":value(cmd::field(receipt,"source")?)?,"source_path":source_path,"receipt_digest":Digest256::of_bytes(&cmd::canonical(receipt)?).to_prefixed(),"native_bindings":value(cmd::field(bindings,"bindings")?)?,"grants_admission":false,"external_publication_authorized":false}),
        )
    };
    match observe_public_text(selection, source_path, request, &FILES, deadline, cancelled)? {
        PrivateTextCustody::Published(files) => {
            let (receipt, bindings) = verify_retained(
                selection,
                request,
                &files,
                software,
                components,
                worker,
                Some(&target),
                deadline,
                cancelled,
            )?;
            return result(&receipt, &bindings, true);
        }
        PrivateTextCustody::Pending(files) => {
            let (receipt, bindings) = verify_retained(
                selection, request, &files, software, components, worker, None, deadline, cancelled,
            )?;
            let p = prepare(
                selection, software, components, worker, None, None, deadline, cancelled,
            )?;
            drop(locks);
            publish_public_text(
                selection,
                source_path,
                request,
                &files,
                || selection.verify_current(deadline, cancelled),
                || p.current(selection, software, components, None, deadline, cancelled),
                None,
                deadline,
                cancelled,
            )?;
            return result(&receipt, &bindings, false);
        }
        PrivateTextCustody::Absent => (),
    }
    let mut p = prepare(
        selection, software, components, worker, None, None, deadline, cancelled,
    )?;
    check_request(selection, request, &p.dependencies)?;
    let home = source_path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("public Text home"))?
        .0;
    let started = instant()?;
    let inputs=p.cache.files.iter().map(|(reference,raw)|json!({"entity_ref":reference,"role":"exact-public-construction-input","sha256":Digest256::of_bytes(raw).to_hex(),"size_bytes":raw.len(),"media_type":if reference==config["source"]["ref"].as_str().unwrap_or("") {config["source"]["media_type"].as_str().unwrap_or("application/octet-stream")} else {"application/octet-stream"},"availability":"tracked","content_disclosure":"public_content","fixity_verified":true,"fixity_verified_at":started})).collect();
    capture_public_project_text(
        request,
        cmd::text(
            cmd::field(&selection.config, "identities")?,
            "provenance_event_id",
        )?,
        home,
        source_path,
        cmd::field(&selection.config, "rights_record_refs")?,
        &foundation(&json!([config["publication_authority"]]))?,
        inputs,
        &mut p.assembly.files,
        software,
        components,
        deadline,
        cancelled,
    )?;
    checked_schema(
        worker,
        "source-create-provenance.jsonl",
        &p.assembly.files["source-create-provenance.jsonl"],
        "ToS/contracts/provenance-event-v2.schema.json",
        deadline,
        cancelled,
    )?;
    let receipt = foundation(
        &json!({"schema_version":"tos_local_source_create_receipt_v1","command_id":id,"request_digest":Digest256::of_bytes(&cmd::canonical(request)?).to_prefixed(),"principal_id":config["principal_id"],"authority_ref":config["authority_ref"],"owner_configuration":configuration,"recorded_at":instant()?,"source_path":source_path,"source":value(&cmd::reference(&p.assembly.packet,"packet_id","packet_version")?)?,"dependencies":p.dependencies,"files":value(&file_refs(p.assembly.files.iter()))?,"grants_admission":false}),
    )?;
    p.assembly
        .files
        .insert("source-create-receipt.json".into(), line(&receipt)?);
    if p.assembly.files.len() != FILES.len()
        || p.assembly
            .files
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|n| n > 2 * 1024 * 1024)
    {
        return Err(SourceCommandError::Unsupported(
            "public Text complete package cap",
        ));
    }
    p.current(selection, software, components, None, deadline, cancelled)?;
    drop(locks);
    publish_public_text(
        selection,
        source_path,
        request,
        &p.assembly.files,
        || selection.verify_current(deadline, cancelled),
        || p.current(selection, software, components, None, deadline, cancelled),
        None,
        deadline,
        cancelled,
    )?;
    result(&receipt, &p.assembly.bindings, false)
}
