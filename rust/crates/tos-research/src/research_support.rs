//! Shared source custody helpers retained while research stages migrate.
//! Consolidate with the source-passage implementation when that stage moves.
use serde_json::{Value, json};
use tos_foundation::{JsonLimits, JsonMode};
type ResearchExecution = Execution;
type Result<T> = std::result::Result<T, String>;
const CAP: usize = 4 * 1024 * 1024;
use tos_compiler::owned_native_child::{CaptureLimits, capture_with_cancel};
use tos_source_store::execution::Execution;

pub fn ensure(value: bool, message: &str) -> Result<()> {
    if value { Ok(()) } else { Err(message.into()) }
}
pub fn sha(raw: &[u8]) -> String {
    tos_foundation::Digest256::of_bytes(raw).to_hex()
}
pub fn s(value: &Value) -> Result<&str> {
    value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("required nonempty string".into())
}
pub fn read_optional(ctx: &Execution, reference: &str) -> Result<Option<Vec<u8>>> {
    match std::fs::symlink_metadata(ctx.root().join(reference)) {
        Ok(_) => Ok(Some(ctx.read(reference)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub fn private_boundary(ctx: &Execution, reference: &str) -> Result<()> {
    use std::os::fd::AsRawFd;
    ensure(
        reference.contains("/local-content/"),
        "private text output must remain below local-content",
    )?;
    for (args, expected) in [
        (vec!["check-ignore", "-q", "--", reference], 0),
        (vec!["ls-files", "--error-unmatch", "--", reference], 1),
    ] {
        let result = capture_with_cancel(
            std::process::Command::new("/usr/bin/git")
                .args(args)
                .current_dir(format!(
                    "/proc/self/fd/{}",
                    ctx.root_directory().as_raw_fd()
                )),
            None,
            CaptureLimits {
                max_stdin_bytes: 0,
                max_stdout_bytes: 4096,
                max_stderr_bytes: 16384,
            },
            ctx.deadline(),
            ctx.cancellation_flag(),
        )?;
        ensure(
            result.status.code() == Some(expected),
            "private text must be Git-ignored and untracked",
        )?;
    }
    Ok(())
}
pub fn utc_now() -> Result<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let raw = libc::time_t::try_from(secs).map_err(|_| "timestamp out of range")?;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    ensure(
        !unsafe { libc::gmtime_r(&raw, tm.as_mut_ptr()) }.is_null(),
        "UTC conversion failed",
    )?;
    let t = unsafe { tm.assume_init() };
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.tm_year + 1900,
        t.tm_mon + 1,
        t.tm_mday,
        t.tm_hour,
        t.tm_min,
        t.tm_sec
    ))
}

pub fn encode(v: &Value, pretty: bool) -> Result<Vec<u8>> {
    let raw = serde_json::to_vec(v).map_err(|e| e.to_string())?;
    let limits = JsonLimits::new(CAP, 128, 100000, 4300).map_err(|e| e.to_string())?;
    let doc = tos_foundation::parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| e.to_string())?;
    if pretty {
        let (mut bytes, _) = tos_foundation::emit_python_pretty_sorted_json_with_state_budget(
            doc.root(),
            limits,
            32 * CAP,
            &mut || Ok(()),
            &mut |_, _| Ok(()),
        )
        .map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    } else {
        tos_foundation::emit_python_compact_json(doc.root(), limits).map_err(|e| e.to_string())
    }
}

pub fn schema(ctx: &ResearchExecution, reference: &str, value: &Value) -> Result<()> {
    use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};
    let raw = ctx.read(reference)?;
    let uri = format!("https://tree-of-sophia.local/{reference}");
    let probe = SchemaBackendProbe::new(
        [SchemaResource {
            uri: uri.clone(),
            raw,
        }],
        FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("schema preparation: {e:?}"))?;
    ensure(
        probe
            .is_valid_raw(&uri, &encode(value, false)?)
            .map_err(|e| format!("schema execution: {e:?}"))?,
        &format!("{reference} validation failed"),
    )?;
    ctx.check()
}

pub fn fresh_or_matching(ctx: &ResearchExecution, reference: &str, bytes: &[u8]) -> Result<bool> {
    fresh_or_matching_limit(ctx, reference, bytes, CAP)
}

