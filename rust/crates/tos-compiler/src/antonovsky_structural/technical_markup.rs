//! Exact witness-local v1 layout producer. This predecessor is distinct from
//! v2 paragraphs/verse. All source words remain private and unnormalized.
use super::*;
use crate::research_execution::ResearchExecution;
mod packet;
mod validation;
mod views;
const SCHEMA: &str = "ToS/contracts/source-text-unit-packet-v1.schema.json";
const BUILDER: &str = "scripts/build_antonovsky_1911_technical_markup.py";
const BUILDER_SHA: &str = "4c80683124592bc969e0db6ffe0d5696ec28d4079bc522725788aeff44abd8eb";
const EVENT: &str = "tos.event.segmentation.zarathustra-antonovsky-1911-pdf-layout-v1.2026-09-01";
const SCREEN_EVENT: &str =
    "tos.event.screening.zarathustra-antonovsky-1911-pdf-layout-v1.2026-09-01";
const OUTPUT_KEYS: &[&str] = &[
    "packet_ref",
    "citation_spine_ref",
    "region_candidates_ref",
    "heading_candidates_ref",
    "summary_ref",
    "heading_screening_ref",
    "paragraph_segment_overlay_ref",
    "screening_summary_ref",
    "provenance_ref",
];
fn plan_ref() -> String {
    format!("{V1}/plan.v1.json")
}
fn screening_ref() -> String {
    format!("{V1}/technical-screening-plan.v1.json")
}
fn load(ctx: &ResearchExecution, r: &str) -> Result<Value> {
    serde_json::from_slice(&ctx.read(r)?).map_err(|e| e.to_string())
}
fn load_lines(ctx: &ResearchExecution, r: &str) -> Result<Vec<Value>> {
    let raw = ctx.read(r)?;
    let mut out = Vec::new();
    for line in std::str::from_utf8(&raw)
        .map_err(|e| e.to_string())?
        .lines()
    {
        ctx.tick(1)?;
        out.push(serde_json::from_str(line).map_err(|e| e.to_string())?);
    }
    Ok(out)
}
fn pretty(ctx: &ResearchExecution, v: &Value) -> Result<Vec<u8>> {
    ctx.check()?;
    let raw = encode_until(ctx.deadline(), v, true)?;
    ctx.check()?;
    Ok(raw)
}
fn compact(ctx: &ResearchExecution, v: &Value) -> Result<Vec<u8>> {
    encode_until(ctx.deadline(), v, false)
}
fn jsonl(ctx: &ResearchExecution, v: &[Value]) -> Result<Vec<u8>> {
    encodel_until(ctx.deadline(), v)
}
fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { fail(message) }
}
fn binding(ctx: &ResearchExecution, r: &str) -> Result<Value> {
    Ok(json!({"ref":r,"sha256":sha(&ctx.read(r)?)}))
}
fn out<'a>(plan: &'a Value, key: &str) -> Result<&'a str> {
    plan["outputs"][key]
        .as_str()
        .ok_or_else(|| format!("output reference missing: {key}"))
}
fn plans(ctx: &ResearchExecution) -> Result<(Value, Value)> {
    let plan = load(ctx, &plan_ref())?;
    ensure(
        s(&plan["contract_ref"]) == SCHEMA && s(&plan["route_root"]) == V1,
        "plan contract/route drift",
    )?;
    ensure(
        s(&plan["technical_screening_plan_ref"]) == screening_ref(),
        "screening reference drift",
    )?;
    let screening = load(ctx, &screening_ref())?;
    ensure(
        screening["source_plan_ref"] == plan_ref()
            && screening["source_packet_ref"] == plan["outputs"]["packet_ref"]
            && screening["source_heading_candidates_ref"]
                == plan["outputs"]["heading_candidates_ref"]
            && screening["source_file_sha256"] == plan["source_item"]["file_sha256"]
            && screening["source_bbox_sha256"] == plan["extraction_policy"]["expected_bbox_sha256"]
            && screening["heading_screening"]["expected_candidate_count"]
                == plan["expected_counts"]["heading_candidates"],
        "screening source/count binding drift",
    )?;
    let mut ordinals = Vec::new();
    for group in a(&screening["heading_screening"]["classification_groups"]) {
        ctx.tick(1)?;
        ensure(
            !s(&group["technical_role"]).is_empty(),
            "screening role invalid",
        )?;
        let values = group["candidate_ordinals"]
            .as_array()
            .ok_or("heading ordinal list")?;
        for ordinal in values {
            ordinals.push(ordinal.as_u64().ok_or("heading ordinal integer")? as usize);
        }
    }
    ordinals.sort_unstable();
    ensure(
        ordinals == (1..=n(&plan["expected_counts"]["heading_candidates"])).collect::<Vec<_>>(),
        "screening classifications must be exact/exhaustive",
    )?;
    let leaves = [
        "source-text-unit.v1.json",
        "citation-spine.v1.jsonl",
        "region-candidates.v1.jsonl",
        "heading-candidates.v1.jsonl",
        "summary.v1.json",
        "heading-screening.v1.jsonl",
        "paragraph-segment-overlay.v1.jsonl",
        "screening-summary.v1.json",
        "provenance.jsonl",
    ];
    for (key, leaf) in OUTPUT_KEYS.iter().zip(leaves) {
        ensure(
            out(&plan, key)? == format!("{V1}/{leaf}"),
            "tracked output binding drift",
        )?;
    }
    ensure(
        s(&plan["identity_policy"]["issuance_ref"]) == format!("{V1}/identity-issuance.v1.json"),
        "issuance output binding drift",
    )?;
    let private_root = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/technical-markup-v1";
    ensure(
        s(&plan["private_layer_root"]) == private_root,
        "private output parent drift",
    )?;
    for (key, leaf) in [
        (
            "private_text_layer_ref",
            "antonovsky-1911-pdf-layout.v1.txt",
        ),
        (
            "private_bbox_ref",
            "antonovsky-1911-pdf.bbox-layout.v1.html",
        ),
    ] {
        ensure(
            out(&plan, key)? == format!("{private_root}/{leaf}"),
            "private output binding drift",
        )?;
    }
    Ok((plan, screening))
}
#[derive(Debug)]
struct Span {
    locator: String,
    start: usize,
    end: usize,
    blocks: Vec<usize>,
    page: usize,
    panel: Option<String>,
}
struct Document {
    blocks: Vec<Block>,
    pages: Vec<Span>,
    panels: Vec<Span>,
    starts: Vec<usize>,
    ends: Vec<usize>,
    region: Vec<String>,
    headings: Vec<bool>,
    gaps: Vec<(f64, f64)>,
    flow_counts: Vec<usize>,
    dimensions: Vec<[f64; 2]>,
    inventory: Vec<Value>,
    text: String,
    text_sha: String,
    offsets: Vec<usize>,
    bbox: Vec<u8>,
}
impl Document {
    fn slice(&self, start: usize, end: usize) -> Result<&str> {
        let a = *self.offsets.get(start).ok_or("selector start")?;
        let b = *self.offsets.get(end).ok_or("selector end")?;
        self.text
            .get(a..b)
            .ok_or_else(|| "selector interval".into())
    }
    fn locators(&self) -> Vec<String> {
        let mut v = vec!["pdf-document".into()];
        for page in &self.pages {
            v.push(page.locator.clone());
            for panel in self.panels.iter().filter(|p| p.page == page.page) {
                v.push(panel.locator.clone());
                v.extend(panel.blocks.iter().map(|i| self.blocks[*i].locator.clone()));
            }
        }
        v
    }
    fn counts(&self, selected: &[usize]) -> (usize, usize, usize, usize) {
        (
            selected.len(),
            selected.iter().map(|i| self.blocks[*i].lines.len()).sum(),
            selected.iter().map(|i| self.blocks[*i].words).sum(),
            selected.iter().filter(|i| self.headings[**i]).count(),
        )
    }
}
fn region(plan: &Value, page: usize, panel: &str) -> Result<String> {
    let key = (page, usize::from(panel != "left"));
    let rows: Vec<_> = a(&plan["region_candidates"])
        .iter()
        .filter(|r| {
            let part = |v: &Value| (n(&v["pdf_page"]), usize::from(s(&v["panel"]) != "left"));
            part(&r["start"]) <= key && key <= part(&r["end"])
        })
        .collect();
    ensure(rows.len() == 1, "panel region closure")?;
    Ok(s(&rows[0]["region_id"]).into())
}
fn prepare(ctx: &ResearchExecution, plan: &Value) -> Result<Document> {
    let src = &plan["source_item"];
    let manifest = load(ctx, s(&src["manifest_ref"]))?;
    ensure(
        manifest["item_id"] == src["item_ref"] && manifest["embodiment_ref"] == src["edition_ref"],
        "source item/edition binding drift",
    )?;
    let entries: Vec<_> = a(&manifest["payload_files"])
        .iter()
        .filter(|r| r["file_id"] == src["file_ref"])
        .collect();
    ensure(
        entries.len() == 1 && entries[0]["sha256"] == src["file_sha256"],
        "PDF manifest drift",
    )?;
    let pdf_ref = Path::new(s(&src["manifest_ref"]))
        .parent()
        .ok_or("manifest parent")?
        .join(s(&entries[0]["relative_path"]));
    let pdf_ref = pdf_ref.to_str().ok_or("PDF reference")?;
    let mut file = ctx.source_file(pdf_ref, 64 * 1024 * 1024)?;
    let raw = ctx.read_file(&mut file, 64 * 1024 * 1024)?;
    let digest = sha(&raw);
    ensure(
        digest == s(&src["file_sha256"]) && digest == PDF_SHA,
        "PDF digest drift",
    )?;
    drop(raw);
    let inventory = load(ctx, s(&src["resource_inventory_ref"]))?;
    ensure(
        inventory["item_id"] == src["item_ref"],
        "inventory item drift",
    )?;
    let files: Vec<_> = a(&inventory["files"])
        .iter()
        .filter(|r| r["file_id"] == src["file_ref"])
        .collect();
    ensure(
        files.len() == 1 && files[0]["file_sha256"] == src["file_sha256"],
        "inventory PDF drift",
    )?;
    let inventory = a(&files[0]["resources"]).to_vec();
    ensure(
        inventory.len() == n(&plan["expected_counts"]["pdf_pages"])
            && inventory
                .iter()
                .enumerate()
                .all(|(i, r)| s(&r["resource_id"]) == format!("pdf-page-{:04}", i + 1)),
        "inventory page identity drift",
    )?;
    let bbox = poppler_bbox(ctx.deadline(), &file, plan)?;
    let observed = observation(&bbox, &inventory, None, plan, ctx.deadline())?;
    let blocks = observed.blocks;
    let mut doc = Document {
        starts: vec![0; blocks.len()],
        ends: vec![0; blocks.len()],
        region: vec![String::new(); blocks.len()],
        headings: vec![false; blocks.len()],
        gaps: vec![(0., 0.); blocks.len()],
        blocks,
        pages: vec![],
        panels: vec![],
        flow_counts: observed.flow_counts,
        dimensions: observed.dimensions,
        inventory,
        text: String::new(),
        text_sha: String::new(),
        offsets: vec![],
        bbox,
    };
    let mut position = 0;
    for page in 1..=doc.inventory.len() {
        ctx.tick(1)?;
        let page_start = position;
        let mut page_blocks = Vec::new();
        let panels: Vec<Vec<usize>> = a(&plan["extraction_policy"]["panel_order"])
            .iter()
            .map(|panel| {
                doc.blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.page == page && b.panel == s(panel))
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty())
            .collect();
        for (panel_index, indices) in panels.iter().enumerate() {
            ctx.tick(1)?;
            let panel_start = position;
            let first = &doc.blocks[indices[0]];
            let panel_name = first.panel.clone();
            let reg = region(plan, page, &panel_name)?;
            for (bi, index) in indices.iter().enumerate() {
                ctx.tick(1)?;
                let b = &doc.blocks[*index];
                let text = b
                    .lines
                    .iter()
                    .map(|(text, _)| text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                doc.starts[*index] = position;
                position += text.chars().count();
                doc.text.push_str(&text);
                doc.ends[*index] = position;
                doc.region[*index] = reg.clone();
                let gap_before = b.bbox[1]
                    - bi.checked_sub(1)
                        .map(|i| doc.blocks[indices[i]].bbox[3])
                        .unwrap_or(0.);
                let gap_after = indices
                    .get(bi + 1)
                    .map(|i| doc.blocks[*i].bbox[1])
                    .unwrap_or(b.height)
                    - b.bbox[3];
                doc.gaps[*index] = (gap_before, gap_after);
                let rule = &plan["heading_candidate_rule"];
                let f = |k: &str| rule[k].as_f64().unwrap_or(f64::NAN);
                doc.headings[*index] = b.words > 0
                    && b.lines.len() <= n(&rule["maximum_line_count"])
                    && b.words <= n(&rule["maximum_word_count"])
                    && b.bbox[2] - b.bbox[0] <= f("maximum_width_points")
                    && ((b.bbox[0] + b.bbox[2]) / 2.
                        - b.width * if b.panel == "left" { 0.25 } else { 0.75 })
                    .abs()
                        <= f("panel_center_tolerance_points")
                    && b.bbox[1] >= f("minimum_y_points")
                    && b.bbox[3] <= b.height - f("bottom_margin_points")
                    && gap_before >= f("minimum_gap_before_points")
                    && gap_after >= f("minimum_gap_after_points")
                    && median(&b.word_heights) >= f("minimum_median_word_height_points");
                if bi + 1 < indices.len() {
                    doc.text.push_str("\n\n");
                    position += 2;
                }
            }
            doc.panels.push(Span {
                locator: format!("pdf-page-{page:04}/panel-{panel_name}"),
                start: panel_start,
                end: position,
                blocks: indices.clone(),
                page,
                panel: Some(panel_name),
            });
            page_blocks.extend(indices);
            if panel_index + 1 < panels.len() {
                doc.text.push_str("\n\n\n");
                position += 3;
            }
        }
        if page < doc.inventory.len() {
            doc.text.push('\x0c');
            position += 1;
        }
        doc.pages.push(Span {
            locator: format!("pdf-page-{page:04}"),
            start: page_start,
            end: position,
            blocks: page_blocks,
            page,
            panel: None,
        });
    }
    doc.text_sha = sha(doc.text.as_bytes());
    doc.offsets = doc
        .text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(doc.text.len()))
        .collect();
    ensure(
        doc.offsets.len() == position + 1 && doc.text_sha == TEXT_SHA,
        "text layer position/fixity drift",
    )?;
    ctx.check()?;
    Ok(doc)
}
const FIXED: &[(&str, &str)] = &[
    ("packet_id", "source-text-unit-packet"),
    ("scheme_id", "text-unit-scheme"),
    ("segmentation_id", "text-segmentation"),
    ("projection_id", "text-unit-projection"),
];
fn issuance(ctx: &ResearchExecution, plan: &Value, doc: Option<&Document>) -> Result<Value> {
    let v = load(ctx, s(&plan["identity_policy"]["issuance_ref"]))?;
    ensure(
        v["identity_policy"] == plan["identity_policy"]["policy"]
            && v["source_file_sha256"] == plan["source_item"]["file_sha256"],
        "issuance policy/fixity drift",
    )?;
    if let Some(doc) = doc {
        ensure(v["bbox_sha256"] == sha(&doc.bbox), "issuance bbox drift")?;
        ensure(
            a(&v["units"])
                .iter()
                .map(|r| s(&r["source_locator"]).to_string())
                .collect::<Vec<_>>()
                == doc.locators(),
            "issuance locator/order drift",
        )?;
    }
    for (name, prefix) in FIXED {
        ensure(
            s(&v["fixed_ids"][name]).starts_with(&format!("tos.{prefix}.sid-")),
            "fixed opaque identity invalid",
        )?;
    }
    for key in ["source_locator", "unit_id", "anchor_ref"] {
        let rows = a(&v["units"]);
        ensure(
            !rows.is_empty()
                && rows
                    .iter()
                    .map(|r| s(&r[key]))
                    .collect::<BTreeSet<_>>()
                    .len()
                    == rows.len(),
            "issuance collision/emptiness",
        )?;
    }
    Ok(v)
}
fn issue(ctx: &ResearchExecution, plan: &Value, doc: &Document) -> Result<()> {
    let mut fixed = serde_json::Map::new();
    for (name, prefix) in FIXED {
        fixed.insert(name.to_string(), json!(mint(&format!("tos.{prefix}"))?));
    }
    let mut units = Vec::new();
    for loc in doc.locators() {
        ctx.tick(1)?;
        units.push(json!({"source_locator":loc,"unit_id":mint("tos.text-unit")?,"anchor_ref":mint("tos.anchor.zarathustra-antonovsky-1911-layout-v1")?}));
    }
    let v = json!({"schema_version":"tos_opaque_identity_issuance_v1","issuance_id":"tos.identity-issuance.zarathustra-antonovsky-1911-pdf-layout-v1","issued_at":plan["created_at"],"identity_policy":plan["identity_policy"]["policy"],"source_locator_is_binding_not_identity":true,"automatic_remint_on_locator_drift":false,"source_file_ref":plan["source_item"]["file_ref"],"source_file_sha256":plan["source_item"]["file_sha256"],"bbox_sha256":sha(&doc.bbox),"configuration_ref":plan_ref(),"fixed_ids":fixed,"units":units,"source_text_included":false,"semantic_promotion":false});
    ctx.write(
        s(&plan["identity_policy"]["issuance_ref"]),
        &pretty(ctx, &v)?,
        0o644,
        true,
    )
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Build { issue_identities: bool },
    Check,
    ValidateTracked,
}
pub fn run(
    root: &Path,
    action: Action,
    max_seconds: u64,
    scratch_bytes: Option<u64>,
) -> Result<Vec<u8>> {
    let ctx = if matches!(action, Action::Build { .. }) {
        ResearchExecution::new_with_scratch(
            root,
            max_seconds,
            scratch_bytes.ok_or("writing requires explicit admitted --scratch-bytes")?,
        )?
    } else {
        ResearchExecution::new(root, max_seconds)?
    };
    let (plan, screening) = plans(&ctx)?;
    if matches!(action, Action::ValidateTracked) {
        let (summary, screen) = validation::tracked(&ctx, &plan, &screening)?;
        ctx.check()?;
        return Ok(format!("antonovsky technical markup tracked validation passed: units={} blocks={} headings={} screened_headings={} split_boundaries={}\n",n(&summary["tracked_unit_count"]),n(&summary["layout_block_paragraph_candidate_count"]),n(&summary["heading_candidate_count"]),n(&screen["heading_candidate_count"]),n(&screen["split_boundary_count"])).into_bytes());
    }
    // The current maintained recipe differs from the historical capture only
    // in five provenance warnings; do not relabel that older source event.
    ensure(
        sha(&ctx.read(BUILDER)?) == BUILDER_SHA,
        "current v1 producer source binding drift; historical recipe is retained separately",
    )?;
    let doc = prepare(&ctx, &plan)?;
    if matches!(
        action,
        Action::Build {
            issue_identities: true
        }
    ) {
        issue(&ctx, &plan, &doc)?;
    }
    let ids = issuance(&ctx, &plan, Some(&doc))?;
    let artifacts = packet::build(&ctx, &doc, &plan, &screening, &ids)?;
    if matches!(action, Action::Build { .. }) {
        for key in OUTPUT_KEYS {
            ctx.tick(1)?;
            let reference = out(&plan, key)?;
            ctx.write(reference, &artifacts.tracked[reference], 0o644, false)?;
        }
        for key in ["private_text_layer_ref", "private_bbox_ref"] {
            let reference = out(&plan, key)?;
            ctx.write(reference, &artifacts.private[reference], 0o600, false)?;
        }
    } else {
        let mut drift = Vec::new();
        for (reference, expected) in artifacts.tracked.iter().chain(artifacts.private.iter()) {
            ctx.tick(1)?;
            let file = ctx.source_file(reference, 64 * 1024 * 1024);
            match file {
                Err(e) => drift.push(format!("missing/unsafe: {reference}: {e}")),
                Ok(mut file) => {
                    let raw = ctx.read_file(&mut file, 64 * 1024 * 1024)?;
                    if raw != *expected {
                        drift.push(format!("drifted: {reference}"));
                    }
                    if artifacts.private.contains_key(reference) {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = file
                            .metadata()
                            .map_err(|e| e.to_string())?
                            .permissions()
                            .mode()
                            & 0o777;
                        if mode != 0o600 {
                            drift.push(format!("private mode drifted: {reference}"));
                        }
                    }
                }
            }
        }
        ensure(
            drift.is_empty(),
            &format!("artifact parity failed: {}", drift.join("; ")),
        )?;
    }
    let summary: Value = serde_json::from_slice(&artifacts.tracked[out(&plan, "summary_ref")?])
        .map_err(|e| e.to_string())?;
    let screen: Value =
        serde_json::from_slice(&artifacts.tracked[out(&plan, "screening_summary_ref")?])
            .map_err(|e| e.to_string())?;
    ctx.check()?;
    Ok(format!("antonovsky technical markup {}: pages={} panels={} blocks={} headings={} units={} screened_headings={} split_boundaries={}\n",if matches!(action,Action::Build {..}) {"built"}else{"parity passed"},n(&summary["pdf_page_count"]),n(&summary["observed_panel_count"]),n(&summary["layout_block_paragraph_candidate_count"]),n(&summary["heading_candidate_count"]),n(&summary["tracked_unit_count"]),n(&screen["heading_candidate_count"]),n(&screen["split_boundary_count"])).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_selector_counts_unicode_scalars_and_preserves_separators() {
        let text = "ѣя\n\n\u{c}😀".to_string();
        let offsets = text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .collect();
        let doc = Document {
            blocks: vec![],
            pages: vec![],
            panels: vec![],
            starts: vec![],
            ends: vec![],
            region: vec![],
            headings: vec![],
            gaps: vec![],
            flow_counts: vec![],
            dimensions: vec![],
            inventory: vec![],
            text_sha: sha(text.as_bytes()),
            text,
            offsets,
            bbox: vec![],
        };
        assert_eq!(doc.slice(0, 2).unwrap(), "ѣя");
        assert_eq!(doc.slice(2, 5).unwrap(), "\n\n\u{c}");
        assert_eq!(doc.slice(5, 6).unwrap(), "😀");
        assert!(doc.slice(0, 7).is_err());
    }
    #[test]
    #[ignore = "requires explicit current maintained full v1 oracle source selection; never skips missing private layers"]
    fn full_v1_eleven_outputs_and_tracked_validator() {
        let root = std::env::var_os("TOS_ANTONOVSKY_V1_ORACLE_SOURCE_ROOT")
            .expect("explicit v1 source fixture required");
        let root = Path::new(&root);
        let stdout = run(root, Action::Check, 600, None)
            .expect("all nine tracked and two private bytes plus private modes must match");
        assert!(
            std::str::from_utf8(&stdout)
                .unwrap()
                .starts_with("antonovsky technical markup parity passed:")
        );
        let stdout =
            run(root, Action::ValidateTracked, 600, None).expect("tracked contract and controls");
        assert!(
            std::str::from_utf8(&stdout)
                .unwrap()
                .starts_with("antonovsky technical markup tracked validation passed:")
        );
    }
}
