//! Complete registered-source scope over one private normalized stage.
//! These hashes describe derived rows. Input completeness still comes from
//! the independent sealed owner receipt checked by KnowledgeStage::finish.

use crate::{
    Error, QueryVocabulary, Result,
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet};
use tos_foundation::Digest256Hasher;

#[derive(Clone, Copy, Debug)]
pub struct ScopeLimits {
    pub max_sources: usize,
    pub max_rows: u64,
    pub max_index_work_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ScopeReceipt {
    pub source_count: usize,
    pub node_count: u64,
    pub relation_count: u64,
    pub source_scope_root_sha256: String,
}

struct Counts {
    input_role: String,
    adapter_profile: String,
    nodes: u64,
    relations: u64,
    node_hash: Digest256Hasher,
    relation_hash: Digest256Hasher,
}

fn hash_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}
fn hash_text(hash: &mut Digest256Hasher, text: &str) {
    hash.update(&(text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}

fn scan(
    db: &Connection,
    table: &'static str,
    states: &mut BTreeMap<String, Counts>,
    limits: ScopeLimits,
    work: &mut u64,
) -> Result<u64> {
    let sql = match table {
        "knowledge_nodes" => {
            "SELECT source_graph,id,source_order,payload_len,payload_sha256
            FROM knowledge_nodes ORDER BY source_order"
        }
        "knowledge_relations" => {
            "SELECT source_graph,id,source_order,payload_len,payload_sha256
            FROM knowledge_relations ORDER BY source_order"
        }
        _ => return Err(Error::Invalid("knowledge scope table")),
    };
    let mut statement = db.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut total = 0u64;
    let mut previous: Option<(String, String)> = None;
    while let Some(row) = rows.next()? {
        let source: String = row.get(0)?;
        let id: String = row.get(1)?;
        let order: i64 = row.get(2)?;
        let payload_len: i64 = row.get(3)?;
        let digest: Vec<u8> = row.get(4)?;
        if id.is_empty()
            || source.is_empty()
            || digest.len() != 32
            || payload_len < 0
            || order < 0
            || order as u64 != total
        {
            return Err(Error::Invalid("knowledge scope row metadata"));
        }
        if previous
            .as_ref()
            .is_some_and(|old| old >= &(source.clone(), id.clone()))
        {
            return Err(Error::Invalid("knowledge scope row order"));
        }
        previous = Some((source.clone(), id.clone()));
        *work = work
            .checked_add((source.len() + id.len() + digest.len() + 24) as u64)
            .ok_or(Error::Budget("knowledge scope work bytes"))?;
        if *work > limits.max_index_work_bytes {
            return Err(Error::Budget("knowledge scope work bytes"));
        }
        let state = states
            .get_mut(&source)
            .ok_or(Error::Invalid("unregistered knowledge output source"))?;
        if table == "knowledge_nodes" {
            hash_item(&mut state.node_hash, &id, &digest);
            state.nodes = state
                .nodes
                .checked_add(1)
                .ok_or(Error::Budget("scope nodes"))?;
        } else {
            hash_item(&mut state.relation_hash, &id, &digest);
            state.relations = state
                .relations
                .checked_add(1)
                .ok_or(Error::Budget("scope relations"))?;
        }
        total = total.checked_add(1).ok_or(Error::Budget("scope rows"))?;
        if total > limits.max_rows {
            return Err(Error::Budget("scope rows"));
        }
    }
    Ok(total)
}

/// Call after all source adapters and before KnowledgeStage::finish. The
/// selected opener must recheck this table/root against the owner receipt;
/// this function alone never claims source admission or publication.
pub fn write_source_scope(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: ScopeLimits,
) -> Result<ScopeReceipt> {
    let result = write_source_scope_inner(stage, vocabulary, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

fn write_source_scope_inner(
    stage: &mut KnowledgeStage<'_>,
    vocabulary: &QueryVocabulary,
    limits: ScopeLimits,
) -> Result<ScopeReceipt> {
    if limits.max_sources == 0
        || limits.max_rows == 0
        || limits.max_index_work_bytes == 0
        || vocabulary.sources.is_empty()
        || vocabulary.sources.len() > limits.max_sources
    {
        return Err(Error::Budget("knowledge scope limits"));
    }
    let mut states = BTreeMap::new();
    for source in &vocabulary.sources {
        if states
            .insert(
                source.source_graph_id.clone(),
                Counts {
                    input_role: source.input_role.clone(),
                    adapter_profile: source.adapter_profile.clone(),
                    nodes: 0,
                    relations: 0,
                    node_hash: Digest256Hasher::new(),
                    relation_hash: Digest256Hasher::new(),
                },
            )
            .is_some()
        {
            return Err(Error::Invalid("duplicate knowledge source registration"));
        }
    }
    let mut covered = BTreeSet::new();
    for input in &stage.exact_receipt().collections {
        let source = states
            .get(&input.source_graph)
            .ok_or(Error::Invalid("unregistered knowledge input source"))?;
        if input.input_role != source.input_role || input.adapter_profile != source.adapter_profile
        {
            return Err(Error::Invalid("knowledge input adapter mismatch"));
        }
        covered.insert(input.source_graph.as_str());
    }
    if covered.len() != states.len() {
        return Err(Error::Invalid("knowledge source input omitted"));
    }
    stage.with_connection(WritePhase::Sort, |db| {
        let mut work = 0u64;
        let node_count = scan(db, "knowledge_nodes", &mut states, limits, &mut work)?;
        let relation_count = scan(db, "knowledge_relations", &mut states, limits, &mut work)?;
        if node_count
            .checked_add(relation_count)
            .ok_or(Error::Budget("scope rows"))?
            > limits.max_rows
        {
            return Err(Error::Budget("scope rows"));
        }
        let tx = db.transaction()?;
        tx.execute_batch(
            "CREATE TABLE source_scope(
          source_graph TEXT PRIMARY KEY,input_role TEXT NOT NULL,adapter_profile TEXT NOT NULL,
          expected_node_count INTEGER NOT NULL,expected_relation_count INTEGER NOT NULL,
          node_root_sha256 BLOB NOT NULL,relation_root_sha256 BLOB NOT NULL
        ) WITHOUT ROWID",
        )?;
        let mut root = Digest256Hasher::new();
        for (source, counts) in states {
            let node_digest = counts.node_hash.finalize();
            let relation_digest = counts.relation_hash.finalize();
            let nodes =
                i64::try_from(counts.nodes).map_err(|_| Error::Budget("source node count"))?;
            let relations = i64::try_from(counts.relations)
                .map_err(|_| Error::Budget("source relation count"))?;
            tx.execute(
                "INSERT INTO source_scope VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    &source,
                    &counts.input_role,
                    &counts.adapter_profile,
                    nodes,
                    relations,
                    &node_digest.as_bytes()[..],
                    &relation_digest.as_bytes()[..],
                ],
            )?;
            hash_text(&mut root, &source);
            hash_text(&mut root, &counts.input_role);
            hash_text(&mut root, &counts.adapter_profile);
            root.update(&counts.nodes.to_be_bytes());
            root.update(&counts.relations.to_be_bytes());
            root.update(node_digest.as_bytes());
            root.update(relation_digest.as_bytes());
        }
        tx.commit()?;
        Ok(ScopeReceipt {
            source_count: vocabulary.sources.len(),
            node_count,
            relation_count,
            source_scope_root_sha256: root.finalize().to_hex(),
        })
    })
}
