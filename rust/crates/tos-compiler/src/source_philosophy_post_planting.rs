//! Native producer for the maintained ToS philosophy post-planting audit.
//! This is a read-only projection of authored/current material. It does not
//! admit source, graph candidates, canon, publication, or runtime state.

use crate::source_philosophy_support::{check_run, fallback, source_space};
use crate::{Error, Result};
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, Write};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub const AUDIT_JSON_REF: &str =
    "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json";
pub const AUDIT_MARKDOWN_REF: &str =
    "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.md";
const TABLE_ROOT: &str = "ToS/philosophy/atlas/master-tables";
const DOSSIER_INDEX_REF: &str = "ToS/philosophy/atlas/dossiers/index.jsonl";
const DOSSIER_SUMMARY_REF: &str = "ToS/philosophy/atlas/dossiers/graph-shape-summary.json";
const SOURCE_ANCHOR_BACKLOG_REF: &str = "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl";
const TERM_INDEX_REF: &str = "ToS/philosophy/atlas/dossiers/term-index.jsonl";
const TRANSMISSION_BACKLOG_REF: &str = "ToS/philosophy/atlas/dossiers/transmission-backlog.jsonl";
const GRAPH_PROJECTION_REF: &str = "ToS/derived-exports/philosophy_graph_projection.min.json";
const GRAPH_VIEWS_REF: &str = "ToS/derived-exports/philosophy_graph_views.min.json";
const ATLAS_PROJECTION_REF: &str = "ToS/derived-exports/philosophy_atlas_projection.min.json";
const BRANCH_SUMMARY_SUFFIX: &str = "graph-workbench/pre-canon-summary.json";
const BRANCH_SUPPORT: [&str; 4] = [
    "README.md",
    "branch.manifest.json",
    "sources/source-anchor-backlog.jsonl",
    "graph-workbench/pre-canon-summary.json",
];
const PROPOSED_NODES_REFS: [&str; 3] = [
    "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-nodes/table-ii-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-nodes/table-iii-prepared-dossiers.jsonl",
];
const PROPOSED_RELATIONS_REFS: [&str; 3] = [
    "ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-relations/table-ii-prepared-dossiers.jsonl",
    "ToS/philosophy/graph-workbench/proposed-relations/table-iii-prepared-dossiers.jsonl",
];
const LANGUAGE_PACKET_REFS: [&str; 3] = [
    "ToS/philosophy/graph-workbench/language-packets/table-i-text-bearing-nodes.jsonl",
    "ToS/philosophy/graph-workbench/language-packets/table-ii-text-bearing-nodes.jsonl",
    "ToS/philosophy/graph-workbench/language-packets/table-iii-text-bearing-nodes.jsonl",
];
const BRANCH_FRAGMENT_REFS: [&str; 3] = [
    "ToS/philosophy/graph-workbench/branch-fragments/table-i-prepared-dossier-branches.json",
    "ToS/philosophy/graph-workbench/branch-fragments/table-ii-prepared-dossier-branches.json",
    "ToS/philosophy/graph-workbench/branch-fragments/table-iii-prepared-dossier-branches.json",
];

#[derive(Clone, Copy, Debug)]
pub struct AuditLimits {
    pub max_authored_file_bytes: usize,
    pub max_projection_bytes: usize,
    pub max_total_source_bytes: usize,
    pub max_rows: usize,
    pub max_enumerated_refs: usize,
    pub max_work_units: u64,
    pub max_output_bytes: usize,
}

impl Default for AuditLimits {
    fn default() -> Self {
        Self {
            max_authored_file_bytes: 32 * 1024 * 1024,
            max_projection_bytes: 128 * 1024 * 1024,
            max_total_source_bytes: 256 * 1024 * 1024,
            max_rows: 1_000_000,
            max_enumerated_refs: 100_000,
            max_work_units: 1_000_000_000,
            max_output_bytes: 32 * 1024 * 1024,
        }
    }
}

