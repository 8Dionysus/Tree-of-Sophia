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

pub(crate) fn executable(
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Digest256> {
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

pub(crate) fn selected_components(
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
        if request
            .object_get("record")
            .and_then(|r| r.object_get("schema_version"))
            .and_then(JsonValue::as_str)
            == Some("tos_artifact_source_witness_v2")
        {
            "native-artifact-metadata-serialization"
        } else {
            "native-source-metadata-serialization"
        },
        None,
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
        None,
        deadline,
        cancelled,
    )
}

struct TextCaptureProfile<'a> {
    rights: &'a JsonValue,
    publication_authority: Option<&'a JsonValue>,
    inputs: Vec<Value>,
    event_type: &'static str,
    source_path: &'a str,
    warning: &'static str,
    purpose: &'static str,
    replay_scope: &'static str,
}

/// The separately granted owner-local Text writer uses the same observed
/// request/ELF/software capture, with explicit private source dependencies.
/// This still does not authenticate an upstream OCR or an owner's assessment.
pub(crate) fn capture_initial_text_layer(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    source_path: &str,
    rights: &JsonValue,
    inputs: Vec<Value>,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if inputs.len() != 2 || files.get("content.txt").is_none() {
        return Err(SourceCommandError::Invalid(
            "native Text capture source closure",
        ));
    }
    capture_creation_with_procedure(
        request,
        event_id,
        home,
        files,
        software,
        components,
        "exact-native-text-layer-structural-extraction",
        Some(TextCaptureProfile {
            rights,
            publication_authority: None,
            inputs,
            event_type: "native_extraction",
            source_path,
            warning: "Exact acquired File and member verified; declared bounded structural extraction completed; atomic private publication follows and no textual assessment is performed.",
            purpose: "Extract a separately granted exact EPUB member/selector into an immutable unreviewed private TextLayer; no OCR, silent Unicode rewrite or segmentation.",
            replay_scope: "Exact retained source/configuration/implementation and bounded extraction output; not textual correctness or deterministic provenance timestamps.",
        }),
        deadline,
        cancelled,
    )
}

pub(crate) fn capture_first_text_unit(
    first: bool,
    request: &JsonValue,
    event_id: &str,
    home: &str,
    source_path: &str,
    rights: &JsonValue,
    inputs: Vec<Value>,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if inputs.len() != if first { 2 } else { 3 } || !files.contains_key("source-text-unit.v1.json")
    {
        return Err(SourceCommandError::Invalid(
            "native TextUnit capture closure",
        ));
    }
    capture_creation_with_procedure(
        request,
        event_id,
        home,
        files,
        software,
        components,
        if first {
            "exact-native-first-text-unit-segmentation"
        } else {
            "exact-native-text-unit-construction"
        },
        Some(TextCaptureProfile {
            rights,
            publication_authority: None,
            inputs,
            event_type: "annotation",
            source_path,
            warning: if first {
                "Exact selected TextLayer and private representation verified; first segmentation is a method proposal, not source or linguistic truth."
            } else {
                "Exact selected native packet, TextLayer and private representation verified; new segmentation is a method proposal, not source or linguistic truth."
            },
            purpose: if first {
                "Construct one bounded first TextUnit segmentation from an independently granted exact TextLayer; no content mutation or semantic promotion."
            } else {
                "Construct a bounded TextUnit proposal from one exact predecessor packet and layer; no content mutation or semantic promotion."
            },
            replay_scope: "Exact retained layer/configuration/implementation and proposed boundaries; not historical source truth or deterministic provenance timestamps.",
        }),
        deadline,
        cancelled,
    )
}

/// Capture only the source selection and serialization of a supplied mapping.
/// Both-side rights/content checks belong to the alignment caller; this event
/// does not claim an aligner run or assess the proposal.
pub(crate) fn capture_owner_alignment(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    source_path: &str,
    rights: &JsonValue,
    inputs: Vec<Value>,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if inputs.is_empty()
        || inputs.len() > 128
        || !files.contains_key("native-translation-alignment.v1.json")
    {
        return Err(SourceCommandError::Invalid(
            "native alignment capture source closure",
        ));
    }
    capture_creation_with_procedure(
        request,
        event_id,
        home,
        files,
        software,
        components,
        "native-supplied-alignment-proposal-capture",
        Some(TextCaptureProfile {
            rights,
            publication_authority: None,
            inputs,
            event_type: "annotation",
            source_path,
            warning: "Exact selected native inputs and both rights gates were checked; no aligner execution or translation assessment occurred.",
            purpose: "Record an independently supplied alignment proposal against two exact native source closures without judging translation quality.",
            replay_scope: "Exact retained mapping, owner grants, native inputs and selected software; no semantic truth or deterministic capture clocks.",
        }),
        deadline,
        cancelled,
    )
}

