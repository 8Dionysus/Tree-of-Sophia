//! Source-claims preflight over one owner-registered sealed bibliographic cut.
//!
//! This creates a private indexed dependency map. It deliberately does not
//! normalize or insert final graph rows: Python's source-claims finalization
//! needs navigation carriers, semantic registries and global endpoint titles.

use crate::knowledge_stage::{ExactInputReceipt, KnowledgeStage, SeekRow, WritePhase};
use crate::{Error, QueryVocabulary, Result, knowledge_normalization::SourceRow};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use tos_foundation::{Digest256, Digest256Hasher};

const PROFILE: &str = "reified-bibliographic-claims-v1";
const COLLECTIONS: [&str; 3] = ["nodes", "edges", "claim_traces"];

#[derive(Clone, Copy, Debug)]
pub struct ClaimPrepareLimits {
    pub max_nodes: u64,
    pub max_edges: u64,
    pub max_claims: u64,
    pub max_page_rows: usize,
    pub max_row_bytes: usize,
    pub max_work_bytes: u64,
}
impl ClaimPrepareLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_edges == 0
            || self.max_claims == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("source-claims preparation limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimExternalDependency {
    EntityTypeHierarchyAndLabels,
    RelationPredicatePolicyAndNavigationTemplate,
    SourceNavigationRetainedObjectLinkParity,
    SourceNavigationDossierAndIdentityPairing,
    GlobalEndpointTitlesAndPlaceholders,
    ClaimLiteralFinalizationAndReadableContext,
    SemanticInterchangeIdentityJoin,
}
const EXTERNAL_DEPENDENCIES: &[ClaimExternalDependency] = &[
    ClaimExternalDependency::EntityTypeHierarchyAndLabels,
    ClaimExternalDependency::RelationPredicatePolicyAndNavigationTemplate,
    ClaimExternalDependency::SourceNavigationRetainedObjectLinkParity,
    ClaimExternalDependency::SourceNavigationDossierAndIdentityPairing,
    ClaimExternalDependency::GlobalEndpointTitlesAndPlaceholders,
    ClaimExternalDependency::ClaimLiteralFinalizationAndReadableContext,
    ClaimExternalDependency::SemanticInterchangeIdentityJoin,
];

#[derive(Clone, Debug)]
pub struct ClaimPrepareReceipt {
    pub source_graph: String,
    pub input_role: String,
    pub source_cut: String,
    pub nodes: u64,
    pub edges: u64,
    pub claim_traces: u64,
    pub node_input_root_sha256: String,
    pub edge_input_root_sha256: String,
    pub trace_input_root_sha256: String,
    /// Root of the private dependency rows; independent owner input roots
    /// still require KnowledgeStage::finish and sealed-cut recheck.
    pub dependency_root_sha256: String,
    pub external_dependencies: &'static [ClaimExternalDependency],
    pub final_graph_rows_written: bool,
}

#[derive(Clone)]
struct Selected {
    source: String,
    role: String,
    expected: [(u64, String); 3],
    cut: String,
}

