//! The current route-local relation-pack check. Canon and candidate CSV files
//! remain the owners; a matching projection is mechanical evidence only.

use csv::ReaderBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use tos_foundation::JsonLimits;

const INTAKE: &str = "ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/mode-b";
const PACK: &str =
    "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv";
const PREDICATES: &str = "ToS/canon/registries/predicates.csv";
const CLASSES: &str = "ToS/canon/registries/classes.csv";
const PACK_HEADERS: &[&str] = &[
    "edge_id",
    "edge_kind",
    "from_id",
    "predicate_id",
    "to_id",
    "layer",
    "anchor_mode",
    "anchor_start_secondary",
    "anchor_end_secondary",
    "anchor_segment_ids",
    "witness_scope",
    "connectivity_role",
    "confidence",
    "note",
];
const PREDICATE_HEADERS: &[&str] = &[
    "predicate_id",
    "predicate_ru",
    "inverse_predicate_id",
    "allowed_from_classes",
    "allowed_to_classes",
    "status",
    "note",
    "count_in_master",
    "row_kinds",
];
const CLASS_HEADERS: &[&str] = &[
    "class_id",
    "family",
    "parent_class",
    "status",
    "note",
    "count_as_from",
    "count_as_to",
    "example_id",
    "source_side",
];

pub type Issue = (String, String);

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[derive(Clone, Eq, PartialEq)]
struct Row {
    fields: BTreeMap<String, String>,
    // csv.DictReader keeps surplus cells under its None key. They participate
    // in projection equality even though valid owner tables have none.
    surplus: Vec<String>,
}

impl Row {
    fn get(&self, key: &str) -> io::Result<&str> {
        self.fields
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| invalid(format!("CSV row lacks {key}")))
    }
}

struct Table {
    headers: Vec<String>,
    rows: Vec<Row>,
}

fn table(root: &Path, relative: &str) -> io::Result<Table> {
    let file = File::open(root.join(relative))?;
    let limit = JsonLimits::default().max_bytes;
    if file.metadata()?.len() > limit as u64 {
        return Err(invalid(format!(
            "CSV input exceeds native byte bound: {relative}"
        )));
    }
    let mut raw = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() > limit {
        return Err(invalid(format!(
            "CSV input exceeds native byte bound: {relative}"
        )));
    }
    // Python read_csv opens utf-8-sig with newline="". csv handles quoted
    // physical CR/LF, commas, and doubled quotes as one logical record.
    let body = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&raw);
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(body);
    let mut records = reader.byte_records();
    let headers = match records.next() {
        Some(record) => utf8_cells(&record.map_err(csv_error)?)?,
        None => Vec::new(),
    };
    let mut rows = Vec::new();
    for record in records {
        let cells = utf8_cells(&record.map_err(csv_error)?)?;
        if cells.is_empty() {
            continue; // csv.DictReader skips blank logical records.
        }
        let cell_count = cells.len();
        let mut fields = BTreeMap::new();
        let mut surplus = Vec::new();
        for (index, value) in cells.into_iter().enumerate() {
            if let Some(header) = headers.get(index) {
                // DictReader's duplicate-header map keeps the last value.
                fields.insert(header.clone(), value);
            } else {
                surplus.push(value);
            }
        }
        for header in headers.iter().skip(cell_count) {
            fields.insert(header.clone(), String::new());
        }
        rows.push(Row { fields, surplus });
    }
    Ok(Table { headers, rows })
}

fn csv_error(error: csv::Error) -> io::Error {
    invalid(format!("CSV parse: {error}"))
}

fn utf8_cells(record: &csv::ByteRecord) -> io::Result<Vec<String>> {
    record
        .iter()
        .map(|cell| {
            std::str::from_utf8(cell)
                .map(str::to_owned)
                .map_err(|_| invalid("CSV cell is not UTF-8"))
        })
        .collect()
}

fn headers_match(actual: &[String], expected: &[&str]) -> bool {
    actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

fn intake_path(name: &str) -> String {
    format!("{INTAKE}/{name}.csv")
}

fn slug(raw: &str, skip_dots: usize) -> io::Result<String> {
    raw.splitn(skip_dots + 2, '.')
        .nth(skip_dots + 1)
        .map(|tail| tail.replace('_', "-"))
        .ok_or_else(|| invalid("promoted intake id lacks required parts"))
}

fn canonical_ids(
    nodes: &[Row],
    events: &[Row],
    principles: &[Row],
) -> io::Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for row in nodes {
        if row.get("status")? == "promoted" {
            map.insert(
                row.get("node_id")?.into(),
                format!(
                    "tos.support.thus-spoke-zarathustra.prologue.{}",
                    row.get("canonical_label")?.replace('_', "-")
                ),
            );
        }
    }
    for row in events {
        if matches!(row.get("status")?, "promoted" | "promoted_to_analogy") {
            map.insert(
                row.get("es_id")?.into(),
                format!(
                    "tos.{}.thus-spoke-zarathustra.prologue.{}",
                    row.get("kind")?,
                    slug(row.get("es_id")?, 1)?
                ),
            );
        }
    }
    for row in principles {
        let status = row.get("status")?;
        if matches!(status, "promoted" | "promoted_to_synthesis") {
            let class = if status == "promoted_to_synthesis" {
                "synthesis"
            } else {
                "principle"
            };
            map.insert(
                row.get("principle_id")?.into(),
                format!(
                    "tos.{class}.thus-spoke-zarathustra.prologue.{}",
                    slug(row.get("principle_id")?, 0)?
                ),
            );
        }
    }
    Ok(map)
}

