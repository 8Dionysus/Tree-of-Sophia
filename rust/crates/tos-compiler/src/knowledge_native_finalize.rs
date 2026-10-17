//! Final native graph pass after all source nodes, Claims and relations exist.
//! Global inheritance precedes readable context and final revision stamping.
//! Source witnesses stay private until this pass has consumed them.

use crate::knowledge_inherited_views::{
    InheritedViewReceipt, endpoint_inherited_relation_ids, endpoint_inherited_views,
};
use crate::knowledge_normalization::{SourceRow, stamp_content_revision};
use crate::knowledge_stage::{KnowledgeStage, NormalizedLogical, WritePhase};
use crate::{
    Error, KnowledgeRegistry, ReadableContextCarrier, ReadableContextCompiler,
    ReadableContextLimits, Result, ordered_readable_witness,
};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tos_foundation::Digest256;

macro_rules! processing_fingerprint {
    ($handler:literal, [$($source:expr),+ $(,)?] $(,)?) => {{
        static FINGERPRINT: std::sync::OnceLock<tos_foundation::Digest256> =
            std::sync::OnceLock::new();
        *FINGERPRINT.get_or_init(|| {
            crate::processing_cache::rust_source_fingerprint($handler, &[$($source),+])
        })
    }};
}

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

impl NativeFinalizeLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_rows == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8 * 1024 * 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self
                .max_page_rows
                .checked_mul(self.max_row_bytes)
                .is_none_or(|n| n > self.max_page_bytes)
            || self.max_view_ids_per_node == 0
            || self.max_context_sources == 0
            || self.max_context_sources > 64
            || self.max_work_bytes == 0
        {
            return Err(Error::Budget("native finalization limits"));
        }
        Ok(())
    }
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
    source_packet: Option<Vec<u8>>,
}

