//! Exact, text-free three-witness comparison. Retained acquisition and review
//! evidence remain historical; a fresh comparison receives its own identity.
use crate::{
    research_execution::ResearchExecution,
    research_html::{HtmlEvent, events},
    research_text_comparison::{alpha, opcodes, space},
    source_philosophy_dossier_docx::OfficeArchive,
    source_text_foundation::{
        Node, ensure, fresh_or_matching_limit, load, private_boundary, s, schema, sha,
        xml_with_doctype,
    },
    transfer_target_passages::{encode, json_lines, jsonl, node_text, read_optional},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
use unicode_normalization::UnicodeNormalization;
type Result<T> = std::result::Result<T, String>;
const GOLD: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1";
const DISCOVERY: &str =
    "ToS/source-witnesses/discovery/runs/ekgwb-za-i-vorrede-1-http-fallback.2026-07-29.v2.json";
const SCHEMA: &str = "ToS/contracts/german-source-triangulation.schema.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/german_triangulation.rs";
const REFERENCE: &str = "6d3deb76b489989f2a8ed3782f3ae9c12914a0ab6baea1b9f21432bcc8749e16";
const CAP: usize = 4 * 1024 * 1024;
fn tokens(text: &str) -> Result<Vec<String>> {
    ensure(text.len() <= CAP, "normalization byte bound")?;
    let normalized = text.nfkc().collect::<String>();
    let folded =
        tos_foundation::python_casefold_unicode16_v1(&normalized, CAP * 4, CAP * 12, CAP * 12)
            .map_err(|e| e.to_string())?;
    Ok(folded
        .split(|c| !alpha(c))
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect())
}
fn dehyphenate(text: &str) -> String {
    let mut skip_space = false;
    text.chars()
        .filter(|&c| {
            if c == '¬' {
                skip_space = true;
                false
            } else if skip_space && space(c) {
                false
            } else {
                skip_space = false;
                true
            }
        })
        .collect()
}
fn ekgwb(ctx: &ResearchExecution, text: &str) -> Result<Vec<Vec<String>>> {
    let start = text
        .find("id=\"eKGWB/Za-I-Vorrede-1\"")
        .ok_or("eKGWB target element absent")?;
    let end = start
        + text[start..]
            .find("<a name=\"eKGWB/Za-I-Vorrede-2\"")
            .ok_or("eKGWB next section absent")?;
    let (mut depth, mut capture) = (0usize, None);
    let mut current = String::new();
    let mut paragraphs = vec![];
    for event in events(ctx, &text[start..end])? {
        match event {
            HtmlEvent::Start { tag, attrs, .. } if tag == "div" => {
                depth += 1;
                ensure(depth <= 256, "HTML div depth")?;
                if capture.is_none()
                    && attrs
                        .get("class")
                        .is_some_and(|s| s.split(space).any(|c| c == "p"))
                {
                    capture = Some(depth);
                    current.clear();
                }
            }
            HtmlEvent::End(tag) if tag == "div" => {
                if capture == Some(depth) {
                    let t = tokens(&current)?;
                    if t.len() > 1 {
                        paragraphs.push(t);
                    }
                    capture = None;
                    current.clear();
                }
                depth = depth.saturating_sub(1);
            }
            HtmlEvent::Text(text) if capture.is_some() => current.push_str(&text),
            _ => (),
        }
    }
    ensure(paragraphs.len() == 12, "eKGWB paragraph count drift")?;
    Ok(paragraphs)
}
fn first<'a>(node: &'a Node, name: &str) -> Result<&'a Node> {
    node.children(name)
        .first()
        .copied()
        .ok_or_else(|| format!("TEI {name} absent"))
}
fn dta(ctx: &ResearchExecution, raw: &[u8]) -> Result<(Vec<Vec<String>>, usize)> {
    let root = xml_with_doctype(ctx, raw, false)?;
    ensure(
        root.name == "TEI"
            && root
                .attrs
                .get("xmlns")
                .is_some_and(|v| v == "http://www.tei-c.org/ns/1.0"),
        "DTA TEI namespace",
    )?;
    let section = first(first(first(first(&root, "text")?, "body")?, "div")?, "div")?;
    let mut paragraphs = vec![];
    let mut raw_count = 0;
    for p in section.children("p") {
        let raw = node_text(p);
        raw_count += tokens(&raw)?.len();
        let t = tokens(&dehyphenate(&raw))?;
        if t.len() > 1 {
            paragraphs.push(t);
        }
    }
    ensure(paragraphs.len() == 12, "DTA paragraph count drift")?;
    Ok((paragraphs, raw_count))
}
fn body(ctx: &ResearchExecution, text: &str) -> Result<Vec<String>> {
    let (mut depth, mut suppressed) = (0usize, 0usize);
    let mut parts = vec![];
    for event in events(ctx, text)? {
        match event {
            HtmlEvent::Start { tag, .. } if tag == "body" => depth += 1,
            HtmlEvent::Start { tag, .. }
                if depth > 0 && matches!(tag.as_str(), "script" | "style") =>
            {
                suppressed += 1
            }
            HtmlEvent::End(tag) if depth > 0 && matches!(tag.as_str(), "script" | "style") => {
                suppressed = suppressed.saturating_sub(1)
            }
            HtmlEvent::End(tag) if tag == "body" => depth = depth.saturating_sub(1),
            HtmlEvent::Text(text) if depth > 0 && suppressed == 0 => parts.push(text),
            _ => (),
        }
    }
    tokens(&parts.join(" "))
}
fn comparison(
    ctx: &ResearchExecution,
    reference: &[String],
    candidate: &[String],
) -> Result<Value> {
    let ops = opcodes(ctx, reference, candidate)?;
    let actual = ops
        .iter()
        .map(|o| (o.tag, o.i1, o.i2, o.j1, o.j2))
        .collect::<Vec<_>>();
    ensure(
        actual
            == [
                ("equal", 0, 111, 0, 111),
                ("replace", 111, 112, 111, 112),
                ("equal", 112, 165, 112, 165),
                ("insert", 165, 165, 165, 180),
                ("equal", 165, 261, 180, 276),
                ("insert", 261, 261, 276, 392),
            ],
        "Naumann comparison shape drift",
    )?;
    Ok(
        json!({"candidate_tokens":candidate.len(),"equal_reference_tokens":ops.iter().filter(|o| o.tag=="equal").map(|o| o.i2-o.i1).sum::<usize>(),"single_token_replacements":ops.iter().filter(|o| o.tag=="replace" && o.i2-o.i1==1 && o.j2-o.j1==1).count(),"page_furniture_insertions":15,"trailing_next_section_tokens":116,"equal_run_lengths":ops.iter().filter(|o| o.tag=="equal").map(|o| o.i2-o.i1).collect::<Vec<_>>(),"exact_textual_identity":false,"translation_claimed":false}),
    )
}
fn builder_sha() -> String {
    let mut h = tos_foundation::Digest256Hasher::new();
    for raw in [
        include_bytes!("german_triangulation.rs").as_slice(),
        include_bytes!("german_triangulation/templates.json").as_slice(),
        include_bytes!("research_html.rs").as_slice(),
        include_bytes!("research_html_entities.rs").as_slice(),
        include_bytes!("research_text_comparison.rs").as_slice(),
        include_bytes!("source_philosophy_dossier_docx.rs").as_slice(),
        include_bytes!("source_text_foundation.rs").as_slice(),
        include_bytes!("transfer_target_passages.rs").as_slice(),
    ] {
        h.update(&(raw.len() as u64).to_be_bytes());
        h.update(raw);
    }
    h.finalize().to_hex()
}
fn bindings(ctx: &ResearchExecution, p: &mut Value) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut inputs = BTreeMap::new();
    for key in [
        "assisted_review_plan",
        "source_review_plan",
        "metadata_critical_witness_packet",
    ] {
        let b = &mut p["bindings"][key];
        let r = s(&b["ref"])?.to_owned();
        let raw = ctx.read(&r)?;
        b["sha256"] = json!(sha(&raw));
        inputs.insert(r, raw);
    }
    for key in ["dta_tei", "naumann_auto_epub"] {
        let w = &mut p["inputs"][key];
        let r = s(&w["item_manifest"]["ref"])?.to_owned();
        let (raw, manifest) = load(ctx, &r)?;
        w["item_manifest"]["sha256"] = json!(sha(&raw));
        inputs.insert(r.clone(), raw);
        let item = r
            .strip_suffix("/item.manifest.json")
            .ok_or("manifest path")?;
        let relative = s(&w["payload_relative_path"])?
            .strip_prefix(
                item.strip_prefix("ToS/source-witnesses/")
                    .ok_or("item root")?,
            )
            .and_then(|p| p.strip_prefix('/'))
            .ok_or("payload outside item")?;
        let rows = manifest["payload_files"]
            .as_array()
            .ok_or("payload entries")?
            .iter()
            .filter(|e| e["relative_path"] == relative)
            .collect::<Vec<_>>();
        ensure(
            rows.len() == 1 && rows[0]["sha256"] == w["file_sha256"],
            "manifest payload fixity drift",
        )?;
        w["item_ref"] = manifest["item_id"].clone();
        w["file_ref"] = rows[0]["file_id"].clone();
        let rr = s(&w["rights_record"]["ref"])?.to_owned();
        let raw = ctx.read(&rr)?;
        w["rights_record"]["sha256"] = json!(sha(&raw));
        inputs.insert(rr, raw);
    }
    Ok(inputs)
}
#[derive(Clone, Copy)]
pub enum Action {
    Build,
    Check,
    ValidateTracked,
}
pub struct Options<'a> {
    pub action: Action,
    pub input_root: Option<&'a Path>,
    pub generation: Option<&'a str>,
    pub prepared_at: Option<&'a str>,
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let generation = opts.generation.unwrap_or("v1");
    ensure(
        regex::Regex::new(r"^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$")
            .unwrap()
            .is_match(generation),
        "generation component",
    )?;
    let historical = generation == "v1";
    let templates: Value =
        serde_json::from_str(include_str!("german_triangulation/templates.json"))
            .map_err(|e| e.to_string())?;
    let mut packet = templates["packet"].clone();
    let output = format!(
        "{GOLD}/german-source-triangulation.ekgwb-dta-naumann.za-i-vorrede-1.{generation}.json"
    );
    let journal = if historical {
        format!("{GOLD}/provenance.german-source-triangulation.jsonl")
    } else {
        format!("{GOLD}/provenance.german-source-triangulation.{generation}.jsonl")
    };
    let frozen = read_optional(ctx, &output)?;
    if historical {
        ensure(
            opts.prepared_at
                .is_none_or(|v| Some(v) == packet["prepared_at"].as_str()),
            "historical timestamp is immutable",
        )?;
    } else {
        let at = opts
            .prepared_at
            .map(str::to_owned)
            .or_else(|| {
                frozen
                    .as_ref()
                    .and_then(|b| serde_json::from_slice::<Value>(b).ok())
                    .and_then(|v| v["prepared_at"].as_str().map(str::to_owned))
            })
            .ok_or("fresh generation requires prepared-at timestamp")?;
        packet["prepared_at"] = json!(at);
        packet["packet_id"] = json!(format!(
            "tos.german-source-triangulation.ekgwb-dta-naumann.za-i-vorrede-1.{generation}"
        ));
        packet["method"]["implementation"] = json!("rust");
        packet["method"]["comparison"] = json!("sequence_matcher_autojunk_false");
    }
    let inputs = bindings(ctx, &mut packet)?;
    schema(ctx, SCHEMA, &packet)?;
    let discovery = encode(ctx, &templates["discovery"], true)?;
    ensure(
        ctx.read(DISCOVERY)? == discovery,
        "retained discovery evidence changed",
    )?;
    let mut event = templates["alignment_event"].clone();
    if !historical {
        event["event_id"] = json!(format!(
            "tos.event.alignment.zarathustra-german-source-triangulation.za-i-vorrede-1.{generation}"
        ));
        event["started_at"] = packet["prepared_at"].clone();
        event["ended_at"] = packet["prepared_at"].clone();
        event["agent_refs"] = json!(["software:tos-compiler"]);
        event["method"]["maker_type"] = json!("software");
        event["method"]["artifact_digest"] = json!(builder_sha());
        event["method"]["runtime"] = json!("Rust tos-compiler");
        event["method"]["configuration"]["comparison"] = json!("SequenceMatcher(autojunk=False)");
        event["method"]["configuration"]["native_builder_ref"] = json!(BUILDER);
    }
    if !matches!(opts.action, Action::ValidateTracked) {
        let local = ctx.select_directory(
            opts.input_root
                .ok_or("explicit local input root required")?,
        )?;
        let mut held = vec![];
        let mut raw = vec![];
        for key in ["ekgwb", "dta_tei", "naumann_auto_epub"] {
            let w = &packet["inputs"][key];
            let (reference, digest) = if key == "ekgwb" {
                (
                    s(&w["local_payload_ref"])?.to_string(),
                    s(&w["payload_sha256"])?.to_owned(),
                )
            } else {
                (
                    format!("ToS/source-witnesses/{}", s(&w["payload_relative_path"])?),
                    s(&w["file_sha256"])?.to_owned(),
                )
            };
            private_boundary(ctx, &reference)?;
            let mut file = local.source_file(&reference, 64 * 1024 * 1024)?;
            let bytes = local.read_file(&mut file, 64 * 1024 * 1024)?;
            ensure(sha(&bytes) == digest, "local payload fixity drift")?;
            held.push((file, digest));
            raw.push(bytes);
        }
        ensure(raw[0].len() == 180138, "eKGWB response byte size drift")?;
        let ep = ekgwb(ctx, std::str::from_utf8(&raw[0]).map_err(|_| "eKGWB UTF8")?)?;
        let reference = ep.iter().flatten().cloned().collect::<Vec<_>>();
        ensure(
            reference.len() == 261 && sha(reference.join(" ").as_bytes()) == REFERENCE,
            "eKGWB normalized sequence drift",
        )?;
        let (dp, naive) = dta(ctx, &raw[1])?;
        let target = dp.iter().flatten().cloned().collect::<Vec<_>>();
        ensure(
            dp == ep && target == reference && naive.checked_sub(target.len()) == Some(3),
            "DTA source-aware comparison or false-split control drift",
        )?;
        let archive = OfficeArchive::open(&raw[2], &mut |n| ctx.tick(n))?;
        let mut candidate = vec![];
        for member in packet["results"]["naumann_ocr_comparison"]["epub_members"]
            .as_array()
            .ok_or("member references")?
        {
            let bytes = archive.read(s(&member["path"])?, &mut |n| ctx.tick(n))?;
            ensure(
                sha(&bytes) == s(&member["sha256"])?,
                "Naumann member digest drift",
            )?;
            candidate.extend(body(
                ctx,
                std::str::from_utf8(&bytes).map_err(|_| "member UTF8")?,
            )?);
        }
        let metrics = comparison(ctx, &reference, &candidate)?;
        for (k, v) in metrics.as_object().unwrap() {
            ensure(
                packet["results"]["naumann_ocr_comparison"][k] == *v,
                "Naumann aggregate metric drift",
            )?;
        }
        for (file, expected) in &mut held {
            ensure(
                local.hash_file(file, 64 * 1024 * 1024)? == *expected,
                "payload changed during comparison",
            )?;
        }
    } else {
        ensure(
            opts.input_root.is_none(),
            "tracked validation reads no private payload",
        )?;
    }
    let encoded = encode(ctx, &packet, true)?;
    let journal_bytes = if historical {
        let raw = ctx.read(&journal)?;
        ensure(
            sha(&raw) == s(&templates["historical_provenance_sha256"])?,
            "retained provenance history changed",
        )?;
        let rows = json_lines(ctx, &raw)?;
        for (_, e) in rows {
            schema(ctx, "ToS/contracts/provenance-event.schema.json", &e)?;
        }
        raw
    } else {
        event["inputs"][1]["ref"] = packet["inputs"]["dta_tei"]["file_ref"].clone();
        event["inputs"][2]["ref"] = packet["inputs"]["naumann_auto_epub"]["file_ref"].clone();
        event["inputs"][3]["sha256"] = packet["bindings"]["assisted_review_plan"]["sha256"].clone();
        event["outputs"][0]["ref"] = json!(output);
        event["outputs"][0]["sha256"] = json!(sha(&encoded));
        schema(ctx, "ToS/contracts/provenance-event.schema.json", &event)?;
        jsonl(ctx, &[event])?
    };
    for (r, raw) in inputs {
        ensure(
            ctx.read(&r)? == raw,
            "metadata or rights changed before output",
        )?;
    }
    let pending = [(&output, &encoded), (&journal, &journal_bytes)]
        .into_iter()
        .map(|(r, b)| Ok((r, b, fresh_or_matching_limit(ctx, r, b, CAP)?)))
        .collect::<Result<Vec<_>>>()?;
    ensure(
        matches!(opts.action, Action::Build) || pending.iter().all(|(_, _, missing)| !missing),
        "tracked output absent",
    )?;
    let mut written = vec![];
    for (r, b, missing) in pending {
        if missing {
            ctx.write(r, b, 0o644, true)?;
            written.push(r.clone());
        }
    }
    Ok(
        json!({"status":"passed","packet":output,"sha256":sha(&encoded),"provenance":journal,"historical_replay":historical,"private_payloads_checked":!matches!(opts.action,Action::ValidateTracked),"written":written,"source_text_emitted":false,"source_admission_performed":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nfkc_casefold_dta_marker_and_visible_body() {
        assert_eq!(
            tokens("Straße STRAẞE ﬃ ² Ａ Ä").unwrap(),
            vec!["strasse", "strasse", "ffi", "a", "ä"]
        );
        assert_eq!(dehyphenate("Wort¬\r\n ende noch¬\tmal"), "Wortende nochmal");
        let t = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(t.path(), 30).unwrap();
        assert_eq!(body(&ctx,"<head>excluded</head><body><p>Ä &amp; B</p><script>x</script><style>y</style>ß</body>tail").unwrap(),vec!["ä","b","ss"]);
    }
}