fn select(
    vocabulary: &QueryVocabulary,
    receipt: &ExactInputReceipt,
    limits: ClaimPrepareLimits,
) -> Result<Selected> {
    let sources: Vec<_> = vocabulary
        .sources
        .iter()
        .filter(|source| source.adapter_profile == PROFILE)
        .collect();
    if sources.len() != 1 {
        return Err(Error::Invalid("source-claims adapter profile selection"));
    }
    let source = sources[0];
    let mut expected = std::array::from_fn(|_| (0, String::new()));
    for (index, collection) in COLLECTIONS.iter().enumerate() {
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
            return Err(Error::Invalid(
                "source-claims input collection registration",
            ));
        }
        expected[index] = (
            entries[0].expected_count,
            entries[0].expected_root_sha256.clone(),
        );
    }
    if receipt.collections.iter().any(|entry| {
        entry.source_graph == source.source_graph_id
            && !COLLECTIONS.contains(&entry.collection.as_str())
    }) {
        return Err(Error::Invalid("unknown source-claims input collection"));
    }
    if expected[0].0 > limits.max_nodes
        || expected[1].0 > limits.max_edges
        || expected[2].0 > limits.max_claims
    {
        return Err(Error::Budget("source-claims registered row count"));
    }
    Ok(Selected {
        source: source.source_graph_id.clone(),
        role: source.input_role.clone(),
        expected,
        cut: receipt.binding.source_cut.clone(),
    })
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .ok_or(Error::Invalid("source-claims required carrier field"))
}
fn digest(value: &str) -> Result<[u8; 32]> {
    let digest = Digest256::from_hex(value).map_err(|_| Error::Invalid("source-claims digest"))?;
    Ok(*digest.as_bytes())
}
fn root_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}
fn hash_text(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
fn charge(work: &mut u64, bytes: usize, limits: ClaimPrepareLimits) -> Result<()> {
    *work = work
        .checked_add(bytes as u64)
        .ok_or(Error::Budget("source-claims work bytes"))?;
    if *work > limits.max_work_bytes {
        return Err(Error::Budget("source-claims work bytes"));
    }
    Ok(())
}
fn row(raw: &SeekRow, limits: ClaimPrepareLimits) -> Result<SourceRow> {
    SourceRow::parse(&raw.payload, limits.max_row_bytes)
}
fn root_row(hash: &mut Digest256Hasher, raw: &SeekRow) -> Result<()> {
    root_item(hash, &raw.id, &digest(&raw.payload_sha256)?);
    Ok(())
}

struct InputWalk {
    count: u64,
    root: Digest256Hasher,
    after: Option<String>,
}
impl InputWalk {
    fn new() -> Self {
        Self {
            count: 0,
            root: Digest256Hasher::new(),
            after: None,
        }
    }
    fn add(&mut self, raw: &SeekRow, cap: u64) -> Result<()> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or(Error::Budget("source-claims rows"))?;
        if self.count > cap {
            return Err(Error::Budget("source-claims rows"));
        }
        root_row(&mut self.root, raw)
    }
    fn complete(self, expected: &(u64, String)) -> Result<u64> {
        if self.count != expected.0 || self.root.finalize().to_hex() != expected.1 {
            return Err(Error::Invalid("source-claims input count/root mismatch"));
        }
        Ok(self.count)
    }
}

fn create_tables(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Schema, |db| {
        db.execute_batch(
            r#"
CREATE TABLE knowledge_claim_dependencies(
 claim_ref TEXT PRIMARY KEY, claim_node_id TEXT NOT NULL, subject_node_id TEXT NOT NULL,
 object_node_id TEXT NOT NULL, predicate_id TEXT NOT NULL,
 source_claim_sha256 BLOB NOT NULL CHECK(length(source_claim_sha256)=32),
 trace_sha256 BLOB NOT NULL CHECK(length(trace_sha256)=32));
CREATE TABLE knowledge_claim_edge_bindings(
 edge_id TEXT PRIMARY KEY, claim_ref TEXT NOT NULL, from_id TEXT NOT NULL,
 to_id TEXT NOT NULL, edge_kind TEXT NOT NULL,
 edge_sha256 BLOB NOT NULL CHECK(length(edge_sha256)=32),
 listed_by_trace INTEGER NOT NULL DEFAULT 0 CHECK(listed_by_trace IN (0,1)),
 FOREIGN KEY(claim_ref) REFERENCES knowledge_claim_dependencies(claim_ref));
CREATE INDEX knowledge_claim_edges_claim ON knowledge_claim_edge_bindings(claim_ref,edge_id);
"#,
        )?;
        Ok(())
    })
}

fn walk_nodes(
    stage: &KnowledgeStage<'_>,
    selected: &Selected,
    limits: ClaimPrepareLimits,
    work: &mut u64,
) -> Result<u64> {
    let mut walk = InputWalk::new();
    loop {
        let page = stage.scan_input(
            &selected.source,
            "nodes",
            walk.after.as_deref(),
            limits.max_page_rows,
        )?;
        for raw in &page.rows {
            charge(work, raw.payload.len(), limits)?;
            walk.add(raw, limits.max_nodes)?;
            let source = row(raw, limits)?;
            let value = source.value();
            if required(value, "node_id")? != raw.id {
                return Err(Error::Invalid("source-claims node identity"));
            }
            required(value, "node_kind")?;
            digest(required(value, "source_sha256")?)?;
            if !value.get("properties").is_some_and(Value::is_object) {
                return Err(Error::Invalid("source-claims node properties"));
            }
        }
        match page.next_id {
            Some(next) => walk.after = Some(next),
            None => break,
        }
    }
    walk.complete(&selected.expected[0])
}

