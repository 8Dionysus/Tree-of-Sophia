//! Private, bounded preparation of a descriptor-selected source-navigation cut.
//!
//! The owner nodes/edges remain in `raw_records`. These indexes retain exact
//! identities and dependency addresses; they do not normalize final graph
//! rows or admit a source header, rights, source currentness, or semantics.

use crate::knowledge_stage::{ExactInputReceipt, KnowledgeStage, SeekRow, WritePhase};
use crate::{Error, QueryVocabulary, Result, knowledge_normalization::SourceRow};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use tos_foundation::{Digest256, Digest256Hasher};

const PROFILE: &str = "source-navigation-node-edge-v1";
const COLLECTIONS: [&str; 2] = ["nodes", "edges"];
const MAX_ADDRESS: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug)]
pub struct NavigationPrepareLimits {
    pub max_nodes: u64,
    pub max_edges: u64,
    pub max_endpoint_refs: u64,
    pub max_page_rows: usize,
    pub max_row_bytes: usize,
    pub max_header_bytes: usize,
    pub max_work_bytes: u64,
}
impl NavigationPrepareLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_edges == 0
            || self.max_endpoint_refs == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_header_bytes == 0
            || self.max_header_bytes > 1024 * 1024
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("navigation preparation limits"));
        }
        Ok(())
    }
}

