//! Complete registered-source scope over one private normalized stage.
//! These hashes describe derived rows. Input completeness still comes from
//! the independent sealed owner receipt checked by KnowledgeStage::finish.

use crate::{
    Error, QueryVocabulary, Result,
    d1_public_capture::CreationState,
    knowledge_stage::{KnowledgeStage, WritePhase},
};
use rusqlite::{Connection, params};
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

fn account_work(
    work: &mut u64,
    limits: ScopeLimits,
    bytes: usize,
    creation: Option<&CreationState<'_>>,
) -> Result<()> {
    let next = work
        .checked_add(u64::try_from(bytes).map_err(|_| Error::Budget("knowledge scope work bytes"))?)
        .ok_or(Error::Budget("knowledge scope work bytes"))?;
    if next > limits.max_index_work_bytes {
        return Err(Error::Budget("knowledge scope work bytes"));
    }
    if let Some(creation) = creation {
        creation.charge_work(bytes)?;
    }
    *work = next;
    Ok(())
}

fn state_index(
    states: &[(String, Counts)],
    source: &str,
    work: &mut u64,
    limits: ScopeLimits,
    creation: Option<&CreationState<'_>>,
) -> Result<usize> {
    let levels = usize::BITS as usize - states.len().max(1).leading_zeros() as usize;
    let lookup_work = levels
        .checked_mul(
            source
                .len()
                .max(1)
                .checked_add(std::mem::size_of::<usize>())
                .ok_or(Error::Budget("knowledge scope lookup work"))?,
        )
        .ok_or(Error::Budget("knowledge scope lookup work"))?;
    account_work(work, limits, lookup_work, creation)?;
    states
        .binary_search_by(|(candidate, _)| candidate.as_str().cmp(source))
        .map_err(|_| Error::Invalid("unregistered knowledge output source"))
}

