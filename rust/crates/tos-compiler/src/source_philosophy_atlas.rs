//! Authored atlas source return and full deterministic atlas projection.
use crate::source_philosophy_multilingual::Multilingual;
use crate::source_philosophy_support::check_run;
use crate::source_philosophy_support::{
    array, bytes, digest, fallback, object_with_profile, required, sha1_hex, string, truth,
};
use crate::{Error, PhilosophySourceReadProfile, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};
pub const ATLAS_SOURCE: &str = "ToS/philosophy/atlas/atlas.manifest.json";
pub const DOSSIERS_SOURCE: &str = "ToS/philosophy/atlas/dossiers/index.jsonl";
pub const DOSSIERS_MANIFEST: &str = "ToS/philosophy/atlas/dossiers/branch.manifest.json";
pub const GRAPH_SHAPE: &str = "ToS/philosophy/atlas/dossiers/graph-shape-summary.json";
pub const ALIASES_SOURCE: &str =
    "ToS/philosophy/graph-workbench/proposed-relations/reviewed-endpoint-aliases.json";
pub const ATLAS_SCHEMA: &str = "ToS/contracts/philosophy-atlas-projection.schema.json";

/// Maintained standalone assertions, after schema and rebuild equality.
/// These constrain the derived carrier and do not admit philosophical meaning.
pub fn validate_assertions(current: &Value) -> Result<()> {
    let counts = &current["counts"];
    for (key, expected) in [
        ("master_tables", 3),
        ("master_rows", 190),
        ("dossiers", 190),
        ("dossier_node_rows", 7193),
        ("dossier_relation_rows", 8564),
        ("candidate_nodes", 7193),
        ("candidate_relations", 8564),
    ] {
        if counts[key].as_f64() != Some(expected as f64) {
            return Err(Error::Source(format!(
                "philosophy atlas projection must keep counts.{key}={expected}"
            )));
        }
    }
    if counts["graph_views"].as_f64().unwrap_or(0.0) < 1.0 {
        return Err(Error::Invalid(
            "philosophy atlas projection graph view route nodes",
        ));
    }
    if counts["nodes"].as_f64().unwrap_or(0.0) <= counts["master_rows"].as_f64().unwrap_or(0.0) {
        return Err(Error::Invalid(
            "philosophy atlas projection structural nodes",
        ));
    }
    if counts["edges"].as_f64().unwrap_or(0.0) <= counts["nodes"].as_f64().unwrap_or(0.0) {
        return Err(Error::Invalid("philosophy atlas projection graph edges"));
    }
    if current["runtime_projection_boundary"]["runtime_owner"].as_str() != Some("abyss-stack") {
        return Err(Error::Invalid(
            "philosophy atlas projection runtime ownership",
        ));
    }
    if current["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|item| item.is_object() && item["level"].as_str() == Some("error"))
    {
        return Err(Error::Invalid(
            "philosophy atlas projection error diagnostics",
        ));
    }
    Ok(())
}
pub const NODE_SOURCES: [&str; 3] = [
    "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-nodes/table-ii-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-nodes/table-iii-prepared-dossiers.jsonl",
];
pub const RELATION_SOURCES: [&str; 3] = [
    "ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-relations/table-ii-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-relations/table-iii-prepared-dossiers.jsonl",
];
const BACKLOGS: [(&str, &str); 3] = [
    ("source_anchor_backlog", "source_anchor_count"),
    ("term_index", "term_count"),
    ("transmission_backlog", "transmission_count"),
];
#[derive(Clone, Copy, Debug)]
pub struct AtlasLimits {
    pub max_source_file_bytes: usize,
    pub max_input_bytes: u64,
    pub max_records: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_output_bytes: usize,
    pub max_row_bytes: usize,
}
impl Default for AtlasLimits {
    fn default() -> Self {
        Self {
            max_source_file_bytes: 32 * 1024 * 1024,
            max_input_bytes: 64 * 1024 * 1024,
            max_records: 100_000,
            max_nodes: 100_000,
            max_edges: 200_000,
            max_output_bytes: 128 * 1024 * 1024,
            max_row_bytes: 8 * 1024 * 1024,
        }
    }
}
struct Snapshot<'a, F> {
    read: &'a mut F,
    limits: AtlasLimits,
    work: u64,
    digests: BTreeMap<String, String>,
    records: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    input_profile: PhilosophySourceReadProfile,
}
impl<F: FnMut(&str) -> Result<Vec<u8>>> Snapshot<'_, F> {
    fn raw(&mut self, path: &str) -> Result<Vec<u8>> {
        RelativePath::parse(path).map_err(|e| Error::Source(e.to_string()))?;
        if !path.starts_with("ToS/philosophy/") || path.split('/').any(|s| s == "payload") {
            return Err(Error::Invalid("philosophy exact authored path"));
        }
        let raw = (self.read)(path)?;
        self.work = self
            .work
            .checked_add(raw.len() as u64)
            .ok_or(Error::Budget("philosophy atlas inputs"))?;
        if raw.len() > self.limits.max_source_file_bytes || self.work > self.limits.max_input_bytes
        {
            return Err(Error::Budget("philosophy atlas inputs"));
        }
        let sha = Digest256::of_bytes(&raw).to_hex();
        if self.digests.get(path).is_some_and(|old| old != &sha) {
            return Err(Error::Invalid("philosophy atlas source drift"));
        }
        self.digests.insert(path.into(), sha);
        Ok(raw)
    }
    fn context(
        &self,
        path: &str,
        row: &Value,
        format: &str,
        line: Option<usize>,
        ordinal: Option<usize>,
    ) -> Result<Value> {
        let mut out = json!({"source_record":row,"source_record_ref":path,"source_file_sha256":self.digests[path],"source_record_sha256":digest(row,self.limits.max_source_file_bytes)?,"source_format":format});
        if let (Some(line), Some(ordinal)) = (line, ordinal) {
            out["source_line"] = json!(line);
            out["source_row"] = json!(ordinal);
        } else {
            out["source_pointer"] = json!("");
        }
        Ok(out)
    }
    fn object(&mut self, path: &str) -> Result<(Value, Value)> {
        let raw = self.raw(path)?;
        let v = object_with_profile(&raw, self.limits.max_source_file_bytes, self.input_profile)?;
        let c = self.context(path, &v, "json", None, None)?;
        Ok((v, c))
    }
    fn rows(&mut self, path: &str, key: Option<&str>) -> Result<Vec<(Value, Value)>> {
        let raw = self.raw(path)?;
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let mut offset = 0;
        let mut line = 0;
        while offset < raw.len() {
            check_run(self.deadline, self.cancelled)?;
            line += 1;
            let start = offset;
            while offset < raw.len() && !matches!(raw[offset], b'\r' | b'\n') {
                offset += 1;
            }
            let end = offset;
            if offset < raw.len() {
                let cr = raw[offset] == b'\r';
                offset += 1;
                if cr && raw.get(offset) == Some(&b'\n') {
                    offset += 1;
                }
            }
            let content = &raw[start..end];
            if content
                .iter()
                .all(|b| matches!(*b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
            {
                continue;
            }
            self.records = self
                .records
                .checked_add(1)
                .ok_or(Error::Budget("philosophy source records"))?;
            if self.records > self.limits.max_records {
                return Err(Error::Budget("philosophy source records"));
            }
            let row = object_with_profile(
                content,
                self.limits.max_source_file_bytes,
                self.input_profile,
            )?;
            if let Some(key) = key {
                if !seen.insert(required(&row, key)?.to_owned()) {
                    return Err(Error::Invalid("philosophy atlas row identity"));
                }
            }
            if row.get("source_ref").is_some_and(|v| v != path) {
                return Err(Error::Invalid("philosophy atlas exact source ref"));
            }
            let c = self.context(path, &row, "jsonl", Some(line), Some(result.len() + 1))?;
            result.push((row, c));
        }
        Ok(result)
    }
}
fn merge(mut v: Value, context: &Value) -> Result<Value> {
    let out = v
        .as_object_mut()
        .ok_or(Error::Invalid("philosophy properties object"))?;
    for (k, v) in context
        .as_object()
        .ok_or(Error::Invalid("philosophy source context"))?
    {
        out.insert(k.clone(), v.clone());
    }
    Ok(v)
}
fn clean(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.retain(|_, v| !v.is_null());
    }
    v
}
struct Material<'a> {
    nodes: Vec<Value>,
    edges: Vec<Value>,
    multilingual: &'a Multilingual,
    limits: AtlasLimits,
    used: usize,
    node_ids: BTreeSet<String>,
    edge_ids: BTreeSet<String>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl Material<'_> {
    fn charge(&mut self, v: &Value, node: bool) -> Result<()> {
        check_run(self.deadline, self.cancelled)?;
        let bytes = bytes(v, self.limits.max_row_bytes)?;
        self.used = self
            .used
            .checked_add(bytes.len())
            .ok_or(Error::Budget("philosophy atlas output"))?;
        if self.used > self.limits.max_output_bytes
            || if node {
                self.nodes.len() >= self.limits.max_nodes
            } else {
                self.edges.len() >= self.limits.max_edges
            }
        {
            return Err(Error::Budget("philosophy atlas output"));
        }
        Ok(())
    }
    fn node(&mut self, id: &str, kind: &str, label: &str, path: &str, props: Value) -> Result<()> {
        if !self.node_ids.insert(id.to_owned()) {
            return Err(Error::Invalid("philosophy atlas duplicate node identity"));
        }
        let props = clean(props);
        let mut language_props = props.clone();
        language_props["node_type"] = json!(kind);
        let v = json!({"node_id":id,"node_type":kind,"label":label,"multilingual":self.multilingual.label(label,path,&language_props)?,"source_ref":path,"properties":props});
        self.charge(&v, true)?;
        self.nodes.push(v);
        Ok(())
    }
    fn edge(
        &mut self,
        id: &str,
        from: &str,
        predicate: &str,
        to: &str,
        path: &str,
        props: Value,
    ) -> Result<()> {
        if !self.edge_ids.insert(id.to_owned()) {
            return Err(Error::Invalid("philosophy atlas duplicate edge identity"));
        }
        let v = json!({"edge_id":id,"from_id":from,"predicate_id":predicate,"to_id":to,"source_ref":path,"properties":clean(props)});
        self.charge(&v, false)?;
        self.edges.push(v);
        Ok(())
    }
}
fn row_fields(row: &Value) -> Value {
    let n = if row["normalized"].is_object() {
        &row["normalized"]
    } else {
        &Value::Null
    };
    let first = |names: &[&str]| {
        names
            .iter()
            .map(|k| &n[*k])
            .find(|v| truth(v))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let mut out = json!({});
    for k in [
        "table_id",
        "table_label",
        "row_order",
        "source_document",
        "source_section",
        "dossier_id",
        "dossier_available",
    ] {
        out[k] = row[k].clone();
    }
    for k in [
        "launch_order",
        "status",
        "confidence",
        "formation",
        "dossier_intake_status",
    ] {
        out[k] = n[k].clone();
    }
    out["research_node"] = {
        let v = first(&[
            "macroregion_research_node",
            "research_node",
            "node_and_task",
        ]);
        if truth(&v) { v } else { row["row_id"].clone() }
    };
    out["fixation"] = first(&[
        "written_fixation",
        "fixation_translation",
        "fixation_print_institutional_entry",
    ]);
    out["canonization"] = first(&[
        "canonization_redaction_commentary",
        "canonization_commentary",
        "canonization_academization",
    ]);
    out
}
fn qualified(label: &str) -> Option<&str> {
    let (id, tail) = label.split_once('/')?;
    if id
        .trim_matches(crate::source_philosophy_support::source_space)
        .is_empty()
        || tail
            .trim_matches(crate::source_philosophy_support::source_space)
            .is_empty()
    {
        None
    } else {
        Some(id.trim_matches(crate::source_philosophy_support::source_space))
    }
}
fn endpoint(dossier: &str, label: &str) -> String {
    format!(
        "candidate-endpoint:{dossier}:{}",
        &sha1_hex(&format!("{dossier}|{label}"))[..12]
    )
}
fn unavailable(label: &str, ids: &BTreeSet<String>) -> Option<String> {
    let mut ids = ids.iter().collect::<Vec<_>>();
    ids.sort_by_key(|id| std::cmp::Reverse(id.chars().count()));
    ids.into_iter()
        .find(|id| {
            label == id.as_str()
                || label
                    .strip_prefix(id.as_str())
                    .and_then(|t| t.chars().next())
                    .is_some_and(|c| matches!(c, ' ' | '/' | ':' | '—' | '–' | '-' | '('))
        })
        .cloned()
}
type AliasKey = (String, String, String);
fn aliases(
    payload: &Value,
    nodes: &[(Value, Value)],
    relations: &[(Value, Value)],
    dossiers: &BTreeSet<String>,
    l: AtlasLimits,
) -> Result<(BTreeMap<AliasKey, String>, BTreeMap<AliasKey, String>)> {
    if required(payload, "schema_version")? != "tos_reviewed_endpoint_aliases_v2" {
        return Err(Error::Invalid("philosophy reviewed alias version"));
    }
    let candidates = nodes
        .iter()
        .map(|(n, _)| Ok((required(n, "candidate_id")?.to_owned(), n)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut observed = BTreeSet::new();
    for (r, _) in relations {
        for (role, label) in [
            ("source", "source_endpoint_label"),
            ("target", "target_endpoint_label"),
        ] {
            observed.insert((
                fallback(&r["dossier_id"], ""),
                role.into(),
                fallback(&r[label], ""),
            ));
        }
    }
    let rows = array(payload, "aliases")?;
    if rows.len() > l.max_records {
        return Err(Error::Budget("philosophy endpoint aliases"));
    }
    let mut result = BTreeMap::new();
    let mut pointers = BTreeMap::new();
    for (i, row) in rows.iter().enumerate() {
        for k in [
            "endpoint_label",
            "origin_dossier_id",
            "endpoint_role",
            "target_dossier_id",
            "target_candidate_id",
            "target_label",
        ] {
            required(row, k)?;
        }
        let origin = required(row, "origin_dossier_id")?;
        let role = required(row, "endpoint_role")?;
        let label = required(row, "endpoint_label")?;
        let target = required(row, "target_dossier_id")?;
        let cid = required(row, "target_candidate_id")?;
        let key = (origin.into(), role.into(), label.into());
        if row["projection_review_status"] != "reviewed_for_pre_canon_routing"
            || !matches!(role, "source" | "target")
            || !dossiers.contains(origin)
            || !dossiers.contains(target)
            || !observed.contains(&key)
            || qualified(label).is_some_and(|d| d != target)
        {
            return Err(Error::Invalid("philosophy reviewed alias routing"));
        }
        let candidate = candidates
            .get(cid)
            .ok_or(Error::Invalid("philosophy alias candidate"))?;
        if candidate["dossier_id"] != target
            || candidate["label"] != row["target_label"]
            || result.insert(key.clone(), cid.into()).is_some()
        {
            return Err(Error::Invalid("philosophy alias candidate/label identity"));
        }
        pointers.insert(key, format!("/aliases/{i}"));
    }
    Ok((result, pointers))
}
fn alias(
    label: &str,
    dossier: &str,
    role: &str,
    dossiers: &BTreeSet<String>,
    aliases: &BTreeMap<AliasKey, String>,
) -> Option<String> {
    if !dossiers.contains(dossier) || qualified(label).is_some_and(|id| !dossiers.contains(id)) {
        return None;
    }
    aliases
        .get(&(dossier.into(), role.into(), label.into()))
        .cloned()
}
fn properties(row: &Value, keys: &[&str]) -> Value {
    let mut out = json!({});
    for k in keys {
        out[*k] = row[*k].clone();
    }
    out
}
pub fn build_atlas<F>(
    read: &mut F,
    optional_refs: &BTreeSet<String>,
    view_refs: &[String],
    multilingual: &Multilingual,
    l: AtlasLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    build_atlas_with_input_profile(
        read,
        optional_refs,
        view_refs,
        multilingual,
        l,
        deadline,
        cancelled,
        PhilosophySourceReadProfile::PublishedStrict,
    )
}
/// Same atlas algorithm with an explicit consumer decoding profile. Raw
/// callback bytes still supply source_file_sha256 before any JSON decoding.
pub fn build_atlas_with_input_profile<F>(
    read: &mut F,
    optional_refs: &BTreeSet<String>,
    view_refs: &[String],
    multilingual: &Multilingual,
    l: AtlasLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
    input_profile: PhilosophySourceReadProfile,
) -> Result<Value>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    if l.max_source_file_bytes == 0
        || l.max_source_file_bytes > 32 * 1024 * 1024
        || l.max_input_bytes == 0
        || l.max_records == 0
        || l.max_nodes == 0
        || l.max_edges == 0
        || l.max_output_bytes == 0
        || l.max_output_bytes > 256 * 1024 * 1024
        || l.max_row_bytes == 0
        || l.max_row_bytes > 8 * 1024 * 1024
        || view_refs.len() > l.max_nodes
    {
        return Err(Error::Budget("philosophy atlas limits"));
    }
    let mut s = Snapshot {
        read,
        limits: l,
        work: 0,
        digests: BTreeMap::new(),
        records: 0,
        deadline,
        cancelled,
        input_profile,
    };
    let (atlas, atlas_context) = s.object(ATLAS_SOURCE)?;
    let dossiers = s.rows(DOSSIERS_SOURCE, Some("dossier_id"))?;
    let (manifest, _) = s.object(DOSSIERS_MANIFEST)?;
    if manifest["branch_id"] != "philosophy.atlas.dossiers" {
        return Err(Error::Invalid("philosophy dossier backlog manifest"));
    }
    let by_id = dossiers
        .iter()
        .map(|(d, _)| Ok((required(d, "dossier_id")?.to_owned(), d)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let admitted = by_id.keys().cloned().collect::<BTreeSet<_>>();
    let mut backlogs = admitted
        .iter()
        .map(|id| (id.clone(), json!({})))
        .collect::<BTreeMap<_, _>>();
    let mut backlog_refs = BTreeSet::new();
    for (family, count_field) in BACKLOGS {
        let path = required(&manifest, family)?;
        if !path.ends_with(".jsonl") || !backlog_refs.insert(path.to_owned()) {
            return Err(Error::Invalid("philosophy backlog refs"));
        }
        let rows = s.rows(path, None)?;
        let sha = s.digests[path].clone();
        for (id, _) in &by_id {
            backlogs.get_mut(id).expect("dossier")[family] =
                json!({"source_ref":path,"source_file_sha256":sha,"record_count":0,"records":[]});
        }
        for (row, context) in rows {
            let id = required(&row, "dossier_id")?;
            let parent = by_id
                .get(id)
                .ok_or(Error::Invalid("philosophy backlog parent dossier"))?;
            if row["atlas_row_id"] != parent["dossier_id"]
                || row["source_ref"] != path
                || ["source_document", "branch_path"]
                    .iter()
                    .any(|k| row[*k] != parent[*k])
                || row
                    .get("table_id")
                    .is_some_and(|v| v != &parent["table_id"])
            {
                return Err(Error::Invalid("philosophy backlog exact parent"));
            }
            backlogs.get_mut(id).expect("dossier")[family]["records"]
                .as_array_mut()
                .expect("records")
                .push(context);
        }
        for (id, parent) in &by_id {
            let c = &mut backlogs.get_mut(id).expect("dossier")[family];
            let count = c["records"].as_array().expect("records").len();
            if parent[count_field].as_u64() != Some(count as u64) {
                return Err(Error::Invalid("philosophy declared dossier backlog count"));
            }
            c["record_count"] = json!(count);
        }
    }
    let shape = object_with_profile(&s.raw(GRAPH_SHAPE)?, l.max_source_file_bytes, input_profile)?;
    let mut candidate_nodes = Vec::new();
    let mut candidate_relations = Vec::new();
    for (refs, rows) in [
        (&NODE_SOURCES, &mut candidate_nodes),
        (&RELATION_SOURCES, &mut candidate_relations),
    ] {
        for path in refs {
            if optional_refs.contains(*path) {
                rows.extend(s.rows(path, Some("candidate_id"))?);
            }
        }
        let mut ids = BTreeSet::new();
        for (row, _) in rows.iter() {
            if !ids.insert(required(row, "candidate_id")?.to_owned()) {
                return Err(Error::Invalid(
                    "philosophy duplicate cross-stream candidate",
                ));
            }
        }
    }
    let mut m = Material {
        nodes: Vec::new(),
        edges: Vec::new(),
        multilingual,
        limits: l,
        used: 0,
        node_ids: BTreeSet::new(),
        edge_ids: BTreeSet::new(),
        deadline,
        cancelled,
    };
    let mut row_count = 0;
    let mut master_ids = BTreeSet::new();
    // These structural names are the maintained builder's derived topology grammar,
    // not hardcoded authored record IDs or a replacement identity registry.
    for (id, kind, label, path) in [
        (
            "philosophy",
            "domain-root",
            "Philosophy",
            "ToS/philosophy/philosophy.manifest.json",
        ),
        (
            "philosophy.atlas.master-tables",
            "atlas-section",
            "Master Tables",
            "ToS/philosophy/atlas/master-tables/branch.manifest.json",
        ),
        (
            "philosophy.atlas.dossiers",
            "atlas-section",
            "Dossiers",
            DOSSIERS_MANIFEST,
        ),
        (
            "philosophy.graph-views",
            "view-section",
            "Graph Views",
            "ToS/philosophy/graph-workbench/views/README.md",
        ),
    ] {
        m.node(id, kind, label, path, json!({}))?;
        if id == "philosophy" {
            m.node(
                "philosophy.atlas",
                "atlas",
                "Philosophy Atlas",
                ATLAS_SOURCE,
                atlas_context.clone(),
            )?;
        }
    }
    for (id, from, predicate, to) in [
        (
            "edge:philosophy:has-atlas",
            "philosophy",
            "has_atlas",
            "philosophy.atlas",
        ),
        (
            "edge:atlas:has-master-tables",
            "philosophy.atlas",
            "has_section",
            "philosophy.atlas.master-tables",
        ),
        (
            "edge:atlas:has-dossiers",
            "philosophy.atlas",
            "has_section",
            "philosophy.atlas.dossiers",
        ),
        (
            "edge:atlas:has-graph-views",
            "philosophy.atlas",
            "has_view_section",
            "philosophy.graph-views",
        ),
    ] {
        m.edge(id, from, predicate, to, ATLAS_SOURCE, json!({}))?;
    }
    let mut diagnostics = Vec::new();
    let tables = array(&atlas, "master_tables")?;
    if tables.len() > l.max_records {
        return Err(Error::Budget("philosophy atlas tables"));
    }
    for table in tables {
        if !table.is_object() {
            diagnostics.push(json!({"level":"error","path":ATLAS_SOURCE,"message":"master_tables entry is not an object"}));
            continue;
        }
        let id = fallback(&table["table_id"], "");
        let tid = format!("atlas-table:{id}");
        let manifest = required(table, "manifest")?;
        let path = required(table, "rows")?;
        let rows = s.rows(path, Some("row_id"))?;
        let (_, context) = s.object(manifest)?;
        row_count += rows.len();
        m.node(&tid,"master-table",&fallback(&table["table_label"],&id),manifest,merge(json!({"table_id":id,"row_count":rows.len(),"source_document":table["source_document"],"rows_ref":path}),&context)?)?;
        m.edge(
            &format!("edge:master-tables:contains:{id}"),
            "philosophy.atlas.master-tables",
            "contains_table",
            &tid,
            manifest,
            json!({"row_count":rows.len()}),
        )?;
        for (row, context) in rows {
            let rid = required(&row, "row_id")?;
            if !master_ids.insert(rid.to_owned()) {
                return Err(Error::Invalid("philosophy cross-table row identity"));
            }
            let nid = format!("atlas-row:{rid}");
            m.node(
                &nid,
                "master-table-row",
                rid,
                path,
                merge(row_fields(&row), &context)?,
            )?;
            m.edge(
                &format!("edge:{id}:contains-row:{rid}"),
                &tid,
                "contains_row",
                &nid,
                path,
                json!({"row_order":row["row_order"]}),
            )?;
            if let Some(d) = row["dossier_id"].as_str().filter(|s| !s.is_empty()) {
                m.edge(
                    &format!("edge:row:{rid}:has-dossier:{d}"),
                    &nid,
                    "has_prepared_dossier",
                    &format!("atlas-dossier:{d}"),
                    path,
                    json!({}),
                )?;
            }
        }
    }
    for (d, context) in &dossiers {
        let id = required(d, "dossier_id")?;
        let nid = format!("atlas-dossier:{id}");
        let mut props = properties(
            d,
            &[
                "dossier_id",
                "source_document",
                "node_row_count",
                "relation_row_count",
                "table_count",
                "table_id",
                "route_kind",
                "review_posture",
                "review_reason",
                "master_status",
                "master_confidence",
            ],
        );
        props["source_backlogs"] = backlogs[id].clone();
        m.node(
            &nid,
            "prepared-dossier",
            &fallback(&d["title"], id),
            DOSSIERS_SOURCE,
            merge(props, context)?,
        )?;
        m.edge(
            &format!("edge:dossiers:contains:{id}"),
            "philosophy.atlas.dossiers",
            "contains_dossier",
            &nid,
            DOSSIERS_SOURCE,
            json!({}),
        )?;
        for (field, predicate, kind) in [
            ("node_type_counts", "has_node_type_pressure", "node-type"),
            ("relation_counts", "has_relation_pressure", "relation-kind"),
        ] {
            if let Some(counts) = d[field].as_object() {
                for (key, count) in counts {
                    m.edge(
                        &format!("edge:dossier:{id}:{kind}:{key}"),
                        &nid,
                        predicate,
                        &format!("atlas-{kind}:{key}"),
                        DOSSIERS_SOURCE,
                        json!({"count":count}),
                    )?;
                }
            }
        }
    }
    for (field, predicate, kind, node_type) in [
        (
            "node_type_counts",
            "has_node_type_pressure",
            "node-type",
            "atlas-node-type",
        ),
        (
            "relation_counts",
            "has_relation_pressure",
            "relation-kind",
            "atlas-relation-kind",
        ),
    ] {
        if let Some(counts) = shape[field].as_object() {
            for (key, count) in counts {
                let id = format!("atlas-{kind}:{key}");
                m.node(&id, node_type, key, GRAPH_SHAPE, json!({"count":count}))?;
                m.edge(
                    &format!("edge:atlas:{kind}:{key}"),
                    "philosophy.atlas",
                    predicate,
                    &id,
                    GRAPH_SHAPE,
                    json!({"count":count}),
                )?;
            }
        }
    }
    for (n, context) in &candidate_nodes {
        let id = required(n, "candidate_id")?;
        let dossier = fallback(&n["dossier_id"], "");
        let path = fallback(&n["source_ref"], NODE_SOURCES[0]);
        let nid = format!("candidate-node:{id}");
        let mut props = properties(
            n,
            &[
                "candidate_id",
                "dossier_id",
                "atlas_row_id",
                "branch_path",
                "original_node_id",
                "period",
                "priority",
                "canon_status",
                "authority_posture",
                "source_document",
                "table_id",
                "route_kind",
                "review_posture",
                "review_reason",
                "master_status",
                "master_confidence",
            ],
        );
        props["original_node_type"] = n["node_kind"].clone();
        m.node(
            &nid,
            "candidate-node",
            &fallback(&n["label"], id),
            &path,
            merge(props, context)?,
        )?;
        if !dossier.is_empty() {
            m.edge(
                &format!("edge:dossier:{dossier}:candidate-node:{id}"),
                &format!("atlas-dossier:{dossier}"),
                "has_candidate_node",
                &nid,
                &path,
                json!({}),
            )?;
        }
    }
    let unavailable_ids = master_ids
        .difference(&admitted)
        .cloned()
        .collect::<BTreeSet<_>>();
    let (alias_payload, alias_context) = s.object(ALIASES_SOURCE)?;
    let (aliases, pointers) = aliases(
        &alias_payload,
        &candidate_nodes,
        &candidate_relations,
        &admitted,
        l,
    )?;
    let mut roles: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (r, _) in &candidate_relations {
        let d = fallback(&r["dossier_id"], "");
        for (role, cid, field, default) in [
            (
                "source",
                "source_candidate_id",
                "source_endpoint_label",
                "source endpoint",
            ),
            (
                "target",
                "target_candidate_id",
                "target_endpoint_label",
                "target endpoint",
            ),
        ] {
            let label = fallback(&r[field], default);
            if !r[cid].as_str().is_some_and(|s| !s.is_empty())
                && !admitted.contains(&label)
                && alias(&label, &d, role, &admitted, &aliases).is_none()
                && unavailable(&label, &unavailable_ids).is_none()
            {
                roles
                    .entry(endpoint(&d, &label))
                    .or_default()
                    .insert(role.into());
            }
        }
    }
    let mut endpoint_nodes = BTreeSet::new();
    for (r, context) in &candidate_relations {
        let id = required(r, "candidate_id")?;
        let dossier = fallback(&r["dossier_id"], "");
        let path = fallback(&r["source_ref"], RELATION_SOURCES[0]);
        let mut endpoints = Vec::new();
        let mut resolved_aliases = Vec::new();
        let mut resolved_master = Vec::new();
        let mut labels = Vec::new();
        for (role, cid, field, default) in [
            (
                "source",
                "source_candidate_id",
                "source_endpoint_label",
                "source endpoint",
            ),
            (
                "target",
                "target_candidate_id",
                "target_endpoint_label",
                "target endpoint",
            ),
        ] {
            let label = fallback(&r[field], default);
            let a = alias(&label, &dossier, role, &admitted, &aliases);
            let master = unavailable(&label, &unavailable_ids);
            let endpoint = if let Some(cid) = r[cid].as_str().filter(|s| !s.is_empty()) {
                format!("candidate-node:{cid}")
            } else if admitted.contains(&label) {
                format!("atlas-dossier:{label}")
            } else if let Some(ref cid) = a {
                format!("candidate-node:{cid}")
            } else if let Some(ref rid) = master {
                format!("atlas-row:{rid}")
            } else {
                let nid = endpoint(&dossier, &label);
                if endpoint_nodes.insert(nid.clone()) {
                    let observed = roles
                        .get(&nid)
                        .ok_or(Error::Invalid("philosophy endpoint observed roles"))?;
                    let mut props = properties(
                        r,
                        &[
                            "branch_path",
                            "table_id",
                            "route_kind",
                            "review_posture",
                            "review_reason",
                            "master_status",
                            "master_confidence",
                        ],
                    );
                    props["dossier_id"] = json!(dossier);
                    props["canon_status"] = json!("pre-canon");
                    props["endpoint_role"] = json!(if observed.len() == 1 {
                        observed.first().expect("role").as_str()
                    } else {
                        "source_and_target"
                    });
                    props["endpoint_roles"] = json!(observed);
                    m.node(&nid, "candidate-endpoint", &label, &path, props)?;
                }
                nid
            };
            endpoints.push(endpoint);
            resolved_aliases.push(a);
            resolved_master.push(master);
            labels.push(label);
        }
        let mut props = properties(
            r,
            &[
                "candidate_id",
                "dossier_id",
                "atlas_row_id",
                "branch_path",
                "relation_label",
                "confidence",
                "canon_status",
                "authority_posture",
                "endpoint_resolution",
                "comment",
                "table_id",
                "route_kind",
                "review_posture",
                "review_reason",
                "master_status",
                "master_confidence",
            ],
        );
        let aliased = resolved_aliases.iter().any(Option::is_some);
        let posture = if aliased {
            Some(
                if resolved_aliases
                    .iter()
                    .zip(&labels)
                    .any(|(a, label)| a.is_some() && qualified(label).is_none())
                {
                    "reviewed_origin_role_alias"
                } else {
                    "reviewed_qualified_alias"
                },
            )
        } else if resolved_master.iter().any(Option::is_some) {
            Some("known_unavailable_master_row")
        } else if labels.iter().any(|s| admitted.contains(s)) {
            Some("exact_admitted_dossier")
        } else {
            None
        };
        props["projection_endpoint_resolution"] = json!(posture);
        if aliased {
            props["endpoint_alias_ref"] = json!(ALIASES_SOURCE);
            props["endpoint_alias_source"] = alias_context.clone();
            props["endpoint_alias_pointers"] = json!(
                resolved_aliases
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.is_some())
                    .map(|(i, _)| pointers[&(
                        dossier.clone(),
                        if i == 0 { "source" } else { "target" }.into(),
                        labels[i].clone()
                    )]
                        .clone())
                    .collect::<Vec<_>>()
            );
        }
        m.edge(
            &format!("edge:candidate-relation:{id}"),
            &endpoints[0],
            &fallback(&r["relation_kind"], "related_to"),
            &endpoints[1],
            &path,
            merge(props, context)?,
        )?;
    }
    for path in view_refs {
        let filename = path
            .strip_prefix("ToS/philosophy/graph-workbench/views/")
            .and_then(|s| s.strip_suffix(".graph.md"))
            .filter(|s| !s.contains('/'))
            .ok_or(Error::Invalid("philosophy view selected path"))?;
        let id = format!("graph-view:{filename}");
        m.node(&id, "graph-view", filename, path, json!({"view_file":path}))?;
        m.edge(
            &format!("edge:graph-views:contains:{filename}"),
            "philosophy.graph-views",
            "contains_view",
            &id,
            path,
            json!({}),
        )?;
    }
    for edge in &m.edges {
        if !m.node_ids.contains(required(edge, "from_id")?)
            || !m.node_ids.contains(required(edge, "to_id")?)
        {
            return Err(Error::Invalid("philosophy atlas exact endpoint closure"));
        }
    }
    let out = json!({"schema_version":"tos_philosophy_atlas_projection_v1","schema_ref":ATLAS_SCHEMA,"owner_repo":"Tree-of-Sophia","surface_kind":"derived_philosophy_atlas_projection","source_atlas_ref":ATLAS_SOURCE,"content_language_contract":multilingual.content_language_contract()?,"runtime_projection_boundary":{"runtime_owner":"abyss-stack","runtime_scope":["read this projection as a ToS-owned graph input","serve MCP/API resources that point back to ToS surfaces","render UI, graph layout, and local caches downstream"],"tos_authority_scope":["canon remains in ToS canon and source-owned atlas surfaces","source witnesses and atlas rows remain the authored evidence route","runtime graph state returns through an explicit ToS change route"]},"validation_refs":["scripts/build_philosophy_atlas_projection.py","scripts/validate_philosophy_atlas_projection.py","tests/test_philosophy_atlas_projection.py"],"counts":{"master_tables":tables.len(),"master_rows":row_count,"dossiers":dossiers.len(),"dossier_node_rows":if truth(&shape["node_row_count"]){shape["node_row_count"].clone()}else{json!(0)},"dossier_relation_rows":if truth(&shape["relation_row_count"]){shape["relation_row_count"].clone()}else{json!(0)},"candidate_nodes":candidate_nodes.len(),"candidate_relations":candidate_relations.len(),"candidate_endpoint_placeholders":endpoint_nodes.len(),"graph_views":view_refs.len(),"nodes":m.nodes.len(),"edges":m.edges.len(),"diagnostics":diagnostics.len()},"nodes":m.nodes,"edges":m.edges,"diagnostics":diagnostics});
    bytes(&out, l.max_output_bytes)?;
    Ok(out)
}

/// Portable source-body and locator consistency, never export authentication.
/// Validate real nested backlog and reviewed-alias bodies before graph use.
pub(crate) fn validate_authored_context(
    item: &Value,
    max_row: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    fn inner(
        item: &Value,
        max_row: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        depth: usize,
        visits: &mut usize,
    ) -> Result<()> {
        check_run(deadline, cancelled)?;
        *visits = visits
            .checked_add(1)
            .ok_or(Error::Budget("philosophy source-context visits"))?;
        if depth > 96 || *visits > 2_000_000 {
            return Err(Error::Budget("philosophy source-context traversal"));
        }
        let p = &item["properties"];
        let Some(record) = p.get("source_record") else {
            return Ok(());
        };
        if !record.is_object()
            || p["source_record_ref"] != item["source_ref"]
            || p["source_record_sha256"] != digest(record, max_row)?
        {
            return Err(Error::Invalid(
                "philosophy source-context body/owner digest",
            ));
        }
        for key in ["source_row", "source_line"] {
            if p.get(key)
                .is_some_and(|v| v.as_u64().is_none_or(|n| n == 0))
            {
                return Err(Error::Invalid("philosophy positive source locator"));
            }
        }
        if let Some(alias) = p.get("endpoint_alias_source").filter(|v| !v.is_null()) {
            if !alias.is_object() || p["endpoint_alias_ref"] != ALIASES_SOURCE {
                return Err(Error::Invalid("philosophy alias context route"));
            }
            inner(
                &json!({"source_ref":ALIASES_SOURCE,"properties":alias}),
                max_row,
                deadline,
                cancelled,
                depth + 1,
                visits,
            )?;
            let aliases = array(&alias["source_record"], "aliases")?;
            let pointers = array(p, "endpoint_alias_pointers")?;
            if pointers.is_empty()
                || pointers.iter().any(|pointer| {
                    pointer
                        .as_str()
                        .and_then(|s| s.strip_prefix("/aliases/"))
                        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                        .and_then(|s| s.parse::<usize>().ok())
                        .is_none_or(|index| index >= aliases.len())
                })
            {
                return Err(Error::Invalid("philosophy alias source pointer"));
            }
        }
        if let Some(families) = p.get("source_backlogs") {
            let parent_id = required(record, "dossier_id")?;
            let families = families
                .as_object()
                .ok_or(Error::Invalid("philosophy backlog family object"))?;
            if families.len() != BACKLOGS.len()
                || item["node_id"] != format!("atlas-dossier:{parent_id}")
            {
                return Err(Error::Invalid("philosophy backlog dossier owner"));
            }
            let mut refs = BTreeSet::new();
            for (family, count_field) in BACKLOGS {
                let context = families
                    .get(family)
                    .ok_or(Error::Invalid("philosophy backlog exact family set"))?;
                let source_ref = required(context, "source_ref")?;
                let records = array(context, "records")?;
                if !source_ref.starts_with("ToS/philosophy/")
                    || !source_ref.ends_with(".jsonl")
                    || !refs.insert(source_ref)
                    || context["record_count"].as_u64() != Some(records.len() as u64)
                    || record[count_field].as_u64() != Some(records.len() as u64)
                {
                    return Err(Error::Invalid("philosophy backlog family count/route"));
                }
                let (mut prior_row, mut prior_line) = (0, 0);
                for entry in records {
                    let child = &entry["source_record"];
                    let row = entry["source_row"]
                        .as_u64()
                        .ok_or(Error::Invalid("philosophy backlog row"))?;
                    let line = entry["source_line"]
                        .as_u64()
                        .ok_or(Error::Invalid("philosophy backlog line"))?;
                    if !child.is_object()
                        || entry["source_format"] != "jsonl"
                        || child["source_ref"] != source_ref
                        || entry["source_file_sha256"] != context["source_file_sha256"]
                        || child["dossier_id"] != parent_id
                        || child["atlas_row_id"] != parent_id
                        || ["source_document", "branch_path"]
                            .iter()
                            .any(|key| child[key] != record[key])
                        || child
                            .get("table_id")
                            .is_some_and(|v| v != &record["table_id"])
                        || row <= prior_row
                        || line <= prior_line
                    {
                        return Err(Error::Invalid("philosophy backlog body/parent/locator"));
                    }
                    inner(
                        &json!({"source_ref":source_ref,"properties":entry}),
                        max_row,
                        deadline,
                        cancelled,
                        depth + 1,
                        visits,
                    )?;
                    (prior_row, prior_line) = (row, line);
                }
            }
        }
        Ok(())
    }
    inner(item, max_row, deadline, cancelled, 0, &mut 0)
}

#[cfg(test)]
mod context_tests {
    use super::*;
    #[test]
    fn legacy_decoding_keeps_the_original_file_witness() {
        let raw = b" {\"value\": 1, \"value\": 2}\n".to_vec();
        let mut read = |_: &str| Ok(raw.clone());
        let cancelled = AtomicBool::new(false);
        let mut snapshot = Snapshot {
            read: &mut read,
            limits: AtlasLimits::default(),
            work: 0,
            digests: BTreeMap::new(),
            records: 0,
            deadline: Instant::now() + std::time::Duration::from_secs(5),
            cancelled: &cancelled,
            input_profile: PhilosophySourceReadProfile::LegacyPythonJsonLoads,
        };
        let (record, context) = snapshot.object("ToS/philosophy/source.json").unwrap();
        assert_eq!(record["value"], 2);
        assert_eq!(
            context["source_file_sha256"],
            Digest256::of_bytes(&raw).to_hex()
        );
        assert_eq!(
            context["source_record_sha256"],
            digest(&record, 4096).unwrap()
        );
        assert_ne!(
            context["source_file_sha256"],
            context["source_record_sha256"]
        );
    }
    #[test]
    fn graph_never_returns_changed_body_or_boolean_locator_as_exact_source() {
        let record = json!({"unknown":{"meaning":"source remains exact"}});
        let mut item = json!({"source_ref":"ToS/philosophy/source.jsonl","properties":{"source_record":record,"source_record_ref":"ToS/philosophy/source.jsonl","source_record_sha256":digest(&record,8192).unwrap(),"source_line":1,"source_row":1}});
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        let cancelled = AtomicBool::new(false);
        validate_authored_context(&item, 8192, deadline, &cancelled).unwrap();
        item["properties"]["source_row"] = json!(true);
        assert!(validate_authored_context(&item, 8192, deadline, &cancelled).is_err());
        item["properties"]["source_row"] = json!(1);
        item["properties"]["source_record"]["unknown"]["meaning"] = json!("changed");
        assert!(validate_authored_context(&item, 8192, deadline, &cancelled).is_err());
    }
}
