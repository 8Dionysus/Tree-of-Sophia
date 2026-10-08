//! Deterministic private whole-page selection before transfer variants.
//! Frozen historical assessments are verified; a fresh write needs explicit review.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{
        ensure, fresh_or_matching_limit, load, private_boundary, s, schema, sha, utc_now,
    },
    transfer_target_passages::{Poppler, encode, json_lines, jsonl, n, read_optional},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::PermissionsExt,
    path::Path,
};
#[path = "transfer_candidates/constants.rs"]
mod constants;
use constants::*;
type Result<T> = std::result::Result<T, String>;
const CAP: usize = 4 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/transfer_candidates.rs";
const LEGACY_EVENT_SHA: &str = "b87ba0c258b9d21b9d675d16691054e02adb90b2cf597f20e9d60299a663c939";
const LEGACY_BUILDER_SHA: &str = "71f61e32b16bd797ae3bc80c97b9cba454539661422a1ff652ddefbf2e849d2c";
fn a(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("required array".into())
}
fn python_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
fn linebreak(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\x0b'
            | '\x0c'
            | '\x1c'
            | '\x1d'
            | '\x1e'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}
fn heading(line: &str) -> bool {
    let v = line
        .strip_suffix('.')
        .or_else(|| line.strip_suffix(')'))
        .unwrap_or(line);
    let len = v.chars().count();
    (len > 0 && len <= 3 && v.bytes().all(|c| c.is_ascii_digit()))
        || (len > 0 && len <= 8 && v.bytes().all(|c| b"IVXLCDM".contains(&c)))
}
fn alpha(c: char) -> bool {
    static LETTER: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^\p{L}$").unwrap());
    let mut bytes = [0u8; 4];
    LETTER.is_match(c.encode_utf8(&mut bytes))
}
fn metrics(raw: &[u8]) -> Result<Value> {
    let text = std::str::from_utf8(raw).map_err(|_| "page UTF-8")?;
    let lines = text
        .split(linebreak)
        .map(|l| l.trim_matches(python_space))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>();
    let substantive = lines
        .iter()
        .copied()
        .filter(|l| {
            l.chars().count() > 2 && !(l.len() <= 4 && l.bytes().all(|c| c.is_ascii_digit()))
        })
        .collect::<Vec<_>>();
    let nonspace = text.chars().filter(|c| !python_space(*c)).count();
    let alphabetic = text.chars().filter(|c| alpha(*c)).count();
    let punctuation = text
        .chars()
        .filter(|c| ".,;:!?—–-«»\"()…".contains(*c))
        .count();
    let hyphen = lines
        .iter()
        .filter(|l| l.ends_with(['-', '—', '–']))
        .count();
    let headings = lines.iter().filter(|l| heading(l)).count();
    let mut edges = 0;
    if let Some(first) = substantive.first() {
        if first.chars().next().is_some_and(char::is_lowercase)
            || first.ends_with([',', ';', ':', '-', '—', '–'])
        {
            edges += 1
        }
        if substantive
            .last()
            .unwrap()
            .ends_with([',', ';', ':', '-', '—', '–'])
        {
            edges += 1
        }
    }
    Ok(
        json!({"nonspace_characters":nonspace,"alphabetic_characters":alphabetic,"nonblank_lines":lines.len(),"punctuation_characters":punctuation,"line_end_hyphenations":hyphen,"page_edge_fragment_signals":edges,"numbered_heading_candidates":headings,"mechanical_hardness_score":nonspace+punctuation*6+hyphen*160+edges*250+headings*100}),
    )
}
fn signals(m: &Value, stratum: &str) -> Result<Vec<&'static str>> {
    let mut out = vec![if stratum == "random" {
        "digest-random-baseline"
    } else {
        "mechanical-hardness-top-rank"
    }];
    for (key, threshold, label) in [
        ("nonspace_characters", 3200, "dense-text"),
        ("punctuation_characters", 180, "punctuation-rich"),
        ("line_end_hyphenations", 1, "line-end-hyphenation"),
        ("page_edge_fragment_signals", 1, "page-edge-fragment-risk"),
        (
            "numbered_heading_candidates",
            1,
            "numbered-heading-candidate",
        ),
    ] {
        if n(&m[key])? >= threshold {
            out.push(label)
        }
    }
    Ok(out)
}
pub enum Action {
    Build,
    Check,
    Select,
    ValidateTracked,
}
pub struct Options<'a> {
    pub generation: Option<&'a str>,
    pub event_id: Option<&'a str>,
    pub input_root: Option<&'a Path>,
    pub output_root: Option<&'a Path>,
    pub confirm_review: bool,
    pub action: Action,
}
struct Paths {
    historical: bool,
    generation: String,
    event: String,
    plan: String,
    anchors: String,
    private: String,
}
impl Paths {
    fn new(opts: &Options<'_>) -> Result<Self> {
        let g = opts.generation.unwrap_or("v1");
        ensure(
            !g.is_empty()
                && g.len() <= 64
                && g.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "generation name",
        )?;
        let historical = g == "v1";
        let event = opts.event_id.unwrap_or(EVENT_ID);
        ensure(
            (event == EVENT_ID) == historical
                && event.starts_with("tos.event.")
                && event.len() <= 200
                && event
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)),
            "fresh generation requires new event",
        )?;
        Ok(Self {
            historical,
            generation: g.into(),
            event: event.into(),
            plan: if historical {
                PLAN_PATH.into()
            } else {
                format!("{GOLD_ROOT}/transfer-samples.{g}.json")
            },
            anchors: if historical {
                ANCHOR_PATH.into()
            } else {
                format!("{GOLD_ROOT}/transfer-target-anchors.{g}.jsonl")
            },
            private: if historical {
                LOCAL_CONTENT_ROOT.into()
            } else {
                format!("{GOLD_ROOT}/local-content/transfer-targets/{g}")
            },
        })
    }
    fn id(&self, base: &str) -> String {
        if self.historical {
            base.into()
        } else {
            format!("{base}-{}", self.generation)
        }
    }
}
fn rights(v: &Value) -> Result<()> {
    ensure(
        v["visibility"] == "local_only"
            && v["derivative_posture"] == "local_research_only"
            && v["redistribution_posture"] == "not_authorized",
        "current rights do not admit local candidate preparation",
    )
}
fn anchors(paths: &Paths, candidates: &[Value]) -> Vec<Value> {
    candidates.iter().map(|c|json!({"schema_version":"tos_source_anchor_v1","anchor_id":c["anchor_ref"],"item_id":c["item_ref"],"file_id":c["file_ref"],"file_sha256":EXPECTED_FILE_SHA256,"passage_id":null,"selectors":[{"type":"page_region","page":c["page"],"x":0,"y":0,"width":1,"height":1,"coordinate_space":"normalized_0_1"}],"selector_method":{"maker_type":"mixed","method":"deterministic pre-output page selection plus model-visible content-bearing confirmation","version":"1","configuration_ref":paths.plan},"status":"proposed","provenance_event_ref":paths.event,"anchor_version":1,"supersedes_anchor_ref":null,"review_ref":null})).collect()
}
fn validate(ctx: &ResearchExecution, paths: &Paths, p: &Value, anchors: &[Value]) -> Result<()> {
    schema(ctx, SCHEMA_PATH, p)?;
    ensure(
        a(&p["target_units"])?.is_empty()
            && a(&p["candidate_target_units"])?.len() == 20
            && anchors.len() == 20,
        "candidate count or eligible targets drift",
    )?;
    let mut seen = BTreeSet::new();
    for c in a(&p["candidate_target_units"])? {
        ensure(
            c["candidate_scope"] == "whole-page"
                && c["target_gold_status"] == "not_started"
                && c["frozen_before_variant_outputs"] == true
                && c["eligible_for_variant_execution"] == false,
            "candidate authority drift",
        )?;
        ensure(seen.insert(s(&c["unit_id"])?), "duplicate candidate")?;
        ensure(
            s(&c["source_content_ref"])?.starts_with(&format!("{}/", paths.private)),
            "private output escaped generation",
        )?;
        private_boundary(ctx, s(&c["source_content_ref"])?)?;
    }
    ensure(
        anchors == self::anchors(paths, a(&p["candidate_target_units"])?),
        "candidate anchor drift",
    )?;
    for row in anchors {
        schema(ctx, "ToS/contracts/source-anchor.schema.json", row)?
    }
    Ok(())
}
fn private_check(ctx: &ResearchExecution, candidates: &[Value], historical: bool) -> Result<()> {
    for c in candidates {
        let r = s(&c["source_content_ref"])?;
        let raw = ctx.read(r)?;
        ensure(
            sha(&raw) == s(&c["source_content_sha256"])?
                && raw.len() == n(&c["source_content_bytes"])?,
            "private candidate bytes drift",
        )?;
        let mode = ctx
            .source_file(r, CAP as u64)?
            .metadata()
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o777;
        // The retained Python generation used 0644; every new write uses 0600.
        ensure(
            mode == 0o600 || (historical && mode == 0o644),
            "private candidate mode drift",
        )?;
    }
    Ok(())
}
fn output_entities(paths: &Paths, p: &Value, raw: &[u8], ars: &[u8]) -> Result<Vec<Value>> {
    let mut rows = vec![
        json!({"ref":paths.plan,"role":"blocked-transfer-plan-with-private-ineligible-candidate-soil","sha256":sha(raw)}),
        json!({"ref":paths.anchors,"role":"proposed-whole-page-transfer-candidate-anchors","sha256":sha(ars)}),
    ];
    for c in a(&p["candidate_target_units"])? {
        rows.push(json!({"ref":c["source_content_ref"],"role":"gitignored-local-only-pdftotext-page-candidate","sha256":c["source_content_sha256"]}))
    }
    Ok(rows)
}
fn historical_closure(
    ctx: &ResearchExecution,
    journal: &[u8],
    plan: &Value,
    raw: &[u8],
    ars: &[u8],
) -> Result<Value> {
    let mut freeze = None;
    let mut latest = None;
    let private = a(&plan["candidate_target_units"])?;
    for (bytes, e) in json_lines(ctx, journal)? {
        if e["event_id"] == EVENT_ID {
            ensure(
                freeze.is_none()
                    && sha(&bytes) == LEGACY_EVENT_SHA
                    && e["method"]["artifact_digest"] == LEGACY_BUILDER_SHA,
                "retained candidate freeze drift",
            )?;
            freeze = Some(e.clone())
        }
        if e["event_id"] == plan["provenance_event_ref"] {
            ensure(latest.is_none(), "duplicate active plan event")?;
            latest = Some(e)
        }
    }
    let freeze = freeze.ok_or("historical freeze absent")?;
    let active = latest.ok_or("active plan provenance absent")?;
    schema(ctx, "ToS/contracts/provenance-event.schema.json", &active)?;
    ensure(
        a(&active["outputs"])?
            .iter()
            .any(|r| r["ref"] == PLAN_PATH && r["sha256"] == sha(raw)),
        "active plan output digest drift",
    )?;
    ensure(
        a(&freeze["outputs"])?
            .iter()
            .any(|r| r["ref"] == ANCHOR_PATH && r["sha256"] == sha(ars)),
        "historical anchor digest drift",
    )?;
    for c in private {
        ensure(
            a(&freeze["outputs"])?.iter().any(|r| {
                r["ref"] == c["source_content_ref"] && r["sha256"] == c["source_content_sha256"]
            }),
            "historical candidate output closure",
        )?;
    }
    Ok(freeze)
}
fn verify_native_event(
    ctx: &ResearchExecution,
    event: &Value,
    paths: &Paths,
    base: &Value,
    pinned: &BTreeMap<&str, String>,
    entities: &[Value],
    plan: &Value,
) -> Result<()> {
    schema(ctx, "ToS/contracts/provenance-event.schema.json", event)?;
    let refs = [
        PLAN_PATH,
        BOUNDARY_MAP_PATH,
        RIGHTS_PATH,
        SCHEMA_PATH,
        GOLD_ASSURANCE_PATH,
        "ToS/contracts/source-anchor.schema.json",
        "ToS/contracts/provenance-event.schema.json",
    ];
    let inputs=refs.iter().map(|r|json!({"ref":r,"role":"source-candidate-preparation-input","sha256":pinned[r]})).chain(std::iter::once(json!({"ref":base["target_source"]["file_ref"],"role":"fixity-verified-private-target-source-pdf","sha256":EXPECTED_FILE_SHA256}))).collect::<Vec<_>>();
    ensure(
        event["event_id"] == paths.event
            && event["event_type"] == "segmentation"
            && event["inputs"] == json!(inputs)
            && event["outputs"] == json!(entities)
            && event["rights_basis_ref"] == RIGHTS_PATH
            && event["method"]["runtime"] == "Rust plus Poppler 26.01.0"
            && event["method"]["artifact_digest"]
                == plan["candidate_preparation"]["builder"]["sha256"]
            && event["status"] == "completed_with_warnings"
            && event["receipt_refs"] == json!([paths.plan]),
        "native candidate event closure drift",
    )
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let paths = Paths::new(&opts)?;
    let build = matches!(opts.action, Action::Build);
    let source_refs = [
        PLAN_PATH,
        BOUNDARY_MAP_PATH,
        RIGHTS_PATH,
        SCHEMA_PATH,
        GOLD_ASSURANCE_PATH,
        "ToS/contracts/source-anchor.schema.json",
        "ToS/contracts/provenance-event.schema.json",
    ];
    let mut pinned = BTreeMap::new();
    let mut values = BTreeMap::new();
    for r in source_refs {
        let raw = ctx.read(r)?;
        values.insert(r, load(ctx, r)?);
        pinned.insert(r, sha(&raw));
    }
    rights(&values[RIGHTS_PATH])?;
    let base = &values[PLAN_PATH];
    ensure(
        a(&base["target_units"])?.is_empty(),
        "cannot prepare over eligible target units",
    )?;
    ensure(
        base["target_source"]["file_sha256"] == EXPECTED_FILE_SHA256
            && values[BOUNDARY_MAP_PATH]["file_sha256"] == EXPECTED_FILE_SHA256,
        "PDF identity binding",
    )?;
    let journal = read_optional(ctx, PROVENANCE_PATH)?.unwrap_or_default();
    let mut found = None;
    for (_, e) in json_lines(ctx, &journal)? {
        if e["event_id"] == paths.event {
            ensure(found.is_none(), "duplicate preparation event")?;
            found = Some(e)
        }
    }
    if matches!(opts.action, Action::ValidateTracked) {
        let p = load(ctx, &paths.plan)?;
        let ar = json_lines(ctx, &ctx.read(&paths.anchors)?)?
            .into_iter()
            .map(|(_, v)| v)
            .collect::<Vec<_>>();
        validate(ctx, &paths, &p, &ar)?;
        if paths.historical {
            historical_closure(
                ctx,
                &journal,
                &p,
                &ctx.read(&paths.plan)?,
                &ctx.read(&paths.anchors)?,
            )?;
        } else {
            let e = found.ok_or("preparation event absent")?;
            ensure(
                e["outputs"]
                    == json!(output_entities(
                        &paths,
                        &p,
                        &ctx.read(&paths.plan)?,
                        &ctx.read(&paths.anchors)?
                    )?),
                "preparation provenance closure",
            )?;
            verify_native_event(
                ctx,
                &e,
                &paths,
                base,
                &pinned,
                &output_entities(
                    &paths,
                    &p,
                    &ctx.read(&paths.plan)?,
                    &ctx.read(&paths.anchors)?,
                )?,
                &p,
            )?;
        }
        if let Some(root) = opts.output_root {
            private_check(
                &ctx.select_directory(root)?,
                a(&p["candidate_target_units"])?,
                paths.historical,
            )?;
        }
        return Ok(
            json!({"status":"passed","mode":"validate-tracked","candidate_count":20,"private_payloads_read":false,"private_content_checked":opts.output_root.is_some()}),
        );
    }
    let source = ctx.select_directory(
        opts.input_root
            .ok_or("explicit private input root required")?,
    )?;
    let mut pdf = source.source_file(SOURCE_PDF_PATH, 256 * 1024 * 1024)?;
    ensure(
        source.hash_file(&mut pdf, 256 * 1024 * 1024)? == EXPECTED_FILE_SHA256,
        "source PDF digest drift",
    )?;
    let mut poppler = Poppler::open(ctx)?;
    let seed = sha(format!(
        "{}|{}|candidate-protocol-v1",
        EXPECTED_FILE_SHA256,
        s(&base["transfer_plan_id"])?
    )
    .as_bytes());
    let templates: Value = serde_json::from_str(include_str!("transfer_candidates/templates.json"))
        .map_err(|e| e.to_string())?;
    let mut candidates = vec![];
    let mut private = BTreeMap::new();
    for (work, slug, quota) in WORKS {
        let members = a(&values[BOUNDARY_MAP_PATH]["members"])?;
        let selected = members
            .iter()
            .filter(|m| m["work_ref"] == work)
            .collect::<Vec<_>>();
        ensure(selected.len() == 1, "work boundary absent or ambiguous")?;
        let member = selected[0];
        let first = n(&member["start_page"])?
            .checked_add(3)
            .ok_or("page overflow")?;
        let last = n(&member["end_page"])?
            .checked_sub(3)
            .ok_or("page underflow")?;
        ensure(
            first <= last && last - first <= 1000,
            "work page range bound",
        )?;
        let mut eligible = vec![];
        let mut retained_bytes = 0usize;
        for start in (first..=last).step_by(8) {
            let end = (start + 7).min(last);
            for (page, raw) in poppler.layout_pages(ctx, &pdf, start, end)? {
                let m = metrics(&raw)?;
                if n(&m["alphabetic_characters"])? < 800 || n(&m["nonblank_lines"])? < 15 {
                    continue;
                }
                let rank = sha(format!("{seed}|{work}|{page}|{}", sha(&raw)).as_bytes());
                retained_bytes = retained_bytes
                    .checked_add(raw.len())
                    .ok_or("page memory overflow")?;
                ensure(
                    retained_bytes <= 16 * 1024 * 1024,
                    "retained page memory bound",
                )?;
                eligible.push((page, raw, m, rank));
            }
        }
        ensure(
            eligible.len() >= 2 * quota,
            "insufficient content-bearing pages",
        )?;
        eligible.sort_by(|a, b| a.3.cmp(&b.3).then(a.0.cmp(&b.0)));
        let random = eligible.drain(..quota).collect::<Vec<_>>();
        eligible.sort_by(|a, b| {
            b.2["mechanical_hardness_score"]
                .as_u64()
                .unwrap()
                .cmp(&a.2["mechanical_hardness_score"].as_u64().unwrap())
                .then(a.0.cmp(&b.0))
        });
        let hard = eligible.drain(..quota).collect::<Vec<_>>();
        for (stratum, rows) in [("random", random), ("hard", hard)] {
            for (rank, (page, raw, m, _)) in rows.into_iter().enumerate() {
                let id = paths.id(&format!("tos-target-candidate-{slug}-p{page:04}-{stratum}"));
                let local = format!("{}/{id}.txt", paths.private);
                let anchor = paths.id(&format!(
                    "tos.anchor.zarathustra-foundation-pilot-v1.transfer-{slug}-p{page:04}"
                ));
                let row = json!({"unit_id":id,"anchor_ref":anchor,"work_ref":work,"expression_ref":member["expression_ref"],"item_ref":values[BOUNDARY_MAP_PATH]["item_ref"],"file_ref":values[BOUNDARY_MAP_PATH]["file_id"],"page":page,"page_resource_id":format!("pdf-page-{page:04}"),"candidate_scope":"whole-page","stratum":stratum,"selection_basis":if stratum=="random"{"deterministic-digest-order"}else{"mechanical-layout-hardness"},"selection_rank":rank+1,"selection_metrics":m,"provisional_difficulty_signals":signals(&m,stratum)?,"source_content_ref":local,"source_content_sha256":sha(&raw),"source_content_bytes":raw.len(),"source_layer":"embedded-pdf-text-pdftotext-layout","source_review_status":"model_source_visible","source_review_scope":"content-bearing-page-and-route-only","target_gold_status":"not_started","frozen_before_variant_outputs":true,"eligible_for_variant_execution":false,"limitations":templates[format!("{stratum}_limitations")]});
                private.insert(local, raw);
                candidates.push(row)
            }
        }
    }
    candidates.sort_by_key(|c| {
        (
            if c["stratum"] == "random" { 0 } else { 1 },
            WORKS
                .iter()
                .position(|(w, _, _)| c["work_ref"] == *w)
                .unwrap(),
            c["selection_rank"].as_u64().unwrap(),
        )
    });
    ensure(candidates.len() == 20, "selected count")?;
    poppler.verify()?;
    ensure(
        source.hash_file(&mut pdf, 256 * 1024 * 1024)? == EXPECTED_FILE_SHA256,
        "source PDF changed",
    )?;
    if matches!(opts.action, Action::Select) {
        let rows=candidates.iter().map(|c|json!({"unit_id":c["unit_id"],"work_ref":c["work_ref"],"page":c["page"],"stratum":c["stratum"],"selection_rank":c["selection_rank"],"selection_metrics":c["selection_metrics"]})).collect::<Vec<_>>();
        return Ok(
            json!({"status":"passed","selection":rows,"writes":false,"review_performed":false}),
        );
    }
    let destination = ctx.select_directory(
        opts.output_root
            .ok_or("explicit private output root required")?,
    )?;
    let (plan, anchor_rows, event) = if paths.historical {
        let stored = a(&base["candidate_target_units"])?;
        for (expected, actual) in candidates.iter_mut().zip(stored) {
            expected["limitations"] = actual["limitations"].clone();
        }
        ensure(
            candidates == *stored,
            "retained candidate selection or metadata drift",
        )?;
        let ars = anchors(&paths, &candidates);
        let event = historical_closure(
            ctx,
            &journal,
            base,
            &ctx.read(PLAN_PATH)?,
            &jsonl(ctx, &ars)?,
        )?;
        (base.clone(), ars, event)
    } else {
        ensure(
            !build || opts.confirm_review || found.is_some(),
            "fresh candidate write requires explicit model source-visible review",
        )?;
        let at = if let Some(e) = &found {
            s(&e["started_at"])?.into()
        } else {
            utc_now()?
        };
        let mut p = base.clone();
        p["transfer_plan_id"] = json!(paths.id(s(&base["transfer_plan_id"])?));
        p["frozen_at"] = json!(at);
        p["candidate_target_units"] = json!(candidates);
        p["candidate_preparation"] = templates["preparation"].clone();
        let prep = &mut p["candidate_preparation"];
        prep["prepared_at"] = json!(at);
        prep["deterministic_randomization_key_sha256"] = json!(seed);
        prep["builder"] = json!({"ref":BUILDER,"sha256":builder_sha()});
        p["provenance_event_ref"] = json!(paths.event);
        p["kernel_evidence_gate"]["blockers"] = templates["blockers"].clone();
        p["result"]["conclusion"] = templates["conclusion"].clone();
        let ars = anchors(&paths, a(&p["candidate_target_units"])?);
        let event=found.clone().unwrap_or_else(||json!({"schema_version":"tos_provenance_event_v1","event_id":paths.event,"event_type":"segmentation","started_at":at,"ended_at":at,"agent_refs":["software:tos-native","software:poppler-26.01.0"],"inputs":source_refs.iter().map(|r|json!({"ref":r,"role":"source-candidate-preparation-input","sha256":pinned[r]})).chain(std::iter::once(json!({"ref":base["target_source"]["file_ref"],"role":"fixity-verified-private-target-source-pdf","sha256":EXPECTED_FILE_SHA256}))).collect::<Vec<_>>(),"outputs":[],"method":{"maker_type":"mixed","name":"pre-output-private-transfer-candidate-freeze","version":"2","artifact_digest":builder_sha(),"runtime":"Rust plus Poppler 26.01.0","device":"abyss-machine","configuration":{"network_access":false,"variant_outputs_visible":false,"candidate_count":20,"random_candidate_count":10,"hard_candidate_count":10,"model_source_visible_content_check":true,"human_review_performed":false,"eligible_target_units_created":0,"target_gold_created":0,"semantic_labels_created":0,"source_text_tracked":false},"prompt_or_instruction_ref":"ToS/research-packets/foundation-laboratory-2026-07/GOLDEN_KERNEL_TRANSFER_REPORT.md"},"status":"completed_with_warnings","warnings":["Private whole-page automatic candidates remain proposed and ineligible; source-visible review is explicitly supplied by the caller; no gold, semantics, publication or canon is created."],"receipt_refs":[paths.plan],"rights_basis_ref":RIGHTS_PATH,"event_version":1,"supersedes_event_ref":null}));
        (p, ars, event)
    };
    validate(ctx, &paths, &plan, &anchor_rows)?;
    let raw = encode(ctx, &plan, true)?;
    let ar = jsonl(ctx, &anchor_rows)?;
    let entities = output_entities(&paths, &plan, &raw, &ar)?;
    let mut event = event;
    if !paths.historical {
        if found.is_none() {
            ensure(build, "preparation event absent")?;
            event["outputs"] = json!(entities)
        }
        ensure(
            event["outputs"] == json!(entities),
            "preparation event output drift",
        )?;
        verify_native_event(ctx, &event, &paths, base, &pinned, &entities, &plan)?;
    }
    let tracked = BTreeMap::from([(paths.plan.clone(), raw), (paths.anchors.clone(), ar)]);
    let mut missing_tracked = vec![];
    let mut missing_private = vec![];
    for (r, b) in &tracked {
        let missing = fresh_or_matching_limit(ctx, r, b, CAP)?;
        ensure(build || !missing, "tracked candidate output absent")?;
        if missing {
            missing_tracked.push(r)
        }
    }
    for (r, b) in &private {
        private_boundary(ctx, r)?;
        let missing = fresh_or_matching_limit(&destination, r, b, CAP)?;
        ensure(build || !missing, "private candidate output absent")?;
        if missing {
            missing_private.push(r)
        }
    }
    for (r, h) in pinned {
        ensure(sha(&ctx.read(r)?) == h, "candidate source changed")?;
    }
    ensure(
        read_optional(ctx, PROVENANCE_PATH)?.unwrap_or_default() == journal,
        "candidate journal changed",
    )?;
    if build {
        for r in missing_private {
            destination.write(r, &private[r], 0o600, true)?;
        }
        for r in missing_tracked {
            ctx.write(r, &tracked[r], 0o644, true)?;
        }
        if found.is_none() {
            let mut next = journal.clone();
            if !next.is_empty() && !next.ends_with(b"\n") {
                next.push(b'\n')
            }
            next.extend(jsonl(ctx, &[event])?);
            ensure(next.len() <= CAP, "journal bound")?;
            if journal.is_empty() {
                ctx.write(PROVENANCE_PATH, &next, 0o644, true)?;
            } else {
                ctx.write_replacing_exact(PROVENANCE_PATH, &next, 0o644, &journal)?;
            }
        }
    }
    private_check(
        &destination,
        a(&plan["candidate_target_units"])?,
        paths.historical,
    )?;
    Ok(
        json!({"status":"passed","mode":if build{"build"}else{"check"},"candidate_count":20,"random":10,"hard":10,"eligible_target_units":0,"target_gold":0,"publication_authorized":false}),
    )
}
fn builder_sha() -> String {
    let mut h = tos_foundation::Digest256Hasher::new();
    h.update(include_bytes!("transfer_candidates.rs"));
    h.update(include_bytes!("transfer_candidates/constants.rs"));
    h.update(include_bytes!("transfer_candidates/templates.json"));
    h.finalize().to_hex()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_layout_metrics() {
        assert_eq!(
            metrics("17\r\nABC—\nsmall,\nI.\nlast:\n".as_bytes()).unwrap(),
            json!({"nonspace_characters":19,"alphabetic_characters":13,"nonblank_lines":5,"punctuation_characters":4,"line_end_hyphenations":1,"page_edge_fragment_signals":2,"numbered_heading_candidates":2,"mechanical_hardness_score":903})
        );
    }
    #[test]
    fn fresh_identity_is_explicit() {
        assert!(
            Paths::new(&Options {
                generation: Some("new"),
                event_id: None,
                input_root: None,
                output_root: None,
                confirm_review: false,
                action: Action::Select
            })
            .is_err()
        );
    }
}
