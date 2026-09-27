//! Final native graph pass after all source nodes, Claims and relations exist.
//! Global inheritance precedes readable context and final revision stamping.
//! Source witnesses stay private until this pass has consumed them.

use crate::knowledge_inherited_views::{InheritedViewReceipt, endpoint_inherited_views};
use crate::knowledge_normalization::{SourceRow, stamp_content_revision};
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{
    Error, KnowledgeRegistry, ReadableContextCarrier, ReadableContextCompiler,
    ReadableContextLimits, Result, ordered_readable_witness,
};
use rusqlite::params;
use serde_json::Value;
use std::collections::BTreeSet;
use tos_foundation::Digest256;

#[derive(Clone, Copy, Debug)]
pub struct NativeFinalizeLimits {
    pub max_rows: u64,
    pub max_page_rows: usize,
    pub max_page_bytes: usize,
    pub max_row_bytes: usize,
    pub max_view_ids_per_node: usize,
    pub max_context_sources: usize,
    pub max_work_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct NativeFinalizeReceipt {
    pub source_cut: String,
    pub nodes: u64,
    pub relations: u64,
    pub readable_rows: u64,
    pub node_root_sha256: String,
    pub relation_root_sha256: String,
}

struct Row {
    id: String,
    source: String,
    native: Option<String>,
    order: i64,
    payload: Vec<u8>,
    sha: Vec<u8>,
}

fn page(
    stage: &mut KnowledgeStage<'_>,
    table: &str,
    after: i64,
    limits: NativeFinalizeLimits,
) -> Result<Vec<Row>> {
    stage.with_connection(WritePhase::Finalize, |db| {
        let sql = format!("SELECT id,source_graph,native_id,source_order,
            CASE WHEN typeof(payload)='blob' AND payload_len=length(payload) AND length(payload)<=?2 THEN payload ELSE NULL END,
            payload_sha256 FROM {table} WHERE source_order>?1 ORDER BY source_order LIMIT ?3");
        let mut statement = db.prepare(&sql)?;
        let mut rows = statement.query(params![after, limits.max_row_bytes as i64, limits.max_page_rows as i64])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(Row {id:row.get(0)?,source:row.get(1)?,native:row.get(2)?,order:row.get(3)?,
                payload:row.get::<_,Option<Vec<u8>>>(4)?.ok_or(Error::Budget("native final carrier bytes"))?,sha:row.get(5)?});
        }
        Ok(out)
    })
}

fn has_context(value: &Value) -> bool {
    value
        .pointer("/attributes/source_record")
        .is_some_and(|v| !v.is_null())
        || value
            .pointer("/attributes/source_claim")
            .is_some_and(|v| !v.is_null())
        || value
            .pointer("/attributes/human_forms")
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty())
        || value
            .pointer("/semantics/assertion_contexts")
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty())
}

/// The callback returns exact ordered source witnesses for one referenced
/// Claim group. It must use the same complete source-cut group receipt used
/// by relation normalization, not a newer source or inferred Claim content.
pub fn finalize_native_graph_rows<F>(
    stage: &mut KnowledgeStage<'_>,
    registry: &KnowledgeRegistry,
    entity_registry_bytes: &[u8],
    inherited: &InheritedViewReceipt,
    limits: NativeFinalizeLimits,
    claim_sources: F,
) -> Result<NativeFinalizeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<Vec<Vec<u8>>>,
{
    finalize_native_graph_rows_with_witnesses(
        stage,
        registry,
        entity_registry_bytes,
        inherited,
        limits,
        claim_sources,
        |_, _, _, _, _| Ok(None),
    )
}

