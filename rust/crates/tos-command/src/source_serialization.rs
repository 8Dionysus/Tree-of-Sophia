//! In-process serialization observation for maintained source creation.
//! No caller-supplied event, executable digest or claimed producer is accepted.
//! Selected source components prove bytes, not their relationship to this ELF.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tos_foundation::{Digest256, Digest256Hasher, JsonValue};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(SourceCommandError::Denied(
            "native serialization cancelled or expired",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn instant() -> SourceCommandResult<String> {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SourceCommandError::Invalid("native clock predates Unix epoch"))?;
    // Gregorian civil date from positive Unix days; timestamps do not confer
    // clock trust or authorization. Preserve nanoseconds of the observed clock.
    let z = i64::try_from(t.as_secs() / 86400)
        .map_err(|_| SourceCommandError::Invalid("native clock range"))?
        + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    if !(1970..=9999).contains(&year) {
        return Err(SourceCommandError::Invalid("native clock UTC year"));
    }
    let seconds = t.as_secs() % 86400;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:09}Z",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        t.subsec_nanos()
    ))
}

fn executable(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<Digest256> {
    // Kernel-owned /proc/self/exe deliberately addresses the running image,
    // including a deleted inode. Never open a request-selected runtime path.
    let mut file = File::open("/proc/self/exe")
        .map_err(|_| SourceCommandError::Unsupported("running native executable unavailable"))?;
    let before = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("native executable metadata"))?;
    if !before.is_file() || before.len() == 0 {
        return Err(SourceCommandError::Invalid(
            "running native executable metadata",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        active(deadline, cancelled)?;
        let n = file
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Invalid("native executable read"))?;
        if n == 0 {
            break;
        }
        if total == 0 && !buffer[..n].starts_with(b"\x7fELF") {
            return Err(SourceCommandError::Invalid(
                "running native image is not ELF",
            ));
        }
        total = total
            .checked_add(n as u64)
            .ok_or(SourceCommandError::Invalid(
                "running executable length overflow",
            ))?;
        if total > before.len() {
            return Err(SourceCommandError::Conflict("running executable grew"));
        }
        hasher.update(&buffer[..n]);
    }
    active(deadline, cancelled)?;
    let after = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("native executable readback metadata"))?;
    if total != before.len()
        || (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec(),
        ) != (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        )
    {
        return Err(SourceCommandError::Conflict(
            "running executable changed during capture",
        ));
    }
    active(deadline, cancelled)?;
    Ok(hasher.finalize())
}

fn encoded(value: Value) -> SourceCommandResult<Vec<u8>> {
    let raw = serde_json::to_vec(&value)
        .map_err(|_| SourceCommandError::Invalid("native capture JSON"))?;
    let mut raw = cmd::canonical(&cmd::parse(&raw)?)?;
    raw.push(b'\n');
    Ok(raw)
}

fn selected_components(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<Value>> {
    if components.capture() != software.selection() || components.members().count() == 0 {
        return Err(SourceCommandError::Conflict(
            "native producer selected capture differs or is empty",
        ));
    }
    let mut selected = Vec::new();
    let mut remaining = 16_777_216u64;
    for member in components.members() {
        active(deadline, cancelled)?;
        if member.path.as_str().starts_with("ToS/") || member.size_bytes > remaining {
            return Err(SourceCommandError::Invalid(
                "native software namespace or total byte budget",
            ));
        }
        remaining -= member.size_bytes;
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| {
                SourceCommandError::Conflict("native producer selected component read differs")
            })?;
        selected.push(json!({"name":member.path.as_str(),"version":"selected-capture-bytes","role":"serialization-source-observation","artifact_ref":member.path.as_str(),"artifact_sha256":Digest256::of_bytes(&raw).to_hex(),"verification_status":"verified"}));
    }
    Ok(selected)
}

/// Called inside the create handler after it serializes validated records/forms.
/// Mutates only the pending in-memory package; no publication or attestation.
pub(crate) fn capture_creation(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    capture_creation_with_procedure(
        request,
        event_id,
        home,
        files,
        software,
        components,
        "native-source-metadata-serialization",
        deadline,
        cancelled,
    )
}

pub(crate) fn capture_claim_creation(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    capture_creation_with_procedure(
        request,
        event_id,
        home,
        files,
        software,
        components,
        "native-claim-serialization",
        deadline,
        cancelled,
    )
}

