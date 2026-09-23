//! Bounded finalization of source-navigation base nodes after the complete
//! all-source relation pass has produced inherited endpoint views.
//!
//! Readable context is attached by the following owner-registry phase. This
//! phase does not seal a graph or claim that every source family is present.

use crate::knowledge_inherited_views::{InheritedViewReceipt, endpoint_inherited_views};
use crate::knowledge_normalization::{SourceRow, stamp_content_revision};
use crate::knowledge_source_navigation_prepare::NavigationPrepareReceipt;
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{Error, Result};
use rusqlite::params;
use serde_json::Value;
use std::collections::BTreeSet;
use tos_foundation::Digest256;

#[derive(Clone, Copy, Debug)]
pub struct NavigationFinalizeLimits {
    pub max_nodes: u64,
    pub max_page_rows: usize,
    pub max_page_bytes: usize,
    pub max_row_bytes: usize,
    pub max_view_ids_per_node: usize,
    pub max_work_bytes: u64,
}

impl NavigationFinalizeLimits {
    fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self
                .max_page_rows
                .checked_mul(self.max_row_bytes)
                .is_none_or(|bytes| bytes > self.max_page_bytes)
            || self.max_view_ids_per_node == 0
            || self.max_view_ids_per_node > 4096
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("navigation finalization limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct NavigationInheritedReceipt {
    pub source_cut: String,
    pub source_graph: String,
    pub base_node_count: u64,
    pub base_node_root_sha256: String,
    pub relation_root_sha256: String,
    pub updated_nodes: u64,
    pub updated_root_sha256: String,
}

struct Node {
    id: String,
    native_id: String,
    raw_present: bool,
    payload: Vec<u8>,
    sha: Vec<u8>,
}

fn page(
    stage: &mut KnowledgeStage<'_>,
    source: &str,
    after: &str,
    limits: NavigationFinalizeLimits,
) -> Result<Vec<Node>> {
    stage.with_connection(WritePhase::Sort, |db| {
        let mut stmt = db.prepare(
            "SELECT n.id,n.native_id,r.id IS NOT NULL, \
             CASE WHEN typeof(n.payload)='blob' AND n.payload_len=length(n.payload) \
             AND length(n.payload)<=?3 THEN n.payload ELSE NULL END, \
             CASE WHEN typeof(n.payload_sha256)='blob' AND length(n.payload_sha256)=32 \
             THEN n.payload_sha256 ELSE NULL END \
             FROM knowledge_nodes AS n LEFT JOIN raw_records AS r \
             ON r.source_graph=n.source_graph AND r.collection='nodes' AND r.id=n.native_id \
             WHERE n.source_graph=?1 AND n.id>?2 ORDER BY n.id LIMIT ?4",
        )?;
        let mut rows = stmt.query(params![
            source,
            after,
            limits.max_row_bytes as i64,
            limits.max_page_rows as i64
        ])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(Node {
                id: row.get(0)?,
                native_id: row
                    .get::<_, Option<String>>(1)?
                    .ok_or(Error::Invalid("navigation finalization native ID"))?,
                raw_present: row.get::<_, i64>(2)? == 1,
                payload: row
                    .get::<_, Option<Vec<u8>>>(3)?
                    .ok_or(Error::Budget("navigation finalization row bytes"))?,
                sha: row
                    .get::<_, Option<Vec<u8>>>(4)?
                    .ok_or(Error::Invalid("navigation finalization digest"))?,
            });
        }
        Ok(out)
    })
}

fn apply_views(value: &mut Value, inherited: &[String], max_bytes: usize) -> Result<bool> {
    let original = value
        .get("view_ids")
        .and_then(Value::as_array)
        .ok_or(Error::Invalid("navigation base view IDs"))?;
    let mut views = BTreeSet::new();
    for entry in original {
        if let Some(text) = entry.as_str().filter(|text| !text.is_empty()) {
            views.insert(text.to_owned());
        }
    }
    views.extend(inherited.iter().cloned());
    let updated = views.into_iter().map(Value::String).collect::<Vec<_>>();
    if *original == updated {
        return Ok(false);
    }
    value["view_ids"] = Value::Array(updated);
    stamp_content_revision(value, max_bytes)?;
    Ok(true)
}

