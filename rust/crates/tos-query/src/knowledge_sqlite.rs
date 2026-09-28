//! Native gram-stat seek on CMP's already-cold-verified pinned knowledge model.
//! No selected path, rights decision, or source-payload authority is accepted.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use rusqlite::{ErrorCode, OptionalExtension, params};
use tos_compiler::{
    MAX_POSTING_DELTA_BYTES, MAX_POSTINGS_PER_BLOCK, VerifiedKnowledgeModel, decode_posting_block,
};
use tos_foundation::Digest256;

use crate::search_candidate::{
    CandidateReadBudget, CandidateReadCharge, SearchCandidateModel, SelectedSearchCandidate,
};
use crate::search_index::{
    GramSeekCharge, GramStat, PostingPage, SearchGramModel, SearchPostingModel,
};
use crate::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode};

fn error(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}

fn sql_error(reason: rusqlite::Error) -> SearchV2Error {
    if matches!(reason, rusqlite::Error::SqliteFailure(failure, _) if failure.code == ErrorCode::OperationInterrupted)
    {
        error(
            SearchV2ErrorCode::BudgetExceeded,
            "knowledge gram-stat VM budget exceeded",
        )
    } else {
        error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected gram-stat seek failed",
        )
    }
}

impl SearchGramModel for VerifiedKnowledgeModel<'_> {
    fn gram_stat(
        &mut self,
        kind: SearchKind,
        gram: &str,
        max_vm_steps: u64,
        max_rows: u64,
        max_decoded_bytes: u64,
    ) -> Result<GramStat, SearchV2Error> {
        if max_vm_steps == 0 || max_rows == 0 || max_decoded_bytes < 8 || gram.chars().count() != 3
        {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "gram-stat seek admission unavailable",
            ));
        }
        self.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "selected knowledge model pin changed",
            )
        })?;
        let kind = match kind {
            SearchKind::Nodes => "nodes",
            SearchKind::Relations => "relations",
        };
        let connection = self.connection();
        let count = Arc::new(AtomicU64::new(0));
        let observed = Arc::clone(&count);
        connection.progress_handler(
            1,
            Some(move || observed.fetch_add(1, Ordering::Relaxed) >= max_vm_steps),
        );
        let selected = (|| {
            let mut statement = connection
                .prepare_cached(
                    "SELECT postings FROM search_gram_stats WHERE kind=?1 AND n=3 AND gram=?2",
                )
                .map_err(sql_error)?;
            statement
                .query_row(params![kind, gram.as_bytes()], |row| row.get::<_, i64>(0))
                .optional()
                .map_err(sql_error)
        })();
        connection.progress_handler(0, None::<fn() -> bool>);
        let steps = count.load(Ordering::Relaxed);
        if steps > max_vm_steps {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "gram-stat seek exceeded VM budget",
            ));
        }
        let selected = selected?;
        if selected.is_some_and(|count| count < 0) {
            return Err(error(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "selected posting count is negative",
            ));
        }
        let rows = u64::from(selected.is_some());
        Ok(GramStat {
            postings: selected.map(|value| value as u64),
            charged: GramSeekCharge {
                lookups: 1,
                vm_steps: steps,
                rows,
                decoded_bytes: rows * 8,
            },
        })
    }
}