struct ClaimDependencyRow {
    claim_ref: String,
    claim_node_id: String,
    subject_node_id: String,
    object_node_id: String,
    predicate: String,
    claim_sha: [u8; 32],
    trace_sha: [u8; 32],
}

fn walk_traces(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: ClaimPrepareLimits,
    work: &mut u64,
) -> Result<u64> {
    let mut walk = InputWalk::new();
    loop {
        let page = stage.scan_input(
            &selected.source,
            "claim_traces",
            walk.after.as_deref(),
            limits.max_page_rows,
        )?;
        let mut prepared = Vec::with_capacity(page.rows.len());
        for raw in &page.rows {
            charge(work, raw.payload.len(), limits)?;
            walk.add(raw, limits.max_claims)?;
            let source = row(raw, limits)?;
            let value = source.value();
            let claim_ref = required(value, "claim_ref")?;
            if claim_ref != raw.id {
                return Err(Error::Invalid("source-claims trace identity"));
            }
            let claim_node_id = required(value, "claim_node_id")?;
            let subject_node_id = required(value, "subject_node_id")?;
            let object_node_id = required(value, "object_node_id")?;
            let predicate = required(value, "predicate")?;
            let claim_sha = digest(required(value, "claim_sha256")?)?;
            let edges = value
                .get("edge_ids")
                .and_then(Value::as_array)
                .filter(|edges| !edges.is_empty())
                .ok_or(Error::Invalid("source-claims trace edges"))?;
            if edges
                .iter()
                .any(|edge| edge.as_str().is_none_or(str::is_empty))
            {
                return Err(Error::Invalid("source-claims trace edge ID"));
            }
            for id in [claim_node_id, subject_node_id, object_node_id] {
                if stage.raw_by_id(&selected.source, "nodes", id)?.is_none() {
                    return Err(Error::Invalid("source-claims trace node missing"));
                }
            }
            let claim_node = stage
                .raw_by_id(&selected.source, "nodes", claim_node_id)?
                .ok_or(Error::Invalid("source-claims claim node missing"))?;
            let claim_node = SourceRow::parse(&claim_node.payload, limits.max_row_bytes)?;
            let claim_value = claim_node.value();
            if required(claim_value, "node_kind")? != "claim"
                || required(claim_value, "source_sha256")? != required(value, "claim_sha256")?
                || claim_value
                    .get("properties")
                    .and_then(|v| v.get("claim_ref"))
                    .and_then(Value::as_str)
                    != Some(claim_ref)
            {
                return Err(Error::Invalid("source-claims trace/Claim carrier mismatch"));
            }
            prepared.push(ClaimDependencyRow {
                claim_ref: claim_ref.into(),
                claim_node_id: claim_node_id.into(),
                subject_node_id: subject_node_id.into(),
                object_node_id: object_node_id.into(),
                predicate: predicate.into(),
                claim_sha,
                trace_sha: digest(&raw.payload_sha256)?,
            });
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for record in prepared {
                tx.execute(
                    "INSERT INTO knowledge_claim_dependencies VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        record.claim_ref,
                        record.claim_node_id,
                        record.subject_node_id,
                        record.object_node_id,
                        record.predicate,
                        &record.claim_sha[..],
                        &record.trace_sha[..]
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
    walk.complete(&selected.expected[2])
}

struct EdgeBindingRow {
    edge_id: String,
    claim_ref: String,
    from_id: String,
    to_id: String,
    edge_kind: String,
    edge_sha: [u8; 32],
    claim_sha: [u8; 32],
}
fn walk_edges(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: ClaimPrepareLimits,
    work: &mut u64,
) -> Result<u64> {
    let mut walk = InputWalk::new();
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
            let source = row(raw, limits)?;
            let value = source.value();
            let edge_id = required(value, "edge_id")?;
            if edge_id != raw.id {
                return Err(Error::Invalid("source-claims edge identity"));
            }
            let claim_ref = required(value, "claim_ref")?;
            let from_id = required(value, "from_id")?;
            let to_id = required(value, "to_id")?;
            let edge_kind = required(value, "edge_kind")?;
            for id in [from_id, to_id] {
                if stage.raw_by_id(&selected.source, "nodes", id)?.is_none() {
                    return Err(Error::Invalid("source-claims edge endpoint missing"));
                }
            }
            prepared.push(EdgeBindingRow {
                edge_id: edge_id.into(),
                claim_ref: claim_ref.into(),
                from_id: from_id.into(),
                to_id: to_id.into(),
                edge_kind: edge_kind.into(),
                edge_sha: digest(&raw.payload_sha256)?,
                claim_sha: digest(required(value, "claim_sha256")?)?,
            });
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for edge in prepared {
                let affected = tx.execute("INSERT INTO knowledge_claim_edge_bindings(edge_id,claim_ref,from_id,to_id,edge_kind,edge_sha256) \
                    SELECT ?1,?2,?3,?4,?5,?6 FROM knowledge_claim_dependencies \
                    WHERE claim_ref=?2 AND source_claim_sha256=?7", params![
                    edge.edge_id, edge.claim_ref, edge.from_id, edge.to_id, edge.edge_kind,
                    &edge.edge_sha[..], &edge.claim_sha[..]
                ])?;
                if affected != 1 { return Err(Error::Invalid("source-claims edge/trace mismatch")); }
            }
            tx.commit()?;
            Ok(())
        })?;
        match page.next_id {
            Some(next) => walk.after = Some(next),
            None => break,
        }
    }
    walk.complete(&selected.expected[1])
}

fn mark_trace_edges(
    stage: &mut KnowledgeStage<'_>,
    selected: &Selected,
    limits: ClaimPrepareLimits,
    work: &mut u64,
) -> Result<()> {
    let mut after: Option<String> = None;
    loop {
        let page = stage.scan_input(
            &selected.source,
            "claim_traces",
            after.as_deref(),
            limits.max_page_rows,
        )?;
        let mut bindings = Vec::new();
        for raw in &page.rows {
            charge(work, raw.payload.len(), limits)?;
            let source = row(raw, limits)?;
            let value = source.value();
            let claim_ref = required(value, "claim_ref")?;
            let edges = value
                .get("edge_ids")
                .and_then(Value::as_array)
                .ok_or(Error::Invalid("source-claims trace edges"))?;
            for edge in edges {
                let edge_id = edge
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .ok_or(Error::Invalid("source-claims trace edge ID"))?;
                bindings.push((claim_ref.to_owned(), edge_id.to_owned()));
            }
        }
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for (claim_ref, edge_id) in bindings {
                let affected = tx.execute(
                    "UPDATE knowledge_claim_edge_bindings SET listed_by_trace=1 \
                    WHERE edge_id=?1 AND claim_ref=?2 AND listed_by_trace=0",
                    params![edge_id, claim_ref],
                )?;
                if affected != 1 {
                    return Err(Error::Invalid(
                        "source-claims trace edge missing/duplicate/mismatch",
                    ));
                }
            }
            tx.commit()?;
            Ok(())
        })?;
        match page.next_id {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    stage.with_connection(WritePhase::Sort, |db| {
        let unlisted: Option<String> = db
            .query_row(
                "SELECT edge_id FROM knowledge_claim_edge_bindings \
            WHERE listed_by_trace=0 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if unlisted.is_some() {
            return Err(Error::Invalid("source-claims unlisted edge"));
        }
        Ok(())
    })
}

fn dependency_root(stage: &mut KnowledgeStage<'_>) -> Result<String> {
    stage.with_connection(WritePhase::Sort, |db| {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-source-claims-prepared-v1\0");
        for (table, sql) in [
            ("claims", "SELECT claim_ref,trace_sha256 FROM knowledge_claim_dependencies ORDER BY claim_ref"),
            ("edges", "SELECT edge_id,edge_sha256 FROM knowledge_claim_edge_bindings ORDER BY edge_id"),
        ] {
            hash_text(&mut hash, table);
            let mut statement = db.prepare(sql)?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let digest: Vec<u8> = row.get(1)?;
                if digest.len() != 32 { return Err(Error::Invalid("source-claims prepared digest")); }
                root_item(&mut hash, &id, &digest);
            }
        }
        Ok(hash.finalize().to_hex())
    })
}