fn capture_creation_with_procedure(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    procedure_name: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    if components.capture() != software.selection() || components.members().count() == 0 {
        return Err(SourceCommandError::Conflict(
            "native producer selected capture differs or is empty",
        ));
    }
    if files.is_empty()
        || files.len() > 40
        || files.values().any(|raw| raw.len() > 8_388_608)
        || files
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|n| n > 33_554_432)
        || [
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
            "source-create-receipt.json",
        ]
        .iter()
        .any(|name| files.contains_key(*name))
    {
        return Err(SourceCommandError::Invalid(
            "native capture initial package closure",
        ));
    }
    let started_at = instant()?;
    let started = Instant::now();
    let runtime = executable(deadline, cancelled)?.to_hex();
    let argv: Vec<String> = std::env::args_os()
        .map(|arg| {
            arg.into_string()
                .map_err(|_| SourceCommandError::Invalid("native argv is not UTF-8"))
        })
        .collect::<SourceCommandResult<_>>()?;
    let argv_raw = serde_json::to_vec(&argv)
        .map_err(|_| SourceCommandError::Invalid("native argv capture"))?;
    let argv_digest = Digest256::of_bytes(&cmd::canonical(&cmd::parse(&argv_raw)?)?).to_hex();
    let mut selected = selected_components(software, components, deadline, cancelled)?;
    selected.push(json!({"name":"executing native process","version":env!("CARGO_PKG_VERSION"),"role":"serialization-runner","artifact_ref":"runtime:tos-native-executable","artifact_sha256":runtime,"verification_status":"verified"}));
    let outputs: Vec<_> = files
        .iter()
        .map(|(name, raw)| entity(home, name, raw, "serialized-source-metadata"))
        .collect();
    let mut request_raw = cmd::canonical(request)?;
    request_raw.push(b'\n');
    let environment = json!({"runtime":"native ELF process","runtime_version":env!("CARGO_PKG_VERSION"),"runtime_artifact_sha256":runtime,"backend":"tos-command source serialization","hardware_target":std::env::consts::ARCH,"unicode_version":"16.0.0 source-command whitespace profile"});
    let environment_raw = encoded(environment.clone())?;
    let binding = |name: &str, raw: &[u8]| json!({"ref":format!("{home}/{name}"),"sha256":Digest256::of_bytes(raw).to_hex()});
    let derivations: Vec<_> = files.keys().enumerate().map(|(i,name)| json!({"derivation_id":format!("{}.output-{i}",event_id.replacen("tos.event.","tos.derivation.",1)),"input_entity_ref":format!("{home}/source-create-request.json"),"output_entity_ref":format!("{home}/{name}"),"relation":"was_derived_from","influence_asserted":true,"description":"Technical source-copy selection and buffer serialization; caller authorship and content judgment are outside this operation."})).collect();
    let mut method_environment = environment;
    method_environment["environment_profile_binding"] =
        binding("source-create-environment.json", &environment_raw);
    // The observed ELF digest is runtime metadata, not a repository-file
    // responsibility binding or an authenticated source-to-image relation.
    let event = json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json","schema_version":"tos_provenance_event_v2","event_id":event_id,"event_version":1,"supersedes_event_ref":null,
        "record_binding":{"manifest_ref":format!("{home}/source-create-receipt.json"),"digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"},
        "activity":{"event_type":"annotation","started_at":started_at,"ended_at":instant()?,"status":"completed_with_warnings","terminal_reason":null,"exit_code":0,"warnings":["Completed in-process buffer serialization; atomic publication occurs afterward.","Selected source bytes are not build or execution authentication; stored-byte fixity is not attested."]},
        "entities":{"inputs":[entity(home,"source-create-request.json",&request_raw,"caller-supplied-metadata-request")],"outputs":outputs,"byproducts":[entity(home,"source-create-environment.json",&environment_raw,"runtime-description")]},
        "derivations":derivations,
        "responsibility":[{"agent_ref":"software:tos-native-source-commands","agent_kind":"software","role":"executor","responsibility_posture":"performed","evidence_binding":null,"human_evidence_status":"not_applicable"}],
        "method":{"procedure":{"name":procedure_name,"version":"1","purpose":"Serialize supplied source metadata and source-copy forms without judging their content."},"command_capture":{"disclosure":"withheld_digest_only","argv":null,"argv_sha256":argv_digest,"withholding_reason":"Observed process argv may contain private paths; exact library request is captured separately and process argv does not authenticate its invocation."},"configuration_binding":binding("source-create-request.json",&request_raw),"software_components":selected,"model_invocations":[],"environment":method_environment},
        "manual_changes":{"status":"none_declared","change_receipts":[],"statement":"No manual editing inside this serialization operation; caller authorship is outside its scope."},
        "measurements":[{"metric":"wall_duration_ms","status":"measured","value":started.elapsed().as_secs_f64()*1000.0,"unit":"ms","method":"Rust monotonic Instant from capture through executable/source observation and buffer binding; excludes commit.","evidence_binding":null}],
        "evidence_authentication":{"capture_posture":"tool_captured","signature_status":"unsigned","signature_bindings":[],"verification_status":"unverified","producer_control_boundary":"The same unsigned executing process serializes and observes; hashes do not authenticate execution truth."},
        "rights_and_visibility":{"rights_record_bindings":[],"intended_uses":["local_research","public_metadata"],"content_visibility":"tracked_public_metadata","publication_authorized":false,"publication_authority_bindings":[]},
        "review_and_authority":{"mechanical_validation":"not_run","human_review_status":"not_performed","review_bindings":[],"accepted_uses":[],"promotion_authorized":false,"competence_evidence_bindings":[]},
        "reproducibility":{"classification":"partially_specified","known_gaps":["Selected source capture is byte evidence only; compiler, dependencies, build and source-to-ELF relationship are not attested.","Upstream research/source reading/model invocation are outside this serialization.","Clock observations and durations are not deterministic; complete runtime environment is not archived."],"replay_scope":"Exact retained request and source-copy buffers only, not historical or semantic correctness."},
        "authority_boundary":{"validator_role":"mechanics_and_closure_only_not_truth","claims_not_established":["execution_truth","content_truth","source_fidelity","translation_quality","semantic_correctness","rights_clearance","human_review","publication_authority","canon_authority"]}
    });
    let event_raw = encoded(event)?;
    files.insert("source-create-request.json".into(), request_raw);
    files.insert("source-create-environment.json".into(), environment_raw);
    files.insert("source-create-provenance.jsonl".into(), event_raw);
    active(deadline, cancelled)
}