fn canonical_classes(
    nodes: &[Row],
    events: &[Row],
    principles: &[Row],
    canonical: &BTreeMap<String, String>,
) -> io::Result<BTreeMap<String, String>> {
    let mut raw = BTreeMap::new();
    let mut order = Vec::new();
    for row in nodes {
        let id = row.get("node_id")?.to_owned();
        if !raw.contains_key(&id) {
            order.push(id.clone());
        }
        raw.insert(id, row.get("node_class")?.to_owned());
    }
    for row in events {
        let id = row.get("es_id")?.to_owned();
        if !raw.contains_key(&id) {
            order.push(id.clone());
        }
        raw.insert(id, row.get("kind")?.to_owned());
    }
    for row in principles {
        let id = row.get("principle_id")?.to_owned();
        if !raw.contains_key(&id) {
            order.push(id.clone());
        }
        raw.insert(
            id,
            if row.get("status")? == "promoted_to_synthesis" {
                "synthesis"
            } else {
                "principle"
            }
            .into(),
        );
    }
    let mut classes = BTreeMap::new();
    for id in order {
        if let (Some(name), Some(class)) = (canonical.get(&id), raw.get(&id)) {
            classes.insert(name.clone(), class.clone());
        }
    }
    Ok(classes)
}

fn promoted_edges(edges: &[Row], canonical: &BTreeMap<String, String>) -> io::Result<Vec<Row>> {
    let mut projected = Vec::new();
    for row in edges {
        if row.get("status")? != "promoted" {
            continue;
        }
        let (Some(from), Some(to)) = (
            canonical.get(row.get("from_id")?),
            canonical.get(row.get("to_id")?),
        ) else {
            continue;
        };
        let mut selected = row.clone();
        selected.fields.remove("status");
        selected.fields.insert("from_id".into(), from.clone());
        selected.fields.insert("to_id".into(), to.clone());
        projected.push(selected);
    }
    Ok(projected)
}

fn split_pipe(value: &str) -> BTreeSet<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Read-only verification of the source-owned prologue projection. Issues stay
/// in maintained header, projection, registry, then relation-row order.
pub fn validate(root: &Path) -> io::Result<Vec<Issue>> {
    if !root.is_absolute() || std::fs::canonicalize(root)? != root {
        return Err(invalid(
            "repository root must be an absolute path without symlinks",
        ));
    }
    if !root.join(PACK).is_file() {
        return Ok(vec![(
            PACK.into(),
            "missing canonical relation pack".into(),
        )]);
    }
    let mut issues = Vec::new();
    let pack = table(root, PACK)?;
    if !headers_match(&pack.headers, PACK_HEADERS) {
        issues.push((
            PACK.into(),
            "header drift from the relation-pack contract".into(),
        ));
    }
    let nodes = table(root, &intake_path("nodes"))?;
    let events = table(root, &intake_path("event_state_nodes"))?;
    let principles = table(root, &intake_path("principles"))?;
    let edges = table(root, &intake_path("edges"))?;
    let canonical = canonical_ids(&nodes.rows, &events.rows, &principles.rows)?;
    let classes = canonical_classes(&nodes.rows, &events.rows, &principles.rows, &canonical)?;
    if pack.rows != promoted_edges(&edges.rows, &canonical)? {
        issues.push((
            PACK.into(),
            "canonical relation pack drifted from the promoted intake projection".into(),
        ));
    }
    let predicates = table(root, PREDICATES)?;
    if !headers_match(&predicates.headers, PREDICATE_HEADERS) {
        issues.push((PREDICATES.into(), "predicate registry header drift".into()));
    }
    let mut predicates_by_id = BTreeMap::new();
    for row in &predicates.rows {
        predicates_by_id.insert(
            row.get("predicate_id")?.to_owned(),
            (
                split_pipe(row.get("allowed_from_classes")?),
                split_pipe(row.get("allowed_to_classes")?),
            ),
        );
    }
    let class_table = table(root, CLASSES)?;
    if !headers_match(&class_table.headers, CLASS_HEADERS) {
        issues.push((CLASSES.into(), "class registry header drift".into()));
    }
    let mut class_ids = BTreeSet::new();
    for row in &class_table.rows {
        class_ids.insert(row.get("class_id")?.to_owned());
    }
    for row in &pack.rows {
        let edge_id = row.get("edge_id")?;
        let from = row.get("from_id")?;
        let to = row.get("to_id")?;
        let predicate = row.get("predicate_id")?;
        if !from.starts_with("tos.") || !to.starts_with("tos.") {
            issues.push((
                PACK.into(),
                format!("{edge_id} must use only canonical tos.* ids"),
            ));
        }
        let Some(from_class) = classes.get(from) else {
            issues.push((
                PACK.into(),
                format!("{edge_id} points from unknown canonical id {from}"),
            ));
            continue;
        };
        let Some(to_class) = classes.get(to) else {
            issues.push((
                PACK.into(),
                format!("{edge_id} points to unknown canonical id {to}"),
            ));
            continue;
        };
        if !class_ids.contains(from_class) {
            issues.push((
                PACK.into(),
                format!("{edge_id} uses unknown from-class {from_class}"),
            ));
        }
        if !class_ids.contains(to_class) {
            issues.push((
                PACK.into(),
                format!("{edge_id} uses unknown to-class {to_class}"),
            ));
        }
        let Some((allowed_from, allowed_to)) = predicates_by_id.get(predicate) else {
            issues.push((
                PACK.into(),
                format!("{edge_id} uses unregistered predicate {predicate}"),
            ));
            continue;
        };
        if !allowed_from.contains(from_class) {
            issues.push((
                PACK.into(),
                format!("{edge_id} violates allowed_from_classes for predicate {predicate}"),
            ));
        }
        if !allowed_to.contains(to_class) {
            issues.push((
                PACK.into(),
                format!("{edge_id} violates allowed_to_classes for predicate {predicate}"),
            ));
        }
    }
    Ok(issues)
}
