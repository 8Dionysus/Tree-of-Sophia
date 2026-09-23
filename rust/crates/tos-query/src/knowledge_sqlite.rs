//! Native gram-stat seek on CMP's already-cold-verified pinned knowledge model.
//! No selected path, rights decision, or source-payload authority is accepted.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use rusqlite::{ErrorCode, OptionalExtension, params};
use tos_compiler::VerifiedKnowledgeModel;

use crate::search_index::{GramSeekCharge, GramStat, SearchGramModel};
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
