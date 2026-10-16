//! Exact authored-route reconciliation, with separate source and authored boundaries.
//! Historical receipts remain historical; fresh execution grants no new authority.
use crate::{
    research_execution::ResearchExecution,
    research_text_comparison::{opcodes_with_autojunk, space},
    source_text_foundation::{
        Node, Part, encode, ensure, fresh_or_matching_limit, metadata, private_boundary,
        private_input_boundary, s, schema, sha, utc_now, xml_with_doctype,
    },
    transfer_target_passages::read_optional,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::Path,
};
#[path = "authored_canon_bridge/records.rs"]
mod records;
type Result<T> = std::result::Result<T, String>;
const CAP: usize = 2 * 1024 * 1024;
const PLAN: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/authored-canon-evidence-bridge.plan.v1.json";
const EVENT: &str =
    "tos.event.reconciliation.zarathustra-authored-canon-evidence-bridge.2026-08-12";
const BUILDER: &str = "rust/crates/tos-compiler/src/authored_canon_bridge.rs";
const LEGACY: &str = "scripts/build_zarathustra_authored_canon_evidence_bridge.py";
const SOFTWARE_AGENT: &str = "software:tos-zarathustra-authored-canon-evidence-bridge-builder";
const BRIDGE_AUTHORITY: &str = "the bridge proves exact representation and inventory closure only; it preserves the authored route without converting its witness roles, review notes, nodes, or relations into accepted German, accepted translation, semantic truth, modern human attestation, claim-evidence closure, graph admission, canon revision, publication permission, or server-transfer authority";
fn a(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or("required bridge array".into())
}
fn utf8(raw: &[u8]) -> Result<&str> {
    std::str::from_utf8(raw).map_err(|_| "bridge UTF8".into())
}
fn parsed(raw: &[u8]) -> Result<Value> {
    tos_foundation::parse_json(
        raw,
        tos_foundation::JsonMode::PublishedStrict,
        tos_foundation::JsonLimits::default(),
    )
    .map_err(|e| e.to_string())?;
    serde_json::from_slice(raw).map_err(|e| e.to_string())
}
fn canonical(v: &Value) -> Result<Vec<u8>> {
    fn sorted(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.clone(), sorted(v)))
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(v) => Value::Array(v.iter().map(sorted).collect()),
            _ => v.clone(),
        }
    }
    encode(&sorted(v), false)
}
struct Inputs<'a> {
    root: &'a ResearchExecution,
    files: BTreeMap<String, (File, String, u64)>,
}
impl<'a> Inputs<'a> {
    fn new(root: &'a ResearchExecution) -> Self {
        Self {
            root,
            files: BTreeMap::new(),
        }
    }
    fn read(&mut self, r: &str, expected: Option<&str>) -> Result<Vec<u8>> {
        let mut file = self.root.source_file(r, CAP as u64)?;
        let raw = self.root.read_file(&mut file, CAP as u64)?;
        let digest = sha(&raw);
        ensure(
            expected.is_none_or(|x| x == digest),
            "bridge input fixity drift",
        )?;
        if let Some((_, old, _)) = self.files.get(r) {
            ensure(old == &digest, "bridge input changed between reads")?;
        }
        self.files
            .insert(r.to_string(), (file, digest, raw.len() as u64));
        Ok(raw)
    }
    fn json(&mut self, r: &str) -> Result<Value> {
        parsed(&self.read(r, None)?)
    }
    fn details(&self, r: &str) -> Result<(String, u64)> {
        self.files
            .get(r)
            .map(|(_, h, n)| (h.clone(), *n))
            .ok_or("unread bridge input".into())
    }
    fn binding(&self, r: &str) -> Result<Value> {
        Ok(json!({"ref":r,"sha256":self.details(r)?.0}))
    }
    fn verify(&mut self) -> Result<()> {
        for (r, (file, h, n)) in &mut self.files {
            ensure(
                self.root.hash_file(file, CAP as u64)? == *h,
                "held bridge input drift",
            )?;
            let mut current = self.root.source_file(r, *n)?;
            ensure(
                self.root.hash_file(&mut current, CAP as u64)? == *h,
                "bridge input path drift",
            )?;
        }
        Ok(())
    }
}
fn render(node: &Node) -> Result<String> {
    let mut out = String::new();
    let mut consume_lf = false;
    for part in &node.content {
        match part {
            Part::Text(text) => {
                if consume_lf {
                    ensure(text.starts_with('\n'), "TEI lb formatting line feed absent")?;
                    out.push_str(&text[1..]);
                    consume_lf = false;
                } else {
                    out.push_str(text);
                }
            }
            Part::Child(child) => {
                ensure(!consume_lf, "TEI lb formatting tail absent")?;
                match child.name.as_str() {
                    "lb" => {
                        ensure(child.content.is_empty(), "TEI lb must be empty")?;
                        out.push('\n');
                        consume_lf = true;
                    }
                    "hi" => out.push_str(&render(child)?),
                    _ => return Err("unexpected inline element in source paragraph".into()),
                }
            }
        }
    }
    ensure(!consume_lf, "TEI lb formatting tail absent")?;
    Ok(out)
}
fn extract(ctx: &ResearchExecution, raw: &[u8]) -> Result<Vec<String>> {
    let root = xml_with_doctype(ctx, raw, false)?;
    ensure(root.name == "TEI", "source is not TEI")?;
    let texts = root.children("text");
    ensure(texts.len() == 1, "TEI direct text cardinality")?;
    let bodies = texts[0].children("body");
    ensure(bodies.len() == 1, "TEI direct body cardinality")?;
    let outer = bodies[0].children("div");
    let outer = outer.first().ok_or("TEI outer div absent")?;
    let inner = outer.children("div");
    let inner = inner.first().ok_or("TEI inner div absent")?;
    let paragraphs = inner.children("p");
    ensure(
        paragraphs.len() == 12,
        "expected twelve direct TEI paragraphs",
    )?;
    paragraphs.into_iter().map(render).collect()
}
fn collapsed(value: &str) -> String {
    value
        .split(space)
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn source_normalized(value: &str) -> String {
    collapsed(&value.replace("\u{ac}\n", ""))
}
fn authored_normalized(value: &str) -> String {
    let raw = value.replace('*', "");
    let mut chars = raw.chars().peekable();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'\n') {
            chars.next();
            while chars.peek().is_some_and(|c| space(*c)) {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    collapsed(&out)
}
fn slice(value: &str, start: usize, end: usize) -> Result<&str> {
    let n = value.chars().count();
    ensure(start <= end && end <= n, "codepoint span")?;
    let offset = |i| {
        if i == n {
            value.len()
        } else {
            value.char_indices().nth(i).unwrap().0
        }
    };
    Ok(&value[offset(start)..offset(end)])
}
type Span = (usize, usize);
fn spans(parts: &[String], complete: &str) -> Result<(Vec<Span>, Vec<Span>)> {
    ensure(
        parts.join(" ") == complete,
        "complete normalized segment sequence mismatch",
    )?;
    let mut cursor = 0;
    let mut spans = vec![];
    let mut gaps = vec![];
    for (i, p) in parts.iter().enumerate() {
        let end = cursor + p.chars().count();
        ensure(slice(complete, cursor, end)? == p, "segment position drift")?;
        spans.push((cursor, end));
        cursor = end;
        if i + 1 < parts.len() {
            gaps.push((cursor, cursor + 1));
            cursor += 1;
        }
    }
    ensure(cursor == complete.chars().count(), "segment coverage")?;
    Ok((spans, gaps))
}
fn operations(ctx: &ResearchExecution, raw: &str, normalized: &str) -> Result<Value> {
    let from = raw.chars().collect::<Vec<_>>();
    let to = normalized.chars().collect::<Vec<_>>();
    let mut rows = vec![];
    for op in opcodes_with_autojunk(ctx, &from, &to, true)? {
        if op.tag == "equal" {
            continue;
        }
        let input = slice(raw, op.i1, op.i2)?;
        let output = slice(normalized, op.j1, op.j2)?;
        rows.push(json!({"operation":op.tag,"input_span":[op.i1,op.i2],"output_span":[op.j1,op.j2],"input_exact":input,"input_sha256":sha(input.as_bytes()),"output_exact":output,"output_sha256":sha(output.as_bytes())}));
    }
    Ok(
        json!({"schema_version":"tos_private_editorial_normalization_operations_v1","input_sha256":sha(raw.as_bytes()),"output_sha256":sha(normalized.as_bytes()),"operation_count":rows.len(),"declared_steps":["remove U+00AC plus following line feed","collapse remaining Unicode whitespace runs to U+0020","trim leading and trailing whitespace"],"discretionary_break_count":raw.matches("\u{ac}\n").count(),"operations":rows}),
    )
}
fn comparison(
    raw: &str,
    normalized: &str,
    source_spans: &[Span],
    segments: &[Value],
    authored_spans: &[Span],
) -> Result<Value> {
    ensure(
        segments.len() == authored_spans.len(),
        "comparison membership",
    )?;
    let mut rows = vec![];
    for (segment, &(start, end)) in segments.iter().zip(authored_spans) {
        let observed = slice(normalized, start, end)?;
        let legacy = authored_normalized(s(&segment["text"])?);
        ensure(observed == legacy, "complete authored match")?;
        rows.push(json!({"legacy_segment_id":segment["segment_id"],"source_span":[start,end],"dta_normalized":observed,"legacy_normalized":legacy,"exact_match":true}));
    }
    Ok(
        json!({"schema_version":"tos_private_authored_route_comparison_v1","raw_sha256":sha(raw.as_bytes()),"normalized_sha256":sha(normalized.as_bytes()),"source_paragraph_count":source_spans.len(),"source_paragraph_spans":source_spans,"authored_segment_count":rows.len(),"authored_segments":rows,"complete_sequence_match":true,"authority_boundary":"private mechanical comparison details only; no language, translation, semantic, review, or canon authority"}),
    )
}
fn route_paths(ctx: &ResearchExecution, source_node: &str) -> Result<Vec<String>> {
    fn walk(
        ctx: &ResearchExecution,
        dir: File,
        prefix: &str,
        depth: usize,
        seen: &mut usize,
        paths: &mut BTreeSet<String>,
    ) -> Result<()> {
        ensure(depth <= 32, "canon inventory directory depth")?;
        let scan = std::fs::read_dir(format!("/proc/self/fd/{}", dir.as_raw_fd()))
            .map_err(|e| e.to_string())?;
        for entry in scan {
            ctx.tick(1)?;
            *seen += 1;
            ensure(*seen <= 4096, "canon inventory member bound")?;
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_str().ok_or("canon filename UTF8")?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            ensure(!kind.is_symlink(), "canon inventory symlink")?;
            let reference = format!("{prefix}/{name}");
            if kind.is_dir() {
                let child = tos_fd_open::open_directory_at(&dir, Path::new(name))
                    .map_err(|e| e.to_string())?;
                walk(ctx, child, &reference, depth + 1, seen, paths)?;
            } else if name == "node.json" {
                ensure(kind.is_file(), "canon node is not regular")?;
                paths.insert(reference);
            }
        }
        Ok(())
    }
    let mut paths = BTreeSet::from([
        source_node.to_string(),
        "ToS/canon/concept/becoming/node.json".into(),
        "ToS/canon/concept/overcoming/node.json".into(),
    ]);
    let mut seen = 0;
    for family in [
        "analogy",
        "event",
        "lineage",
        "principle",
        "state",
        "support",
        "synthesis",
    ] {
        let prefix =
            format!("ToS/canon/{family}/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1");
        let mut dir =
            tos_fd_open::reopen_directory(ctx.root_directory()).map_err(|e| e.to_string())?;
        for part in prefix.split('/') {
            dir =
                tos_fd_open::open_directory_at(&dir, Path::new(part)).map_err(|e| e.to_string())?;
        }
        walk(ctx, dir, &prefix, 0, &mut seen, &mut paths)?;
    }
    ensure(paths.len() == 92, "expected 92 authored route nodes")?;
    Ok(paths.into_iter().collect())
}
fn node_inventory(
    inputs: &mut Inputs<'_>,
    paths: &[String],
) -> Result<(Vec<Value>, BTreeMap<String, usize>)> {
    let mut ids = BTreeSet::new();
    let mut kinds = BTreeMap::new();
    let mut rows = vec![];
    for r in paths {
        let v = inputs.json(r)?;
        let id = s(&v["node_id"])?;
        let kind = s(&v["node_type"])?;
        ensure(
            ids.insert(id.to_string()),
            "duplicate authored node identity",
        )?;
        *kinds.entry(kind.to_string()).or_insert(0) += 1;
        rows.push(json!({"ref":r,"sha256":inputs.details(r)?.0,"node_id":id,"node_type":kind}));
    }
    Ok((rows, kinds))
}
fn relation_inventory(ctx: &ResearchExecution, raw: &[u8]) -> Result<Value> {
    use crate::knowledge_canon_source::{CanonSourceLimits, csv_records};
    let limits = CanonSourceLimits {
        max_manifest_members: 4096,
        max_selected_members: 4096,
        max_nodes: 4096,
        max_packs: 16,
        max_edges: 8192,
        max_source_bytes: CAP,
        max_raw_row_bytes: CAP,
        max_csv_fields: 128,
        max_csv_record_bytes: CAP,
        max_forms: 256,
        max_forms_output_bytes: 262144,
        max_page_rows: 16,
        max_page_bytes: CAP,
        max_work_bytes: 64 * 1024 * 1024,
    };
    let mut records = Vec::new();
    csv_records(
        raw,
        limits,
        ctx.deadline(),
        ctx.cancellation_flag(),
        |row| {
            records.push(row);
            Ok(())
        },
    )
    .map_err(|e| e.to_string())?;
    ensure(
        records.len() == 126,
        "expected 125 authored relations and one header",
    )?;
    let header = records.remove(0);
    ensure(
        header.iter().collect::<BTreeSet<_>>().len() == header.len(),
        "duplicate CSV header",
    )?;
    let index = |key: &str| {
        header
            .iter()
            .position(|h| h == key)
            .ok_or_else(|| format!("relation header missing: {key}"))
    };
    let id = index("edge_id")?;
    let kind = index("edge_kind")?;
    let segments = index("anchor_segment_ids")?;
    let valid = (1..=12)
        .map(|i| format!("seg.1.1.1.{i}"))
        .collect::<BTreeSet<_>>();
    let mut ids = vec![];
    let mut kinds = BTreeMap::<String, usize>::new();
    for row in records {
        ctx.tick(row.len() as u64)?;
        ensure(row.len() == header.len(), "relation CSV width")?;
        let anchors = row[segments]
            .split('|')
            .filter(|x| !x.is_empty())
            .collect::<BTreeSet<_>>();
        ensure(
            !anchors.is_empty() && anchors.iter().all(|x| valid.contains(*x)),
            "relation segment locator",
        )?;
        ids.push(row[id].clone());
        *kinds.entry(row[kind].clone()).or_default() += 1;
    }
    ensure(
        ids.iter().collect::<BTreeSet<_>>().len() == 125,
        "duplicate relation identity",
    )?;
    Ok(
        json!({"relation_count":125,"relation_kind_counts":kinds,"relation_ids_sha256":sha((ids.join("\n")+"\n").as_bytes()),"relations_with_legacy_segment_locators":125}),
    )
}
fn bridge(
    inputs: &Inputs<'_>,
    plan: &Value,
    node: &Value,
    segments: &[Value],
    normalized: &str,
    authored_spans: &[Span],
    nodes: &[Value],
    kinds: &BTreeMap<String, usize>,
    relations: &Value,
    bindings: &Value,
) -> Result<Value> {
    let mut witnesses = vec![];
    let mut languages = BTreeSet::new();
    for witness in a(&node["language_witnesses"])? {
        let language = s(&witness["language"])?;
        ensure(
            languages.insert(language),
            "duplicate authored language witness",
        )?;
        let source = language == "de";
        witnesses.push(json!({"language":language,"legacy_role":witness["role"],"segment_count":a(&witness["segments"])?.len(),"content_sha256":sha(&canonical(&witness["segments"])?),"authorship_posture":if source {"nietzsche-source-witness-role"}else{"dionysus-authored-translation-witness"},"current_assurance":if source {"mechanically-crosswalked-not-philologically-accepted"}else{"legacy-authored-translation-not-modernly-reviewed"},"modern_review_refs":[]}));
    }
    ensure(
        languages == BTreeSet::from(["de", "ru", "en"]),
        "exact authored witness language membership",
    )?;
    let ids = &plan["opaque_ids"];
    let unit_ids = a(&ids["authored_unit_ids"])?;
    let anchor_ids = a(&ids["authored_anchor_ids"])?;
    ensure(
        segments.len() == 12
            && unit_ids.len() == 12
            && anchor_ids.len() == 12
            && authored_spans.len() == 12,
        "crosswalk identity membership",
    )?;
    let mut crosswalk = vec![];
    for (i, segment) in segments.iter().enumerate() {
        let (start, end) = authored_spans[i];
        let observed = slice(normalized, start, end)?;
        let legacy = authored_normalized(s(&segment["text"])?);
        ensure(observed == legacy, "authored segment source mismatch")?;
        crosswalk.push(json!({"legacy_segment_id":segment["segment_id"],"source_unit_ref":unit_ids[i],"source_anchor_ref":anchor_ids[i],"dta_normalized_sha256":sha(observed.as_bytes()),"legacy_normalized_sha256":sha(legacy.as_bytes()),"normalized_match":true,"match_scope":"mechanical-representation-only","modern_review_refs":[]}));
    }
    let surfaces = &plan["authored_surfaces"];
    let reviews = a(&surfaces["review_record_refs"])?;
    let review_bindings = reviews
        .iter()
        .map(|r| inputs.binding(s(r)?))
        .collect::<Result<Vec<_>>>()?;
    let mut inventory = relations.clone();
    inventory["node_count"] = json!(nodes.len());
    inventory["node_kind_counts"] = json!(kinds);
    inventory["node_records_sha256"] = json!(sha(&canonical(&json!(nodes))?));
    inventory["relations_with_modern_claim_refs"] = json!(0);
    inventory["relations_with_modern_evidence_refs"] = json!(0);
    inventory["legacy_review_record_count"] = json!(reviews.len());
    inventory["machine_readable_human_attestation_count"] = json!(0);
    let route_scope = [
        "route_ref",
        "work_ref",
        "expression_ref",
        "edition_ref",
        "item_ref",
        "file_ref",
        "file_sha256",
    ]
    .into_iter()
    .map(|k| (k.to_string(), plan["scope"][k].clone()))
    .collect::<serde_json::Map<_, _>>();
    Ok(json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/authored-route-evidence-bridge-v1.schema.json",
        "schema_version":"tos_authored_route_evidence_bridge_v1","bridge_id":"tos.authored-route-evidence-bridge.zarathustra-prologue-1","bridge_version":1,
        "research":inputs.binding(s(&plan["research_ref"])?)?,"route_scope":route_scope,
        "source_foundation":{"source_anchor":{"ref":plan["outputs"]["anchor_ref"],"sha256":bindings["anchor"]},"raw_layer":{"ref":plan["outputs"]["raw_layer_ref"],"sha256":bindings["raw_layer"]},"normalized_layer":{"ref":plan["outputs"]["normalized_layer_ref"],"sha256":bindings["normalized_layer"]},"unit_packet":{"ref":plan["outputs"]["unit_packet_ref"],"sha256":bindings["unit_packet"]},"rights":inputs.binding(s(&plan["scope"]["rights_ref"])?)?},
        "authored_surfaces":{"source_node":inputs.binding(s(&surfaces["source_node_ref"])?)?,"alignment_witness":inputs.binding(s(&surfaces["alignment_witness_ref"])?)?,"relation_pack":inputs.binding(s(&surfaces["relation_pack_ref"])?)?,"review_records":review_bindings},
        "witness_roles":witnesses,"segment_crosswalk":crosswalk,"canon_inventory":inventory,
        "assurance":{"source_comparison":"complete-mechanical-normalized-match","authored_review_posture":"legacy-review-records-without-machine-readable-human-attestation","modern_semantic_packet_posture":"not-materialized-by-this-bridge","claim_evidence_closure":false,"graph_admission":false,"current_canon_retained":true,"bulk_migration_authorized":false},
        "effects":{"source_text_accepted":false,"german_competence_attested":false,"translation_accepted":false,"sign_or_concept_promoted":false,"legacy_relation_migrated":false,"graph_projection_admitted":false,"canon_revised":false,"human_task_created":false,"new_publication_authorized":false,"server_transfer_authorized":false},
        "authority_boundary":BRIDGE_AUTHORITY
    }))
}
fn entity(
    r: &str,
    role: &str,
    digest: &str,
    size: u64,
    media: &str,
    availability: &str,
    disclosure: &str,
    at: &Value,
) -> Value {
    json!({"entity_ref":r,"role":role,"sha256":digest,"size_bytes":size,"media_type":media,"availability":availability,"content_disclosure":disclosure,"fixity_verified":true,"fixity_verified_at":at})
}
fn input_entities(
    plan: &Value,
    plan_ref: &str,
    tracked: &Inputs<'_>,
    local: &Inputs<'_>,
    at: &Value,
) -> Result<Vec<Value>> {
    let r = s(&plan["scope"]["source_relative_ref"])?;
    let (h, n) = local.details(r)?;
    let mut rows = vec![entity(
        r,
        "fixity-verified-local-dta-tei-source",
        &h,
        n,
        "application/xml",
        "owner_local",
        "private_content",
        at,
    )];
    let mut specs = vec![
        (
            plan_ref,
            "tracked-text-free-bridge-plan",
            "public_metadata_only",
        ),
        (
            s(&plan["research_ref"])?,
            "tracked-reconciliation-research",
            "public_content",
        ),
        (
            s(&plan["scope"]["rights_ref"])?,
            "tracked-source-rights-record",
            "public_metadata_only",
        ),
        (
            s(&plan["authored_surfaces"]["source_node_ref"])?,
            "tracked-authored-source-node",
            "public_content",
        ),
        (
            s(&plan["authored_surfaces"]["alignment_witness_ref"])?,
            "tracked-authored-trilingual-donor",
            "public_content",
        ),
        (
            s(&plan["authored_surfaces"]["relation_pack_ref"])?,
            "tracked-authored-relation-pack",
            "public_content",
        ),
    ];
    for r in a(&plan["authored_surfaces"]["review_record_refs"])? {
        specs.push((s(r)?, "tracked-legacy-review-record", "public_content"));
    }
    for (r, role, disclosure) in specs {
        let (h, n) = tracked.details(r)?;
        let media = if r.ends_with(".json") {
            "application/json"
        } else if r.ends_with(".md") {
            "text/markdown; charset=utf-8"
        } else if r.ends_with(".csv") {
            "text/csv; charset=utf-8"
        } else {
            "application/octet-stream"
        };
        rows.push(entity(r, role, &h, n, media, "tracked", disclosure, at));
    }
    Ok(rows)
}
fn builder_sha() -> String {
    let mut hash = tos_foundation::Digest256Hasher::new();
    for raw in [
        include_bytes!("authored_canon_bridge.rs").as_slice(),
        include_bytes!("authored_canon_bridge/records.rs").as_slice(),
        include_bytes!("authored_canon_bridge/historical-event.json").as_slice(),
        include_bytes!("authored_canon_bridge/historical-plan.json").as_slice(),
        include_bytes!("authored_canon_bridge/authority-boundary.json").as_slice(),
        include_bytes!("research_text_comparison.rs").as_slice(),
        include_bytes!("source_text_foundation.rs").as_slice(),
    ] {
        hash.update(&(raw.len() as u64).to_be_bytes());
        hash.update(raw);
    }
    hash.finalize().to_hex()
}
fn event(
    ctx: &ResearchExecution,
    plan: &Value,
    plan_ref: &str,
    plan_sha: &str,
    event_id: &str,
    historical: bool,
    inputs: &[Value],
    argv_sha: &str,
    runtime_version: &str,
    at: &Value,
    outputs: &[(String, Vec<u8>)],
    private: &[(String, Vec<u8>)],
    rights: &Value,
) -> Result<Value> {
    let mut e: Value =
        serde_json::from_str(include_str!("authored_canon_bridge/historical-event.json"))
            .map_err(|e| e.to_string())?;
    let mut rows = vec![];
    for (r, b) in private {
        rows.push(entity(
            r,
            "ignored-local-evidence-bridge-artifact",
            &sha(b),
            b.len() as u64,
            if r.ends_with(".json") {
                "application/json"
            } else {
                "text/plain; charset=utf-8"
            },
            "ignored_local",
            "private_content",
            at,
        ));
    }
    for (r, b) in outputs {
        rows.push(entity(
            r,
            "tracked-text-free-evidence-bridge-record",
            &sha(b),
            b.len() as u64,
            "application/json",
            "tracked",
            "public_metadata_only",
            at,
        ));
    }
    if historical {
        for (kind, actual) in [("inputs", json!(inputs)), ("outputs", json!(rows))] {
            let expected = a(&e["entities"][kind])?;
            let actual = a(&actual)?;
            ensure(
                expected.len() == actual.len(),
                "historical event membership count drift",
            )?;
            for (index, (left, right)) in expected.iter().zip(actual).enumerate() {
                ensure(
                    left == right,
                    &format!(
                        "historical {kind} entity {} fixity or membership drift",
                        index + 1
                    ),
                )?;
            }
        }
        ensure(
            e["rights_and_visibility"]["rights_record_bindings"] == json!([rights]),
            "historical rights binding drift",
        )?;
        return Ok(e);
    }
    e["event_id"] = json!(event_id);
    e["record_binding"]["manifest_ref"] = json!(plan_ref);
    e["activity"]["started_at"] = at.clone();
    e["activity"]["ended_at"] = at.clone();
    e["entities"] = json!({"inputs":inputs,"outputs":rows,"byproducts":[]});
    let builder_digest = builder_sha();
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = ctx.select_directory(executable.parent().ok_or("executable parent")?)?;
    let mut file = dir.source_file(
        executable
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("executable name")?,
        256 * 1024 * 1024,
    )?;
    let executable_sha = dir.hash_file(&mut file, 256 * 1024 * 1024)?;
    e["responsibility"] = json!([{"agent_ref":SOFTWARE_AGENT,"agent_kind":"software","role":"executor","responsibility_posture":"performed","evidence_binding":{"ref":BUILDER,"sha256":builder_digest},"human_evidence_status":"not_applicable"}]);
    e["method"]["procedure"] =
        json!({"name":"authored-canon-evidence-bridge","version":"1","purpose":plan["question"]});
    e["method"]["command_capture"] = json!({"disclosure":"withheld_digest_only","argv":null,"argv_sha256":argv_sha,"withholding_reason":"The captured build argv includes owner-local absolute roots; only its exact canonical digest is published."});
    e["method"]["configuration_binding"] = json!({"ref":plan_ref,"sha256":plan_sha});
    e["method"]["software_components"] = json!([{"name":"Tree of Sophia authored canon evidence bridge builder","version":"1","role":"section-extraction-normalization-crosswalk-and-record-builder","artifact_ref":BUILDER,"artifact_sha256":builder_digest,"verification_status":"verified"},{"name":"tos-access","version":runtime_version,"role":"native-executable","artifact_ref":"runtime:tos-native-executable","artifact_sha256":executable_sha,"verification_status":"verified"}]);
    e["method"]["environment"] = json!({"runtime":"Rust native executable","runtime_version":runtime_version,"runtime_artifact_sha256":executable_sha,"backend":"rust-tei-csv-sequence-matcher-autojunk","hardware_target":"cpu","unicode_version":"16.0.0","environment_profile_binding":{"ref":plan_ref,"sha256":plan_sha}});
    let mut derivations = vec![];
    let prefix = format!(
        "tos.derivation.authored-bridge.{}",
        &sha(event_id.as_bytes())[..16]
    );
    for (row_index, row) in rows.iter().enumerate() {
        derivations.push(json!({"derivation_id":format!("{prefix}.{}",row_index+1),"input_entity_ref":plan["scope"]["source_relative_ref"],"output_entity_ref":row["entity_ref"],"relation":"selection_from","influence_asserted":true,"description":"The exact TEI, frozen plan and digest-bound authored route determine this non-promoting bridge output."}));
    }
    for row in inputs.iter().skip(4) {
        derivations.push(json!({"derivation_id":format!("{prefix}.authored-{}",derivations.len()+1),"input_entity_ref":row["entity_ref"],"output_entity_ref":plan["outputs"]["bridge_ref"],"relation":"was_derived_from","influence_asserted":true,"description":"The bridge inventory binds this exact authored or historical-review surface without promoting it."}));
    }
    e["derivations"] = json!(derivations);
    e["measurements"][0]["value"] = inputs[0]["size_bytes"].clone();
    e["measurements"][1]["value"] = json!(
        outputs
            .iter()
            .chain(private)
            .map(|(_, b)| b.len() as u64)
            .sum::<u64>()
    );
    e["rights_and_visibility"]["rights_record_bindings"] = json!([rights]);
    Ok(e)
}
pub struct Options<'a> {
    pub build: bool,
    pub input_root: &'a Path,
    pub output_root: &'a Path,
    pub plan_ref: Option<&'a str>,
    pub event_id: Option<&'a str>,
    pub argv: &'a Value,
    pub runtime_version: &'a str,
}
fn plan_identity(plan: &Value, plan_ref: &str, event_id: &str, plan_sha: &str) -> Result<bool> {
    let historical = plan_ref == PLAN;
    ensure(
        historical == (event_id == EVENT),
        "new plan requires a new event identity",
    )?;
    ensure(
        regex::Regex::new(r"^tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*$")
            .unwrap()
            .is_match(event_id),
        "event identity syntax",
    )?;
    let original: Value =
        serde_json::from_str(include_str!("authored_canon_bridge/historical-plan.json"))
            .map_err(|e| e.to_string())?;
    ensure(
        plan["schema_version"] == original["schema_version"]
            && plan["authority_boundary"] == original["authority_boundary"]
            && plan["normalization_policy"] == original["normalization_policy"]
            && plan["source_selector"] == original["source_selector"],
        "plan changes the declared method or authority boundary",
    )?;
    if historical {
        ensure(
            plan_sha == "097b12fe1c183b31dd15e720a50b437664011a4c70150cb7335fa3a5d7b2e04e",
            "historical plan bytes changed; use a new plan and event",
        )?;
    }
    let outputs = plan["outputs"].as_object().ok_or("output references")?;
    let old_outputs = original["outputs"].as_object().unwrap();
    ensure(
        outputs.len() == old_outputs.len()
            && outputs.keys().all(|key| old_outputs.contains_key(key)),
        "exact output membership",
    )?;
    let mut refs = BTreeSet::new();
    for (key, value) in outputs {
        let r = s(value)?;
        ensure(
            r.starts_with("ToS/source-witnesses/")
                && r != plan_ref
                && Path::new(r)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
                && refs.insert(r),
            "unsafe or duplicate output reference",
        )?;
        if !historical {
            ensure(
                !old_outputs.values().any(|old| old == value),
                "new event must use separate historical outputs",
            )?;
        }
        ensure(
            key.starts_with("private_") == r.contains("/local-content/"),
            "private and tracked output boundary",
        )?;
    }
    let ids = plan["opaque_ids"].as_object().ok_or("opaque IDs")?;
    let old_ids = original["opaque_ids"].as_object().unwrap();
    ensure(
        ids.len() == old_ids.len() && ids.keys().all(|k| old_ids.contains_key(k)),
        "opaque identity membership",
    )?;
    let mut current = BTreeSet::new();
    let mut old = BTreeSet::new();
    for (key, value) in ids {
        let values = if value.is_array() {
            let entries = a(value)?;
            ensure(
                entries.len() == a(&old_ids[key])?.len(),
                "opaque identity array count",
            )?;
            entries.iter().collect::<Vec<_>>()
        } else {
            vec![value]
        };
        for v in values {
            ensure(current.insert(s(v)?), "duplicate opaque identity")?;
        }
    }
    for value in old_ids.values() {
        if let Some(values) = value.as_array() {
            for v in values {
                old.insert(s(v)?);
            }
        } else {
            old.insert(s(value)?);
        }
    }
    if !historical {
        ensure(
            current.is_disjoint(&old),
            "new event requires separate opaque identities",
        )?;
        ensure(
            plan["plan_id"] != original["plan_id"],
            "new plan identity required",
        )?;
        let id = s(&plan["bridge_id"])?;
        ensure(
            id != "tos.authored-route-evidence-bridge.zarathustra-prologue-1",
            "new bridge identity required",
        )?;
    }
    Ok(historical)
}
pub fn run(ctx: &ResearchExecution, opts: Options<'_>) -> Result<Value> {
    let plan_ref = opts.plan_ref.unwrap_or(PLAN);
    let event_id = opts.event_id.unwrap_or(EVENT);
    let local_ctx = ctx.select_directory(opts.input_root)?;
    let out = ctx.select_output_directory(opts.output_root, opts.build)?;
    let mut tracked = Inputs::new(ctx);
    let mut local = Inputs::new(&local_ctx);
    let plan_raw = tracked.read(plan_ref, None)?;
    let plan_sha = sha(&plan_raw);
    let plan = parsed(&plan_raw)?;
    let historical = plan_identity(&plan, plan_ref, event_id, &plan_sha)?;
    let scope = &plan["scope"];
    let source_ref = s(&scope["source_relative_ref"])?;
    private_input_boundary(ctx, source_ref)?;
    let rights_ref = s(&scope["rights_ref"])?;
    let rights = tracked.json(rights_ref)?;
    schema(ctx, "ToS/contracts/rights-record.schema.json", &rights)?;
    ensure(
        a(&rights["scope_refs"])?.contains(&scope["item_ref"])
            && a(&rights["scope_refs"])?.contains(&scope["file_ref"])
            && matches!(
                rights["derivative_posture"].as_str(),
                Some("local_research_only" | "allowed_with_conditions" | "allowed")
            ),
        "current rights do not cover local derivative processing",
    )?;
    tracked.read(s(&plan["research_ref"])?, None)?;
    let surfaces = &plan["authored_surfaces"];
    tracked.read(s(&surfaces["alignment_witness_ref"])?, None)?;
    let relation_raw = tracked.read(s(&surfaces["relation_pack_ref"])?, None)?;
    let relations = relation_inventory(ctx, &relation_raw)?;
    for r in a(&surfaces["review_record_refs"])? {
        tracked.read(s(r)?, None)?;
    }
    let node_ref = s(&surfaces["source_node_ref"])?;
    let paths = route_paths(ctx, node_ref)?;
    let (nodes, kinds) = node_inventory(&mut tracked, &paths)?;
    let node = tracked.json(node_ref)?;
    ensure(
        node["node_id"] == scope["route_ref"],
        "authored source identity drift",
    )?;
    let witnesses = a(&node["language_witnesses"])?;
    let de = witnesses
        .iter()
        .filter(|w| w["language"] == "de")
        .collect::<Vec<_>>();
    ensure(de.len() == 1, "exact German witness membership")?;
    let segments = a(&de[0]["segments"])?;
    ensure(
        segments.len() == 12
            && segments
                .iter()
                .enumerate()
                .all(|(i, s)| s["segment_id"] == format!("seg.1.1.1.{}", i + 1)),
        "authored segment identity or order drift",
    )?;
    let source = local.read(source_ref, Some(s(&scope["file_sha256"])?))?;
    let raw_parts = extract(ctx, &source)?;
    let raw = raw_parts.join("\n\n");
    let normalized = source_normalized(&raw);
    let source_parts = raw_parts
        .iter()
        .map(|p| source_normalized(p))
        .collect::<Vec<_>>();
    let (source_spans, _) = spans(&source_parts, &normalized)?;
    let authored_parts = segments
        .iter()
        .map(|s0| s(&s0["text"]).map(authored_normalized))
        .collect::<Result<Vec<_>>>()?;
    let (authored_spans, _) = spans(&authored_parts, &normalized)?;
    let operations = operations(ctx, &raw, &normalized)?;
    let comparison = comparison(&raw, &normalized, &source_spans, segments, &authored_spans)?;
    let private = vec![
        (
            s(&plan["outputs"]["private_raw_content_ref"])?.to_string(),
            raw.as_bytes().to_vec(),
        ),
        (
            s(&plan["outputs"]["private_normalized_content_ref"])?.to_string(),
            normalized.as_bytes().to_vec(),
        ),
        (
            s(&plan["outputs"]["private_operations_ref"])?.to_string(),
            encode(&operations, true)?,
        ),
        (
            s(&plan["outputs"]["private_comparison_ref"])?.to_string(),
            encode(&comparison, true)?,
        ),
    ];
    for (r, _) in &private {
        private_boundary(ctx, r)?;
    }
    let anchor = records::source_anchor(&plan, plan_ref, &plan_sha, event_id)?;
    let anchor_bytes = encode(&anchor, true)?;
    let anchor_sha = sha(&anchor_bytes);
    let rights_sha = tracked.details(rights_ref)?.0;
    let raw_layer = records::common_layer(
        &plan,
        plan_ref,
        &plan_sha,
        event_id,
        &plan["opaque_ids"]["raw_layer_id"],
        "machine_transcription",
        &plan["outputs"]["private_raw_content_ref"],
        raw.as_bytes(),
        &anchor_sha,
        &rights_sha,
        json!({"method":"structural_extraction","input_layers":[],"maker":{"maker_type":"software","agent_ref":SOFTWARE_AGENT,"method":"tei-lb-aware-section-extraction","version":"1","configuration_ref":plan_ref,"configuration_digest":plan_sha},"preservation_goal":"source_near","loss_posture":"preservation_intended","silent_changes_allowed":false,"change_payload":{"kind":"none"}}),
        "none",
        "source_preserved",
        "machine_candidate",
        "encode_explicitly",
    )?;
    let raw_layer_bytes = encode(&raw_layer, true)?;
    let normalized_layer = records::common_layer(
        &plan,
        plan_ref,
        &plan_sha,
        event_id,
        &plan["opaque_ids"]["normalized_layer_id"],
        "normalized_text",
        &plan["outputs"]["private_normalized_content_ref"],
        normalized.as_bytes(),
        &anchor_sha,
        &rights_sha,
        json!({"method":"editorial_normalization","input_layers":[{"layer_id":plan["opaque_ids"]["raw_layer_id"],"record_ref":plan["outputs"]["raw_layer_ref"],"record_sha256":sha(&raw_layer_bytes),"content_sha256":sha(raw.as_bytes())}],"maker":{"maker_type":"software","agent_ref":SOFTWARE_AGENT,"method":plan["normalization_policy"]["method"],"version":plan["normalization_policy"]["method_version"],"configuration_ref":plan_ref,"configuration_digest":plan_sha},"preservation_goal":"normalized_for_search","loss_posture":"normalization_intended","silent_changes_allowed":false,"change_payload":{"kind":"withheld_operations_receipt","operation_count":operations["operation_count"],"operations_sha256":sha(&private[2].1),"private_ref":plan["outputs"]["private_operations_ref"]}}),
        "editorial",
        "logical_reflow",
        "normalized_access",
        "logical_reflow",
    )?;
    let packet = records::unit_packet(
        &plan,
        plan_ref,
        if historical { LEGACY } else { BUILDER },
        event_id,
        &normalized,
        &source_parts,
        &authored_parts,
    )?;
    let bindings = json!({"anchor":anchor_sha,"raw_layer":sha(&raw_layer_bytes),"normalized_layer":sha(&encode(&normalized_layer,true)?),"unit_packet":sha(&encode(&packet,true)?)});
    let mut bridge = bridge(
        &tracked,
        &plan,
        &node,
        segments,
        &normalized,
        &authored_spans,
        &nodes,
        &kinds,
        &relations,
        &bindings,
    )?;
    if !historical {
        bridge["bridge_id"] = plan["bridge_id"].clone();
    }
    let mut outputs = vec![];
    for (key, kind, contract, value) in [
        (
            "anchor_ref",
            "anchor",
            "source-anchor-v2.schema.json",
            anchor,
        ),
        (
            "raw_layer_ref",
            "layer",
            "source-text-layer.schema.json",
            raw_layer,
        ),
        (
            "normalized_layer_ref",
            "layer",
            "source-text-layer.schema.json",
            normalized_layer,
        ),
        (
            "unit_packet_ref",
            "units",
            "source-text-unit-packet-v1.schema.json",
            packet,
        ),
        (
            "bridge_ref",
            "bridge",
            "authored-route-evidence-bridge-v1.schema.json",
            bridge,
        ),
    ] {
        let r = s(&plan["outputs"][key])?;
        schema(ctx, &format!("ToS/contracts/{contract}"), &value)?;
        if kind != "bridge" {
            metadata(ctx, kind, r, &value)?;
        }
        let bytes = encode(&value, true)?;
        if kind == "units" {
            let report =
                tos_validation::layer_family_rules::inspect_supplied_source_text_unit_semantics(
                    r,
                    &bytes,
                    s(&plan["outputs"]["private_normalized_content_ref"])?,
                    normalized.as_bytes(),
                    &plan_sha,
                    tos_validation::item_rules::ItemLimits {
                        max_member_bytes: CAP,
                        max_total_bytes: (32 * CAP) as u64,
                        max_state_bytes: 32 * CAP,
                        max_issues: 64,
                        deadline: ctx.deadline(),
                    },
                )
                .map_err(|e| format!("unit semantics: {e:?}"))?;
            ensure(
                report.state == tos_validation::text_rules::TextRuleState::Checked
                    && report.issues.is_empty(),
                "source/authored competing segmentation semantics",
            )?;
        }
        outputs.push((r.to_string(), bytes));
    }
    let existing_event = read_optional(ctx, s(&plan["outputs"]["provenance_event_ref"])?)?
        .map(|raw| parsed(&raw))
        .transpose()?;
    ensure(
        opts.build || existing_event.is_some(),
        "check requires retained provenance",
    )?;
    let at = if historical {
        plan["created_at"].clone()
    } else if let Some(e) = &existing_event {
        e["activity"]["started_at"].clone()
    } else {
        json!(utc_now()?)
    };
    let argv_sha = if !historical && existing_event.is_some() {
        s(&existing_event.as_ref().unwrap()["method"]["command_capture"]["argv_sha256"])?
            .to_string()
    } else {
        sha(&canonical(opts.argv)?)
    };
    ensure(
        argv_sha.len() == 64
            && argv_sha
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "captured argv digest syntax",
    )?;
    let inputs = input_entities(&plan, plan_ref, &tracked, &local, &at)?;
    let e = event(
        ctx,
        &plan,
        plan_ref,
        &plan_sha,
        event_id,
        historical,
        &inputs,
        &argv_sha,
        opts.runtime_version,
        &at,
        &outputs,
        &private,
        &tracked.binding(rights_ref)?,
    )?;
    if let Some(existing) = existing_event {
        ensure(
            existing == e,
            "retained provenance differs from bound inputs, outputs or native method",
        )?;
    }
    schema(ctx, "ToS/contracts/provenance-event-v2.schema.json", &e)?;
    let issues = tos_validation::provenance_rules::semantic_issues(&e, 64, ctx.deadline())
        .map_err(|e| format!("provenance semantics: {e:?}"))?;
    ensure(issues.is_empty(), "provenance closure semantics")?;
    outputs.push((
        s(&plan["outputs"]["provenance_event_ref"])?.to_string(),
        encode(&e, true)?,
    ));
    for bytes in outputs
        .iter()
        .map(|(_, b)| b)
        .chain(std::iter::once(&plan_raw))
    {
        let view = utf8(bytes)?;
        ensure(
            ![raw.as_str(), normalized.as_str()]
                .iter()
                .any(|p| view.contains(p)),
            "tracked output exposes private source text",
        )?;
    }
    tracked.verify()?;
    local.verify()?;
    ensure(
        route_paths(ctx, node_ref)? == paths,
        "authored node inventory changed",
    )?;
    let mut pending = vec![];
    for (r, b) in &private {
        let missing = fresh_or_matching_limit(&out, r, b, CAP)?;
        if !missing {
            ensure(
                out.source_file(r, CAP as u64)?
                    .metadata()
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode()
                    & 0o777
                    == 0o600,
                "private output must remain 0600",
            )?;
        }
        pending.push((&out, r, b, 0o600, missing));
    }
    for (r, b) in &outputs {
        ensure(
            !tracked.files.contains_key(r) && !local.files.contains_key(r),
            "output aliases input",
        )?;
        pending.push((ctx, r, b, 0o644, fresh_or_matching_limit(ctx, r, b, CAP)?));
    }
    ensure(
        opts.build || pending.iter().all(|(_, _, _, _, m)| !*m),
        "required output absent",
    )?;
    let mut written = 0;
    for (context, r, b, mode, missing) in pending {
        if missing {
            context.write(r, b, mode, true)?;
            written += 1;
        }
    }
    Ok(
        json!({"status":"passed","plan_ref":plan_ref,"event_id":event_id,"historical_replay":historical,"native_executor":"tos authored-canon-bridge","source_sha256":scope["file_sha256"],"source_paragraph_count":12,"authored_segment_count":12,"normalized_match_count":12,"canon_node_count":nodes.len(),"canon_relation_count":relations["relation_count"],"modern_claim_closure_count":0,"modern_human_attestation_count":0,"tracked_record_count":outputs.len(),"private_artifact_count":private.len(),"written":written,"accepted_german":false,"accepted_translation":false,"semantic_or_graph_promotion":false,"canon_revised":false,"human_task_created":false,"new_publication_authorized":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_source_and_authored_boundaries_remain_independent() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = ResearchExecution::new(temp.path(), 30).unwrap();
        let body = (0..12)
            .map(|i| format!("<p><hi>Текст</hi> {i}<lb/>\nдальше</p>"))
            .collect::<String>();
        let xml = format!("<TEI><text><body><div><div>{body}</div></div></body></text></TEI>");
        let parts = extract(&ctx, xml.as_bytes()).unwrap();
        assert_eq!(parts[0], "Текст 0\nдальше");
        assert_eq!(
            source_normalized(" Mor\u{ac}\ngen\u{1c}  Licht "),
            "Morgen Licht"
        );
        assert_eq!(
            authored_normalized(" *Mor-\n  gen*\u{1f} Licht "),
            "Morgen Licht"
        );
        let text = "α β γ";
        let (source, _) = spans(&["α β".into(), "γ".into()], text).unwrap();
        let (authored, _) = spans(&["α".into(), "β γ".into()], text).unwrap();
        assert_ne!(source, authored);
        assert_eq!(slice(text, 2, 5).unwrap(), "β γ");
        assert!(spans(&["α γ".into()], text).is_err());
        let changed = xml
            .replace("<hi>", "<choice>")
            .replace("</hi>", "</choice>");
        assert!(extract(&ctx, changed.as_bytes()).is_err());
    }
    #[test]
    fn plan_cannot_widen_authority_method_or_reuse_opaque_identity() {
        let original: Value =
            serde_json::from_str(include_str!("authored_canon_bridge/historical-plan.json"))
                .unwrap();
        let digest = "097b12fe1c183b31dd15e720a50b437664011a4c70150cb7335fa3a5d7b2e04e";
        assert!(plan_identity(&original, PLAN, EVENT, digest).unwrap());
        let mut p = original.clone();
        p["authority_boundary"]["accepted_german"] = json!(true);
        assert!(plan_identity(&p, PLAN, EVENT, digest).is_err());
        p = original.clone();
        p["source_selector"]["expected_paragraph_count"] = json!(13);
        assert!(plan_identity(&p, PLAN, EVENT, digest).is_err());
        p = original.clone();
        p["opaque_ids"]["source_anchor_ids"][1] = p["opaque_ids"]["source_anchor_ids"][0].clone();
        assert!(plan_identity(&p, PLAN, EVENT, digest).is_err());
        assert!(
            plan_identity(
                &original,
                "ToS/source-witnesses/new-plan.json",
                "tos.event.reconciliation.new",
                digest
            )
            .is_err()
        );
    }
}
