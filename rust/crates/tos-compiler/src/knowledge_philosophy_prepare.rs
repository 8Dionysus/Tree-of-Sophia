//! Private prepared phase for one descriptor-selected philosophy graph cut.
//!
//! The full `philosophy-node-edge-v1` adapter is not standalone: Python
//! finalizes inherited views and readable context after all relation sources.
//! These indexes preserve owner carriers without publishing graph rows.

use crate::knowledge_stage::{ExactInputReceipt, KnowledgeStage, SeekRow, WritePhase};
use crate::{Error, QueryVocabulary, Result, knowledge_normalization::SourceRow};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use tos_foundation::{Digest256, Digest256Hasher};

const PROFILE: &str = "philosophy-node-edge-v1";
const COLLECTIONS: [&str; 2] = ["nodes", "edges"];

#[derive(Clone, Copy, Debug)]
pub struct PhilosophyPrepareLimits {
    pub max_nodes: u64,
    pub max_edges: u64,
    pub max_edge_view_bindings: u64,
    pub max_page_rows: usize,
    pub max_row_bytes: usize,
    pub max_work_bytes: u64,
}
impl PhilosophyPrepareLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_edges == 0
            || self.max_edge_view_bindings == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("philosophy preparation limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhilosophyExternalDependency {
    EntityRegistryKindsLabelsAndAncestry,
    RelationRegistryPredicatesAndLabels,
    GlobalNormalizedEndpointTitles,
    AllRelationSourcesInheritedViews,
    GlobalEndpointPlaceholderClosure,
    ReadableContextAndRevisionFinalization,
}
const EXTERNAL_DEPENDENCIES: &[PhilosophyExternalDependency] = &[
    PhilosophyExternalDependency::EntityRegistryKindsLabelsAndAncestry,
    PhilosophyExternalDependency::RelationRegistryPredicatesAndLabels,
    PhilosophyExternalDependency::GlobalNormalizedEndpointTitles,
    PhilosophyExternalDependency::AllRelationSourcesInheritedViews,
    PhilosophyExternalDependency::GlobalEndpointPlaceholderClosure,
    PhilosophyExternalDependency::ReadableContextAndRevisionFinalization,
];

#[derive(Clone, Debug)]
pub struct PhilosophyPrepareReceipt {
    pub source_graph: String,
    pub input_role: String,
    pub source_cut: String,
    pub nodes: u64,
    pub edges: u64,
    pub edge_view_bindings: u64,
    pub unresolved_endpoint_refs: u64,
    pub node_input_root_sha256: String,
    pub edge_input_root_sha256: String,
    pub dependency_root_sha256: String,
    pub external_dependencies: &'static [PhilosophyExternalDependency],
    pub final_graph_rows_written: bool,
}

struct Selected {
    source: String,
    role: String,
    cut: String,
    expected: [(u64, String); 2],
}
fn select(
    vocabulary: &QueryVocabulary,
    receipt: &ExactInputReceipt,
    limits: PhilosophyPrepareLimits,
) -> Result<Selected> {
    let sources: Vec<_> = vocabulary
        .sources
        .iter()
        .filter(|source| source.adapter_profile == PROFILE)
        .collect();
    if sources.len() != 1 {
        return Err(Error::Invalid("philosophy adapter profile selection"));
    }
    let source = sources[0];
    let mut expected = std::array::from_fn(|_| (0, String::new()));
    for (i, collection) in COLLECTIONS.iter().enumerate() {
        let entries: Vec<_> = receipt
            .collections
            .iter()
            .filter(|entry| {
                entry.source_graph == source.source_graph_id && entry.collection == *collection
            })
            .collect();
        if entries.len() != 1
            || entries[0].input_role != source.input_role
            || entries[0].adapter_profile != PROFILE
        {
            return Err(Error::Invalid("philosophy input collection registration"));
        }
        expected[i] = (
            entries[0].expected_count,
            entries[0].expected_root_sha256.clone(),
        );
    }
    if receipt.collections.iter().any(|entry| {
        entry.source_graph == source.source_graph_id
            && !COLLECTIONS.contains(&entry.collection.as_str())
    }) {
        return Err(Error::Invalid("unknown philosophy input collection"));
    }
    if expected[0].0 > limits.max_nodes || expected[1].0 > limits.max_edges {
        return Err(Error::Budget("philosophy registered row count"));
    }
    Ok(Selected {
        source: source.source_graph_id.clone(),
        role: source.input_role.clone(),
        cut: receipt.binding.source_cut.clone(),
        expected,
    })
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty() && text.len() <= 4096)
        .ok_or(Error::Invalid("philosophy required carrier field"))
}
fn typed_id<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let id = required(value, field)?;
    if id.trim() != id {
        return Err(Error::Invalid("philosophy ambiguous whitespace ID"));
    }
    Ok(id)
}
fn required_array<'a>(value: &'a Value, field: &str) -> Result<&'a [Value]> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("philosophy required carrier list"))?;
    if values
        .iter()
        .any(|value| value.as_str().is_none_or(str::is_empty))
    {
        return Err(Error::Invalid("philosophy carrier list member"));
    }
    Ok(values)
}
fn owner_shape(value: &Value, node: bool) -> Result<()> {
    if !value.get("properties").is_some_and(Value::is_object) {
        return Err(Error::Invalid("philosophy owner properties"));
    }
    required(value, "source_ref")?;
    required_array(value, "graph_layers")?;
    required_array(value, "view_ids")?;
    if node {
        required(value, "label")?;
        let multilingual = value
            .get("multilingual")
            .filter(|value| value.is_object())
            .ok_or(Error::Invalid("philosophy multilingual carrier"))?;
        if required(multilingual, "schema_version")? != "tos_multilingual_label_v1"
            || !multilingual.get("label").is_some_and(Value::is_object)
        {
            return Err(Error::Invalid("philosophy multilingual label"));
        }
        let labels = multilingual.get("label").expect("checked label object");
        required(labels, "ru")?;
        required(labels, "en")?;
        required(multilingual, "source_ref")?;
    }
    Ok(())
}
fn digest(raw: &str) -> Result<[u8; 32]> {
    Ok(*Digest256::from_hex(raw)
        .map_err(|_| Error::Invalid("philosophy row digest"))?
        .as_bytes())
}
fn root_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}
fn root_text(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
fn charge(work: &mut u64, bytes: usize, limits: PhilosophyPrepareLimits) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .ok_or(Error::Budget("philosophy work bytes"))?;
    if *work > limits.max_work_bytes {
        return Err(Error::Budget("philosophy work bytes"));
    }
    Ok(())
}

