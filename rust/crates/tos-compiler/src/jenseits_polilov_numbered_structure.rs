//! Numbered translation structure from the exact embedded PDF navigation layer
//! and retained page-review decisions. No translation text is emitted or accepted.
use crate::{
    constructor_library::Out,
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, s, sha},
    transfer_target_passages::{Poppler, f},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::Path,
};
#[path = "jenseits_polilov_numbered_structure/constants.rs"]
mod constants;
#[path = "jenseits_polilov_numbered_structure/records.rs"]
mod records;
use constants::*;
type Result<T> = std::result::Result<T, String>;
const CAP: u64 = 4 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/jenseits_polilov_numbered_structure.rs";
fn q(v: impl Into<String>) -> Out {
    Out::String(v.into())
}
fn n(v: u64) -> Out {
    Out::Integer(v)
}
fn o(v: Vec<(&str, Out)>) -> Out {
    Out::Object(v.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn field<'a>(v: &'a mut Out, key: &str) -> Result<&'a mut Out> {
    let Out::Object(rows) = v else {
        return Err("ordered object required".into());
    };
    rows.iter_mut()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| format!("missing ordered field {key}"))
}
fn set(v: &mut Out, key: &str, next: Out) -> Result<()> {
    *field(v, key)? = next;
    Ok(())
}
fn append(v: &mut Out, next: Out) -> Result<()> {
    let Out::Array(rows) = v else {
        return Err("ordered array required".into());
    };
    rows.push(next);
    Ok(())
}
fn render(v: &Out, pretty: bool) -> Result<Vec<u8>> {
    let mut b = if pretty {
        serde_json::to_vec_pretty(v)
    } else {
        serde_json::to_vec(v)
    }
    .map_err(|e| e.to_string())?;
    b.push(b'\n');
    ensure(b.len() <= CAP as usize, "structure packet byte bound")?;
    Ok(b)
}
fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("array required".into())
}
fn keys() -> Vec<String> {
    (1..=296)
        .flat_map(|n| {
            let mut v = vec![n.to_string()];
            if [65, 73].contains(&n) {
                v.push(format!("{n}a"));
            }
            v
        })
        .collect()
}
#[derive(Debug)]
struct Candidate {
    page: usize,
    key: String,
    raw: String,
}
fn normalize(raw: &str) -> Option<String> {
    let key: String = raw
        .trim()
        .trim_matches('.')
        .trim()
        .chars()
        .map(|c| match c {
            'а' | 'А' => 'a',
            'б' => '6',
            _ => c,
        })
        .collect();
    let number = key.strip_suffix('a').unwrap_or(&key);
    if number.is_empty() || number.starts_with('0') || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = number.parse::<u32>().ok()?;
    (1..=299).contains(&n).then_some(key)
}
fn candidates(ctx: &ResearchExecution, poppler: &Poppler, pdf: &File) -> Result<Vec<Candidate>> {
    let mut out = Vec::new();
    for start in (238..=406).step_by(8) {
        for (page, value) in poppler.pages(ctx, pdf, start, (start + 7).min(406))? {
            let mut lines = array(&value["lines"])?.iter().collect::<Vec<_>>();
            lines.sort_by_key(|v| v["source_order"].as_u64());
            for line in lines {
                ctx.tick(1)?;
                let words = array(&line["words"])?;
                if words.len() != 1
                    || !(145.0..=175.0).contains(&f(&line["x_min"])?)
                    || !(40.0..=490.0).contains(&f(&line["y_min"])?)
                {
                    continue;
                }
                let raw = s(&words[0])?;
                let stripped = raw.trim().trim_matches('.').trim();
                if !(1..=5).contains(&stripped.chars().count()) {
                    continue;
                }
                if let Some(key) = normalize(raw) {
                    out.push(Candidate {
                        page,
                        key,
                        raw: raw.into(),
                    });
                    ensure(out.len() <= 4096, "bbox candidate bound")?;
                }
            }
        }
    }
    Ok(out)
}
fn ordered(
    keys: &[String],
    candidates: &[Candidate],
) -> (
    BTreeMap<String, usize>,
    BTreeMap<String, String>,
    Vec<String>,
) {
    let positions = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    let mut next = 0;
    let mut pages = BTreeMap::new();
    let mut raw = BTreeMap::new();
    for c in candidates {
        if let Some(&i) = positions.get(c.key.as_str()) {
            if i < next {
                continue;
            }
            pages.insert(c.key.clone(), c.page);
            raw.insert(c.key.clone(), c.raw.clone());
            next = i + 1;
        }
    }
    let skipped = keys
        .iter()
        .filter(|k| !pages.contains_key(*k))
        .cloned()
        .collect();
    (pages, raw, skipped)
}
fn anchor_id(key: &str, generation: Option<&str>) -> String {
    let old = format!(
        "tos.anchor.friedrich-nietzsche.jenseits-von-gut-und-boese.ru-polilov-mysl-1996.unit-{key}.pdf-start-page"
    );
    generation.map_or_else(|| old.clone(), |g| format!("{old}.native-{g}"))
}
fn passage_id(key: &str) -> String {
    format!(
        "tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese.ru-polilov-mysl-1996.unit-{key}"
    )
}
fn basis(key: &str) -> &'static str {
    if PAGE_OVERRIDES.iter().any(|(k, _)| *k == key) {
        "source_visible_gap_review"
    } else if key == "6" {
        "source_visible_ocr_disambiguation"
    } else {
        "embedded_pdf_bbox_order_candidate"
    }
}
struct Held {
    reference: String,
    file: File,
    metadata: std::fs::Metadata,
    digest: String,
}
impl Held {
    fn open(ctx: &ResearchExecution, reference: &str, cap: u64) -> Result<Self> {
        let mut file = ctx.source_file(reference, cap)?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        let digest = ctx.hash_file(&mut file, cap)?;
        Ok(Self {
            reference: reference.into(),
            file,
            metadata,
            digest,
        })
    }
    fn verify(&mut self, ctx: &ResearchExecution) -> Result<()> {
        ctx.verify_file_unchanged(&self.file, &self.metadata)?;
        let current = ctx.source_file(&self.reference, self.metadata.len())?;
        ctx.verify_file_unchanged(&current, &self.metadata)?;
        ensure(
            ctx.hash_file(&mut self.file, self.metadata.len())? == self.digest,
            "held source bytes changed",
        )
    }
}
pub struct Options<'a> {
    pub build: bool,
    pub input_root: Option<&'a Path>,
    pub generation: Option<&'a str>,
    pub event_at: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, options: Options<'_>) -> Result<Value> {
    ensure(
        !options.build || options.generation.is_some(),
        "build requires a new generation",
    )?;
    ensure(
        options.generation.is_some() == options.event_at.is_some(),
        "generation and event-at must be selected together",
    )?;
    if let Some(g) = options.generation {
        ensure(
            !g.is_empty()
                && g.len() <= 48
                && g.as_bytes()[0].is_ascii_alphanumeric()
                && g.as_bytes().last().unwrap().is_ascii_alphanumeric()
                && g.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "generation component syntax",
        )?;
    }
    let mut schemas = Vec::new();
    let mut schema_holds = Vec::new();
    for reference in [
        "ToS/contracts/target-numbered-unit-page-map.schema.json",
        "ToS/contracts/source-anchor.schema.json",
        "ToS/contracts/provenance-event.schema.json",
    ] {
        let mut h = Held::open(ctx, reference, CAP)?;
        schemas.push(tos_validation::SchemaResource {
            uri: format!("https://tree-of-sophia.local/{reference}"),
            raw: ctx.read_file(&mut h.file, CAP)?,
        });
        schema_holds.push(h);
    }
    let schemas = tos_validation::SchemaBackendProbe::new(
        schemas,
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("schema preparation: {e:?}"))?;
    let validate = |reference: &str, raw: &[u8]| -> Result<()> {
        ensure(
            schemas
                .is_valid_raw(&format!("https://tree-of-sophia.local/{reference}"), raw)
                .map_err(|e| format!("schema execution: {e:?}"))?,
            &format!("{reference} validation failed"),
        )
    };
    let mut held = Vec::new();
    let mut values = Vec::new();
    for reference in [
        MANIFEST_PATH,
        INVENTORY_PATH,
        RIGHTS_PATH,
        PROVENANCE_PATH,
        WORK_BOUNDARY_PATH,
        SOURCE_MAP_PATH,
    ] {
        let mut h = Held::open(ctx, reference, CAP)?;
        let raw = ctx.read_file(&mut h.file, CAP)?;
        let v: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        ensure(v.is_object(), "source metadata object")?;
        held.push(h);
        values.push(v);
    }
    let (manifest, inventory, rights, event) = (&values[0], &values[1], &values[2], &values[3]);
    ensure(
        event["event_id"] == EVENT_ID,
        "historical review event differs",
    )?;
    ensure(
        manifest["embodiment_ref"] == EDITION_REF
            && manifest["rights_ref"] == RIGHTS_PATH
            && manifest["resource_inventory_ref"] == INVENTORY_PATH,
        "target witness binding differs",
    )?;
    let source = ctx.select_directory(options.input_root.unwrap_or(ctx.root()))?;
    let entries = array(&manifest["payload_files"])?;
    ensure(
        entries.len() == 1 && entries[0]["media_type"] == "application/pdf",
        "exactly one target PDF required",
    )?;
    let entry = &entries[0];
    let reference = format!("{TARGET_ITEM_DIR}/{}", s(&entry["relative_path"])?);
    let mut pdf = Held::open(&source, &reference, 128 * 1024 * 1024)?;
    ensure(
        entry["sha256"] == pdf.digest
            && entry["byte_size"].as_u64() == Some(pdf.metadata.len())
            && entry["file_id"] == format!("tos.file.sha256.{}", pdf.digest),
        "target payload fixity differs",
    )?;
    let files = array(&inventory["files"])?;
    ensure(files.len() == 1, "target inventory count differs")?;
    let inventory_pdf = &files[0];
    ensure(
        inventory_pdf["profile"] == "pdf_pages_v1"
            && inventory_pdf["file_id"] == entry["file_id"]
            && inventory_pdf["file_sha256"] == entry["sha256"]
            && inventory_pdf["summary"]["page_count"] == 831,
        "target inventory binding differs",
    )?;
    let members = array(&values[4]["members"])?
        .iter()
        .filter(|v| {
            v["sequence"] == 2 && v["work_ref"] == WORK_REF && v["expression_ref"] == EXPRESSION_REF
        })
        .collect::<Vec<_>>();
    ensure(
        members.len() == 1,
        "target work boundary resolution differs",
    )?;
    let member = members[0];
    ensure(
        member["start_page"] == 238 && member["end_page"] == 406,
        "target work boundary pages differ",
    )?;
    ensure(
        held[5].digest == SOURCE_MAP_SHA256
            && array(&values[5]["unit_starts"])?
                .iter()
                .any(|v| v["unit_key"] == "237a"),
        "source numbered-unit map differs",
    )?;
    let mut poppler = Poppler::open(ctx)?;
    let candidates = candidates(ctx, &poppler, &pdf.file)?;
    let keys = keys();
    let (mut pages, raw_matches, skipped) = ordered(&keys, &candidates);
    let matched = pages.len();
    ensure(
        matched == 265
            && skipped.iter().map(String::as_str).collect::<BTreeSet<_>>()
                == PAGE_OVERRIDES.iter().map(|(k, _)| *k).collect(),
        "ordered bbox matches or review gaps differ",
    )?;
    ensure(
        pages.get("6") == Some(&244)
            && raw_matches.get("6").map(String::as_str) == Some("б")
            && pages.contains_key("65a")
            && pages.contains_key("73a"),
        "retained numeral disambiguation differs",
    )?;
    for (k, p) in PAGE_OVERRIDES {
        pages.insert((*k).into(), *p);
    }
    ensure(
        pages.len() == keys.len()
            && keys.iter().all(|k| pages.contains_key(k))
            && keys.windows(2).all(|v| pages[&v[0]] <= pages[&v[1]]),
        "target coverage/order differs",
    )?;
    let resources = array(&inventory_pdf["resources"])?;
    for page in pages.values() {
        let id = format!("pdf-page-{page:04}");
        ensure(
            resources
                .iter()
                .filter(|v| {
                    v["resource_id"] == id
                        && v["locator"]["page_index"].as_u64() == Some(*page as u64)
                })
                .count()
                == 1,
            "target page leaves inventory",
        )?;
    }
    let generation = options.generation;
    let directory =
        generation.map_or_else(|| OUTPUT_DIR.into(), |g| format!("{OUTPUT_DIR}/native-{g}"));
    let map_ref = format!("{directory}/numbered-unit-page-map.json");
    let anchor_ref = format!("{directory}/numbered-unit-anchors.jsonl");
    let provenance_ref = format!("{directory}/provenance.jsonl");
    let map_id = generation.map_or_else(|| MAP_ID.into(), |g| format!("{MAP_ID}.native-{g}"));
    let event_id = generation.map_or_else(|| EVENT_ID.into(), |g| format!("{EVENT_ID}.native-{g}"));
    let event_at = options.event_at.unwrap_or(s(&event["started_at"])?);
    let units = keys
        .iter()
        .enumerate()
        .map(|(i, k)| {
            o(vec![
                ("sequence", n((i + 1) as u64)),
                ("unit_key", q(k)),
                ("pdf_page", n(pages[k] as u64)),
                ("resource_id", q(format!("pdf-page-{:04}", pages[k]))),
                ("anchor_ref", q(anchor_id(k, generation))),
                ("basis", q(basis(k))),
                ("status", q("proposed")),
                ("human_review_performed", Out::Bool(false)),
            ])
        })
        .collect::<Vec<_>>();
    let mut map = records::map(
        manifest,
        entry,
        member,
        &units,
        matched,
        &held[1].digest,
        &held[4].digest,
        &held[5].digest,
        &map_id,
        &event_id,
        &provenance_ref,
    )?;
    if generation.is_none() {
        set(
            &mut map,
            "authority_boundary",
            q(
                "model-reviewed target-visible numbered-label start-page candidates and proposed whole-page addresses for one exact translation scan only; no target text, exact line boundary, translation alignment, translation equivalence or quality, textual identity, semantics, rights clearance, or canon authority",
            ),
        )?;
    } else {
        set(&mut map, "supersedes_map_ref", q(MAP_ID))?;
    }
    let map_raw = render(&map, true)?;
    let mut anchor_raw = Vec::new();
    for k in &keys {
        let anchor = o(vec![
            ("schema_version", q("tos_source_anchor_v1")),
            ("anchor_id", q(anchor_id(k, generation))),
            ("item_id", q(s(&manifest["item_id"])?)),
            ("file_id", q(s(&entry["file_id"])?)),
            ("file_sha256", q(s(&entry["sha256"])?)),
            ("passage_id", q(passage_id(k))),
            (
                "selectors",
                Out::Array(vec![
                    o(vec![
                        ("type", q("structural")),
                        (
                            "path",
                            Out::Array(vec![
                                q("work:jenseits-von-gut-und-boese"),
                                q("expression:ru-polilov-mysl-1996"),
                                q(format!("numbered-unit:{k}")),
                            ]),
                        ),
                        ("scheme", q("jgb-target-numbered-unit-start-v1")),
                    ]),
                    o(vec![
                        ("type", q("page_region")),
                        ("page", n(pages[k] as u64)),
                        ("x", n(0)),
                        ("y", n(0)),
                        ("width", n(1)),
                        ("height", n(1)),
                        ("coordinate_space", q("normalized_0_1")),
                    ]),
                ]),
            ),
            (
                "selector_method",
                o(vec![
                    ("maker_type", q("mixed")),
                    (
                        "method",
                        q(
                            "ordered embedded-PDF bbox numeral candidate plus bounded target-visible page review",
                        ),
                    ),
                    ("version", q("1")),
                    ("configuration_ref", q(format!("{map_ref}#unit-{k}"))),
                ]),
            ),
            ("status", q("proposed")),
            ("provenance_event_ref", q(&event_id)),
            ("anchor_version", n(1)),
            ("supersedes_anchor_ref", Out::Null),
            ("review_ref", Out::Null),
        ]);
        let raw = render(&anchor, false)?;
        validate("ToS/contracts/source-anchor.schema.json", &raw)?;
        anchor_raw.extend(raw);
    }
    let mut event = records::event(
        entry,
        matched,
        &held[1].digest,
        &held[4].digest,
        &held[5].digest,
        &sha(&map_raw),
        &sha(&anchor_raw),
        &map_ref,
        &anchor_ref,
        &event_id,
        event_at,
    )?;
    if generation.is_some() {
        set(
            &mut event,
            "agent_refs",
            Out::Array(vec![q("software:tos-rust"), q("software:poppler-26.01.0")]),
        )?;
        let method = field(&mut event, "method")?;
        set(
            method,
            "artifact_digest",
            q(sha(include_bytes!(
                "jenseits_polilov_numbered_structure.rs"
            ))),
        )?;
        set(
            method,
            "runtime",
            q(
                "Tree of Sophia Rust; Poppler 26.01.0; retained target-visible page-review decisions",
            ),
        )?;
        for (reference, role, digest) in [
            (
                PROVENANCE_PATH,
                "retained-source-visible-scan-review",
                held[3].digest.as_str(),
            ),
            (
                RIGHTS_PATH,
                "current-rights-posture",
                held[2].digest.as_str(),
            ),
            (
                BUILDER,
                "native-generator-source",
                sha(include_bytes!("jenseits_polilov_numbered_structure.rs")).as_str(),
            ),
            (
                "rust/crates/tos-compiler/src/jenseits_polilov_numbered_structure/constants.rs",
                "native-generator-constants",
                sha(include_bytes!(
                    "jenseits_polilov_numbered_structure/constants.rs"
                ))
                .as_str(),
            ),
            (
                "rust/crates/tos-compiler/src/jenseits_polilov_numbered_structure/records.rs",
                "native-generator-records",
                sha(include_bytes!(
                    "jenseits_polilov_numbered_structure/records.rs"
                ))
                .as_str(),
            ),
        ] {
            append(
                field(&mut event, "inputs")?,
                o(vec![
                    ("ref", q(reference)),
                    ("role", q(role)),
                    ("sha256", q(digest)),
                ]),
            )?;
        }
    }
    if generation.is_none() {
        let Out::Array(warnings) = field(&mut event, "warnings")? else {
            return Err("event warnings array".into());
        };
        warnings[4] = q(
            "No source-to-target unit alignment, translation equivalence, translation quality, or semantic claim was made.",
        );
    }
    let event_raw = render(&event, false)?;
    validate(
        "ToS/contracts/target-numbered-unit-page-map.schema.json",
        &map_raw,
    )?;
    validate("ToS/contracts/provenance-event.schema.json", &event_raw)?;
    ensure(rights.is_object(), "rights object required")?;
    for h in held.iter_mut().chain(schema_holds.iter_mut()) {
        h.verify(ctx)?;
    }
    pdf.verify(&source)?;
    poppler.verify()?;
    let outputs = [
        (map_ref, map_raw),
        (anchor_ref, anchor_raw),
        (provenance_ref, event_raw),
    ];
    let mut missing = Vec::new();
    for (reference, bytes) in &outputs {
        match std::fs::symlink_metadata(ctx.root().join(reference)) {
            Ok(_) => {
                ensure(
                    ctx.read(reference)? == *bytes,
                    &format!(
                        "structure output differs: {reference}; expected_sha256={} retained_sha256={}",
                        sha(bytes),
                        sha(&ctx.read(reference)?)
                    ),
                )?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ensure(
                    options.build,
                    &format!("structure output absent: {reference}"),
                )?;
                missing.push(reference.as_str());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let mut written = 0;
    for (reference, bytes) in &outputs {
        if missing.contains(&reference.as_str()) {
            ctx.write(reference, bytes, 0o644, true)?;
            written += 1;
        }
    }
    Ok(
        json!({"status":if options.build{"built"}else{"current"},"generator":BUILDER,"generation":generation,"historical_reconstruction_only":generation.is_none(),"numbered_units":keys.len(),"machine_matches":matched,"reviewed_overrides":PAGE_OVERRIDES.len(),"outputs_written":written,"outputs":outputs.iter().map(|(r,b)|json!({"ref":r,"bytes":b.len(),"sha256":sha(b)})).collect::<Vec<_>>(),"accepted_source_text":false,"human_review_performed":false,"canon_effect":false,"execution_budget":ctx.budget_report()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_candidates_preserve_missing_units_and_raw_labels() {
        let keys = ["1", "2", "3", "4"].map(str::to_owned);
        let candidates =
            [(10, "1"), (11, "3"), (12, "2"), (13, "4")].map(|(page, key)| Candidate {
                page,
                key: key.into(),
                raw: format!("{key}."),
            });
        let (pages, raw, gaps) = ordered(&keys, &candidates);
        assert_eq!(
            pages,
            [("1".into(), 10), ("3".into(), 11), ("4".into(), 13)].into()
        );
        assert_eq!(gaps, ["2"]);
        assert_eq!(
            raw,
            [
                ("1".into(), "1.".into()),
                ("3".into(), "3.".into()),
                ("4".into(), "4.".into())
            ]
            .into()
        );
    }
}