fn entity(home: &str, name: &str, raw: &[u8], role: &str) -> Value {
    json!({"entity_ref":format!("{home}/{name}"),"role":role,"media_type":if name.ends_with(".jsonl"){"application/x-ndjson"}else{"application/json"},"size_bytes":raw.len(),"sha256":Digest256::of_bytes(raw).to_hex(),"availability":"owner_local","content_disclosure":"public_metadata_only","fixity_verified":false,"fixity_verified_at":null})
}

/// Reuse only exact historical byte observations supplied by the durable owner.
/// No current runtime capture or past authority is inferred from these buffers.
pub(crate) fn restore_creation_capture(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
    retained: &BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let capture_names = [
        "source-create-request.json",
        "source-create-environment.json",
        "source-create-provenance.jsonl",
        "source-create-receipt.json",
    ];
    if files.is_empty()
        || retained.len() != files.len() + capture_names.len()
        || retained.len() > 40
        || retained.values().any(|raw| raw.len() > 8_388_608)
        || retained
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|n| n > 33_554_432)
        || files.iter().any(|(name, raw)| {
            capture_names.contains(&name.as_str()) || retained.get(name) != Some(raw)
        })
        || capture_names
            .iter()
            .any(|name| !retained.contains_key(*name))
    {
        return Err(SourceCommandError::Conflict(
            "retained native capture original output closure differs",
        ));
    }
    let mut request_raw = cmd::canonical(request)?;
    request_raw.push(b'\n');
    if retained["source-create-request.json"] != request_raw {
        return Err(SourceCommandError::Conflict(
            "retained native capture request bytes differ",
        ));
    }
    let environment_raw = &retained["source-create-environment.json"];
    let environment: Value = serde_json::from_slice(environment_raw)
        .map_err(|_| SourceCommandError::Invalid("retained native environment JSON"))?;
    if !environment.is_object() || encoded(environment.clone())? != *environment_raw {
        return Err(SourceCommandError::Conflict(
            "retained native environment encoding differs",
        ));
    }
    let event_raw = &retained["source-create-provenance.jsonl"];
    let event: Value = serde_json::from_slice(event_raw)
        .map_err(|_| SourceCommandError::Invalid("retained native provenance JSON"))?;
    if encoded(event.clone())? != *event_raw {
        return Err(SourceCommandError::Conflict(
            "retained native provenance encoding differs",
        ));
    }
    let binding = |name: &str, raw: &[u8]| {
        json!({"ref":format!("{home}/{name}"),
        "sha256":Digest256::of_bytes(raw).to_hex()})
    };
    let runtime = environment
        .get("runtime_artifact_sha256")
        .and_then(Value::as_str)
        .ok_or(SourceCommandError::Invalid(
            "retained native runtime observation absent",
        ))?;
    // The digest is a retained observation, not the current executable or an
    // authored-file binding; the caller's selected software is checked below.
    if runtime.len() != 64
        || !runtime
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(SourceCommandError::Invalid(
            "retained native runtime digest",
        ));
    }
    let runtime_version = environment
        .get("runtime_version")
        .and_then(Value::as_str)
        .ok_or(SourceCommandError::Invalid(
            "retained native runtime version absent",
        ))?;
    let mut selected = selected_components(software, components, deadline, cancelled)?;
    selected.push(
        json!({"name":"executing native process","version":runtime_version,
        "role":"serialization-runner","artifact_ref":"runtime:tos-native-executable",
        "artifact_sha256":runtime,"verification_status":"verified"}),
    );
    let mut method_environment = environment.clone();
    method_environment["environment_profile_binding"] =
        binding("source-create-environment.json", environment_raw);
    if event.get("event_id") != Some(&json!(event_id))
        || event.get("record_binding")
            != Some(
                &json!({"manifest_ref":format!("{home}/source-create-receipt.json"),
            "digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"}),
            )
        || event.pointer("/method/configuration_binding")
            != Some(&binding("source-create-request.json", &request_raw))
        || event.pointer("/method/environment") != Some(&method_environment)
        || event.pointer("/method/software_components") != Some(&Value::Array(selected))
    {
        return Err(SourceCommandError::Conflict(
            "retained native capture selected bindings differ",
        ));
    }
    for name in capture_names.into_iter().take(3) {
        files.insert(name.into(), retained[name].clone());
    }
    active(deadline, cancelled)
}