struct Walk {
    count: u64,
    hash: Digest256Hasher,
    after: Option<String>,
}
impl Walk {
    fn new() -> Self {
        Self {
            count: 0,
            hash: Digest256Hasher::new(),
            after: None,
        }
    }
    fn add(&mut self, raw: &SeekRow, cap: u64) -> Result<()> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or(Error::Budget("philosophy rows"))?;
        if self.count > cap {
            return Err(Error::Budget("philosophy rows"));
        }
        root_item(&mut self.hash, &raw.id, &digest(&raw.payload_sha256)?);
        Ok(())
    }
    fn complete(self, expected: &(u64, String)) -> Result<u64> {
        if self.count != expected.0 || self.hash.finalize().to_hex() != expected.1 {
            return Err(Error::Invalid("philosophy input count/root mismatch"));
        }
        Ok(self.count)
    }
}

fn create_tables(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Schema, |db| {
        db.execute_batch(
            r#"
CREATE TABLE knowledge_philosophy_nodes(
 node_id TEXT PRIMARY KEY, node_type TEXT NOT NULL, semantic_kind TEXT NOT NULL,
 source_ref TEXT NOT NULL, raw_sha256 BLOB NOT NULL CHECK(length(raw_sha256)=32));
CREATE TABLE knowledge_philosophy_edges(
 edge_id TEXT PRIMARY KEY, from_id TEXT NOT NULL, to_id TEXT NOT NULL,
 from_source_graph TEXT NOT NULL, to_source_graph TEXT NOT NULL,
 predicate_id TEXT NOT NULL, source_ref TEXT NOT NULL,
 raw_sha256 BLOB NOT NULL CHECK(length(raw_sha256)=32));
CREATE INDEX knowledge_philosophy_edges_from ON knowledge_philosophy_edges(from_id,edge_id);
CREATE INDEX knowledge_philosophy_edges_to ON knowledge_philosophy_edges(to_id,edge_id);
CREATE TABLE knowledge_philosophy_edge_views(
 edge_id TEXT NOT NULL, view_id TEXT NOT NULL,
 PRIMARY KEY(edge_id,view_id));
CREATE INDEX knowledge_philosophy_views_view ON knowledge_philosophy_edge_views(view_id,edge_id);
CREATE TABLE knowledge_philosophy_unresolved_endpoints(
 edge_id TEXT NOT NULL, endpoint_role TEXT NOT NULL,
 source_graph TEXT NOT NULL, native_id TEXT NOT NULL,
 PRIMARY KEY(edge_id,endpoint_role));
CREATE INDEX knowledge_philosophy_unresolved_source
 ON knowledge_philosophy_unresolved_endpoints(source_graph,native_id,edge_id);
"#,
        )?;
        Ok(())
    })
}

