//! Independent public synthetic provenance operations with captured native execution.
//! Records are unsigned producer evidence, never human review or execution truth.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{encode, ensure, s, sha, utc_now},
    transfer_target_passages::read_optional,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs::File, time::Instant};
use unicode_normalization::UnicodeNormalization;
type Result<T> = std::result::Result<T, String>;
const ROOT: &str = "ToS/research-packets/foundation-laboratory-2026-07/provenance-event-v2-abc";
const BUILDER: &str = "rust/crates/tos-compiler/src/provenance_event_lab.rs";
const CONTRACT: &str = "ToS/contracts/provenance-event-v2.schema.json";
const CAP: usize = 2 * 1024 * 1024;
const FAILURE: &[u8] = b"status=failed\nreason=non_ascii_input\nexit_code=7\n";
fn path(name: &str) -> String {
    format!("{ROOT}/{name}")
}
fn definitions() -> Value {
    serde_json::from_str(include_str!("provenance_event_lab/definitions.json"))
        .expect("compiled synthetic definitions")
}
struct Inputs {
    files: BTreeMap<String, Vec<u8>>,
    held: Vec<(String, File, String)>,
}
impl Inputs {
    fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            held: vec![],
        }
    }
    fn read(&mut self, ctx: &ResearchExecution, reference: &str) -> Result<()> {
        let mut file = ctx.source_file(reference, CAP as u64)?;
        let raw = ctx.read_file(&mut file, CAP as u64)?;
        self.held.push((reference.into(), file, sha(&raw)));
        self.files.insert(reference.into(), raw);
        Ok(())
    }
    fn json(&self, reference: &str) -> Result<Value> {
        let raw = self
            .files
            .get(reference)
            .ok_or("unselected provenance input")?;
        tos_foundation::parse_json(
            raw,
            tos_foundation::JsonMode::PublishedStrict,
            tos_foundation::JsonLimits::new(CAP, 96, 100_000, 4096)
                .map_err(|e| format!("limits: {e:?}"))?,
        )
        .map_err(|e| format!("strict input: {e:?}"))?;
        serde_json::from_slice(raw).map_err(|e| e.to_string())
    }
    fn verify(&mut self, ctx: &ResearchExecution) -> Result<()> {
        for (reference, file, digest) in &mut self.held {
            ensure(
                ctx.hash_file(file, CAP as u64)? == *digest,
                "held provenance input changed",
            )?;
            let mut current = ctx.source_file(reference, CAP as u64)?;
            ensure(
                ctx.hash_file(&mut current, CAP as u64)? == *digest,
                "provenance input path changed",
            )?;
        }
        Ok(())
    }
    fn binding(&self, reference: &str) -> Result<Value> {
        Ok(
            json!({"ref":reference,"sha256":sha(self.files.get(reference).ok_or("unselected binding")?)}),
        )
    }
}
fn inputs(ctx: &ResearchExecution) -> Result<Inputs> {
    let mut inputs = Inputs::new();
    for reference in [
        CONTRACT.to_string(),
        path("plan.json"),
        path("input-fixture.txt"),
    ] {
        inputs.read(ctx, &reference)?;
    }
    let definitions = definitions();
    ensure(
        inputs.json(&path("plan.json"))? == definitions["plan"],
        "synthetic provenance plan drift",
    )?;
    ensure(
        inputs.files[&path("input-fixture.txt")].as_slice()
            == include_bytes!("provenance_event_lab/input-fixture.txt").as_slice(),
        "provenance fixture is not the declared public synthetic input",
    )?;
    Ok(inputs)
}
fn executable(ctx: &ResearchExecution) -> Result<String> {
    let path = std::env::current_exe().map_err(|e| e.to_string())?;
    let directory = ctx.select_directory(path.parent().ok_or("native executable parent")?)?;
    let mut file = directory.source_file(
        path.file_name()
            .and_then(|n| n.to_str())
            .ok_or("native executable name")?,
        256 * 1024 * 1024,
    )?;
    directory.hash_file(&mut file, 256 * 1024 * 1024)
}
fn save(ctx: &ResearchExecution, reference: &str, raw: &[u8]) -> Result<bool> {
    ensure(raw.len() <= CAP, "provenance output bound")?;
    match read_optional(ctx, reference)? {
        Some(old) if old == raw => Ok(false),
        Some(old) => {
            ctx.write_replacing_exact(reference, raw, 0o644, &old)?;
            Ok(true)
        }
        None => {
            ctx.write(reference, raw, 0o644, true)?;
            Ok(true)
        }
    }
}
fn entity(reference: &str, raw: &[u8], role: &str, at: &str) -> Value {
    json!({"entity_ref":reference,"role":role,"sha256":sha(raw),"size_bytes":raw.len(),"media_type":"text/plain; charset=utf-8","availability":"tracked","content_disclosure":"synthetic_public","fixity_verified":true,"fixity_verified_at":at})
}
fn schema(ctx: &ResearchExecution, inputs: &Inputs, event: &Value) -> Result<()> {
    let uri = format!("https://tree-of-sophia.local/{CONTRACT}");
    let schema = tos_validation::SchemaBackendProbe::new(
        [tos_validation::SchemaResource {
            uri: uri.clone(),
            raw: inputs.files[CONTRACT].clone(),
        }],
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("schema preparation: {e:?}"))?;
    ensure(
        schema
            .is_valid_raw(&uri, &encode(event, false)?)
            .map_err(|e| format!("schema execution: {e:?}"))?,
        "provenance event schema invalid",
    )?;
    let issues = tos_validation::provenance_rules::semantic_issues(event, 128, ctx.deadline())
        .map_err(|e| format!("provenance semantics: {e:?}"))?;
    ensure(
        issues.is_empty(),
        &format!("provenance semantics: {issues:?}"),
    )
}
pub fn prepare(ctx: &ResearchExecution, version: &str) -> Result<Value> {
    let mut selected = inputs(ctx)?;
    let executable_sha = executable(ctx)?;
    let mut uname = std::mem::MaybeUninit::<libc::utsname>::uninit();
    ensure(
        unsafe { libc::uname(uname.as_mut_ptr()) } == 0,
        "platform capture failed",
    )?;
    let uname = unsafe { uname.assume_init() };
    fn text(raw: &[libc::c_char]) -> Result<String> {
        unsafe { std::ffi::CStr::from_ptr(raw.as_ptr()) }
            .to_str()
            .map(str::to_owned)
            .map_err(|e| e.to_string())
    }
    let value = json!({"schema_version":"tos_provenance_environment_profile_v1","profile_id":"tos.environment.provenance-event-v2.synthetic-abc","captured_at":utc_now()?,
        "runtime":{"implementation":"Rust native executable","version":version,"executable_sha256":executable_sha,"unicode_version":"16.0.0"},
        "platform":{"system":text(&uname.sysname)?,"release":text(&uname.release)?,"machine":text(&uname.machine)?,"hardware_target":"cpu"},
        "capture_boundary":{"absolute_paths_recorded":false,"environment_variables_recorded":false,"packages_enumerated":false,"reason":"The native lab uses bounded exact byte operations and the compiled Unicode NFC implementation."}});
    selected.verify(ctx)?;
    save(
        ctx,
        &path("environment-profile.json"),
        &encode(&value, true)?,
    )?;
    Ok(
        json!({"status":"prepared","environment_profile":path("environment-profile.json"),"execution_truth_established":false}),
    )
}
pub fn variant(
    ctx: &ResearchExecution,
    id: &str,
    argv: &[String],
    version: &str,
) -> Result<(Value, i32)> {
    ensure(matches!(id, "A" | "B" | "C"), "variant must be A, B or C")?;
    let mut selected = inputs(ctx)?;
    selected.read(ctx, &path("environment-profile.json"))?;
    let environment = selected.json(&path("environment-profile.json"))?;
    ensure(
        environment["runtime"]["implementation"] == "Rust native executable"
            && environment["runtime"]["version"] == version
            && environment["runtime"]["executable_sha256"] == executable(ctx)?
            && environment["runtime"]["unicode_version"] == "16.0.0",
        "environment profile differs; prepare with this native executable",
    )?;
    let start_at = utc_now()?;
    let start = Instant::now();
    let input = selected.files[&path("input-fixture.txt")].clone();
    let (output_name, bytes, exit) = match id {
        "A" => ("variant-a.copy.txt", input.clone(), 0),
        "B" => (
            "variant-b.nfc.txt",
            std::str::from_utf8(&input)
                .map_err(|e| e.to_string())?
                .nfc()
                .collect::<String>()
                .into_bytes(),
            0,
        ),
        _ => {
            ensure(
                !std::str::from_utf8(&input)
                    .map_err(|e| e.to_string())?
                    .is_ascii(),
                "ASCII negative control unexpectedly accepted input",
            )?;
            ("variant-c.failure.log", FAILURE.to_vec(), 7)
        }
    };
    ctx.tick((input.len() + bytes.len()) as u64)?;
    selected.verify(ctx)?;
    save(ctx, &path(output_name), &bytes)?;
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    let end_at = utc_now()?;
    let templates = definitions();
    let mut event = templates["events"][id].clone();
    let seed = encode(
        &json!([
            &start_at,
            &end_at,
            duration_ms,
            argv,
            environment["runtime"]["executable_sha256"]
        ]),
        false,
    )?;
    event["event_id"] = json!(format!(
        "tos.event.provenance-v2.synthetic-{}.native-{}",
        id.to_lowercase(),
        &sha(&seed)[..24]
    ));
    event["activity"]["started_at"] = json!(start_at);
    event["activity"]["ended_at"] = json!(end_at);
    let result_entity = entity(
        &path(output_name),
        &bytes,
        if exit == 0 {
            "authoritative-synthetic-output"
        } else {
            "failure-diagnostic-byproduct"
        },
        &end_at,
    );
    event["entities"] = json!({"inputs":[entity(&path("input-fixture.txt"),&input,"public-synthetic-input",&end_at)],"outputs":if exit==0 {vec![result_entity.clone()]} else {vec![]},"byproducts":if exit==7 {vec![result_entity]} else {vec![]}});
    let builder_sha = sha(include_bytes!("provenance_event_lab.rs"));
    let binding = json!({"ref":BUILDER,"sha256":builder_sha});
    event["responsibility"][0]["evidence_binding"] = binding.clone();
    event["method"]["command_capture"] = json!({"disclosure":"withheld_digest_only","argv":null,"argv_sha256":sha(&encode(&json!(argv),false)?),"withholding_reason":"The exact invocation includes host-local executable and source-root paths. Its canonical argv digest is captured without publishing those paths."});
    event["method"]["configuration_binding"] = selected.binding(&path("plan.json"))?;
    event["method"]["software_components"] = json!([{"name":"Tree of Sophia native provenance v2 laboratory builder","version":"1","role":"transformation-runner","artifact_ref":BUILDER,"artifact_sha256":builder_sha,"verification_status":"verified"},{"name":"tos-access","version":version,"role":"native-executable","artifact_ref":"runtime:tos-native-executable","artifact_sha256":environment["runtime"]["executable_sha256"],"verification_status":"verified"}]);
    event["method"]["environment"] = json!({"runtime":"Rust native executable","runtime_version":version,"runtime_artifact_sha256":environment["runtime"]["executable_sha256"],"backend":"rust-unicode-normalization","hardware_target":"cpu","unicode_version":"16.0.0","environment_profile_binding":selected.binding(&path("environment-profile.json"))?});
    event["measurements"][0]["value"] = json!(duration_ms);
    event["measurements"][0]["method"] = json!(
        "std::time::Instant around the bounded variant transformation and exact output write"
    );
    event["measurements"][1]["value"] = json!(input.len());
    event["measurements"][1]["method"] = json!("Length of the exact held input bytes");
    event["measurements"][2]["value"] = if exit == 0 {
        json!(bytes.len())
    } else {
        Value::Null
    };
    if exit == 0 {
        event["measurements"][2]["method"] =
            json!("Length of the exact bytes committed by the native writer");
    }
    event["evidence_authentication"]["producer_control_boundary"] = json!(
        "The same local native process performed the operation and wrote this unsigned record. Byte closure does not independently authenticate execution truth."
    );
    event["reproducibility"] = json!({"classification":"partially_specified","known_gaps":["The execution receipt is unsigned and self-reported by the transformation runner.","Host-local invocation paths are withheld; only the actual canonical argv digest is public."],"replay_scope":"The explicit A/B/C native commands reproduce the declared synthetic output bytes using the tracked plan, fixture, native builder, executable digest and Unicode version. Repeating execution captures a new event and measured duration."});
    schema(ctx, &selected, &event)?;
    selected.verify(ctx)?;
    let event_ref = path(&format!("variant-{}.event.v2.json", id.to_lowercase()));
    save(ctx, &event_ref, &encode(&event, true)?)?;
    Ok((
        json!({"variant":id,"status":if exit==0 {"completed"} else {"failed"},"exit_code":exit,"event_ref":event_ref,"event_id":event["event_id"],"execution_truth_established":false}),
        exit,
    ))
}
pub fn finalize(ctx: &ResearchExecution, build: bool) -> Result<Value> {
    let mut selected = inputs(ctx)?;
    selected.read(ctx, &path("environment-profile.json"))?;
    for file in [
        "variant-a.event.v2.json",
        "variant-b.event.v2.json",
        "variant-c.event.v2.json",
        "variant-a.copy.txt",
        "variant-b.nfc.txt",
        "variant-c.failure.log",
    ] {
        selected.read(ctx, &path(file))?;
    }
    if !build {
        selected.read(ctx, &path("lab.manifest.json"))?;
        let existing = selected.json(&path("lab.manifest.json"))?;
        const HISTORICAL_BUILDER: &str = "scripts/build_provenance_event_v2_lab.py";
        if existing["builder"]["ref"] == HISTORICAL_BUILDER {
            // The retained script is exact historical input bytes only. It is
            // never imported or executed, and no recorded event is rewritten.
            let digest = s(&existing["builder"]["sha256"])?;
            ensure(
                digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
                "historical builder digest shape",
            )?;
            let archive = format!(
                "ToS/research-packets/retained-builder-inputs/build_provenance_event_v2_lab/{digest}.py"
            );
            selected.read(ctx, &archive)?;
            let builder = &selected.files[&archive];
            ensure(
                sha(builder) == digest,
                "retained historical builder fixity differs",
            )?;
            let contract_digest = s(&existing["contract"]["sha256"])?;
            ensure(
                existing["contract"]["ref"] == CONTRACT
                    && contract_digest.len() == 64
                    && contract_digest.bytes().all(|b| b.is_ascii_hexdigit()),
                "historical contract binding shape",
            )?;
            if sha(&selected.files[CONTRACT]) != contract_digest {
                let archived_contract = format!("ToS/contracts/history/{contract_digest}.json");
                selected.read(ctx, &archived_contract)?;
                ensure(
                    sha(&selected.files[&archived_contract]) == contract_digest
                        && selected.json(&archived_contract)?["$id"]
                            == format!("https://tree-of-sophia.local/{CONTRACT}"),
                    "retained historical contract fixity or identity differs",
                )?;
            }
            crate::synthetic_foundation_labs::validate_overlay(
                ctx,
                &selected.files,
                BTreeMap::new(),
                HISTORICAL_BUILDER,
                &selected.files[&archive],
                CONTRACT,
                tos_validation::source_foundation_labs::SourceFoundationLab::ProvenanceV2,
            )?;
            selected.verify(ctx)?;
            return Ok(
                json!({"status":"passed","historical_records_preserved":true,"variants":3,"negative_controls":14,"written":false,"execution_truth_established":false,"source_admission_performed":false,"human_review_performed":false,"canon_effect":false}),
            );
        }
    }
    let mut manifest = definitions()["manifest"].clone();
    manifest["builder"] =
        json!({"ref":BUILDER,"sha256":sha(include_bytes!("provenance_event_lab.rs"))});
    for (name, reference) in [
        ("contract", CONTRACT.to_string()),
        ("plan", path("plan.json")),
        ("environment_profile", path("environment-profile.json")),
        ("input_fixture", path("input-fixture.txt")),
    ] {
        manifest[name] = selected.binding(&reference)?;
    }
    let builder_binding = manifest["builder"].clone();
    let environment = selected.json(&path("environment-profile.json"))?;
    for row in manifest["variants"]
        .as_array_mut()
        .ok_or("compiled variants")?
    {
        let event_ref = s(&row["event_ref"])?;
        let event = selected.json(event_ref)?;
        schema(ctx, &selected, &event)?;
        ensure(
            event["method"]["environment"]["environment_profile_binding"]
                == selected.binding(&path("environment-profile.json"))?
                && event["responsibility"][0]["evidence_binding"] == builder_binding
                && event["method"]["environment"]["runtime_artifact_sha256"]
                    == environment["runtime"]["executable_sha256"]
                && event["method"]["environment"]["runtime_version"]
                    == environment["runtime"]["version"]
                && event["method"]["environment"]["unicode_version"]
                    == environment["runtime"]["unicode_version"],
            "variant environment or builder differs",
        )?;
        for kind in ["event", "input", "output", "byproduct"] {
            if let Some(reference) = row[format!("{kind}_ref")].as_str() {
                let digest = sha(selected
                    .files
                    .get(reference)
                    .ok_or("unselected variant member")?);
                row[format!("{kind}_sha256")] = json!(digest);
            }
        }
        row["captured_command"] = event["method"]["command_capture"]["argv"].clone();
        row["captured_command_sha256"] = event["method"]["command_capture"]["argv_sha256"].clone();
    }
    let manifest_ref = path("lab.manifest.json");
    let raw = encode(&manifest, true)?;
    let mut files = selected.files.clone();
    files.insert(manifest_ref.clone(), raw.clone());
    crate::synthetic_foundation_labs::validate_overlay(
        ctx,
        &files,
        BTreeMap::new(),
        BUILDER,
        include_bytes!("provenance_event_lab.rs"),
        CONTRACT,
        tos_validation::source_foundation_labs::SourceFoundationLab::ProvenanceV2,
    )?;
    let prior = read_optional(ctx, &manifest_ref)?;
    ensure(
        build || prior.as_deref() == Some(raw.as_slice()),
        "provenance lab manifest stale",
    )?;
    selected.verify(ctx)?;
    let written = if build {
        save(ctx, &manifest_ref, &raw)?
    } else {
        false
    };
    Ok(
        json!({"status":"passed","variants":3,"negative_controls":14,"manifest":manifest_ref,"written":written,"execution_truth_established":false,"source_admission_performed":false,"human_review_performed":false,"canon_effect":false}),
    )
}
