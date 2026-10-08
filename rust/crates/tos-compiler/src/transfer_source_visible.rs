//! Reconstruct a text-free receipt from the explicitly selected private review.
//! This verifies retained evidence; it neither performs nor promotes its review.
use crate::{
    research_execution::ResearchExecution,
    research_html::anchored_text,
    research_text_comparison::{alpha_tokens, source_aware_text, token_diff},
    source_text_foundation::{
        ensure, fresh_or_matching_limit, load, private_boundary, s, schema, sha,
    },
    transfer_target_passages::{n, read_optional},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::Path,
};
const GOLD: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1";
const PRIVATE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/transfer-source-visible-review/v1/jenseits-187/review-bundle.json";
const PRIVATE_SCHEMA: &str =
    "ToS/contracts/private-transfer-source-visible-review-bundle.schema.json";
const RECEIPT_SCHEMA: &str = "ToS/contracts/transfer-source-visible-review-receipt.schema.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/transfer_source_visible.rs";
const LEGACY: &str = "scripts/record_golden_kernel_transfer_source_visible_review.py";
const LEGACY_SHA: &str = "ac20b7dce1259aa3ce6d6efaf356dafa010c9fd7af392caf1c47d58b0a2f6679";
const LEGACY_BUNDLE: &str = "4db81ebd5c819da431faca7a5a02b5c4a58e7c8a69e26b2b2cd5cac517da4a2b";
const CAP: usize = 4 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;
fn a(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("required array".into())
}
fn canonical(v: &Value) -> Result<Vec<u8>> {
    fn sorted(v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                let b = m
                    .iter()
                    .map(|(k, v)| (k.clone(), sorted(v)))
                    .collect::<BTreeMap<_, _>>();
                Value::Object(b.into_iter().collect())
            }
            Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
            _ => v.clone(),
        }
    }
    let mut raw = serde_json::to_vec(&sorted(v)).map_err(|e| e.to_string())?;
    raw.push(b'\n');
    ensure(raw.len() <= CAP, "receipt byte bound")?;
    Ok(raw)
}
fn builder_sha() -> String {
    let mut h = tos_foundation::Digest256Hasher::new();
    for b in [
        include_bytes!("transfer_source_visible.rs").as_slice(),
        include_bytes!("transfer_source_visible/templates.json").as_slice(),
        include_bytes!("research_html.rs").as_slice(),
        include_bytes!("research_html_entities.rs").as_slice(),
        include_bytes!("research_text_comparison.rs").as_slice(),
    ] {
        h.update(&(b.len() as u64).to_be_bytes());
        h.update(b)
    }
    h.finalize().to_hex()
}
fn utc_timestamp(value: &str) -> Result<String> {
    // Private schema validates RFC3339. Preserve Python microsecond formatting.
    let raw = value.as_bytes();
    ensure(
        raw.len() >= 20 && raw[..19].is_ascii(),
        "review timestamp syntax",
    )?;
    let num = |a, b| {
        value
            .get(a..b)
            .ok_or("timestamp range")?
            .parse::<i32>()
            .map_err(|_| "timestamp number")
    };
    ensure(
        raw[4] == b'-'
            && raw[7] == b'-'
            && matches!(raw[10], b'T' | b't')
            && raw[13] == b':'
            && raw[16] == b':',
        "timestamp separators",
    )?;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = num(0, 4)? - 1900;
    tm.tm_mon = num(5, 7)? - 1;
    tm.tm_mday = num(8, 10)?;
    tm.tm_hour = num(11, 13)?;
    tm.tm_min = num(14, 16)?;
    tm.tm_sec = num(17, 19)?;
    let mut tail = &value[19..];
    let mut micros = String::new();
    if let Some(frac) = tail.strip_prefix('.') {
        let count = frac.bytes().take_while(u8::is_ascii_digit).count();
        ensure(count > 0, "timestamp fraction")?;
        micros = frac[..count.min(6)].to_owned();
        while micros.len() < 6 {
            micros.push('0')
        }
        tail = &frac[count..];
    }
    let offset = if matches!(tail, "Z" | "z") {
        0
    } else {
        let b = tail.as_bytes();
        ensure(
            b.len() == 6 && matches!(b[0], b'+' | b'-') && b[3] == b':',
            "timestamp zone",
        )?;
        let hours = tail[1..3].parse::<i64>().map_err(|_| "zone hour")?;
        let minutes = tail[4..6].parse::<i64>().map_err(|_| "zone minute")?;
        ensure(hours < 24 && minutes < 60, "zone range")?;
        (hours * 3600 + minutes * 60) * if b[0] == b'+' { 1 } else { -1 }
    };
    let epoch = unsafe { libc::timegm(&mut tm) }
        .checked_sub(offset)
        .ok_or("timestamp overflow")?;
    let mut out = std::mem::MaybeUninit::<libc::tm>::uninit();
    ensure(
        !unsafe { libc::gmtime_r(&epoch, out.as_mut_ptr()) }.is_null(),
        "timestamp UTC",
    )?;
    let t = unsafe { out.assume_init() };
    let fraction = if micros.bytes().any(|b| b != b'0') {
        format!(".{micros}")
    } else {
        String::new()
    };
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{fraction}Z",
        t.tm_year + 1900,
        t.tm_mon + 1,
        t.tm_mday,
        t.tm_hour,
        t.tm_min,
        t.tm_sec
    ))
}
struct RenderTool {
    root: ResearchExecution,
    file: File,
    digest: String,
    command: String,
}
impl RenderTool {
    fn open(ctx: &ResearchExecution) -> Result<Self> {
        use crate::owned_native_child::{CaptureLimits, capture_with_cancel};
        let p = std::fs::canonicalize("/usr/bin/pdftoppm").map_err(|e| e.to_string())?;
        let root = ctx.select_directory(p.parent().ok_or("renderer parent")?)?;
        let mut file = root.source_file(
            p.file_name()
                .and_then(|n| n.to_str())
                .ok_or("renderer filename")?,
            32 * 1024 * 1024,
        )?;
        let digest = root.hash_file(&mut file, 32 * 1024 * 1024)?;
        let command = format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd());
        let r = capture_with_cancel(
            std::process::Command::new(&command).arg("-v"),
            None,
            CaptureLimits {
                max_stdin_bytes: 0,
                max_stdout_bytes: 16384,
                max_stderr_bytes: 16384,
            },
            ctx.deadline(),
            ctx.cancellation_flag(),
        )?;
        let mut raw = r.stdout;
        raw.extend(r.stderr);
        ensure(
            r.status.success()
                && std::str::from_utf8(&raw)
                    .map_err(|_| "renderer version UTF8")?
                    .lines()
                    .next()
                    == Some("pdftoppm version 26.01.0"),
            "exact render tool version",
        )?;
        Ok(Self {
            root,
            file,
            digest,
            command,
        })
    }
    fn page(
        &self,
        ctx: &ResearchExecution,
        pdf: &File,
        page: usize,
        dpi: usize,
        expected: &str,
    ) -> Result<()> {
        use crate::owned_native_child::{CaptureLimits, capture_with_cancel};
        ensure(
            page > 0 && page <= 100000 && dpi == 180,
            "render page/resolution bound",
        )?;
        let pdf = format!("/proc/{}/fd/{}", std::process::id(), pdf.as_raw_fd());
        let r = capture_with_cancel(
            std::process::Command::new(&self.command).args([
                "-f",
                &page.to_string(),
                "-l",
                &page.to_string(),
                "-r",
                &dpi.to_string(),
                "-png",
                "-singlefile",
                &pdf,
            ]),
            None,
            CaptureLimits {
                max_stdin_bytes: 0,
                max_stdout_bytes: 32 * 1024 * 1024,
                max_stderr_bytes: 16384,
            },
            ctx.deadline(),
            ctx.cancellation_flag(),
        )?;
        ctx.tick(r.stdout.len() as u64)?;
        ensure(
            r.status.success()
                && r.stdout.starts_with(b"\x89PNG\r\n\x1a\n")
                && sha(&r.stdout) == expected,
            "reproduced page render differs",
        )
    }
    fn verify(&mut self) -> Result<()> {
        ensure(
            self.root.hash_file(&mut self.file, 32 * 1024 * 1024)? == self.digest,
            "renderer executable changed",
        )
    }
}
#[derive(Clone, Copy)]
pub enum Action {
    Build,
    Check,
    ValidateTracked,
}
pub struct Options<'a> {
    pub input_root: Option<&'a Path>,
    pub generation: Option<&'a str>,
    pub action: Action,
}
fn paths(g: &str) -> Result<(String, String)> {
    ensure(
        !g.is_empty() && g.len() <= 64 && g.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "generation name",
    )?;
    Ok((
        format!("{GOLD}/transfer-source-visible-review.jenseits-187.{g}.json"),
        format!("tos.transfer-source-visible-review.jenseits-187.{g}"),
    ))
}
fn current_rights(ctx: &ResearchExecution, w: &Value) -> Result<(Vec<u8>, Value)> {
    let (raw, r) = load(ctx, s(&w["rights_ref"])?)?;
    ensure(
        r["visibility"] == "local_only"
            && r["derivative_posture"] == "local_research_only"
            && r["redistribution_posture"] == "not_authorized"
            && a(&r["scope_refs"])?.contains(&w["item_ref"])
            && a(&r["scope_refs"])?.contains(&w["file_ref"]),
        "current witness rights drift",
    )?;
    Ok((raw, r))
}
fn witness_receipt(ctx: &ResearchExecution, w: &Value) -> Result<Value> {
    let (raw, _) = current_rights(ctx, w)?;
    let mut out = serde_json::Map::new();
    for k in [
        "language",
        "expression_ref",
        "item_ref",
        "file_ref",
        "file_sha256",
        "rights_ref",
        "automatic_candidate_ref",
        "automatic_candidate_sha256",
    ] {
        out.insert(k.into(), w[k].clone());
    }
    out.insert("rights_sha256".into(), json!(sha(&raw)));
    out.insert(
        "page_renders".into(),
        json!(
            a(&w["pages"])?
                .iter()
                .map(|p| json!({"page":p["page"],"sha256":p["render_sha256"]}))
                .collect::<Vec<_>>()
        ),
    );
    Ok(Value::Object(out))
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let generation = opts.generation.unwrap_or("v1");
    let historical = generation == "v1";
    let (output, id) = paths(generation)?;
    let templates: Value =
        serde_json::from_str(include_str!("transfer_source_visible/templates.json"))
            .map_err(|e| e.to_string())?;
    if matches!(opts.action, Action::ValidateTracked) {
        let (_, v) = load(ctx, &output)?;
        schema(ctx, RECEIPT_SCHEMA, &v)?;
        ensure(
            v["receipt_id"] == id
                && v["effects"] == templates["effects"]
                && v["rights_and_visibility"] == templates["rights_and_visibility"],
            "receipt identity/authority drift",
        )?;
        for side in ["source", "target"] {
            let w = &v["evidence_inputs"][side];
            let (raw, _) = current_rights(ctx, w)?;
            ensure(
                sha(&raw) == s(&w["rights_sha256"])?,
                "receipt rights fixity",
            )?;
            private_boundary(ctx, s(&w["automatic_candidate_ref"])?)?;
        }
        private_boundary(ctx, PRIVATE)?;
        ensure(
            v["generator"]["ref"] == if historical { LEGACY } else { BUILDER }
                && v["generator"]["sha256"]
                    == if historical {
                        LEGACY_SHA.into()
                    } else {
                        builder_sha()
                    },
            "receipt generator binding",
        )?;
        return Ok(
            json!({"status":"passed","action":"validate-tracked","private_inputs_read":false,"receipt":output}),
        );
    }
    let local = ctx.select_directory(
        opts.input_root
            .ok_or("explicit private input root required")?,
    )?;
    private_boundary(ctx, PRIVATE)?;
    let mut bf = local.source_file(PRIVATE, CAP as u64)?;
    ensure(
        bf.metadata()
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o777
            == 0o600,
        "private review bundle mode",
    )?;
    let bytes = local.read_file(&mut bf, CAP as u64)?;
    tos_foundation::parse_json(
        &bytes,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(|e| e.to_string())?;
    let bundle: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    schema(ctx, PRIVATE_SCHEMA, &bundle)?;
    if historical {
        ensure(
            sha(&bytes) == LEGACY_BUNDLE,
            "historical review bundle differs",
        )?;
    }
    let mut tool = RenderTool::open(ctx)?;
    let mut pdfs = vec![];
    let mut candidates = vec![];
    let mut witnesses = vec![];
    let mut rendered = 0;
    for side in ["source", "target"] {
        let w = &bundle[side];
        current_rights(ctx, w)?;
        let payload = s(&w["payload_ref"])?;
        private_boundary(ctx, payload)?;
        let mut file = local.source_file(payload, 256 * 1024 * 1024)?;
        ensure(
            local.hash_file(&mut file, 256 * 1024 * 1024)? == s(&w["file_sha256"])?
                && w["file_ref"] == format!("tos.file.sha256.{}", s(&w["file_sha256"])?),
            "witness PDF fixity",
        )?;
        for p in a(&w["pages"])? {
            tool.page(
                ctx,
                &file,
                n(&p["page"])?,
                n(&bundle["render_method"]["resolution_dpi"])?,
                s(&p["render_sha256"])?,
            )?;
            rendered += 1;
        }
        pdfs.push((file, s(&w["file_sha256"])?.to_string()));
        let cr = s(&w["automatic_candidate_ref"])?;
        private_boundary(ctx, cr)?;
        let (raw, c) = load(&local, cr)?;
        ensure(
            sha(&raw) == s(&w["automatic_candidate_sha256"])?
                && c["qualified_unit_key"] == bundle["qualified_unit_key"],
            "automatic candidate binding",
        )?;
        candidates.push(c);
        witnesses.push(witness_receipt(ctx, w)?);
    }
    ensure(rendered == 4, "four rendered pages required")?;
    let critical = &bundle["source"]["critical_witness"];
    let cr = s(&critical["local_payload_ref"])?;
    private_boundary(ctx, cr)?;
    let mut html = local.source_file(cr, CAP as u64)?;
    let raw = local.read_file(&mut html, CAP as u64)?;
    ensure(
        sha(&raw) == s(&critical["local_payload_sha256"])?
            && raw.len() == n(&critical["local_payload_bytes"])?,
        "critical payload fixity",
    )?;
    let selected = alpha_tokens(&anchored_text(
        ctx,
        std::str::from_utf8(&raw).map_err(|_| "critical HTML UTF8")?,
        s(&critical["selector"]["value"])?,
    )?);
    ensure(
        selected == alpha_tokens(s(&critical["text"])?),
        "critical declared text differs from selected alpha tokens",
    )?;
    let mut diplomat = vec![];
    for side in ["source", "target"] {
        let lines = a(&bundle[side]["diplomatic_lines"])?
            .iter()
            .map(|v| Ok(s(&v["text"])?.to_string()))
            .collect::<Result<Vec<_>>>()?;
        diplomat.push(alpha_tokens(&source_aware_text(&lines)));
    }
    let mut sc = token_diff(ctx, &selected, &diplomat[0])?;
    sc["comparison_role"] = json!("critical_comparison_not_historical_identity");
    sc["historical_critical_equivalence_established"] = json!(false);
    let mut sa = token_diff(
        ctx,
        &diplomat[0],
        &alpha_tokens(s(&candidates[0]["automatic_candidate_text"])?),
    )?;
    sa["comparison_role"] = json!("automatic_ocr_diagnostic_against_private_model_transcript");
    let mut ta = token_diff(
        ctx,
        &diplomat[1],
        &alpha_tokens(s(&candidates[1]["automatic_candidate_text"])?),
    )?;
    ta["comparison_role"] =
        json!("automatic_extraction_diagnostic_against_private_model_transcript");
    ta["observed_complete_visible_line_omission_count"] = json!(1);
    let receipt = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/transfer-source-visible-review-receipt.schema.json","schema_version":"tos_transfer_source_visible_review_receipt_v1","generated_or_authored":"generated_from_private_model_source_visible_review_bundle","receipt_id":id,"recorded_at_utc":utc_timestamp(s(&bundle["created_at"])?)?,"status":"machine-triangulated-candidate-not-admitted","route_readiness_id":bundle["route_readiness_id"],"work_ref":bundle["work_ref"],"qualified_unit_key":bundle["qualified_unit_key"],"generator":{"ref":if historical{LEGACY}else{BUILDER},"sha256":if historical{LEGACY_SHA.into()}else{builder_sha()}},"private_bundle":{"owner":"operator-local-tree-of-sophia","relative_ref":PRIVATE,"sha256":sha(&bytes),"bytes":bytes.len(),"mode":"0600","source_bearing":true,"review_mode":"model_source_visible","human_review_performed":false},"evidence_inputs":{"source":witnesses[0],"target":witnesses[1],"critical_witness":{"stable_locator":critical["stable_locator"],"source_role":critical["source_role"],"payload_sha256":critical["local_payload_sha256"],"payload_bytes":critical["local_payload_bytes"],"selector_kind":critical["selector"]["kind"],"selector_value":critical["selector"]["value"],"transport":critical["retrieval"]["transport"],"bounded_repeat_fetches":critical["retrieval"]["bounded_repeat_fetches"],"repeat_fetches_byte_identical":critical["retrieval"]["repeat_fetches_byte_identical"],"transport_authenticated":false}},"reproduction":{"private_schema_validated":true,"private_fixity_verified":true,"source_payload_fixity_verified":true,"target_payload_fixity_verified":true,"automatic_candidate_fixity_verified":true,"critical_payload_fixity_verified":true,"critical_selector_resolved":true,"critical_declared_text_matches_selected_alpha_tokens":true,"render_tool":bundle["render_method"]["tool"],"resolution_dpi":bundle["render_method"]["resolution_dpi"],"render_count":4,"render_hashes_reproduced":true,"temporary_renders_retained":false,"network_used":false},"comparisons":{"source_diplomatic_against_critical":sc,"source_automatic_against_diplomatic":sa,"target_automatic_against_diplomatic":ta},"observed_findings":a(&bundle["observed_findings"])?.iter().map(|f|json!({"finding_id":f["finding_id"],"scope":f["scope"],"severity":f["severity"],"evidence_role":"model_observation_not_human_truth"})).collect::<Vec<_>>(),"effects":templates["effects"],"rights_and_visibility":templates["rights_and_visibility"],"authority_boundary":templates["authority_boundary"]});
    schema(ctx, RECEIPT_SCHEMA, &receipt)?;
    for (file, expected) in &mut pdfs {
        ensure(
            local.hash_file(file, 256 * 1024 * 1024)? == *expected,
            "witness PDF changed",
        )?;
    }
    tool.verify()?;
    ensure(
        local.hash_file(&mut bf, CAP as u64)? == sha(&bytes)
            && local.hash_file(&mut html, CAP as u64)? == sha(&raw),
        "review inputs changed",
    )?;
    for side in ["source", "target"] {
        let w = &bundle[side];
        let (raw, _) = load(&local, s(&w["automatic_candidate_ref"])?)?;
        ensure(
            sha(&raw) == s(&w["automatic_candidate_sha256"])?,
            "candidate changed",
        )?;
        let now = witness_receipt(ctx, w)?;
        ensure(
            now == receipt["evidence_inputs"][side],
            "witness rights changed",
        )?;
    }
    let encoded = canonical(&receipt)?;
    let missing = fresh_or_matching_limit(ctx, &output, &encoded, CAP)?;
    ensure(
        matches!(opts.action, Action::Build) || !missing,
        "receipt absent",
    )?;
    if missing {
        ctx.write(&output, &encoded, 0o644, true)?;
    }
    Ok(
        json!({"status":"passed","receipt":output,"sha256":sha(&encoded),"bytes":encoded.len(),"historical_replay":historical,"renders_verified":rendered,"new_source_visible_review":false,"written":missing}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamp_conversion_and_generation_identity() {
        assert_eq!(
            utc_timestamp("2026-08-10T15:15:24+06:00").unwrap(),
            "2026-08-10T09:15:24Z"
        );
        assert_eq!(
            utc_timestamp("2026-08-10T09:15:24.12Z").unwrap(),
            "2026-08-10T09:15:24.120000Z"
        );
        assert!(paths("../out").is_err());
        assert_ne!(paths("v1").unwrap(), paths("native-r1").unwrap());
    }
}