struct RowRef<'a> {
    id: &'a str,
    source: &'a str,
    native: Option<&'a str>,
    order: i64,
    payload: NormalizedLogical<'a>,
    digest: Digest256,
    source_packet: Option<&'a [u8]>,
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
                payload:row.get::<_,Option<Vec<u8>>>(4)?.ok_or(Error::Budget("native final carrier bytes"))?,sha:row.get(5)?,source_packet:None});
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
    claim_sources: F,
    source_material: G,
) -> Result<NativeFinalizeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<Vec<Vec<u8>>>,
    G: FnMut(&mut KnowledgeStage<'_>, bool, &str, &str, &str) -> Result<Option<Vec<u8>>>,
{
    finalize_native_graph_rows_with_observer(
        stage,
        registry,
        entity_registry_bytes,
        inherited,
        limits,
        claim_sources,
        source_material,
        None,
    )
}

/// The observer borrows each final authenticated Value only after its CAS write
/// succeeds (or after an unchanged row's authenticated read). It cannot issue
/// source authority. Completion must check every observed row against final roots.
pub(crate) trait FinalRowObserver {
    fn begin(&mut self, stage: &mut KnowledgeStage<'_>) -> Result<()>;
    fn row(
        &mut self,
        stage: &mut KnowledgeStage<'_>,
        relation: bool,
        id: &str,
        order: i64,
        length: usize,
        digest: Digest256,
        value: &Value,
    ) -> Result<()>;
}

fn processing_base_row(
    run: &crate::processing_cache::ProcessingRunHandle,
    source_graph: &str,
    stable_id: &str,
    digest: Digest256,
    payload_len: usize,
    relation: bool,
    value: &Value,
) -> Result<crate::processing_cache::ProcessingValue> {
    let kind = if relation { "relation" } else { "node" };
    let task_id = format!("{kind}:{}", stable_id);
    if let Some(previous) = run.result(&task_id) {
        if previous.value() == value {
            return Ok(previous);
        }
    }
    if let Some(previous) = run.latest_output(kind, stable_id) {
        if previous.value() != value {
            return Err(Error::Invalid(
                "processing final row differs from latest producer",
            ));
        }
        return Ok(previous);
    }
    if run.result(&task_id).is_some() {
        return Err(Error::Invalid("processing base row differs from producer"));
    }
    let input = run.input(
        &format!("normalized-row:{kind}:{}", stable_id),
        &json!({
            "source_graph":source_graph,
            "stable_id":stable_id,
            "row_sha256":digest.to_hex(),
            "row_bytes":payload_len,
        }),
    )?;
    let fingerprint = processing_fingerprint!(
        "knowledge_native_finalize::bind_base_row",
        [
            include_bytes!("knowledge_native_finalize.rs"),
            include_bytes!("knowledge_stage.rs"),
            include_bytes!("processing_cache.rs"),
        ],
    );
    run.evaluate_task(
        &task_id,
        kind,
        "knowledge_native_finalize::bind_base_row",
        fingerprint,
        &json!({"source_graph":source_graph,"stable_id":stable_id,"table":if relation {"relations"} else {"nodes"}}),
        &[input],
        || Ok(value.clone()),
    )
}

fn processing_relation_views(
    run: &crate::processing_cache::ProcessingRunHandle,
    stage: &mut KnowledgeStage<'_>,
    relation_id: &str,
    max_row_bytes: usize,
) -> Result<crate::processing_cache::ProcessingValue> {
    let selected: (Option<Vec<u8>>, i64, Option<Vec<u8>>) = stage.with_connection(
        WritePhase::Sort,
        |db| {
            db.query_row(
                "SELECT CASE WHEN typeof(payload)='blob' AND payload_len=length(payload) AND payload_len<=?2 THEN payload END,\
                        payload_len,CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32 THEN payload_sha256 END\
                   FROM knowledge_relations WHERE id=?1",
                params![relation_id, max_row_bytes as i64],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(Error::Invalid("inherited relation row absent"))
        },
    )?;
    let (Some(payload), payload_len, Some(sha)) = selected else {
        return Err(Error::Invalid("inherited relation row projection"));
    };
    if payload_len < 0
        || payload_len as usize != payload.len()
        || sha.as_slice() != Digest256::of_bytes(&payload).as_bytes()
    {
        return Err(Error::Invalid("inherited relation row digest"));
    }
    let value: Value =
        serde_json::from_slice(&payload).map_err(|_| Error::Invalid("inherited relation JSON"))?;
    if value.get("id").and_then(Value::as_str) != Some(relation_id) {
        return Err(Error::Invalid("inherited relation identity"));
    }
    let base = if let Some(previous) = run.result(&format!("relation:{relation_id}")) {
        if previous.value() != &value {
            return Err(Error::Invalid("processing relation differs from stage row"));
        }
        previous
    } else {
        let input = run.input(
            &format!("normalized-row:relation:{relation_id}"),
            &json!({
                "stable_id":relation_id,
                "row_sha256":Digest256::of_bytes(&payload).to_hex(),
                "row_bytes":payload.len(),
            }),
        )?;
        let fingerprint = processing_fingerprint!(
            "knowledge_native_finalize::bind_relation_row",
            [
                include_bytes!("knowledge_native_finalize.rs"),
                include_bytes!("knowledge_stage.rs"),
                include_bytes!("processing_cache.rs"),
            ],
        );
        run.evaluate_task(
            &format!("relation:{relation_id}"),
            "relation",
            "knowledge_native_finalize::bind_relation_row",
            fingerprint,
            &json!({"stable_id":relation_id}),
            &[input],
            || Ok(value.clone()),
        )?
    };
    let views = value
        .get("view_ids")
        .and_then(Value::as_array)
        .cloned()
        .ok_or(Error::Invalid("inherited relation view IDs"))?;
    if let Some(previous) = run.result(&format!("relation-views:{relation_id}")) {
        if previous.value() != &Value::Array(views) {
            return Err(Error::Invalid("processing relation views differ from row"));
        }
        return Ok(previous);
    }
    let fingerprint = processing_fingerprint!(
        "knowledge_native_finalize::relation_views",
        [
            include_bytes!("knowledge_native_finalize.rs"),
            include_bytes!("processing_cache.rs"),
        ],
    );
    run.evaluate_task(
        &format!("relation-views:{relation_id}"),
        "relation-views",
        "knowledge_native_finalize::relation_views",
        fingerprint,
        &json!({"relation_id":relation_id}),
        &[base],
        || Ok(Value::Array(views)),
    )
}

pub(crate) fn finalize_native_graph_rows_with_observer<F, G>(
    stage: &mut KnowledgeStage<'_>,
    registry: &KnowledgeRegistry,
    entity_registry_bytes: &[u8],
    inherited: &InheritedViewReceipt,
    limits: NativeFinalizeLimits,
    claim_sources: F,
    source_material: G,
    observer: Option<&mut dyn FinalRowObserver>,
) -> Result<NativeFinalizeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<Vec<Vec<u8>>>,
    G: FnMut(&mut KnowledgeStage<'_>, bool, &str, &str, &str) -> Result<Option<Vec<u8>>>,
{
    finalize_native_graph_rows_with_observer_processing(
        stage,
        registry,
        entity_registry_bytes,
        inherited,
        limits,
        claim_sources,
        source_material,
        observer,
        None,
    )
}

pub(crate) fn finalize_native_graph_rows_with_observer_processing<F, G>(
    stage: &mut KnowledgeStage<'_>,
    registry: &KnowledgeRegistry,
    entity_registry_bytes: &[u8],
    inherited: &InheritedViewReceipt,
    limits: NativeFinalizeLimits,
    mut claim_sources: F,
    mut source_material: G,
    mut observer: Option<&mut dyn FinalRowObserver>,
    processing_run: Option<&crate::processing_cache::ProcessingRunHandle>,
) -> Result<NativeFinalizeReceipt>
where
    F: FnMut(&mut KnowledgeStage<'_>, &str, &str) -> Result<Vec<Vec<u8>>>,
    G: FnMut(&mut KnowledgeStage<'_>, bool, &str, &str, &str) -> Result<Option<Vec<u8>>>,
{
    let mut total = 0u64;
    let mut work = 0u64;
    let mut readable = 0u64;
    let result = (|| {
        limits.validate()?;
        if inherited.source_cut != stage.exact_receipt()?.binding.source_cut
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
        eprintln!(
            "Native finalize start nodes={} relations={}",
            roots.nodes, roots.relations
        );
        if let Some(observer) = observer.as_deref_mut() {
            observer.begin(stage)?;
        }
        for table in ["knowledge_nodes", "knowledge_relations"] {
            let mut after = -1i64;
            loop {
                let page_after = after;
                let owned = stage.owned_creation_state().is_some();
                let batch = if owned {
                    None
                } else {
                    Some(page(stage, table, page_after, limits)?)
                };
                if batch.as_ref().is_some_and(Vec::is_empty) {
                    break;
                }
                let count = stage.with_write_page(
                    WritePhase::Finalize,
                    limits.max_page_rows,
                    limits.max_page_bytes as u64,
                    |stage| {
                        let mut count = 0usize;
                        let mut process = |stage: &mut KnowledgeStage<'_>, row: RowRef<'_>| -> Result<()> {
                            // One row owns its transient mutation workspace through SQL.
                            // The normalized reader retains the actual payload and source.
                            let _row_hold = stage.hold_normalized_page(1, limits.max_row_bytes, limits.max_row_bytes)?;
                    if row.order <= after
                        || match &row.payload {
                            NormalizedLogical::Bytes(raw) => Digest256::of_bytes(raw) != row.digest,
                            NormalizedLogical::Value { digest, .. } => *digest != row.digest,
                        }
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
                    let state = stage.owned_creation_state();
                    let original_len = row.payload.len();
                    let source_receipt = match &row.payload {
                        NormalizedLogical::Value { source_receipt, .. } => *source_receipt,
                        NormalizedLogical::Bytes(_) => None,
                    };
                    let mut process_value = |mut value: Value| -> Result<()> {
                    if value.get("id").and_then(Value::as_str) != Some(row.id)
                        || value.get("source_graph").and_then(Value::as_str)
                            != Some(row.source)
                    {
                        return Err(Error::Invalid("native final row identity"));
                    }
                    let mut changed = false;
                    let mut final_processing = None;
                    if let Some(run) = processing_run {
                        let relation = table == "knowledge_relations";
                        let base = processing_base_row(run, row.source, row.id, row.digest, original_len, relation, &value)?;
                        if relation {
                            let views = processing_relation_views(
                                run,
                                stage,
                                row.id,
                                limits.max_row_bytes,
                            )?;
                            let fingerprint = processing_fingerprint!(
                                "knowledge_native_finalize::final_relation",
                                [
include_bytes!("knowledge_native_finalize.rs"),
                                    include_bytes!("processing_cache.rs"),
                                ],
                            );
                            let final_row = run.evaluate_task(
                                &format!("final-relation:{}", row.id),
                                "final-relation",
                                "knowledge_native_finalize::final_relation",
                                fingerprint,
                                &json!({"source_graph":row.source,"stable_id":row.id}),
                                &[base, views],
                                || Ok(value.clone()),
                            )?;
                            changed = final_row.value() != &value;
                            value = final_row.value().clone();
                            final_processing = Some(final_row);
                        } else {
                            let exact_views = endpoint_inherited_views(
                                stage,
                                row.id,
                                limits.max_view_ids_per_node,
                            )?;
                            let relation_ids = endpoint_inherited_relation_ids(
                                stage,
                                row.id,
                                limits.max_rows.min(65_536) as usize,
                            )?;
                            let mut relation_views = Vec::with_capacity(relation_ids.len());
                            for relation_id in &relation_ids {
                                relation_views.push(processing_relation_views(
                                    run,
                                    stage,
                                    relation_id,
                                    limits.max_row_bytes,
                                )?);
                            }
                            let inherited_fingerprint =
                                processing_fingerprint!(
                                    "knowledge_inherited_views::endpoint_inherited_views",
                                    [
include_bytes!("knowledge_inherited_views.rs"),
                                        include_bytes!("processing_cache.rs"),
                                    ],
                                );
                            let inherited = run.evaluate_task(
                                &format!("inherited-views:{}", row.id),
                                "inherited-views",
                                "knowledge_inherited_views::endpoint_inherited_views",
                                inherited_fingerprint,
                                &json!({"endpoint_id":row.id,"max_views":limits.max_view_ids_per_node}),
                                &relation_views,
                                || {
                                    let mut values = BTreeSet::new();
                                    for relation in &relation_views {
                                        for item in relation
                                            .value()
                                            .as_array()
                                            .ok_or(Error::Invalid("processing relation views array"))?
                                        {
                                            values.insert(
                                                item.as_str()
                                                    .ok_or(Error::Invalid("processing relation view token"))?
                                                    .to_owned(),
                                            );
                                        }
                                    }
                                    Ok(Value::Array(values.into_iter().map(Value::String).collect()))
                                },
                            )?;
                            if inherited.value() != &json!(exact_views) {
                                return Err(Error::Invalid("processing inherited views differ from index"));
                            }
                            let final_fingerprint =
                                processing_fingerprint!(
                                    "knowledge_native_finalize::final_node",
                                    [
include_bytes!("knowledge_native_finalize.rs"),
                                        include_bytes!("knowledge_normalization.rs"),
                                        include_bytes!("knowledge_seal.rs"),
                                        include_bytes!("processing_cache.rs"),
                                    ],
                                );
                            let final_row = run.evaluate_task(
                                &format!("final-node:{}", row.id),
                                "final-node",
                                "knowledge_native_finalize::final_node",
                                final_fingerprint,
                                &json!({"source_graph":row.source,"stable_id":row.id,"max_views":limits.max_view_ids_per_node}),
                                &[base, inherited],
                                || {
                                    let mut result = value.clone();
                                    let current = result
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
                                    union.extend(exact_views.iter().cloned());
                                    if union.len() > limits.max_view_ids_per_node {
                                        return Err(Error::Budget("native finalized view IDs"));
                                    }
                                    result["view_ids"] = Value::Array(
                                        union.into_iter().map(Value::String).collect(),
                                    );
                                    stamp_content_revision(&mut result, limits.max_row_bytes)?;
                                    Ok(result)
                                },
                            )?;
                            changed = final_row.value() != &value;
                            value = final_row.value().clone();
                            final_processing = Some(final_row);
                        }
                    } else if table == "knowledge_nodes" {
                        let views =
                            endpoint_inherited_views(stage, row.id, limits.max_view_ids_per_node)?;
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
                    let precompiled = stage.exact_receipt()?.collections.iter().any(|entry| {
                        entry.source_graph == row.source
                            && entry.adapter_profile == "indexed-node-edge-v1"
                    });
                    if let Some(compiler) = compiler.as_ref().filter(|_| {
                        !precompiled
                            && (has_context(&value) || value.get("readable_context").is_some())
                    }) {
                        let native = row
                            .native
                            .ok_or(Error::Invalid("native context source identity absent"))?;
                        let collections: &[&str] = if table == "knowledge_nodes" {
                            &["nodes"]
                        } else {
                            &["edges", "relations"]
                        };
                        let owner = source_material(
                            stage,
                            table == "knowledge_relations",
                            row.source,
                            row.id,
                            native,
                        )?;
                        let needs_raw = owner.is_none();
                        // Keep the exact input and its scoped admission until
                        // readable-context compilation finishes for this row.
                        let mut raw_owner = None;
                        for collection in collections.iter().filter(|_| needs_raw) {
                            if !stage.exact_receipt()?.collections.iter().any(|entry| {
                                entry.source_graph == row.source && entry.collection == *collection
                            }) {
                                continue;
                            }
                            let raw = stage.scoped_raw_by_id(row.source, collection, native)?;
                            if raw.as_ref().is_some() {
                                if raw_owner.is_some() {
                                    return Err(Error::Invalid("ambiguous native context source"));
                                }
                                raw_owner = Some(raw);
                            }
                        }
                        let original_owner = owner.as_deref().or_else(|| {
                            raw_owner.as_ref().and_then(|raw| raw.as_ref()).map(|raw| raw.payload.as_slice())
                        }).ok_or_else(|| {
                            // Identities only; never print private source payloads.
                            eprintln!("Native final source witness absent: table={table} source={:?} id={:?} native={:?}", row.source, row.id, native);
                            Error::Invalid("native ordered source witness absent")
                        })?;
                        let mut material = None;
                        if table == "knowledge_nodes"
                            && stage.exact_receipt()?.collections.iter().any(|entry| {
                                entry.source_graph == row.source
                                    && entry.collection == "nodes"
                                    && entry.adapter_profile == "reified-bibliographic-claims-v1"
                            })
                        {
                            material = Some(crate::knowledge_source_claims::ordered_claim_node_material(
                                original_owner,
                                limits.max_row_bytes,
                            )?);
                        }
                        if table == "knowledge_relations"
                            && stage.exact_receipt()?.collections.iter().any(|entry| {
                                entry.source_graph == row.source
                                    && entry.collection == "edges"
                                    && entry.adapter_profile == "reified-bibliographic-claims-v1"
                            })
                        {
                            material = Some(
                                crate::knowledge_source_claims::claim_relation_material_witness(
                                    stage,
                                    row.source,
                                    row.id,
                                    limits.max_row_bytes,
                                )?);
                        }
                        let owner = material.as_deref().unwrap_or(original_owner);
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
                        let mut context_dependencies = Vec::new();
                        let mut context_group = Vec::new();
                        for reference in keys {
                            let reference_sources = claim_sources(stage, row.source, &reference)?;
                            let mut source_ids = Vec::new();
                            for (index, source) in reference_sources.iter().enumerate() {
                                if let Some(run) = processing_run {
                                    context_dependencies.push(run.input(
                                        &format!("claim-context-source:{table}:{}:{reference}:{index}", row.id),
                                        &json!({
                                            "sha256":Digest256::of_bytes(source).to_hex(),
                                            "bytes":source.len(),
                                        }),
                                    )?);
                                }
                                source_ids.push(json!({
                                    "sha256":Digest256::of_bytes(source).to_hex(),
                                    "bytes":source.len(),
                                }));
                            }
                            context_group.push(json!({
                                "claim_ref":reference,
                                "sources":source_ids,
                            }));
                            sources.extend(reference_sources);
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
                        if let Some(run) = processing_run {
                            let owner_input = run.input(
                                &format!("source-witness:{table}:{}", row.id),
                                &json!({
                                    "sha256":Digest256::of_bytes(&owner).to_hex(),
                                    "bytes":owner.len(),
                                }),
                            )?;
                            let group_fingerprint =
                                processing_fingerprint!(
                                    "knowledge_source_claims::ordered_context_group",
                                    [
include_bytes!("knowledge_source_claims.rs"),
                                        include_bytes!("knowledge_native_finalize.rs"),
                                        include_bytes!("processing_cache.rs"),
                                    ],
                                );
                            let group = run.evaluate_task(
                                &format!("claim-context-group:{table}:{}", row.id),
                                "claim-context-group",
                                "knowledge_source_claims::ordered_context_group",
                                group_fingerprint,
                                &json!({"stable_id":row.id,"group_count":context_group.len()}),
                                &context_dependencies,
                                || Ok(Value::Array(context_group)),
                            )?;
                            let final_task = final_processing
                                .clone()
                                .ok_or(Error::Invalid("native final processing task absent"))?;
                            let readable_kind = if table == "knowledge_nodes" {
                                "readable-node"
                            } else {
                                "readable-relation"
                            };
                            let readable_fingerprint =
                                processing_fingerprint!(
                                    "knowledge_native_finalize::compile_readable_context",
                                    [
include_bytes!("knowledge_native_finalize.rs"),
                                        include_bytes!("knowledge_readable_context.rs"),
                                        include_bytes!("knowledge_source_claims.rs"),
                                        include_bytes!("knowledge_normalization.rs"),
                                        include_bytes!("knowledge_seal.rs"),
                                        include_bytes!("processing_cache.rs"),
                                    ],
                                );
                            let readable_task = run.evaluate_task(
                                &format!("{readable_kind}:{}", row.id),
                                readable_kind,
                                "knowledge_native_finalize::compile_readable_context",
                                readable_fingerprint,
                                &json!({"source_graph":row.source,"stable_id":row.id,"max_row_bytes":limits.max_row_bytes}),
                                &[final_task, owner_input, group],
                                || {
                                    let mut result = value.clone();
                                    match compiler.compile(&raw, Some(&witness))? {
                                        ReadableContextCarrier::Absent => {
                                            result.as_object_mut().unwrap().remove("readable_context");
                                        }
                                        ReadableContextCarrier::Sidecar(raw) => {
                                            result["readable_context"] = serde_json::from_slice(&raw)
                                                .map_err(|_| Error::Invalid("native readable sidecar"))?;
                                        }
                                    }
                                    stamp_content_revision(&mut result, limits.max_row_bytes)?;
                                    Ok(result)
                                },
                            )?;
                            value = readable_task.value().clone();
                            if value.get("readable_context").is_some() {
                                readable += 1;
                            }
                        } else {
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
                        }
                        changed = true;
                    }
                    if changed {
                        let mut write = |value: &Value, raw: &[u8]| -> Result<()> {
                        work = work
                            .checked_add(raw.len() as u64)
                            .ok_or(Error::Budget("native final output work"))?;
                        if raw.len() > limits.max_row_bytes || work > limits.max_work_bytes {
                            return Err(Error::Budget("native final output bytes"));
                        }
                        if stage.owned_creation_state().is_some() {
                            stage.replace_finalized_value_if_current(
                                table == "knowledge_relations", row.id, value, raw,
                                row.source_packet, row.digest, source_receipt,
                            )?;
                        } else {
                            stage.charge_materialized(1, raw.len() as u64)?;
                            let sha = Digest256::of_bytes(&raw);
                            let changed = stage.with_connection(WritePhase::Finalize, |db| {
                                Ok(db.execute(&format!("UPDATE {table} SET payload_len=?1,payload_sha256=?2,payload=?3 WHERE id=?4 AND payload_sha256=?5"),
                                    params![raw.len() as i64,sha.as_bytes().as_slice(),raw,row.id,row.digest.as_bytes().as_slice()])?)
                            })?;
                            if changed != 1 {
                                return Err(Error::Invalid("native final concurrent row change"));
                            }
                        }
                        if let Some(observer)=observer.as_deref_mut() {
                            if let Some(state)=state {state.charge_work(raw.len())?;}
                            observer.row(stage,table == "knowledge_relations",row.id,row.order,
                                raw.len(),Digest256::of_bytes(raw),value)?;
                        }
                            Ok(())
                        };
                        if let Some(state) = state {
                            crate::knowledge_normalization::with_content_revision_owned(
                                state, value, limits.max_row_bytes, |value, raw| write(value, raw),
                            )?;
                        } else {
                            stamp_content_revision(&mut value, limits.max_row_bytes)?;
                            let raw = serde_json::to_vec(&value)
                                .map_err(|_| Error::Invalid("native final JSON"))?;
                            write(&value, &raw)?;
                        }
                    } else if let Some(observer)=observer.as_deref_mut() {
                        observer.row(stage,table == "knowledge_relations",row.id,row.order,
                            original_len,row.digest,&value)?;
                    }
                    Ok(())
                    };
                    match row.payload {
                        NormalizedLogical::Bytes(raw) => {
                            SourceRow::parse_scoped_with_optional_owned_state(raw, limits.max_row_bytes, state)?
                                .with_value(process_value)?;
                        }
                        NormalizedLogical::Value { value, .. } => process_value(value)?,
                    }
                    after = row.order;
                    if total.is_power_of_two() {
                        eprintln!("Native finalize progress table={table} rows={total} readable_rows={readable} source_output_bytes={work}");
                    }
                            count += 1;
                            Ok(())
                        };
                        if let Some(batch) = batch {
                            for row in batch {
                                let digest = Digest256::from_bytes(row.sha.as_slice().try_into()
                                    .map_err(|_| Error::Invalid("native final row digest size"))?);
                                process(stage, RowRef {
                                    id: &row.id, source: &row.source, native: row.native.as_deref(),
                                    order: row.order, payload: NormalizedLogical::Bytes(&row.payload), digest,
                                    source_packet: row.source_packet.as_deref(),
                                })?;
                            }
                        } else {
                            stage.with_normalized_rows_decoded_owned(
                                table == "knowledge_relations", page_after,
                                limits.max_page_rows, limits.max_row_bytes, true,
                                |stage, metadata, logical, source| process(stage, RowRef {
                                    id: metadata.id, source: metadata.source_graph, native: metadata.native_id,
                                    order: metadata.source_order, payload: logical,
                                    digest: metadata.logical_digest, source_packet: source,
                                }),
                            )?;
                        }
                        Ok(count)
                    },
                )?;
                if count == 0 {
                    break;
                }
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
    eprintln!(
        "Native finalize complete status={} rows={total} readable_rows={readable} source_output_bytes={work}",
        if result.is_ok() { "passed" } else { "refused" }
    );
    if result.is_err() {
        stage.poison();
    }
    result
}