impl AuditLimits {
    fn validate(self) -> Result<()> {
        if self.max_authored_file_bytes == 0
            || self.max_authored_file_bytes > 32 * 1024 * 1024
            || self.max_projection_bytes == 0
            || self.max_projection_bytes > 128 * 1024 * 1024
            || self.max_total_source_bytes == 0
            || self.max_total_source_bytes > 512 * 1024 * 1024
            || self.max_rows == 0
            || self.max_rows > 2_000_000
            || self.max_enumerated_refs == 0
            || self.max_enumerated_refs > 100_000
            || self.max_work_units == 0
            || self.max_work_units > 1_000_000_000
            || self.max_output_bytes == 0
            || self.max_output_bytes > 32 * 1024 * 1024
        {
            return Err(Error::Budget("philosophy post-planting audit limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AuditPacket {
    pub payload: Value,
    pub json: String,
    pub markdown: String,
}

struct Work<'a> {
    limits: AuditLimits,
    source_bytes: usize,
    rows: usize,
    units: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl Work<'_> {
    fn check(&self) -> Result<()> {
        check_run(self.deadline, self.cancelled)
    }

    fn charge(&mut self, units: usize) -> Result<()> {
        self.check()?;
        self.units = self
            .units
            .checked_add(units as u64)
            .ok_or(Error::Budget("philosophy post-planting audit work"))?;
        if self.units > self.limits.max_work_units {
            return Err(Error::Budget("philosophy post-planting audit work"));
        }
        Ok(())
    }

    fn read<R>(&mut self, read: &mut R, path: &str, limit: usize) -> Result<Vec<u8>>
    where
        R: FnMut(&str, usize) -> Result<Vec<u8>>,
    {
        self.check()?;
        let raw = read(path, limit)?;
        if raw.len() > limit {
            return Err(Error::Budget("philosophy post-planting audit file bytes"));
        }
        self.source_bytes = self
            .source_bytes
            .checked_add(raw.len())
            .ok_or(Error::Budget("philosophy post-planting audit source bytes"))?;
        if self.source_bytes > self.limits.max_total_source_bytes {
            return Err(Error::Budget("philosophy post-planting audit source bytes"));
        }
        self.charge(raw.len())?;
        Ok(raw)
    }

    fn count_rows(&mut self, count: usize) -> Result<()> {
        self.rows = self
            .rows
            .checked_add(count)
            .ok_or(Error::Budget("philosophy post-planting audit row count"))?;
        if self.rows > self.limits.max_rows {
            return Err(Error::Budget("philosophy post-planting audit row count"));
        }
        self.charge(count)
    }
}

fn object(raw: &[u8]) -> Result<Value> {
    let value = crate::source_philosophy_support::parse_with_profile(
        raw,
        raw.len(),
        crate::PhilosophySourceReadProfile::LegacyPythonJsonLoads,
    )?;
    if !value.is_object() {
        return Err(Error::Invalid(
            "philosophy post-planting audit source object",
        ));
    }
    Ok(value)
}

fn jsonl(raw: &[u8], work: &mut Work<'_>) -> Result<Vec<Value>> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| Error::Source("philosophy post-planting audit UTF-8".into()))?;
    // Path.read_text uses universal newline handling. Keep the same line
    // boundaries for LF, CRLF, and lone CR source files.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut rows = Vec::new();
    for line in normalized.split('\n') {
        work.check()?;
        if line.trim_matches(source_space).is_empty() {
            continue;
        }
        let row = object(line.as_bytes())?;
        rows.push(row);
        work.count_rows(1)?;
    }
    Ok(rows)
}

fn read_json<R>(read: &mut R, work: &mut Work<'_>, path: &str) -> Result<Value>
where
    R: FnMut(&str, usize) -> Result<Vec<u8>>,
{
    let raw = work.read(read, path, work.limits.max_authored_file_bytes)?;
    object(&raw)
}

fn read_jsonl<R>(read: &mut R, work: &mut Work<'_>, path: &str) -> Result<Vec<Value>>
where
    R: FnMut(&str, usize) -> Result<Vec<u8>>,
{
    let raw = work.read(read, path, work.limits.max_authored_file_bytes)?;
    jsonl(&raw, work)
}

fn table_ref(table_id: &str) -> String {
    format!("{TABLE_ROOT}/{table_id}/rows.jsonl")
}

fn table_audit(source_ref: &str, rows: &[Value], work: &mut Work<'_>) -> Result<Value> {
    let mut available = Vec::new();
    let mut unavailable = Vec::new();
    let mut unavailable_by_status: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        work.charge(1)?;
        let row_id = fallback(&row["row_id"], "");
        if row["dossier_available"] == true {
            available.push(row_id);
        } else {
            unavailable.push(row_id.clone());
            let normalized = row.get("normalized").filter(|value| value.is_object());
            let status = normalized
                .map(|value| fallback(&value["dossier_intake_status"], "status_not_declared"))
                .unwrap_or_else(|| "status_not_declared".to_owned());
            unavailable_by_status
                .entry(status)
                .or_default()
                .push(row_id);
        }
    }
    for ids in unavailable_by_status.values_mut() {
        ids.sort();
    }
    Ok(json!({
        "row_count": rows.len(),
        "dossier_available_count": available.len(),
        "dossier_unavailable_count": unavailable.len(),
        "available_row_ids": available,
        "unavailable_row_ids": unavailable,
        "unavailable_row_ids_by_intake_status": unavailable_by_status,
        "source_ref": source_ref,
    }))
}

fn push_diagnostic(diagnostics: &mut Vec<Value>, level: &str, path: &str, message: &str) {
    diagnostics.push(json!({"level":level,"path":path,"message":message}));
}

fn branch_audit<E, G>(
    dossier_rows: &[Value],
    exists: &mut E,
    enumerate_refs: &mut G,
    work: &mut Work<'_>,
) -> Result<Value>
where
    E: FnMut(&str) -> Result<bool>,
    G: FnMut(&str, &str, usize) -> Result<Vec<String>>,
{
    let mut branch_paths = BTreeSet::new();
    for row in dossier_rows {
        work.charge(1)?;
        if let Some(path) = row["branch_path"].as_str().filter(|path| !path.is_empty()) {
            branch_paths.insert(path.to_owned());
        }
    }

    let mut missing_branch_paths = Vec::new();
    let mut missing_support = Vec::new();
    for branch_path in &branch_paths {
        work.charge(1)?;
        if !exists(branch_path)? {
            missing_branch_paths.push(branch_path.clone());
            continue;
        }
        for child in BRANCH_SUPPORT {
            work.charge(1)?;
            let support_ref = format!("{branch_path}/{child}");
            if !exists(&support_ref)? {
                missing_support.push(json!({"branch_path":branch_path,"missing":child}));
            }
        }
    }

    let mut local_summaries = BTreeSet::new();
    let mut enumerated_count = 0usize;
    for root in ["ToS/philosophy/eras", "ToS/philosophy/frontiers"] {
        work.check()?;
        let refs = enumerate_refs(root, BRANCH_SUMMARY_SUFFIX, work.limits.max_enumerated_refs)?;
        if refs.len() > work.limits.max_enumerated_refs {
            return Err(Error::Budget("philosophy post-planting audit branch refs"));
        }
        enumerated_count = enumerated_count
            .checked_add(refs.len())
            .ok_or(Error::Budget("philosophy post-planting audit branch refs"))?;
        if enumerated_count > work.limits.max_enumerated_refs {
            return Err(Error::Budget("philosophy post-planting audit branch refs"));
        }
        for path in refs {
            work.charge(1)?;
            if !path.starts_with(&format!("{root}/")) || !path.ends_with(BRANCH_SUMMARY_SUFFIX) {
                return Err(Error::Invalid(
                    "philosophy post-planting audit enumerated ref",
                ));
            }
            local_summaries.insert(path);
        }
    }
    let expected_summaries = branch_paths
        .iter()
        .map(|branch| format!("{branch}/graph-workbench/pre-canon-summary.json"))
        .collect::<BTreeSet<_>>();
    let orphan_summaries = local_summaries
        .difference(&expected_summaries)
        .cloned()
        .collect::<Vec<_>>();

    Ok(json!({
        "prepared_branch_count": branch_paths.len(),
        "prepared_branch_paths": branch_paths,
        "missing_branch_paths": missing_branch_paths,
        "missing_branch_support": missing_support,
        "orphan_local_graph_summaries": orphan_summaries,
    }))
}

#[derive(Default)]
struct Counter {
    items: Vec<(String, usize)>,
    indexes: BTreeMap<String, usize>,
}

impl Counter {
    fn add(&mut self, key: String) {
        if let Some(index) = self.indexes.get(&key).copied() {
            self.items[index].1 += 1;
        } else {
            self.indexes.insert(key.clone(), self.items.len());
            self.items.push((key, 1));
        }
    }

    fn sorted_object(&self) -> Map<String, Value> {
        self.items
            .iter()
            .map(|(key, count)| (key.clone(), json!(count)))
            .collect()
    }

    fn most_common(&self, limit: usize) -> Map<String, Value> {
        let mut ranked = self.items.iter().enumerate().collect::<Vec<_>>();
        ranked.sort_by(|(left_index, (_, left)), (right_index, (_, right))| {
            right.cmp(left).then_with(|| left_index.cmp(right_index))
        });
        ranked
            .into_iter()
            .take(limit)
            .map(|(_, (key, count))| (key.clone(), json!(count)))
            .collect()
    }
}

fn graph_workbench_audit(
    proposed_nodes: &[Value],
    proposed_relations: &[Value],
    language_packets: &[Value],
    branch_fragments: &[Value],
    work: &mut Work<'_>,
) -> Result<Value> {
    let mut endpoint_resolution_counts = Counter::default();
    for row in proposed_relations {
        work.charge(1)?;
        endpoint_resolution_counts.add(fallback(&row["endpoint_resolution"], "missing"));
    }
    let mut node_kind_counts = Counter::default();
    for row in proposed_nodes {
        work.charge(1)?;
        node_kind_counts.add(fallback(&row["node_kind"], "unspecified"));
    }
    let mut relation_kind_counts = Counter::default();
    for row in proposed_relations {
        work.charge(1)?;
        relation_kind_counts.add(fallback(&row["relation_kind"], "related_to"));
    }

    let mut branch_fragment_count = 0i128;
    let mut all_pre_canon = true;
    for fragment in branch_fragments {
        work.charge(1)?;
        let count = py_int_or_zero(&fragment["branch_count"])?;
        branch_fragment_count = branch_fragment_count
            .checked_add(count)
            .ok_or(Error::Budget("philosophy post-planting audit branch count"))?;
        all_pre_canon &= fragment["canon_status"] == "pre-canon";
    }

    Ok(json!({
        "proposed_node_count": proposed_nodes.len(),
        "proposed_relation_count": proposed_relations.len(),
        "language_packet_count": language_packets.len(),
        "text_bearing_node_count": node_kind_counts.items.iter().find(|(key, _)| key == "text_corpus").map_or(0, |(_, count)| *count),
        "endpoint_resolution_counts": endpoint_resolution_counts.sorted_object(),
        "node_kind_top": node_kind_counts.most_common(12),
        "relation_kind_top": relation_kind_counts.most_common(12),
        "branch_fragment_count": branch_fragment_count,
        "canon_status": if all_pre_canon {"pre-canon"} else {"mixed"},
        "source_refs": vec![
            PROPOSED_NODES_REFS[0],
            PROPOSED_NODES_REFS[1],
            PROPOSED_NODES_REFS[2],
            PROPOSED_RELATIONS_REFS[0],
            PROPOSED_RELATIONS_REFS[1],
            PROPOSED_RELATIONS_REFS[2],
            LANGUAGE_PACKET_REFS[0],
            LANGUAGE_PACKET_REFS[1],
            LANGUAGE_PACKET_REFS[2],
            BRANCH_FRAGMENT_REFS[0],
            BRANCH_FRAGMENT_REFS[1],
            BRANCH_FRAGMENT_REFS[2],
        ],
    }))
}

fn py_int_or_zero(value: &Value) -> Result<i128> {
    if !crate::source_philosophy_support::truth(value) {
        return Ok(0);
    }
    let integer = match value {
        Value::Bool(value) => i128::from(u8::from(*value)),
        Value::Number(value) => {
            if let Some(n) = value.as_i64() {
                i128::from(n)
            } else if let Some(n) = value.as_u64() {
                i128::from(n)
            } else if let Some(n) = value.as_f64() {
                if !n.is_finite() || n < i128::MIN as f64 || n > i128::MAX as f64 {
                    return Err(Error::Invalid("philosophy post-planting audit integer"));
                }
                n as i128
            } else {
                return Err(Error::Invalid("philosophy post-planting audit integer"));
            }
        }
        Value::String(value) => value
            .trim_matches(source_space)
            .parse::<i128>()
            .map_err(|_| Error::Invalid("philosophy post-planting audit integer"))?,
        _ => return Err(Error::Invalid("philosophy post-planting audit integer")),
    };
    Ok(integer)
}

struct ProjectionCounts {
    counts: Value,
    snapshot_ready: bool,
}

struct SnapshotIsObject(bool);

impl<'de> Deserialize<'de> for SnapshotIsObject {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SnapshotVisitor;
        impl<'de> Visitor<'de> for SnapshotVisitor {
            type Value = bool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("any JSON value")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                while map.next_key::<IgnoredAny>()?.is_some() {
                    let _: IgnoredAny = map.next_value()?;
                }
                Ok(true)
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(false)
            }

            fn visit_bool<E>(self, _: bool) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(false)
            }

            fn visit_i64<E>(self, _: i64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(false)
            }

            fn visit_u64<E>(self, _: u64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(false)
            }

            fn visit_f64<E>(self, _: f64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(false)
            }

            fn visit_str<E>(self, _: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(false)
            }

            fn visit_string<E>(self, _: String) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(false)
            }

            fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(false)
            }
        }
        deserializer
            .deserialize_any(SnapshotVisitor)
            .map(SnapshotIsObject)
    }
}