/// Exact detached header bytes supplied by the owner. A digest here is a
/// mechanical check of those bytes; StageOwner must separately admit the cut.
#[derive(Clone, Debug)]
pub struct NavigationHeaderClaim {
    pub raw_json: Vec<u8>,
    pub expected_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationExternalDependency {
    EntityTypeHierarchyLabelsAndRecordVersionView,
    RelationPredicatePolicyLabelsAndEndpointTitles,
    SourceDossierAndIdentityPairing,
    RetainedObjectLinkClaimParity,
    GlobalEndpointPlaceholdersAndInheritedViews,
    SemanticInterchangeAndReadableContext,
    IndependentRightsCurrentnessAndSourceAdmission,
}
const EXTERNAL_DEPENDENCIES: &[NavigationExternalDependency] = &[
    NavigationExternalDependency::EntityTypeHierarchyLabelsAndRecordVersionView,
    NavigationExternalDependency::RelationPredicatePolicyLabelsAndEndpointTitles,
    NavigationExternalDependency::SourceDossierAndIdentityPairing,
    NavigationExternalDependency::RetainedObjectLinkClaimParity,
    NavigationExternalDependency::GlobalEndpointPlaceholdersAndInheritedViews,
    NavigationExternalDependency::SemanticInterchangeAndReadableContext,
    NavigationExternalDependency::IndependentRightsCurrentnessAndSourceAdmission,
];

#[derive(Clone, Debug)]
pub struct NavigationPrepareReceipt {
    pub source_graph: String,
    pub input_role: String,
    pub source_cut: String,
    pub nodes: u64,
    pub edges: u64,
    pub endpoint_refs: u64,
    pub unresolved_endpoint_refs: u64,
    pub header_claim_sha256: String,
    pub header_claim_rights_count: u64,
    pub node_input_root_sha256: String,
    pub edge_input_root_sha256: String,
    pub dependency_root_sha256: String,
    pub external_dependencies: &'static [NavigationExternalDependency],
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
    limits: NavigationPrepareLimits,
) -> Result<Selected> {
    let sources: Vec<_> = vocabulary
        .sources
        .iter()
        .filter(|s| s.adapter_profile == PROFILE)
        .collect();
    if sources.len() != 1 {
        return Err(Error::Invalid("navigation adapter profile selection"));
    }
    let source = sources[0];
    let mut expected = std::array::from_fn(|_| (0, String::new()));
    for (i, collection) in COLLECTIONS.iter().enumerate() {
        let entries: Vec<_> = receipt
            .collections
            .iter()
            .filter(|e| e.source_graph == source.source_graph_id && e.collection == *collection)
            .collect();
        if entries.len() != 1
            || entries[0].input_role != source.input_role
            || entries[0].adapter_profile != PROFILE
        {
            return Err(Error::Invalid("navigation input collection registration"));
        }
        expected[i] = (
            entries[0].expected_count,
            entries[0].expected_root_sha256.clone(),
        );
    }
    if receipt.collections.iter().any(|e| {
        e.source_graph == source.source_graph_id && !COLLECTIONS.contains(&e.collection.as_str())
    }) {
        return Err(Error::Invalid("unknown navigation input collection"));
    }
    if expected[0].0 > limits.max_nodes || expected[1].0 > limits.max_edges {
        return Err(Error::Budget("navigation registered row count"));
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
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
        .ok_or(Error::Invalid("navigation owner field"))
}
fn native_id<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let id = required(value, field)?;
    if id.trim() != id {
        return Err(Error::Invalid("navigation ambiguous identity whitespace"));
    }
    Ok(id)
}
fn optional_source(value: &Value, field: &str) -> Result<Option<String>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s))
            if !s.is_empty() && s.len() <= 4096 && s.trim() == s && !s.contains('\0') =>
        {
            Ok(Some(s.clone()))
        }
        _ => Err(Error::Invalid("navigation endpoint source graph")),
    }
}
fn exact_header(
    claim: &NavigationHeaderClaim,
    selected: &Selected,
    limits: NavigationPrepareLimits,
) -> Result<u64> {
    if claim.raw_json.is_empty() || claim.raw_json.len() > limits.max_header_bytes {
        return Err(Error::Budget("navigation header bytes"));
    }
    let expected = Digest256::from_hex(&claim.expected_sha256)
        .map_err(|_| Error::Invalid("navigation header digest"))?;
    if Digest256::of_bytes(&claim.raw_json) != expected {
        return Err(Error::Invalid("navigation header bytes differ"));
    }
    let header = SourceRow::parse(&claim.raw_json, limits.max_header_bytes)?;
    let value = header.value();
    if required(&value, "schema_version")? != "tos_source_navigation_v1" {
        return Err(Error::Invalid("navigation header schema"));
    }
    required(&value, "authority_boundary")?;
    if value.get("nodes").is_some() || value.get("edges").is_some() || value.get("rights").is_some()
    {
        return Err(Error::Invalid("navigation header must be detached"));
    }
    let counts = value
        .get("counts")
        .and_then(Value::as_object)
        .ok_or(Error::Invalid("navigation header counts"))?;
    if counts.len() != 3 {
        return Err(Error::Invalid("navigation header count coverage"));
    }
    let count = |name: &str| {
        counts
            .get(name)
            .and_then(Value::as_u64)
            .filter(|n| *n <= MAX_ADDRESS)
            .ok_or(Error::Invalid("navigation header count"))
    };
    if count("nodes")? != selected.expected[0].0 || count("edges")? != selected.expected[1].0 {
        return Err(Error::Invalid("navigation header count differs"));
    }
    count("rights")
}
fn digest(value: &str) -> Result<[u8; 32]> {
    Ok(*Digest256::from_hex(value)
        .map_err(|_| Error::Invalid("navigation row digest"))?
        .as_bytes())
}
fn root_item(hash: &mut Digest256Hasher, id: &str, sha: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(sha);
}
fn root_text(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
fn charge(work: &mut u64, bytes: usize, limits: NavigationPrepareLimits) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .ok_or(Error::Budget("navigation work bytes"))?;
    if *work > limits.max_work_bytes {
        return Err(Error::Budget("navigation work bytes"));
    }
    Ok(())
}
struct Walk {
    count: u64,
    root: Digest256Hasher,
    after: Option<String>,
}
impl Walk {
    fn new() -> Self {
        Self {
            count: 0,
            root: Digest256Hasher::new(),
            after: None,
        }
    }
    fn add(&mut self, row: &SeekRow, cap: u64) -> Result<()> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or(Error::Budget("navigation rows"))?;
        if self.count > cap {
            return Err(Error::Budget("navigation rows"));
        }
        root_item(&mut self.root, &row.id, &digest(&row.payload_sha256)?);
        Ok(())
    }
    fn complete(self, expected: &(u64, String)) -> Result<u64> {
        if self.count != expected.0 || self.root.finalize().to_hex() != expected.1 {
            return Err(Error::Invalid("navigation input count/root mismatch"));
        }
        Ok(self.count)
    }
}
fn create_tables(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.create_preparation_tables(r#"
CREATE TABLE knowledge_navigation_nodes(
 node_id TEXT PRIMARY KEY, node_kind TEXT NOT NULL, label TEXT NOT NULL,
 source_ref TEXT NOT NULL, identity_status TEXT NOT NULL,
 raw_sha256 BLOB NOT NULL CHECK(length(raw_sha256)=32));
CREATE TABLE knowledge_navigation_edges(
 edge_id TEXT PRIMARY KEY, from_id TEXT NOT NULL, to_id TEXT NOT NULL,
 predicate_id TEXT NOT NULL, edge_kind TEXT NOT NULL, review_status TEXT NOT NULL,
 raw_sha256 BLOB NOT NULL CHECK(length(raw_sha256)=32));
CREATE TABLE knowledge_navigation_endpoints(
 edge_id TEXT NOT NULL, endpoint_role TEXT NOT NULL CHECK(endpoint_role IN ('from','to')),
 source_graph TEXT NOT NULL, native_id TEXT NOT NULL,
 locally_resolved INTEGER NOT NULL CHECK(locally_resolved IN (0,1)),
 PRIMARY KEY(edge_id,endpoint_role));
CREATE INDEX knowledge_navigation_endpoints_seek
 ON knowledge_navigation_endpoints(source_graph,native_id,edge_id);
"#)
}

struct NodeIndex {
    id: String,
    kind: String,
    label: String,
    source_ref: String,
    identity_status: String,
    sha: [u8; 32],
}
fn walk_nodes(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: NavigationPrepareLimits,
    work: &mut u64,
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
            let source = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
            let value = source.value();
            if native_id(value, "node_id")? != raw.id {
                return Err(Error::Invalid("navigation node identity"));
            }
            if !value.get("properties").is_some_and(Value::is_object) {
                return Err(Error::Invalid("navigation node properties"));
            }
            prepared.push(NodeIndex {
                id: raw.id.clone(),
                kind: required(value, "node_kind")?.into(),
                label: required(value, "label")?.into(),
                source_ref: required(value, "source_ref")?.into(),
                identity_status: required(value, "identity_status")?.into(),
                sha: digest(&raw.payload_sha256)?,
            });
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for row in prepared {
                tx.execute(
                    "INSERT INTO knowledge_navigation_nodes VALUES (?1,?2,?3,?4,?5,?6)",
                    params![
                        row.id,
                        row.kind,
                        row.label,
                        row.source_ref,
                        row.identity_status,
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
    kind: String,
    review: String,
    from_source: Option<String>,
    to_source: Option<String>,
    sha: [u8; 32],
}
fn walk_edges(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: NavigationPrepareLimits,
    work: &mut u64,
) -> Result<u64> {
    let mut walk = Walk::new();
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
            if walk
                .count
                .checked_mul(2)
                .is_none_or(|n| n > limits.max_endpoint_refs)
            {
                return Err(Error::Budget("navigation endpoint refs"));
            }
            let source = SourceRow::parse(&raw.payload, limits.max_row_bytes)?;
            let value = source.value();
            if native_id(value, "edge_id")? != raw.id {
                return Err(Error::Invalid("navigation edge identity"));
            }
            let refs = value
                .get("source_refs")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("navigation source refs"))?;
            if refs
                .iter()
                .any(|r| r.as_str().is_none_or(|s| s.is_empty() || s.len() > 4096))
            {
                return Err(Error::Invalid("navigation source ref member"));
            }
            if value.get("properties").is_some_and(|v| !v.is_object()) {
                return Err(Error::Invalid("navigation edge properties"));
            }
            prepared.push(EdgeIndex {
                id: raw.id.clone(),
                from: native_id(value, "from_id")?.into(),
                to: native_id(value, "to_id")?.into(),
                predicate: required(value, "predicate_id")?.into(),
                kind: required(value, "edge_kind")?.into(),
                review: required(value, "review_status")?.into(),
                from_source: optional_source(value, "from_source_graph")?,
                to_source: optional_source(value, "to_source_graph")?,
                sha: digest(&raw.payload_sha256)?,
            });
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for row in prepared {
                tx.execute("INSERT INTO knowledge_navigation_edges VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![row.id,row.from,row.to,row.predicate,row.kind,row.review,&row.sha[..]])?;
                for (role, source, native) in [("from",row.from_source.as_deref().unwrap_or(&selected.source),&row.from),
                    ("to",row.to_source.as_deref().unwrap_or(&selected.source),&row.to)] {
                    tx.execute("INSERT INTO knowledge_navigation_endpoints(edge_id,endpoint_role,source_graph,native_id,locally_resolved) VALUES (?1,?2,?3,?4,0)",
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
    walk.complete(&selected.expected[1])
}
fn resolve_endpoints(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: NavigationPrepareLimits,
) -> Result<(u64, u64)> {
    stage.with_connection(WritePhase::Sort, |db| {
        let count: i64 = db.query_row("SELECT count(*) FROM knowledge_navigation_endpoints", [], |r| r.get(0))?;
        let count = u64::try_from(count).map_err(|_| Error::Budget("navigation endpoint count"))?;
        if count > limits.max_endpoint_refs { return Err(Error::Budget("navigation endpoint refs")); }
        db.execute("UPDATE knowledge_navigation_endpoints SET locally_resolved=1 WHERE source_graph=?1 AND EXISTS (SELECT 1 FROM knowledge_navigation_nodes n WHERE n.node_id=knowledge_navigation_endpoints.native_id)", params![selected.source])?;
        let unresolved: i64 = db.query_row("SELECT count(*) FROM knowledge_navigation_endpoints WHERE locally_resolved=0", [], |r| r.get(0))?;
        Ok((count,u64::try_from(unresolved).map_err(|_| Error::Budget("navigation unresolved endpoints"))?))
    })
}
fn dependency_root(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    header_sha: &str,
) -> Result<String> {
    stage.with_connection(WritePhase::Sort, |db| {
        let mut hash = Digest256Hasher::new(); hash.update(b"tos-navigation-prepared-v1\0");
        root_text(&mut hash, &selected.source);
        root_text(&mut hash, &selected.role);
        root_text(&mut hash, &selected.cut);
        root_text(&mut hash,header_sha);
        for (name,sql,fields) in [
            ("nodes","SELECT node_id,raw_sha256,node_kind,label,source_ref,identity_status FROM knowledge_navigation_nodes ORDER BY node_id",6),
            ("edges","SELECT edge_id,raw_sha256,from_id,to_id,predicate_id,edge_kind,review_status FROM knowledge_navigation_edges ORDER BY edge_id",7),
        ] {
            root_text(&mut hash,name);
            let mut statement=db.prepare(sql)?; let mut rows=statement.query([])?;
            while let Some(row)=rows.next()? {
                let id:String=row.get(0)?; let sha:Vec<u8>=row.get(1)?;
                if sha.len()!=32 { return Err(Error::Invalid("navigation prepared digest")); }
                root_item(&mut hash,&id,&sha);
                for i in 2..fields { let value:String=row.get(i)?; root_text(&mut hash,&value); }
            }
        }
        root_text(&mut hash,"endpoints");
        let mut statement=db.prepare("SELECT edge_id,endpoint_role,source_graph,native_id,locally_resolved FROM knowledge_navigation_endpoints ORDER BY edge_id,endpoint_role")?;
        let mut rows=statement.query([])?;
        while let Some(row)=rows.next()? {
            for i in 0..4 { let value:String=row.get(i)?; root_text(&mut hash,&value); }
            let resolved:i64=row.get(4)?; hash.update(&[u8::try_from(resolved).map_err(|_| Error::Invalid("navigation endpoint flag"))?]);
        }
        Ok(hash.finalize().to_hex())
    })
}
fn prepare_inner(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    header: &NavigationHeaderClaim,
    limits: NavigationPrepareLimits,
) -> Result<NavigationPrepareReceipt> {
    limits.validate()?;
    let selected = select(vocabulary, stage.exact_receipt()?, limits)?;
    let rights = exact_header(header, &selected, limits)?;
    create_tables(stage)?;
    let mut work = header.raw_json.len() as u64;
    if work > limits.max_work_bytes {
        return Err(Error::Budget("navigation work bytes"));
    }
    let nodes = walk_nodes(stage, &selected, limits, &mut work)?;
    let edges = walk_edges(stage, &selected, limits, &mut work)?;
    let (endpoint_refs, unresolved_endpoint_refs) = resolve_endpoints(stage, &selected, limits)?;
    let dependency_root_sha256 = dependency_root(stage, &selected, &header.expected_sha256)?;
    Ok(NavigationPrepareReceipt {
        source_graph: selected.source,
        input_role: selected.role,
        source_cut: selected.cut,
        nodes,
        edges,
        endpoint_refs,
        unresolved_endpoint_refs,
        header_claim_sha256: header.expected_sha256.clone(),
        header_claim_rights_count: rights,
        node_input_root_sha256: selected.expected[0].1.clone(),
        edge_input_root_sha256: selected.expected[1].1.clone(),
        dependency_root_sha256,
        external_dependencies: EXTERNAL_DEPENDENCIES,
        final_graph_rows_written: false,
    })
}
/// Prepare exact owner carriers; poison the private stage on every failure.
pub fn prepare_source_navigation(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    header: &NavigationHeaderClaim,
    limits: NavigationPrepareLimits,
) -> Result<NavigationPrepareReceipt> {
    let result = prepare_inner(stage, vocabulary, header, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Future all-source assembler result. Fields are private and this module
/// deliberately offers no constructor until raw-node/edge coverage and the
/// global missing-endpoint root have an independently checked producer.
/// A caller cannot turn a self-asserted `complete` flag into cleanup authority.
pub struct NavigationJoinClosure {
    pub(crate) source_graph: String,
    pub(crate) source_cut: String,
    pub(crate) prepared_dependency_root_sha256: String,
    pub(crate) raw_node_count: u64,
    pub(crate) raw_node_input_root_sha256: String,
    pub(crate) raw_edge_count: u64,
    pub(crate) raw_edge_input_root_sha256: String,
    pub(crate) placeholder_count: u64,
    pub(crate) placeholder_absence_root_sha256: String,
    pub(crate) core_node_count: u64,
    pub(crate) core_node_root_sha256: String,
    pub(crate) core_relation_count: u64,
    pub(crate) core_relation_root_sha256: String,
}

fn clear_inner(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    header: &NavigationHeaderClaim,
    prepared: &NavigationPrepareReceipt,
    closure: &NavigationJoinClosure,
    limits: NavigationPrepareLimits,
) -> Result<()> {
    limits.validate()?;
    let selected = select(vocabulary, stage.exact_receipt()?, limits)?;
    let rights = exact_header(header, &selected, limits)?;
    if prepared.source_graph != selected.source
        || prepared.input_role != selected.role
        || prepared.source_cut != selected.cut
        || prepared.nodes != selected.expected[0].0
        || prepared.edges != selected.expected[1].0
        || prepared.node_input_root_sha256 != selected.expected[0].1
        || prepared.edge_input_root_sha256 != selected.expected[1].1
        || prepared.header_claim_sha256 != header.expected_sha256
        || prepared.header_claim_rights_count != rights
        || prepared.external_dependencies != EXTERNAL_DEPENDENCIES
        || prepared.final_graph_rows_written
        || closure.source_graph != prepared.source_graph
        || closure.source_cut != prepared.source_cut
        || closure.prepared_dependency_root_sha256 != prepared.dependency_root_sha256
        || closure.raw_node_count != prepared.nodes
        || closure.raw_edge_count != prepared.edges
        || closure.raw_node_input_root_sha256 != prepared.node_input_root_sha256
        || closure.raw_edge_input_root_sha256 != prepared.edge_input_root_sha256
        || Digest256::from_hex(&closure.placeholder_absence_root_sha256).is_err()
        || Digest256::from_hex(&closure.core_node_root_sha256).is_err()
        || Digest256::from_hex(&closure.core_relation_root_sha256).is_err()
    {
        return Err(Error::Invalid("navigation cleanup closure binding"));
    }
    let actual_root = dependency_root(stage, &selected, &header.expected_sha256)?;
    if actual_root != prepared.dependency_root_sha256 {
        return Err(Error::Invalid("navigation cleanup prepared root drift"));
    }
    let core = stage.core_roots()?;
    if core.nodes != closure.core_node_count
        || core.node_sha256 != closure.core_node_root_sha256
        || core.relations != closure.core_relation_count
        || core.relation_sha256 != closure.core_relation_root_sha256
    {
        return Err(Error::Invalid("navigation cleanup core root drift"));
    }
    stage.with_connection(WritePhase::Finalize, |db| {
        let tx = db.transaction()?;
        let count = |sql: &str| -> Result<u64> {
            let value: i64 = tx.query_row(sql, [], |row| row.get(0))?;
            u64::try_from(value).map_err(|_| Error::Budget("navigation cleanup count"))
        };
        let nodes = count("SELECT count(*) FROM knowledge_navigation_nodes")?;
        let edges = count("SELECT count(*) FROM knowledge_navigation_edges")?;
        let endpoints = count("SELECT count(*) FROM knowledge_navigation_endpoints")?;
        let unresolved =
            count("SELECT count(*) FROM knowledge_navigation_endpoints WHERE locally_resolved=0")?;
        if nodes != prepared.nodes
            || edges != prepared.edges
            || endpoints != prepared.endpoint_refs
            || unresolved != prepared.unresolved_endpoint_refs
            || endpoints
                != edges
                    .checked_mul(2)
                    .ok_or(Error::Budget("navigation cleanup endpoint count"))?
        {
            return Err(Error::Invalid("navigation cleanup prepared count drift"));
        }
        let source = &prepared.source_graph;
        let covered_nodes: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM knowledge_navigation_nodes p WHERE NOT EXISTS (
             SELECT 1 FROM knowledge_nodes n WHERE n.id=?1||':'||p.node_id
             AND n.source_graph=?1 AND n.native_id=p.node_id) LIMIT 1",
                params![source],
                |row| row.get(0),
            )
            .optional()?;
        let covered_edges: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM knowledge_navigation_edges p WHERE NOT EXISTS (
             SELECT 1 FROM knowledge_relations r WHERE r.id=?1||':'||p.edge_id
             AND r.source_graph=?1 AND r.native_id=p.edge_id) LIMIT 1",
                params![source],
                |row| row.get(0),
            )
            .optional()?;
        if covered_nodes.is_some() || covered_edges.is_some() {
            return Err(Error::Invalid("navigation cleanup raw coverage"));
        }
        let nav_nodes: i64 = tx.query_row(
            "SELECT count(*) FROM knowledge_nodes WHERE source_graph=?1",
            params![source],
            |row| row.get(0),
        )?;
        let nav_relations: i64 = tx.query_row(
            "SELECT count(*) FROM knowledge_relations WHERE source_graph=?1",
            params![source],
            |row| row.get(0),
        )?;
        let expected_nodes = prepared
            .nodes
            .checked_add(closure.placeholder_count)
            .ok_or(Error::Budget("navigation cleanup node coverage"))?;
        if u64::try_from(nav_nodes).ok() != Some(expected_nodes)
            || u64::try_from(nav_relations).ok() != Some(prepared.edges)
        {
            return Err(Error::Invalid("navigation cleanup final family coverage"));
        }
        let extras: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM knowledge_nodes n WHERE n.source_graph=?1 AND NOT EXISTS (
             SELECT 1 FROM knowledge_navigation_nodes p WHERE p.node_id=n.native_id)
             AND (n.kind_id!='relation-endpoint' OR n.native_id IS NULL) LIMIT 1",
                params![source],
                |row| row.get(0),
            )
            .optional()?;
        if extras.is_some() {
            return Err(Error::Invalid("navigation cleanup placeholder shape"));
        }
        // The assembler's separate all-source absence root is the authority
        // for extras; local endpoint flags cannot establish global absence.
        tx.execute_batch(
            "DROP TABLE knowledge_navigation_endpoints;
            DROP TABLE knowledge_navigation_edges;
            DROP TABLE knowledge_navigation_nodes;",
        )?;
        tx.commit()?;
        Ok(())
    })
}

/// Cleanup only after the native all-source assembler supplies its consumed
/// roots and exact placeholder closure. Public clients cannot construct this
/// closure or silently discard private indexes.
pub fn clear_source_navigation_prepare(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    header: &NavigationHeaderClaim,
    prepared: &NavigationPrepareReceipt,
    closure: &NavigationJoinClosure,
    limits: NavigationPrepareLimits,
) -> Result<()> {
    let result = clear_inner(stage, vocabulary, header, prepared, closure, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

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

    type FixtureRow = (&'static str, &'static str, String);
    // Independent CPython hashlib oracle over the literal owner-carrier bytes.
    const NODE_ROOT: &str = "01d97d12f07b669133002912b057c53b4a50b01e7318561056b1455c7a51801c";
    const EDGE_ROOT: &str = "0b57259c0ca134caf612906d1cf09edda91445a842311b692429871be0a03b43";
    const HEADER_SHA: &str = "726156aa58e2c54c7cf29d0e07bde8eb26e750a334ef6771838d15d7dbf3cc7f";
    const PREPARED_ROOT: &str = "d3686c846c0cc24c9da2cb600dcedb9c546f17e306a4642695a5a1f388f27dcc";
    const EMPTY_PREPARED_ROOT: &str =
        "01d5b193dfe7f5a8783d1fa0c20ed667fc0042399bf17ce419717947040e3189";
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
    fn limits() -> NavigationPrepareLimits {
        NavigationPrepareLimits {
            max_nodes: 16,
            max_edges: 16,
            max_endpoint_refs: 32,
            max_page_rows: 2,
            max_row_bytes: 2048,
            max_header_bytes: 2048,
            max_work_bytes: 65536,
        }
    }
    fn vocabulary() -> QueryVocabulary {
        QueryVocabulary {
            query_delivery_caches_retired: false,
            descriptor_sha256: "0".repeat(64),
            descriptor_version: 1,
            sources: vec![RegisteredSource {
                source_graph_id: "navigation-fixture".into(),
                owner_ref: "ToS/source-witnesses/AGENTS.md".into(),
                input_role: "source-navigation".into(),
                adapter_profile: PROFILE.into(),
                representative_priority: 0,
            }],
            registered_source_ids: vec!["navigation-fixture".into()],
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
        ("nodes","n:a",r#"{"node_id":"n:a","node_kind":"work","label":"Alpha","source_ref":"ToS/a.json","identity_status":"not_applicable","properties":{}}"#.into()),
        ("nodes","n:b",r#"{"node_id":"n:b","node_kind":"record-version","label":"Beta","source_ref":"ToS/b.json","identity_status":"not_applicable","properties":{"source_record":{"id":"n:b"}}}"#.into()),
        ("edges","e:ab",r#"{"edge_id":"e:ab","from_id":"n:a","to_id":"n:b","predicate_id":"contains","edge_kind":"authored_branch_hierarchy","review_status":"not_applicable","source_refs":["ToS/a.json"]}"#.into()),
        ("edges","e:missing",r#"{"edge_id":"e:missing","from_id":"n:a","to_id":"n:missing","predicate_id":"references","edge_kind":"source_record_link","review_status":"not_applicable","source_refs":["ToS/a.json"]}"#.into()),
    ]
    }
    fn input_root(rows: &[FixtureRow], collection: &str) -> (u64, String) {
        let mut selected: Vec<_> = rows.iter().filter(|r| r.0 == collection).collect();
        selected.sort_by_key(|r| r.1);
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
    fn header(rows: &[FixtureRow]) -> NavigationHeaderClaim {
        let raw_json=format!("{{\"schema_version\":\"tos_source_navigation_v1\",\"authority_boundary\":\"fixture detached header\",\"counts\":{{\"nodes\":{},\"edges\":{},\"rights\":0}}}}",
            input_root(rows,"nodes").0,input_root(rows,"edges").0).into_bytes();
        NavigationHeaderClaim {
            expected_sha256: Digest256::of_bytes(&raw_json).to_hex(),
            raw_json,
        }
    }
    fn receipt(rows: &[FixtureRow]) -> ExactInputReceipt {
        ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "sealed-navigation-cut".into(),
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
                        source_graph: "navigation-fixture".into(),
                        collection: (*collection).into(),
                        input_role: "source-navigation".into(),
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
            "tos-navigation-prepare-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir.join("private.sqlite3")
    }
    fn run(
        rows: &[FixtureRow],
        registered: &[FixtureRow],
        header_claim: NavigationHeaderClaim,
        limits: NavigationPrepareLimits,
    ) -> Result<NavigationPrepareReceipt> {
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
            receipt(registered),
            &owner,
            &quota,
        )?;
        let result = (|| {
            for (collection, id, payload) in rows {
                stage.ingest_input(InputRow {
                    source_graph: "navigation-fixture",
                    collection,
                    id,
                    payload: payload.as_bytes(),
                })?;
            }
            let result =
                prepare_source_navigation(&mut stage, &vocabulary(), &header_claim, limits)?;
            let roots = stage.core_roots()?;
            assert_eq!((roots.nodes, roots.relations), (0, 0));
            Ok(result)
        })();
        drop(stage);
        fs::remove_dir(candidate.parent().unwrap()).unwrap();
        result
    }
    #[test]
    fn exact_carriers_make_only_private_index_and_zero_graph_rows() {
        let rows = fixture();
        assert_eq!(input_root(&rows, "nodes").1, NODE_ROOT);
        assert_eq!(input_root(&rows, "edges").1, EDGE_ROOT);
        assert_eq!(header(&rows).expected_sha256, HEADER_SHA);
        let result = run(&rows, &rows, header(&rows), limits()).unwrap();
        assert_eq!(
            (
                result.nodes,
                result.edges,
                result.endpoint_refs,
                result.unresolved_endpoint_refs
            ),
            (2, 2, 4, 1)
        );
        assert!(!result.final_graph_rows_written);
        assert_eq!(result.source_cut, "sealed-navigation-cut");
        assert_eq!(result.dependency_root_sha256, PREPARED_ROOT);
        let empty: Vec<FixtureRow> = vec![];
        let result = run(&empty, &empty, header(&empty), limits()).unwrap();
        assert_eq!(
            (result.nodes, result.edges, result.endpoint_refs),
            (0, 0, 0)
        );
        assert_eq!(result.dependency_root_sha256, EMPTY_PREPARED_ROOT);
    }
    #[test]
    fn missing_duplicate_identity_header_and_budget_refuse() {
        let rows = fixture();
        let mut missing = rows.clone();
        missing.pop();
        assert!(run(&missing, &rows, header(&rows), limits()).is_err());
        let mut duplicate = rows.clone();
        duplicate.push(rows[0].clone());
        assert!(run(&duplicate, &duplicate, header(&duplicate), limits()).is_err());
        let mut wrong = rows.clone();
        wrong[0].2 = wrong[0]
            .2
            .replace("\"node_id\":\"n:a\"", "\"node_id\":\"wrong\"");
        assert!(run(&wrong, &wrong, header(&wrong), limits()).is_err());
        let mut claim = header(&rows);
        claim.raw_json[0] = b'[';
        assert!(run(&rows, &rows, claim, limits()).is_err());
        let mut tiny = limits();
        tiny.max_endpoint_refs = 3;
        assert!(run(&rows, &rows, header(&rows), tiny).is_err());
    }

    #[test]
    fn cleanup_refuses_unjoined_private_indexes_and_poisons_stage() {
        let rows = fixture();
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
            receipt(&rows),
            &owner,
            &quota,
        )
        .unwrap();
        for (collection, id, payload) in &rows {
            stage
                .ingest_input(InputRow {
                    source_graph: "navigation-fixture",
                    collection,
                    id,
                    payload: payload.as_bytes(),
                })
                .unwrap();
        }
        let claimed_header = header(&rows);
        let prepared =
            prepare_source_navigation(&mut stage, &vocabulary(), &claimed_header, limits())
                .unwrap();
        let core = stage.core_roots().unwrap();
        // Deliberately forged only inside this module's unit test. The real
        // assembler has no constructor yet, so production cannot do this.
        let fake = NavigationJoinClosure {
            source_graph: prepared.source_graph.clone(),
            source_cut: prepared.source_cut.clone(),
            prepared_dependency_root_sha256: prepared.dependency_root_sha256.clone(),
            raw_node_count: prepared.nodes,
            raw_node_input_root_sha256: prepared.node_input_root_sha256.clone(),
            raw_edge_count: prepared.edges,
            raw_edge_input_root_sha256: prepared.edge_input_root_sha256.clone(),
            placeholder_count: 0,
            placeholder_absence_root_sha256: "0".repeat(64),
            core_node_count: core.nodes,
            core_node_root_sha256: core.node_sha256,
            core_relation_count: core.relations,
            core_relation_root_sha256: core.relation_sha256,
        };
        assert!(
            clear_source_navigation_prepare(
                &mut stage,
                &vocabulary(),
                &claimed_header,
                &prepared,
                &fake,
                limits()
            )
            .is_err()
        );
        assert!(stage.finish().is_err());
        fs::remove_dir(candidate.parent().unwrap()).unwrap();
    }
}