struct NodeIndex {
    id: String,
    kind: String,
    semantic_kind: String,
    source_ref: String,
    sha: [u8; 32],
}
fn walk_nodes(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: PhilosophyPrepareLimits,
    work: &mut u64,
    projection: bool,
) -> Result<u64> {
    let mut walk = Walk::new();
    loop {
        let page = stage.scan_input(
            &selected.source,
            "nodes",
            walk.after.as_deref(),
            limits.max_page_rows,
        )?;
        let mut prepared = Vec::with_capacity(page.rows.len());
        for raw in &page.rows {
            charge(work, raw.payload.len(), limits)?;
            walk.add(raw, limits.max_nodes)?;
            let row = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
            let value = row.value();
            if !projection {
                owner_shape(value, true)?;
            }
            let id = typed_id(value, "node_id")?;
            if id != raw.id {
                return Err(Error::Invalid("philosophy node identity"));
            }
            let kind = if projection {
                value
                    .get("node_type")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        value
                            .get("node_kind")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                    })
                    .or_else(|| {
                        value
                            .get("resource_kind")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                    })
                    .unwrap_or("knowledge-object")
            } else {
                required(value, "node_type")?.trim()
            };
            let semantic_kind = value
                .get("properties")
                .and_then(|p| p.get("original_node_type"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|kind| !kind.is_empty())
                .unwrap_or(kind);
            prepared.push(NodeIndex {
                id: id.into(),
                kind: kind.into(),
                semantic_kind: semantic_kind.into(),
                source_ref: if projection {
                    value
                        .get("source_ref")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into()
                } else {
                    required(value, "source_ref")?.into()
                },
                sha: digest(&raw.payload_sha256)?,
            });
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for row in prepared {
                tx.execute(
                    "INSERT INTO knowledge_philosophy_nodes VALUES (?1,?2,?3,?4,?5)",
                    params![
                        row.id,
                        row.kind,
                        row.semantic_kind,
                        row.source_ref,
                        &row.sha[..]
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })?;
        match page.next_id {
            Some(next) => walk.after = Some(next),
            None => break,
        }
    }
    walk.complete(&selected.expected[0])
}

struct EdgeIndex {
    id: String,
    from: String,
    to: String,
    predicate: String,
    source_ref: String,
    from_source: String,
    to_source: String,
    sha: [u8; 32],
    views: Vec<String>,
    unresolved: Vec<(&'static str, String, String)>,
}
fn walk_edges(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: PhilosophyPrepareLimits,
    work: &mut u64,
    projection: bool,
) -> Result<(u64, u64, u64)> {
    let mut walk = Walk::new();
    let mut view_tokens = 0u64;
    loop {
        let page = stage.scan_input(
            &selected.source,
            "edges",
            walk.after.as_deref(),
            limits.max_page_rows,
        )?;
        let mut prepared = Vec::with_capacity(page.rows.len());
        for raw in &page.rows {
            charge(work, raw.payload.len(), limits)?;
            walk.add(raw, limits.max_edges)?;
            let row = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
            let value = row.value();
            if !projection {
                owner_shape(value, false)?;
            }
            let id = typed_id(value, "edge_id")?;
            if id != raw.id {
                return Err(Error::Invalid("philosophy edge identity"));
            }
            let from = typed_id(value, "from_id")?;
            let to = typed_id(value, "to_id")?;
            let from_source = value
                .get("from_source_graph")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(&selected.source);
            let to_source = value
                .get("to_source_graph")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(&selected.source);
            let mut unresolved = Vec::new();
            for (role, source, endpoint) in [("from", from_source, from), ("to", to_source, to)] {
                if source != selected.source
                    || stage
                        .raw_by_id(&selected.source, "nodes", endpoint)?
                        .is_none()
                {
                    unresolved.push((role, source.to_owned(), endpoint.to_owned()));
                }
            }
            let views = if projection {
                value
                    .get("view_ids")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            } else {
                required_array(value, "view_ids")?
                    .iter()
                    .map(|view| view.as_str().expect("checked view ID").to_owned())
                    .collect::<Vec<_>>()
            };
            view_tokens = view_tokens
                .checked_add(views.len() as u64)
                .ok_or(Error::Budget("philosophy view bindings"))?;
            if view_tokens > limits.max_edge_view_bindings {
                return Err(Error::Budget("philosophy view bindings"));
            }
            prepared.push(EdgeIndex {
                id: id.into(),
                from: from.into(),
                to: to.into(),
                predicate: required(value, "predicate_id")?.trim().into(),
                source_ref: if projection {
                    value
                        .get("source_ref")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into()
                } else {
                    required(value, "source_ref")?.into()
                },
                from_source: from_source.into(),
                to_source: to_source.into(),
                sha: digest(&raw.payload_sha256)?,
                views,
                unresolved,
            });
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for row in prepared {
                tx.execute("INSERT INTO knowledge_philosophy_edges VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",params![
                    row.id,row.from,row.to,row.from_source,row.to_source,row.predicate,row.source_ref,&row.sha[..]
                ])?;
                for view in row.views {
                    tx.execute("INSERT OR IGNORE INTO knowledge_philosophy_edge_views VALUES (?1,?2)",params![row.id,view])?;
                }
                for (role,source,native) in row.unresolved {
                    tx.execute("INSERT INTO knowledge_philosophy_unresolved_endpoints VALUES (?1,?2,?3,?4)",
                        params![row.id,role,source,native])?;
                }
            }
            tx.commit()?; Ok(())
        })?;
        match page.next_id {
            Some(next) => walk.after = Some(next),
            None => break,
        }
    }
    let (bindings, unresolved) = stage.with_connection(WritePhase::Sort, |db| {
        let bindings: i64 = db.query_row(
            "SELECT count(*) FROM knowledge_philosophy_edge_views",
            [],
            |r| r.get(0),
        )?;
        let unresolved: i64 = db.query_row(
            "SELECT count(*) FROM knowledge_philosophy_unresolved_endpoints",
            [],
            |r| r.get(0),
        )?;
        Ok((bindings as u64, unresolved as u64))
    })?;
    Ok((walk.complete(&selected.expected[1])?, bindings, unresolved))
}

pub(crate) fn dependency_root(stage: &mut KnowledgeStage<'_>) -> Result<String> {
    stage.with_connection(WritePhase::Sort, |db| {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-philosophy-prepared-v1\0");
        for (name,sql,fields) in [
            ("nodes","SELECT node_id,raw_sha256,node_type,semantic_kind,source_ref FROM knowledge_philosophy_nodes ORDER BY node_id",5),
            ("edges","SELECT edge_id,raw_sha256,from_id,to_id,from_source_graph,to_source_graph,predicate_id,source_ref FROM knowledge_philosophy_edges ORDER BY edge_id",8),
        ] {
            root_text(&mut hash,name);
            let mut statement = db.prepare(sql)?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?; let sha: Vec<u8> = row.get(1)?;
                if sha.len() != 32 { return Err(Error::Invalid("philosophy prepared row digest")); }
                root_item(&mut hash,&id,&sha);
                for index in 2..fields { let value: String = row.get(index)?; root_text(&mut hash,&value); }
            }
        }
        root_text(&mut hash,"edge_views");
        let mut statement = db.prepare("SELECT edge_id,view_id FROM knowledge_philosophy_edge_views ORDER BY edge_id,view_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let edge: String = row.get(0)?; let view: String = row.get(1)?;
            root_text(&mut hash,&edge); root_text(&mut hash,&view);
        }
        root_text(&mut hash,"unresolved_endpoints");
        let mut statement = db.prepare("SELECT edge_id,endpoint_role,source_graph,native_id FROM knowledge_philosophy_unresolved_endpoints ORDER BY edge_id,endpoint_role")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            for index in 0..4 { let value: String = row.get(index)?; root_text(&mut hash,&value); }
        }
        Ok(hash.finalize().to_hex())
    })
}