impl<'de> Deserialize<'de> for ProjectionCounts {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ProjectionVisitor;
        impl<'de> Visitor<'de> for ProjectionVisitor {
            type Value = ProjectionCounts;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a philosophy graph projection JSON object")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut counts = Value::Null;
                let mut snapshot_ready = false;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "counts" => counts = map.next_value()?,
                        "snapshot_review" => {
                            snapshot_ready = map.next_value::<SnapshotIsObject>()?.0
                        }
                        _ => {
                            let _: IgnoredAny = map.next_value()?;
                        }
                    }
                }
                Ok(ProjectionCounts {
                    counts,
                    snapshot_ready,
                })
            }
        }
        deserializer.deserialize_map(ProjectionVisitor)
    }
}

fn projection_audit<R>(read: &mut R, work: &mut Work<'_>) -> Result<Value>
where
    R: FnMut(&str, usize) -> Result<Vec<u8>>,
{
    // The maintained projection can be tens of MiB. Parse its entire syntax,
    // but retain only the two fields the Python producer observes.
    let raw = work.read(read, GRAPH_PROJECTION_REF, work.limits.max_projection_bytes)?;
    work.check()?;
    let mut deserializer = serde_json::Deserializer::from_slice(&raw);
    let slim = ProjectionCounts::deserialize(&mut deserializer)
        .map_err(|error| Error::Source(error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| Error::Source(error.to_string()))?;
    work.check()?;
    let empty_counts = Value::Null;
    let counts = if slim.counts.is_object() {
        &slim.counts
    } else {
        &empty_counts
    };
    let count = |key: &str| -> Result<i128> { py_int_or_zero(&counts[key]) };
    Ok(json!({
        "views": count("views")?,
        "graph_layers": count("graph_layers")?,
        "nodes": count("nodes")?,
        "edges": count("edges")?,
        "clusters": count("clusters")?,
        "review_packets": count("review_packets")?,
        "unresolved_review_surfaces": count("unresolved_review_surfaces")?,
        "diagnostics": count("diagnostics")?,
        "snapshot_ready": slim.snapshot_ready,
        "source_ref": GRAPH_PROJECTION_REF,
    }))
}

fn build_payload<R, E, G>(
    read: &mut R,
    exists: &mut E,
    enumerate_refs: &mut G,
    work: &mut Work<'_>,
) -> Result<Value>
where
    R: FnMut(&str, usize) -> Result<Vec<u8>>,
    E: FnMut(&str) -> Result<bool>,
    G: FnMut(&str, &str, usize) -> Result<Vec<String>>,
{
    let mut table_audits = BTreeMap::new();
    for table_id in ["table-i", "table-ii", "table-iii"] {
        let source_ref = table_ref(table_id);
        let rows = read_jsonl(read, work, &source_ref)?;
        let audit = table_audit(&source_ref, &rows, work)?;
        table_audits.insert(table_id.to_owned(), audit);
    }

    let dossier_rows = read_jsonl(read, work, DOSSIER_INDEX_REF)?;
    let dossier_summary = read_json(read, work, DOSSIER_SUMMARY_REF)?;
    let source_anchor_rows = read_jsonl(read, work, SOURCE_ANCHOR_BACKLOG_REF)?;
    let term_rows = read_jsonl(read, work, TERM_INDEX_REF)?;
    let transmission_rows = read_jsonl(read, work, TRANSMISSION_BACKLOG_REF)?;
    let mut proposed_nodes = Vec::new();
    for path in PROPOSED_NODES_REFS {
        proposed_nodes.extend(read_jsonl(read, work, path)?);
    }
    let mut proposed_relations = Vec::new();
    for path in PROPOSED_RELATIONS_REFS {
        proposed_relations.extend(read_jsonl(read, work, path)?);
    }
    let mut language_packets = Vec::new();
    for path in LANGUAGE_PACKET_REFS {
        language_packets.extend(read_jsonl(read, work, path)?);
    }
    let mut branch_fragments = Vec::new();
    for path in BRANCH_FRAGMENT_REFS {
        branch_fragments.push(read_json(read, work, path)?);
    }

    let branch_audit = branch_audit(&dossier_rows, exists, enumerate_refs, work)?;
    let graph_audit = graph_workbench_audit(
        &proposed_nodes,
        &proposed_relations,
        &language_packets,
        &branch_fragments,
        work,
    )?;
    let projection_audit = projection_audit(read, work)?;

    let available_dossiers = table_audits
        .values()
        .map(|table| table["dossier_available_count"].as_u64().unwrap_or(0) as usize)
        .sum::<usize>();
    let mut diagnostics = Vec::new();
    if available_dossiers != dossier_rows.len() {
        push_diagnostic(
            &mut diagnostics,
            "error",
            DOSSIER_INDEX_REF,
            "available master-table dossier rows do not match the dossier index",
        );
    }
    for path in branch_audit["missing_branch_paths"]
        .as_array()
        .ok_or(Error::Invalid(
            "post-planting missing branch paths must be an array",
        ))?
    {
        push_diagnostic(
            &mut diagnostics,
            "error",
            path.as_str().unwrap_or(""),
            "prepared branch path is missing",
        );
    }
    for item in branch_audit["missing_branch_support"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let path = format!(
            "{}/{}",
            item["branch_path"].as_str().unwrap_or(""),
            item["missing"].as_str().unwrap_or("")
        );
        push_diagnostic(
            &mut diagnostics,
            "error",
            &path,
            "prepared branch support surface is missing",
        );
    }
    for path in branch_audit["orphan_local_graph_summaries"]
        .as_array()
        .ok_or(Error::Invalid(
            "post-planting orphan graph summaries must be an array",
        ))?
    {
        push_diagnostic(
            &mut diagnostics,
            "warning",
            path.as_str().unwrap_or(""),
            "local graph summary is outside the prepared dossier index",
        );
    }
    if projection_audit["diagnostics"] != 0 {
        push_diagnostic(
            &mut diagnostics,
            "error",
            GRAPH_PROJECTION_REF,
            "graph projection contains diagnostics",
        );
    }
    if graph_audit["language_packet_count"] != graph_audit["text_bearing_node_count"] {
        push_diagnostic(
            &mut diagnostics,
            "error",
            LANGUAGE_PACKET_REFS[0],
            "text-bearing language packets do not match text-corpus proposed node count",
        );
    }
    let error_count = diagnostics
        .iter()
        .filter(|item| item["level"] == "error")
        .count();
    let warning_count = diagnostics.len() - error_count;
    let master_rows = table_audits
        .values()
        .map(|table| table["row_count"].as_u64().unwrap_or(0) as usize)
        .sum::<usize>();

    Ok(json!({
        "schema_version": "tos_philosophy_post_planting_audit_v1",
        "surface_ref": AUDIT_JSON_REF,
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/philosophy/graph-workbench/review-packets/README.md",
        "source_refs": {
            "atlas_projection": ATLAS_PROJECTION_REF,
            "graph_views": GRAPH_VIEWS_REF,
            "graph_projection": GRAPH_PROJECTION_REF,
            "dossier_index": DOSSIER_INDEX_REF,
            "dossier_summary": DOSSIER_SUMMARY_REF,
            "proposed_nodes": PROPOSED_NODES_REFS,
            "proposed_relations": PROPOSED_RELATIONS_REFS,
            "language_packets": LANGUAGE_PACKET_REFS,
            "branch_fragments": BRANCH_FRAGMENT_REFS,
        },
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "runtime_role": "consume the generated graph projection, review packets, and audit as runtime/API/UI inputs",
            "tos_authority": "author and regenerate atlas, graph, dossier, branch, and audit surfaces",
        },
        "counts": {
            "master_tables": 3,
            "master_rows": master_rows,
            "prepared_dossiers": dossier_rows.len(),
            "source_anchor_rows": source_anchor_rows.len(),
            "term_rows": term_rows.len(),
            "transmission_rows": transmission_rows.len(),
            "diagnostics": diagnostics.len(),
            "errors": error_count,
            "warnings": warning_count,
        },
        "master_tables": table_audits,
        "dossier_shape": dossier_summary,
        "branch_audit": branch_audit,
        "graph_workbench_audit": graph_audit,
        "source_anchor_audit": {
            "source_anchor_count": source_anchor_rows.len(),
            "term_count": term_rows.len(),
            "transmission_count": transmission_rows.len(),
        },
        "graph_projection_audit": projection_audit,
        "review_readiness": {
            "status": if error_count == 0 {"ready_for_first_graph_review"} else {"blocked_by_audit_errors"},
            "next_routes": [
                "ToS/philosophy/graph-workbench/views/",
                "ToS/philosophy/graph-workbench/review-packets/",
                "ToS/derived-exports/philosophy_graph_projection.min.json",
                "abyss-stack tos-graph runtime bridge",
            ],
        },
        "diagnostics": diagnostics,
    }))
}

/// Rebuilds the read-only audit payload through owner-supplied source access.
/// `read` receives the per-file byte ceiling; the projection file has its own
/// larger cap because it is parsed with a retaining top-level visitor.
pub fn build_audit<R, E, G>(
    mut read: R,
    mut exists: E,
    mut enumerate_refs: G,
    limits: AuditLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<AuditPacket>
where
    R: FnMut(&str, usize) -> Result<Vec<u8>>,
    E: FnMut(&str) -> Result<bool>,
    G: FnMut(&str, &str, usize) -> Result<Vec<String>>,
{
    limits.validate()?;
    check_run(deadline, cancelled)?;
    let mut work = Work {
        limits,
        source_bytes: 0,
        rows: 0,
        units: 0,
        deadline,
        cancelled,
    };
    let payload = build_payload(&mut read, &mut exists, &mut enumerate_refs, &mut work)?;
    work.check()?;
    let json = render_payload(&payload, limits.max_output_bytes)?;
    let markdown = render_markdown(&payload, limits.max_output_bytes)?;
    Ok(AuditPacket {
        payload,
        json,
        markdown,
    })
}

struct SortedValue<'a>(&'a Value);

impl Serialize for SortedValue<'_> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Value::Object(object) => {
                let mut entries = object.iter().collect::<Vec<_>>();
                entries.sort_by(|(left, _), (right, _)| left.cmp(right));
                let mut output = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    output.serialize_entry(key, &SortedValue(value))?;
                }
                output.end()
            }
            Value::Array(array) => {
                let mut output = serializer.serialize_seq(Some(array.len()))?;
                for value in array {
                    output.serialize_element(&SortedValue(value))?;
                }
                output.end()
            }
            scalar => scalar.serialize(serializer),
        }
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > self.max_bytes)
        {
            return Err(io::Error::other("philosophy post-planting audit output"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn render_payload(payload: &Value, max_bytes: usize) -> Result<String> {
    let mut output = BoundedOutput {
        bytes: Vec::new(),
        max_bytes,
    };
    {
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
        let mut serializer = serde_json::Serializer::with_formatter(&mut output, formatter);
        SortedValue(payload)
            .serialize(&mut serializer)
            .map_err(|_| Error::Budget("philosophy post-planting audit output"))?;
    }
    output
        .write_all(b"\n")
        .map_err(|_| Error::Budget("philosophy post-planting audit output"))?;
    String::from_utf8(output.bytes)
        .map_err(|_| Error::Source("philosophy post-planting audit output UTF-8".into()))
}

fn field_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("philosophy post-planting audit field"))
}

fn field_usize(value: &Value, key: &str) -> Result<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(Error::Invalid("philosophy post-planting audit count"))
}