pub fn fresh_or_matching_limit(
    ctx: &ResearchExecution,
    reference: &str,
    bytes: &[u8],
    cap: usize,
) -> Result<bool> {
    ensure(bytes.len() <= cap, "output byte bound")?;
    // Distinguish an absent file from an unsafe/read-failed existing one before writes.
    match std::fs::symlink_metadata(ctx.root().join(reference)) {
        Ok(_) => {
            let mut f = ctx.source_file(reference, cap as u64)?;
            ensure(
                ctx.read_file(&mut f, cap as u64)? == bytes,
                "existing output differs; preserve it and select a new source-owned plan/revision",
            )?;
            Ok(false)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.to_string()),
    }
}

const UNIT_PACKET_SCHEMA_SHA: &str =
    "6ba96f4298cf5e88ae261c53c46c7d51681d0f0f0faf3bcecce2e309a0e071ab";
pub fn unit_packet_schema(ctx: &ResearchExecution, p: &Value) -> Result<()> {
    use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};
    let raw = ctx.read("ToS/contracts/source-text-unit-packet-v1.schema.json")?;
    // V1 is an exact retained contract. Its two large arrays have only
    // minItems:1/items constraints and no global uniqueness/cardinality clause.
    // A changed schema must receive a new decomposition review, never silently
    // weaken its new law to fit the backend's 1MiB instance cap.
    ensure(
        sha(&raw) == UNIT_PACKET_SCHEMA_SHA,
        "v1 packet schema bytes changed; source-owned adapter review required",
    )?;
    let uri = "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json"
        .to_owned();
    let probe = SchemaBackendProbe::new(
        [SchemaResource {
            uri: uri.clone(),
            raw,
        }],
        FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("packet schema preparation: {e:?}"))?;
    let anchors = p["anchors"].as_array().ok_or("packet anchors array")?;
    let units = p["units"].as_array().ok_or("packet units array")?;
    ensure(
        !anchors.is_empty() && !units.is_empty(),
        "packet arrays must be nonempty",
    )?;
    let mut base = p.clone();
    base["anchors"] = json!([anchors[0]]);
    base["units"] = json!([units[0]]);
    for index in 0..anchors.len().max(units.len()).div_ceil(50) {
        ctx.tick(1)?;
        let mut chunk = base.clone();
        for (key, rows) in [("anchors", anchors), ("units", units)] {
            let start = index * 50;
            chunk[key] = json!(if start < rows.len() {
                &rows[start..(start + 50).min(rows.len())]
            } else {
                &rows[..1]
            });
        }
        let encoded = encode(&chunk, false)?;
        ensure(
            probe
                .is_valid_raw(&uri, &encoded)
                .map_err(|e| format!("packet schema execution: {e:?}"))?,
            "source-text-unit schema failure",
        )?;
        ctx.check()?;
    }
    Ok(())
}

pub fn load(ctx: &ResearchExecution, r: &str) -> Result<(Vec<u8>, Value)> {
    let mut f = ctx.source_file(r, CAP as u64)?;
    let raw = ctx.read_file(&mut f, CAP as u64)?;
    tos_foundation::parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default())
        .map_err(|e| e.to_string())?;
    let v = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    Ok((raw, v))
}

pub fn metadata(ctx: &ResearchExecution, kind: &str, reference: &str, value: &Value) -> Result<()> {
    use tos_validation::text_metadata_rules::*;
    let raw = encode(value, false)?;
    let limits = TextMetadataLimits {
        max_packet_bytes: 2 * 1024 * 1024,
        max_state_bytes: 32 * CAP,
        max_issues: 64,
        deadline: ctx.deadline(),
    };
    let report = match kind {
        "anchor" => {
            inspect_source_anchor_v2_metadata(&raw, reference, limits, ctx.cancellation_flag())
        }
        "layer" => {
            inspect_source_text_layer_metadata(&raw, reference, limits, ctx.cancellation_flag())
        }
        _ => inspect_source_text_unit_v1_metadata(&raw, reference, limits, ctx.cancellation_flag()),
    }
    .map_err(|e| format!("metadata: {e:?}"))?;
    ensure(
        report.state == TextMetadataState::CheckedMetadata && report.issues.is_empty(),
        &format!("{kind} semantic checks: {:?}", report.issues),
    )
}