fn prepare_inner(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: ClaimPrepareLimits,
) -> Result<ClaimPrepareReceipt> {
    limits.validate()?;
    let selected = select(vocabulary, stage.exact_receipt(), limits)?;
    create_tables(stage)?;
    let mut work = 0u64;
    let nodes = walk_nodes(stage, &selected, limits, &mut work)?;
    let claim_traces = walk_traces(stage, &selected, limits, &mut work)?;
    let edges = walk_edges(stage, &selected, limits, &mut work)?;
    if claim_traces == 0 && (nodes != 0 || edges != 0) {
        return Err(Error::Invalid("source-claims rows without claim traces"));
    }
    mark_trace_edges(stage, &selected, limits, &mut work)?;
    let dependency_root_sha256 = dependency_root(stage)?;
    Ok(ClaimPrepareReceipt {
        source_graph: selected.source,
        input_role: selected.role,
        source_cut: selected.cut,
        nodes,
        edges,
        claim_traces,
        node_input_root_sha256: selected.expected[0].1.clone(),
        edge_input_root_sha256: selected.expected[1].1.clone(),
        trace_input_root_sha256: selected.expected[2].1.clone(),
        dependency_root_sha256,
        external_dependencies: EXTERNAL_DEPENDENCIES,
        final_graph_rows_written: false,
    })
}

