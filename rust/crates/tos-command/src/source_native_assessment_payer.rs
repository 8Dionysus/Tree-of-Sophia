//! A resource-only loan of the actual Assessment operation ledger. PG does
//! not have a ClaimCallBudget: this port never creates one or translates an
//! opaque PG identity into a filesystem revision. It issues no authority.
use super::OperationBudget;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use std::{cell::RefCell, mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_compiler::pwe_native::{Error, Limits, Session, SourceAdmission};

pub(crate) struct OriginalAssessmentPayer<'budget, 'cancel> {
    operation: RefCell<&'budget mut OperationBudget>,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    refusal: RefCell<Option<SourceCommandError>>,
}

/// Called once by the actual protected operation, with a source-owned lower
/// Session profile. The SAME original operation pays the full Session work
/// ceiling prospectively; Session input and holders debit it at every hook.
/// No unused work/holder reservation is refunded. There is no default profile
/// and no claim that an arbitrary requested profile leaves enough for evaluation.
pub(crate) fn with_original_pwe_session<T>(
    operation: &mut OperationBudget,
    limits: Limits,
    cancelled: &AtomicBool,
    run: impl FnOnce(&OriginalAssessmentPayer<'_, '_>, &mut Session<'_>) -> SourceCommandResult<T>,
) -> SourceCommandResult<T> {
    active(operation.deadline, cancelled)?;
    let remaining = operation.limits()?;
    if limits.deadline > operation.deadline
        || limits.max_input_bytes > remaining.max_input_bytes
        || limits.max_state_bytes > remaining.max_input_bytes
        || limits.max_work > remaining.max_work
    {
        return Err(SourceCommandError::Invalid(
            "PWE Session exceeds original Assessment remainder",
        ));
    }
    let carriers = size_of::<OriginalAssessmentPayer<'_, '_>>()
        .checked_add(size_of::<Session<'_>>())
        .and_then(|n| n.checked_add(size_of::<SourceCommandResult<T>>()))
        .and_then(|n| n.checked_add(size_of::<Error>()))
        .ok_or(SourceCommandError::Unsupported("PWE original payer carriers"))?;
    operation.charge_bytes(carriers)?;
    operation.charge_work(limits.max_work)?;
    let deadline = operation.deadline;
    let payer = OriginalAssessmentPayer {
        operation: RefCell::new(operation),
        deadline,
        cancelled,
        refusal: RefCell::new(None),
    };
    let mut session = Session::new_source(limits, cancelled, &payer)
        .map_err(|error| payer.error(error))?;
    let result = run(&payer, &mut session);
    // Check on Err too. This is a resource cutoff, not the PG owner's freshness
    // fence; the actual read epoch must additionally perform its own exit.
    let cutoff = payer.finish();
    match (result, cutoff) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

impl OriginalAssessmentPayer<'_, '_> {
    /// Short exclusive loan of the same original operation, for the maintained
    /// evaluator/worker. A Session callback cannot reenter while this loan is
    /// active; borrow refusal is explicit, never a fresh fallback ledger.
    pub(crate) fn with_operation<T>(
        &self,
        run: impl FnOnce(&mut OperationBudget) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        active(self.deadline, self.cancelled)?;
        let mut operation = self.operation.try_borrow_mut().map_err(|_| {
            SourceCommandError::Invalid("original Assessment payer already borrowed")
        })?;
        run(&mut operation)
    }

    fn debit(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        self.with_operation(|operation| operation.charge_bytes(bytes))
            .map_err(|error| {
                let mut first = self.refusal.borrow_mut();
                if first.is_none() {
                    *first = Some(error);
                }
                Error::Upstream("original Assessment payer refused")
            })
    }

    pub(crate) fn error(&self, error: Error) -> SourceCommandError {
        if let Some(original) = self.refusal.borrow_mut().take() {
            return original;
        }
        match error {
            Error::Cancelled => SourceCommandError::Denied("PWE Assessment Session cancelled"),
            Error::Deadline => SourceCommandError::Denied("PWE Assessment Session deadline"),
            Error::ResourceLimit => {
                SourceCommandError::Unsupported("PWE Assessment Session exhausted")
            }
            _ => SourceCommandError::Invalid("PWE Assessment Session operation"),
        }
    }

    fn finish(&self) -> SourceCommandResult<()> {
        if let Some(original) = self.refusal.borrow_mut().take() {
            return Err(original);
        }
        active(self.deadline, self.cancelled)
    }
}

impl SourceAdmission for OriginalAssessmentPayer<'_, '_> {
    fn inspect(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        self.debit(bytes)
    }

    fn retain(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        self.debit(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn original_assessment_session_keeps_failed_prefix_and_work_reservation() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancel = AtomicBool::new(false);
        let mut original = OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        let before_work = original.work_remaining;
        let before_bytes = original.bytes_remaining;
        let limits = Limits {
            max_input_bytes: 128,
            max_nodes: 32,
            max_work: 64,
            max_refs: 8,
            max_depth: 8,
            max_items: 8,
            max_state_bytes: 128,
            deadline,
        };
        let result: SourceCommandResult<()> = with_original_pwe_session(
            &mut original,
            limits,
            &cancel,
            |payer, session| {
                assert!(session.has_source_payer(payer));
                session.admit_source_prefix(7, 2).map_err(|e| payer.error(e))?;
                session.retain_source_state(11).map_err(|e| payer.error(e))?;
                Err(SourceCommandError::Denied("body refused"))
            },
        );
        assert!(matches!(result, Err(SourceCommandError::Denied("body refused"))));
        assert_eq!(original.work_remaining, before_work - limits.max_work);
        assert!(original.bytes_remaining < before_bytes - 18);
        // An impossible profile refuses against the ACTUAL remainder; it does
        // not construct another budget or reset the already consumed prefix.
        let spent_work = original.work_remaining;
        let refused: SourceCommandResult<()> = with_original_pwe_session(
            &mut original,
            Limits { max_work: before_work, ..limits },
            &cancel,
            |_, _| panic!("unfunded source Session must not start"),
        );
        assert!(refused.is_err());
        assert_eq!(original.work_remaining, spent_work);
    }
}
