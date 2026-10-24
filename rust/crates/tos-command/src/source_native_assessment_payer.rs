//! A resource-only loan of the actual Assessment operation ledger. PG does
//! not have a ClaimCallBudget: this port never creates one or translates an
//! opaque PG identity into a filesystem revision. It issues no authority.
use super::OperationBudget;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use std::{
    cell::{Cell, RefCell},
    mem::size_of,
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_compiler::pwe_native::{Error, Limits, Session, SourceAdmission};

#[path = "source_original_read_session.rs"]
pub(crate) mod immutable_read;

pub(crate) struct OriginalAssessmentPayer<'budget, 'cancel> {
    operation: RefCell<&'budget mut OperationBudget>,
    deadline: Instant,
    cancelled: &'cancel AtomicBool,
    refusal: RefCell<Option<SourceCommandError>>,
    failed: Cell<bool>,
}

/// Facts from the cumulative operation, NOT the smaller per-report input
/// ceiling returned by OperationBudget::limits for the assessment engine.
#[derive(Clone, Copy)]
pub(crate) struct AssessmentRemainder {
    pub(crate) bytes: usize,
    pub(crate) work: usize,
    pub(crate) deadline: Instant,
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
    operation.limits()?;
    if limits.deadline > operation.deadline
        || limits.max_input_bytes > operation.bytes_remaining
        || limits.max_state_bytes > operation.bytes_remaining
        || limits.max_work > operation.work_remaining
    {
        return Err(SourceCommandError::Invalid(
            "PWE Session exceeds original Assessment remainder",
        ));
    }
    let carriers = size_of::<OriginalAssessmentPayer<'_, '_>>()
        .checked_add(size_of::<Session<'_>>())
        .and_then(|n| n.checked_add(size_of::<SourceCommandResult<T>>()))
        .and_then(|n| n.checked_add(size_of::<Error>()))
        .ok_or(SourceCommandError::Unsupported(
            "PWE original payer carriers",
        ))?;
    operation.charge_bytes(carriers)?;
    operation.charge_work(limits.max_work)?;
    let deadline = operation.deadline;
    let payer = OriginalAssessmentPayer {
        operation: RefCell::new(operation),
        deadline,
        cancelled,
        refusal: RefCell::new(None),
        failed: Cell::new(false),
    };
    let mut session =
        Session::new_source(limits, cancelled, &payer).map_err(|error| payer.error(error))?;
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
    /// Native frame/codec work outside the pure Session has its own
    /// prospective debit on the same operation. It does not spend a new Claim
    /// allowance or borrow work back from the prepaid pure Session envelope.
    pub(crate) fn inspect(&self, bytes: usize, work: usize) -> SourceCommandResult<()> {
        self.with_operation(|operation| {
            operation.charge_work(work)?;
            operation.charge_bytes(bytes)
        })
    }

    /// Pay one actual STO event's source measures on the SAME source Session.
    /// This is only the currency mapping, not a source/rights grant or a native
    /// fit receipt. The concrete caller must separately prepay its own incoming
    /// captures, activation/control/Result/Try and dispatch work before entering
    /// the lower port. SQL/driver/metadata/native outer costs are not priced here.
    /// No callback I/O, FD cloning, decoder, second ledger or refund is involved.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn charge_sto_source_measures(
        &self,
        session: &mut Session<'_>,
        requested: u64,
        new_owned: usize,
        workspace: usize,
        byte_work: u64,
        native_units: u64,
    ) -> SourceCommandResult<()> {
        if !session.has_source_payer(self) {
            return Err(SourceCommandError::Conflict(
                "STO source Session uses a different original payer",
            ));
        }
        // Also reject an already-sticky original resource refusal, including
        // an event with all zero measures. This short loan ends before hooks.
        self.with_operation(|_| Ok(()))?;
        let requested = usize::try_from(requested).map_err(|_| self.error(Error::ResourceLimit))?;
        let retained = new_owned
            .checked_add(workspace)
            .ok_or_else(|| self.error(Error::ResourceLimit))?;
        let work = byte_work
            .checked_add(native_units)
            .and_then(|work| usize::try_from(work).ok())
            .ok_or_else(|| self.error(Error::ResourceLimit))?;
        // Every event is additive. In particular new_owned is NOT a high-water
        // observation; repeated allocation events each retain their full size.
        // A later refusal never restores an earlier successful debit.
        session
            .retain_source_state(retained)
            .map_err(|error| self.error(error))?;
        session
            .admit_source_prefix(requested, 0)
            .map_err(|error| self.error(error))?;
        session
            .admit_source_work(work)
            .map_err(|error| self.error(error))
    }

    /// Concrete current PG->STO source-measure bridge, using the ACTUAL held
    /// read epoch and original source Session. This grants no source authority.
    /// Its enclosing protected caller still owes query/driver/control/metadata
    /// funding and entry/exit freshness fences on ordinary Ok and Err paths.
    /// Only the lower STO events are mapped here; observations stay with source.
    pub(crate) fn read_current_member_sto_source(
        &self,
        session: &mut Session<'_>,
        source: &mut crate::source_cohort::revision::SourceRevisionReadSession<'_>,
        selected: &crate::source_cohort::revision::PgRecordRevisionIdentity,
        path: &tos_foundation::RelativePath,
        max_bytes: u64,
    ) -> crate::DurableResult<crate::source_cohort::ManagedCurrentMember> {
        if !session.has_source_payer(self) {
            return Err(crate::DurableError::Source(SourceCommandError::Conflict(
                "STO source Session uses a different original payer",
            )));
        }
        self.with_operation(|_| Ok(()))
            .map_err(crate::DurableError::Source)?;
        let remaining = session
            .source_allowance()
            .map_err(|error| crate::DurableError::Source(self.error(error)))?;
        if remaining.work == 0 || remaining.input_bytes == 0 || remaining.state_bytes == 0 {
            return Err(crate::DurableError::Source(
                self.error(Error::ResourceLimit),
            ));
        }
        // End this original operation loan before any Session source hook.
        // The owner identity hashes domain/job bytes; it is not a free copy.
        let actual = self
            .with_operation(|budget| {
                source.selection().identity_with_data_admission(
                    budget,
                    self.deadline,
                    self.cancelled,
                )
            })
            .map_err(crate::DurableError::Source)?;
        if actual != *selected {
            return Err(crate::DurableError::Conflict(
                "STO source read epoch differs from protected selection",
            ));
        }
        let result = match source.read_current_member_with_sto_admission(
            path,
            max_bytes,
            self.deadline,
            self.cancelled,
            &mut |requested, owned, workspace, byte_work, units| {
                self.charge_sto_source_measures(
                    session, requested, owned, workspace, byte_work, units,
                )
            },
        ) {
            Err(original) => Err(crate::DurableError::Source(original)),
            Ok(source) => source,
        };
        // Recheck this same Session on BOTH outcomes without converting a
        // failed source read into success or replacing its original cause.
        // This is a resource cutoff, NOT the caller-owned PG freshness fence.
        let cutoff = session
            .source_allowance()
            .map(|_| ())
            .map_err(|error| crate::DurableError::Source(self.error(error)));
        match (result, cutoff) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(member), Ok(())) => Ok(member),
        }
    }

    /// Retained native frame/codec holders consume the original operation's
    /// cumulative byte currency. This is not a physical allocator/RSS theorem.
    pub(crate) fn reserve(&self, bytes: usize) -> SourceCommandResult<()> {
        self.with_operation(|operation| operation.charge_bytes(bytes))
    }

    pub(crate) fn remaining(&self) -> SourceCommandResult<AssessmentRemainder> {
        self.with_operation(|operation| {
            Ok(AssessmentRemainder {
                bytes: operation.bytes_remaining,
                work: operation.work_remaining,
                deadline: operation.deadline,
            })
        })
    }

    /// Short exclusive loan of the same original operation, for the maintained
    /// evaluator/worker. A Session callback cannot reenter while this loan is
    /// active; borrow refusal is explicit, never a fresh fallback ledger.
    pub(crate) fn with_operation<T>(
        &self,
        run: impl FnOnce(&mut OperationBudget) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        active(self.deadline, self.cancelled).map_err(|error| {
            self.failed.set(true);
            error
        })?;
        let mut operation = self.operation.try_borrow_mut().map_err(|_| {
            self.failed.set(true);
            SourceCommandError::Invalid("original Assessment payer already borrowed")
        })?;
        if self.failed.get() || operation.resource_refused.get() {
            self.failed.set(true);
            return Err(SourceCommandError::Unsupported(
                "original Assessment payer resource refusal persists",
            ));
        }
        let result = run(&mut operation);
        if self.failed.get() || operation.resource_refused.get() {
            self.failed.set(true);
            return match result {
                Err(error) => Err(error),
                Ok(_) => Err(SourceCommandError::Unsupported(
                    "original Assessment payer resource refusal persists",
                )),
            };
        }
        result
    }

    fn debit(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        self.with_operation(|operation| operation.charge_bytes(bytes))
            .map_err(|error| {
                self.failed.set(true);
                let mut first = self.refusal.borrow_mut();
                if first.is_none() {
                    *first = Some(error);
                }
                Error::Upstream("original Assessment payer refused")
            })
    }

    pub(crate) fn error(&self, error: Error) -> SourceCommandError {
        if matches!(
            error,
            Error::Cancelled | Error::Deadline | Error::ResourceLimit
        ) {
            self.failed.set(true);
        }
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
        if self.failed.get()
            || self
                .operation
                .try_borrow()
                .map_or(true, |operation| operation.resource_refused.get())
        {
            return Err(SourceCommandError::Unsupported(
                "original Assessment payer resource refusal persists",
            ));
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
    fn swallowed_mapped_or_loan_resource_refusal_cannot_finish_successfully() {
        let cancel = AtomicBool::new(false);
        for lane in 0..5 {
            // Separate invocations, not a replacement ledger inside a callback.
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut original = OperationBudget::new(
                &serde_json::json!({"budgets":{
                    "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
                }}),
                deadline,
            )
            .unwrap();
            let before_work = original.work_remaining;
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
            let result = with_original_pwe_session(&mut original, limits, &cancel, |payer, _| {
                match lane {
                    0 | 3 => {
                        let error = SourceAdmission::inspect(payer, usize::MAX).unwrap_err();
                        let mapped = payer.error(error); // takes the owned first error
                        assert!(matches!(
                            &mapped,
                            SourceCommandError::Invalid("assessment cumulative byte budget")
                        ));
                        if lane == 3 {
                            return Err(mapped);
                        }
                        drop(mapped); // exact reported bypass: swallow then Ok
                    }
                    1 => {
                        let refused = payer.with_operation(|budget| {
                            let _ = budget.charge_work(usize::MAX);
                            Ok(()) // swallow inside the short original loan
                        });
                        assert!(refused.is_err());
                        drop(refused); // swallowing that refusal must not clear it
                    }
                    4 => {
                        let refused = payer.with_operation(|_| {
                            // Admission cannot reenter the exclusive original loan.
                            let mapped =
                                payer.error(SourceAdmission::inspect(payer, 1).unwrap_err());
                            drop(mapped);
                            Ok(())
                        });
                        assert!(refused.is_err());
                        drop(refused);
                    }
                    _ => {
                        let negative: SourceCommandResult<()> = payer.with_operation(|_| {
                            Err(SourceCommandError::Denied("subject-level negative"))
                        });
                        assert!(negative.is_err()); // no resource debit failed
                    }
                }
                Ok(())
            });
            if lane == 2 {
                assert!(
                    result.is_ok(),
                    "handled subject result is not resource failure"
                );
            } else if lane == 3 {
                assert!(
                    matches!(
                        result,
                        Err(SourceCommandError::Invalid(
                            "assessment cumulative byte budget"
                        ))
                    ),
                    "normal propagation preserves the original error"
                );
            } else {
                assert!(matches!(
                    result,
                    Err(SourceCommandError::Unsupported(
                        "original Assessment payer resource refusal persists"
                    ))
                ));
            }
            assert_eq!(original.work_remaining, before_work - limits.max_work);
        }
    }

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
        let report_input_ceiling = original.limits().unwrap().max_input_bytes;
        let limits = Limits {
            // The cumulative source Session is not one engine report packet.
            // No large input is read/allocated by this resource regression.
            max_input_bytes: report_input_ceiling + 1,
            max_nodes: 32,
            max_work: 64,
            max_refs: 8,
            max_depth: 8,
            max_items: 8,
            max_state_bytes: 128,
            deadline,
        };
        let result: SourceCommandResult<()> =
            with_original_pwe_session(&mut original, limits, &cancel, |payer, session| {
                assert!(session.has_source_payer(payer));
                assert!(payer.remaining()?.bytes > report_input_ceiling);
                payer.inspect(3, 2)?;
                payer.reserve(5)?;
                session
                    .admit_source_prefix(7, 2)
                    .map_err(|e| payer.error(e))?;
                session
                    .retain_source_state(11)
                    .map_err(|e| payer.error(e))?;
                Err(SourceCommandError::Denied("body refused"))
            });
        assert!(matches!(
            result,
            Err(SourceCommandError::Denied("body refused"))
        ));
        assert_eq!(original.work_remaining, before_work - limits.max_work - 2);
        assert!(original.bytes_remaining < before_bytes - 26);
        // An impossible profile refuses against the ACTUAL remainder; it does
        // not construct another budget or reset the already consumed prefix.
        let spent_work = original.work_remaining;
        let refused: SourceCommandResult<()> = with_original_pwe_session(
            &mut original,
            Limits {
                max_work: before_work,
                ..limits
            },
            &cancel,
            |_, _| panic!("unfunded source Session must not start"),
        );
        assert!(refused.is_err());
        assert_eq!(original.work_remaining, spent_work);
    }

    #[test]
    fn sto_source_measures_keep_native_work_out_of_nodes_and_preserve_failed_prefix() {
        for swallow in [false, true] {
            let deadline = Instant::now() + Duration::from_secs(30);
            let cancel = AtomicBool::new(false);
            let mut original = OperationBudget::new(
                &serde_json::json!({"budgets": {
                    "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
                }}),
                deadline,
            )
            .unwrap();
            let work_before = original.work_remaining;
            let bytes_at_callback = Cell::new(0);
            let limits = Limits {
                max_input_bytes: 128,
                max_nodes: 16,
                max_work: 64,
                max_refs: 8,
                max_depth: 8,
                max_items: 8,
                max_state_bytes: 256,
                deadline,
            };
            let result: SourceCommandResult<()> =
                with_original_pwe_session(&mut original, limits, &cancel, |payer, session| {
                    bytes_at_callback.set(payer.remaining()?.bytes);
                    payer.charge_sto_source_measures(session, 7, 11, 13, 17, 19)?;
                    assert_eq!(session.charges(), (7, 0, 43, 0, 24));
                    // Actual receipt-search emissions use repeated new_owned=76.
                    // Each is paid; a zero later event creates no credit.
                    for _ in 0..2 {
                        payer.charge_sto_source_measures(session, 0, 76, 0, 0, 1)?;
                    }
                    payer.charge_sto_source_measures(session, 0, 0, 0, 0, 0)?;
                    assert_eq!(session.charges(), (7, 0, 45, 0, 176));
                    let error = payer
                        .charge_sto_source_measures(session, 0, 5, 0, 20, 0)
                        .unwrap_err();
                    assert_eq!(session.charges(), (7, 0, 45, 0, 181));
                    assert!(matches!(
                        error,
                        SourceCommandError::Unsupported("PWE Assessment Session exhausted")
                    ));
                    if swallow {
                        drop(error);
                        Ok(())
                    } else {
                        Err(error)
                    }
                });
            assert!(matches!(result, Err(SourceCommandError::Unsupported(_))));
            assert_eq!(original.work_remaining, work_before - 64);
            // Same original byte payer, no work proxy, replacement or refill.
            assert_eq!(original.bytes_remaining, bytes_at_callback.get() - 188);
        }
    }
}
