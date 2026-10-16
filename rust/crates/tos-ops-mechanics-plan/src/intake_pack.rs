//! The maintained nine-table intake contract and derived registry counts.
//! Structural success does not promote candidates to canon.
use crate::route_cards::RouteSources;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};
type Row = BTreeMap<String, String>;
pub type Issue = (String, String);
const ROOT: &str = "ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/mode-b";
const SOURCE: &str =
    "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json";
const PREDICATES: &str = "ToS/canon/registries/predicates.csv";
const CLASSES: &str = "ToS/canon/registries/classes.csv";
const COMMENTARY: &str = "pr.departure_from_reflective_origin";
const ANALOGY: &str = "ev.p5.bee_honey_analogy";
const LITERALS: &[&str] = &["literal.ten_years", "literal.too_much"];
const TABLE_HEADERS: &[(&str, &[&str])] = &[
    (
        "corpus_map.csv",
        &[
            "corpus_row_id",
            "work_id",
            "part_no",
            "chapter_no",
            "subchapter_no",
            "paragraph_no",
            "source_secondary",
            "sort_key",
            "title_ru",
            "title_en",
            "note",
        ],
    ),
    (
        "witnesses.csv",
        &[
            "witness_id",
            "language",
            "witness_role",
            "authority_level",
            "author_or_translator",
            "edition_or_source",
            "publication_year",
            "based_on",
            "normalization_note",
            "active",
        ],
    ),
    (
        "segments.csv",
        &[
            "segment_id",
            "source_secondary",
            "paragraph_anchor",
            "sort_key",
            "witness_scope",
            "line_span",
            "cluster_id",
            "working_name",
            "note",
        ],
    ),
    (
        "nodes.csv",
        &[
            "node_id",
            "label_ru",
            "node_class",
            "layer",
            "first_segment_id",
            "first_source_secondary",
            "canonical_label",
            "label_en",
            "status",
            "note",
        ],
    ),
    (
        "event_state_nodes.csv",
        &[
            "es_id",
            "kind",
            "label_ru",
            "anchor_mode",
            "anchor_start_secondary",
            "anchor_end_secondary",
            "anchor_segment_ids",
            "subject_hint",
            "es_class",
            "repeatable",
            "status",
            "note",
        ],
    ),
    (
        "edges.csv",
        &[
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
            "status",
        ],
    ),
    (
        "translation_tensions.csv",
        &[
            "tension_id",
            "normalized_core",
            "anchor_mode",
            "anchor_start_secondary",
            "anchor_end_secondary",
            "anchor_segment_ids",
            "witness_ids",
            "why_load_bearing",
            "decision_status",
            "preferred_handling",
            "note",
        ],
    ),
    (
        "witness_glosses.csv",
        &[
            "gloss_id",
            "witness_id",
            "segment_id",
            "source_secondary",
            "token_or_phrase",
            "normalized_core",
            "tension_id",
            "gloss_note",
            "status",
        ],
    ),
    (
        "principles.csv",
        &[
            "principle_id",
            "layer",
            "formula_ru",
            "anchor_mode",
            "anchor_start_secondary",
            "anchor_end_secondary",
            "anchor_segment_ids",
            "status",
            "note",
        ],
    ),
];
const PROMOTED: &[&str] = &[
    "pr.beginning_through_going_under",
    "pr.blessing_is_reciprocal",
    "pr.descent_is_required_by_gift",
    "pr.excess_seeks_recipients",
    "pr.gift_has_dual_mode",
    "pr.go_under_is_human_name",
    "pr.happiness_is_relational",
    "pr.overflow_can_be_received",
    "pr.reflected_light_can_be_carried",
    "pr.return_as_action",
    "pr.solitude_as_ripening",
    "pr.tranquil_vision_without_envy",
    "pr.wisdom_can_overfill",
];