fn apply_inner(
    stage: &mut KnowledgeStage<'_>,
    prepared: &NavigationPrepareReceipt,
    inherited: &InheritedViewReceipt,
    expected_base_nodes: u64,
    expected_base_node_root: &str,
    limits: NavigationFinalizeLimits,
) -> Result<NavigationInheritedReceipt> {
    limits.validate()?;
    if prepared.final_graph_rows_written
        || inherited.final_graph_rows_written
        || prepared.source_cut != inherited.source_cut
        || expected_base_nodes == 0
        || expected_base_nodes > limits.max_nodes
        || Digest256::from_hex(expected_base_node_root).is_err()
        || Digest256::from_hex(&inherited.relation_root_sha256).is_err()
    {
        return Err(Error::Invalid("navigation finalization cut/root"));
    }
    let roots = stage.core_roots()?;
    if roots.nodes != expected_base_nodes
        || roots.node_sha256 != expected_base_node_root
        || roots.relation_sha256 != inherited.relation_root_sha256
        || roots.relations != inherited.relation_count
    {
        return Err(Error::Invalid("navigation finalization base closure"));
    }
    let mut after = String::new();
    let mut seen = 0u64;
    let mut updated = 0u64;
    let mut work = 0u64;
    loop {
        let batch = page(stage, &prepared.source_graph, &after, limits)?;
        if batch.is_empty() {
            break;
        }
        for node in batch {
            if node.id <= after
                || !node.raw_present
                || node.id != format!("{}:{}", prepared.source_graph, node.native_id)
                || node.sha.len() != 32
                || Digest256::of_bytes(&node.payload).as_bytes().as_slice() != node.sha.as_slice()
            {
                return Err(Error::Invalid("navigation finalization base carrier"));
            }
            work = work
                .checked_add(node.payload.len() as u64)
                .ok_or(Error::Budget("navigation finalization work"))?;
            seen = seen
                .checked_add(1)
                .ok_or(Error::Budget("navigation finalization nodes"))?;
            if seen > limits.max_nodes || work > limits.max_work_bytes {
                return Err(Error::Budget("navigation finalization scan"));
            }
            let parsed = SourceRow::parse(&node.payload, limits.max_row_bytes)?;
            let mut value = parsed.value().clone();
            if value.get("id").and_then(Value::as_str) != Some(node.id.as_str())
                || value.get("source_graph").and_then(Value::as_str)
                    != Some(prepared.source_graph.as_str())
                || value.get("native_id").and_then(Value::as_str) != Some(node.native_id.as_str())
            {
                return Err(Error::Invalid("navigation finalization row binding"));
            }
            let views = endpoint_inherited_views(stage, &node.id, limits.max_view_ids_per_node)?;
            if apply_views(&mut value, &views, limits.max_row_bytes)? {
                let bytes = serde_json::to_vec(&value)
                    .map_err(|_| Error::Invalid("navigation finalization JSON"))?;
                if bytes.len() > limits.max_row_bytes {
                    return Err(Error::Budget("navigation finalization output bytes"));
                }
                work = work
                    .checked_add(bytes.len() as u64)
                    .ok_or(Error::Budget("navigation finalization work"))?;
                if work > limits.max_work_bytes {
                    return Err(Error::Budget("navigation finalization work"));
                }
                stage.charge_materialized(1, bytes.len() as u64)?;
                let digest = Digest256::of_bytes(&bytes);
                let changed = stage.with_connection(WritePhase::Finalize, |db| {
                    Ok(db.execute("UPDATE knowledge_nodes SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND source_graph=?5 AND payload_sha256=?6",
                        params![bytes.len() as i64, digest.as_bytes().as_slice(), bytes, node.id, prepared.source_graph, node.sha])?)
                })?;
                if changed != 1 {
                    return Err(Error::Invalid("navigation finalization concurrent change"));
                }
                updated += 1;
            }
            after = node.id;
        }
    }
    if seen != prepared.nodes {
        // An all-source placeholder for a missing navigation endpoint needs a
        // distinct, explicitly sealed producer lane before this pass can run.
        return Err(Error::Invalid("navigation finalization source coverage"));
    }
    let new_roots = stage.core_roots()?;
    if new_roots.nodes != roots.nodes
        || new_roots.relations != roots.relations
        || new_roots.relation_sha256 != roots.relation_sha256
    {
        return Err(Error::Invalid("navigation finalization root drift"));
    }
    Ok(NavigationInheritedReceipt {
        source_cut: prepared.source_cut.clone(),
        source_graph: prepared.source_graph.clone(),
        base_node_count: roots.nodes,
        base_node_root_sha256: roots.node_sha256,
        relation_root_sha256: roots.relation_sha256,
        updated_nodes: updated,
        updated_root_sha256: new_roots.node_sha256,
    })
}

/// Apply Python `_apply_final_node_changes` inherited-view union to exact
/// source-navigation base rows. The caller must attach readable context and
/// seal final revisions before selected publication.
pub fn apply_navigation_inherited_views(
    stage: &mut KnowledgeStage<'_>,
    prepared: &NavigationPrepareReceipt,
    inherited: &InheritedViewReceipt,
    expected_base_nodes: u64,
    expected_base_node_root: &str,
    limits: NavigationFinalizeLimits,
) -> Result<NavigationInheritedReceipt> {
    let result = apply_inner(
        stage,
        prepared,
        inherited,
        expected_base_nodes,
        expected_base_node_root,
        limits,
    );
    if result.is_err() {
        stage.poison();
    }
    result
}
