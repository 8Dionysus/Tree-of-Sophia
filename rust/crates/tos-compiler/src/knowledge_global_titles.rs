//! Disk-backed title dependencies for relation normalization.
//!
//! The source families first write all base nodes. This phase reads their
//! complete node root once, stores only bounded title packets, and supplies
//! indexed exact endpoint lookups. The caller still owns all-source coverage
//! and missing-endpoint proof; this is a private dependency, never a graph.

use crate::d1_public_capture::{CreationState, CreationStateHold};
use crate::knowledge_normalization::{OwnedSourceRow, SourceRow};
use crate::knowledge_stage::{KnowledgeStage, WritePhase};
use crate::{Error, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use tos_foundation::{Digest256, Digest256Hasher};
use tos_source_store::{PinnedBoundedStatement, StoreError, StoreErrorCode};

#[derive(Clone, Debug)]
pub struct CompleteBaseNodes {
    pub source_cut: String,
    pub node_count: u64,
    pub node_root_sha256: String,
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalTitleLimits {
    pub max_nodes: u64,
    pub max_page_rows: usize,
    pub max_page_bytes: usize,
    pub max_node_bytes: usize,
    pub max_title_bytes: usize,
    pub max_work_bytes: u64,
}
impl GlobalTitleLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_page_rows == 0
            || self.max_page_rows > 1024
            || self.max_page_bytes == 0
            || self.max_page_bytes > 64 * 1024 * 1024
            || self.max_node_bytes == 0
            || self.max_node_bytes > 8 * 1024 * 1024
            || self.max_title_bytes == 0
            || self.max_title_bytes > 64 * 1024
            || self.max_work_bytes == 0
            || self
                .max_page_rows
                .checked_mul(self.max_node_bytes)
                .is_none_or(|bytes| bytes > self.max_page_bytes)
        {
            return Err(Error::Budget("global title index limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct GlobalTitleReceipt {
    pub source_cut: String,
    pub base_node_count: u64,
    pub base_node_root_sha256: String,
    pub title_count: u64,
    pub title_root_sha256: String,
}

struct BaseRow {
    id: String,
    order: i64,
    payload: Vec<u8>,
    sha: [u8; 32],
}

fn page(
    stage: &mut KnowledgeStage<'_>,
    after_order: i64,
    limits: GlobalTitleLimits,
) -> Result<Vec<BaseRow>> {
    let owned = stage.owned_creation_state();
    let mut batch = stage.with_connection(WritePhase::Sort, |db| {
        let payload = if owned.is_some() {
            "NULL"
        } else {
            "CASE WHEN typeof(payload)='blob' AND payload_len=length(payload) AND length(payload)<=?2 THEN payload ELSE NULL END"
        };
        let sql = format!("SELECT CASE WHEN typeof(id)='text' AND length(CAST(id AS BLOB))<=4096 THEN id ELSE NULL END,
             source_order,{payload},
             CASE WHEN typeof(payload_sha256)='blob' AND length(payload_sha256)=32
             THEN payload_sha256 ELSE NULL END
             FROM knowledge_nodes WHERE source_order>?1 ORDER BY source_order LIMIT ?3");
        let mut statement = db.prepare(&sql)?;
        let mut rows = statement.query(params![after_order, limits.max_node_bytes as i64, limits.max_page_rows as i64])?;
        let mut out = Vec::with_capacity(limits.max_page_rows);
        while let Some(row) = rows.next()? {
            let id: Option<String> = row.get(0)?;
            let payload: Option<Vec<u8>> = row.get(2)?;
            let sha: Option<Vec<u8>> = row.get(3)?;
            let sha = sha.ok_or(Error::Invalid("global title node digest"))?;
            let sha: [u8; 32] = sha.try_into().map_err(|_| Error::Invalid("global title node digest"))?;
            out.push(BaseRow {
                id: id.ok_or(Error::Budget("global title node ID bytes"))?,
                order: row.get(1)?,
                payload: if owned.is_some() { Vec::new() } else { payload.ok_or(Error::Budget("global title node bytes"))? },
                sha,
            });
        }
        Ok(out)
    })?;
    if let Some(state) = owned {
        for row in &mut batch {
            let payload = &mut row.payload;
            stage
                .with_node_payload_owned(&row.id, limits.max_node_bytes, |_, logical| {
                    state.charge_work(logical.len())?;
                    *payload = logical.to_vec();
                    Ok(())
                })?
                .ok_or(Error::Invalid("global title node disappeared"))?;
        }
    }
    Ok(batch)
}

fn root_item(hash: &mut Digest256Hasher, id: &str, sha: &[u8; 32]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(sha);
}

fn build_inner(
    stage: &mut KnowledgeStage<'_>,
    sealed: &CompleteBaseNodes,
    limits: GlobalTitleLimits,
) -> Result<GlobalTitleReceipt> {
    limits.validate()?;
    if sealed.source_cut.is_empty()
        || sealed.source_cut != stage.exact_receipt()?.binding.source_cut
        || sealed.node_count > limits.max_nodes
        || Digest256::from_hex(&sealed.node_root_sha256).is_err()
    {
        return Err(Error::Invalid("global title base seal"));
    }
    let roots = stage.core_roots()?;
    if roots.nodes != sealed.node_count || roots.node_sha256 != sealed.node_root_sha256 {
        return Err(Error::Invalid("global title complete base root"));
    }
    stage.create_preparation_tables(crate::knowledge_stage::preparation_schema!(
            table r#"knowledge_global_titles(
             node_id TEXT PRIMARY KEY,title_len INTEGER NOT NULL,
             title_sha256 BLOB NOT NULL CHECK(length(title_sha256)=32),
             title_json BLOB NOT NULL) WITHOUT ROWID"#
        ))?;
    let mut after = -1i64;
    let mut count = 0u64;
    let mut work = 0u64;
    let mut root = Digest256Hasher::new();
    root.update(b"tos-global-titles-v1\0");
    root.update(&(sealed.source_cut.len() as u64).to_be_bytes());
    root.update(sealed.source_cut.as_bytes());
    root.update(
        Digest256::from_hex(&sealed.node_root_sha256)
            .map_err(|_| Error::Invalid("global title node root"))?
            .as_bytes(),
    );
    loop {
        let _page_hold = stage.hold_normalized_page(
            limits.max_page_rows,
            limits.max_node_bytes,
            limits.max_title_bytes,
        )?;
        let batch = page(stage, after, limits)?;
        if batch.is_empty() {
            break;
        }
        let mut titles = Vec::with_capacity(batch.len());
        for row in batch {
            if row.order <= after
                || row.order < 0
                || Digest256::of_bytes(&row.payload).as_bytes() != &row.sha
            {
                return Err(Error::Invalid("global title base row"));
            }
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("global title rows"))?;
            work = work
                .checked_add(row.payload.len() as u64)
                .ok_or(Error::Budget("global title work"))?;
            if count > limits.max_nodes || work > limits.max_work_bytes {
                return Err(Error::Budget("global title scan"));
            }
            let node = SourceRow::parse_scoped_with_optional_owned_state(
                &row.payload,
                limits.max_node_bytes,
                stage.owned_creation_state(),
            )?;
            let value = node.value();
            if value.get("id").and_then(Value::as_str) != Some(row.id.as_str()) {
                return Err(Error::Invalid("global title node binding"));
            }
            let title = value
                .get("display")
                .and_then(|v| v.get("title"))
                .filter(|v| v.is_object())
                .ok_or(Error::Invalid("global title missing display"))?;
            let bytes =
                serde_json::to_vec(title).map_err(|_| Error::Invalid("global title JSON"))?;
            if bytes.len() > limits.max_title_bytes {
                return Err(Error::Budget("global title bytes"));
            }
            work = work
                .checked_add(bytes.len() as u64)
                .ok_or(Error::Budget("global title work"))?;
            if work > limits.max_work_bytes {
                return Err(Error::Budget("global title work"));
            }
            let digest = Digest256::of_bytes(&bytes);
            root_item(&mut root, &row.id, digest.as_bytes());
            after = row.order;
            titles.push((row.id, bytes, *digest.as_bytes()));
        }
        let output_bytes = titles
            .iter()
            .try_fold(0u64, |n, (_, bytes, _)| n.checked_add(bytes.len() as u64))
            .ok_or(Error::Budget("global title materialization"))?;
        stage.charge_materialized(titles.len() as u64, output_bytes)?;
        stage.with_connection(WritePhase::Normalized, |db| {
            let tx = db.transaction()?;
            for (id, bytes, sha) in titles {
                tx.execute(
                    "INSERT INTO knowledge_global_titles VALUES(?1,?2,?3,?4)",
                    params![id, bytes.len() as i64, sha.as_slice(), bytes],
                )?;
            }
            tx.commit()?;
            Ok(())
        })?;
    }
    if count != sealed.node_count {
        return Err(Error::Invalid("global title node coverage"));
    }
    let after_roots = stage.core_roots()?;
    if after_roots.node_sha256 != roots.node_sha256
        || after_roots.relation_sha256 != roots.relation_sha256
    {
        return Err(Error::Invalid("global title core drift"));
    }
    Ok(GlobalTitleReceipt {
        source_cut: sealed.source_cut.clone(),
        base_node_count: count,
        base_node_root_sha256: sealed.node_root_sha256.clone(),
        title_count: count,
        title_root_sha256: root.finalize().to_hex(),
    })
}

/// Build a complete private title index from a separately sealed all-source
/// base-node root. Every failure poisons the private stage.
pub fn prepare_global_titles(
    stage: &mut KnowledgeStage<'_>,
    sealed: &CompleteBaseNodes,
    limits: GlobalTitleLimits,
) -> Result<GlobalTitleReceipt> {
    let result = build_inner(stage, sealed, limits);
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Bounded exact endpoint title lookup for relation display. The caller must
/// retain the receipt and the same private stage until relation completion.
pub fn endpoint_title(
    stage: &mut KnowledgeStage<'_>,
    receipt: &GlobalTitleReceipt,
    endpoint_id: &str,
    max_title_bytes: usize,
) -> Result<Value> {
    if receipt.source_cut != stage.exact_receipt()?.binding.source_cut
        || endpoint_id.is_empty()
        || endpoint_id.len() > 4096
        || max_title_bytes == 0
        || max_title_bytes > 64 * 1024
    {
        return Err(Error::Invalid("global title lookup binding"));
    }
    let (length, sha, raw): (i64, Vec<u8>, Vec<u8>) = stage.with_connection(WritePhase::Sort, |db| {
        db.query_row("SELECT title_len,title_sha256,CASE WHEN typeof(title_json)='blob' AND length(title_json)<=?2 THEN title_json ELSE NULL END FROM knowledge_global_titles WHERE node_id=?1",
            params![endpoint_id,max_title_bytes as i64], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))
            .optional()?.ok_or(Error::Invalid("global title endpoint absent"))
    })?;
    if length < 0
        || length as usize != raw.len()
        || sha.len() != 32
        || Digest256::of_bytes(&raw).as_bytes().as_slice() != sha.as_slice()
    {
        return Err(Error::Invalid("global title endpoint digest"));
    }
    let title = SourceRow::parse(
        &format!(
            "{{\"title\":{}}}",
            String::from_utf8(raw).map_err(|_| Error::Invalid("global title UTF-8"))?
        )
        .into_bytes(),
        max_title_bytes + 16,
    )?;
    title
        .value()
        .get("title")
        .cloned()
        .ok_or(Error::Invalid("global title JSON"))
}

pub(crate) struct OwnedGlobalTitle<'state, 'budget> {
    value: Value,
    _hold: Option<crate::d1_public_capture::CreationStateHold<'state, 'budget>>,
}
impl std::ops::Deref for OwnedGlobalTitle<'_, '_> {
    type Target = Value;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
pub(crate) fn endpoint_title_scoped<'state, 'budget>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &GlobalTitleReceipt,
    endpoint_id: &str,
    max_title_bytes: usize,
    owner_state: Option<&'state crate::d1_public_capture::CreationState<'budget>>,
) -> Result<OwnedGlobalTitle<'state, 'budget>> {
    let Some(state) = owner_state else {
        return Ok(OwnedGlobalTitle {
            value: endpoint_title(stage, receipt, endpoint_id, max_title_bytes)?,
            _hold: None,
        });
    };
    if receipt.source_cut != stage.exact_receipt()?.binding.source_cut
        || endpoint_id.is_empty()
        || endpoint_id.len() > 4096
        || max_title_bytes == 0
        || max_title_bytes > 64 * 1024
    {
        return Err(Error::Invalid("global title lookup binding"));
    }
    let read_upper = max_title_bytes
        .checked_add(512)
        .ok_or(Error::Budget("global title SQL row geometry"))?;
    let _read_hold = state.hold(read_upper)?;
    let (length, sha, raw): (i64, Vec<u8>, Vec<u8>) = stage.with_connection(WritePhase::Sort, |db| {
        db.query_row("SELECT title_len,title_sha256,CASE WHEN typeof(title_json)='blob' AND length(title_json)<=?2 THEN title_json ELSE NULL END FROM knowledge_global_titles WHERE node_id=?1",
            params![endpoint_id,max_title_bytes as i64], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))
            .optional()?.ok_or(Error::Invalid("global title endpoint absent"))
    })?;
    if length < 0
        || length as usize != raw.len()
        || sha.len() != 32
        || Digest256::of_bytes(&raw).as_bytes().as_slice() != sha.as_slice()
    {
        return Err(Error::Invalid("global title endpoint digest"));
    }
    state.charge_work(raw.len())?;
    let wrapper_bytes = raw
        .len()
        .checked_add(16)
        .ok_or(Error::Budget("global title wrapper bytes"))?;
    let wrapper_hold = state.hold(wrapper_bytes)?;
    let title_raw = format!(
        "{{\"title\":{}}}",
        String::from_utf8(raw).map_err(|_| Error::Invalid("global title UTF-8"))?
    )
    .into_bytes();
    let title = SourceRow::parse_scoped_with_optional_owned_state(
        &title_raw,
        max_title_bytes + 16,
        Some(state),
    )?;
    let field = title
        .value()
        .get("title")
        .ok_or(Error::Invalid("global title JSON"))?;
    let clone_bytes = state.value_clone_state_upper_bound(field)?;
    state.charge_work(clone_bytes)?;
    let output_hold = state.hold(clone_bytes)?;
    let value = field.clone();
    drop(title);
    drop(wrapper_hold);
    Ok(OwnedGlobalTitle {
        value,
        _hold: Some(output_hold),
    })
}

fn bounded_title_store_error(error: StoreError) -> Error {
    match error.code {
        StoreErrorCode::BudgetExceeded => Error::Budget("global title bounded SQLite"),
        _ => Error::Invalid("global title bounded SQLite"),
    }
}

fn endpoint_title_row_owned<'s, 'budget>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &GlobalTitleReceipt,
    endpoint_id: &str,
    max_title_bytes: usize,
    state: &'s CreationState<'budget>,
) -> Result<OwnedSourceRow<'s, 'budget>> {
    let sql = c"SELECT title_len,title_sha256,
        CASE WHEN typeof(title_json)='blob' AND length(title_json)<=?2
        THEN title_json ELSE NULL END
        FROM knowledge_global_titles WHERE node_id=?1";
    stage.with_connection(WritePhase::Sort, |db| {
        let workspace = PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound()
            .checked_add(sql.to_bytes_with_nul().len())
            .and_then(|n| n.checked_add(std::mem::size_of::<(String, i64)>()))
            .ok_or(Error::Budget("global title bounded SQL workspace"))?;
        let _sql_hold: CreationStateHold<'_, 'budget> = state.hold(workspace)?;
        state.active()?;
        let mut statement = PinnedBoundedStatement::prepare_on_owned_connection(db, sql)
            .map_err(bounded_title_store_error)?;
        state.charge_work(endpoint_id.len())?;
        statement
            .bind_text(1, endpoint_id)
            .map_err(bounded_title_store_error)?;
        statement
            .bind_i64(2, max_title_bytes as i64)
            .map_err(bounded_title_store_error)?;
        state.active()?;
        if !statement.step().map_err(bounded_title_store_error)? {
            return Err(Error::Invalid("global title endpoint absent"));
        }
        let length = statement.integer(0).map_err(bounded_title_store_error)?;
        let sha = match statement.value_ref(1).map_err(bounded_title_store_error)? {
            rusqlite::types::ValueRef::Blob(bytes) => bytes,
            _ => return Err(Error::Invalid("global title endpoint digest")),
        };
        let raw = match statement.value_ref(2).map_err(bounded_title_store_error)? {
            rusqlite::types::ValueRef::Blob(bytes) => bytes,
            _ => return Err(Error::Budget("global title endpoint bytes")),
        };
        if length < 0 || length as usize != raw.len() || sha.len() != 32 {
            return Err(Error::Invalid("global title endpoint digest"));
        }
        state.charge_work(raw.len())?;
        if Digest256::of_bytes(raw).as_bytes().as_slice() != sha {
            return Err(Error::Invalid("global title endpoint digest"));
        }
        let row = SourceRow::parse_scoped_with_owned_state(raw, max_title_bytes, state)?;
        if !row.value().is_object() {
            return Err(Error::Invalid("global title JSON object"));
        }
        Ok(row)
    })
}

/// Keep both decoded endpoint title trees under the same CreationState through
/// normalization, encoding and synchronous insertion. Each native SQL
/// statement is finalized before the next lookup and before `consume`.
pub(crate) fn with_endpoint_title_pair_owned<T>(
    stage: &mut KnowledgeStage<'_>,
    receipt: &GlobalTitleReceipt,
    left_id: &str,
    right_id: &str,
    max_title_bytes: usize,
    consume: impl FnOnce(&mut KnowledgeStage<'_>, &Value, &Value) -> Result<T>,
) -> Result<T> {
    let state = stage
        .owned_creation_state()
        .ok_or(Error::Invalid("owned title state absent"))?;
    if receipt.source_cut != stage.exact_receipt()?.binding.source_cut
        || left_id.is_empty()
        || left_id.len() > 4096
        || right_id.is_empty()
        || right_id.len() > 4096
        || max_title_bytes == 0
        || max_title_bytes > 64 * 1024
    {
        return Err(Error::Invalid("global title owned lookup binding"));
    }
    let left = endpoint_title_row_owned(stage, receipt, left_id, max_title_bytes, state)?;
    let right = endpoint_title_row_owned(stage, receipt, right_id, max_title_bytes, state)?;
    let result = consume(stage, left.value(), right.value());
    drop(right);
    drop(left);
    result
}

/// Recheck the private complete title carrier before another source relation
/// family consumes it. Claim finalization may have changed node payloads;
/// this receipt intentionally binds the earlier complete base-node generation.
pub fn verify_global_titles(
    stage: &mut KnowledgeStage<'_>,
    receipt: &GlobalTitleReceipt,
    max_title_bytes: usize,
    max_work_bytes: u64,
) -> Result<()> {
    let result = (|| {
        if receipt.source_cut != stage.exact_receipt()?.binding.source_cut
            || receipt.title_count != receipt.base_node_count
            || max_title_bytes == 0
            || max_title_bytes > 64 * 1024
            || max_work_bytes == 0
        {
            return Err(Error::Invalid("global title verification binding"));
        }
        let base = Digest256::from_hex(&receipt.base_node_root_sha256)
            .map_err(|_| Error::Invalid("global title verification base root"))?;
        stage.with_connection(WritePhase::Sort, |db| {
            let mut root = Digest256Hasher::new();
            root.update(b"tos-global-titles-v1\0");
            root.update(&(receipt.source_cut.len() as u64).to_be_bytes());
            root.update(receipt.source_cut.as_bytes());
            root.update(base.as_bytes());
            let mut count = 0u64;
            let mut work = 0u64;
            let mut statement = db.prepare(
                "SELECT t.node_id,t.title_len,t.title_sha256,
                CASE WHEN typeof(t.title_json)='blob' AND t.title_len=length(t.title_json)
                AND length(t.title_json)<=?1 THEN t.title_json ELSE NULL END
                FROM knowledge_global_titles t JOIN knowledge_nodes n ON n.id=t.node_id
                ORDER BY n.source_graph,n.id",
            )?;
            let mut rows = statement.query([max_title_bytes as i64])?;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let length: i64 = row.get(1)?;
                let sha: Vec<u8> = row.get(2)?;
                let bytes = row
                    .get::<_, Option<Vec<u8>>>(3)?
                    .ok_or(Error::Budget("global title verify bytes"))?;
                if length < 0
                    || length as usize != bytes.len()
                    || sha.len() != 32
                    || Digest256::of_bytes(&bytes).as_bytes().as_slice() != sha
                {
                    return Err(Error::Invalid("global title verify digest"));
                }
                count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("global title verify rows"))?;
                work = work
                    .checked_add(bytes.len() as u64)
                    .ok_or(Error::Budget("global title verify work"))?;
                if count > receipt.title_count || work > max_work_bytes {
                    return Err(Error::Budget("global title verify scan"));
                }
                root_item(
                    &mut root,
                    &id,
                    &sha.try_into()
                        .map_err(|_| Error::Invalid("global title verify digest"))?,
                );
            }
            let all: u64 =
                db.query_row("SELECT count(*) FROM knowledge_global_titles", [], |r| {
                    r.get(0)
                })?;
            if count != receipt.title_count
                || all != count
                || root.finalize().to_hex() != receipt.title_root_sha256
            {
                return Err(Error::Invalid("global title verify complete root"));
            }
            Ok(())
        })
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// Each family appends unique private rows. Sort them only after the complete
/// node or relation pass, before root-dependent joins or selected sealing.
/// The disk-backed mapping avoids copying payloads or retaining all IDs in RAM.
pub fn order_native_graph_rows(
    stage: &mut KnowledgeStage<'_>,
    max_rows: u64,
    max_work_bytes: u64,
) -> Result<()> {
    let result = (|| {
        if max_rows == 0 || max_work_bytes == 0 {
            return Err(Error::Budget("native graph order limits"));
        }
        for table in ["knowledge_nodes", "knowledge_relations"] {
            let (count, bytes): (u64, u64) = stage.with_connection(WritePhase::Sort, |db| {
                Ok(db.query_row(
                    &format!(
                        "SELECT count(*),coalesce(sum(length(CAST(id AS BLOB))+16),0) FROM {table}"
                    ),
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?)
            })?;
            if count > max_rows || bytes > max_work_bytes || count > i64::MAX as u64 {
                return Err(Error::Budget("native graph order work"));
            }
            stage.charge_materialized(count, bytes)?;
            let location = if stage.payload_layout().uses_carriers() { "TEMP " } else { "" };
            stage.with_connection(WritePhase::Sort, |db| {
                let tx = db.transaction()?;
                tx.execute_batch(&format!(
                    "CREATE {location}TABLE knowledge_native_order(id TEXT PRIMARY KEY,position INTEGER NOT NULL UNIQUE) WITHOUT ROWID;
                     INSERT INTO knowledge_native_order SELECT id,row_number() OVER(ORDER BY source_graph,id)-1 FROM {table};
                     UPDATE {table} SET source_order=-source_order-1;
                     UPDATE {table} SET source_order=(SELECT position FROM knowledge_native_order o WHERE o.id={table}.id);
                     DROP TABLE knowledge_native_order;"
                ))?;
                tx.commit()?;
                Ok(())
            })?;
        }
        // Also checks dense order, so roots cannot silently bind insertion order.
        stage.core_roots()?;
        Ok(())
    })();
    if result.is_err() {
        stage.poison();
    }
    result
}

/// A selected artifact must contain neither dependency tables nor raw inputs.
pub fn clear_global_titles(stage: &mut KnowledgeStage<'_>) -> Result<()> {
    stage.with_connection(WritePhase::Finalize, |db| {
        db.execute_batch("DROP TABLE knowledge_global_titles")?;
        Ok(())
    })
}