fn prepare_inner(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: PhilosophyPrepareLimits,
    projection: bool,
) -> Result<PhilosophyPrepareReceipt> {
    limits.validate()?;
    let selected = select(vocabulary, stage.exact_receipt(), limits)?;
    create_tables(stage)?;
    let mut work = 0u64;
    let nodes = walk_nodes(stage, &selected, limits, &mut work, projection)?;
    let (edges, edge_view_bindings, unresolved_endpoint_refs) =
        walk_edges(stage, &selected, limits, &mut work, projection)?;
    let dependency_root_sha256 = dependency_root(stage)?;
    Ok(PhilosophyPrepareReceipt {
        source_graph: selected.source,
        input_role: selected.role,
        source_cut: selected.cut,
        nodes,
        edges,
        edge_view_bindings,
        unresolved_endpoint_refs,
        node_input_root_sha256: selected.expected[0].1.clone(),
        edge_input_root_sha256: selected.expected[1].1.clone(),
        dependency_root_sha256,
        external_dependencies: EXTERNAL_DEPENDENCIES,
        final_graph_rows_written: false,
    })
}

/// Prepare a sealed philosophy source carrier. On any error the private stage
/// is poisoned; the caller cannot mistake preparation for selected output.
pub fn prepare_philosophy(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: PhilosophyPrepareLimits,
) -> Result<PhilosophyPrepareReceipt> {
    let result = prepare_inner(stage, vocabulary, limits, false);
    if result.is_err() {
        stage.poison();
    }
    result
}

// Maintained prepare reads a derived projection with optional display carriers.
// Raw packets/roots remain exact and the normalizer owns optional-field meaning.
// This profile is never the default sealed philosophy or D1 admission route.
pub(crate) fn prepare_philosophy_projection(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: PhilosophyPrepareLimits,
) -> Result<PhilosophyPrepareReceipt> {
    if !stage.public_build() {
        return Err(Error::Invalid("prepare projection computational stage"));
    }
    let result = prepare_inner(stage, vocabulary, limits, true);
    if result.is_err() {
        stage.poison();
    }
    result
}