fn intake_lines(table: &Value, table_label: &str) -> Result<String> {
    let mut lines = Vec::new();
    let groups = table
        .get("unavailable_row_ids_by_intake_status")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid(
            "philosophy post-planting audit intake statuses",
        ))?;
    let mut ordered_groups = groups.iter().collect::<Vec<_>>();
    ordered_groups.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (status, row_ids) in ordered_groups {
        let row_ids = row_ids.as_array().ok_or(Error::Invalid(
            "philosophy post-planting audit intake row ids",
        ))?;
        let ids = row_ids
            .iter()
            .map(|row_id| {
                row_id
                    .as_str()
                    .map(|row_id| format!("`{row_id}`"))
                    .ok_or(Error::Invalid(
                        "philosophy post-planting audit intake row id",
                    ))
            })
            .collect::<Result<Vec<_>>>()?;
        lines.push(format!(
            "- {table_label} unavailable ({status}): {}",
            ids.join(", ")
        ));
    }
    Ok(lines.join("\n"))
}

pub fn render_markdown(payload: &Value, max_bytes: usize) -> Result<String> {
    let counts = payload
        .get("counts")
        .ok_or(Error::Invalid("philosophy post-planting audit counts"))?;
    let tables = payload
        .get("master_tables")
        .ok_or(Error::Invalid("philosophy post-planting audit tables"))?;
    let table_i = tables
        .get("table-i")
        .ok_or(Error::Invalid("philosophy post-planting audit table i"))?;
    let table_ii = tables
        .get("table-ii")
        .ok_or(Error::Invalid("philosophy post-planting audit table ii"))?;
    let table_iii = tables
        .get("table-iii")
        .ok_or(Error::Invalid("philosophy post-planting audit table iii"))?;
    let branch = payload
        .get("branch_audit")
        .ok_or(Error::Invalid("philosophy post-planting audit branch"))?;
    let graph = payload
        .get("graph_workbench_audit")
        .ok_or(Error::Invalid("philosophy post-planting audit graph"))?;
    let projection = payload
        .get("graph_projection_audit")
        .ok_or(Error::Invalid("philosophy post-planting audit projection"))?;
    let readiness = payload
        .get("review_readiness")
        .ok_or(Error::Invalid("philosophy post-planting audit readiness"))?;
    let diagnostics = payload
        .get("diagnostics")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("philosophy post-planting audit diagnostics"))?;

    let table_ii_intake = intake_lines(table_ii, "Table II")?;
    let table_iii_intake = intake_lines(table_iii, "Table III")?;
    let diagnostic_lines = diagnostics
        .iter()
        .map(|item| {
            Ok(format!(
                "- {}: `{}` - {}",
                field_string(item, "level")?,
                field_string(item, "path")?,
                field_string(item, "message")?
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    let diagnostic_lines = if diagnostic_lines.is_empty() {
        "- clear".to_owned()
    } else {
        diagnostic_lines
    };
    let routes = readiness
        .get("next_routes")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("philosophy post-planting audit next routes"))?
        .iter()
        .map(|route| {
            route
                .as_str()
                .map(|route| format!("- `{route}`"))
                .ok_or(Error::Invalid("philosophy post-planting audit next route"))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    let markdown = format!(
        "# Prepared-Dossier Post-Planting Audit\n\n\
This generated review packet checks the supported prepared-dossier plantings against the ToS philosophy topology before runtime graph review.\n\n\
## Readiness\n\n\
- Status: `{}`\n\
- Prepared dossiers: {} / {}\n\
- Table II dossiers: {} / {}\n\
{}\n\
- Table III dossiers: {} / {}\n\
{}\n\
- Prepared branches: {}\n\
- Proposed nodes: {}\n\
- Proposed relations: {}\n\
- Text-bearing language packets: {}\n\
- Graph views: {}\n\
- Review packets: {}\n\n\
## Counts\n\n\
| Surface | Count |\n\
| --- | ---: |\n\
| master rows | {} |\n\
| dossier rows | {} |\n\
| source anchors | {} |\n\
| terms | {} |\n\
| transmissions | {} |\n\
| projection nodes | {} |\n\
| projection edges | {} |\n\
| clusters | {} |\n\n\
## Diagnostics\n\n\
{}\n\n\
## Next Routes\n\n\
{}\n",
        field_string(readiness, "status")?,
        field_usize(table_i, "dossier_available_count")?,
        field_usize(table_i, "row_count")?,
        field_usize(table_ii, "dossier_available_count")?,
        field_usize(table_ii, "row_count")?,
        table_ii_intake,
        field_usize(table_iii, "dossier_available_count")?,
        field_usize(table_iii, "row_count")?,
        table_iii_intake,
        field_usize(branch, "prepared_branch_count")?,
        field_usize(graph, "proposed_node_count")?,
        field_usize(graph, "proposed_relation_count")?,
        field_usize(graph, "language_packet_count")?,
        field_usize(projection, "views")?,
        field_usize(projection, "review_packets")?,
        field_usize(counts, "master_rows")?,
        field_usize(counts, "prepared_dossiers")?,
        field_usize(counts, "source_anchor_rows")?,
        field_usize(counts, "term_rows")?,
        field_usize(counts, "transmission_rows")?,
        field_usize(projection, "nodes")?,
        field_usize(projection, "edges")?,
        field_usize(projection, "clusters")?,
        diagnostic_lines,
        routes,
    );
    if markdown.len() > max_bytes {
        return Err(Error::Budget("philosophy post-planting audit Markdown"));
    }
    Ok(markdown)
}

/// Matches the maintained validator's fixed source/currentness guardrails.
pub fn validate_assertions(current: &Value) -> Result<()> {
    if field_string(current, "schema_version")? != "tos_philosophy_post_planting_audit_v1" {
        return Err(Error::Invalid(
            "philosophy post-planting audit schema version",
        ));
    }
    if field_string(
        current
            .get("runtime_projection_boundary")
            .ok_or(Error::Invalid(
                "philosophy post-planting audit runtime boundary",
            ))?,
        "runtime_owner",
    )? != "abyss-stack"
    {
        return Err(Error::Invalid(
            "philosophy post-planting audit runtime owner",
        ));
    }
    if field_usize(
        current
            .get("counts")
            .ok_or(Error::Invalid("philosophy post-planting audit counts"))?,
        "prepared_dossiers",
    )? != 190
    {
        return Err(Error::Invalid(
            "philosophy post-planting audit dossier count",
        ));
    }
    let counts = &current["counts"];
    if field_usize(counts, "errors")? != 0 {
        return Err(Error::Invalid("philosophy post-planting audit errors"));
    }
    if field_string(
        current
            .get("review_readiness")
            .ok_or(Error::Invalid("philosophy post-planting audit readiness"))?,
        "status",
    )? != "ready_for_first_graph_review"
    {
        return Err(Error::Invalid("philosophy post-planting audit readiness"));
    }
    if field_string(
        current
            .get("graph_workbench_audit")
            .ok_or(Error::Invalid("philosophy post-planting audit graph"))?,
        "canon_status",
    )? != "pre-canon"
    {
        return Err(Error::Invalid(
            "philosophy post-planting audit canon status",
        ));
    }
    let projection = current
        .get("graph_projection_audit")
        .ok_or(Error::Invalid("philosophy post-planting audit projection"))?;
    if field_usize(projection, "views")? != 11 || field_usize(projection, "review_packets")? != 11 {
        return Err(Error::Invalid(
            "philosophy post-planting audit projection counts",
        ));
    }
    Ok(())
}

pub fn validate_audit(
    current: &Value,
    current_markdown: &str,
    expected: &AuditPacket,
    limits: AuditLimits,
) -> Result<()> {
    let rendered = render_payload(current, limits.max_output_bytes)?;
    if rendered != expected.json {
        return Err(Error::Invalid("philosophy post-planting audit JSON parity"));
    }
    if current_markdown != expected.markdown {
        return Err(Error::Invalid(
            "philosophy post-planting audit Markdown parity",
        ));
    }
    validate_assertions(current)
}
