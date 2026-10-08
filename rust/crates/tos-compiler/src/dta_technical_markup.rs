//! Exact, source-owned DTA TEI technical structure. Text stays in explicitly
//! selected private storage; observed markup confers no textual authority.
use crate::{
    research_execution::ResearchExecution,
    source_text_foundation::{
        Node, Part, ensure, private_boundary, s, sha, utc_now, xml_with_doctype,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::PermissionsExt,
    path::Path,
};
use tos_foundation::{JsonLimits, JsonMode, RelativePath};
#[path = "dta_technical_markup/records.rs"]
mod records;
type Result<T> = std::result::Result<T, String>;
pub const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/dta-first-editions-parts-1-4-v1/plan.v1.json";
const BUILDER: &str = "rust/crates/tos-compiler/src/dta_technical_markup.rs";
const LEGACY_BUILDER: &str = "scripts/build_zarathustra_technical_markup.py";
const LEGACY_EVENT: &str = "tos.event.segmentation.zarathustra-dta-technical-markup-v1.2026-09-01";
const LEGACY_EVENT_SHA: &str = "16a93b52ee7a86b72bd4ad1a8bb4e0f245c2eab95f447a9f600faf04890fdd17";
const SCHEMA: &str = "ToS/contracts/source-text-unit-packet-v1.schema.json";
const PROVENANCE_SCHEMA: &str = "ToS/contracts/provenance-event.schema.json";
const ID_NAMESPACE: &str = "tos-zarathustra-dta-technical-markup-v1-opaque-issuance";
const CAP: usize = 32 * 1024 * 1024;
const UNIT_CAP: usize = 20_000;
const LEGACY_UNIT_REASON: &str = "The unit records only a boundary explicitly present in the exact selected DTA TEI representation; this is not accepted German, editorial hierarchy, linguistic analysis, translation, or semantics.";
#[derive(Clone, Copy)]
pub enum Action {
    Build { issue_identities: bool },
    Check,
    ValidateTracked,
}
pub struct Options<'a> {
    pub plan_ref: &'a str,
    pub input_root: Option<&'a Path>,
    pub output_root: Option<&'a Path>,
    pub action: Action,
    pub event_id: Option<&'a str>,
}
#[derive(Debug)]
struct Unit {
    locator: String,
    source_tag: String,
    unit_kind: &'static str,
    parent: Option<usize>,
    children: Vec<usize>,
    start: usize,
    end: usize,
    resource_id: Value,
    correspondence_id: Value,
    correspondence_sequence: Value,
    structural_role: &'static str,
}
struct ObservedPart {
    config: Value,
    payload_ref: String,
    content: String,
    positions: Vec<usize>,
    units: Vec<Unit>,
    excluded: Vec<Value>,
}
fn array(v: &Value) -> Result<&[Value]> {
    v.as_array()
        .map(Vec::as_slice)
        .ok_or("expected array".into())
}
fn number(v: &Value) -> Result<u64> {
    v.as_u64().ok_or("expected unsigned integer".into())
}
fn load(ctx: &ResearchExecution, r: &str) -> Result<(Vec<u8>, Value)> {
    let mut f = ctx.source_file(r, CAP as u64)?;
    let raw = ctx.read_file(&mut f, CAP as u64)?;
    let limits = JsonLimits::new(CAP, 128, 2_000_000, 4300).map_err(|e| e.to_string())?;
    tos_foundation::parse_json(&raw, JsonMode::PublishedStrict, limits)
        .map_err(|e| e.to_string())?;
    let v = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    ctx.check()?;
    Ok((raw, v))
}
fn encode(ctx: &ResearchExecution, v: &Value, pretty: bool) -> Result<Vec<u8>> {
    ctx.check()?;
    let mut ordered = v.clone();
    if pretty {
        ordered.sort_all_objects();
    }
    let mut raw = if pretty {
        serde_json::to_vec_pretty(&ordered)
    } else {
        serde_json::to_vec(&ordered)
    }
    .map_err(|e| e.to_string())?;
    ensure(raw.len() < CAP, "technical JSON byte cap")?;
    ctx.tick(raw.len() as u64)?;
    if pretty {
        raw.push(b'\n');
    }
    Ok(raw)
}
fn jsonl(ctx: &ResearchExecution, rows: &[Value]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for row in rows {
        let raw = encode(ctx, row, false)?;
        ensure(
            raw.len() + 1 <= CAP.saturating_sub(out.len()),
            "technical JSONL byte cap",
        )?;
        out.extend(raw);
        out.push(b'\n');
    }
    Ok(out)
}
fn lines(ctx: &ResearchExecution, r: &str) -> Result<(Vec<u8>, Vec<Value>)> {
    let raw = ctx.read(r)?;
    ensure(raw.len() <= CAP, "technical JSONL byte cap")?;
    let mut rows = Vec::new();
    for line in raw.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        ctx.tick(1)?;
        ensure(rows.len() < UNIT_CAP, "technical row cap")?;
        tos_foundation::parse_json(line, JsonMode::PublishedStrict, JsonLimits::default())
            .map_err(|e| e.to_string())?;
        rows.push(serde_json::from_slice(line).map_err(|e| e.to_string())?);
    }
    Ok((raw, rows))
}
fn out(plan: &Value, key: &str) -> Result<String> {
    Ok(s(&plan["outputs"][key])?.to_owned())
}
fn pattern(plan: &Value, key: &str, part: u64) -> Result<String> {
    let p = s(&plan["outputs"][key])?;
    ensure(
        p.matches("{part_order}").count() == 1,
        "expected one part-order output placeholder",
    )?;
    let p = p.replace("{part_order}", &part.to_string());
    RelativePath::parse(&p).map_err(|e| e.to_string())?;
    Ok(p)
}
fn kind(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "body" => "document",
        "div" | "cit" | "list" => "section",
        "p" => "paragraph",
        "lg" => "verse_group",
        "l" => "verse_line",
        "head" | "item" | "quote" | "bibl" => "other",
        "pb" | "milestone" => "milestone",
        _ => return None,
    })
}
fn clean_text(raw: &str, after_lb: bool) -> &str {
    let raw = if after_lb {
        raw.strip_prefix('\n').unwrap_or(raw)
    } else {
        raw
    };
    if !raw.is_empty()
        && raw
            .chars()
            .all(|c| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
        && raw.contains(['\n', '\r', '\t'])
    {
        ""
    } else {
        raw
    }
}
fn paths<'a>(
    n: &'a Node,
    path: String,
    rows: &mut BTreeMap<usize, String>,
    all: &mut BTreeSet<String>,
) -> Result<()> {
    ensure(
        rows.len() < 100_000 && all.insert(path.clone()),
        "TEI path cap/collision",
    )?;
    rows.insert(n as *const _ as usize, path.clone());
    let mut counts = BTreeMap::new();
    for p in &n.content {
        if let Part::Child(c) = p {
            let count = counts.entry(&c.name).or_insert(0);
            *count += 1;
            paths(c, format!("{path}/{}[{count}]", c.name), rows, all)?;
        }
    }
    Ok(())
}
fn observe(
    ctx: &ResearchExecution,
    config: Value,
    payload_ref: String,
    raw: &[u8],
    resources: &BTreeMap<String, Value>,
    major: &BTreeMap<(String, String), Value>,
) -> Result<ObservedPart> {
    let root = xml_with_doctype(ctx, raw, false)?;
    ensure(
        root.name == "TEI"
            && root.attrs.get("xmlns").map(String::as_str) == Some("http://www.tei-c.org/ns/1.0"),
        "unexpected TEI namespace/root",
    )?;
    let texts = root.children("text");
    ensure(texts.len() == 1, "one TEI text required")?;
    let bodies = texts[0].children("body");
    ensure(bodies.len() == 1, "one TEI body required")?;
    let body = bodies[0];
    let mut xml_paths = BTreeMap::new();
    let mut all_paths = BTreeSet::new();
    paths(&root, "TEI".into(), &mut xml_paths, &mut all_paths)?;
    ensure(
        xml_paths[&(body as *const _ as usize)] == "TEI/text[1]/body[1]",
        "body selector drift",
    )?;
    let mut excluded = BTreeSet::new();
    let mut excluded_resources = Vec::new();
    for id in array(&config["excluded_resource_ids"])? {
        let matches = resources
            .iter()
            .filter(|(_, r)| r["resource_id"] == *id)
            .collect::<Vec<_>>();
        ensure(matches.len() == 1, "excluded resource must resolve once")?;
        let (path, r) = matches[0];
        ensure(all_paths.contains(path), "excluded TEI locator absent")?;
        excluded.insert(path.clone());
        excluded_resources
            .push(json!({"resource_id":id,"resource_kind":r["resource_kind"],"tei_path":path}));
    }
    excluded_resources.sort_by(|a, b| a["resource_id"].as_str().cmp(&b["resource_id"].as_str()));
    struct Walker<'a> {
        ctx: &'a ResearchExecution,
        label: &'a str,
        paths: &'a BTreeMap<usize, String>,
        resources: &'a BTreeMap<String, Value>,
        major: &'a BTreeMap<(String, String), Value>,
        excluded: &'a BTreeSet<String>,
        text: String,
        length: usize,
        units: Vec<Unit>,
    }
    impl Walker<'_> {
        fn append(&mut self, text: &str) -> Result<()> {
            self.ctx.tick(text.len() as u64)?;
            ensure(
                text.len() <= (2 * 1024 * 1024usize).saturating_sub(self.text.len()),
                "TEI rendered text cap",
            )?;
            self.length += text.chars().count();
            self.text.push_str(text);
            Ok(())
        }
        fn visit(&mut self, n: &Node, parent: Option<usize>) -> Result<()> {
            self.ctx.tick(1)?;
            let path = &self.paths[&(n as *const _ as usize)];
            let tag = n.name.as_str();
            if self.excluded.contains(path) || matches!(tag, "fw" | "note") {
                return Ok(());
            }
            if tag == "choice" {
                let children = n
                    .content
                    .iter()
                    .filter_map(|p| {
                        if let Part::Child(c) = p {
                            Some(c)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                let selected = ["sic", "orig", "abbr"]
                    .into_iter()
                    .find_map(|name| children.iter().find(|c| c.name == name).copied())
                    .or_else(|| children.first().copied());
                if let Some(c) = selected {
                    self.visit(c, parent)?;
                }
                return Ok(());
            }
            let index = if let Some(k) = kind(tag) {
                ensure(self.units.len() < UNIT_CAP, "technical unit cap")?;
                let resource = self.resources.get(path);
                let resource_id = resource
                    .map(|r| r["resource_id"].clone())
                    .unwrap_or(Value::Null);
                let correspondence = resource_id
                    .as_str()
                    .and_then(|id| self.major.get(&(self.label.to_owned(), id.to_owned())));
                let role = if tag == "body" {
                    "part_scope"
                } else if let Some(c) = correspondence {
                    if c["correspondence_id"] == "structure-i-002" {
                        "major_wrapper_candidate"
                    } else {
                        "major_reading_unit_candidate"
                    }
                } else if tag == "div" {
                    "nested_section"
                } else if matches!(tag, "cit" | "list") {
                    "paratext_container"
                } else {
                    "source_unit"
                };
                let at = self.units.len();
                self.units.push(Unit {
                    locator: path.clone(),
                    source_tag: tag.into(),
                    unit_kind: k,
                    parent,
                    children: vec![],
                    start: self.length,
                    end: self.length,
                    resource_id,
                    correspondence_id: correspondence
                        .map(|c| c["correspondence_id"].clone())
                        .unwrap_or(Value::Null),
                    correspondence_sequence: correspondence
                        .map(|c| c["sequence"].clone())
                        .unwrap_or(Value::Null),
                    structural_role: role,
                });
                if let Some(p) = parent {
                    self.units[p].children.push(at);
                }
                Some(at)
            } else {
                None
            };
            let parent = index.or(parent);
            let mut after_lb = false;
            for piece in &n.content {
                match piece {
                    Part::Text(text) => {
                        self.append(clean_text(text, after_lb))?;
                        after_lb = false;
                    }
                    Part::Child(child) => {
                        let child_path = &self.paths[&(child as *const _ as usize)];
                        if self.excluded.contains(child_path) {
                            after_lb = false;
                        } else if child.name == "lb" {
                            self.append("\n")?;
                            after_lb = true;
                        } else {
                            self.visit(child, parent)?;
                            after_lb = false;
                        }
                    }
                }
            }
            if let Some(at) = index {
                self.units[at].end = self.length;
                ensure(
                    self.units[at].unit_kind == "milestone" || self.units[at].start < self.length,
                    "empty non-milestone source span",
                )?;
            }
            Ok(())
        }
    }
    let label = s(&config["part_label"])?;
    let mut walker = Walker {
        ctx,
        label,
        paths: &xml_paths,
        resources,
        major,
        excluded: &excluded,
        text: String::new(),
        length: 0,
        units: vec![],
    };
    walker.visit(body, None)?;
    ensure(
        !walker.text.is_empty() && walker.units[0].end == walker.length,
        "part document coverage",
    )?;
    let mut positions = walker
        .text
        .char_indices()
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    positions.push(walker.text.len());
    let content = walker.text;
    let units = walker.units;
    Ok(ObservedPart {
        config,
        payload_ref,
        content,
        positions,
        units,
        excluded: excluded_resources,
    })
}
fn load_part(
    ctx: &ResearchExecution,
    input: &ResearchExecution,
    config: &Value,
    major: &BTreeMap<(String, String), Value>,
    bindings: &mut BTreeMap<String, Vec<u8>>,
) -> Result<ObservedPart> {
    let (manifest_raw, manifest) = load(ctx, s(&config["manifest_ref"])?)?;
    let (inventory_raw, inventory) = load(ctx, s(&config["resource_inventory_ref"])?)?;
    let (rights_raw, rights) = load(ctx, s(&config["rights_ref"])?)?;
    ensure(
        manifest["item_id"] == config["item_ref"]
            && manifest["embodiment_ref"] == config["edition_ref"],
        "item/edition binding drift",
    )?;
    ensure(
        array(&rights["scope_refs"])?.contains(&config["item_ref"]),
        "rights scope omits item",
    )?;
    let entries = array(&manifest["payload_files"])?
        .iter()
        .filter(|r| r["file_id"] == config["file_ref"] && r["sha256"] == config["file_sha256"])
        .collect::<Vec<_>>();
    ensure(
        entries.len() == 1 && entries[0]["media_type"] == "application/xml",
        "exact TEI payload missing",
    )?;
    let manifest_ref = s(&config["manifest_ref"])?;
    ensure(
        manifest_ref.starts_with("ToS/source-witnesses/"),
        "manifest leaves source owner",
    )?;
    let parent = Path::new(manifest_ref).parent().ok_or("manifest parent")?;
    let payload_ref = parent
        .join(s(&entries[0]["relative_path"])?)
        .to_str()
        .ok_or("payload reference UTF8")?
        .to_owned();
    RelativePath::parse(&payload_ref).map_err(|e| e.to_string())?;
    let mut file = input.source_file(&payload_ref, 2 * 1024 * 1024)?;
    let raw = input.read_file(&mut file, 2 * 1024 * 1024)?;
    ensure(
        raw.len() as u64 == number(&entries[0]["byte_size"])?
            && sha(&raw) == s(&config["file_sha256"])?,
        "exact TEI size/fixity drift",
    )?;
    let files = array(&inventory["files"])?
        .iter()
        .filter(|r| r["file_id"] == config["file_ref"] && r["file_sha256"] == config["file_sha256"])
        .collect::<Vec<_>>();
    ensure(
        files.len() == 1 && files[0]["profile"] == "tei_structure_v1",
        "one inventory TEI file required",
    )?;
    let mut resources = BTreeMap::new();
    for r in array(&files[0]["resources"])? {
        if let Some(path) = r["locator"]["tei_path"].as_str() {
            ensure(
                resources.insert(path.to_owned(), r.clone()).is_none(),
                "duplicate inventory TEI path",
            )?;
        }
    }
    for (key, raw) in [
        ("manifest_ref", manifest_raw),
        ("resource_inventory_ref", inventory_raw),
        ("rights_ref", rights_raw),
    ] {
        bindings.insert(s(&config[key])?.to_owned(), raw);
    }
    observe(ctx, config.clone(), payload_ref, &raw, &resources, major)
}
fn mint(kind: &str, sequence: usize) -> String {
    let digest = sha(format!("{ID_NAMESPACE}:issued:{sequence}").as_bytes());
    if kind == "anchor" {
        format!("tos.anchor.zarathustra-technical-v1.sid-{}", &digest[..32])
    } else {
        format!("tos.{kind}.sid-{}", &digest[..32])
    }
}
fn issue(plan: &Value, parts: &[ObservedPart]) -> Value {
    let mut issued = 0;
    let mut part_rows = Vec::new();
    let mut unit_rows = Vec::new();
    for part in parts {
        let mut row = json!({"part_order":part.config["part_order"]});
        for (field, kind) in [
            ("packet_id", "source-text-unit-packet"),
            ("scheme_id", "text-unit-scheme"),
            ("segmentation_id", "text-segmentation"),
            ("projection_id", "text-unit-projection"),
        ] {
            issued += 1;
            row[field] = json!(mint(kind, issued));
            row[format!("{field}_issued_sequence")] = json!(issued);
        }
        part_rows.push(row);
        for unit in &part.units {
            issued += 1;
            let unit_seq = issued;
            let unit_id = mint("text-unit", issued);
            issued += 1;
            unit_rows.push(json!({"part_order":part.config["part_order"],"source_locator":unit.locator,"unit_id":unit_id,"unit_id_issued_sequence":unit_seq,"anchor_ref":mint("anchor",issued),"anchor_ref_issued_sequence":issued}));
        }
    }
    json!({"schema_version":"tos_zarathustra_technical_identity_issuance_v1","issuance_id":"tos.identity-issuance.zarathustra-dta-technical-markup-v1","created_at":plan["created_at"],"identity_policy":plan["identity_policy"]["policy"],"namespace_probe_sha256":sha(ID_NAMESPACE.as_bytes()),"ids_minted_from":"issuance-namespace-and-issued-sequence-only","source_locator_role":"binding-only-not-identity-input","automatic_remint_on_locator_drift":false,"part_identities":part_rows,"unit_identities":unit_rows,"issued_identity_count":issued,"authority_boundary":"This journal records opaque identity issuance for technical source units and the source bindings used to retain them."})
}
type Identities = (BTreeMap<u64, Value>, BTreeMap<(u64, String), Value>);
fn identities(issuance: &Value) -> Result<Identities> {
    let mut part = BTreeMap::new();
    let mut units = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for row in array(&issuance["part_identities"])? {
        ensure(
            part.insert(number(&row["part_order"])?, row.clone())
                .is_none(),
            "duplicate part identity",
        )?;
        for k in ["packet_id", "scheme_id", "segmentation_id", "projection_id"] {
            ensure(
                ids.insert(s(&row[k])?.to_owned()),
                "duplicate part opaque identity",
            )?;
        }
    }
    for row in array(&issuance["unit_identities"])? {
        let key = (
            number(&row["part_order"])?,
            s(&row["source_locator"])?.to_owned(),
        );
        ensure(
            units.insert(key, row.clone()).is_none(),
            "duplicate unit binding",
        )?;
        for k in ["unit_id", "anchor_ref"] {
            ensure(
                ids.insert(s(&row[k])?.to_owned()),
                "duplicate issued unit/anchor identity",
            )?;
        }
    }
    Ok((part, units))
}
fn citations(
    ctx: &ResearchExecution,
    plan: &Value,
    parts: &[ObservedPart],
    ids: &Identities,
) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    let mut displayed = BTreeSet::new();
    for part in parts {
        let order = number(&part.config["part_order"])?;
        let label = s(&part.config["part_label"])?;
        let mut citations = Vec::<String>::new();
        let mut counters = BTreeMap::new();
        let mut major_counters = BTreeMap::new();
        let mut descendants = vec![(0usize, 0usize); part.units.len()];
        for i in (0..part.units.len()).rev() {
            for &child in &part.units[i].children {
                descendants[i].0 +=
                    descendants[child].0 + usize::from(part.units[child].unit_kind == "paragraph");
                descendants[i].1 +=
                    descendants[child].1 + usize::from(part.units[child].unit_kind == "verse_line");
            }
        }
        for (i, u) in part.units.iter().enumerate() {
            ctx.tick(1)?;
            let (citation, ordinal) = if u.unit_kind == "document" {
                (format!("Za-DE-{label}"), 1)
            } else {
                let prefix = match u.unit_kind {
                    "section" => "s",
                    "paragraph" => "p",
                    "verse_group" => "vg",
                    "verse_line" => "v",
                    "milestone" => {
                        if u.source_tag == "pb" {
                            "pb"
                        } else {
                            "m"
                        }
                    }
                    _ => match u.source_tag.as_str() {
                        "head" => "h",
                        "item" => "item",
                        "quote" => "q",
                        "bibl" => "b",
                        _ => "u",
                    },
                };
                let ordinal = counters.entry((u.parent, prefix)).or_insert(0);
                *ordinal += 1;
                (
                    format!(
                        "{}.{prefix}{:03}",
                        citations[u.parent.ok_or("citation parent absent")?],
                        ordinal
                    ),
                    *ordinal,
                )
            };
            ensure(
                displayed.insert(citation.clone()),
                "display citation collision",
            )?;
            citations.push(citation.clone());
            let identity = &ids.1[&(order, u.locator.clone())];
            let parent = u
                .parent
                .map(|p| ids.1[&(order, part.units[p].locator.clone())]["unit_id"].clone());
            let mut cursor = Some(i);
            while let Some(at) = cursor {
                if part.units[at].structural_role == "major_reading_unit_candidate" {
                    break;
                }
                cursor = part.units[at].parent;
            }
            let major = cursor.map(|at| &part.units[at]);
            let major_id = major.map(|m| ids.1[&(order, m.locator.clone())]["unit_id"].clone());
            let major_ordinal = cursor.map(|at| {
                let n = major_counters.entry((at, u.unit_kind)).or_insert(0);
                *n += 1;
                *n
            });
            let children = u
                .children
                .iter()
                .map(|&c| &part.units[c])
                .collect::<Vec<_>>();
            rows.push(json!({"schema_version":"tos_zarathustra_technical_citation_v1","work_ref":plan["work_ref"],"part_order":order,"part_label":label,"unit_id":identity["unit_id"],"anchor_ref":identity["anchor_ref"],"parent_unit_id":parent,"unit_kind":u.unit_kind,"source_tag":u.source_tag,"structural_role":u.structural_role,"display_citation":citation,"ordinal_within_parent_kind":ordinal,"nearest_major_unit_id":major_id,"nearest_major_correspondence_id":major.map(|m|&m.correspondence_id),"ordinal_within_nearest_major_kind":major_ordinal,"direct_child_count":children.len(),"direct_paragraph_count":children.iter().filter(|u|u.unit_kind=="paragraph").count(),"descendant_paragraph_count":descendants[i].0,"direct_verse_line_count":children.iter().filter(|u|u.unit_kind=="verse_line").count(),"descendant_verse_line_count":descendants[i].1,"source_locator":u.locator,"source_resource_id":u.resource_id,"major_correspondence_id":u.correspondence_id,"major_correspondence_sequence":u.correspondence_sequence,"boundary_posture":"source_attested","segmentation_status":"observed_source_structure","source_text_included":false,"semantic_promotion":false}));
        }
    }
    Ok(rows)
}
fn packet(
    ctx: &ResearchExecution,
    plan: &Value,
    part: &ObservedPart,
    ids: &Identities,
    citation_ref: &str,
    citation_digest: &str,
    method: &Value,
    historical: bool,
) -> Result<Value> {
    let order = number(&part.config["part_order"])?;
    let private_ref = pattern(plan, "private_layer_pattern", order)?;
    let content_digest = sha(part.content.as_bytes());
    let mut anchors = Vec::new();
    let mut units = Vec::new();
    for (i, u) in part.units.iter().enumerate() {
        ctx.tick(1)?;
        let id = &ids.1[&(order, u.locator.clone())];
        let role = match u.unit_kind {
            "document" => "scope",
            "milestone" => "milestone",
            _ => "content",
        };
        let exact = &part.content[part.positions[u.start]..part.positions[u.end]];
        anchors.push(records::anchor(
            id,
            i + 1,
            &private_ref,
            &content_digest,
            u,
            &part.payload_ref,
            exact,
            role,
        ));
        let parents = u
            .parent
            .map(|p| ids.1[&(order, part.units[p].locator.clone())]["unit_id"].clone())
            .into_iter()
            .collect::<Vec<_>>();
        let children = u
            .children
            .iter()
            .map(|&c| ids.1[&(order, part.units[c].locator.clone())]["unit_id"].clone())
            .collect::<Vec<_>>();
        let mut value = records::unit(id, u, &json!(parents), &json!(children));
        if historical {
            value["status_reason"] = json!(LEGACY_UNIT_REASON);
        }
        units.push(value);
    }
    let result = records::packet(
        plan,
        &part.config,
        &ids.0[&order],
        &content_digest,
        &private_ref,
        method,
        &anchors,
        &units,
        citation_ref,
        citation_digest,
    );
    validate_packet(ctx, &result, Some((&part.content, &part.positions)))?;
    Ok(result)
}
fn forbidden(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.iter().any(|(k, v)| {
            [
                "lemma",
                "lexeme",
                "sense",
                "sign",
                "concept",
                "semantic_relation",
                "translation_alignment",
                "canon_ref",
            ]
            .contains(&k.as_str())
                || forbidden(v)
        }),
        Value::Array(a) => a.iter().any(forbidden),
        _ => false,
    }
}
fn validate_packet(
    ctx: &ResearchExecution,
    p: &Value,
    text: Option<(&str, &[usize])>,
) -> Result<()> {
    crate::source_text_foundation::unit_packet_schema(ctx, p)?;
    let mut units = BTreeMap::new();
    let mut anchors = BTreeMap::new();
    for u in array(&p["units"])? {
        ensure(
            units.insert(s(&u["unit_id"])?, u).is_none(),
            "duplicate unit identity",
        )?;
    }
    for a in array(&p["anchors"])? {
        ensure(
            anchors.insert(s(&a["anchor_ref"])?, a).is_none(),
            "duplicate anchor identity",
        )?;
        let start = number(&a["selector"]["start"])? as usize;
        let end = number(&a["selector"]["end"])? as usize;
        ensure(start <= end, "anchor order")?;
        if let Some((text, positions)) = text {
            ensure(end < positions.len(), "anchor leaves text layer")?;
            ensure(
                a["exact_sha256"] == sha(text[positions[start]..positions[end]].as_bytes()),
                "anchor exact bytes differ",
            )?;
        }
    }
    ensure(
        units.len() == anchors.len(),
        "one anchor per technical unit",
    )?;
    for (id, u) in &units {
        ctx.tick(1)?;
        for p in array(&u["parent_unit_refs"])? {
            let parent = units.get(s(p)?).ok_or("unit parent absent")?;
            ensure(
                array(&parent["ordered_child_unit_refs"])?
                    .iter()
                    .any(|v| v == *id),
                "parent/child not reciprocal",
            )?;
        }
        for c in array(&u["ordered_child_unit_refs"])? {
            let child = units.get(s(c)?).ok_or("unit child absent")?;
            ensure(
                array(&child["parent_unit_refs"])?.iter().any(|v| v == *id),
                "child/parent not reciprocal",
            )?;
        }
    }
    ensure(!forbidden(p), "semantic field escaped technical packet")
}
fn count<'a>(iter: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for k in iter {
        *out.entry(k.to_owned()).or_insert(0) += 1;
    }
    out
}
fn summary(plan: &Value, parts: &[ObservedPart], citations: &[Value]) -> Result<Value> {
    let mut part_rows = Vec::new();
    let kinds = count(
        parts
            .iter()
            .flat_map(|p| p.units.iter().map(|u| u.unit_kind)),
    );
    let tags = count(
        parts
            .iter()
            .flat_map(|p| p.units.iter().map(|u| u.source_tag.as_str())),
    );
    for p in parts {
        let kinds = count(p.units.iter().map(|u| u.unit_kind));
        let tags = count(p.units.iter().map(|u| u.source_tag.as_str()));
        let roles = count(p.units.iter().map(|u| u.structural_role));
        let actual = json!({"major_structure_candidates":roles.get("major_reading_unit_candidate").unwrap_or(&0)+roles.get("major_wrapper_candidate").unwrap_or(&0),"divisions":tags.get("div").unwrap_or(&0),"paragraphs":kinds.get("paragraph").unwrap_or(&0),"verse_groups":kinds.get("verse_group").unwrap_or(&0),"verse_lines":kinds.get("verse_line").unwrap_or(&0)});
        ensure(
            plan["expected_counts"]["part_counts"][s(&p.config["part_label"])?] == actual,
            "part technical counts drifted",
        )?;
        part_rows.push(json!({"part_order":p.config["part_order"],"part_label":p.config["part_label"],"item_ref":p.config["item_ref"],"source_file_sha256":p.config["file_sha256"],"private_text_layer_sha256":sha(p.content.as_bytes()),"private_text_layer_code_points":p.positions.len()-1,"unit_count":p.units.len(),"unit_kind_counts":kinds,"source_tag_counts":tags,"structural_role_counts":roles,"excluded_resources":p.excluded,"source_text_included":false}));
    }
    let value = json!({"schema_version":"tos_zarathustra_dta_technical_markup_summary_v1","work_ref":plan["work_ref"],"language":"de","part_count":parts.len(),"unit_count":parts.iter().map(|p|p.units.len()).sum::<usize>(),"citation_count":citations.len(),"display_citations_unique":citations.iter().map(|r|r["display_citation"].as_str()).collect::<BTreeSet<_>>().len()==citations.len(),"unit_kind_counts":kinds,"source_tag_counts":tags,"major_structure_candidate_count":citations.iter().filter(|r|!r["major_correspondence_id"].is_null()).count(),"major_reading_unit_candidate_count":citations.iter().filter(|r|r["structural_role"]=="major_reading_unit_candidate").count(),"major_wrapper_candidate_count":citations.iter().filter(|r|r["structural_role"]=="major_wrapper_candidate").count(),"parts":part_rows,"source_text_included":false,"segmentation_status":"observed_source_structure","human_review_status":"unreviewed","semantic_fields_materialized":false,"russian_parallel_structure_materialized":false,"authority_boundary":plan["authority_boundary"]});
    let actual = json!({"parts":parts.len(),"major_structure_candidates":value["major_structure_candidate_count"],"reading_units_excluding_wrapper":value["major_reading_unit_candidate_count"],"divisions":tags.get("div").unwrap_or(&0),"paragraphs":kinds.get("paragraph").unwrap_or(&0),"verse_groups":kinds.get("verse_group").unwrap_or(&0),"verse_lines":kinds.get("verse_line").unwrap_or(&0)});
    for (k, v) in actual.as_object().unwrap() {
        ensure(
            plan["expected_counts"][k] == *v,
            &format!("technical count drift: {k}"),
        )?;
    }
    Ok(value)
}
fn schema(ctx: &ResearchExecution, path: &str, value: &Value) -> Result<()> {
    crate::source_text_foundation::schema(ctx, path, value)
}
fn current_plan(ctx: &ResearchExecution, reference: &str) -> Result<(Vec<u8>, Value)> {
    let (raw, plan) = load(ctx, reference)?;
    ensure(
        plan["schema_version"] == "tos_zarathustra_dta_technical_markup_plan_v1"
            && plan["contract_ref"] == SCHEMA
            && plan["language"] == "de",
        "unsupported DTA technical plan",
    )?;
    ensure(
        plan["identity_policy"]["source_locator_is_binding_not_identity"] == true
            && plan["identity_policy"]["automatic_remint_on_locator_drift"] == false,
        "identity policy drift",
    )?;
    ensure(
        plan["extraction_policy"]
            == json!({"included_region":"TEI/text[1]/body[1] minus exact excluded resource subtrees or milestones","excluded_elements":["fw","note"],"choice_policy":"prefer-sic-then-orig-then-abbr-then-first","line_break_policy":"render-lb-as-one-U+000A-and-drop-one-XML-formatting-newline-from-its-tail","formatting_whitespace_policy":"drop-whitespace-only-node-content-containing-newline-carriage-return-or-tab; preserve-space-only-content","unicode_normalization":"none","source_text_mutation_authorized":false}),
        "unsupported extraction policy",
    )?;
    let sources = array(&plan["source_items"])?;
    let orders = sources
        .iter()
        .map(|c| number(&c["part_order"]))
        .collect::<Result<BTreeSet<_>>>()?;
    ensure(
        sources.len() == 4 && orders == BTreeSet::from([1, 2, 3, 4]),
        "technical source must declare four distinct parts",
    )?;
    for c in sources {
        ensure(
            c["part_label"] == ["I", "II", "III", "IV"][(number(&c["part_order"])? - 1) as usize],
            "part order/label differs",
        )?;
    }
    Ok((raw, plan))
}
fn retained_event(
    ctx: &ResearchExecution,
    plan: &Value,
    opts: &Options<'_>,
) -> Result<Option<(Vec<u8>, Value)>> {
    let reference = out(plan, "provenance_ref")?;
    let exists = match std::fs::symlink_metadata(ctx.root().join(&reference)) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.to_string()),
    };
    if !exists {
        return Ok(None);
    }
    let (raw, events) = lines(ctx, &reference)?;
    ensure(
        events.len() == 1,
        "technical provenance must contain one event",
    )?;
    let event = events.into_iter().next().unwrap();
    schema(ctx, PROVENANCE_SCHEMA, &event)?;
    if let Some(id) = opts.event_id {
        ensure(
            event["event_id"] == id,
            "selected event differs from retained provenance",
        )?;
    }
    if event["event_id"] == LEGACY_EVENT {
        ensure(
            sha(&raw) == LEGACY_EVENT_SHA,
            "historical provenance must retain exact original bytes",
        )?;
    }
    Ok(Some((raw, event)))
}
fn event_inputs(
    ctx: &ResearchExecution,
    plan: &Value,
    plan_ref: &str,
    issuance_digest: &str,
) -> Result<Vec<Value>> {
    let mut inputs = vec![
        json!({"ref":plan_ref,"role":"tracked-technical-markup-plan","sha256":sha(&ctx.read(plan_ref)?)}),
        json!({"ref":plan["identity_policy"]["issuance_ref"],"role":"tracked-opaque-identity-issuance","sha256":issuance_digest}),
        json!({"ref":plan["structure_correspondence_ref"],"role":"tracked-major-structure-candidate-map","sha256":sha(&ctx.read(s(&plan["structure_correspondence_ref"])?)?)}),
    ];
    let mut parts = array(&plan["source_items"])?.iter().collect::<Vec<_>>();
    parts.sort_by_key(|c| c["part_order"].as_u64());
    for p in parts {
        let n = number(&p["part_order"])?;
        inputs.push(json!({"ref":p["file_ref"],"role":format!("local-fixity-bound-dta-part-{n}-tei"),"sha256":p["file_sha256"]}));
        for (key, role) in [
            (
                "resource_inventory_ref",
                format!("tracked-text-free-part-{n}-resource-inventory"),
            ),
            ("rights_ref", format!("tracked-part-{n}-rights-record")),
        ] {
            inputs.push(json!({"ref":p[key],"role":role,"sha256":sha(&ctx.read(s(&p[key])?)?)}));
        }
    }
    Ok(inputs)
}
fn event_closure(
    ctx: &ResearchExecution,
    plan: &Value,
    plan_ref: &str,
    event: &Value,
    issuance_digest: &str,
    outputs: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    schema(ctx, PROVENANCE_SCHEMA, event)?;
    let outputs:Vec<_>=outputs.iter().map(|(r,b)|json!({"ref":r,"role":"tracked-text-free-technical-markup-artifact","sha256":sha(b)})).collect();
    let refs = outputs.iter().map(|v| v["ref"].clone()).collect::<Vec<_>>();
    ensure(
        event["inputs"] == json!(event_inputs(ctx, plan, plan_ref, issuance_digest)?)
            && event["outputs"] == json!(outputs)
            && event["receipt_refs"] == json!(refs),
        "technical provenance input/output closure drift",
    )?;
    ensure(
        event["event_type"] == "segmentation"
            && event["status"] == "completed_with_warnings"
            && event["method"]["maker_type"] == "software"
            && event["method"]["prompt_or_instruction_ref"] == plan_ref
            && event["rights_basis_ref"] == plan["source_items"][0]["rights_ref"],
        "technical provenance method/rights drift",
    )?;
    for key in [
        "source_text_tracked",
        "semantic_fields_materialized",
        "russian_parallel_structure_materialized",
    ] {
        ensure(
            event["method"]["configuration"][key] == false,
            "provenance authority drift",
        )?;
    }
    Ok(())
}
fn tracked(ctx: &ResearchExecution, plan: &Value, plan_ref: &str, event: &Value) -> Result<Value> {
    let (issuance_raw, issuance) = load(ctx, s(&plan["identity_policy"]["issuance_ref"])?)?;
    let ids = identities(&issuance)?;
    ensure(
        ids.0.keys().copied().collect::<BTreeSet<_>>() == BTreeSet::from([1, 2, 3, 4]),
        "tracked part identity set drift",
    )?;
    let citation_ref = out(plan, "citation_spine_ref")?;
    let (citation_raw, citations) = lines(ctx, &citation_ref)?;
    ensure(!citations.is_empty(), "empty citation spine")?;
    let mut unit_ids = BTreeSet::new();
    let mut display = BTreeSet::new();
    let mut bindings = BTreeMap::new();
    for c in &citations {
        ensure(
            unit_ids.insert(s(&c["unit_id"])?) && display.insert(s(&c["display_citation"])?),
            "tracked citation identity/citation collision",
        )?;
        ensure(
            c["source_text_included"] == false && c["semantic_promotion"] == false && !forbidden(c),
            "tracked citation boundary widened",
        )?;
        ensure(
            bindings
                .insert(
                    (
                        number(&c["part_order"])?,
                        s(&c["source_locator"])?.to_owned(),
                    ),
                    (c["unit_id"].clone(), c["anchor_ref"].clone()),
                )
                .is_none(),
            "tracked citation locator collision",
        )?;
    }
    ensure(
        bindings
            == ids
                .1
                .iter()
                .map(|(k, v)| (k.clone(), (v["unit_id"].clone(), v["anchor_ref"].clone())))
                .collect(),
        "citation and issuance binding differ",
    )?;
    let mut outputs = BTreeMap::from([(citation_ref.clone(), citation_raw)]);
    let citation_digest = sha(&outputs[&citation_ref]);
    for source in array(&plan["source_items"])? {
        let n = number(&source["part_order"])?;
        let reference = pattern(plan, "part_packet_pattern", n)?;
        let (raw, p) = load(ctx, &reference)?;
        validate_packet(ctx, &p, None)?;
        let fixed = &ids.0[&n];
        ensure(
            p["packet_id"] == fixed["packet_id"]
                && p["schemes"][0]["scheme_id"] == fixed["scheme_id"]
                && p["segmentations"][0]["segmentation_id"] == fixed["segmentation_id"],
            "tracked packet identity drift",
        )?;
        let projection = &p["projections"][0];
        ensure(
            projection["projection_id"] == fixed["projection_id"]
                && projection["artifact_ref"] == citation_ref
                && projection["artifact_sha256"] == citation_digest,
            "tracked citation projection differs",
        )?;
        for (list, key) in [("units", "unit_id"), ("anchors", "anchor_ref")] {
            let actual = array(&p[list])?
                .iter()
                .map(|v| s(&v[key]))
                .collect::<Result<BTreeSet<_>>>()?;
            let expected = citations
                .iter()
                .filter(|c| c["part_order"] == n)
                .map(|c| s(&c[key]))
                .collect::<Result<BTreeSet<_>>>()?;
            ensure(
                actual == expected,
                "tracked citation packet closure differs",
            )?;
        }
        for key in [
            "expression_ref",
            "edition_ref",
            "item_ref",
            "file_ref",
            "file_sha256",
        ] {
            ensure(
                p["source_scope"][key] == source[key],
                "tracked packet source binding differs",
            )?;
        }
        let (_, rights) = load(ctx, s(&source["rights_ref"])?)?;
        ensure(
            array(&rights["scope_refs"])?.contains(&source["item_ref"]),
            "current rights omit source",
        )?;
        let (_, manifest) = load(ctx, s(&source["manifest_ref"])?)?;
        ensure(
            manifest["item_id"] == source["item_ref"]
                && manifest["embodiment_ref"] == source["edition_ref"]
                && array(&manifest["payload_files"])?.iter().any(|f| {
                    f["file_id"] == source["file_ref"] && f["sha256"] == source["file_sha256"]
                }),
            "tracked manifest source binding differs",
        )?;
        outputs.insert(reference, raw);
    }
    let summary_ref = out(plan, "summary_ref")?;
    let (raw, summary) = load(ctx, &summary_ref)?;
    ensure(
        summary["unit_count"] == citations.len(),
        "tracked summary count differs",
    )?;
    let observed = [
        (
            "major_structure_candidates",
            citations
                .iter()
                .filter(|v| !v["major_correspondence_id"].is_null())
                .count(),
        ),
        (
            "reading_units_excluding_wrapper",
            citations
                .iter()
                .filter(|v| v["structural_role"] == "major_reading_unit_candidate")
                .count(),
        ),
        (
            "divisions",
            citations
                .iter()
                .filter(|v| v["source_tag"] == "div")
                .count(),
        ),
        (
            "paragraphs",
            citations
                .iter()
                .filter(|v| v["unit_kind"] == "paragraph")
                .count(),
        ),
        (
            "verse_groups",
            citations
                .iter()
                .filter(|v| v["unit_kind"] == "verse_group")
                .count(),
        ),
        (
            "verse_lines",
            citations
                .iter()
                .filter(|v| v["unit_kind"] == "verse_line")
                .count(),
        ),
    ];
    for (k, n) in observed {
        ensure(
            plan["expected_counts"][k] == n,
            &format!("tracked count differs: {k}"),
        )?;
    }
    ensure(
        !citations.iter().any(|v| {
            v["part_label"] == "IV"
                && ["tei-pb-0143", "tei-pb-0144", "tei-div-0070"]
                    .iter()
                    .any(|r| v["source_resource_id"] == *r)
        }),
        "Part IV auxiliary exclusion escaped",
    )?;
    outputs.insert(summary_ref, raw);
    event_closure(ctx, plan, plan_ref, event, &sha(&issuance_raw), &outputs)?;
    Ok(summary)
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let (plan_raw, plan) = current_plan(ctx, opts.plan_ref)?;
    let retained = retained_event(ctx, &plan, &opts)?;
    let build = matches!(opts.action, Action::Build { .. });
    if matches!(opts.action, Action::ValidateTracked) {
        let event = &retained.as_ref().ok_or("tracked provenance absent")?.1;
        let summary = tracked(ctx, &plan, opts.plan_ref, event)?;
        ctx.check()?;
        return Ok(
            json!({"status":"passed","mode":"validate-tracked","summary":summary,"private_text_read":false,"publication_authorized":false}),
        );
    }
    ensure(
        build || retained.is_some(),
        "check requires retained provenance",
    )?;
    let event_id = if let Some((_, e)) = &retained {
        s(&e["event_id"])?.to_owned()
    } else {
        let id = opts
            .event_id
            .ok_or("fresh native output requires a new --event-id")?;
        ensure(
            id != LEGACY_EVENT && id.starts_with("tos.event.") && id.len() < 512,
            "new event identity",
        )?;
        id.to_owned()
    };
    let historical = event_id == LEGACY_EVENT;
    let made_at = if historical {
        s(&plan["created_at"])?.to_owned()
    } else if let Some((_, e)) = &retained {
        s(&e["ended_at"])?.to_owned()
    } else {
        utc_now()?
    };
    let input = ctx.select_directory(opts.input_root.ok_or("local input root required")?)?;
    let output = ctx.select_directory(opts.output_root.ok_or("local output root required")?)?;
    let (major_raw, major_doc) = load(ctx, s(&plan["structure_correspondence_ref"])?)?;
    ensure(
        major_doc["work_ref"] == plan["work_ref"],
        "major correspondence work differs",
    )?;
    let mut major = BTreeMap::new();
    for row in array(&major_doc["correspondences"])? {
        let key = (
            s(&row["part_label"])?.to_owned(),
            s(&row["source"]["resource_id"])?.to_owned(),
        );
        ensure(
            major.insert(key, row.clone()).is_none(),
            "duplicate major correspondence",
        )?;
    }
    ensure(
        major.len() as u64 == number(&plan["expected_counts"]["major_structure_candidates"])?,
        "major correspondence count differs",
    )?;
    let mut bindings = BTreeMap::from([
        (opts.plan_ref.to_owned(), plan_raw),
        (
            s(&plan["structure_correspondence_ref"])?.to_owned(),
            major_raw,
        ),
    ]);
    let mut configs = array(&plan["source_items"])?.iter().collect::<Vec<_>>();
    configs.sort_by_key(|c| c["part_order"].as_u64());
    let mut parts = Vec::new();
    for config in configs {
        parts.push(load_part(ctx, &input, config, &major, &mut bindings)?);
    }
    let issuance_ref = s(&plan["identity_policy"]["issuance_ref"])?;
    let issue_new = matches!(
        opts.action,
        Action::Build {
            issue_identities: true
        }
    );
    let (issuance_raw, issuance) = if issue_new {
        ensure(
            !ctx.root()
                .join(issuance_ref)
                .try_exists()
                .map_err(|e| e.to_string())?,
            "identity issuance already exists; automatic remint forbidden",
        )?;
        let value = issue(&plan, &parts);
        (encode(ctx, &value, true)?, value)
    } else {
        let (raw, v) = load(ctx, issuance_ref)?;
        ensure(
            raw == encode(ctx, &v, true)?,
            "issuance is not canonical JSON",
        )?;
        bindings.insert(issuance_ref.to_owned(), raw.clone());
        (raw, v)
    };
    let ids = identities(&issuance)?;
    let expected = parts
        .iter()
        .flat_map(|p| {
            p.units
                .iter()
                .map(|u| (p.config["part_order"].as_u64().unwrap(), u.locator.clone()))
        })
        .collect::<BTreeSet<_>>();
    ensure(
        ids.1.keys().cloned().collect::<BTreeSet<_>>() == expected
            && ids.0.keys().copied().collect::<BTreeSet<_>>() == BTreeSet::from([1, 2, 3, 4]),
        "locator identity set drifted; explicit successor work required",
    )?;
    let citations = citations(ctx, &plan, &parts, &ids)?;
    let citation_raw = jsonl(ctx, &citations)?;
    let citation_digest = sha(&citation_raw);
    let citation_ref = out(&plan, "citation_spine_ref")?;
    let mut tracked = BTreeMap::from([(citation_ref.clone(), citation_raw)]);
    let mut private = BTreeMap::new();
    let builder = if historical { LEGACY_BUILDER } else { BUILDER };
    let method = records::method(opts.plan_ref, builder, &event_id, &made_at);
    for p in &parts {
        let n = number(&p.config["part_order"])?;
        let packet = packet(
            ctx,
            &plan,
            p,
            &ids,
            &citation_ref,
            &citation_digest,
            &method,
            historical,
        )?;
        let reference = pattern(&plan, "part_packet_pattern", n)?;
        ensure(
            tracked
                .insert(reference, encode(ctx, &packet, true)?)
                .is_none(),
            "duplicate tracked output",
        )?;
        let reference = pattern(&plan, "private_layer_pattern", n)?;
        private_boundary(ctx, &reference)?;
        ensure(
            private
                .insert(reference, p.content.as_bytes().to_vec())
                .is_none(),
            "duplicate private output",
        )?;
    }
    let summary = summary(&plan, &parts, &citations)?;
    ensure(
        tracked
            .insert(out(&plan, "summary_ref")?, encode(ctx, &summary, true)?)
            .is_none(),
        "duplicate summary output",
    )?;
    let event = if let Some((_, e)) = &retained {
        e.clone()
    } else {
        let inputs = event_inputs(ctx, &plan, opts.plan_ref, &sha(&issuance_raw))?;
        let outputs=tracked.iter().map(|(r,b)|json!({"ref":r,"role":"tracked-text-free-technical-markup-artifact","sha256":sha(b)})).collect::<Vec<_>>();
        let refs = tracked.keys().cloned().collect::<Vec<_>>();
        let mut event = records::event(
            opts.plan_ref,
            &event_id,
            &made_at,
            parts.len(),
            &sha(include_bytes!("dta_technical_markup.rs")),
            &inputs,
            &outputs,
            &refs,
            &parts[0].config["rights_ref"],
        );
        event["agent_refs"] = json!(["software:tos-zarathustra-technical-markup-builder"]);
        event["method"]["runtime"] = json!(concat!(
            "Tree-of-Sophia native/Rust ",
            env!("CARGO_PKG_VERSION")
        ));
        event
    };
    event_closure(
        ctx,
        &plan,
        opts.plan_ref,
        &event,
        &sha(&issuance_raw),
        &tracked,
    )?;
    let event_raw = if let Some((raw, _)) = &retained {
        raw.clone()
    } else {
        jsonl(ctx, &[event])?
    };
    ensure(
        tracked
            .insert(out(&plan, "provenance_ref")?, event_raw)
            .is_none(),
        "duplicate provenance output",
    )?;
    if issue_new {
        ensure(
            tracked
                .insert(issuance_ref.to_owned(), issuance_raw)
                .is_none(),
            "issuance aliases output",
        )?;
    }
    let mut names = BTreeSet::new();
    for reference in tracked.keys().chain(private.keys()) {
        RelativePath::parse(reference).map_err(|e| e.to_string())?;
        ensure(
            reference.starts_with("ToS/source-witnesses/")
                && names.insert(reference)
                && !bindings.contains_key(reference)
                && !parts.iter().any(|p| &p.payload_ref == reference),
            "output aliases input/other output or leaves source owner",
        )?;
    }
    // Captured source and rights must still be current before any output write.
    for (r, raw) in &bindings {
        ensure(ctx.read(r)? == *raw, "metadata input changed before output")?;
    }
    for p in &parts {
        let mut f = input.source_file(&p.payload_ref, 2 * 1024 * 1024)?;
        ensure(
            input.hash_file(&mut f, 2 * 1024 * 1024)? == s(&p.config["file_sha256"])?,
            "TEI input changed before output",
        )?;
    }
    let mut write_tracked = Vec::new();
    let mut write_private = Vec::new();
    for (r, raw) in &tracked {
        let missing = crate::source_text_foundation::fresh_or_matching_limit(ctx, r, raw, CAP)?;
        ensure(build || !missing, "tracked output absent")?;
        if missing {
            write_tracked.push(r);
        }
    }
    for (r, raw) in &private {
        let missing = crate::source_text_foundation::fresh_or_matching_limit(&output, r, raw, CAP)?;
        ensure(build || !missing, "private output absent")?;
        if missing {
            write_private.push(r);
        } else {
            let f = output.source_file(r, CAP as u64)?;
            ensure(
                f.metadata()
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode()
                    & 0o077
                    == 0,
                "private output mode wider than0600",
            )?;
        }
    }
    if build {
        for r in write_private {
            output.write(r, &private[r], 0o600, true)?;
        }
        for r in write_tracked {
            ctx.write(r, &tracked[r], 0o644, true)?;
        }
    }
    ctx.check()?;
    Ok(
        json!({"status":"passed","mode":if build{"build"}else{"check"},"native_executor":"tos dta-technical-markup","event_id":event_id,"historical_provenance_preserved":historical,"summary":summary,"tracked_output_count":tracked.len(),"private_layer_count":private.len(),"publication_authorized":false,"canon_effect":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_parallel_readiness_keeps_witnesses_and_authority_separate() {
        let readiness: Value = serde_json::from_str(include_str!("../../../../ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/technical-markup/parallel-technical-readiness.v1.json")).unwrap();
        assert_eq!(readiness["verification_status"], "agent_verified_complete_technical_parallel");
        let german = &readiness["witnesses"]["german_dta_first_editions"];
        let russian = &readiness["witnesses"]["russian_antonovsky_1911"];
        assert_eq!(german["reading_unit_count"], 81);
        assert_eq!(russian["reading_unit_count"], 81);
        assert_eq!(german["reading_units_by_part"], russian["reading_units_by_part"]);
        assert!(readiness["checks"].as_object().unwrap().values().all(|v| v == true));
        assert_eq!(readiness["authority_boundary"]["technical_parallelism"], true);
        for field in ["translation_alignment_created", "ordinal_equality_creates_correspondence", "linguistic_or_semantic_authority"] {
            assert_eq!(readiness["authority_boundary"][field], false);
        }
    }
    fn fixture() -> Value {
        serde_json::from_str(include_str!("dta_technical_markup/synthetic-parity.json")).unwrap()
    }
    fn observed(ctx: &ResearchExecution, f: &Value) -> ObservedPart {
        let resources = array(&f["resources"])
            .unwrap()
            .iter()
            .map(|r| (s(&r["locator"]["tei_path"]).unwrap().to_owned(), r.clone()))
            .collect();
        let major = array(&f["major"])
            .unwrap()
            .iter()
            .map(|r| {
                (
                    (
                        s(&r["part_label"]).unwrap().to_owned(),
                        s(&r["resource_id"]).unwrap().to_owned(),
                    ),
                    r["value"].clone(),
                )
            })
            .collect();
        observe(
            ctx,
            f["config"].clone(),
            s(&f["payload_ref"]).unwrap().into(),
            s(&f["xml"]).unwrap().as_bytes(),
            &resources,
            &major,
        )
        .unwrap()
    }
    fn schema_root() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join(SCHEMA);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(
            p,
            include_bytes!("../../../../ToS/contracts/source-text-unit-packet-v1.schema.json"),
        )
        .unwrap();
        t
    }
    #[test]
    fn exact_xml_identity_citation_and_packet_match_retained_recipe() {
        let t = schema_root();
        let ctx = ResearchExecution::new(t.path(), 60).unwrap();
        let f = fixture();
        let p = observed(&ctx, &f);
        assert_eq!(p.content, f["expected_text"]);
        assert_eq!(p.units.len(), f["unit_count"].as_u64().unwrap() as usize);
        assert!(p.units.iter().all(|u| u.resource_id != "aux"));
        let parts = vec![p];
        let issued = issue(&f["plan"], &parts);
        assert_eq!(
            sha(&encode(&ctx, &issued, true).unwrap()),
            f["issuance_sha256"]
        );
        let ids = identities(&issued).unwrap();
        let cs = citations(&ctx, &f["plan"], &parts, &ids).unwrap();
        let csraw = jsonl(&ctx, &cs).unwrap();
        assert_eq!(sha(&csraw), f["citations_sha256"]);
        let method = records::method(
            PLAN,
            LEGACY_BUILDER,
            LEGACY_EVENT,
            s(&f["plan"]["created_at"]).unwrap(),
        );
        let packet = packet(
            &ctx,
            &f["plan"],
            &parts[0],
            &ids,
            s(&f["plan"]["outputs"]["citation_spine_ref"]).unwrap(),
            &sha(&csraw),
            &method,
            false,
        )
        .unwrap();
        assert_eq!(
            sha(&encode(&ctx, &packet, true).unwrap()),
            f["packet_sha256"]
        );
        let paragraph = parts[0]
            .units
            .iter()
            .find(|u| u.unit_kind == "paragraph")
            .unwrap();
        let exact = &parts[0].content
            [parts[0].positions[paragraph.start]..parts[0].positions[paragraph.end]];
        assert_eq!(exact, "A😀\nB old tail.");
    }
    #[test]
    fn rejects_wrong_namespace_dtd_empty_units_and_duplicate_identities() {
        let t = schema_root();
        let ctx = ResearchExecution::new(t.path(), 60).unwrap();
        let f = fixture();
        let mut config = f["config"].clone();
        config["excluded_resource_ids"] = json!([]);
        for raw in [
            "<TEI xmlns=\"wrong\"><text><body><p>x</p></body></text></TEI>",
            "<!DOCTYPE TEI><TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><text><body><p>x</p></body></text></TEI>",
            "<TEI xmlns=\"http://www.tei-c.org/ns/1.0\"><text><body><p/></body></text></TEI>",
        ] {
            assert!(
                observe(
                    &ctx,
                    config.clone(),
                    "payload.xml".into(),
                    raw.as_bytes(),
                    &BTreeMap::new(),
                    &BTreeMap::new()
                )
                .is_err()
            );
        }
        let p = observed(&ctx, &f);
        let mut v = issue(&f["plan"], &[p]);
        v["unit_identities"][1]["unit_id"] = v["unit_identities"][0]["unit_id"].clone();
        assert!(identities(&v).is_err());
    }
    #[test]
    fn packet_refuses_reciprocal_drift_and_semantic_promotion() {
        let t = schema_root();
        let ctx = ResearchExecution::new(t.path(), 60).unwrap();
        let f = fixture();
        let parts = vec![observed(&ctx, &f)];
        let ids = identities(&issue(&f["plan"], &parts)).unwrap();
        let cs = citations(&ctx, &f["plan"], &parts, &ids).unwrap();
        let method = records::method(
            PLAN,
            BUILDER,
            "tos.event.native.test",
            s(&f["plan"]["created_at"]).unwrap(),
        );
        let p = packet(
            &ctx,
            &f["plan"],
            &parts[0],
            &ids,
            "ToS/source-witnesses/test.jsonl",
            &sha(&jsonl(&ctx, &cs).unwrap()),
            &method,
            false,
        )
        .unwrap();
        let mut v = p.clone();
        v["units"][0]["ordered_child_unit_refs"] = json!([]);
        assert!(validate_packet(&ctx, &v, None).is_err());
        let mut v = p.clone();
        v["units"][1]["semantic_promotion"] = json!(true);
        assert!(validate_packet(&ctx, &v, None).is_err());
        let mut v = p;
        v["anchors"][0]["exact_sha256"] = json!("0".repeat(64));
        assert!(validate_packet(&ctx, &v, Some((&parts[0].content, &parts[0].positions))).is_err());
    }
}