pub(crate) fn capture_derived_text_layer(
    operation: &str,
    request: &JsonValue,
    event_id: &str,
    home: &str,
    source_path: &str,
    rights: &JsonValue,
    inputs: Vec<Value>,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if inputs.is_empty()
        || inputs.len() > 3
        || !files.contains_key("content.txt")
        || !matches!(
            operation,
            "text-layer.correct"
                | "text-layer.normalize"
                | "text-layer.record-transcription"
                | "text-layer.record-ocr"
                | "text-layer.record-owner-ocr"
                | "text-layer.record-owner-page-ocr"
        )
    {
        return Err(SourceCommandError::Invalid(
            "native derived Text capture closure",
        ));
    }
    let procedure = format!(
        "exact-native-text-layer-{}",
        operation
            .strip_prefix("text-layer.")
            .ok_or(SourceCommandError::Invalid("native derived Text procedure"))?
    );
    capture_creation_with_procedure(
        request,
        event_id,
        home,
        files,
        software,
        components,
        &procedure,
        Some(TextCaptureProfile {
            rights,
            publication_authority: None,
            inputs,
            event_type: if matches!(
                operation,
                "text-layer.record-transcription"
                    | "text-layer.record-ocr"
                    | "text-layer.record-owner-ocr"
                    | "text-layer.record-owner-page-ocr"
            ) {
                "annotation"
            } else if operation == "text-layer.normalize" {
                "normalization"
            } else {
                "correction"
            },
            source_path,
            warning: if matches!(
                operation,
                "text-layer.record-owner-ocr" | "text-layer.record-owner-page-ocr"
            ) {
                "The stronger owner's retained OCR execution was authenticated; ToS copied its result without rerunning inference. Private publication follows, with no textual truth or review granted."
            } else {
                "Exact selected source and owner-local representation were checked; private publication follows and no textual truth or review is granted."
            },
            purpose: if matches!(
                operation,
                "text-layer.record-owner-ocr" | "text-layer.record-owner-page-ocr"
            ) {
                "Record one separately granted TextLayer from an exact signed stronger-owner OCR result; no new inference or semantic promotion occurs in this command."
            } else {
                "Record one separately granted bounded TextLayer derivation from exact selected bytes; no inference or semantic promotion is claimed."
            },
            replay_scope: "Exact retained owner configuration, source and implementation with bounded derived bytes; not source fidelity or deterministic capture clocks.",
        }),
        deadline,
        cancelled,
    )
}

