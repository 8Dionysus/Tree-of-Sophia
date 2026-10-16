//! Exact private Poppler page extraction and text-free source foundation records.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{
        LEGACY_LAYER_AUTHORITY, Node, Part, encode, ensure, fresh_or_matching, load, metadata,
        private_boundary, s, schema, sha, utc_now, validate_event, xml_with_doctype,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::Path,
};
#[path = "target_text_foundation/records.rs"]
mod records;
type Result<T> = std::result::Result<T, String>;
pub use crate::source_text_foundation::Options;
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-antonovsky-1911-target-text-foundation.plan.v1.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/target_text_foundation.rs";
const LEGACY_BUILDER: &str = "scripts/build_zarathustra_target_text_foundation.py";
const LEGACY_EVENT: &str =
    "tos.event.native-extraction.za-i-vorrede-1.antonovsky-1911-pdf-page-6-opening.2026-08-12";
const LEGACY_UNIT_REASON: &str = "The unit records only one exact Poppler bbox line or its line-break code point in the unreviewed embedded-text layer; it is not accepted Russian, a linguistic unit, or a translation alignment.";
const CAP: usize = 4 * 1024 * 1024;
const PDF_CAP: u64 = 128 * 1024 * 1024;
fn n(v: &Value) -> Result<usize> {
    v.as_u64()
        .and_then(|v| v.try_into().ok())
        .ok_or("required nonnegative integer".into())
}
fn descendants<'a>(node: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    if node.name == name {
        out.push(node);
    }
    for child in &node.content {
        if let Part::Child(child) = child {
            descendants(child, name, out);
        }
    }
}
fn word(node: &Node) -> Result<&str> {
    ensure(
        node.name == "word" && node.content.len() == 1,
        "invalid bbox word structure",
    )?;
    match &node.content[0] {
        Part::Text(t) if !t.is_empty() => Ok(t),
        _ => Err("bbox word is empty or nested".into()),
    }
}
fn number(node: &Node, key: &str) -> Result<f64> {
    let v = node
        .attrs
        .get(key)
        .ok_or("bbox numeric attribute absent")?
        .parse::<f64>()
        .map_err(|e| e.to_string())?;
    ensure(v.is_finite(), "bbox coordinate is nonfinite")?;
    Ok(v)
}
fn from_bbox(ctx: &ResearchExecution, raw: &[u8], plan: &Value) -> Result<String> {
    // Poppler's XHTML DOCTYPE is data. The bounded parser never fetches a DTD
    // or resolves external/general entities; predefined/numeric entities only.
    let root = xml_with_doctype(ctx, raw, true)?;
    let selector = &plan["selector"];
    let mut pages = vec![];
    descendants(&root, "page", &mut pages);
    ensure(pages.len() == 1, "expected one bbox page")?;
    let page = pages[0];
    ensure(
        number(page, "width")? == selector["page_width_points"].as_f64().ok_or("page width")?
            && number(page, "height")?
                == selector["page_height_points"]
                    .as_f64()
                    .ok_or("page height")?,
        "bbox page geometry drift",
    )?;
    let mut blocks = vec![];
    descendants(page, "block", &mut blocks);
    let mut selected = vec![];
    for block in blocks {
        ctx.tick(1)?;
        let lines = block.children("line");
        if let Some(line) = lines.first() {
            if let Some(first) = line.children("word").first() {
                if sha(word(first)?.as_bytes()) == s(&selector["first_word_sha256"])? {
                    selected.push(block);
                }
            }
        }
    }
    ensure(
        selected.len() == 1 && n(&selector["block_match_count"])? == 1,
        "opening bbox block match drift",
    )?;
    let lines = selected[0].children("line");
    let start = n(&selector["selected_line_start"])?;
    let count = n(&selector["selected_line_count"])?;
    let end = start.checked_add(count).ok_or("line window overflow")?;
    ensure(
        count == 6 && end < lines.len(),
        "six-line window requires following boundary guard",
    )?;
    let guard = lines[end].children("word");
    let guard = guard.first().ok_or("next-line word absent")?;
    ensure(
        sha(word(guard)?.as_bytes()) == s(&selector["next_line_first_word_sha256"])?,
        "next-line boundary drift",
    )?;
    let lines = &lines[start..end];
    let mut rendered = vec![];
    let mut words = vec![];
    for line in lines {
        let row = line.children("word");
        ensure(!row.is_empty(), "empty bbox line")?;
        let texts = row.iter().map(|w| word(w)).collect::<Result<Vec<_>>>()?;
        rendered.push(texts.join(" "));
        words.extend(texts);
    }
    ensure(
        words.len() == n(&selector["expected_word_count"])?
            && sha(words.last().ok_or("words absent")?.as_bytes())
                == s(&selector["last_word_sha256"])?,
        "bbox word count/end guard drift",
    )?;
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for line in lines {
        bounds[0] = bounds[0].min(number(line, "xMin")?);
        bounds[1] = bounds[1].min(number(line, "yMin")?);
        bounds[2] = bounds[2].max(number(line, "xMax")?);
        bounds[3] = bounds[3].max(number(line, "yMax")?);
    }
    let region = &selector["region_points"];
    let x = region["x"].as_f64().ok_or("region x")?;
    let y = region["y"].as_f64().ok_or("region y")?;
    let expected = [
        x,
        y,
        x + region["width"].as_f64().ok_or("region width")?,
        y + region["height"].as_f64().ok_or("region height")?,
    ];
    ensure(
        bounds
            .iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() <= 0.000001),
        "bbox region drift",
    )?;
    let text = rendered.join("\n");
    let policy = &plan["extraction_policy"];
    ensure(
        !text.contains('\r')
            && !text.ends_with('\n')
            && text.len() == n(&policy["expected_content_bytes"])?
            && text.chars().count() == n(&policy["expected_content_codepoints"])?
            && sha(text.as_bytes()) == s(&policy["expected_content_sha256"])?,
        "target text fixity or line endings drift",
    )?;
    Ok(text)
}
struct Extraction {
    bbox: Vec<u8>,
    text: String,
    backend_digest: String,
    backend_version: String,
}
fn extract(ctx: &ResearchExecution, pdf: &File, plan: &Value) -> Result<Extraction> {
    use crate::owned_native_child::{CaptureLimits, capture_with_cancel};
    let binary = std::fs::canonicalize("/usr/bin/pdftotext").map_err(|e| e.to_string())?;
    let binary_root = ctx.select_directory(binary.parent().ok_or("Poppler parent")?)?;
    let name = binary
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("Poppler filename")?;
    let mut executable = binary_root.source_file(name, 32 * 1024 * 1024)?;
    let backend_digest = binary_root.hash_file(&mut executable, 32 * 1024 * 1024)?;
    // Execute the held exact inode, and pass the held PDF without reopening its pathname.
    let command_path = format!("/proc/{}/fd/{}", std::process::id(), executable.as_raw_fd());
    let version = capture_with_cancel(
        std::process::Command::new(&command_path).arg("-v"),
        None,
        CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: 16384,
            max_stderr_bytes: 16384,
        },
        ctx.deadline(),
        ctx.cancellation_flag(),
    )?;
    let mut bytes = version.stdout;
    bytes.extend(version.stderr);
    let version_text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let backend_version = s(&plan["extraction_policy"]["software_version"])?.to_owned();
    ensure(
        version.status.success()
            && version_text.lines().next()
                == Some(format!("pdftotext version {backend_version}").as_str()),
        "Poppler version drift",
    )?;
    let page = n(&plan["selector"]["page_number"])?;
    ensure((1..=100000).contains(&page), "page range")?;
    let carrier = format!("/proc/{}/fd/{}", std::process::id(), pdf.as_raw_fd());
    let result = capture_with_cancel(
        std::process::Command::new(&command_path).args([
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
            "-bbox-layout",
            &carrier,
            "-",
        ]),
        None,
        CaptureLimits {
            max_stdin_bytes: 0,
            max_stdout_bytes: CAP,
            max_stderr_bytes: 16384,
        },
        ctx.deadline(),
        ctx.cancellation_flag(),
    )?;
    ensure(result.status.success(), "Poppler bbox extraction failed")?;
    let policy = &plan["extraction_policy"];
    let bbox = result.stdout;
    ensure(
        bbox.len() == n(&policy["expected_bbox_bytes"])?
            && sha(&bbox) == s(&policy["expected_bbox_sha256"])?,
        "Poppler bbox fixity drift",
    )?;
    let warnings = String::from_utf8(result.stderr).map_err(|e| e.to_string())?;
    let lines = warnings.lines().collect::<Vec<_>>();
    ensure(
        lines.len() == n(&policy["expected_warning_count"])?
            && lines
                .iter()
                .all(|v| Some(*v) == policy["expected_warning"].as_str()),
        "Poppler warning surface drift",
    )?;
    ensure(
        binary_root.hash_file(&mut executable, 32 * 1024 * 1024)? == backend_digest,
        "Poppler executable changed during extraction",
    )?;
    let text = from_bbox(ctx, &bbox, plan)?;
    Ok(Extraction {
        bbox,
        text,
        backend_digest,
        backend_version,
    })
}