impl SearchPostingModel for VerifiedKnowledgeModel<'_> {
    fn seek_postings(
        &mut self,
        kind: SearchKind,
        gram: &str,
        after: Option<u64>,
        max_rows: usize,
        max_vm_steps: u64,
        max_decoded_bytes: u64,
    ) -> Result<PostingPage, SearchV2Error> {
        if max_rows == 0
            || max_rows > 1024
            || max_vm_steps == 0
            || (max_rows as u64)
                .checked_mul(8)
                .is_none_or(|bytes| bytes > max_decoded_bytes)
            || after.is_some_and(|position| position > i64::MAX as u64)
            || gram.chars().count() != 3
        {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "posting page admission unavailable",
            ));
        }
        self.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "selected knowledge model pin changed",
            )
        })?;
        let kind = match kind {
            SearchKind::Nodes => "nodes",
            SearchKind::Relations => "relations",
        };
        let connection = self.connection();
        let count = Arc::new(AtomicU64::new(0));
        let observed = Arc::clone(&count);
        connection.progress_handler(
            1,
            Some(move || observed.fetch_add(1, Ordering::Relaxed) >= max_vm_steps),
        );
        let selected =
            (|| {
                // The final CASE is the transfer bound: no BLOB is materialized by
                // row.get until its SQLite type and declared length are admitted.
                // One block always contributes at least one position after `after`,
                // so max_rows is also a bound on consulted block rows.
                let mut statement = connection.prepare_cached(
                "SELECT CASE WHEN typeof(first_position)='integer' THEN first_position END, \
                        CASE WHEN typeof(last_position)='integer' THEN last_position END, \
                        CASE WHEN typeof(postings)='integer' THEN postings END, \
                        CASE WHEN typeof(deltas)='blob' THEN 1 ELSE 0 END, length(deltas), \
                        CASE WHEN typeof(deltas)='blob' AND length(deltas)<=?5 \
                             THEN deltas END \
                 FROM search_posting_blocks \
                 WHERE kind=?1 AND n=3 AND gram=?2 AND last_position>?3 \
                 ORDER BY last_position LIMIT ?4",
            ).map_err(sql_error)?;
                let mut rows = statement
                    .query(params![
                        kind,
                        gram.as_bytes(),
                        after.map_or(-1, |value| value as i64),
                        max_rows as i64,
                        max_decoded_bytes.min(MAX_POSTING_DELTA_BYTES as u64) as i64,
                    ])
                    .map_err(sql_error)?;
                let mut positions = Vec::with_capacity(max_rows);
                let mut decoded_bytes = 0u64;
                let mut previous_block_last = None;
                let mut exhausted = true;
                while let Some(row) = rows.next().map_err(sql_error)? {
                    let first: i64 = row
                        .get::<_, Option<i64>>(0)
                        .map_err(sql_error)?
                        .ok_or_else(|| {
                            error(
                                SearchV2ErrorCode::CorruptSelectedCarrier,
                                "selected posting first position type invalid",
                            )
                        })?;
                    let last: i64 = row
                        .get::<_, Option<i64>>(1)
                        .map_err(sql_error)?
                        .ok_or_else(|| {
                            error(
                                SearchV2ErrorCode::CorruptSelectedCarrier,
                                "selected posting last position type invalid",
                            )
                        })?;
                    let postings: i64 = row
                        .get::<_, Option<i64>>(2)
                        .map_err(sql_error)?
                        .ok_or_else(|| {
                            error(
                                SearchV2ErrorCode::CorruptSelectedCarrier,
                                "selected posting count type invalid",
                            )
                        })?;
                    if first < 0
                        || last < first
                        || postings < 1
                        || postings > MAX_POSTINGS_PER_BLOCK as i64
                        || previous_block_last.is_some_and(|prior| first <= prior)
                        || after.is_some_and(|prior| last as u64 <= prior)
                    {
                        return Err(error(
                            SearchV2ErrorCode::CorruptSelectedCarrier,
                            "selected posting block bounds invalid",
                        ));
                    }
                    let delta_is_blob: i64 = row.get(3).map_err(sql_error)?;
                    let delta_len: i64 = row
                        .get::<_, Option<i64>>(4)
                        .map_err(sql_error)?
                        .ok_or_else(|| {
                            error(
                                SearchV2ErrorCode::CorruptSelectedCarrier,
                                "selected posting delta length invalid",
                            )
                        })?;
                    if delta_is_blob != 1
                        || delta_len < 0
                        || delta_len as u64 > MAX_POSTING_DELTA_BYTES as u64
                    {
                        return Err(error(
                            SearchV2ErrorCode::CorruptSelectedCarrier,
                            "selected posting delta carrier invalid",
                        ));
                    }
                    // Five selected scalar fields (including the type/length guards),
                    // the bounded encoded carrier, and every decoded position.
                    let field_bytes = 40u64
                        .checked_add(delta_len as u64)
                        .and_then(|bytes| bytes.checked_add((postings as u64).checked_mul(8)?))
                        .ok_or_else(|| {
                            error(
                                SearchV2ErrorCode::BudgetExceeded,
                                "posting block byte charge overflow",
                            )
                        })?;
                    decoded_bytes = decoded_bytes.checked_add(field_bytes).ok_or_else(|| {
                        error(
                            SearchV2ErrorCode::BudgetExceeded,
                            "posting block byte charge overflow",
                        )
                    })?;
                    if decoded_bytes > max_decoded_bytes {
                        return Err(error(
                            SearchV2ErrorCode::BudgetExceeded,
                            "posting block decoded byte budget exceeded",
                        ));
                    }
                    let deltas: Vec<u8> = row
                        .get::<_, Option<Vec<u8>>>(5)
                        .map_err(sql_error)?
                        .ok_or_else(|| {
                            error(
                                SearchV2ErrorCode::CorruptSelectedCarrier,
                                "selected posting deltas unavailable",
                            )
                        })?;
                    let block =
                        decode_posting_block(first as u64, last as u64, postings as u16, &deltas)
                            .map_err(|reason| match reason {
                            tos_compiler::Error::Budget(_) => error(
                                SearchV2ErrorCode::BudgetExceeded,
                                "posting block decode budget exceeded",
                            ),
                            _ => error(
                                SearchV2ErrorCode::CorruptSelectedCarrier,
                                "selected posting block invalid",
                            ),
                        })?;
                    previous_block_last = Some(last);
                    for position in block {
                        if after.is_some_and(|prior| position <= prior) {
                            continue;
                        }
                        positions.push(position);
                        if positions.len() == max_rows {
                            exhausted = false;
                            break;
                        }
                    }
                    if !exhausted {
                        break;
                    }
                }
                Ok((positions, exhausted, decoded_bytes))
            })();
        connection.progress_handler(0, None::<fn() -> bool>);
        let steps = count.load(Ordering::Relaxed);
        if steps > max_vm_steps {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "posting seek exceeded VM budget",
            ));
        }
        let (positions, exhausted, decoded_bytes) = selected?;
        let rows = positions.len() as u64;
        Ok(PostingPage {
            exhausted,
            positions,
            charged: GramSeekCharge {
                lookups: 1,
                vm_steps: steps,
                rows,
                decoded_bytes,
            },
        })
    }
}

