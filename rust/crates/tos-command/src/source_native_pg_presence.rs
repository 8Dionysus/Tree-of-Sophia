//! Physical point-presence navigation for the selected PG metadata reader.
//! All logical query DATA is priced before its statement. No source payload,
//! semantic owner, publication virtual carrier or application grant is issued.
//! Driver/error/network/kernel and complete activation funding remain separate.
use super::{SourceRevisionReadSession, finish_source_revision_epoch, query_data};
use crate::PredicateRead;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_native_cli::private_assessment::OperationBudget;
use crate::{DurableError, DurableResult};
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::RelativePath;

const EXISTS: &str = "SELECT EXISTS(SELECT 1 FROM cmd2_current WHERE domain=$1 AND subject=$2)";
const SETUP: &str = "SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'";

fn overflow() -> DurableError {
    DurableError::Source(SourceCommandError::Unsupported(
        "PG presence data quote overflow",
    ))
}
fn debit(budget: &mut OperationBudget, bytes: usize, work: usize) -> DurableResult<()> {
    budget.charge_work(work).map_err(DurableError::Source)?;
    budget.charge_bytes(bytes).map_err(DurableError::Source)
}

impl SourceRevisionReadSession<'_> {
    /// Physical existence only. A true result must subsequently be bound by
    /// the actual admitted member read; it is not exact content/owner evidence.
    /// False records the actual absence under the held source-head/read epoch.
    /// It never substitutes a missing selected byte buffer for proven absence.
    pub(crate) fn has_current_metadata_with_data_admission(
        &mut self,
        path: &RelativePath,
        budget: &mut OperationBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<bool> {
        if !path.as_str().starts_with("ToS/") {
            return Err(DurableError::Refused(
                "PG admitted presence outside authored namespace",
            ));
        }
        if crate::source_work_transaction::is_selected_publication_control_path(path.as_str())
            || crate::source_work_transaction::is_selected_publication_journal_metadata_path(
                path.as_str(),
            )
        {
            return Err(DurableError::Refused(
                "PG physical presence cannot select publication virtual carrier",
            ));
        }
        let result = (|| {
            self.verify_fresh_with_query_data_admission(budget, deadline, cancelled)?;
            query_data::begin(budget, deadline, cancelled).map_err(DurableError::Source)?;
            let mut tx = self.coordinator.client.transaction()?;
            query_data::statement(
                budget,
                SETUP,
                &[],
                query_data::RowShape::None,
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?;
            tx.batch_execute(SETUP)?;
            // Reserve each real Row/result/parameter carrier before the call;
            // foreign driver allocation/error capacities are not inferred here.
            let carriers = size_of::<postgres::Row>()
                .checked_add(size_of::<Result<postgres::Row, postgres::Error>>())
                .and_then(|n| n.checked_add(size_of::<DurableResult<bool>>()))
                .and_then(|n| n.checked_add(size_of::<[&(dyn postgres::types::ToSql + Sync); 2]>()))
                .ok_or_else(overflow)?;
            debit(budget, carriers, 1)?;
            query_data::statement(
                budget,
                EXISTS,
                &[self.selection.cohort.domain.len(), path.as_str().len()],
                query_data::RowShape::Presence,
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?;
            let row = tx.query_one(EXISTS, &[&self.selection.cohort.domain, &path.as_str()])?;
            Self::charge_work(&mut self.work.metadata_member_lookups, 1)?;
            let present: bool = row.get(0);
            tx.commit()?;
            if !present {
                self.record_admitted_absence(path.as_str(), budget)?;
            }
            Ok(present)
        })();
        // Real paid owner exit on ordinary Ok AND Err, including a refused
        // query/absence append. Sticky exhaustion may prevent physical SQL.
        finish_source_revision_epoch(result, || {
            self.verify_fresh_with_query_data_admission(budget, deadline, cancelled)
        })
    }

    fn record_admitted_absence(
        &mut self,
        path: &str,
        budget: &mut OperationBudget,
    ) -> DurableResult<()> {
        // Same namespace/key law and duplicate suppression as record_absent.
        // Pay actual operand lengths BEFORE each comparison, including misses.
        for read in &self.reads.reads {
            debit(budget, 0, 1)?;
            if let PredicateRead::Absent { namespace, key } = read {
                let bytes = namespace
                    .len()
                    .checked_add("source".len())
                    .and_then(|n| n.checked_add(key.len()))
                    .and_then(|n| n.checked_add(path.len()))
                    .ok_or_else(overflow)?;
                debit(budget, 0, bytes.checked_add(2).ok_or_else(overflow)?)?;
                if namespace == "source" && key == path {
                    return Ok(());
                }
            }
        }
        let vec = &self.reads.reads;
        let needed = vec.len().checked_add(1).ok_or_else(overflow)?;
        let (allocation, relocation) = if needed > vec.capacity() {
            // Pinned Rust1.98.1 Vec<PredicateRead>::push: non-u8 RawVec minimum4
            // and doubling. The old backing remains live; quote full new layout.
            let capacity = vec
                .capacity()
                .checked_mul(2)
                .ok_or_else(overflow)?
                .max(needed)
                .max(4);
            (
                capacity
                    .checked_mul(size_of::<PredicateRead>())
                    .ok_or_else(overflow)?,
                vec.capacity()
                    .checked_mul(size_of::<PredicateRead>())
                    .ok_or_else(overflow)?,
            )
        } else {
            (0, 0)
        };
        let text = path
            .len()
            .checked_add("source".len())
            .ok_or_else(overflow)?;
        let holders = allocation
            .checked_add(text)
            .and_then(|n| n.checked_add(size_of::<PredicateRead>()))
            .and_then(|n| n.checked_add(size_of::<SourceCommandResult<()>>()))
            .ok_or_else(overflow)?;
        debit(
            budget,
            holders,
            relocation
                .checked_add(text)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(overflow)?,
        )?;
        self.reads.reads.push(PredicateRead::Absent {
            namespace: "source".to_owned(),
            key: path.to_owned(),
        });
        Ok(())
    }
}
