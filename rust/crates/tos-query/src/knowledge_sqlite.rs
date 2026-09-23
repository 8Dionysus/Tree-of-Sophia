//! Native gram-stat seek on CMP's already-cold-verified pinned knowledge model.
//! No selected path, rights decision, or source-payload authority is accepted.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use rusqlite::{ErrorCode, OptionalExtension, params};
use tos_compiler::VerifiedKnowledgeModel;

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
        let selected = (|| {
            let mut statement = connection.prepare_cached(
                "SELECT position FROM search_grams WHERE kind=?1 AND n=3 AND gram=?2 AND position>?3 ORDER BY position LIMIT ?4",
            ).map_err(sql_error)?;
            let mut rows = statement
                .query(params![
                    kind,
                    gram.as_bytes(),
                    after.map_or(-1, |value| value as i64),
                    max_rows as i64
                ])
                .map_err(sql_error)?;
            let mut positions = Vec::with_capacity(max_rows);
            while let Some(row) = rows.next().map_err(sql_error)? {
                let position: i64 = row.get(0).map_err(sql_error)?;
                if position < 0 {
                    return Err(error(
                        SearchV2ErrorCode::CorruptSelectedCarrier,
                        "selected posting position is negative",
                    ));
                }
                positions.push(position as u64);
            }
            Ok(positions)
        })();
        connection.progress_handler(0, None::<fn() -> bool>);
        let steps = count.load(Ordering::Relaxed);
        if steps > max_vm_steps {
            return Err(error(
                SearchV2ErrorCode::BudgetExceeded,
                "posting seek exceeded VM budget",
            ));
        }
        let positions = selected?;
        let rows = positions.len() as u64;
        Ok(PostingPage {
            exhausted: positions.len() < max_rows,
            positions,
            charged: GramSeekCharge {
                lookups: 1,
                vm_steps: steps,
                rows,
                decoded_bytes: rows * 8,
            },
        })
    }
}