/// Observe the whole public project-text construction, including its first unit.
/// Rights and publication scope are supplied owner evidence, not process assessment.
pub(crate) fn capture_public_project_text(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    source_path: &str,
    rights: &JsonValue,
    publication_authority: &JsonValue,
    inputs: Vec<Value>,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if inputs.is_empty()
        || [
            "content.txt",
            "source-text-layer.v1.json",
            "source-text-unit.v1.json",
            "construction-plan.json",
        ]
        .iter()
        .any(|name| !files.contains_key(*name))
    {
        return Err(SourceCommandError::Invalid(
            "native public project-text capture closure",
        ));
    }
    capture_creation_with_procedure(
        request,
        event_id,
        home,
        files,
        software,
        components,
        "tos.project-authored.utf8-range.v1",
        Some(TextCaptureProfile {
            rights,
            publication_authority: Some(publication_authority),
            inputs,
            event_type: "native_extraction",
            source_path,
            warning: "Exact project-authored UTF-8 range and explicit partition captured; atomic source publication follows. No textual, linguistic, semantic or rights assessment was performed.",
            purpose: "Capture a literal UTF-8 source range and proposed segmentation under independently supplied project-text public authority; preserve original source bytes and separate native identities.",
            replay_scope: "Exact source range and proposed partition with bound inputs, implementation and public plan; not content correctness or deterministic timestamps. Protected grant paths are not published.",
        }),
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
    text_capture: Option<TextCaptureProfile<'_>>,
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
    let mut event = json!({
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
    if let Some(profile) = text_capture {
        let public = profile.publication_authority.is_some();
        let rights: Value = serde_json::from_slice(&cmd::canonical(profile.rights)?)
            .map_err(|_| SourceCommandError::Invalid("native Text rights capture"))?;
        let input_count = profile.inputs.len();
        event["activity"]["event_type"] = json!(profile.event_type);
        event["activity"]["warnings"][0] = json!(profile.warning);
        event["entities"]["inputs"]
            .as_array_mut()
            .ok_or(SourceCommandError::Invalid("native Text event inputs"))?
            .extend(profile.inputs);
        for group in ["inputs", "outputs", "byproducts"] {
            for row in event["entities"][group]
                .as_array_mut()
                .ok_or(SourceCommandError::Invalid("native Text event entities"))?
            {
                row["content_disclosure"] = json!(if public {
                    "public_content"
                } else {
                    "private_content"
                });
                if public {
                    row["availability"] = json!("tracked");
                }
                if row["entity_ref"]
                    .as_str()
                    .is_some_and(|reference| reference.ends_with("/content.txt"))
                {
                    row["media_type"] = json!("text/plain; charset=utf-8");
                }
            }
        }
        for index in 0..input_count {
            let source_ref = event["entities"]["inputs"][index + 1]["entity_ref"].clone();
            event["derivations"].as_array_mut().ok_or(SourceCommandError::Invalid("native Text event derivations"))?
                .push(json!({"derivation_id":format!("{}.{}-{index}",event_id.replacen("tos.event.","tos.derivation.",1), if public { "public-native" } else { "native" }),
                    "input_entity_ref":source_ref,"output_entity_ref":profile.source_path,
                    "relation":"selection_from","influence_asserted":true,
                    "description":if public { "Exact project-text construction dependency, not historical influence or assessment." } else { "Technical exact-source dependency for structural extraction, not source fidelity or textual truth." }}));
        }
        let configuration_file = if public {
            "construction-plan.json"
        } else {
            "source-create-owner-configuration.json"
        };
        event["method"]["configuration_binding"] = binding(
            configuration_file,
            files
                .get(configuration_file)
                .ok_or(SourceCommandError::Invalid(
                    "native Text retained configuration",
                ))?,
        );
        event["method"]["procedure"]["purpose"] = json!(profile.purpose);
        event["rights_and_visibility"]["rights_record_bindings"] = rights;
        event["rights_and_visibility"]["intended_uses"] = if public {
            json!(["public_metadata", "publication"])
        } else {
            json!(["local_research"])
        };
        event["rights_and_visibility"]["content_visibility"] = json!(if public {
            "public_content"
        } else {
            "local_only"
        });
        if let Some(authority) = profile.publication_authority {
            let authority: Value = serde_json::from_slice(&cmd::canonical(authority)?)
                .map_err(|_| SourceCommandError::Invalid("native public authority capture"))?;
            event["rights_and_visibility"]["publication_authorized"] = json!(true);
            event["rights_and_visibility"]["publication_authority_bindings"] = json!([authority]);
        }
        event["reproducibility"]["known_gaps"][0] = json!(if public {
            "Project authorship, licensing and delegated publication scope are supplied owner evidence, not granted or authenticated by this unsigned process; no model or human assessment is executed."
        } else {
            "Source selection and rights decisions belong to the independent owner; this capture records bounded extraction mechanics without assessing source fidelity or content truth."
        });
        event["reproducibility"]["replay_scope"] = json!(profile.replay_scope);
    }
    let event_raw = encoded(event)?;
    files.insert("source-create-request.json".into(), request_raw);
    files.insert("source-create-environment.json".into(), environment_raw);
    files.insert("source-create-provenance.jsonl".into(), event_raw);
    active(deadline, cancelled)
}

fn entity(home: &str, name: &str, raw: &[u8], role: &str) -> Value {
    entity_at(&format!("{home}/{name}"), raw, role)
}
fn entity_at(reference: &str, raw: &[u8], role: &str) -> Value {
    json!({"entity_ref":reference,"role":role,"media_type":if reference.ends_with(".jsonl"){"application/x-ndjson"}else if reference.ends_with(".md") || reference.ends_with("/fixity.sha256") {"text/plain; charset=utf-8"}else{"application/json"},"size_bytes":raw.len(),"sha256":Digest256::of_bytes(raw).to_hex(),"availability":"owner_local","content_disclosure":"public_metadata_only","fixity_verified":false,"fixity_verified_at":null})
}

/// Exact process observation for the Work→Expression owner.  The returned
/// event is a retained native capture, not a re-created Python event, source
/// approval, or permission to publish a selected transaction.
pub(crate) struct WorkNativeCapture {
    pub(crate) environment_raw: Vec<u8>,
    pub(crate) event_raw: Vec<u8>,
}
pub(crate) fn capture_work_expression(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    archive_path: &str,
    before: &BTreeMap<String, Vec<u8>>,
    outputs: &[(&str, &[u8])],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkNativeCapture> {
    capture_compound(
        request,
        event_id,
        home,
        archive_path,
        before,
        outputs,
        software,
        components,
        deadline,
        cancelled,
        CompoundCapture::WorkExpression,
    )
}
pub(crate) fn capture_item_adoption(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    archive_path: &str,
    before: &BTreeMap<String, Vec<u8>>,
    outputs: &[(&str, &[u8])],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkNativeCapture> {
    capture_compound(
        request,
        event_id,
        home,
        archive_path,
        before,
        outputs,
        software,
        components,
        deadline,
        cancelled,
        CompoundCapture::EditionItem,
    )
}
pub(crate) fn capture_collection_membership(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    archive_path: &str,
    before: &BTreeMap<String, Vec<u8>>,
    outputs: &[(&str, &[u8])],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkNativeCapture> {
    capture_compound(
        request,
        event_id,
        home,
        archive_path,
        before,
        outputs,
        software,
        components,
        deadline,
        cancelled,
        CompoundCapture::CollectionMembership,
    )
}
pub(crate) fn capture_expression_responsibility(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    archive_path: &str,
    before: &BTreeMap<String, Vec<u8>>,
    outputs: &[(&str, &[u8])],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkNativeCapture> {
    capture_compound(
        request,
        event_id,
        home,
        archive_path,
        before,
        outputs,
        software,
        components,
        deadline,
        cancelled,
        CompoundCapture::ExpressionResponsibility,
    )
}
pub(crate) fn capture_expression_edition(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    archive_path: &str,
    before: &BTreeMap<String, Vec<u8>>,
    outputs: &[(&str, &[u8])],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkNativeCapture> {
    capture_compound(
        request,
        event_id,
        home,
        archive_path,
        before,
        outputs,
        software,
        components,
        deadline,
        cancelled,
        CompoundCapture::ExpressionEdition,
    )
}
pub(crate) fn capture_object_link(
    request: &JsonValue,
    event_id: &str,
    home: &str,
    outputs: &[(&str, &[u8])],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkNativeCapture> {
    capture_compound(
        request,
        event_id,
        home,
        "",
        &BTreeMap::new(),
        outputs,
        software,
        components,
        deadline,
        cancelled,
        CompoundCapture::ObjectLink,
    )
}
enum CompoundCapture {
    ObjectLink,
    WorkExpression,
    EditionItem,
    CollectionMembership,
    ExpressionResponsibility,
    ExpressionEdition,
}
fn capture_compound(
    request: &JsonValue,
    event_id: &str,
    expression_home: &str,
    archive_path: &str,
    before: &BTreeMap<String, Vec<u8>>,
    outputs: &[(&str, &[u8])],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
    profile: CompoundCapture,
) -> SourceCommandResult<WorkNativeCapture> {
    let object_link = matches!(profile, CompoundCapture::ObjectLink);
    let (receipt_name, warning, procedure, purpose) = match profile {
        CompoundCapture::ObjectLink => (
            "object-link-creation-receipt.json",
            "Completed in-process Object/Link buffer serialization; atomic selected-metadata publication occurs afterward.",
            "native-object-link-serialization",
            "Serialize one declared Object/Link association and explicit source-copy forms without judging content.",
        ),
        CompoundCapture::WorkExpression => (
            "work-expression-receipt.json",
            "Completed in-process Work/Expression buffer serialization; atomic selected-metadata publication occurs afterward.",
            "native-work-expression-serialization",
            "Serialize one declared Work/Expression link and explicit source-copy forms without judging content.",
        ),
        CompoundCapture::EditionItem => (
            "edition-item-receipt.json",
            "Completed in-process Edition/Item buffer serialization; atomic selected-metadata publication occurs afterward.",
            "native-item-adoption-serialization",
            "Serialize one declared Edition/Item link and explicit source-copy forms without judging content.",
        ),
        CompoundCapture::CollectionMembership => (
            "membership-attachment-receipt.json",
            "Completed in-process Collection membership buffer serialization; atomic selected-metadata publication occurs afterward.",
            "native-collection-membership-serialization",
            "Serialize one qualified Collection membership Claim and explicit source-copy forms without judging membership.",
        ),
        CompoundCapture::ExpressionResponsibility => (
            "responsibility-attachment-receipt.json",
            "Completed in-process Expression responsibility buffer serialization; atomic selected-metadata publication occurs afterward.",
            "native-expression-responsibility-serialization",
            "Serialize one qualified Expression responsibility Claim and explicit source-copy forms without judging attribution.",
        ),
        CompoundCapture::ExpressionEdition => (
            "expression-edition-receipt.json",
            "Completed in-process Expression/Edition buffer serialization; atomic selected-metadata publication occurs afterward.",
            "native-expression-edition-serialization",
            "Serialize one declared Expression/Edition link and explicit source-copy forms without judging content.",
        ),
    };
    active(deadline, cancelled)?;
    if (!object_link && before.is_empty())
        || (object_link && (!before.is_empty() || !archive_path.is_empty()))
        || outputs.is_empty()
        || outputs.len() > 64
        || outputs.iter().any(|(_, raw)| raw.len() > 8_388_608)
        || outputs
            .iter()
            .try_fold(0usize, |n, (_, raw)| n.checked_add(raw.len()))
            .is_none_or(|n| n > 8_388_608)
    {
        return Err(SourceCommandError::Invalid(
            "native Work capture selected byte budget",
        ));
    }
    let started_at = instant()?;
    let started = Instant::now();
    let runtime = executable(deadline, cancelled)?.to_hex();
    let argv = std::env::args_os()
        .map(|arg| {
            arg.into_string()
                .map_err(|_| SourceCommandError::Invalid("native Work argv is not UTF-8"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let argv_raw = serde_json::to_vec(&argv)
        .map_err(|_| SourceCommandError::Invalid("native Work argv capture"))?;
    let argv_digest = Digest256::of_bytes(&cmd::canonical(&cmd::parse(&argv_raw)?)?).to_hex();
    let mut software_rows = selected_components(software, components, deadline, cancelled)?;
    software_rows.push(json!({"name":"executing native process","version":env!("CARGO_PKG_VERSION"),"role":"serialization-runner","artifact_ref":"runtime:tos-native-executable","artifact_sha256":runtime,"verification_status":"verified"}));
    let mut request_raw = cmd::canonical(request)?;
    request_raw.push(b'\n');
    let request_ref = format!("{expression_home}/source-create-request.json");
    let environment_ref = format!("{expression_home}/source-create-environment.json");
    let environment = json!({"runtime":"native ELF process","runtime_version":env!("CARGO_PKG_VERSION"),"runtime_artifact_sha256":runtime,"backend":"tos-command source serialization","hardware_target":std::env::consts::ARCH,"unicode_version":"16.0.0 source-command whitespace profile","argv_sha256":argv_digest});
    let environment_raw = encoded(environment.clone())?;
    let mut inputs = vec![entity_at(
        &request_ref,
        &request_raw,
        "caller-supplied-metadata-request",
    )];
    let mut prior = BTreeMap::new();
    for raw in before.values() {
        prior.insert(
            format!("{archive_path}/{}.blob", Digest256::of_bytes(raw).to_hex()),
            raw,
        );
    }
    inputs.extend(
        prior
            .iter()
            .map(|(path, raw)| entity_at(path, raw, "retained-parent-metadata-input")),
    );
    let mut sorted_outputs = outputs.to_vec();
    sorted_outputs.sort_by(|a, b| a.0.cmp(b.0));
    if sorted_outputs.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(SourceCommandError::Invalid(
            "duplicate native Work output path",
        ));
    }
    let prepared_outputs = sorted_outputs
        .iter()
        .map(|(path, raw)| entity_at(path, raw, "prepared-compound-source-metadata"))
        .collect::<Vec<_>>();
    let derivation = event_id.replacen("tos.event.", "tos.derivation.", 1);
    let derivations = sorted_outputs.iter().enumerate().map(|(index, (path, _))| json!({"derivation_id":format!("{derivation}.output-{index}"),"input_entity_ref":request_ref,"output_entity_ref":path,"relation":"was_derived_from","influence_asserted":true,"description":"Technical source metadata serialization; no historical influence or textual identity is asserted."}))
        .collect::<Vec<_>>();
    let mut method_environment = environment;
    method_environment["environment_profile_binding"] =
        json!({"ref":environment_ref,"sha256":Digest256::of_bytes(&environment_raw).to_hex()});
    method_environment
        .as_object_mut()
        .unwrap()
        .remove("argv_sha256");
    let event = json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json","schema_version":"tos_provenance_event_v2","event_id":event_id,"event_version":1,"supersedes_event_ref":null,
        "record_binding":{"manifest_ref":format!("{expression_home}/{}",receipt_name),"digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"},
        "activity":{"event_type":"annotation","started_at":started_at,"ended_at":instant()?,"status":"completed_with_warnings","terminal_reason":null,"exit_code":0,"warnings":[warning,"The declared record link is not accepted bibliographic or textual truth."]},
        "entities":{"inputs":inputs,"outputs":prepared_outputs,"byproducts":[entity_at(&environment_ref,&environment_raw,"runtime-description")]},
        "derivations":derivations,
        "responsibility":[{"agent_ref":"software:tos-native-source-commands","agent_kind":"software","role":"executor","responsibility_posture":"performed","evidence_binding":null,"human_evidence_status":"not_applicable"}],
        "method":{"procedure":{"name":procedure,"version":"1","purpose":purpose},"command_capture":{"disclosure":"withheld_digest_only","argv":null,"argv_sha256":argv_digest,"withholding_reason":"Observed process argv may contain private paths; the exact library request is captured separately."},"configuration_binding":{"ref":request_ref,"sha256":Digest256::of_bytes(&request_raw).to_hex()},"software_components":software_rows,"model_invocations":[],"environment":method_environment},
        "manual_changes":{"status":"none_declared","change_receipts":[],"statement":"Caller authorship precedes this operation; no manual edits occur inside serialization."},
        "measurements":[{"metric":"wall_duration_ms","status":"measured","value":started.elapsed().as_secs_f64()*1000.0,"unit":"ms","method":"Rust monotonic Instant from capture through native executable/source observation and buffer binding; excludes commit.","evidence_binding":null}],
        "evidence_authentication":{"capture_posture":"tool_captured","signature_status":"unsigned","signature_bindings":[],"verification_status":"unverified","producer_control_boundary":"The same unsigned native process serializes and observes; hashes do not authenticate execution truth."},
        "rights_and_visibility":{"rights_record_bindings":[],"intended_uses":["local_research","public_metadata"],"content_visibility":"tracked_public_metadata","publication_authorized":false,"publication_authority_bindings":[]},
        "review_and_authority":{"mechanical_validation":"not_run","human_review_status":"not_performed","review_bindings":[],"accepted_uses":[],"promotion_authorized":false,"competence_evidence_bindings":[]},
        "reproducibility":{"classification":"partially_specified","known_gaps":["Selected source capture is byte evidence only; compiler, dependencies, build and source-to-ELF relation are not attested.","Upstream research, reading and model invocation are outside this serialization.","Clock observations and durations are not deterministic; complete runtime environment is not archived."],"replay_scope":"Exact retained request and source-copy buffers only, not bibliographic truth."},
        "authority_boundary":{"validator_role":"mechanics_and_closure_only_not_truth","claims_not_established":["execution_truth","content_truth","source_fidelity","translation_quality","semantic_correctness","rights_clearance","human_review","publication_authority","canon_authority"]}
    });
    Ok(WorkNativeCapture {
        environment_raw,
        event_raw: encoded(event)?,
    })
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
    let artifact = request
        .object_get("record")
        .and_then(|r| r.object_get("schema_version"))
        .and_then(JsonValue::as_str)
        == Some("tos_artifact_source_witness_v2");
    if artifact
        && event.pointer("/method/procedure/name")
            != Some(&json!("native-artifact-metadata-serialization"))
    {
        return Err(SourceCommandError::Conflict(
            "retained Artifact serialization procedure differs",
        ));
    }
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