struct Issues {
    rows: Vec<Issue>,
    bytes: usize,
}
impl Issues {
    fn push(&mut self, path: &str, message: impl Into<String>) -> io::Result<()> {
        let message = message.into();
        self.bytes = self
            .bytes
            .checked_add(path.len() + message.len())
            .ok_or_else(|| invalid("intake issue accounting"))?;
        if self.rows.len() >= 4096 || self.bytes > 1_048_576 {
            return Err(invalid("intake issue bound"));
        }
        self.rows.push((path.into(), message));
        Ok(())
    }
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
fn tick(s: &RouteSources, cancel: &AtomicI32) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != 0 {
        return Err(invalid("intake validation cancelled"));
    }
    s.check()
}
fn value<'a>(row: &'a Row, key: &str) -> &'a str {
    row.get(key).map(String::as_str).unwrap_or("")
}
fn pipe(text: &str) -> impl Iterator<Item = &str> {
    text.split('|').map(str::trim).filter(|s| !s.is_empty())
}
fn boolean(text: &str) -> bool {
    matches!(text.trim().to_lowercase().as_str(), "true" | "false")
}
fn ids(rows: &[Row], key: &str) -> BTreeSet<String> {
    rows.iter().map(|r| value(r, key).into()).collect()
}
fn csv(
    s: &mut RouteSources,
    path: &str,
    expected: &[&str],
    used: &mut usize,
    issues: &mut Issues,
    cancel: &AtomicI32,
) -> io::Result<Vec<Row>> {
    tick(s, cancel)?;
    if !s.is_file(path)? {
        issues.push(path, "missing required CSV")?;
        return Ok(Vec::new());
    }
    let raw = s.bounded_bytes(path, 2 * 1_048_576, used, 16 * 1_048_576)?;
    let raw = raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&raw);
    let mut reader = csv::ReaderBuilder::new().from_reader(raw);
    let header = reader.headers().map_err(io::Error::other)?.clone();
    if !header.iter().eq(expected.iter().copied()) {
        issues.push(path, "header drift from the current tabular base contract")?;
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        tick(s, cancel)?;
        if rows.len() >= 16384 {
            return Err(invalid("intake CSV row bound"));
        }
        let record = record.map_err(io::Error::other)?;
        rows.push(
            header
                .iter()
                .zip(record.iter())
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        );
    }
    Ok(rows)
}
fn anchor(
    table: &str,
    row: &Row,
    id_key: &str,
    secondaries: &BTreeSet<String>,
    segments: &BTreeSet<String>,
    issues: &mut Issues,
) -> io::Result<()> {
    let id = value(row, id_key);
    let mode = value(row, "anchor_mode");
    let start = value(row, "anchor_start_secondary");
    let end = value(row, "anchor_end_secondary");
    if !matches!(mode, "single" | "multi") {
        issues.push(table, format!("{id} has invalid anchor_mode {mode}"))?;
    }
    if !secondaries.contains(start) {
        issues.push(
            table,
            format!("{id} has invalid anchor_start_secondary {start}"),
        )?;
    }
    if !end.is_empty() && !secondaries.contains(end) {
        issues.push(
            table,
            format!("{id} has invalid anchor_end_secondary {end}"),
        )?;
    }
    if mode == "single" && !end.is_empty() {
        issues.push(
            table,
            format!("{id} should keep anchor_end_secondary empty for single anchors"),
        )?;
    }
    if mode == "multi" && end.is_empty() {
        issues.push(
            table,
            format!("{id} needs anchor_end_secondary for multi anchors"),
        )?;
    }
    let mut count = 0;
    for segment in pipe(value(row, "anchor_segment_ids")) {
        count += 1;
        if !segments.contains(segment) {
            issues.push(
                table,
                format!("{id} points at unknown anchor segment {segment}"),
            )?;
        }
    }
    if count == 0 {
        issues.push(
            table,
            format!("{id} must keep at least one anchor_segment_id"),
        )?;
    }
    Ok(())
}
fn canonical_ids(nodes: &[Row], events: &[Row], principles: &[Row]) -> BTreeSet<String> {
    // Edge classification depends only on the promoted key set, not generated slugs.
    nodes
        .iter()
        .filter(|r| value(r, "status") == "promoted")
        .map(|r| value(r, "node_id").into())
        .chain(
            events
                .iter()
                .filter(|r| matches!(value(r, "status"), "promoted" | "promoted_to_analogy"))
                .map(|r| value(r, "es_id").into()),
        )
        .chain(
            principles
                .iter()
                .filter(|r| matches!(value(r, "status"), "promoted" | "promoted_to_synthesis"))
                .map(|r| value(r, "principle_id").into()),
        )
        .collect()
}
fn edge_status<'a>(from: &str, to: &str, canonical: &BTreeSet<String>) -> &'a str {
    if canonical.contains(from) && canonical.contains(to) {
        "promoted"
    } else if [from, to].contains(&COMMENTARY) {
        "deferred_commentary"
    } else if [from, to].contains(&ANALOGY) {
        "deferred_analogy"
    } else if [from, to].iter().any(|v| LITERALS.contains(v)) {
        "deferred_literal"
    } else {
        "invalid_residue"
    }
}
pub fn validate(s: &mut RouteSources, cancel: &AtomicI32) -> io::Result<Vec<Issue>> {
    let mut issues = Issues {
        rows: Vec::new(),
        bytes: 0,
    };
    let mut used = 0;
    let mut expected: BTreeSet<String> = TABLE_HEADERS
        .iter()
        .map(|(n, _)| format!("{ROOT}/{n}"))
        .collect();
    expected.insert(format!("{ROOT}/README.md"));
    let actual: BTreeSet<String> = s
        .paths(ROOT)?
        .into_iter()
        .filter(|p| {
            p.strip_prefix(ROOT)
                .and_then(|v| v.strip_prefix('/'))
                .is_some_and(|v| !v.contains('/'))
        })
        .filter_map(|p| match s.is_file(&p) {
            Ok(true) => Some(Ok(p)),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<io::Result<_>>()?;
    if actual != expected {
        issues.push(
            ROOT,
            "intake pack file set does not match the 9-table contract",
        )?;
    }
    let mut tables = BTreeMap::new();
    for (name, headers) in TABLE_HEADERS {
        tables.insert(
            *name,
            csv(
                s,
                &format!("{ROOT}/{name}"),
                headers,
                &mut used,
                &mut issues,
                cancel,
            )?,
        );
    }
    let raw = s.bounded_bytes(SOURCE, 2 * 1_048_576, &mut used, 16 * 1_048_576)?;
    parse_json(&raw, JsonMode::RequestLastWins, JsonLimits::default())
        .map_err(|e| invalid(format!("intake source JSON: {e:?}")))?;
    let source: Value = serde_json::from_slice(&raw).map_err(io::Error::other)?;
    if !source.is_object() {
        issues.push(
            "tree source node",
            "canonical source node must remain a JSON object",
        )?;
    }
    let corpus = &tables["corpus_map.csv"];
    let witnesses = &tables["witnesses.csv"];
    let segments = &tables["segments.csv"];
    let nodes = &tables["nodes.csv"];
    let events = &tables["event_state_nodes.csv"];
    let edges = &tables["edges.csv"];
    let tensions = &tables["translation_tensions.csv"];
    let glosses = &tables["witness_glosses.csv"];
    let principles = &tables["principles.csv"];
    if corpus
        .iter()
        .enumerate()
        .any(|(i, r)| value(r, "sort_key") != (i + 1).to_string())
    {
        issues.push(
            "corpus_map.csv",
            "sort_key must run strictly from 1 through the current corpus row count",
        )?;
    }
    let secondaries = ids(corpus, "source_secondary");
    if secondaries.len() != corpus.len() {
        issues.push("corpus_map.csv", "source_secondary values must be unique")?;
    }
    let witness_ids = ids(witnesses, "witness_id");
    if witnesses.is_empty() {
        issues.push(
            "witnesses.csv",
            "witness table must declare at least one witness",
        )?;
    }
    if witness_ids.len() != witnesses.len() {
        issues.push("witnesses.csv", "witness ids must be unique")?;
    }
    if witnesses
        .iter()
        .filter(|r| value(r, "witness_role") == "canonical_source")
        .count()
        != 1
    {
        issues.push(
            "witnesses.csv",
            "witness table must declare exactly one canonical_source",
        )?;
    }
    if !witnesses.iter().any(|r| {
        matches!(
            value(r, "witness_role"),
            "working_translation" | "bridge_translation"
        )
    }) {
        issues.push(
            "witnesses.csv",
            "witness table must declare at least one translation witness",
        )?;
    }
    for r in witnesses {
        tick(s, cancel)?;
        let role = value(r, "witness_role");
        let based = value(r, "based_on");
        if !matches!(
            role,
            "canonical_source" | "working_translation" | "bridge_translation"
        ) {
            issues.push("witnesses.csv", format!("unknown witness_role {role}"))?;
        }
        if !based.is_empty() && !witness_ids.contains(based) {
            issues.push(
                "witnesses.csv",
                format!(
                    "{} based_on points at unknown witness {based}",
                    value(r, "witness_id")
                ),
            )?;
        }
        if !boolean(value(r, "active")) {
            issues.push(
                "witnesses.csv",
                format!("invalid active value {}", value(r, "active")),
            )?;
        }
    }
    if segments.len() != corpus.len()
        || segments
            .iter()
            .enumerate()
            .any(|(i, r)| value(r, "segment_id") != format!("seg.1.1.1.{}", i + 1))
    {
        issues.push(
            "segments.csv",
            "segment spine must match the current corpus sort_key sequence",
        )?;
    }
    if segments.len() != corpus.len()
        || segments
            .iter()
            .enumerate()
            .any(|(i, r)| value(r, "paragraph_anchor") != format!("[{}]", i + 1))
    {
        issues.push(
            "segments.csv",
            "paragraph_anchor must stay aligned to corpus sort_key values",
        )?;
    }
    for (r, c) in segments.iter().zip(corpus) {
        if value(r, "source_secondary") != value(c, "source_secondary") {
            issues.push(
                "segments.csv",
                format!(
                    "segment {} is out of source_secondary order",
                    value(r, "segment_id")
                ),
            )?;
        }
    }
    let segment_ids = ids(segments, "segment_id");
    let node_ids = ids(nodes, "node_id");
    let mut graph = node_ids.clone();
    graph.extend(ids(events, "es_id"));
    graph.extend(ids(principles, "principle_id"));
    let canonical = canonical_ids(nodes, events, principles);
    if LITERALS.iter().any(|v| !node_ids.contains(*v)) {
        issues.push(
            "nodes.csv",
            "literal.ten_years and literal.too_much must remain explicit node rows",
        )?;
    }
    for r in nodes {
        tick(s, cancel)?;
        if !segment_ids.contains(value(r, "first_segment_id")) {
            issues.push(
                "nodes.csv",
                format!("unknown first_segment_id {}", value(r, "first_segment_id")),
            )?;
        }
        if !secondaries.contains(value(r, "first_source_secondary")) {
            issues.push(
                "nodes.csv",
                format!(
                    "unknown first_source_secondary {}",
                    value(r, "first_source_secondary")
                ),
            )?;
        }
        let expected = if LITERALS.contains(&value(r, "node_id")) {
            "deferred_literal"
        } else {
            "promoted"
        };
        if value(r, "status") != expected {
            issues.push(
                "nodes.csv",
                format!("{} must be marked {expected}", value(r, "node_id")),
            )?;
        }
    }
    for r in events {
        tick(s, cancel)?;
        let id = value(r, "es_id");
        anchor(
            "event_state_nodes.csv",
            r,
            "es_id",
            &secondaries,
            &segment_ids,
            &mut issues,
        )?;
        if !boolean(value(r, "repeatable")) {
            issues.push(
                "event_state_nodes.csv",
                format!(
                    "{id} has invalid repeatable value {}",
                    value(r, "repeatable")
                ),
            )?;
        }
        let status = match value(r, "kind") {
            "event" | "state" => Some("promoted"),
            "analogy" => {
                if id != ANALOGY {
                    issues.push(
                        "event_state_nodes.csv",
                        format!("unexpected analogy row {id}"),
                    )?;
                }
                Some("promoted_to_analogy")
            }
            kind => {
                issues.push("event_state_nodes.csv", format!("unexpected kind {kind}"))?;
                None
            }
        };
        if let Some(expected) = status {
            if value(r, "status") != expected {
                issues.push(
                    "event_state_nodes.csv",
                    format!("{id} must be marked {expected}"),
                )?;
            }
        }
    }
    for r in principles {
        tick(s, cancel)?;
        let id = value(r, "principle_id");
        anchor(
            "principles.csv",
            r,
            "principle_id",
            &secondaries,
            &segment_ids,
            &mut issues,
        )?;
        let expected = if PROMOTED.contains(&id) {
            Some("promoted")
        } else if id == COMMENTARY {
            Some("promoted_to_synthesis")
        } else {
            issues.push("principles.csv", format!("unexpected principle id {id}"))?;
            None
        };
        if let Some(expected) = expected {
            if value(r, "status") != expected {
                issues.push("principles.csv", format!("{id} must be marked {expected}"))?;
            }
        }
    }
    if ids(principles, "principle_id")
        != PROMOTED
            .iter()
            .copied()
            .chain([COMMENTARY])
            .map(str::to_owned)
            .collect()
    {
        issues.push(
            "principles.csv",
            "principle id set drifted from the current bounded route",
        )?;
    }
    for r in edges {
        tick(s, cancel)?;
        let id = value(r, "edge_id");
        for (key, direction) in [("from_id", "from"), ("to_id", "to")] {
            if !graph.contains(value(r, key)) {
                issues.push(
                    "edges.csv",
                    format!("{id} points {direction} unknown id {}", value(r, key)),
                )?;
            }
        }
        anchor(
            "edges.csv",
            r,
            "edge_id",
            &secondaries,
            &segment_ids,
            &mut issues,
        )?;
        let expected = edge_status(value(r, "from_id"), value(r, "to_id"), &canonical);
        if value(r, "status") != expected {
            issues.push("edges.csv", format!("{id} must be marked {expected}"))?;
        }
    }
    if tensions.len()
        != source["translation_tensions"]
            .as_array()
            .map_or(0, Vec::len)
    {
        issues.push(
            "translation_tensions.csv",
            "row count must stay aligned with the compact source-node translation_tensions surface",
        )?;
    }
    let tension_ids = ids(tensions, "tension_id");
    for r in tensions {
        tick(s, cancel)?;
        anchor(
            "translation_tensions.csv",
            r,
            "tension_id",
            &secondaries,
            &segment_ids,
            &mut issues,
        )?;
        for witness in pipe(value(r, "witness_ids")) {
            if !witness_ids.contains(witness) {
                issues.push(
                    "translation_tensions.csv",
                    format!(
                        "{} points at unknown witness {witness}",
                        value(r, "tension_id")
                    ),
                )?;
            }
        }
    }
    let mut pairs: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for r in glosses {
        let tension = value(r, "tension_id");
        if !tension.is_empty() {
            *pairs.entry((tension, value(r, "witness_id"))).or_default() += 1;
        }
    }
    let mut missing = Vec::new();
    let mut missing_count = 0usize;
    for tension in &tension_ids {
        for witness in &witness_ids {
            tick(s, cancel)?;
            if !pairs.contains_key(&(tension.as_str(), witness.as_str())) {
                missing_count += 1;
                if missing.len() < 8 {
                    missing.push(format!("{tension}/{witness}"));
                }
            }
        }
    }
    if missing_count > 0 {
        let suffix = if missing_count > 8 {
            format!(", +{} more", missing_count - 8)
        } else {
            String::new()
        };
        issues.push(
            "witness_glosses.csv",
            format!("missing gloss coverage for {}{suffix}", missing.join(", ")),
        )?;
    }
    for ((t, w), n) in &pairs {
        if *n > 1 {
            issues.push(
                "witness_glosses.csv",
                format!("duplicate gloss coverage for {t}/{w}"),
            )?;
        }
    }
    for r in glosses {
        tick(s, cancel)?;
        let id = value(r, "gloss_id");
        for (key, label, set, optional) in [
            ("witness_id", "witness", &witness_ids, false),
            ("segment_id", "segment", &segment_ids, false),
            ("source_secondary", "source_secondary", &secondaries, false),
            ("tension_id", "tension_id", &tension_ids, true),
        ] {
            let v = value(r, key);
            if !(set.contains(v) || optional && v.is_empty()) {
                issues.push(
                    "witness_glosses.csv",
                    format!("{id} points at unknown {label} {v}"),
                )?;
            }
        }
    }
    // Compare finite intervals instead of expanding the authored numeric range.
    let mut intervals = Vec::new();
    for r in edges {
        tick(s, cancel)?;
        let start = value(r, "anchor_start_secondary");
        let end = value(r, "anchor_end_secondary");
        let parse = |v: &str| {
            v.rsplit(',')
                .next()
                .unwrap_or("")
                .trim()
                .parse::<num_bigint::BigInt>()
                .map_err(|_| invalid("intake anchor paragraph must be an integer"))
        };
        intervals.push((
            parse(start)?,
            parse(if end.is_empty() { start } else { end })?,
        ));
    }
    let mut uncovered = Vec::new();
    for value in &secondaries {
        tick(s, cancel)?;
        let Some(number) = value
            .strip_prefix("1,1,1,")
            .and_then(|v| v.parse::<num_bigint::BigInt>().ok())
        else {
            uncovered.push(value.as_str());
            continue;
        };
        if !intervals.iter().any(|(a, b)| *a <= number && number <= *b) {
            uncovered.push(value.as_str());
        }
    }
    if !uncovered.is_empty() {
        issues.push(
            "edges.csv",
            format!(
                "edge coverage is missing paragraphs: {}",
                uncovered.join(", ")
            ),
        )?;
    }
    let predicates = csv(
        s,
        PREDICATES,
        &[
            "predicate_id",
            "predicate_ru",
            "inverse_predicate_id",
            "allowed_from_classes",
            "allowed_to_classes",
            "status",
            "note",
            "count_in_master",
            "row_kinds",
        ],
        &mut used,
        &mut issues,
        cancel,
    )?;
    let classes = csv(
        s,
        CLASSES,
        &[
            "class_id",
            "family",
            "parent_class",
            "status",
            "note",
            "count_as_from",
            "count_as_to",
            "example_id",
            "source_side",
        ],
        &mut used,
        &mut issues,
        cancel,
    )?;
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for r in edges {
        *counts.entry(value(r, "predicate_id")).or_default() += 1;
        kinds
            .entry(value(r, "predicate_id"))
            .or_default()
            .insert(value(r, "edge_kind"));
    }
    for r in &predicates {
        tick(s, cancel)?;
        let id = value(r, "predicate_id");
        if value(r, "count_in_master") != counts.get(id).unwrap_or(&0).to_string() {
            issues.push(PREDICATES, format!("count drift for predicate {id}"))?;
        }
        let expected = kinds
            .get(id)
            .map(|set| set.iter().copied().collect::<Vec<_>>().join("|"))
            .unwrap_or_default();
        if value(r, "row_kinds") != expected {
            issues.push(PREDICATES, format!("row_kinds drift for predicate {id}"))?;
        }
    }
    let mut entity_classes: BTreeMap<&str, &str> = nodes
        .iter()
        .map(|r| (value(r, "node_id"), value(r, "node_class")))
        .collect();
    entity_classes.extend(events.iter().map(|r| (value(r, "es_id"), value(r, "kind"))));
    entity_classes.extend(principles.iter().map(|r| {
        (
            value(r, "principle_id"),
            if value(r, "status") == "promoted_to_synthesis" {
                "synthesis"
            } else {
                "principle"
            },
        )
    }));
    let mut from: BTreeMap<&str, usize> = BTreeMap::new();
    let mut to: BTreeMap<&str, usize> = BTreeMap::new();
    for r in edges {
        if let Some(class) = entity_classes.get(value(r, "from_id")) {
            *from.entry(class).or_default() += 1;
        }
        if let Some(class) = entity_classes.get(value(r, "to_id")) {
            *to.entry(class).or_default() += 1;
        }
    }
    for r in &classes {
        tick(s, cancel)?;
        let id = value(r, "class_id");
        let f = *from.get(id).unwrap_or(&0);
        let t = *to.get(id).unwrap_or(&0);
        if value(r, "count_as_from") != f.to_string() {
            issues.push(CLASSES, format!("count_as_from drift for class {id}"))?;
        }
        if value(r, "count_as_to") != t.to_string() {
            issues.push(CLASSES, format!("count_as_to drift for class {id}"))?;
        }
        let expected = match (f > 0, t > 0) {
            (true, true) => "from+to",
            (true, false) => "from_only",
            (false, true) => "to_only",
            _ => "unused",
        };
        if value(r, "source_side") != expected {
            issues.push(CLASSES, format!("source_side drift for class {id}"))?;
        }
    }
    Ok(issues.rows)
}