fn scan(
    db: &Connection,
    table: &'static str,
    states: &mut [(String, Counts)],
    limits: ScopeLimits,
    work: &mut u64,
    creation: Option<&CreationState<'_>>,
) -> Result<u64> {
    let sql = match table {
        "knowledge_nodes" => {
            "SELECT source_graph,id,source_order,payload_len,payload_sha256,
                    length(CAST(source_graph AS BLOB)),length(CAST(id AS BLOB)),length(payload_sha256)
            FROM knowledge_nodes ORDER BY source_order"
        }
        "knowledge_relations" => {
            "SELECT source_graph,id,source_order,payload_len,payload_sha256,
                    length(CAST(source_graph AS BLOB)),length(CAST(id AS BLOB)),length(payload_sha256)
            FROM knowledge_relations ORDER BY source_order"
        }
        _ => return Err(Error::Invalid("knowledge scope table")),
    };
    let mut statement = db.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut total = 0u64;
    let mut previous: Option<(String, String)> = None;
    let mut previous_hold = None;
    while let Some(row) = rows.next()? {
        // SQLite reports these lengths without transferring any row-owned
        // strings or digest bytes. Admit both the live row and the ordering
        // key before `get` creates those owners.
        let source_len: i64 = row.get(5)?;
        let id_len: i64 = row.get(6)?;
        let digest_len: i64 = row.get(7)?;
        if source_len < 0 || id_len < 0 || digest_len < 0 {
            return Err(Error::Invalid("knowledge scope row lengths"));
        }
        let source_bytes = usize::try_from(source_len)
            .ok()
            .ok_or(Error::Budget("knowledge scope row allocation"))?;
        let id_bytes =
            usize::try_from(id_len).map_err(|_| Error::Budget("knowledge scope row allocation"))?;
        let digest_bytes = usize::try_from(digest_len)
            .map_err(|_| Error::Budget("knowledge scope row allocation"))?;
        let row_bytes = source_bytes
            .checked_add(id_bytes)
            .and_then(|n| n.checked_add(digest_bytes))
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, String, Vec<u8>)>() + 48))
            .ok_or(Error::Budget("knowledge scope row allocation"))?;
        let row_work = source_bytes
            .checked_add(id_bytes)
            .and_then(|n| n.checked_add(digest_bytes))
            .and_then(|n| n.checked_add(24))
            .ok_or(Error::Budget("knowledge scope work bytes"))?;
        account_work(work, limits, row_work, creation)?;
        let _row_hold = if let Some(creation) = creation {
            Some(creation.hold(row_bytes)?)
        } else {
            None
        };
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
            .is_some_and(|old| (old.0.as_str(), old.1.as_str()) >= (source.as_str(), id.as_str()))
        {
            return Err(Error::Invalid("knowledge scope row order"));
        }
        let next_previous_hold = if let Some(creation) = creation {
            let previous_bytes = source
                .len()
                .checked_add(id.len())
                .and_then(|n| n.checked_add(std::mem::size_of::<(String, String)>()))
                .and_then(|n| n.checked_add(32))
                .ok_or(Error::Budget("knowledge scope previous row"))?;
            creation.charge_work(source.len().saturating_add(id.len()))?;
            Some(creation.hold(previous_bytes)?)
        } else {
            None
        };
        previous = Some((source.clone(), id.clone()));
        previous_hold = next_previous_hold;
        let _ = &previous_hold;
        let index = state_index(states, &source, work, limits, creation)?;
        let state = &mut states[index].1;
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
    let creation = stage.owned_creation_state();
    let mut source_bytes = vocabulary
        .sources
        .len()
        .checked_mul(std::mem::size_of::<(String, Counts)>())
        .ok_or(Error::Budget("knowledge scope source rows"))?;
    for source in &vocabulary.sources {
        source_bytes = source_bytes
            .checked_add(source.source_graph_id.len())
            .and_then(|n| n.checked_add(source.input_role.len()))
            .and_then(|n| n.checked_add(source.adapter_profile.len()))
            .and_then(|n| n.checked_add(2 * std::mem::size_of::<String>()))
            .ok_or(Error::Budget("knowledge scope source map"))?;
    }
    let _states_hold = creation.map(|owner| owner.hold(source_bytes)).transpose()?;
    if let Some(creation) = creation {
        creation.charge_work(source_bytes)?;
    }
    let mut states = Vec::new();
    states
        .try_reserve_exact(vocabulary.sources.len())
        .map_err(|_| Error::Budget("knowledge scope source rows"))?;
    for source in &vocabulary.sources {
        states.push((
            source.source_graph_id.clone(),
            Counts {
                input_role: source.input_role.clone(),
                adapter_profile: source.adapter_profile.clone(),
                nodes: 0,
                relations: 0,
                node_hash: Digest256Hasher::new(),
                relation_hash: Digest256Hasher::new(),
            },
        ));
    }
    let sort_levels = usize::BITS as usize - states.len().max(1).leading_zeros() as usize;
    let max_source_bytes = states
        .iter()
        .map(|(source, _)| source.len())
        .max()
        .unwrap_or(0);
    let sort_work = states
        .len()
        .checked_mul(sort_levels)
        .and_then(|n| n.checked_mul(max_source_bytes))
        .ok_or(Error::Budget("knowledge scope source sort work"))?;
    if let Some(creation) = creation {
        creation.charge_work(sort_work)?;
    }
    states.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    if states.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::Invalid("duplicate knowledge source registration"));
    }
    let covered_bytes = states.len();
    let _covered_hold = creation
        .map(|owner| owner.hold(covered_bytes))
        .transpose()?;
    if let Some(creation) = creation {
        creation.charge_work(covered_bytes)?;
    }
    let mut work = 0u64;
    let mut covered = Vec::new();
    covered
        .try_reserve_exact(states.len())
        .map_err(|_| Error::Budget("knowledge scope covered sources"))?;
    covered.resize(states.len(), 0u8);
    for input in &stage.exact_receipt()?.collections {
        let source_index = state_index(&states, &input.source_graph, &mut work, limits, creation)?;
        let source = &states[source_index].1;
        if let Some(creation) = creation {
            creation.charge_work(
                input
                    .input_role
                    .len()
                    .checked_add(input.adapter_profile.len())
                    .ok_or(Error::Budget("knowledge scope adapter work"))?,
            )?;
        }
        if input.input_role != source.input_role || input.adapter_profile != source.adapter_profile
        {
            return Err(Error::Invalid("knowledge input adapter mismatch"));
        }
        if covered[source_index] == 0 {
            account_work(&mut work, limits, input.source_graph.len(), creation)?;
            covered[source_index] = 1;
        }
    }
    account_work(&mut work, limits, covered.len(), creation)?;
    if covered.iter().filter(|source| **source != 0).count() != states.len() {
        return Err(Error::Invalid("knowledge source input omitted"));
    }
    stage.with_connection(WritePhase::Sort, |db| {
        let node_count = scan(
            db,
            "knowledge_nodes",
            &mut states,
            limits,
            &mut work,
            creation,
        )?;
        let relation_count = scan(
            db,
            "knowledge_relations",
            &mut states,
            limits,
            &mut work,
            creation,
        )?;
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
            if let Some(creation) = creation {
                let root_work = source
                    .len()
                    .checked_add(counts.input_role.len())
                    .and_then(|n| n.checked_add(counts.adapter_profile.len()))
                    .and_then(|n| n.checked_add(64))
                    .ok_or(Error::Budget("knowledge scope root work"))?;
                creation.charge_work(root_work)?;
            }
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
        if let Some(creation) = creation {
            creation.retain(64)?;
            creation.charge_work(64)?;
        }
        let source_scope_root_sha256 = root.finalize().to_hex();
        Ok(ScopeReceipt {
            source_count: vocabulary.sources.len(),
            node_count,
            relation_count,
            source_scope_root_sha256,
        })
    })
}