fn packet(plan: &Value, content: &[u8], text: &str, method: &Value) -> Result<Value> {
    let ids = &plan["opaque_ids"];
    let anchor_ids = ids["anchor_ids"].as_array().ok_or("anchor IDs absent")?;
    let unit_ids = ids["unit_ids"].as_array().ok_or("unit IDs absent")?;
    ensure(
        anchor_ids.len() == 12 && unit_ids.len() == 11,
        "source-layout identity counts",
    )?;
    let layer = s(&plan["outputs"]["text_layer_ref"])?;
    let mut anchors = vec![records::scope_anchor(plan, layer, content, text)];
    let mut units = vec![];
    let mut segments = vec![];
    let mut start = 0;
    for (i, line) in text.split('\n').enumerate() {
        let end = start + line.chars().count();
        segments.push(("physical_line", start, end));
        start = end;
        if i < 5 {
            segments.push(("whitespace", start, start + 1));
            start += 1;
        }
    }
    ensure(
        start == text.chars().count() && segments.len() == 11,
        "source-layout coverage construction drifted",
    )?;
    let offsets = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    let digest = sha(content);
    for (i, (kind, start, end)) in segments.into_iter().enumerate() {
        let aid = &anchor_ids[i + 1];
        anchors.push(records::content_anchor(
            plan,
            layer,
            &digest,
            aid,
            i + 2,
            kind,
            start,
            end,
            &content[offsets[start]..offsets[end]],
        ));
        units.push(records::unit(&unit_ids[i], aid, kind));
    }
    Ok(records::packet(
        plan, layer, content, anchors, units, method,
    ))
}

