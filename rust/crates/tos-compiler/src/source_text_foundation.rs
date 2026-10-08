//! Exact DTA paragraph extraction with source-owned address/layer/unit records.
//! Historical provenance remains a declaration about its original producer;
//! native creation records the actual installed executable and never overwrites it.
use crate::research_execution::ResearchExecution;
use quick_xml::{Reader, events::Event};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::PermissionsExt,
    path::Path,
};
use tos_foundation::{Digest256, JsonLimits, JsonMode};
#[path = "source_text_foundation/records.rs"]
mod records;
type Result<T> = std::result::Result<T, String>;
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-source-text-foundation.plan.v1.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/source_text_foundation.rs";
const LEGACY_BUILDER: &str = "scripts/build_zarathustra_source_text_foundation.py";
const LEGACY_EVENT: &str = "tos.event.native-extraction.za-i-vorrede-1.dta-part-1-p1.2026-08-12";
const OPENING: &str = "a507f7293ac73ec37f7c78a38b3270f45bc98418b8fd824f2ab72ebc18377224";
const SELECTOR: &str = "/*[local-name()='TEI']/*[local-name()='text']/*[local-name()='body']/*[local-name()='div'][1]/*[local-name()='div'][1]/*[local-name()='p'][1]";
const CAP: usize = 4 * 1024 * 1024;
pub(super) fn ensure(v: bool, s: &str) -> Result<()> {
    if v { Ok(()) } else { Err(s.into()) }
}
pub(super) fn sha(raw: &[u8]) -> String {
    Digest256::of_bytes(raw).to_hex()
}
pub(super) fn s(v: &Value) -> Result<&str> {
    v.as_str()
        .filter(|x| !x.is_empty())
        .ok_or("required nonempty string".into())
}
pub(super) fn encode(v: &Value, pretty: bool) -> Result<Vec<u8>> {
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
pub(super) fn load(ctx: &ResearchExecution, r: &str) -> Result<(Vec<u8>, Value)> {
    let mut f = ctx.source_file(r, CAP as u64)?;
    let raw = ctx.read_file(&mut f, CAP as u64)?;
    tos_foundation::parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default())
        .map_err(|e| e.to_string())?;
    let v = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    Ok((raw, v))
}
#[derive(Default)]
pub(super) struct Node {
    pub(super) name: String,
    pub(super) attrs: BTreeMap<String, String>,
    pub(super) content: Vec<Part>,
}
pub(super) enum Part {
    Text(String),
    Child(Node),
}
impl Node {
    fn text(&mut self, text: &str) {
        if let Some(Part::Text(last)) = self.content.last_mut() {
            last.push_str(text)
        } else {
            self.content.push(Part::Text(text.into()))
        }
    }
    pub(super) fn children(&self, name: &str) -> Vec<&Node> {
        self.content
            .iter()
            .filter_map(|p| match p {
                Part::Child(n) if n.name == name => Some(n),
                _ => None,
            })
            .collect()
    }
}
fn xml(ctx: &ResearchExecution, raw: &[u8]) -> Result<Node> {
    xml_with_doctype(ctx, raw, false)
}
pub(super) fn xml_with_doctype(
    ctx: &ResearchExecution,
    raw: &[u8],
    allow_doctype: bool,
) -> Result<Node> {
    fn attrs(
        e: &quick_xml::events::BytesStart<'_>,
        decoder: quick_xml::encoding::Decoder,
    ) -> Result<BTreeMap<String, String>> {
        let mut values = BTreeMap::new();
        for a in e.attributes() {
            let a = a.map_err(|e| e.to_string())?;
            let key = std::str::from_utf8(a.key.as_ref())
                .map_err(|e| e.to_string())?
                .to_owned();
            let value = a
                .decode_and_unescape_value(decoder)
                .map_err(|e| e.to_string())?
                .into_owned();
            ensure(
                values.len() < 128 && values.insert(key, value).is_none(),
                "XML attribute count/duplicate",
            )?;
        }
        Ok(values)
    }
    ensure(raw.len() <= CAP, "TEI source byte bound")?;
    let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    let mut stack = vec![Node::default()];
    let mut count = 0;
    loop {
        ctx.tick(1)?;
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                count += 1;
                ensure(count <= 100000 && stack.len() < 128, "TEI node/depth bound")?;
                stack.push(Node {
                    name: std::str::from_utf8(e.local_name().as_ref())
                        .map_err(|e| e.to_string())?
                        .into(),
                    attrs: attrs(&e, reader.decoder())?,
                    content: vec![],
                });
            }
            Event::Empty(e) => {
                count += 1;
                ensure(count <= 100000, "TEI node bound")?;
                stack.last_mut().unwrap().content.push(Part::Child(Node {
                    name: std::str::from_utf8(e.local_name().as_ref())
                        .map_err(|e| e.to_string())?
                        .into(),
                    attrs: attrs(&e, reader.decoder())?,
                    content: vec![],
                }));
            }
            Event::End(_) => {
                ensure(stack.len() > 1, "unexpected XML end")?;
                let child = stack.pop().unwrap();
                stack.last_mut().unwrap().content.push(Part::Child(child));
            }
            Event::Text(e) => {
                let decoded = e
                    .xml_content(quick_xml::XmlVersion::Explicit1_0)
                    .map_err(|e| e.to_string())?;
                stack.last_mut().unwrap().text(&decoded);
            }
            Event::CData(e) => {
                let decoded = e
                    .xml_content(quick_xml::XmlVersion::Explicit1_0)
                    .map_err(|e| e.to_string())?;
                stack.last_mut().unwrap().text(&decoded);
            }
            Event::GeneralRef(e) => {
                let decoded = e.decode().map_err(|e| e.to_string())?;
                let entity = format!("&{decoded};");
                let value = quick_xml::escape::unescape(&entity).map_err(|e| e.to_string())?;
                stack.last_mut().unwrap().text(&value);
            }
            Event::Decl(e) => {
                ensure(
                    e.xml_version().map_err(|e| e.to_string())?
                        != quick_xml::XmlVersion::Explicit1_1,
                    "source requires XML 1.0",
                )?;
            }
            Event::DocType(_) if !allow_doctype => {
                return Err("bounded DTA source does not allow a DTD".into());
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure(stack.len() == 1, "unclosed XML element")?;
    let mut outer = stack.pop().unwrap();
    ensure(
        outer
            .content
            .iter()
            .filter(|x| matches!(x, Part::Child(_)))
            .count()
            == 1,
        "XML root count",
    )?;
    let at = outer
        .content
        .iter()
        .position(|x| matches!(x, Part::Child(_)))
        .unwrap();
    match outer.content.remove(at) {
        Part::Child(n) => Ok(n),
        _ => unreachable!(),
    }
}
fn selected_paragraph<'a>(root: &'a Node) -> Result<&'a Node> {
    ensure(root.name == "TEI", "source root is not TEI")?;
    let texts = root.children("text");
    ensure(texts.len() == 1, "expected one direct text")?;
    let bodies = texts[0].children("body");
    ensure(bodies.len() == 1, "expected one direct body")?;
    let div = bodies[0].children("div");
    let div = div.first().ok_or("TEI body has no direct div")?;
    let inner = div.children("div");
    let inner = inner.first().ok_or("first TEI div has no direct div")?;
    inner
        .children("p")
        .first()
        .copied()
        .ok_or("selected TEI div has no direct paragraph".into())
}
fn render_paragraph(p: &Node) -> Result<String> {
    let mut text = String::new();
    let mut consume_lf = false;
    for part in &p.content {
        match part {
            Part::Text(chunk) => {
                if consume_lf {
                    ensure(
                        chunk.starts_with('\n'),
                        "bounded lb lacks declared formatting line feed",
                    )?;
                    text.push_str(&chunk[1..]);
                    consume_lf = false;
                } else {
                    text.push_str(chunk)
                }
            }
            Part::Child(n) => {
                ensure(
                    n.name == "lb" && n.content.is_empty(),
                    "unexpected child or nonempty lb in bounded paragraph",
                )?;
                ensure(!consume_lf, "lb lacks formatting tail")?;
                text.push('\n');
                consume_lf = true;
            }
        }
    }
    ensure(
        !consume_lf && !text.contains('\r') && !text.ends_with('\n'),
        "rendered paragraph line-ending policy",
    )?;
    ensure(
        text.split('\n').count() == 7,
        "expected seven source-observed print lines",
    )?;
    Ok(text)
}
fn extract(ctx: &ResearchExecution, raw: &[u8]) -> Result<String> {
    let doc = xml(ctx, raw)?;
    let text = render_paragraph(selected_paragraph(&doc)?)?;
    let opening = text.split('.').next().unwrap_or("").replace('\n', " ") + ".";
    ensure(
        sha(opening.as_bytes()) == OPENING,
        "opening-sentence calibration digest drifted",
    )?;
    Ok(text)
}
fn packet(plan: &Value, content: &[u8], text: &str, method: &Value) -> Result<Value> {
    let ids = &plan["opaque_ids"];
    let anchor_ids = ids["anchor_ids"].as_array().ok_or("anchor IDs absent")?;
    let unit_ids = ids["unit_ids"].as_array().ok_or("unit IDs absent")?;
    ensure(
        anchor_ids.len() == 14 && unit_ids.len() == 13,
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
        if i < 6 {
            segments.push(("whitespace", start, start + 1));
            start += 1;
        }
    }
    ensure(
        start == text.chars().count() && segments.len() == 13,
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
pub(super) fn schema(ctx: &ResearchExecution, reference: &str, value: &Value) -> Result<()> {
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
pub(super) fn metadata(
    ctx: &ResearchExecution,
    kind: &str,
    reference: &str,
    value: &Value,
) -> Result<()> {
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
pub(super) fn private_boundary(ctx: &ResearchExecution, reference: &str) -> Result<()> {
    use crate::owned_native_child::{CaptureLimits, capture_with_cancel};
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
pub(super) const LEGACY_LAYER_AUTHORITY: &str = "a source text layer is one immutable, source-returnable representation with explicit derivation, uncertainty, review, competence, rights, and use scope; mechanical validation, model output, normalization, or agreement with another layer does not make it accepted source text, translation evidence, linguistic truth, semantic evidence, graph truth, canon authority, or publication permission";
const LEGACY_UNIT_REASON: &str = "The unit records only one exact TEI lb-delimited print line or its line-break code point; it is not accepted German or linguistic analysis.";
const LEGACY_SEGMENTATION_REASON: &str = "Only the exact TEI lb-delimited layout of one provider-transcription paragraph is observed; no word or sentence boundary is asserted.";
pub(super) fn utc_now() -> Result<String> {
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
pub(super) fn fresh_or_matching(
    ctx: &ResearchExecution,
    reference: &str,
    bytes: &[u8],
) -> Result<bool> {
    // Distinguish an absent file from an unsafe/read-failed existing one before writes.
    match std::fs::symlink_metadata(ctx.root().join(reference)) {
        Ok(_) => {
            let mut f = ctx.source_file(reference, CAP as u64)?;
            ensure(
                ctx.read_file(&mut f, CAP as u64)? == bytes,
                "existing output differs; preserve it and select a new source-owned plan/revision",
            )?;
            Ok(false)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.to_string()),
    }
}
fn output_entities(
    plan: &Value,
    content: &[u8],
    outputs: &BTreeMap<String, Vec<u8>>,
    at: &str,
) -> Value {
    let mut rows = vec![
        json!({"entity_ref":plan["outputs"]["private_content_ref"],"role":"ignored-local-machine-transcription","sha256":sha(content),"size_bytes":content.len(),"media_type":"text/plain; charset=utf-8","availability":"ignored_local","content_disclosure":"private_content","fixity_verified":true,"fixity_verified_at":at}),
    ];
    // Existing owner order is anchor, layer, units (not lexical path order).
    for key in ["anchor_ref", "text_layer_ref", "text_unit_packet_ref"] {
        let reference = plan["outputs"][key].as_str().unwrap();
        let raw = &outputs[reference];
        rows.push(json!({"entity_ref":reference,"role":"tracked-text-free-foundation-record","sha256":sha(raw),"size_bytes":raw.len(),"media_type":"application/json","availability":"tracked","content_disclosure":"public_metadata_only","fixity_verified":true,"fixity_verified_at":at}));
    }
    json!(rows)
}
pub(super) fn validate_event(
    ctx: &ResearchExecution,
    plan: &Value,
    plan_ref: &str,
    plan_raw: &[u8],
    source_digest: &str,
    source_len: usize,
    derivation_prefix: &str,
    rights_raw: &[u8],
    event: &Value,
    entities: &Value,
) -> Result<()> {
    schema(ctx, "ToS/contracts/provenance-event-v2.schema.json", event)?;
    let issues = tos_validation::provenance_rules::semantic_issues(event, 64, ctx.deadline())
        .map_err(|e| format!("provenance: {e:?}"))?;
    ensure(
        issues.is_empty(),
        &format!("provenance semantic issues: {issues:?}"),
    )?;
    ensure(
        event["entities"]["outputs"] == *entities,
        "provenance output bytes/membership changed",
    )?;
    let inputs = event["entities"]["inputs"]
        .as_array()
        .ok_or("event inputs absent")?;
    ensure(inputs.len() == 2, "foundation event input count")?;
    for (r, digest, len) in [
        (
            s(&plan["scope"]["source_relative_ref"])?,
            source_digest.to_owned(),
            source_len,
        ),
        (plan_ref, sha(plan_raw), plan_raw.len()),
    ] {
        let row = inputs
            .iter()
            .find(|v| v["entity_ref"] == r)
            .ok_or("provenance input absent")?;
        ensure(
            row["sha256"] == digest && row["size_bytes"] == len && row["fixity_verified"] == true,
            "provenance input fixity differs",
        )?;
    }
    let binding = json!({"ref":plan_ref,"sha256":sha(plan_raw)});
    ensure(
        event["method"]["configuration_binding"] == binding
            && event["method"]["environment"]["environment_profile_binding"] == binding,
        "provenance plan binding differs",
    )?;
    ensure(
        event["method"]["command_capture"]["argv_sha256"]
            == sha(&encode(&event["method"]["command_capture"]["argv"], false)?),
        "captured argv digest differs",
    )?;
    ensure(
        event["rights_and_visibility"]["rights_record_bindings"]
            == json!([{"ref":plan["scope"]["rights_ref"],"sha256":sha(rights_raw)}]),
        "provenance rights binding differs",
    )?;
    ensure(
        event["rights_and_visibility"]["publication_authorized"] == false
            && event["review_and_authority"]["promotion_authorized"] == false
            && event["review_and_authority"]["accepted_uses"] == json!([])
            && event["evidence_authentication"]["signature_status"] == "unsigned",
        "provenance crossed review/publication authority",
    )?;
    let rows = event["derivations"]
        .as_array()
        .ok_or("event derivations absent")?;
    ensure(
        rows.len() == entities.as_array().unwrap().len(),
        "provenance derivation closure",
    )?;
    for (i, (row, entity)) in rows.iter().zip(entities.as_array().unwrap()).enumerate() {
        ensure(
            row["derivation_id"] == format!("{derivation_prefix}.{}", i + 1)
                && row["input_entity_ref"] == plan["scope"]["source_relative_ref"]
                && row["output_entity_ref"] == entity["entity_ref"]
                && row["relation"] == "selection_from"
                && row["influence_asserted"] == true,
            "provenance derivation source/output differs",
        )?;
    }
    ctx.check()
}
pub struct Options<'a> {
    pub plan_ref: &'a str,
    pub input_root: &'a Path,
    pub output_root: &'a Path,
    pub build: bool,
    pub event_id: Option<&'a str>,
    pub argv: &'a Value,
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let (plan_raw, plan) = load(ctx, opts.plan_ref)?;
    ensure(
        plan["schema_version"] == "tos_zarathustra_source_text_foundation_plan_v1"
            && plan["selector"]["scheme"] == "xpath-1.0-local-name"
            && plan["selector"]["value"] == SELECTOR
            && plan["selector"]["expected_match_count"] == 1,
        "unsupported source foundation plan/selector",
    )?;
    ensure(
        plan["extraction_policy"]["method"] == "tei-lb-aware-structural-extraction"
            && plan["extraction_policy"]["method_version"] == "1"
            && plan["extraction_policy"]["accepted_child_elements"] == json!(["lb"])
            && plan["extraction_policy"]["terminal_newline"] == false
            && plan["extraction_policy"]["unicode_normalization"] == "none",
        "extraction policy differs",
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
    let mut source_file = input.source_file(source_ref, CAP as u64)?;
    let source = input.read_file(&mut source_file, CAP as u64)?;
    ensure(
        sha(&source) == s(&plan["scope"]["file_sha256"])?,
        "exact local DTA source digest drifted",
    )?;
    let text = extract(ctx, &source)?;
    let content = text.as_bytes();
    let rights_ref = s(&plan["scope"]["rights_ref"])?;
    let rights_raw = ctx.read(rights_ref)?;
    let private_ref = s(&plan["outputs"]["private_content_ref"])?;
    private_boundary(ctx, private_ref)?;
    let mut names = BTreeSet::new();
    for key in [
        "anchor_ref",
        "text_layer_ref",
        "text_unit_packet_ref",
        "provenance_event_ref",
        "private_content_ref",
    ] {
        let reference = s(&plan["outputs"][key])?;
        tos_foundation::RelativePath::parse(reference).map_err(|e| e.to_string())?;
        ensure(
            reference.starts_with("ToS/source-witnesses/")
                && names.insert(reference)
                && ![source_ref, opts.plan_ref, rights_ref].contains(&reference),
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
    let method = records::method(opts.plan_ref, builder, &event_id, unicode_version, made_at);
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
            == "d4cf34d6e0f4524ce3f87c8251972cc4d2e1fbf52de54820d21bd9c233924583"
    {
        layer["authority_boundary"] = json!(LEGACY_LAYER_AUTHORITY);
        for unit in units["units"].as_array_mut().unwrap() {
            unit["status_reason"] = json!(LEGACY_UNIT_REASON);
        }
        units["segmentations"][0]["status_reason"] = json!(LEGACY_SEGMENTATION_REASON);
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
                .contains(&text),
            "tracked record contains private paragraph",
        )?;
        outputs.insert(reference.to_owned(), bytes);
    }
    let entities = output_entities(&plan, content, &outputs, &observed);
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
        let derivations=rows.iter().enumerate().map(|(i,e)|json!({"derivation_id":format!("tos.derivation.za-i-vorrede-1.source-text-foundation.{}",i+1),"input_entity_ref":source_ref,"output_entity_ref":e["entity_ref"],"relation":"selection_from","influence_asserted":true,"description":"The exact fixity-bound TEI source and the tracked plan determine this bounded text, address, lineage, or layout record."})).collect::<Vec<_>>();
        records::provenance(records::Provenance {
            plan: &plan,
            plan_ref: opts.plan_ref,
            plan_digest: &sha(&plan_raw),
            event_id: &event_id,
            source_digest: &sha(&source),
            source_len: source.len(),
            plan_len: plan_raw.len(),
            builder_ref: BUILDER,
            builder_digest: &sha(include_bytes!("source_text_foundation.rs")),
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
        &sha(&source),
        source.len(),
        "tos.derivation.za-i-vorrede-1.source-text-foundation",
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
            && input.read(source_ref)? == source,
        "source/plan/rights changed before output",
    )?;
    let mut writes = vec![];
    for (reference, raw) in &outputs {
        if fresh_or_matching(ctx, reference, raw)? {
            ensure(opts.build, "tracked output absent")?;
            writes.push((reference, raw));
        }
    }
    let private_missing = fresh_or_matching(&output, private_ref, content)?;
    ensure(opts.build || !private_missing, "private output absent")?;
    if opts.build {
        if private_missing {
            output.write(private_ref, content, 0o600, true)?;
        }
        for (reference, raw) in writes {
            ctx.write(reference, raw, 0o644, true)?;
        }
    }
    let private_file = output.source_file(private_ref, CAP as u64)?;
    ensure(
        private_file
            .metadata()
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o777
            == 0o600,
        "private materialized content mode is not 0600",
    )?;
    ctx.check()?;
    Ok(
        json!({"status":"passed","question":plan["question"],"source_sha256":sha(&source),"private_content_sha256":sha(content),"private_content_bytes":content.len(),"private_content_codepoints":text.chars().count(),"source_observed_print_lines":7,"source_observed_line_breaks":6,"tracked_record_count":outputs.len(),"accepted_german":false,"accepted_translation_input":false,"human_review_performed":false,"semantic_or_canon_effect":false,"publication_authorized":false,"historical_provenance_preserved":historical,"execution_truth_authenticated":false,"native_executor":"tos source-text-foundation","event_id":event_id}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wrap(p: &str) -> String {
        format!(
            "<TEI xmlns=\"urn:tei\"><text><body><div><div><p>{p}</p></div></div></body></text></TEI>"
        )
    }
    #[test]
    fn exact_xml_layout_handles_entities_and_rejects_ambiguous_input() {
        let ctx = ResearchExecution::new(&std::env::temp_dir(), 30).unwrap();
        let paragraph = "Ä &amp; &#x1F600;<lb/>\nB<lb/>\nC<lb/>\nD<lb/>\nE<lb/>\nF<lb/>\nG";
        let raw = wrap(paragraph);
        let node = xml(&ctx, raw.as_bytes()).unwrap();
        assert_eq!(
            render_paragraph(selected_paragraph(&node).unwrap()).unwrap(),
            "Ä & 😀\nB\nC\nD\nE\nF\nG"
        );
        // This synthetic text is never accepted as the calibrated historical paragraph.
        assert!(extract(&ctx, raw.as_bytes()).is_err());
        for modified in [
            paragraph.replace("<lb/>\nB", "<lb/>B"),
            paragraph.replace("<lb/>", "<hi/>"),
            paragraph.replace("<lb/>", "<lb>x</lb>"),
        ] {
            let node = xml(&ctx, wrap(&modified).as_bytes()).unwrap();
            assert!(render_paragraph(selected_paragraph(&node).unwrap()).is_err());
        }
        let duplicate = raw.replace("</TEI>", "<text><body/></text></TEI>");
        assert!(selected_paragraph(&xml(&ctx, duplicate.as_bytes()).unwrap()).is_err());
        assert!(
            xml(
                &ctx,
                format!("<!DOCTYPE TEI [<!ENTITY x 'hidden'>]>{raw}").as_bytes()
            )
            .is_err()
        );
    }
    #[test]
    fn synthetic_unicode_records_replay_frozen_previous_producer_bytes() {
        let fixture: Value =
            serde_json::from_str(include_str!("source_text_foundation/synthetic-parity.json"))
                .unwrap();
        let plan = &fixture["plan"];
        let digest = fixture["plan_digest"].as_str().unwrap();
        let text = fixture["text"].as_str().unwrap();
        let content = text.as_bytes();
        let anchor = records::anchor(plan, PLAN, digest, LEGACY_EVENT);
        let anchor_bytes = encode(&anchor, true).unwrap();
        let layer = records::layer(
            plan,
            PLAN,
            digest,
            &sha(&anchor_bytes),
            fixture["rights_digest"].as_str().unwrap(),
            content,
            text,
            LEGACY_EVENT,
        );
        let method = records::method(
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
            ("anchor", anchor_bytes),
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
        assert!(text.len() > text.chars().count());
        let uv = std::char::UNICODE_VERSION;
        let native = records::method(
            PLAN,
            BUILDER,
            "tos.event.native-source-fixture",
            &format!("{}.{}.{}", uv.0, uv.1, uv.2),
            "2026-10-08T00:00:00Z",
        );
        let native_packet = packet(plan, content, text, &native).unwrap();
        let uri =
            "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json";
        let probe = tos_validation::SchemaBackendProbe::new(
            [tos_validation::SchemaResource {
                uri: uri.into(),
                raw: include_bytes!(
                    "../../../../ToS/contracts/source-text-unit-packet-v1.schema.json"
                )
                .to_vec(),
            }],
            tos_validation::FormatProfile::AssertedSourceCandidateV1,
        )
        .unwrap();
        assert!(
            probe
                .is_valid_raw(uri, &encode(&native_packet, false).unwrap())
                .unwrap()
        );
    }
}