fn clear_inner(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    prepared: &PhilosophyPrepareReceipt,
    limits: PhilosophyPrepareLimits,
    final_nodes: u64,
    final_node_root: &str,
    final_relations: u64,
    final_relation_root: &str,
) -> Result<()> {
    limits.validate()?;
    let selected = select(vocabulary, stage.exact_receipt(), limits)?;
    if prepared.source_graph != selected.source
        || prepared.input_role != selected.role
        || prepared.source_cut != selected.cut
        || prepared.nodes != selected.expected[0].0
        || prepared.edges != selected.expected[1].0
        || prepared.node_input_root_sha256 != selected.expected[0].1
        || prepared.edge_input_root_sha256 != selected.expected[1].1
        || prepared.external_dependencies != EXTERNAL_DEPENDENCIES
        || prepared.final_graph_rows_written
        || dependency_root(stage)? != prepared.dependency_root_sha256
    {
        return Err(Error::Invalid("philosophy cleanup prepared binding"));
    }
    let core = stage.core_roots()?;
    if core.nodes != final_nodes
        || core.node_sha256 != final_node_root
        || core.relations != final_relations
        || core.relation_sha256 != final_relation_root
    {
        return Err(Error::Invalid("philosophy cleanup final graph drift"));
    }
    stage.with_connection(WritePhase::Finalize,|db| {
        let tx = db.transaction()?;
        let source = &selected.source;
        let count = |sql: &str| -> Result<u64> {
            let n: i64 = tx.query_row(sql,params![source],|r|r.get(0))?;
            u64::try_from(n).map_err(|_|Error::Budget("philosophy cleanup count"))
        };
        let absent = |sql: &str| -> Result<bool> {
            let found: Option<i64> = tx.query_row(sql,params![source],|r|r.get(0)).optional()?;
            Ok(found.is_none())
        };
        if count("SELECT count(*) FROM knowledge_philosophy_nodes WHERE ?1 IS NOT NULL")? != prepared.nodes
            || count("SELECT count(*) FROM knowledge_philosophy_edges WHERE ?1 IS NOT NULL")? != prepared.edges
            || count("SELECT count(*) FROM knowledge_philosophy_edge_views WHERE ?1 IS NOT NULL")? != prepared.edge_view_bindings
            || count("SELECT count(*) FROM knowledge_philosophy_unresolved_endpoints WHERE ?1 IS NOT NULL")? != prepared.unresolved_endpoint_refs {
            return Err(Error::Invalid("philosophy cleanup prepared count drift"));
        }
        if !absent("SELECT 1 FROM knowledge_philosophy_nodes p WHERE NOT EXISTS (
            SELECT 1 FROM knowledge_nodes n WHERE n.id=?1||':'||p.node_id
            AND n.source_graph=?1 AND n.native_id=p.node_id AND n.kind_id=p.semantic_kind) LIMIT 1")?
            || !absent("SELECT 1 FROM knowledge_philosophy_edges p WHERE NOT EXISTS (
            SELECT 1 FROM knowledge_relations r WHERE r.id=?1||':'||p.edge_id
            AND r.source_graph=?1 AND r.native_id=p.edge_id AND r.predicate_id=p.predicate_id
            AND r.from_id=p.from_source_graph||':'||p.from_id
            AND r.to_id=p.to_source_graph||':'||p.to_id) LIMIT 1")? {
            return Err(Error::Invalid("philosophy cleanup raw graph coverage"));
        }
        if count("SELECT count(*) FROM knowledge_relations WHERE source_graph=?1")? != prepared.edges {
            return Err(Error::Invalid("philosophy cleanup final relation coverage"));
        }
        // Foreign-source relations may require placeholders in this source.
        // Their all-source absence/context authority remains with the assembler.
        if !absent("SELECT 1 FROM knowledge_nodes n WHERE n.source_graph=?1 AND NOT EXISTS (
            SELECT 1 FROM knowledge_philosophy_nodes p WHERE p.node_id=n.native_id)
            AND (n.kind_id!='relation-endpoint' OR n.native_id IS NULL
                OR n.id!=?1||':'||n.native_id) LIMIT 1")? {
            return Err(Error::Invalid("philosophy cleanup additional node shape"));
        }
        let placeholders = count("SELECT count(*) FROM knowledge_nodes n WHERE n.source_graph=?1
            AND NOT EXISTS (SELECT 1 FROM knowledge_philosophy_nodes p WHERE p.node_id=n.native_id)")?;
        if count("SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1")? != prepared.nodes.checked_add(placeholders)
            .ok_or(Error::Budget("philosophy cleanup final node count"))? {
            return Err(Error::Invalid("philosophy cleanup final node coverage"));
        }
        if !absent("SELECT 1 FROM knowledge_philosophy_edges p WHERE ?1 IS NOT NULL AND (
            NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=p.from_source_graph||':'||p.from_id)
            OR NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=p.to_source_graph||':'||p.to_id)) LIMIT 1")? {
            return Err(Error::Invalid("philosophy cleanup global endpoint closure"));
        }
        tx.execute_batch("DROP TABLE knowledge_philosophy_edge_views;
            DROP TABLE knowledge_philosophy_unresolved_endpoints;
            DROP TABLE knowledge_philosophy_edges;
            DROP TABLE knowledge_philosophy_nodes;")?;
        tx.commit()?;
        Ok(())
    })
}

/// Remove only this source's private preparation indexes after the assembler
/// supplies complete final graph counts/roots. Actual staged roots, raw native
/// coverage and global endpoint existence are checked before any table drops.
/// All-source completeness, placeholder absence authority and final semantics
/// remain the caller's responsibility; these mechanical roots cannot grant it.
#[allow(clippy::too_many_arguments)]
pub fn clear_philosophy_prepare(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    prepared: &PhilosophyPrepareReceipt,
    limits: PhilosophyPrepareLimits,
    final_nodes: u64,
    final_node_root: &str,
    final_relations: u64,
    final_relation_root: &str,
) -> Result<()> {
    let result = clear_inner(
        stage,
        vocabulary,
        prepared,
        limits,
        final_nodes,
        final_node_root,
        final_relations,
        final_relation_root,
    );
    if result.is_err() {
        stage.poison();
    }
    result
}