fn output_entities(
    plan: &Value,
    content: &[u8],
    bbox: &[u8],
    outputs: &BTreeMap<String, Vec<u8>>,
    at: &str,
) -> Value {
    let mut rows = vec![
        json!({"entity_ref":plan["outputs"]["private_content_ref"],"role":"ignored-local-raw-embedded-text","sha256":sha(content),"size_bytes":content.len(),"media_type":"text/plain; charset=utf-8","availability":"ignored_local","content_disclosure":"private_content","fixity_verified":true,"fixity_verified_at":at}),
    ];
    rows.push(json!({"entity_ref":plan["outputs"]["private_bbox_ref"],"role":"ignored-local-poppler-bbox-intermediate","sha256":sha(bbox),"size_bytes":bbox.len(),"media_type":"application/xhtml+xml","availability":"ignored_local","content_disclosure":"private_content","fixity_verified":true,"fixity_verified_at":at}));
    // Existing owner order is anchor, layer, units (not lexical path order).
    for key in ["anchor_ref", "text_layer_ref", "text_unit_packet_ref"] {
        let reference = plan["outputs"][key].as_str().unwrap();
        let raw = &outputs[reference];
        rows.push(json!({"entity_ref":reference,"role":"tracked-text-free-foundation-record","sha256":sha(raw),"size_bytes":raw.len(),"media_type":"application/json","availability":"tracked","content_disclosure":"public_metadata_only","fixity_verified":true,"fixity_verified_at":at}));
    }
    json!(rows)
}

pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let (plan_raw, plan) = load(ctx, opts.plan_ref)?;
    ensure(
        plan["schema_version"] == "tos_zarathustra_target_text_foundation_plan_v1"
            && plan["extraction_policy"]["method"] == "poppler-bbox-word-line-extraction"
            && plan["extraction_policy"]["method_version"] == "1"
            && plan["extraction_policy"]["software_name"] == "pdftotext"
            && plan["extraction_policy"]["unicode_normalization"] == "none",
        "unsupported target foundation plan/policy",
    )?;
    ensure(
        plan["storage_and_rights"]["publication_authorized"] == false
            && plan["storage_and_rights"]["source_visibility"] == "local_only"
            && plan["storage_and_rights"]["private_output_mode"] == "0600",
        "source visibility boundary differs",
    )?;
    let input = ctx.select_directory(opts.input_root)?;
    let output = ctx.select_directory(opts.output_root)?;
    let source_ref = s(&plan["scope"]["source_relative_ref"])?;
    let mut source_file = input.source_file(source_ref, PDF_CAP)?;
    let source_digest = input.hash_file(&mut source_file, PDF_CAP)?;
    let source_len = usize::try_from(source_file.metadata().map_err(|e| e.to_string())?.len())
        .map_err(|_| "source size overflow")?;
    ensure(
        source_digest == s(&plan["scope"]["file_sha256"])?,
        "exact local PDF digest drift",
    )?;
    let inventory_ref = s(&plan["scope"]["resource_inventory_ref"])?;
    let (inventory_raw, inventory) = load(ctx, inventory_ref)?;
    let resources = inventory["files"]
        .as_array()
        .ok_or("inventory files absent")?
        .iter()
        .flat_map(|v| v["resources"].as_array().into_iter().flatten())
        .filter(|v| v["resource_id"] == plan["selector"]["page_resource_id"])
        .collect::<Vec<_>>();
    ensure(
        resources.len() == 1,
        "exact page resource absent or ambiguous",
    )?;
    let locator = &resources[0]["locator"];
    ensure(
        locator["page_index"] == plan["selector"]["page_number"]
            && locator["width_points"] == plan["selector"]["page_width_points"]
            && locator["height_points"] == plan["selector"]["page_height_points"]
            && locator["rotation_degrees"] == 0,
        "tracked page geometry drift",
    )?;
    let extraction = extract(ctx, &source_file, &plan)?;
    let text = &extraction.text;
    let content = text.as_bytes();
    let bbox = &extraction.bbox;
    let rights_ref = s(&plan["scope"]["rights_ref"])?;
    let rights_raw = ctx.read(rights_ref)?;
    let private_ref = s(&plan["outputs"]["private_content_ref"])?;
    private_boundary(ctx, private_ref)?;
    let bbox_ref = s(&plan["outputs"]["private_bbox_ref"])?;
    private_boundary(ctx, bbox_ref)?;
    let mut names = BTreeSet::new();
    for key in [
        "anchor_ref",
        "text_layer_ref",
        "text_unit_packet_ref",
        "provenance_event_ref",
        "private_content_ref",
        "private_bbox_ref",
    ] {
        let reference = s(&plan["outputs"][key])?;
        tos_foundation::RelativePath::parse(reference).map_err(|e| e.to_string())?;
        ensure(
            reference.starts_with("ToS/source-witnesses/")
                && names.insert(reference)
                && ![source_ref, opts.plan_ref, rights_ref, inventory_ref].contains(&reference),
            "output reference aliases another output/input or leaves source owner",
        )?;
    }
    let event_ref = s(&plan["outputs"]["provenance_event_ref"])?;
    let existing = match std::fs::symlink_metadata(ctx.root().join(event_ref)) {
        Ok(_) => Some(load(ctx, event_ref)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    ensure(
        opts.build || existing.is_some(),
        "check requires the retained provenance record",
    )?;
    let observed = if let Some((_, e)) = &existing {
        s(&e["activity"]["ended_at"])?.to_owned()
    } else {
        utc_now()?
    };
    let event_id = if let Some((_, e)) = &existing {
        s(&e["event_id"])?.to_owned()
    } else {
        let id = opts.event_id.ok_or(
            "new native build requires an explicit --event-id and fresh plan-selected outputs",
        )?;
        ensure(
            id != LEGACY_EVENT && id.starts_with("tos.event.") && id.len() < 512,
            "new event ID must be separate from the historical event",
        )?;
        id.into()
    };
    if let Some(selected) = opts.event_id {
        ensure(
            selected == event_id,
            "selected event differs from retained event",
        )?;
    }
    let historical = existing.as_ref().is_some_and(|(_, e)| {
        e["method"]["software_components"][0]["artifact_ref"] == LEGACY_BUILDER
    });
    let uv = std::char::UNICODE_VERSION;
    let native_unicode = format!("{}.{}.{}", uv.0, uv.1, uv.2);
    let (builder, unicode_version, made_at) = if historical {
        ensure(
            event_id == LEGACY_EVENT,
            "unsupported historical event identity",
        )?;
        (LEGACY_BUILDER, "16.0.0", s(&plan["created_at"])?)
    } else {
        (BUILDER, native_unicode.as_str(), observed.as_str())
    };
    let method = records::method(
        &plan,
        opts.plan_ref,
        builder,
        &event_id,
        unicode_version,
        made_at,
    );
    let anchor = records::anchor(&plan, opts.plan_ref, &sha(&plan_raw), &event_id);
    let anchor_raw = encode(&anchor, true)?;
    let mut layer = records::layer(
        &plan,
        opts.plan_ref,
        &sha(&plan_raw),
        &sha(&anchor_raw),
        &sha(&rights_raw),
        content,
        &text,
        &event_id,
    );
    let mut units = packet(&plan, content, &text, &method)?;
    if historical
        && existing.as_ref().unwrap().1["method"]["software_components"][0]["artifact_sha256"]
            == "62d35662d6ebd45833edaf189ec9777b6343e10fa74e7020e4c07b5c1248c21b"
    {
        layer["authority_boundary"] = json!(LEGACY_LAYER_AUTHORITY);
        for unit in units["units"].as_array_mut().unwrap() {
            unit["status_reason"] = json!(LEGACY_UNIT_REASON);
        }
    }
    let mut outputs = BTreeMap::new();
    for (key, kind, schema_ref, value) in [
        (
            "anchor_ref",
            "anchor",
            "ToS/contracts/source-anchor-v2.schema.json",
            anchor,
        ),
        (
            "text_layer_ref",
            "layer",
            "ToS/contracts/source-text-layer.schema.json",
            layer,
        ),
        (
            "text_unit_packet_ref",
            "units",
            "ToS/contracts/source-text-unit-packet-v1.schema.json",
            units,
        ),
    ] {
        let reference = s(&plan["outputs"][key])?;
        schema(ctx, schema_ref, &value)?;
        metadata(ctx, kind, reference, &value)?;
        let bytes = encode(&value, true)?;
        if kind == "units" {
            let report =
                tos_validation::layer_family_rules::inspect_supplied_source_text_unit_semantics(
                    reference,
                    &bytes,
                    private_ref,
                    content,
                    &sha(&plan_raw),
                    tos_validation::item_rules::ItemLimits {
                        max_member_bytes: CAP,
                        max_total_bytes: (32 * CAP) as u64,
                        max_state_bytes: 32 * CAP,
                        max_issues: 64,
                        deadline: ctx.deadline(),
                    },
                )
                .map_err(|e| format!("text replay: {e:?}"))?;
            ensure(
                report.state == tos_validation::text_rules::TextRuleState::Checked
                    && report.issues.is_empty(),
                &format!("text replay issues: {:?}", report.issues),
            )?;
        }
        ensure(
            !std::str::from_utf8(&bytes)
                .map_err(|e| e.to_string())?
                .contains(text.as_str()),
            "tracked record contains private paragraph",
        )?;
        outputs.insert(reference.to_owned(), bytes);
    }
    let entities = output_entities(&plan, content, bbox, &outputs, &observed);
    let event = if let Some((_, event)) = &existing {
        event.clone()
    } else {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let selected =
            ctx.select_directory(executable.parent().ok_or("native executable parent")?)?;
        let mut f = selected.source_file(
            executable
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("native executable name")?,
            512 * 1024 * 1024,
        )?;
        let executable_digest = selected.hash_file(&mut f, 512 * 1024 * 1024)?;
        let rows = entities.as_array().unwrap();
        let derivations=rows.iter().enumerate().map(|(i,e)|json!({"derivation_id":format!("tos.derivation.za-i-vorrede-1.antonovsky-1911-target-text-foundation.{}",i+1),"input_entity_ref":source_ref,"output_entity_ref":e["entity_ref"],"relation":"selection_from","influence_asserted":true,"description":"The exact fixity-bound PDF, tracked plan, pinned Poppler version, and declared word/line rule determine this bounded text, address, or layout record."})).collect::<Vec<_>>();
        records::provenance(records::Provenance {
            plan: &plan,
            plan_ref: opts.plan_ref,
            plan_digest: &sha(&plan_raw),
            event_id: &event_id,
            source_digest: &source_digest,
            source_len,
            pdftotext_version: &extraction.backend_version,
            pdftotext_digest: &extraction.backend_digest,
            plan_len: plan_raw.len(),
            builder_ref: BUILDER,
            builder_digest: &sha(include_bytes!("target_text_foundation.rs")),
            executable_digest: &executable_digest,
            argv: opts.argv,
            argv_digest: &sha(&encode(opts.argv, false)?),
            outputs: rows,
            derivations: &derivations,
            total_output_bytes: rows
                .iter()
                .map(|v| v["size_bytes"].as_u64().unwrap() as usize)
                .sum(),
            rights_binding: &json!({"ref":rights_ref,"sha256":sha(&rights_raw)}),
            observed_at: &observed,
        })
    };
    validate_event(
        ctx,
        &plan,
        opts.plan_ref,
        &plan_raw,
        &source_digest,
        source_len,
        "tos.derivation.za-i-vorrede-1.antonovsky-1911-target-text-foundation",
        &rights_raw,
        &event,
        &entities,
    )?;
    outputs.insert(
        event_ref.to_owned(),
        if let Some((raw, _)) = &existing {
            raw.clone()
        } else {
            encode(&event, true)?
        },
    );
    // Authenticate all inputs again before the first write. Never reuse a source checksum as a write grant.
    ensure(
        ctx.read(opts.plan_ref)? == plan_raw
            && ctx.read(rights_ref)? == rights_raw
            && ctx.read(inventory_ref)? == inventory_raw
            && input.hash_file(&mut source_file, PDF_CAP)? == source_digest,
        "source/plan/rights changed before output",
    )?;
    let mut writes = vec![];
    for (reference, raw) in &outputs {
        if fresh_or_matching(ctx, reference, raw)? {
            ensure(opts.build, "tracked output absent")?;
            writes.push((reference, raw));
        }
    }
    let mut private_writes = vec![];
    for (reference, raw) in [(private_ref, content), (bbox_ref, bbox.as_slice())] {
        if fresh_or_matching(&output, reference, raw)? {
            ensure(opts.build, "private output absent")?;
            private_writes.push((reference, raw));
        }
    }
    if opts.build {
        for (reference, raw) in private_writes {
            output.write(reference, raw, 0o600, true)?;
        }
        for (reference, raw) in writes {
            ctx.write(reference, raw, 0o644, true)?;
        }
    }
    for reference in [private_ref, bbox_ref] {
        ensure(
            output
                .source_file(reference, CAP as u64)?
                .metadata()
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o777
                == 0o600,
            "private target output mode is not 0600",
        )?;
    }
    ctx.check()?;
    Ok(
        json!({"status":"passed","question":plan["question"],"source_sha256":source_digest,"bbox_sha256":sha(bbox),"bbox_bytes":bbox.len(),"private_content_sha256":sha(content),"private_content_bytes":content.len(),"private_content_codepoints":text.chars().count(),"source_observed_print_lines":6,"source_observed_line_breaks":5,"tracked_record_count":outputs.len(),"accepted_russian":false,"accepted_translation":false,"alignment_created":false,"human_review_performed":false,"semantic_or_canon_effect":false,"publication_authorized":false,"historical_provenance_preserved":historical,"execution_truth_authenticated":false,"native_executor":"tos target-text-foundation","event_id":event_id}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_unicode_records_replay_frozen_previous_producer_bytes() {
        let fixture: Value =
            serde_json::from_str(include_str!("target_text_foundation/synthetic-parity.json"))
                .unwrap();
        let plan = &fixture["plan"];
        let digest = fixture["plan_digest"].as_str().unwrap();
        let text = fixture["synthetic_text"].as_str().unwrap();
        let content = text.as_bytes();
        let anchor = records::anchor(plan, PLAN, digest, LEGACY_EVENT);
        let ab = encode(&anchor, true).unwrap();
        let layer = records::layer(
            plan,
            PLAN,
            digest,
            &sha(&ab),
            fixture["rights_digest"].as_str().unwrap(),
            content,
            text,
            LEGACY_EVENT,
        );
        let method = records::method(
            plan,
            PLAN,
            LEGACY_BUILDER,
            LEGACY_EVENT,
            fixture["unicode_version"].as_str().unwrap(),
            plan["created_at"].as_str().unwrap(),
        );
        let units = packet(plan, content, text, &method).unwrap();
        let ctx = ResearchExecution::new(&std::env::temp_dir(), 30).unwrap();
        for (kind, key, value) in [
            ("anchor", "anchor_ref", &anchor),
            ("layer", "text_layer_ref", &layer),
            ("units", "text_unit_packet_ref", &units),
        ] {
            metadata(&ctx, kind, plan["outputs"][key].as_str().unwrap(), value).unwrap();
        }
        for (key, raw) in [
            ("anchor", ab),
            ("layer", encode(&layer, true).unwrap()),
            ("units", encode(&units, true).unwrap()),
        ] {
            assert_eq!(
                sha(&raw),
                fixture["expected_sha256"][key].as_str().unwrap(),
                "{key}"
            );
        }
        assert_eq!(units["anchors"][0]["selector"]["end"], text.chars().count());
    }
    #[test]
    fn bbox_selects_exact_codepoints_and_rejects_guard_geometry_and_entities() {
        let ctx = ResearchExecution::new(&std::env::temp_dir(), 30).unwrap();
        let words = ["А & 😀", "Б", "В", "Г", "Д", "Е"];
        let text = words.join("\n");
        let mut plan = json!({"selector":{"page_width_points":10.0,"page_height_points":20.0,"first_word_sha256":sha(words[0].as_bytes()),"last_word_sha256":sha(words[5].as_bytes()),"next_line_first_word_sha256":sha(b"next"),"block_match_count":1,"selected_line_start":0,"selected_line_count":6,"expected_word_count":6,"region_points":{"x":0.0,"y":0.0,"width":1.0,"height":6.0}},"extraction_policy":{"expected_content_bytes":text.len(),"expected_content_codepoints":text.chars().count(),"expected_content_sha256":sha(text.as_bytes())}});
        let lines = words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                format!(
                    "<line xMin=\"0\" yMin=\"{i}\" xMax=\"1\" yMax=\"{}\"><word>{}</word></line>",
                    i + 1,
                    w.replace('&', "&amp;")
                )
            })
            .collect::<String>();
        let raw = format!(
            "<!DOCTYPE html><html><page width=\"10\" height=\"20\"><block>{lines}<line><word>next</word></line></block></page></html>"
        );
        assert_eq!(from_bbox(&ctx, raw.as_bytes(), &plan).unwrap(), text);
        for changed in [
            raw.replace("<word>next</word>", "<word>other</word>"),
            raw.replace("xMax=\"1\"", "xMax=\"2\""),
            raw.replace("&amp;", "&external;"),
            raw.replace("<word>Б</word>", "<word><b>Б</b></word>"),
        ] {
            assert!(from_bbox(&ctx, changed.as_bytes(), &plan).is_err());
        }
        plan["selector"]["selected_line_start"] = json!(usize::MAX);
        assert!(from_bbox(&ctx, raw.as_bytes(), &plan).is_err());
    }
}
