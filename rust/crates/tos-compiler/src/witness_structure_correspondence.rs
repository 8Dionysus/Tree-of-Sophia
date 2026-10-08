//! Text-free DTA / Naumann structural candidates with exact resource custody.
use crate::{
    constructor_library::Out,
    jenseits_numbered_structure::{Held, array, field, n, o, q, set},
    nietzsche_transfer_source_structure::Witness,
    research_execution::ResearchExecution,
    research_html::HtmlEvent,
    source_philosophy_dossier_docx::OfficeArchive,
    source_text_foundation::{Node, Part, ensure, s, sha, xml_with_doctype},
    transfer_target_passages::{encode, node_text, round},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
use unicode_normalization::UnicodeNormalization;
#[path = "witness_structure_correspondence/constants.rs"]
mod constants;
#[path = "witness_structure_correspondence/records.rs"]
mod records;
use constants::*;
type Result<T> = std::result::Result<T, String>;
const CAP: u64 = 4 * 1024 * 1024;
const BUILDER: &str = "rust/crates/tos-compiler/src/witness_structure_correspondence.rs";
fn u(v: &Value) -> Result<usize> {
    v.as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("required bounded integer".into())
}
fn v(x: &Value) -> Result<Out> {
    match x {
        Value::Null => Ok(Out::Null),
        Value::Bool(b) => Ok(Out::Bool(*b)),
        Value::String(s) => Ok(q(s)),
        Value::Number(k) => k
            .as_u64()
            .map(n)
            .or_else(|| k.as_i64().map(Out::Signed))
            .or_else(|| k.as_f64().filter(|f| f.is_finite()).map(Out::Float))
            .ok_or("finite number required".into()),
        Value::Array(a) => Ok(Out::Array(a.iter().map(v).collect::<Result<_>>()?)),
        Value::Object(m) => Ok(Out::Object(
            m.iter()
                .map(|(k, x)| Ok((k.clone(), v(x)?)))
                .collect::<Result<_>>()?,
        )),
    }
}
fn insert(x: &mut Out, key: &str, next: Out) -> Result<()> {
    let Out::Object(fields) = x else {
        return Err("ordered object required".into());
    };
    ensure(
        !fields.iter().any(|(k, _)| k == key),
        "duplicate generated field",
    )?;
    fields.push((key.into(), next));
    Ok(())
}

fn value(x: &Out) -> Result<Value> {
    serde_json::to_value(x).map_err(|e| e.to_string())
}
fn tokens(text: &str) -> Result<Vec<String>> {
    ensure(text.len() <= CAP as usize, "normalization byte bound")?;
    let nfkc = text.nfkc().collect::<String>();
    let folded = tos_foundation::python_casefold_unicode16_v1(
        &nfkc,
        CAP as usize * 4,
        CAP as usize * 12,
        CAP as usize * 12,
    )
    .map_err(|e| e.to_string())?;
    Ok(folded
        .split(|c: char| !c.is_ascii_lowercase() && !matches!(c, 'ä' | 'ö' | 'ü' | 'ß'))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect())
}
fn count(tokens: &[String]) -> BTreeMap<&str, u128> {
    let mut c = BTreeMap::new();
    for t in tokens {
        *c.entry(t.as_str()).or_default() += 1
    }
    c
}
fn fingerprint(tokens: &[String]) -> Out {
    o(vec![
        ("algorithm", q("sha256")),
        ("normalization", q(NORMALIZATION)),
        ("sha256", q(sha(tokens.join(" ").as_bytes()))),
        ("token_count", n(tokens.len() as u64)),
    ])
}
fn choose(
    ctx: &ResearchExecution,
    heading: &[String],
    source: &[String],
    pages: &BTreeMap<usize, Vec<String>>,
    first: usize,
    last: usize,
) -> Result<(usize, Out)> {
    ensure(first <= last && last - first < 529, "bounded target range")?;
    let empty = Vec::new();
    let exact = (first..=last)
        .filter(|p| {
            !heading.is_empty()
                && pages
                    .get(p)
                    .unwrap_or(&empty)
                    .windows(heading.len())
                    .any(|w| w == heading)
        })
        .collect::<Vec<_>>();
    let candidates = if exact.is_empty() {
        (first..=last).collect::<Vec<_>>()
    } else {
        exact.clone()
    };
    let source = count(&source[..source.len().min(160)]);
    let heading = count(heading);
    let source_sq = source.values().map(|n| n * n).sum::<u128>();
    let heading_total = heading.values().sum::<u128>();
    let mut scores = Vec::new();
    for page in candidates {
        ctx.tick(1)?;
        let mut window = pages.get(&page).unwrap_or(&empty).clone();
        window.extend_from_slice(pages.get(&(page + 1)).unwrap_or(&empty));
        let target = count(&window);
        let denominator =
            ((source_sq * target.values().map(|n| n * n).sum::<u128>()) as f64).sqrt();
        let cosine = if denominator == 0.0 {
            0.0
        } else {
            source
                .iter()
                .map(|(t, n)| n * target.get(t).unwrap_or(&0))
                .sum::<u128>() as f64
                / denominator
        };
        let coverage = if heading_total == 0 {
            0.0
        } else {
            heading
                .iter()
                .map(|(t, n)| (*n).min(*target.get(t).unwrap_or(&0)))
                .sum::<u128>() as f64
                / heading_total as f64
        };
        scores.push((0.8 * cosine + 0.2 * coverage, page, cosine, coverage));
    }
    scores.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let a = scores[0];
    let runner = scores.get(1);
    let mode = match exact.len() {
        0 => "normalized_context_without_heading_sequence",
        1 => "normalized_heading_unique",
        _ => "normalized_heading_context_disambiguated",
    };
    Ok((
        a.1,
        o(vec![
            ("mode", q(mode)),
            ("normalized_heading_occurrence_count", n(exact.len() as u64)),
            ("context_cosine", Out::Float(round(a.2, 6))),
            ("heading_token_coverage", Out::Float(round(a.3, 6))),
            ("candidate_score", Out::Float(round(a.0, 6))),
            (
                "runner_up_score",
                runner.map_or(Out::Null, |b| Out::Float(round(b.0, 6))),
            ),
            (
                "score_margin",
                runner.map_or(Out::Null, |b| Out::Float(round((a.0 - b.0).max(0.0), 6))),
            ),
        ]),
    ))
}
struct Division {
    heading: Vec<String>,
    content: Vec<String>,
}
fn text_segments<'a>(node: &'a Node, out: &mut Vec<&'a str>) {
    for part in &node.content {
        match part {
            Part::Text(t) => out.push(t),
            Part::Child(n) => text_segments(n, out),
        }
    }
}
fn divisions(ctx: &ResearchExecution, root: &Node) -> Result<BTreeMap<String, Division>> {
    fn walk(
        ctx: &ResearchExecution,
        node: &Node,
        path: &str,
        out: &mut BTreeMap<String, Division>,
    ) -> Result<()> {
        let mut counts = BTreeMap::new();
        for part in &node.content {
            if let Part::Child(child) = part {
                ctx.tick(1)?;
                let seq = counts.entry(child.name.as_str()).or_insert(0usize);
                *seq += 1;
                let path = format!("{path}/{}[{seq}]", child.name);
                if child.name == "div" {
                    let head = child
                        .children("head")
                        .first()
                        .map_or(String::new(), |h| node_text(h));
                    let mut parts = Vec::new();
                    text_segments(child, &mut parts);
                    out.insert(
                        path.clone(),
                        Division {
                            heading: tokens(&head)?,
                            content: tokens(&parts.join(" "))?,
                        },
                    );
                }
                walk(ctx, child, &path, out)?;
            }
        }
        Ok(())
    }
    ensure(root.name == "TEI", "source must be a TEI root")?;
    let mut out = BTreeMap::new();
    walk(ctx, root, "TEI", &mut out)?;
    Ok(out)
}
fn selected(part: &str, r: &Value, heading: &[String]) -> bool {
    let Some(path) = r["locator"]["tei_path"].as_str() else {
        return false;
    };
    let depth = r["locator"]["tei_depth"].as_u64();
    r["resource_kind"] == "tei_division"
        && r["structural_role"] == "division"
        && path.contains("/body[1]/")
        && !heading.is_empty()
        && if part == "I" {
            depth == Some(1)
                || depth == Some(2) && path.starts_with("TEI/text[1]/body[1]/div[2]/div[")
        } else {
            depth == Some(1) && !(part == "IV" && path.starts_with("TEI/text[1]/body[1]/div[21]"))
        }
}
fn witness(
    ctx: &ResearchExecution,
    source: &ResearchExecution,
    manifest: &str,
    media: &str,
    profile: &str,
) -> Result<Witness> {
    let dir = manifest
        .strip_suffix("/item.manifest.json")
        .ok_or("witness manifest path")?;
    let w = Witness::open(ctx, source, dir, media, profile)?;
    ensure(
        array(&w.manifest["payload_files"])?.len() == 1,
        "one witness payload required",
    )?;
    Ok(w)
}
fn binding(w: &Witness, part: Option<&str>) -> Result<Out> {
    let mut fields = Vec::new();
    if let Some(p) = part {
        fields.push(("part_label", q(p)))
    }
    fields.extend(vec![
        ("item_ref", v(&w.manifest["item_id"])?),
        ("file_ref", v(&w.entry["file_id"])?),
        ("file_sha256", q(&w.payload.digest)),
        ("inventory_ref", q(&w.held[1].reference)),
        ("profile", q(&w.profile)),
    ]);
    Ok(o(fields))
}
fn input(reference: &str, role: &str, digest: &str) -> Out {
    o(vec![
        ("ref", q(reference)),
        ("role", q(role)),
        ("sha256", q(digest)),
    ])
}
struct Data<'a> {
    source_parts: Vec<Out>,
    epub: Out,
    pdf: Out,
    routes: Vec<Out>,
    rows: Vec<Out>,
    part_counts: Out,
    mode_counts: Out,
    inputs: Vec<Out>,
    bindings: Vec<Out>,
    anchors: Vec<Out>,
    event_at: &'a str,
    map_digest: String,
    set_digest: String,
    anchor_digest: String,
}
fn row(
    ctx: &ResearchExecution,
    part: &Value,
    w: &Witness,
    r: &Value,
    division: &Division,
    epub: &Witness,
    pdf: &Witness,
    pages: &BTreeMap<usize, Vec<String>>,
    epub_resources: &BTreeMap<String, Value>,
    pdf_resources: &BTreeMap<String, Value>,
    sequence: usize,
) -> Result<(usize, Out)> {
    let first = u(&part["first_target_page"])?;
    let last = u(&part["last_target_page"])?;
    let (page, mut matched) = choose(
        ctx,
        &division.heading,
        &division.content,
        pages,
        first,
        last,
    )?;
    let member = format!("EPUB/page_{page}.html");
    let ep = epub_resources
        .get(&member)
        .ok_or("EPUB inventory lacks selected member")?;
    let pdf_id = format!("pdf-page-{:04}", page + 1);
    let pd = pdf_resources
        .get(&pdf_id)
        .ok_or("PDF inventory lacks selected page")?;
    ensure(
        r["label_fingerprint"].is_object(),
        "division label fingerprint required",
    )?;
    let part_label = s(&part["part_label"])?;
    insert(
        &mut matched,
        "search_page_range",
        o(vec![("first", n(first as u64)), ("last", n(last as u64))]),
    )?;
    insert(
        &mut matched,
        "target_window_pages",
        Out::Array((page..=(page + 1).min(last)).map(|p| n(p as u64)).collect()),
    )?;
    insert(&mut matched, "status", q("machine_corroborated_candidate"))?;
    Ok((
        page,
        o(vec![
            (
                "correspondence_id",
                q(format!(
                    "structure-{}-{sequence:03}",
                    part_label.to_ascii_lowercase()
                )),
            ),
            ("part_label", q(part_label)),
            ("sequence", n(sequence as u64)),
            (
                "source",
                o(vec![
                    ("item_ref", v(&w.manifest["item_id"])?),
                    ("file_ref", v(&w.entry["file_id"])?),
                    ("inventory_ref", q(&w.held[1].reference)),
                    ("resource_id", v(&r["resource_id"])?),
                    ("tei_path", v(&r["locator"]["tei_path"])?),
                    ("tei_depth", v(&r["locator"]["tei_depth"])?),
                    ("tei_page_label", v(&r["locator"]["tei_page_label"])?),
                    ("label_fingerprint", v(&r["label_fingerprint"])?),
                    (
                        "matching_window_fingerprint",
                        fingerprint(&division.content[..division.content.len().min(160)]),
                    ),
                ]),
            ),
            (
                "target_epub",
                o(vec![
                    ("item_ref", v(&epub.manifest["item_id"])?),
                    ("file_ref", v(&epub.entry["file_id"])?),
                    ("inventory_ref", q(&epub.held[1].reference)),
                    ("resource_id", v(&ep["resource_id"])?),
                    ("member_path", q(&member)),
                    ("member_sha256", v(&ep["sha256"])?),
                    ("scan_page_number", n(page as u64)),
                    ("spine_index", v(&ep["locator"]["spine_index"])?),
                    ("content_fingerprint", v(&ep["content_fingerprint"])?),
                ]),
            ),
            (
                "target_pdf",
                o(vec![
                    ("item_ref", v(&pdf.manifest["item_id"])?),
                    ("file_ref", v(&pdf.entry["file_id"])?),
                    ("inventory_ref", q(&pdf.held[1].reference)),
                    ("resource_id", v(&pd["resource_id"])?),
                    ("page_index", n((page + 1) as u64)),
                ]),
            ),
            ("match", matched),
        ]),
    ))
}
fn anchors(d: &mut Data) -> Result<()> {
    let witnesses = d
        .source_parts
        .iter()
        .map(value)
        .collect::<Result<Vec<_>>>()?;
    let ew = value(&d.epub)?;
    let pw = value(&d.pdf)?;
    for c in &d.rows {
        let c = value(c)?;
        let id = s(&c["correspondence_id"])?;
        let suffix = id
            .strip_prefix("structure-")
            .ok_or("correspondence identity")?;
        let sw = witnesses
            .iter()
            .find(|w| w["item_ref"] == c["source"]["item_ref"])
            .ok_or("source anchor witness")?;
        let ids = [
            format!("tos.anchor.zarathustra-structure.dta-{suffix}"),
            format!("tos.anchor.zarathustra-structure.naumann-1893-epub-{suffix}"),
            format!("tos.anchor.zarathustra-structure.naumann-1893-pdf-{suffix}"),
        ];
        let selectors = [
            o(vec![
                ("type", q("structural")),
                ("path", Out::Array(vec![v(&c["source"]["tei_path"])?])),
                ("scheme", q("tei-xpath-like-inventory-v1")),
            ]),
            o(vec![
                ("type", q("container_member")),
                ("member_path", v(&c["target_epub"]["member_path"])?),
                ("member_sha256", v(&c["target_epub"]["member_sha256"])?),
            ]),
            o(vec![
                ("type", q("page_region")),
                ("page", v(&c["target_pdf"]["page_index"])?),
                ("x", n(0)),
                ("y", n(0)),
                ("width", n(1)),
                ("height", n(1)),
                ("coordinate_space", q("normalized_0_1")),
            ]),
        ];
        for ((anchor, w), (selector, method)) in
            ids.iter()
                .zip([sw, &ew, &pw])
                .zip(selectors.into_iter().zip([
                    "resource-inventory TEI division locator",
                    "resource-inventory exact EPUB member locator",
                    "resource-inventory whole-page PDF locator",
                ]))
        {
            d.anchors
                .push(records::anchor(anchor, w, selector, method, id)?)
        }
        let mut b = vec![
            ("correspondence_id", q(id)),
            ("part_label", v(&c["part_label"])?),
            ("sequence", v(&c["sequence"])?),
        ];
        for ((role, source), anchor) in [
            ("source_tei", "source"),
            ("target_epub", "target_epub"),
            ("target_pdf", "target_pdf"),
        ]
        .into_iter()
        .zip(ids)
        {
            b.push((
                role,
                o(vec![
                    ("anchor_ref", q(anchor)),
                    ("item_ref", v(&c[source]["item_ref"])?),
                    ("file_ref", v(&c[source]["file_ref"])?),
                    ("resource_id", v(&c[source]["resource_id"])?),
                ]),
            ))
        }
        b.extend([
            (
                "binding_status",
                q("proposed_cross_witness_locator_candidate"),
            ),
            ("exact_textual_identity_claimed", Out::Bool(false)),
        ]);
        d.bindings.push(o(b));
    }
    Ok(())
}
fn rewrite(x: &mut Out, g: &str, paths: &BTreeMap<&str, String>) {
    match x {
        Out::String(s) => {
            for (old, new) in paths {
                if s == old {
                    *s = new.clone();
                    return;
                }
                if let Some(tail) = s.strip_prefix(&format!("{old}#")) {
                    *s = format!("{new}#{tail}");
                    return;
                }
            }
            if [MAP_ID, ANCHOR_SET_ID, EVENT_ID, ANCHOR_EVENT_ID].contains(&s.as_str())
                || (s.starts_with("tos.anchor.zarathustra-structure.")
                    && !s.ends_with(&format!(".native-{g}")))
            {
                s.push_str(&format!(".native-{g}"));
            }
        }
        Out::Array(xs) => {
            for x in xs {
                rewrite(x, g, paths)
            }
        }
        Out::Object(xs) => {
            for (_, x) in xs {
                rewrite(x, g, paths)
            }
        }
        _ => {}
    }
}
fn lines(ctx: &ResearchExecution, rows: &[Out]) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    for r in rows {
        raw.extend(encode(ctx, r, false)?);
        raw.push(b'\n');
        ensure(raw.len() <= CAP as usize, "JSONL byte bound")?;
    }
    Ok(raw)
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
        "build requires new generation",
    )?;
    ensure(
        options.generation.is_some() == options.event_at.is_some(),
        "generation and event-at required together",
    )?;
    if let Some(g) = options.generation {
        ensure(
            !g.is_empty()
                && g.len() <= 48
                && g.as_bytes()[0].is_ascii_alphanumeric()
                && g.as_bytes().last().unwrap().is_ascii_alphanumeric()
                && g.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "generation syntax",
        )?;
    }
    let mut held = Vec::new();
    let mut schema_resources = Vec::new();
    for name in [
        "witness-structure-correspondence",
        "witness-structure-anchor-set",
        "source-anchor",
        "provenance-event",
    ] {
        let reference = format!("ToS/contracts/{name}.schema.json");
        let mut h = Held::open(ctx, &reference, CAP)?;
        schema_resources.push(tos_validation::SchemaResource {
            uri: format!("https://tree-of-sophia.local/{reference}"),
            raw: ctx.read_file(&mut h.file, CAP)?,
        });
        held.push(h)
    }
    let schemas = tos_validation::SchemaBackendProbe::new(
        schema_resources,
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
    )
    .map_err(|e| format!("schema preparation: {e:?}"))?;
    let validate = |name: &str, raw: &[u8]| -> Result<()> {
        ensure(
            schemas
                .is_valid_raw(
                    &format!("https://tree-of-sophia.local/ToS/contracts/{name}.schema.json"),
                    raw,
                )
                .map_err(|e| format!("schema: {e:?}"))?,
            &format!("{name} schema refused"),
        )
    };
    let mut prior = Held::open(ctx, PROVENANCE_PATH, CAP)?;
    let events_raw = ctx.read_file(&mut prior.file, CAP)?;
    let events = crate::transfer_target_passages::json_lines(ctx, &events_raw)?;
    ensure(
        events.len() == 2
            && events[0].1["event_id"] == EVENT_ID
            && events[1].1["event_id"] == ANCHOR_EVENT_ID,
        "retained provenance identities",
    )?;
    let source = ctx.select_directory(options.input_root.unwrap_or(ctx.root()))?;
    let mut epub = witness(
        ctx,
        &source,
        TARGET_EPUB_MANIFEST_REF,
        "application/epub+zip",
        "epub_resources_v1",
    )?;
    let pdf = witness(
        ctx,
        &source,
        TARGET_PDF_MANIFEST_REF,
        "application/pdf",
        "pdf_pages_v1",
    )?;
    let mut epub_resources = BTreeMap::new();
    for r in array(&epub.inventory["resources"])? {
        if let Some(member) = r["locator"]["member_path"].as_str() {
            ensure(
                epub_resources
                    .insert(member.to_string(), r.clone())
                    .is_none(),
                "duplicate EPUB inventory member",
            )?;
        }
    }
    let mut pdf_resources = BTreeMap::new();
    let mut pdf_pages = Vec::new();
    for r in array(&pdf.inventory["resources"])? {
        if r["resource_kind"] == "pdf_page" {
            pdf_pages.push(u(&r["locator"]["page_index"])?);
            ensure(
                pdf_resources
                    .insert(s(&r["resource_id"])?.to_string(), r.clone())
                    .is_none(),
                "duplicate PDF resource",
            )?;
        }
    }
    pdf_pages.sort();
    ensure(
        pdf_pages == (1..530).collect::<Vec<_>>(),
        "PDF enumeration drift",
    )?;
    let bytes = source.read_file(&mut epub.payload.file, 16 * 1024 * 1024)?;
    let archive = OfficeArchive::open(&bytes, &mut |n| ctx.tick(n))?;
    let mut page_names = BTreeMap::new();
    for name in archive.names()? {
        if let Some(x) = name
            .strip_prefix("EPUB/page_")
            .and_then(|s| s.strip_suffix(".html"))
        {
            if x.bytes().all(|c| c.is_ascii_digit()) && !x.is_empty() {
                let p = x
                    .parse::<usize>()
                    .map_err(|_| "EPUB page number overflow")?;
                ensure(
                    page_names.insert(p, name.to_string()).is_none(),
                    "duplicate EPUB page number",
                )?;
            }
        }
    }
    ensure(
        page_names.keys().copied().collect::<Vec<_>>() == (0..529).collect::<Vec<_>>(),
        "EPUB enumeration drift",
    )?;
    let mut pages = BTreeMap::new();
    for (p, name) in page_names {
        let raw = archive.read(&name, &mut |n| ctx.tick(n))?;
        let r = epub_resources
            .get(&name)
            .ok_or("EPUB member missing from inventory")?;
        ensure(r["sha256"] == sha(&raw), "EPUB member digest mismatch")?;
        let text = String::from_utf8_lossy(&raw);
        let mut parts = Vec::new();
        let mut hidden = 0usize;
        for event in crate::research_html::events(ctx, &text)? {
            match event {
                HtmlEvent::Start { tag, .. } if matches!(tag.as_str(), "script" | "style") => {
                    hidden += 1
                }
                HtmlEvent::End(tag) if matches!(tag.as_str(), "script" | "style") => {
                    hidden = hidden.saturating_sub(1)
                }
                HtmlEvent::Text(s) if hidden == 0 => parts.push(s),
                _ => {}
            }
        }
        pages.insert(p, tokens(&parts.join(" "))?);
    }
    drop(archive);
    drop(bytes);
    let mut d = Data {
        source_parts: Vec::new(),
        epub: binding(&epub, None)?,
        pdf: binding(&pdf, None)?,
        routes: Vec::new(),
        rows: Vec::new(),
        part_counts: Out::Null,
        mode_counts: Out::Null,
        inputs: Vec::new(),
        bindings: Vec::new(),
        anchors: Vec::new(),
        event_at: options.event_at.unwrap_or(s(&events[0].1["started_at"])?),
        map_digest: String::new(),
        set_digest: String::new(),
        anchor_digest: String::new(),
    };
    let mut witnesses = Vec::new();
    let parts: Vec<Value> = serde_json::from_slice(include_bytes!(
        "witness_structure_correspondence/parts.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut part_counts = Vec::new();
    let mut modes = BTreeMap::<String, u64>::new();
    for part in parts {
        let label = s(&part["part_label"])?;
        let mut w = witness(
            ctx,
            &source,
            s(&part["manifest_ref"])?,
            "application/xml",
            "tei_structure_v1",
        )?;
        let raw = source.read_file(&mut w.payload.file, CAP)?;
        let root = xml_with_doctype(ctx, &raw, false)?;
        let divisions = divisions(ctx, &root)?;
        d.source_parts.push(binding(&w, Some(label))?);
        d.inputs.push(input(
            s(&w.entry["file_id"])?,
            &format!(
                "fixity-verified-local-tei-part-{}",
                label.to_ascii_lowercase()
            ),
            &w.payload.digest,
        ));
        d.routes.push(o(vec![
            ("part_label", q(label)),
            ("source_item_ref", v(&w.manifest["item_id"])?),
            ("selection_policy", v(&part["selection_policy"])?),
            (
                "target_epub_member_page_range",
                o(vec![
                    ("first", v(&part["first_target_page"])?),
                    ("last", v(&part["last_target_page"])?),
                ]),
            ),
        ]));
        let mut sequence = 0usize;
        let mut previous = 0;
        for resource in array(&w.inventory["resources"])? {
            let Some(path) = resource["locator"]["tei_path"].as_str() else {
                continue;
            };
            let Some(division) = divisions.get(path) else {
                continue;
            };
            if !selected(label, resource, &division.heading) {
                continue;
            }
            sequence += 1;
            let (page, r) = row(
                ctx,
                &part,
                &w,
                resource,
                division,
                &epub,
                &pdf,
                &pages,
                &epub_resources,
                &pdf_resources,
                sequence,
            )?;
            ensure(page >= previous, "nonmonotonic correspondence candidates")?;
            previous = page;
            let parsed = value(&r)?;
            *modes
                .entry(s(&parsed["match"]["mode"])?.to_string())
                .or_default() += 1;
            d.rows.push(r)
        }
        ensure(sequence > 0, "no named structural divisions")?;
        part_counts.push((label.to_string(), n(sequence as u64)));
        witnesses.push(w);
    }
    d.part_counts = Out::Object(part_counts);
    d.mode_counts = Out::Object(modes.into_iter().map(|(k, n_)| (k, n(n_))).collect());
    d.inputs.push(input(
        s(&epub.entry["file_id"])?,
        "fixity-verified-local-naumann-1893-epub",
        &epub.payload.digest,
    ));
    d.inputs.push(input(
        s(&pdf.entry["file_id"])?,
        "fixity-verified-local-naumann-1893-image-pdf",
        &pdf.payload.digest,
    ));
    witnesses.extend([epub, pdf]);
    anchors(&mut d)?;
    let mut paths = BTreeMap::new();
    for old in [
        OUTPUT_PATH,
        ANCHOR_SET_PATH,
        ANCHOR_RECORDS_PATH,
        PROVENANCE_PATH,
    ] {
        let (parent, file) = old.rsplit_once('/').ok_or("output path")?;
        paths.insert(
            old,
            options.generation.map_or_else(
                || old.to_string(),
                |g| format!("{parent}/native-{g}/{file}"),
            ),
        );
    }
    let mut map = records::map(&d)?;
    if let Some(g) = options.generation {
        rewrite(&mut map, g, &paths);
        set(&mut map, "map_version", n(2))?;
        set(&mut map, "supersedes_map_ref", q(OUTPUT_PATH))?;
        for x in d.anchors.iter_mut().chain(d.bindings.iter_mut()) {
            rewrite(x, g, &paths)
        }
    } else {
        set(
            &mut map,
            "authority_boundary",
            q(
                "named structural starts and locator candidates only; no source text, textual identity, edition equivalence, accepted German, translation, semantics, or canon authority",
            ),
        )?;
    }
    let map_raw = encode(ctx, &map, true)?;
    d.map_digest = sha(&map_raw);
    let anchor_raw = lines(ctx, &d.anchors)?;
    d.anchor_digest = sha(&anchor_raw);
    let mut aset = records::anchor_set(&d)?;
    if let Some(g) = options.generation {
        rewrite(&mut aset, g, &paths);
        set(&mut aset, "anchor_set_version", n(2))?;
        set(&mut aset, "supersedes_anchor_set_ref", q(ANCHOR_SET_PATH))?;
    } else {
        set(
            &mut aset,
            "authority_boundary",
            q(
                "stable proposed addresses for named structural-start candidates only; no source text, exact passage boundary, textual identity, edition equivalence, accepted German, translation, semantics, rights clearance, or canon authority",
            ),
        )?;
    }
    let set_raw = encode(ctx, &aset, true)?;
    d.set_digest = sha(&set_raw);
    let mut map_event = records::map_event(&d)?;
    d.event_at = options.event_at.unwrap_or(s(&events[1].1["started_at"])?);
    let mut anchor_event = records::anchor_event(&d)?;
    if let Some(g) = options.generation {
        let mut actual_inputs = Vec::new();
        for (reference, raw) in [
            (
                BUILDER,
                include_bytes!("witness_structure_correspondence.rs").as_slice(),
            ),
            (
                "rust/crates/tos-compiler/src/witness_structure_correspondence/records.rs",
                include_bytes!("witness_structure_correspondence/records.rs").as_slice(),
            ),
            (
                "rust/crates/tos-compiler/src/witness_structure_correspondence/constants.rs",
                include_bytes!("witness_structure_correspondence/constants.rs").as_slice(),
            ),
            (
                "rust/crates/tos-compiler/src/witness_structure_correspondence/parts.json",
                include_bytes!("witness_structure_correspondence/parts.json").as_slice(),
            ),
        ] {
            actual_inputs.push(input(
                reference,
                "native-structural-producer-source",
                &sha(raw),
            ));
        }
        actual_inputs.push(input(
            PROVENANCE_PATH,
            "retained-structural-provenance",
            &prior.digest,
        ));
        for w in &witnesses {
            for h in &w.held {
                actual_inputs.push(input(
                    &h.reference,
                    "fixity-bound-witness-metadata",
                    &h.digest,
                ))
            }
        }
        let builder = actual_inputs[0].clone();
        for e in [&mut map_event, &mut anchor_event] {
            rewrite(e, g, &paths);
            set(
                e,
                "agent_refs",
                Out::Array(vec![q("software:tos-native-rust")]),
            )?;
            set(e, "event_version", n(2))?;
            let method = field(e, "method")?;
            set(
                method,
                "runtime",
                q("Rust quick-xml, bounded ZIP and HTML5 character-reference readers"),
            )?;
            set(method, "artifact_digest", v(&value(&builder)?["sha256"])?)?;
            if let Out::Array(xs) = field(e, "inputs")? {
                xs.extend(actual_inputs.clone())
            }
        }
    } else {
        for (e, warning) in [
            (
                &mut map_event,
                "No German correctness, translation, semantic, or canon conclusion was produced.",
            ),
            (
                &mut anchor_event,
                "No rights clearance, German correctness, translation, semantic, or canon conclusion was produced.",
            ),
        ] {
            let Out::Array(ws) = field(e, "warnings")? else {
                return Err("warnings array".into());
            };
            ws[2] = q(warning);
        }
    }
    let provenance_raw = lines(ctx, &[map_event.clone(), anchor_event.clone()])?;
    validate("witness-structure-correspondence", &map_raw)?;
    validate("witness-structure-anchor-set", &set_raw)?;
    for a in &d.anchors {
        validate("source-anchor", &encode(ctx, a, false)?)?;
    }
    for e in [&map_event, &anchor_event] {
        validate("provenance-event", &encode(ctx, e, false)?)?;
    }
    held.push(prior);
    for h in &mut held {
        h.verify(ctx)?;
    }
    for w in &mut witnesses {
        w.verify(ctx, &source)?;
    }
    let outputs = [
        (paths[OUTPUT_PATH].clone(), map_raw),
        (paths[ANCHOR_SET_PATH].clone(), set_raw),
        (paths[ANCHOR_RECORDS_PATH].clone(), anchor_raw),
        (paths[PROVENANCE_PATH].clone(), provenance_raw),
    ];
    let mut missing = Vec::new();
    for (reference, bytes) in &outputs {
        match std::fs::symlink_metadata(ctx.root().join(reference)) {
            Ok(_) => {
                let retained = ctx.read(reference)?;
                ensure(
                    retained == *bytes,
                    &format!(
                        "structural correspondence differs: {reference}; expected_sha256={} retained_sha256={}",
                        sha(bytes),
                        sha(&retained)
                    ),
                )?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ensure(options.build, &format!("output missing: {reference}"))?;
                missing.push(reference.as_str());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    for (reference, bytes) in &outputs {
        if missing.contains(&reference.as_str()) {
            ctx.write(reference, bytes, 0o644, true)?;
        }
    }
    Ok(
        json!({"status":if options.build{"built"}else{"verified"},"generation":options.generation,"outputs_written":missing.len(),"correspondences":d.rows.len(),"anchors":d.anchors.len(),"scope":"text-free locator candidates; source-visible review, rights, translation and canon remain separate","outputs":outputs.iter().map(|(p,b)|json!({"ref":p,"sha256":sha(b),"bytes":b.len()})).collect::<Vec<_>>()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ts(s: &str) -> Vec<String> {
        tokens(s).unwrap()
    }
    #[test]
    fn heading_uniqueness_and_context_choose_deterministic_pages() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(dir.path(), 30).unwrap();
        let pages = BTreeMap::from([
            (10, ts("unrelated page")),
            (11, ts("named division source context")),
            (12, ts("following page")),
        ]);
        let (page, r) = choose(
            &ctx,
            &ts("named division"),
            &ts("named division source context"),
            &pages,
            10,
            12,
        )
        .unwrap();
        assert_eq!(page, 11);
        let r = value(&r).unwrap();
        assert_eq!(r["mode"], "normalized_heading_unique");
        assert_eq!(r["normalized_heading_occurrence_count"], 1);
        let pages = BTreeMap::from([
            (20, ts("repeated heading other material")),
            (21, ts("other continuation")),
            (30, ts("repeated heading distinctive matching context")),
            (31, ts("matching continuation")),
        ]);
        let (page, r) = choose(
            &ctx,
            &ts("repeated heading"),
            &ts("repeated heading distinctive matching context"),
            &pages,
            20,
            31,
        )
        .unwrap();
        let r = value(&r).unwrap();
        assert_eq!(page, 30);
        assert_eq!(r["mode"], "normalized_heading_context_disambiguated");
        assert!(r["score_margin"].as_f64().unwrap() > 0.0);
        let pages = BTreeMap::from([(1, ts("same heading")), (2, ts("same heading"))]);
        assert_eq!(
            choose(&ctx, &ts("same heading"), &[], &pages, 1, 2)
                .unwrap()
                .0,
            1
        );
        assert_eq!(
            ts("ＡＢＣ Ä Ö Ü ß ſ É Москва"),
            vec!["abc", "ä", "ö", "ü", "ss", "s"]
        );
    }
    #[test]
    fn tei_selection_preserves_primary_division_and_text_boundaries() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(dir.path(), 30).unwrap();
        let root=xml_with_doctype(&ctx,b"<TEI><text><body><div><head>Word<hi>End</hi></head><p>Some<hi>Word</hi> tail</p></div></body></text></TEI>",false).unwrap();
        let ds = divisions(&ctx, &root).unwrap();
        let d = &ds["TEI/text[1]/body[1]/div[1]"];
        assert_eq!(d.heading, ts("WordEnd"));
        assert_eq!(d.content, ts("Word End Some Word tail"));
        let mut r = json!({"resource_kind":"tei_division","structural_role":"division","locator":{"tei_path":"TEI/text[1]/body[1]/div[3]","tei_depth":1}});
        assert!(selected("III", &r, &ts("named")));
        r["locator"] = json!({"tei_path":"TEI/text[1]/body[1]/div[3]/div[1]","tei_depth":2});
        assert!(!selected("III", &r, &ts("named")));
        r["locator"]["tei_path"] = json!("TEI/text[1]/body[1]/div[2]/div[1]");
        assert!(selected("I", &r, &ts("named")));
        r["locator"] = json!({"tei_path":"TEI/text[1]/body[1]/div[21]","tei_depth":1});
        assert!(!selected("IV", &r, &ts("named")));
    }
    #[test]
    fn ordered_floats_and_native_references_are_stable() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(dir.path(), 30).unwrap();
        assert_eq!(
            encode(
                &ctx,
                &o(vec![
                    ("score", Out::Float(0.000001)),
                    ("one", Out::Float(1.0))
                ]),
                false
            )
            .unwrap(),
            b"{\"score\":1e-06,\"one\":1.0}"
        );
        let mut x = q("tos.anchor.zarathustra-structure.dta-i-001");
        rewrite(&mut x, "revision-1", &BTreeMap::new());
        rewrite(&mut x, "revision-1", &BTreeMap::new());
        assert_eq!(
            value(&x).unwrap(),
            "tos.anchor.zarathustra-structure.dta-i-001.native-revision-1"
        );
    }
}