// Exact existing native phi preparation carriers, shared by its tests and cold fixture.
#[cfg(any(test, feature = "test-fixture"))]
pub(crate) const PHILOSOPHY_FIXTURE_0: &str = r#"{"node_id":"n:a","label":"Alpha","multilingual":{"schema_version":"tos_multilingual_label_v1","label":{"ru":"Альфа","en":"Alpha"},"source_ref":"ToS/philosophy/a.md"},"node_type":"concept","graph_layers":["philosophy"],"view_ids":["atlas"],"source_ref":"ToS/philosophy/a.md","properties":{"original_node_type":" concept "}}"#;
#[cfg(any(test, feature = "test-fixture"))]
pub(crate) const PHILOSOPHY_FIXTURE_1: &str = r#"{"node_id":"n:b","label":"Beta","multilingual":{"schema_version":"tos_multilingual_label_v1","label":{"ru":"Бета","en":"Beta"},"source_ref":"ToS/philosophy/b.md"},"node_type":"concept","graph_layers":["philosophy"],"view_ids":[],"source_ref":"ToS/philosophy/b.md","properties":{}}"#;
#[cfg(any(test, feature = "test-fixture"))]
pub(crate) const PHILOSOPHY_FIXTURE_2: &str = r#"{"edge_id":"e:ab","from_id":"n:a","to_id":"n:b","predicate_id":"relates","graph_layers":["philosophy"],"view_ids":["atlas","atlas","route"],"source_ref":"ToS/philosophy/relations.md","properties":{}}"#;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_stage::{
        InputCollectionReceipt, InputRow, StageIsolation, StageLimits, StageOwner,
    };
    use crate::{Limits, RegisteredSource, SourceBinding};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const NODE_ROOT: &str = "424892b09d2efeccccedf284318b748507abcaa02fb1b6bd0ac4ae9a377965c2";
    const EDGE_ROOT: &str = "fb076b86c32ddc5f4e65a886957787112a7f8f716bdb54bbf979edff64c4fac1";
    const PREPARED_ROOT: &str = "4ab103d63bbc76b903f30b76737ef2b67b624f2a18d34deb4e5b911261b0b4a3";
    const EMPTY_PREPARED_ROOT: &str =
        "fe21a78998e46e90280c0894080cc34d8d23775f6e41b7749889ff65e4ebe9ce";
    type FixtureRow = (&'static str, &'static str, String);
    struct Owner;
    impl StageOwner for Owner {
        fn verify_receipt(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
        fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> Result<()> {
            Ok(())
        }
    }
    struct TestQuota;
    impl StageIsolation for TestQuota {
        fn verify(&self, _: &Path, _: StageLimits, _: WritePhase) -> Result<()> {
            Ok(())
        }
    }
    fn limits() -> PhilosophyPrepareLimits {
        PhilosophyPrepareLimits {
            max_nodes: 16,
            max_edges: 16,
            max_edge_view_bindings: 16,
            max_page_rows: 2,
            max_row_bytes: 2048,
            max_work_bytes: 64 * 1024,
        }
    }
    fn vocabulary() -> QueryVocabulary {
        QueryVocabulary {
            descriptor_sha256: "0".repeat(64),
            descriptor_version: 1,
            sources: vec![RegisteredSource {
                source_graph_id: "philosophy".into(),
                owner_ref: "ToS/philosophy/AGENTS.md".into(),
                input_role: "philosophical-atlas".into(),
                adapter_profile: PROFILE.into(),
                representative_priority: 0,
            }],
            registered_source_ids: vec!["philosophy".into()],
            extension_adapter_profile: "indexed-node-edge-v1".into(),
            entity_registry_id: "entities".into(),
            relation_registry_id: "relations".into(),
            semantic_primitive_profile: "fixture".into(),
            shared_entity_id_grammars: vec![],
            overview_route_ids: vec![],
            identity_policy: serde_json::json!({}),
            overview_policy: serde_json::json!({}),
        }
    }
    fn fixture() -> Vec<FixtureRow> {
        vec![
            ("nodes", "n:a", super::PHILOSOPHY_FIXTURE_0.into()),
            ("nodes", "n:b", super::PHILOSOPHY_FIXTURE_1.into()),
            ("edges", "e:ab", super::PHILOSOPHY_FIXTURE_2.into()),
        ]
    }
    fn input_root(rows: &[FixtureRow], collection: &str) -> (u64, String) {
        let mut selected: Vec<_> = rows.iter().filter(|row| row.0 == collection).collect();
        selected.sort_by_key(|row| row.1);
        let mut hash = Digest256Hasher::new();
        for (_, id, payload) in &selected {
            root_item(
                &mut hash,
                id,
                Digest256::of_bytes(payload.as_bytes()).as_bytes(),
            );
        }
        (selected.len() as u64, hash.finalize().to_hex())
    }
    fn receipt(rows: &[FixtureRow]) -> ExactInputReceipt {
        ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "sealed-philosophy-cut".into(),
                through_commit_seq: 1,
                membership_root: "0".repeat(64),
                index_generation: "gen-1".into(),
                route_map_version: "routes-1".into(),
                reader_abi: "reader-1".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: COLLECTIONS
                .iter()
                .map(|collection| {
                    let (count, root) = input_root(rows, collection);
                    InputCollectionReceipt {
                        source_graph: "philosophy".into(),
                        collection: (*collection).into(),
                        input_role: "philosophical-atlas".into(),
                        adapter_profile: PROFILE.into(),
                        expected_count: count,
                        expected_root_sha256: root,
                    }
                })
                .collect(),
        }
    }
    fn path() -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-philosophy-prepare-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir.join("private.sqlite3")
    }
    fn run(
        rows: &[FixtureRow],
        receipt_rows: &[FixtureRow],
        limits: PhilosophyPrepareLimits,
    ) -> Result<PhilosophyPrepareReceipt> {
        let owner = Owner;
        let quota = TestQuota;
        let candidate = path();
        let mut stage = KnowledgeStage::create(
            &candidate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 4096,
            },
            receipt(receipt_rows),
            &owner,
            &quota,
        )?;
        let prepared = (|| {
            for (collection, id, payload) in rows {
                stage.ingest_input(InputRow {
                    source_graph: "philosophy",
                    collection,
                    id,
                    payload: payload.as_bytes(),
                })?;
            }
            let prepared = prepare_philosophy(&mut stage, &vocabulary(), limits)?;
            let roots = stage.core_roots()?;
            assert_eq!((roots.nodes, roots.relations), (0, 0));
            Ok(prepared)
        })();
        drop(stage);
        fs::remove_dir(&candidate.parent().unwrap()).unwrap();
        prepared
    }
    // Root literals were computed independently with CPython hashlib over the
    // exact UTF-8 fixture bytes and length-prefixed IDs.
    #[test]
    fn python_oracle_prepared_receipt_and_zero_rows() {
        let rows = fixture();
        assert_eq!(input_root(&rows, "nodes").1, NODE_ROOT);
        assert_eq!(input_root(&rows, "edges").1, EDGE_ROOT);
        let prepared = run(&rows, &rows, limits()).unwrap();
        assert_eq!(
            (
                prepared.nodes,
                prepared.edges,
                prepared.edge_view_bindings,
                prepared.unresolved_endpoint_refs
            ),
            (2, 1, 2, 0)
        );
        assert_eq!(prepared.dependency_root_sha256, PREPARED_ROOT);
        assert_eq!(prepared.source_cut, "sealed-philosophy-cut");
        assert!(!prepared.final_graph_rows_written);
        let empty = run(&[], &[], limits()).unwrap();
        assert_eq!(
            (empty.nodes, empty.edges, empty.unresolved_endpoint_refs),
            (0, 0, 0)
        );
        assert_eq!(empty.node_input_root_sha256, EMPTY);
        assert_eq!(empty.dependency_root_sha256, EMPTY_PREPARED_ROOT);
    }
    #[test]
    fn missing_duplicate_malformed_and_unresolved_refs() {
        let rows = fixture();
        let mut missing = rows.clone();
        missing.remove(0);
        assert!(run(&missing, &rows, limits()).is_err());
        let mut duplicate = rows.clone();
        duplicate.push(rows[0].clone());
        assert!(run(&duplicate, &duplicate, limits()).is_err());
        let mut malformed = rows.clone();
        malformed[0].2 = malformed[0]
            .2
            .replace("\"node_id\":\"n:a\"", "\"node_id\":\"wrong\"");
        assert!(run(&malformed, &malformed, limits()).is_err());
        let mut unresolved = rows.clone();
        unresolved[2].2 = unresolved[2]
            .2
            .replace("\"to_id\":\"n:b\"", "\"to_id\":\"n:missing\"");
        let prepared = run(&unresolved, &unresolved, limits()).unwrap();
        assert_eq!(prepared.unresolved_endpoint_refs, 1);
        let mut small = limits();
        small.max_edge_view_bindings = 2;
        assert!(run(&rows, &rows, small).is_err());
    }
    // Existing raw prepared fixture, now taken through the native base producer.
    // Revision literals freeze CPython _normalize_node/_normalize_relation with
    // the exact authored registries below, never pre-normalized test carriers.
    #[test]
    fn python_oracle_raw_philosophy_materializes_native_graph_bases() {
        use crate::{
            KnowledgeRegistry, PhilosophyMaterializeLimits, PhilosophyNormalizer,
            materialize_philosophy_nodes, materialize_philosophy_relations,
        };
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let descriptor = include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
        );
        let vocabulary = QueryVocabulary::parse(
            descriptor,
            &[
                "philosophy-node-edge-v1",
                "canon-node-relation-v1",
                "candidate-relation-v1",
                "source-navigation-node-edge-v1",
                "reified-bibliographic-claims-v1",
                "declared-identity-and-source-ref-joins-v1",
                "repository-topology-v1",
                "indexed-node-edge-v1",
            ],
        )
        .unwrap();
        let registry = KnowledgeRegistry::parse(entity, relation).unwrap();
        let normalizer = PhilosophyNormalizer::new(
            &registry,
            entity,
            relation,
            &vocabulary,
            descriptor,
            PhilosophyMaterializeLimits {
                max_raw_bytes: 4096,
                max_output_bytes: 32768,
                max_registry_bytes: 4 * 1024 * 1024,
                max_page_rows: 1,
                max_rows: 16,
                max_work_bytes: 128 * 1024,
            },
        )
        .unwrap();
        let mut rows = fixture();
        let mut alpha: Value = serde_json::from_str(&rows[0].2).unwrap();
        alpha["properties"] = serde_json::json!({"original_node_type":" concept ",
            "variant_labels":[{"language":"fr","value":"Alpha source"}],
            "period":"classical period","packet_id":"fixture-packet",
            "claim_ref":"tos.claim.fixture","qualifiers":{"negated":false},
            "confidence":false,"master_confidence":0});
        rows[0].2 = serde_json::to_string(&alpha).unwrap();
        rows[1].2 = rows[1]
            .2
            .replace("\"node_type\":\"concept\"", "\"node_type\":\"atlas\"");
        rows[2].2 = rows[2].2.replace(
            "\"predicate_id\":\"relates\"",
            "\"predicate_id\":\"influences\"",
        );
        let mut receipt = receipt(&rows);
        let selected = vocabulary
            .sources
            .iter()
            .find(|s| s.adapter_profile == PROFILE)
            .unwrap();
        for input in &mut receipt.collections {
            input.input_role = selected.input_role.clone();
        }
        let owner = Owner;
        let quota = TestQuota;
        let candidate = path();
        let mut stage = KnowledgeStage::create(
            &candidate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 4096,
            },
            receipt,
            &owner,
            &quota,
        )
        .unwrap();
        for (collection, id, payload) in &rows {
            stage
                .ingest_input(InputRow {
                    source_graph: "philosophy",
                    collection,
                    id,
                    payload: payload.as_bytes(),
                })
                .unwrap();
        }
        let prepared = prepare_philosophy(&mut stage, &vocabulary, limits()).unwrap();
        let first = normalizer
            .normalize_node(&mut stage, &prepared, "n:a")
            .unwrap();
        assert_eq!(first.ordered_source_raw(), rows[0].2.as_bytes());
        let nodes = materialize_philosophy_nodes(&mut stage, &normalizer, &prepared).unwrap();
        assert_eq!(nodes.rows, 2);
        assert!(!nodes.finalization_complete);
        let title_lookup =
            |stage: &mut KnowledgeStage<'_>, left: &str, right: &str| -> Result<(Value, Value)> {
                stage.with_connection(WritePhase::Sort, |db| {
                    let title = |id: &str| -> Result<Value> {
                        let raw: Vec<u8> = db.query_row(
                            "SELECT payload FROM knowledge_nodes WHERE id=?1",
                            [id],
                            |r| r.get(0),
                        )?;
                        let value: Value = serde_json::from_slice(&raw).unwrap();
                        Ok(value["display"]["title"].clone())
                    };
                    Ok((title(left)?, title(right)?))
                })
            };
        // Fixture lookup is indexed over real native-produced node bases.
        // Global title root authority and closure belong to the parent assembler.
        let relations = materialize_philosophy_relations(
            &mut stage,
            &normalizer,
            &prepared,
            &prepared.source_cut,
            &"0".repeat(64),
            title_lookup,
        )
        .unwrap();
        assert_eq!(relations.rows, 1);
        assert!(!relations.finalization_complete);
        stage
            .with_connection(WritePhase::Sort, |db| {
                for (table, id, revision) in [
                    (
                        "knowledge_nodes",
                        "philosophy:n:a",
                        "a5b5e815fcc7747c1431e88b5f99879cb0538f9871109b7caf5fbe892fde3f28",
                    ),
                    (
                        "knowledge_nodes",
                        "philosophy:n:b",
                        "dcc54be92aa7b81b3e5f5e3cadca6b1f6d97e2c9fe157752313b7783452eff1b",
                    ),
                    (
                        "knowledge_relations",
                        "philosophy:e:ab",
                        "7d0d50e1611c139a325631cd39b34c0487e3df6f64d08e25ee59d3333abdff32",
                    ),
                ] {
                    let raw: Vec<u8> = db.query_row(
                        &format!("SELECT payload FROM {table} WHERE id=?1"),
                        [id],
                        |r| r.get(0),
                    )?;
                    let value: Value = serde_json::from_slice(&raw).unwrap();
                    assert_eq!(value["content_revision"], revision);
                }
                Ok(())
            })
            .unwrap();
        let core = stage.core_roots().unwrap();
        assert_eq!((core.nodes, core.relations), (2, 1));
        // A changed prepared dependency cannot be reused for another pass.
        let mut stale = prepared.clone();
        stale.dependency_root_sha256 = "1".repeat(64);
        assert!(
            clear_inner(
                &mut stage,
                &vocabulary,
                &stale,
                limits(),
                core.nodes,
                &core.node_sha256,
                core.relations,
                &core.relation_sha256
            )
            .is_err()
        );
        clear_philosophy_prepare(
            &mut stage,
            &vocabulary,
            &prepared,
            limits(),
            core.nodes,
            &core.node_sha256,
            core.relations,
            &core.relation_sha256,
        )
        .unwrap();
        let cleaned = stage.core_roots().unwrap();
        assert_eq!(
            (cleaned.node_sha256, cleaned.relation_sha256),
            (core.node_sha256, core.relation_sha256)
        );
        stage
            .with_connection(WritePhase::Sort, |db| {
                let tables: i64 = db.query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name LIKE 'knowledge_philosophy_%'",
                    [],
                    |r| r.get(0),
                )?;
                assert_eq!(tables, 0);
                Ok(())
            })
            .unwrap();
        drop(stage);
        fs::remove_dir(candidate.parent().unwrap()).unwrap();
    }
}