/// Prepare exactly one descriptor-selected source-claims profile. This is
/// reversible private stage work, never a finished knowledge graph. Any
/// failure poisons the stage so the caller cannot seal it as complete.
pub fn prepare_source_claims(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: ClaimPrepareLimits,
) -> Result<ClaimPrepareReceipt> {
    let result = prepare_inner(stage, vocabulary, limits);
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

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const ROOT_NODES: &str = "4ca6bdc73b564bb2387ff4212a18b56926aef88cd0310a23e20e232cb2f19f63";
    const ROOT_EDGES: &str = "3758a9031190bd7aebdca082d209c43648b8fdc4e4216098d223c254908da6bc";
    const ROOT_TRACES: &str = "9c95cb9d5c1dd3096a9ef74663c4efdca2f8b643913ec1af512b97e72d9cb2fd";
    const ROOT_PREPARED: &str = "7a79841aec7e585e652551b8f88d2592e5ca0388295e2bdec569ff77d9bfcac8";
    const ROOT_PREPARED_EMPTY: &str =
        "928b87e272b3cd79364d459c83a706f841c10827efe5716904f0ed7f2ef4449c";
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
    fn limits() -> ClaimPrepareLimits {
        ClaimPrepareLimits {
            max_nodes: 16,
            max_edges: 16,
            max_claims: 16,
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
                source_graph_id: "fixture.claims".into(),
                owner_ref: "ToS/source-witnesses/AGENTS.md".into(),
                input_role: "bibliographic-claims".into(),
                adapter_profile: PROFILE.into(),
                representative_priority: 0,
            }],
            registered_source_ids: vec!["fixture.claims".into()],
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
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let c = "c".repeat(64);
        vec![
            (
                "nodes",
                "claim:tos.claim.one",
                format!(
                    r#"{{"node_id":"claim:tos.claim.one","node_kind":"claim","properties":{{"claim_ref":"tos.claim.one"}},"source_sha256":"{a}"}}"#
                ),
            ),
            (
                "nodes",
                "identity:tos.subject.one",
                format!(
                    r#"{{"node_id":"identity:tos.subject.one","node_kind":"identity","properties":{{}},"source_sha256":"{b}"}}"#
                ),
            ),
            (
                "nodes",
                "identity:tos.object.one",
                format!(
                    r#"{{"node_id":"identity:tos.object.one","node_kind":"identity","properties":{{}},"source_sha256":"{c}"}}"#
                ),
            ),
            (
                "edges",
                "edge:one",
                format!(
                    r#"{{"edge_id":"edge:one","claim_ref":"tos.claim.one","claim_sha256":"{a}","from_id":"claim:tos.claim.one","to_id":"identity:tos.subject.one","edge_kind":"has_subject"}}"#
                ),
            ),
            (
                "claim_traces",
                "tos.claim.one",
                format!(
                    r#"{{"claim_ref":"tos.claim.one","claim_node_id":"claim:tos.claim.one","subject_node_id":"identity:tos.subject.one","object_node_id":"identity:tos.object.one","predicate":"is_about","claim_sha256":"{a}","edge_ids":["edge:one"]}}"#
                ),
            ),
        ]
    }
    fn collection_root(rows: &[FixtureRow], collection: &str) -> (u64, String) {
        let mut selected: Vec<_> = rows
            .iter()
            .filter(|(name, _, _)| *name == collection)
            .collect();
        selected.sort_by_key(|(_, id, _)| *id);
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
                source_cut: "sealed-claim-cut".into(),
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
                    let (count, root) = collection_root(rows, collection);
                    InputCollectionReceipt {
                        source_graph: "fixture.claims".into(),
                        collection: (*collection).into(),
                        input_role: "bibliographic-claims".into(),
                        adapter_profile: PROFILE.into(),
                        expected_count: count,
                        expected_root_sha256: root,
                    }
                })
                .collect(),
        }
    }
    fn path(label: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-claim-prepare-{label}-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir.join("private.sqlite3")
    }
    fn run(rows: &[FixtureRow]) -> Result<ClaimPrepareReceipt> {
        let owner = Owner;
        let quota = TestQuota;
        let candidate = path("rows");
        let mut stage = KnowledgeStage::create(
            &candidate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 4096,
            },
            receipt(rows),
            &owner,
            &quota,
        )?;
        for (collection, id, payload) in rows {
            stage.ingest_input(InputRow {
                source_graph: "fixture.claims",
                collection,
                id,
                payload: payload.as_bytes(),
            })?;
        }
        let prepared = prepare_source_claims(&mut stage, &vocabulary(), limits());
        if prepared.is_ok() {
            let sealed = stage.finish()?;
            assert_eq!((sealed.node_rows, sealed.relation_rows), (0, 0));
            fs::remove_file(&candidate).unwrap();
        } else {
            drop(stage);
        }
        fs::remove_dir(candidate.parent().unwrap()).unwrap();
        prepared
    }

    // Frozen CPython hashlib oracle for exact raw JSON bytes. Python
    // `_bibliographic_relation_source` also gives `has_subject`, the catalog
    // fallback source_ref and the `bibliographic-claim` layer for this edge;
    // this prepared phase intentionally does not emit that final relation.
    #[test]
    fn python_oracle_prepared_closure() {
        let rows = fixture();
        assert_eq!(collection_root(&rows, "nodes").1, ROOT_NODES);
        assert_eq!(collection_root(&rows, "edges").1, ROOT_EDGES);
        assert_eq!(collection_root(&rows, "claim_traces").1, ROOT_TRACES);
        let prepared = run(&rows).unwrap();
        assert_eq!(
            (prepared.nodes, prepared.edges, prepared.claim_traces),
            (3, 1, 1)
        );
        assert_eq!(prepared.dependency_root_sha256, ROOT_PREPARED);
        assert_eq!(prepared.source_cut, "sealed-claim-cut");
        assert_eq!(prepared.source_graph, "fixture.claims");
        assert!(!prepared.final_graph_rows_written);
        assert_eq!(prepared.external_dependencies, EXTERNAL_DEPENDENCIES);
    }
    #[test]
    fn zero_rows_are_explicitly_covered() {
        let prepared = run(&[]).unwrap();
        assert_eq!(
            (prepared.nodes, prepared.edges, prepared.claim_traces),
            (0, 0, 0)
        );
        assert_eq!(prepared.node_input_root_sha256, EMPTY);
        assert_eq!(prepared.dependency_root_sha256, ROOT_PREPARED_EMPTY);
    }
    #[test]
    fn public_temporal_snapshot_normalizes_genuine_claim_and_context() {
        use crate::knowledge_normalization::stable_digest;
        use crate::knowledge_source_claims::{
            ClaimNormalizeLimits, ClaimNormalizer, claim_context_sources, claim_contexts,
            finalize_source_claims, materialize_source_claim_nodes, prepare_claim_context_groups,
        };
        let fixture: Value = serde_json::from_slice(include_bytes!(
            "../../../../access/tests/fixtures/knowledge-contract/temporal-jenseits-date.json"
        ))
        .unwrap();
        let descriptor = include_bytes!("../tests/fixtures/query-vocabulary.v1.json");
        let entity =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/entity-types.v1.json");
        let relation =
            include_bytes!("../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json");
        let vocabulary = QueryVocabulary::parse(
            descriptor,
            &[
                "philosophy-node-edge-v1",
                "canon-node-relation-v1",
                "candidate-relation-v1",
                "source-navigation-node-edge-v1",
                PROFILE,
                "declared-identity-and-source-ref-joins-v1",
                "repository-topology-v1",
                "indexed-node-edge-v1",
            ],
        )
        .unwrap();
        let registry = crate::KnowledgeRegistry::parse(entity, relation).unwrap();
        let rows: Vec<(String, String, Vec<u8>)> = COLLECTIONS
            .iter()
            .flat_map(|collection| {
                let key = match *collection {
                    "nodes" => "node_id",
                    "edges" => "edge_id",
                    _ => "claim_ref",
                };
                fixture[*collection]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(move |value| {
                        (
                            (*collection).to_owned(),
                            value[key].as_str().unwrap().to_owned(),
                            serde_json::to_vec(value).unwrap(),
                        )
                    })
            })
            .collect();
        let mut exact = receipt(&[]);
        exact.collections = COLLECTIONS
            .iter()
            .map(|collection| {
                let mut selected = rows
                    .iter()
                    .filter(|(name, _, _)| name == collection)
                    .collect::<Vec<_>>();
                selected.sort_by_key(|(_, id, _)| id);
                let mut hash = Digest256Hasher::new();
                for (_, id, bytes) in &selected {
                    root_item(&mut hash, id, Digest256::of_bytes(bytes).as_bytes());
                }
                InputCollectionReceipt {
                    source_graph: "source-claims".into(),
                    collection: (*collection).into(),
                    input_role: "bibliographic-claims".into(),
                    adapter_profile: PROFILE.into(),
                    expected_count: selected.len() as u64,
                    expected_root_sha256: hash.finalize().to_hex(),
                }
            })
            .collect();
        let owner = Owner;
        let quota = TestQuota;
        let candidate = path("native-temporal");
        let mut stage = KnowledgeStage::create(
            &candidate,
            StageLimits {
                sqlite: Limits::default(),
                max_temp_bytes: 64 * 1024 * 1024,
                max_seek_rows: 2,
                max_seek_bytes: 128 * 1024,
            },
            exact,
            &owner,
            &quota,
        )
        .unwrap();
        for (collection, id, payload) in &rows {
            stage
                .ingest_input(InputRow {
                    source_graph: "source-claims",
                    collection,
                    id,
                    payload,
                })
                .unwrap();
        }
        let prepared = prepare_source_claims(
            &mut stage,
            &vocabulary,
            ClaimPrepareLimits {
                max_nodes: 16,
                max_edges: 16,
                max_claims: 4,
                max_page_rows: 2,
                max_row_bytes: 65536,
                max_work_bytes: 2 * 1024 * 1024,
            },
        )
        .unwrap();
        let normalize_limits = ClaimNormalizeLimits {
            max_raw_bytes: 65536,
            max_output_bytes: 262144,
            max_page_rows: 2,
            max_contexts: 16,
            max_work_bytes: 8 * 1024 * 1024,
        };
        let normalizer = ClaimNormalizer::new(
            &registry,
            entity,
            relation,
            &vocabulary,
            descriptor,
            normalize_limits,
        )
        .unwrap();
        assert_eq!(
            materialize_source_claim_nodes(&mut stage, &prepared, &normalizer).unwrap(),
            6
        );
        let mut normalized = std::collections::BTreeMap::<String, Value>::new();
        for (_, id, _) in rows
            .iter()
            .filter(|(collection, _, _)| collection == "nodes")
        {
            let bytes: Vec<u8> = stage
                .with_connection(WritePhase::Sort, |db| {
                    Ok(db.query_row(
                        "SELECT payload FROM knowledge_nodes WHERE native_id=?1",
                        [id],
                        |r| r.get(0),
                    )?)
                })
                .unwrap();
            normalized.insert(id.clone(), serde_json::from_slice(&bytes).unwrap());
        }
        // Frozen bounded Python _normalize_node oracle over the exact public
        // transport snapshot. Its historical navigation descriptor is retained;
        // this does not make a new source/registry admission decision.
        let trace = &fixture["claim_traces"][0];
        let claim_id = trace["claim_node_id"].as_str().unwrap();
        let object_id = trace["object_node_id"].as_str().unwrap();
        assert_eq!(
            stable_digest(&normalized[claim_id]).unwrap(),
            "a9bdecfdca1fde7c266997bae2fe460f9570d1e8e05f8c0e736f3cd236b423e9"
        );
        assert_eq!(
            stable_digest(&normalized[object_id]).unwrap(),
            "fffff9d21a3aaf598a163532ed5d2405db7f379fe1146a5e9d4c59201c03b1ba"
        );
        let contexts = prepare_claim_context_groups(&mut stage, normalize_limits).unwrap();
        let reference = trace["claim_ref"].as_str().unwrap();
        let group = claim_contexts(
            &mut stage,
            &contexts,
            "source-claims",
            reference,
            normalize_limits,
        )
        .unwrap();
        assert_eq!(
            stable_digest(&serde_json::json!(group)).unwrap(),
            "40cc3e37630f6a2bc69db68c6b133a64fdba0b360309124c24222ae678dfc28a"
        );
        let witnesses = claim_context_sources(
            &mut stage,
            &contexts,
            "source-claims",
            reference,
            normalize_limits,
        )
        .unwrap();
        assert_eq!(witnesses.len(), 1);
        let mut expected_source = fixture["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["node_id"].as_str() == Some(claim_id))
            .unwrap()
            .clone();
        let mut expected_layers = expected_source
            .get("graph_layers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        expected_layers.insert("bibliographic-claim".into());
        expected_source["graph_layers"] = serde_json::json!(expected_layers);
        assert_eq!(
            serde_json::from_slice::<Value>(&witnesses[0]).unwrap(),
            expected_source,
            "ordered witness must retain the complete Python bibliography source transform"
        );
        assert_eq!(
            finalize_source_claims(&mut stage, &prepared, &normalizer, &contexts).unwrap(),
            1
        );
        let final_bytes: Vec<u8> = stage
            .with_connection(WritePhase::Sort, |db| {
                Ok(db.query_row(
                    "SELECT payload FROM knowledge_nodes WHERE native_id=?1",
                    [claim_id],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        let final_claim: Value = serde_json::from_slice(&final_bytes).unwrap();
        assert_eq!(
            stable_digest(&final_claim["semantics"]["claim"]).unwrap(),
            "e1482ee857ecf29770bd002ab31a699ad1ac711549730e8a42eea486eff2f4b9"
        );
        assert_ne!(normalized[object_id]["entity_id"], final_claim["entity_id"]);
        let expected = [
            (
                "generated_by",
                "344919ee0e0a9bb31740d07bea36659a9db597bf3a6edfd8457b82d939050949",
            ),
            (
                "has_object",
                "c9eff963f5eefb80fd23304628e06ea19d782dc57502cd0705dcc1ae1b18ec68",
            ),
            (
                "has_subject",
                "3f5537c3a5e74a52126ebd3096912f867f775fafa991708f0c6c6e0109c6e64c",
            ),
            (
                "made_by",
                "d72b3933118ae2764ae12d0a612083251976a41a0d8f7b11f570339553aea019",
            ),
            (
                "supported_by",
                "b97252cba8a02c07213c3b58e1801116e8fa54095d9f75392f01cb7b7890a403",
            ),
        ];
        for edge in fixture["edges"].as_array().unwrap() {
            let raw = stage
                .raw_by_id("source-claims", "edges", edge["edge_id"].as_str().unwrap())
                .unwrap()
                .unwrap();
            let target_id = edge["to_id"].as_str().unwrap();
            let target = fixture["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["node_id"] == target_id)
                .unwrap();
            let object = fixture["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["node_id"] == object_id)
                .unwrap();
            let output = normalizer
                .normalize_base_relation(
                    &raw,
                    &prepared,
                    object,
                    target,
                    &normalized[edge["from_id"].as_str().unwrap()]["display"]["title"],
                    &normalized[target_id]["display"]["title"],
                    &group,
                )
                .unwrap();
            let digest = expected
                .iter()
                .find(|(kind, _)| *kind == edge["edge_kind"].as_str().unwrap())
                .unwrap()
                .1;
            assert_eq!(stable_digest(&output).unwrap(), digest);
        }
        drop(stage);
        fs::remove_dir(candidate.parent().unwrap()).unwrap();
    }
    #[test]
    fn missing_duplicate_and_ref_mismatch_refuse() {
        let mut missing = fixture();
        missing.retain(|(_, id, _)| *id != "identity:tos.subject.one");
        assert!(run(&missing).is_err());
        let mut wrong = fixture();
        let edge = wrong
            .iter_mut()
            .find(|(_, id, _)| *id == "edge:one")
            .unwrap();
        edge.2 = edge.2.replace(
            "\"claim_ref\":\"tos.claim.one\"",
            "\"claim_ref\":\"tos.claim.other\"",
        );
        assert!(run(&wrong).is_err());
        let mut duplicate = fixture();
        let trace = duplicate
            .iter_mut()
            .find(|(_, id, _)| *id == "tos.claim.one")
            .unwrap();
        trace.2 = trace
            .2
            .replace("[\"edge:one\"]", "[\"edge:one\",\"edge:one\"]");
        assert!(run(&duplicate).is_err());
    }
}
