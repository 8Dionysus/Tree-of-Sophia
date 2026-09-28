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

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

struct Row {
    // Only physically present cells are stored. Missing cells are the empty
    // string at lookup, as in the owner's (value or "") DictReader wrapper.
    cells: Vec<String>,
}

struct Table {
    headers: Vec<String>,
    // Last header position wins, including when that cell is missing.
    index: BTreeMap<String, usize>,
    rows: Vec<Row>,
}

impl Table {
    fn get<'a>(&self, row: &'a Row, key: &str) -> io::Result<&'a str> {
        let index = self
            .index
            .get(key)
            .ok_or_else(|| invalid(format!("CSV row lacks {key}")))?;
        Ok(row.cells.get(*index).map(String::as_str).unwrap_or(""))
    }

    fn sparse<'a>(&'a self, row: &'a Row, omit_status: bool) -> BTreeMap<&'a str, &'a str> {
        let mut values = BTreeMap::new();
        for (index, value) in row.cells.iter().take(self.headers.len()).enumerate() {
            let key = self.headers[index].as_str();
            if (omit_status && key == "status") || self.index.get(key) != Some(&index) {
                continue;
            }
            if !value.is_empty() {
                values.insert(key, value.as_str());
            }
        }
        values
    }

    fn surplus<'a>(&self, row: &'a Row) -> &'a [String] {
        row.cells.get(self.headers.len()..).unwrap_or(&[])
    }
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
    // csv-core skips empty physical lines. DictReader instead takes the very
    // first blank logical record as its empty header before skipping later
    // blanks, so keep that source-visible header drift.
    let leading_blank_header = matches!(body.first(), Some(b'\r' | b'\n'));
    let headers = if leading_blank_header {
        Vec::new()
    } else {
        match records.next() {
            Some(record) => utf8_cells(&record.map_err(csv_error)?)?,
            None => Vec::new(),
        }
    };
    let mut index = BTreeMap::new();
    for (position, name) in headers.iter().enumerate() {
        index.insert(name.clone(), position);
    }
    let mut rows = Vec::new();
    for record in records {
        let cells = utf8_cells(&record.map_err(csv_error)?)?;
        if cells.is_empty() {
            continue; // csv.DictReader skips blank logical records.
        }
        rows.push(Row { cells });
    }
    Ok(Table {
        headers,
        index,
        rows,
    })
}

fn csv_error(error: csv::Error) -> io::Error {
    invalid(format!("CSV parse: {error}"))
}