fn raw_digest(value: &[u8]) -> Result<Digest256, SearchV2Error> {
    if value.len() != 32 {
        return Err(error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected digest width differs",
        ));
    }
    let mut hex = String::with_capacity(64);
    use std::fmt::Write;
    for byte in value {
        write!(&mut hex, "{byte:02x}").expect("formatting into String");
    }
    Digest256::from_hex(&hex).map_err(|_| {
        error(
            SearchV2ErrorCode::CorruptSelectedCarrier,
            "selected raw digest invalid",
        )
    })
}

impl SearchCandidateModel for VerifiedKnowledgeModel<'_> {
    fn exact_candidate(
        &mut self,
        kind: SearchKind,
        position: u64,
        budget: CandidateReadBudget,
    ) -> Result<(SelectedSearchCandidate, CandidateReadCharge), SearchV2Error> {
        let field_bytes = (budget.max_field_bytes as u64)
            .checked_mul(8)
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "candidate field admission overflow",
                )
            })?;
        let worst_bytes = field_bytes
            .checked_add(budget.max_payload_bytes as u64)
            .and_then(|value| value.checked_add(80))
            .ok_or_else(|| {
                error(
                    SearchV2ErrorCode::BudgetExceeded,
                    "candidate byte admission overflow",
                )
            })?;
        if position > i64::MAX as u64
            || budget.max_vm_steps == 0
            || budget.max_payload_bytes == 0
            || budget.max_field_bytes == 0
            || budget.max_document_chars == 0
            || budget.max_decoded_bytes < worst_bytes
            || budget.max_payload_bytes > i64::MAX as usize
            || budget.max_field_bytes > i64::MAX as usize
            || budget.max_document_chars > i64::MAX as u64
        {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "exact candidate admission unavailable",
            ));
        }
        self.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "selected knowledge model pin changed",
            )
        })?;
        let (kind_name, table) = match kind {
            SearchKind::Nodes => ("nodes", "knowledge_nodes"),
            SearchKind::Relations => ("relations", "knowledge_relations"),
        };
        let sql = format!(
            "SELECT
             CASE WHEN typeof(d.id)='text' AND length(CAST(d.id AS BLOB))<=?3 THEN d.id END,
             CASE WHEN typeof(d.source_graph)='text' AND length(CAST(d.source_graph AS BLOB))<=?3 THEN d.source_graph END,
             CASE WHEN typeof(d.kind_id)='text' AND length(CAST(d.kind_id AS BLOB))<=?3 THEN d.kind_id END,
             CASE WHEN typeof(d.predicate_id)='text' AND length(CAST(d.predicate_id AS BLOB))<=?3 THEN d.predicate_id END,
             CASE WHEN typeof(d.id_lower)='text' AND length(CAST(d.id_lower AS BLOB))<=?3 THEN d.id_lower END,
             CASE WHEN typeof(d.native_id_lower)='text' AND length(CAST(d.native_id_lower AS BLOB))<=?3 THEN d.native_id_lower END,
             CASE WHEN typeof(d.identity_values)='text' AND length(CAST(d.identity_values AS BLOB))<=?3 THEN d.identity_values END,
             CASE WHEN typeof(d.visible_values)='text' AND length(CAST(d.visible_values AS BLOB))<=?3 THEN d.visible_values END,
             CASE WHEN typeof(d.document_chars)='integer' AND d.document_chars>=0 AND d.document_chars<=?5 THEN d.document_chars END,
             CASE WHEN typeof(d.document_digest)='blob' AND length(d.document_digest)=32 THEN d.document_digest END,
             CASE WHEN typeof(c.payload_len)='integer' AND c.payload_len>=0 AND c.payload_len<=?4 THEN c.payload_len END,
             CASE WHEN typeof(c.payload_sha256)='blob' AND length(c.payload_sha256)=32 THEN c.payload_sha256 END,
             CASE WHEN typeof(c.payload)='blob' AND c.payload_len>=0 AND c.payload_len<=?4 AND length(c.payload)=c.payload_len THEN c.payload END
             FROM search_documents d JOIN {table} c ON c.source_order=d.position
             WHERE d.kind=?1 AND d.position=?2"
        );
        let connection = self.connection();
        let count = Arc::new(AtomicU64::new(0));
        let observed = Arc::clone(&count);
        let max_vm_steps = budget.max_vm_steps;
        connection.progress_handler(
            1,
            Some(move || observed.fetch_add(1, Ordering::Relaxed) >= max_vm_steps),
        );
        let selected = (|| {
            let mut statement = connection.prepare_cached(&sql).map_err(sql_error)?;
            statement
                .query_row(
                    params![
                        kind_name,
                        position as i64,
                        budget.max_field_bytes as i64,
                        budget.max_payload_bytes as i64,
                        budget.max_document_chars as i64
                    ],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, Option<i64>>(8)?,
                            row.get::<_, Option<Vec<u8>>>(9)?,
                            row.get::<_, Option<i64>>(10)?,
                            row.get::<_, Option<Vec<u8>>>(11)?,
                            row.get::<_, Option<Vec<u8>>>(12)?,
                        ))
                    },
                )
                .optional()
                .map_err(sql_error)
        })();
        connection.progress_handler(0, None::<fn() -> bool>);
        let steps = count.load(Ordering::Relaxed);
        if steps > budget.max_vm_steps {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "exact candidate VM budget exceeded",
            ));
        }
        self.check_pin().map_err(|_| {
            error(
                SearchV2ErrorCode::StaleSelection,
                "selected knowledge model pin changed",
            )
        })?;
        let Some((
            id,
            source_graph,
            kind_id,
            predicate_id,
            id_lower,
            native_id_lower,
            identity_values,
            visible_values,
            document_chars,
            document_digest,
            payload_len,
            payload_sha256,
            payload,
        )) = selected?
        else {
            return Err(error(
                SearchV2ErrorCode::IndexIncomplete,
                "selected posting has no document row",
            ));
        };
        let (
            Some(id),
            Some(source_graph),
            Some(kind_id),
            Some(predicate_id),
            Some(id_lower),
            Some(native_id_lower),
            Some(identity_values),
            Some(visible_values),
            Some(document_chars),
            Some(document_digest),
            Some(payload_len),
            Some(payload_sha256),
            Some(payload),
        ) = (
            id,
            source_graph,
            kind_id,
            predicate_id,
            id_lower,
            native_id_lower,
            identity_values,
            visible_values,
            document_chars,
            document_digest,
            payload_len,
            payload_sha256,
            payload,
        )
        else {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "selected candidate exceeds SQL pretransfer cap",
            ));
        };
        if payload.len() as i64 != payload_len {
            return Err(error(
                SearchV2ErrorCode::CorruptSelectedCarrier,
                "selected payload length differs",
            ));
        }
        let decoded_bytes = [
            id.len(),
            source_graph.len(),
            kind_id.len(),
            predicate_id.len(),
            id_lower.len(),
            native_id_lower.len(),
            identity_values.len(),
            visible_values.len(),
            document_digest.len(),
            payload_sha256.len(),
            payload.len(),
        ]
        .into_iter()
        .try_fold(16u64, |sum, size| sum.checked_add(size as u64))
        .ok_or_else(|| {
            error(
                SearchV2ErrorCode::BudgetExceeded,
                "candidate decoded byte overflow",
            )
        })?;
        if decoded_bytes > budget.max_decoded_bytes {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "candidate decoded bytes exceeded",
            ));
        }
        let candidate = SelectedSearchCandidate {
            kind,
            position,
            id,
            source_graph,
            kind_id,
            predicate_id,
            id_lower,
            native_id_lower,
            identity_values,
            visible_values,
            document_chars: document_chars as u64,
            document_digest: raw_digest(&document_digest)?,
            payload_sha256: raw_digest(&payload_sha256)?,
            payload,
        };
        Ok((
            candidate,
            CandidateReadCharge {
                vm_steps: steps,
                rows: 1,
                decoded_bytes,
            },
        ))
    }
}