/// Maintained synthesized families return their exact prepared source witness
/// by normalized identity. A raw lookup by native ID cannot identify canon
/// pack edges, node-local assertions or repository identity overrides.
pub fn finalize_native_graph_rows_with_witnesses<F, G>(
    stage: &mut KnowledgeStage<'_>,
    registry: &KnowledgeRegistry,
    entity_registry_bytes: &[u8],
    inherited: &InheritedViewReceipt,
    limits: NativeFinalizeLimits,
    mut claim_sources: F,
    mut source_material: G,
) -> Result<NativeFinalizeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<Vec<Vec<u8>>>,
    G: FnMut(&mut KnowledgeStage<'_>, bool, &str, &str, &str) -> Result<Option<Vec<u8>>>,
{
    let result = (|| {
        if limits.max_rows == 0
            || limits.max_page_rows == 0
            || limits.max_page_rows > 1024
            || limits.max_row_bytes == 0
            || limits.max_row_bytes > 8 * 1024 * 1024
            || limits.max_page_bytes == 0
            || limits.max_page_bytes > 64 * 1024 * 1024
            || limits
                .max_page_rows
                .checked_mul(limits.max_row_bytes)
                .is_none_or(|n| n > limits.max_page_bytes)
            || limits.max_view_ids_per_node == 0
            || limits.max_context_sources == 0
            || limits.max_context_sources > 64
            || limits.max_work_bytes == 0
        {
            return Err(Error::Budget("native finalization limits"));
        }
        if inherited.source_cut != stage.exact_receipt().binding.source_cut
            || inherited.final_graph_rows_written
        {
            return Err(Error::Invalid("native final inherited cut"));
        }
        let roots = stage.core_roots()?;
        if roots.relations != inherited.relation_count
            || roots.relation_sha256 != inherited.relation_root_sha256
        {
            return Err(Error::Invalid("native final complete relations"));
        }
        let compiler = ReadableContextCompiler::from_selected_registry_bytes(
            entity_registry_bytes,
            &registry.entity_sha256,
            ReadableContextLimits {
                max_input_bytes: limits.max_row_bytes,
                max_work_bytes: limits.max_work_bytes,
            },
        )?;
        let mut total = 0u64;
        let mut work = 0u64;
        let mut readable = 0u64;
        for table in ["knowledge_nodes", "knowledge_relations"] {
            let mut after = -1i64;
            loop {
                let batch = page(stage, table, after, limits)?;
                if batch.is_empty() {
                    break;
                }
                stage.with_write_page(
                    WritePhase::Finalize,
                    limits.max_page_rows,
                    limits.max_page_bytes as u64,
                    |stage| {
                for row in batch {
                    if row.order <= after
                        || row.sha.len() != 32
                        || Digest256::of_bytes(&row.payload).as_bytes().as_slice() != row.sha
                    {
                        return Err(Error::Invalid("native final row digest/order"));
                    }
                    total = total
                        .checked_add(1)
                        .ok_or(Error::Budget("native final rows"))?;
                    work = work
                        .checked_add(row.payload.len() as u64)
                        .ok_or(Error::Budget("native final work"))?;
                    if total > limits.max_rows || work > limits.max_work_bytes {
                        return Err(Error::Budget("native final scan"));
                    }
                    let parsed = SourceRow::parse(&row.payload, limits.max_row_bytes)?;
                    let mut value = parsed.value().clone();
                    if value.get("id").and_then(Value::as_str) != Some(row.id.as_str())
                        || value.get("source_graph").and_then(Value::as_str)
                            != Some(row.source.as_str())
                    {
                        return Err(Error::Invalid("native final row identity"));
                    }
                    let mut changed = false;
                    if table == "knowledge_nodes" {
                        let views =
                            endpoint_inherited_views(stage, &row.id, limits.max_view_ids_per_node)?;
                        if !views.is_empty() {
                            let current = value
                                .get("view_ids")
                                .and_then(Value::as_array)
                                .ok_or(Error::Invalid("native node view IDs"))?;
                            let mut union = BTreeSet::new();
                            for item in current {
                                union.insert(
                                    item.as_str()
                                        .ok_or(Error::Invalid("native node view token"))?
                                        .to_owned(),
                                );
                            }
                            union.extend(views);
                            if union.len() > limits.max_view_ids_per_node {
                                return Err(Error::Budget("native finalized view IDs"));
                            }
                            let next: Vec<_> = union.into_iter().map(Value::String).collect();
                            if next != *current {
                                value["view_ids"] = Value::Array(next);
                                changed = true;
                            }
                        }
                    }
                    let precompiled = stage.exact_receipt().collections.iter().any(|entry| {
                        entry.source_graph == row.source
                            && entry.adapter_profile == "indexed-node-edge-v1"
                    });
                    if let Some(compiler) = compiler.as_ref().filter(|_| {
                        !precompiled
                            && (has_context(&value) || value.get("readable_context").is_some())
                    }) {
                        let native = row
                            .native
                            .as_deref()
                            .ok_or(Error::Invalid("native context source identity absent"))?;
                        let collections: &[&str] = if table == "knowledge_nodes" {
                            &["nodes"]
                        } else {
                            &["edges", "relations"]
                        };
                        let mut owner = source_material(
                            stage,
                            table == "knowledge_relations",
                            &row.source,
                            &row.id,
                            native,
                        )?;
                        let needs_raw = owner.is_none();
                        for collection in collections.iter().filter(|_| needs_raw) {
                            if !stage.exact_receipt().collections.iter().any(|entry| {
                                entry.source_graph == row.source && entry.collection == *collection
                            }) {
                                continue;
                            }
                            if let Some(raw) = stage.raw_by_id(&row.source, collection, native)? {
                                if owner.is_some() {
                                    return Err(Error::Invalid("ambiguous native context source"));
                                }
                                owner = Some(raw.payload);
                            }
                        }
                        let mut owner =
                            owner.ok_or(Error::Invalid("native ordered source witness absent"))?;
                        if table == "knowledge_nodes"
                            && stage.exact_receipt().collections.iter().any(|entry| {
                                entry.source_graph == row.source
                                    && entry.collection == "nodes"
                                    && entry.adapter_profile == "reified-bibliographic-claims-v1"
                            })
                        {
                            owner = crate::knowledge_source_claims::ordered_claim_node_material(
                                &owner,
                                limits.max_row_bytes,
                            )?;
                        }
                        if table == "knowledge_relations"
                            && stage.exact_receipt().collections.iter().any(|entry| {
                                entry.source_graph == row.source
                                    && entry.collection == "edges"
                                    && entry.adapter_profile == "reified-bibliographic-claims-v1"
                            })
                        {
                            owner =
                                crate::knowledge_source_claims::claim_relation_material_witness(
                                    stage,
                                    &row.source,
                                    &row.id,
                                    limits.max_row_bytes,
                                )?;
                        }
                        let mut keys = BTreeSet::new();
                        if let Some(contexts) = value
                            .pointer("/semantics/assertion_contexts")
                            .and_then(Value::as_array)
                        {
                            for context in contexts {
                                if context.get("binding_role").and_then(Value::as_str)
                                    == Some("referenced-claim")
                                {
                                    let fields =
                                        context.get("fields").and_then(Value::as_object).ok_or(
                                            Error::Invalid("native referenced context fields"),
                                        )?;
                                    let reference = fields
                                        .get("claim_id")
                                        .or_else(|| fields.get("claim_ref"))
                                        .and_then(|f| f.get("value"))
                                        .and_then(Value::as_str)
                                        .ok_or(Error::Invalid("native referenced context key"))?;
                                    keys.insert(reference.to_owned());
                                }
                            }
                        }
                        let mut sources = Vec::new();
                        for reference in keys {
                            sources.extend(claim_sources(stage, &row.source, &reference)?);
                        }
                        if sources.len() > limits.max_context_sources {
                            return Err(Error::Budget("native referenced source witnesses"));
                        }
                        let raw = serde_json::to_vec(&value)
                            .map_err(|_| Error::Invalid("native readable JSON"))?;
                        let refs = sources.iter().map(Vec::as_slice).collect::<Vec<_>>();
                        let witness =
                            ordered_readable_witness(&raw, &owner, &refs, limits.max_row_bytes)?;
                        work = work
                            .checked_add(owner.len() as u64)
                            .and_then(|n| n.checked_add(witness.len() as u64))
                            .ok_or(Error::Budget("native final witness work"))?;
                        for source in &sources {
                            work = work
                                .checked_add(source.len() as u64)
                                .ok_or(Error::Budget("native final source work"))?;
                        }
                        if work > limits.max_work_bytes {
                            return Err(Error::Budget("native final source work"));
                        }
                        match compiler.compile(&raw, Some(&witness))? {
                            ReadableContextCarrier::Absent => {
                                value.as_object_mut().unwrap().remove("readable_context");
                            }
                            ReadableContextCarrier::Sidecar(raw) => {
                                value["readable_context"] = serde_json::from_slice(&raw)
                                    .map_err(|_| Error::Invalid("native readable sidecar"))?;
                                readable += 1;
                            }
                        }
                        changed = true;
                    }
                    if changed {
                        stamp_content_revision(&mut value, limits.max_row_bytes)?;
                        let raw = serde_json::to_vec(&value)
                            .map_err(|_| Error::Invalid("native final JSON"))?;
                        work = work
                            .checked_add(raw.len() as u64)
                            .ok_or(Error::Budget("native final output work"))?;
                        if raw.len() > limits.max_row_bytes || work > limits.max_work_bytes {
                            return Err(Error::Budget("native final output bytes"));
                        }
                        stage.charge_materialized(1, raw.len() as u64)?;
                        let sha = Digest256::of_bytes(&raw);
                        let changed = stage.with_connection(WritePhase::Finalize, |db| {
                            Ok(db.execute(&format!("UPDATE {table} SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND payload_sha256=?5"),
                                params![raw.len() as i64,sha.as_bytes().as_slice(),raw,row.id,row.sha])?)
                        })?;
                        if changed != 1 {
                            return Err(Error::Invalid("native final concurrent row change"));
                        }
                    }
                    after = row.order;
                }
                Ok(())
                    },
                )?;
            }
        }
        if total
            != roots
                .nodes
                .checked_add(roots.relations)
                .ok_or(Error::Budget("native final coverage"))?
        {
            return Err(Error::Invalid("native final complete row coverage"));
        }
        let final_roots = stage.core_roots()?;
        Ok(NativeFinalizeReceipt {
            source_cut: inherited.source_cut.clone(),
            nodes: final_roots.nodes,
            relations: final_roots.relations,
            readable_rows: readable,
            node_root_sha256: final_roots.node_sha256,
            relation_root_sha256: final_roots.relation_sha256,
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}