fn utf8_cells(record: &csv::ByteRecord) -> io::Result<Vec<String>> {
    record
        .iter()
        .map(|cell| {
            let text = std::str::from_utf8(cell).map_err(|_| invalid("CSV cell is not UTF-8"))?;
            if text.chars().count() > 131_072 {
                return Err(invalid("CSV field larger than Python csv.field_size_limit"));
            }
            Ok(text.to_owned())
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

fn canonical_ids<'a>(
    nodes: &'a Table,
    events: &'a Table,
    principles: &'a Table,
) -> io::Result<BTreeMap<&'a str, String>> {
    let mut map = BTreeMap::new();
    for row in &nodes.rows {
        if nodes.get(row, "status")? == "promoted" {
            map.insert(
                nodes.get(row, "node_id")?,
                format!(
                    "tos.support.thus-spoke-zarathustra.prologue.{}",
                    nodes.get(row, "canonical_label")?.replace('_', "-")
                ),
            );
        }
    }
    for row in &events.rows {
        if matches!(
            events.get(row, "status")?,
            "promoted" | "promoted_to_analogy"
        ) {
            map.insert(
                events.get(row, "es_id")?,
                format!(
                    "tos.{}.thus-spoke-zarathustra.prologue.{}",
                    events.get(row, "kind")?,
                    slug(events.get(row, "es_id")?, 1)?
                ),
            );
        }
    }
    for row in &principles.rows {
        let status = principles.get(row, "status")?;
        if matches!(status, "promoted" | "promoted_to_synthesis") {
            let class = if status == "promoted_to_synthesis" {
                "synthesis"
            } else {
                "principle"
            };
            map.insert(
                principles.get(row, "principle_id")?,
                format!(
                    "tos.{class}.thus-spoke-zarathustra.prologue.{}",
                    slug(principles.get(row, "principle_id")?, 0)?
                ),
            );
        }
    }
    Ok(map)
}

fn canonical_classes<'a, 'b>(
    nodes: &'a Table,
    events: &'a Table,
    principles: &'a Table,
    canonical: &'b BTreeMap<&'a str, String>,
) -> io::Result<BTreeMap<&'b str, &'a str>> {
    let mut raw = BTreeMap::new();
    let mut order = Vec::new();
    for row in &nodes.rows {
        let id = nodes.get(row, "node_id")?;
        if !raw.contains_key(&id) {
            order.push(id);
        }
        raw.insert(id, nodes.get(row, "node_class")?);
    }
    for row in &events.rows {
        let id = events.get(row, "es_id")?;
        if !raw.contains_key(&id) {
            order.push(id);
        }
        raw.insert(id, events.get(row, "kind")?);
    }
    for row in &principles.rows {
        let id = principles.get(row, "principle_id")?;
        if !raw.contains_key(&id) {
            order.push(id);
        }
        raw.insert(
            id,
            if principles.get(row, "status")? == "promoted_to_synthesis" {
                "synthesis"
            } else {
                "principle"
            },
        );
    }
    let mut classes = BTreeMap::new();
    for id in order {
        if let (Some(name), Some(class)) = (canonical.get(id), raw.get(id)) {
            classes.insert(name.as_str(), *class);
        }
    }
    Ok(classes)
}

fn projection_matches(
    pack: &Table,
    edges: &Table,
    canonical: &BTreeMap<&str, String>,
) -> io::Result<bool> {
    let pack_keys: BTreeSet<_> = pack.index.keys().map(String::as_str).collect();
    let expected_keys: BTreeSet<_> = edges
        .index
        .keys()
        .map(String::as_str)
        .filter(|key| *key != "status")
        .collect();
    let key_sets_match = pack_keys == expected_keys;
    let mut actual = pack.rows.iter();
    for row in &edges.rows {
        if edges.get(row, "status")? != "promoted" {
            continue;
        }
        let (Some(from), Some(to)) = (
            canonical.get(edges.get(row, "from_id")?),
            canonical.get(edges.get(row, "to_id")?),
        ) else {
            continue;
        };
        let Some(current) = actual.next() else {
            return Ok(false);
        };
        if !key_sets_match {
            return Ok(false);
        }
        let mut projected = edges.sparse(row, true);
        projected.insert("from_id", from.as_str());
        projected.insert("to_id", to.as_str());
        if projected != pack.sparse(current, false) || edges.surplus(row) != pack.surplus(current) {
            return Ok(false);
        }
    }
    Ok(actual.next().is_none())
}

fn split_pipe(value: &str) -> BTreeSet<&str> {
    value
        .split('|')
        .map(|part| {
            part.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{001c}'..='\u{001f}'))
        })
        .filter(|part| !part.is_empty())
        .collect()
}

fn issue<F: FnMut(&str, &str) -> io::Result<()>>(
    emit: &mut F,
    found: &mut bool,
    location: &str,
    message: &str,
) -> io::Result<()> {
    *found = true;
    emit(location, message)
}

fn required_columns(table: &Table, keys: &[&str]) -> io::Result<()> {
    if !table.rows.is_empty() {
        for key in keys {
            if !table.index.contains_key(*key) {
                return Err(invalid(format!("CSV row lacks {key}")));
            }
        }
    }
    Ok(())
}

/// Read-only verification of the source-owned prologue projection. Issues are
/// emitted in maintained header, projection, registry, then relation-row order
/// without retaining a graph-sized diagnostic packet.
pub fn validate<F>(root: &Path, mut emit: F) -> io::Result<bool>
where
    F: FnMut(&str, &str) -> io::Result<()>,
{
    if !root.is_absolute() || std::fs::canonicalize(root)? != root {
        return Err(invalid(
            "repository root must be an absolute path without symlinks",
        ));
    }
    if !root.join(PACK).is_file() {
        emit(PACK, "missing canonical relation pack")?;
        return Ok(false);
    }
    let pack = table(root, PACK)?;
    let nodes = table(root, &intake_path("nodes"))?;
    let events = table(root, &intake_path("event_state_nodes"))?;
    let principles = table(root, &intake_path("principles"))?;
    let edges = table(root, &intake_path("edges"))?;
    let predicates = table(root, PREDICATES)?;
    let class_table = table(root, CLASSES)?;
    required_columns(&pack, &["edge_id", "from_id", "to_id", "predicate_id"])?;
    required_columns(
        &nodes,
        &["status", "node_id", "canonical_label", "node_class"],
    )?;
    required_columns(&events, &["status", "es_id", "kind"])?;
    required_columns(&principles, &["status", "principle_id"])?;
    required_columns(&edges, &["status", "from_id", "to_id"])?;
    required_columns(
        &predicates,
        &["predicate_id", "allowed_from_classes", "allowed_to_classes"],
    )?;
    required_columns(&class_table, &["class_id"])?;
    let canonical = canonical_ids(&nodes, &events, &principles)?;
    let classes = canonical_classes(&nodes, &events, &principles, &canonical)?;
    let projection_ok = projection_matches(&pack, &edges, &canonical)?;
    let mut predicates_by_id = BTreeMap::new();
    for row in &predicates.rows {
        predicates_by_id.insert(
            predicates.get(row, "predicate_id")?,
            (
                split_pipe(predicates.get(row, "allowed_from_classes")?),
                split_pipe(predicates.get(row, "allowed_to_classes")?),
            ),
        );
    }
    let mut class_ids = BTreeSet::new();
    for row in &class_table.rows {
        class_ids.insert(class_table.get(row, "class_id")?);
    }
    let mut found = false;
    if !headers_match(&pack.headers, PACK_HEADERS) {
        issue(
            &mut emit,
            &mut found,
            PACK,
            "header drift from the relation-pack contract",
        )?;
    }
    if !projection_ok {
        issue(
            &mut emit,
            &mut found,
            PACK,
            "canonical relation pack drifted from the promoted intake projection",
        )?;
    }
    if !headers_match(&predicates.headers, PREDICATE_HEADERS) {
        issue(
            &mut emit,
            &mut found,
            PREDICATES,
            "predicate registry header drift",
        )?;
    }
    if !headers_match(&class_table.headers, CLASS_HEADERS) {
        issue(
            &mut emit,
            &mut found,
            CLASSES,
            "class registry header drift",
        )?;
    }
    for row in &pack.rows {
        let edge_id = pack.get(row, "edge_id")?;
        let from = pack.get(row, "from_id")?;
        let to = pack.get(row, "to_id")?;
        let predicate = pack.get(row, "predicate_id")?;
        if !from.starts_with("tos.") || !to.starts_with("tos.") {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} must use only canonical tos.* ids"),
            )?;
        }
        let Some(from_class) = classes.get(from) else {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} points from unknown canonical id {from}"),
            )?;
            continue;
        };
        let Some(to_class) = classes.get(to) else {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} points to unknown canonical id {to}"),
            )?;
            continue;
        };
        if !class_ids.contains(from_class) {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} uses unknown from-class {from_class}"),
            )?;
        }
        if !class_ids.contains(to_class) {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} uses unknown to-class {to_class}"),
            )?;
        }
        let Some((allowed_from, allowed_to)) = predicates_by_id.get(predicate) else {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} uses unregistered predicate {predicate}"),
            )?;
            continue;
        };
        if !allowed_from.contains(from_class) {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} violates allowed_from_classes for predicate {predicate}"),
            )?;
        }
        if !allowed_to.contains(to_class) {
            issue(
                &mut emit,
                &mut found,
                PACK,
                &format!("{edge_id} violates allowed_to_classes for predicate {predicate}"),
            )?;
        }
    }
    Ok(!found)
}
