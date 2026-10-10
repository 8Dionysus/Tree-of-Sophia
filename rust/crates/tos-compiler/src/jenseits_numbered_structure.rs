//! Numbered source structure from the exact ABBYY navigation layer and the
//! retained bounded scan-review decisions. No OCR text is emitted or accepted.
use crate::{
    constructor_library::Out,
    research_execution::ResearchExecution,
    source_text_foundation::{ensure, s, sha},
    transfer_target_passages::{descendants, node_text},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    path::Path,
};
#[path = "jenseits_numbered_structure/constants.rs"]
mod constants;
#[path = "jenseits_numbered_structure/records.rs"]
mod records;
use constants::*;
type Result<T> = std::result::Result<T, String>;
const CAP: u64 = 4 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/jenseits_numbered_structure.rs";
pub(super) fn q(v: impl Into<String>) -> Out {
    Out::String(v.into())
}
pub(super) fn n(v: u64) -> Out {
    Out::Integer(v)
}
pub(super) fn o(v: Vec<(&str, Out)>) -> Out {
    Out::Object(v.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub(super) fn field<'a>(v: &'a mut Out, key: &str) -> Result<&'a mut Out> {
    let Out::Object(rows) = v else {
        return Err("ordered object required".into());
    };
    rows.iter_mut()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| format!("missing ordered field {key}"))
}
pub(super) fn set(v: &mut Out, key: &str, next: Out) -> Result<()> {
    *field(v, key)? = next;
    Ok(())
}
pub(super) fn append(v: &mut Out, next: Out) -> Result<()> {
    let Out::Array(rows) = v else {
        return Err("ordered array required".into());
    };
    rows.push(next);
    Ok(())
}
pub(super) fn render(v: &Out, pretty: bool) -> Result<Vec<u8>> {
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
pub(super) fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("array required".into())
}
fn keys() -> Vec<String> {
    (1..=296)
        .flat_map(|n| {
            let mut v = vec![n.to_string()];
            if [65, 73, 237].contains(&n) {
                v.push(format!("{n}a"));
            }
            v
        })
        .collect()
}
#[derive(Clone, Debug)]
struct Candidate {
    page: usize,
    compact: String,
}
fn candidates(ctx: &ResearchExecution, raw: &[u8]) -> Result<Vec<Candidate>> {
    let wanted = (10..=266).collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    crate::transfer_source_passages::visit_xml_pages(
        ctx,
        flate2::read::MultiGzDecoder::new(std::io::Cursor::new(raw)),
        true,
        274,
        &wanted,
        |page, node| {
            let mut paragraphs = vec![];
            descendants(node, "par", &mut paragraphs);
            for par in paragraphs {
                ctx.tick(1)?;
                let mut chars = vec![];
                descendants(par, "charParams", &mut chars);
                let text = chars.into_iter().map(node_text).collect::<String>();
                let text = text.trim();
                let mut lines = vec![];
                descendants(par, "line", &mut lines);
                let mut top = None;
                for line in lines {
                    let value = line
                        .attrs
                        .get("t")
                        .map(String::as_str)
                        .unwrap_or("99999")
                        .parse::<i64>()
                        .map_err(|_| "ABBYY line coordinate")?;
                    top = Some(top.map_or(value, |n: i64| n.min(value)));
                }
                let top = top.unwrap_or(-1);
                if !(400..=3500).contains(&top)
                    || !(1..=14).contains(&text.chars().count())
                    || !text.chars().all(|c| OCR_ALLOWED_CHARACTERS.contains(c))
                    || !text
                        .chars()
                        .any(|c| c.is_ascii_digit() || "IiLl".contains(c))
                {
                    continue;
                }
                let compact = text
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '»')
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if !compact.is_empty() {
                    out.push(Candidate { page, compact });
                    ensure(out.len() <= 4096, "ABBYY candidate count bound")?;
                }
            }
            Ok(())
        },
    )?;
    Ok(out)
}
fn distance(expected: &str, observed: &str) -> f64 {
    let observed = observed.chars().collect::<Vec<_>>();
    let mut previous = (0..=observed.len()).map(|n| n as f64).collect::<Vec<_>>();
    for (row, a) in expected.chars().enumerate() {
        let mut current = vec![(row + 1) as f64];
        for (col, b) in observed.iter().enumerate() {
            let cost = if a == *b {
                0.0
            } else if OCR_SUBSTITUTIONS
                .iter()
                .any(|(k, v)| k.starts_with(a) && v.contains(*b))
            {
                0.22
            } else {
                1.0
            };
            current.push(
                (previous[col + 1] + 1.0)
                    .min(current[col] + 1.0)
                    .min(previous[col] + cost),
            );
        }
        previous = current;
    }
    previous[observed.len()]
}
fn ordered(
    ctx: &ResearchExecution,
    keys: &[String],
    candidates: &[Candidate],
) -> Result<(BTreeMap<String, usize>, Vec<String>)> {
    ensure(
        keys.len() <= 299 && candidates.len() <= 4096,
        "number alignment dimensions",
    )?;
    let width = candidates.len() + 1;
    let len = (keys.len() + 1)
        .checked_mul(width)
        .ok_or("alignment allocation overflow")?;
    let mut scores = vec![f64::INFINITY; len];
    let mut route = vec![0u8; len];
    scores[0] = 0.0;
    for i in 0..=keys.len() {
        for j in 0..=candidates.len() {
            ctx.tick(1)?;
            let at = i * width + j;
            let score = scores[at];
            if i < keys.len() {
                let to = at + width;
                let proposed = score + 0.92;
                if proposed < scores[to] {
                    scores[to] = proposed;
                    route[to] = 1;
                }
            }
            if j < candidates.len() {
                let to = at + 1;
                let proposed = score + 0.04;
                if proposed < scores[to] {
                    scores[to] = proposed;
                    route[to] = 2;
                }
            }
            if i < keys.len() && j < candidates.len() {
                let cost = distance(keys[i].trim_end_matches('a'), &candidates[j].compact);
                let to = at + width + 1;
                let proposed = score + cost;
                if cost <= 1.55 && proposed < scores[to] {
                    scores[to] = proposed;
                    route[to] = 3;
                }
            }
        }
    }
    let (mut i, mut j) = (keys.len(), candidates.len());
    let mut actions = Vec::new();
    while i > 0 || j > 0 {
        ctx.tick(1)?;
        let action = route[i * width + j];
        match action {
            1 => {
                i -= 1;
                actions.push((false, i, j));
            }
            2 => {
                j -= 1;
            }
            3 => {
                i -= 1;
                j -= 1;
                actions.push((true, i, j));
            }
            _ => return Err("ordered ABBYY route incomplete".into()),
        }
    }
    actions.reverse();
    let mut matches = BTreeMap::new();
    let mut skipped = Vec::new();
    for (matched, i, j) in actions {
        if matched {
            matches.insert(keys[i].clone(), candidates[j].page);
        } else {
            skipped.push(keys[i].clone());
        }
    }
    Ok((matches, skipped))
}
fn anchor_id(key: &str, generation: Option<&str>) -> String {
    let old = format!(
        "tos.anchor.friedrich-nietzsche.jenseits-von-gut-und-boese.de-naumann-1886.unit-{key}.pdf-start-page"
    );
    generation.map_or_else(|| old.clone(), |g| format!("{old}.native-{g}"))
}
fn passage_id(key: &str) -> String {
    format!("tos.passage.friedrich-nietzsche.jenseits-von-gut-und-boese.de-naumann-1886.unit-{key}")
}
fn basis(key: &str) -> &'static str {
    if key == "237a" {
        "source_visible_repeated_number_review"
    } else if GAP_REVIEW_KEYS.contains(&key) {
        "source_visible_gap_review"
    } else if OCR_DISAMBIGUATION_KEYS.contains(&key) {
        "source_visible_ocr_disambiguation"
    } else {
        "ocr_order_candidate"
    }
}
pub(super) struct Held {
    pub(super) reference: String,
    pub(super) file: File,
    pub(super) metadata: std::fs::Metadata,
    pub(super) digest: String,
}
impl Held {
    pub(super) fn open(ctx: &ResearchExecution, reference: &str, cap: u64) -> Result<Self> {
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
    pub(super) fn verify(&mut self, ctx: &ResearchExecution) -> Result<()> {
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
        "ToS/contracts/numbered-unit-page-map.schema.json",
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
    for reference in [MANIFEST_PATH, INVENTORY_PATH, RIGHTS_REF, PROVENANCE_PATH] {
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
        manifest["item_id"]
            == "tos.item.friedrich-nietzsche.jenseits-von-gut-und-boese.de-naumann-1886.internet-archive-google-harvard-scan-pdf"
            && manifest["embodiment_ref"] == EDITION_REF
            && manifest["rights_ref"] == RIGHTS_REF
            && manifest["resource_inventory_ref"] == INVENTORY_PATH,
        "Jenseits witness identity differs",
    )?;
    let source = ctx.select_directory(options.input_root.unwrap_or(ctx.root()))?;
    let mut entries = BTreeMap::new();
    let mut payloads = BTreeMap::new();
    for entry in array(&manifest["payload_files"])? {
        let media = s(&entry["media_type"])?;
        let role = match media {
            "application/pdf" => "pdf",
            "application/vnd.djvu+xml" => "djvu",
            "application/gzip" if s(&entry["relative_path"])?.ends_with(".abbyy.xml.gz") => "abbyy",
            _ => return Err("unexpected Jenseits payload media".into()),
        };
        ensure(!entries.contains_key(role), "duplicate payload role")?;
        let reference = format!("{ITEM_DIR}/{}", s(&entry["relative_path"])?);
        let h = Held::open(&source, &reference, 128 * 1024 * 1024)?;
        ensure(
            entry["sha256"] == h.digest
                && entry["byte_size"].as_u64() == Some(h.metadata.len())
                && entry["file_id"] == format!("tos.file.sha256.{}", h.digest),
            "source payload fixity/identity differs",
        )?;
        entries.insert(role.to_owned(), entry.clone());
        payloads.insert(role, h);
    }
    ensure(entries.len() == 3, "PDF DjVu and ABBYY required")?;
    let mut profiles = BTreeSet::new();
    for f in array(&inventory["files"])? {
        let profile = s(&f["profile"])?;
        let role = match profile {
            "pdf_pages_v1" => "pdf",
            "djvu_xml_pages_v1" => "djvu",
            "abbyy_xml_pages_v1" => "abbyy",
            _ => return Err("source inventory profile differs".into()),
        };
        ensure(
            profiles.insert(profile)
                && f["file_id"] == entries[role]["file_id"]
                && f["summary"]["page_count"].as_u64() == Some(274),
            "source inventory identity/count differs",
        )?;
    }
    ensure(profiles.len() == 3, "source inventory profile coverage")?;
    let abbyy = payloads.get_mut("abbyy").unwrap();
    let raw = source.read_file(&mut abbyy.file, abbyy.metadata.len())?;
    let candidates = candidates(ctx, &raw)?;
    drop(raw);
    let keys = keys();
    let (mut pages, skipped) = ordered(ctx, &keys, &candidates)?;
    let matched = pages.len();
    ensure(
        skipped.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == PAGE_OVERRIDES.iter().map(|(k, _)| *k).collect(),
        "ordered ABBYY gap set differs from retained scan review",
    )?;
    for (k, p) in PAGE_OVERRIDES {
        pages.insert((*k).into(), *p);
    }
    ensure(
        pages.len() == keys.len()
            && keys.iter().all(|k| pages.contains_key(k))
            && keys.windows(2).all(|p| pages[&p[0]] <= pages[&p[1]]),
        "numbered-unit coverage or order differs",
    )?;
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
        &entries,
        &units,
        matched,
        &held[1].digest,
        &map_id,
        &event_id,
        &provenance_ref,
    )?;
    if generation.is_none() {
        set(
            &mut map,
            "authority_boundary",
            q(
                "model-reviewed source-visible numbered-unit start-page candidates and proposed whole-page addresses for one exact scan only; no source text, exact line boundary, textual acceptance, critical equivalence, translation correspondence, semantics, rights clearance, or canon authority",
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
            ("file_id", q(s(&entries["pdf"]["file_id"])?)),
            ("file_sha256", q(s(&entries["pdf"]["sha256"])?)),
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
                                q("expression:de-naumann-1886"),
                                q(format!("numbered-unit:{k}")),
                            ]),
                        ),
                        ("scheme", q("jgb-numbered-unit-start-v1")),
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
                            "ordered ABBYY numeral candidate plus bounded source-visible scan review",
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
        &entries,
        matched,
        &held[1].digest,
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
            Out::Array(vec![q("software:tos-rust")]),
        )?;
        let method = field(&mut event, "method")?;
        set(
            method,
            "artifact_digest",
            q(sha(include_bytes!("jenseits_numbered_structure.rs"))),
        )?;
        set(
            method,
            "runtime",
            q("Tree of Sophia Rust; exact ABBYY navigation and retained scan-review decisions"),
        )?;
        for (reference, role, digest) in [
            (
                PROVENANCE_PATH,
                "retained-source-visible-scan-review",
                held[3].digest.as_str(),
            ),
            (
                RIGHTS_REF,
                "current-rights-posture",
                held[2].digest.as_str(),
            ),
            (
                BUILDER,
                "native-generator-source",
                sha(include_bytes!("jenseits_numbered_structure.rs")).as_str(),
            ),
            (
                "rust/crates/tos-compiler/src/jenseits_numbered_structure/constants.rs",
                "native-generator-constants",
                sha(include_bytes!("jenseits_numbered_structure/constants.rs")).as_str(),
            ),
            (
                "rust/crates/tos-compiler/src/jenseits_numbered_structure/records.rs",
                "native-generator-records",
                sha(include_bytes!("jenseits_numbered_structure/records.rs")).as_str(),
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
    let event_raw = render(&event, false)?;
    validate("ToS/contracts/numbered-unit-page-map.schema.json", &map_raw)?;
    validate("ToS/contracts/provenance-event.schema.json", &event_raw)?;
    ensure(rights.is_object(), "rights object required")?;
    for h in held.iter_mut().chain(schema_holds.iter_mut()) {
        h.verify(ctx)?;
    }
    for h in payloads.values_mut() {
        h.verify(&source)?;
    }
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
    fn ordered_numerals_keep_repeated_keys_and_skip_unrelated_marks() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(dir.path(), 30).unwrap();
        let selected = ["65", "65a", "66", "67"].map(str::to_owned);
        let candidates =
            [(10, "65"), (11, "65"), (11, "noise"), (12, "67")].map(|(page, s)| Candidate {
                page,
                compact: s.into(),
            });
        let (matched, skipped) = ordered(&ctx, &selected, &candidates).unwrap();
        assert_eq!(
            matched,
            BTreeMap::from([("65".into(), 10), ("65a".into(), 11), ("67".into(), 12)])
        );
        assert_eq!(skipped, ["66"]);
        assert!((distance("188", "i8s") - 0.44).abs() < 1e-9);
        assert_eq!(keys().len(), 299);
    }
}
