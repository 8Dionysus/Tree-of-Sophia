//! Private, linear custody of the new Assessment body and its ACTUAL exit.
//! No engine callback, report/permission factory, host/worker getter or fence
//! is supplied here. The future concrete producer still owes its native quote
//! and must feed the actual frame exit BEFORE legacy `complete` discards it.
use super::{check_session, overflow};
use crate::source_command::SourceCommandResult;
use crate::source_native_cli::private_assessment::original_payer::{
    OriginalAssessmentPayer,
    pg_assessment_transport::{
        PgAssessmentEngineError, PgAssessmentEngineResult, PgAssessmentFrameFailure,
        PgAssessmentFrameResult, complete_frame,
    },
};
use crate::{DurableError, DurableResult};
use std::{marker::PhantomData, mem::size_of};
use tos_compiler::pwe_native::Session;

// Three logical local transitions: admit fixed storage, capture/latch body,
// consume exit/join. This is not an engine/native-instruction/CPU/RSS quote.
const COMPLETION_TRANSITIONS: usize = 3;

/// Resource-only reservation, issued BEFORE the actual producer runs. Holding
/// the actual payer borrow selects which existing latch capture may touch.
/// This proves neither owner/target authority nor that a supplied exit ran.
#[must_use = "capture the actual body only after prospective producer admission"]
pub(super) struct AssessmentFrameReservation<'payer, 'budget, 'cancel, T> {
    payer: &'payer OriginalAssessmentPayer<'budget, 'cancel>,
    value: PhantomData<fn() -> T>,
}

/// Body custody begins before the actual paid exit. No Clone, default, raw
/// getter, success sentinel or conversion back to DurableError is available.
#[must_use = "complete with the actual paid frame exit before returning"]
pub(super) struct AssessmentFrameBody<T> {
    body: PgAssessmentEngineResult<T>,
}

impl<'payer, 'budget, 'cancel, T> AssessmentFrameReservation<'payer, 'budget, 'cancel, T> {
    fn fixed_bytes() -> DurableResult<usize> {
        // Sum distinct typed local/enclosure slots, not a guessed multiplier.
        // Dynamic backing in T/native diagnostics is NOT priced by size_of.
        size_of::<Self>()
            .checked_add(size_of::<AssessmentFrameBody<T>>())
            .and_then(|n| n.checked_add(size_of::<PgAssessmentEngineResult<T>>()))
            .and_then(|n| n.checked_add(size_of::<PgAssessmentFrameResult<T>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<Self>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<()>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<()>>()))
            .and_then(|n| n.checked_add(size_of::<SourceCommandResult<()>>()))
            .and_then(|n| n.checked_add(size_of::<PgAssessmentFrameFailure>()))
            .and_then(|n| n.checked_add(size_of::<Option<DurableError>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<usize>>()))
            .and_then(|n| n.checked_add(size_of::<&PgAssessmentEngineResult<T>>()))
            .and_then(|n| {
                n.checked_add(size_of::<&tos_validation::assessment::AssessmentRefusal>())
            })
            .and_then(|n| n.checked_add(size_of::<usize>()))
            .ok_or_else(overflow)
    }

    /// Same original pair and original native byte/work currencies. No input,
    /// state, semantic nodes/refs or prepaid pure work is spent or reset here.
    /// The caller prepays incoming activation/admission-failure carriers and
    /// the complete producer/diagnostic/output quote before invoking any work.
    /// Actual verify/owner/private exits retain their OWN prospective debits.
    pub(super) fn admit(
        payer: &'payer OriginalAssessmentPayer<'budget, 'cancel>,
        same: &mut Session<'_>,
    ) -> DurableResult<Self> {
        check_session(same, payer)?;
        let bytes = Self::fixed_bytes()?;
        payer
            .inspect(bytes, COMPLETION_TRANSITIONS)
            .map_err(DurableError::Source)?;
        Ok(Self {
            payer,
            value: PhantomData,
        })
    }

    /// Capture BEFORE the caller attempts the actual exit. The fixed local
    /// transition was prepaid, so there is no new fallible debit that could
    /// replace an already obtained body error. Owned dynamic payloads move.
    pub(super) fn capture(self, body: PgAssessmentEngineResult<T>) -> AssessmentFrameBody<T> {
        if let Err(PgAssessmentEngineError::Native(original)) = &body {
            self.payer.latch_native_assessment_refusal(original);
        }
        AssessmentFrameBody { body }
    }
}

impl<T> AssessmentFrameBody<T> {
    /// Pass the ACTUAL paid frame exit directly, never legacy complete's
    /// already-collapsed result. Both outcomes remain explicit typed values.
    pub(super) fn complete(self, exited: DurableResult<()>) -> PgAssessmentFrameResult<T> {
        complete_frame(self.body, exited)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command::SourceCommandError;
    use crate::source_native_cli::private_assessment::{
        OperationBudget, original_payer::with_original_pwe_session_durable,
    };
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    use tos_compiler::pwe_native::Limits;
    use tos_validation::assessment::AssessmentRefusal;

    fn operation(deadline: Instant) -> OperationBudget {
        OperationBudget::new(
            &serde_json::json!({"budgets": {
                "worker_cpu_seconds": 1, "worker_address_space_bytes": 1048576
            }}),
            deadline,
        )
        .unwrap()
    }

    fn limits(deadline: Instant) -> Limits {
        Limits {
            max_input_bytes: 64,
            max_state_bytes: 4096,
            max_work: 64,
            max_nodes: 16,
            max_refs: 8,
            max_depth: 8,
            max_items: 8,
            deadline,
        }
    }

    #[test]
    fn assessment_completion_keeps_all_four_body_exit_outcomes_without_report_fixture() {
        for body_failed in [false, true] {
            for exit_failed in [false, true] {
                let deadline = Instant::now() + Duration::from_secs(30);
                let cancelled = AtomicBool::new(false);
                let mut original = operation(deadline);
                with_original_pwe_session_durable(
                    &mut original,
                    limits(deadline),
                    &cancelled,
                    |payer, same| {
                        let before = payer.remaining().map_err(DurableError::Source)?;
                        let pure = same.charges();
                        let reservation = AssessmentFrameReservation::<()>::admit(payer, same)?;
                        let after = payer.remaining().map_err(DurableError::Source)?;
                        assert_eq!(before.work - after.work, COMPLETION_TRANSITIONS);
                        assert_eq!(
                            before.bytes - after.bytes,
                            AssessmentFrameReservation::<()>::fixed_bytes()?
                        );
                        assert_eq!(
                            same.charges(),
                            pure,
                            "native local custody is not pure/source input or work"
                        );
                        let text =
                            String::from("owned body detail, not a report or authority fixture");
                        let address = text.as_ptr();
                        let body = if body_failed {
                            Err(PgAssessmentEngineError::Native(
                                AssessmentRefusal::InvalidInput(text),
                            ))
                        } else {
                            Ok(())
                        };
                        let pending = reservation.capture(body);
                        // Explicit supplied outcome operands test the join;
                        // they do not pretend a PG frame/permission was issued.
                        let exited = if exit_failed {
                            Err(DurableError::Conflict("actual test exit operand"))
                        } else {
                            Ok(())
                        };
                        match (body_failed, exit_failed, pending.complete(exited)) {
                            (false, false, Ok(())) => {}
                            (
                                false,
                                true,
                                Err(PgAssessmentFrameFailure {
                                    original:
                                        PgAssessmentEngineError::Source(DurableError::Conflict(
                                            "actual test exit operand",
                                        )),
                                    frame_exit: None,
                                }),
                            ) => {}
                            (
                                true,
                                failed,
                                Err(PgAssessmentFrameFailure {
                                    original:
                                        PgAssessmentEngineError::Native(
                                            AssessmentRefusal::InvalidInput(text),
                                        ),
                                    frame_exit,
                                }),
                            ) => {
                                assert_eq!(
                                    text.as_ptr(),
                                    address,
                                    "owned body string is moved, not rewritten"
                                );
                                assert_eq!(
                                    text,
                                    "owned body detail, not a report or authority fixture"
                                );
                                match (failed, frame_exit) {
                                    (false, None)
                                    | (
                                        true,
                                        Some(DurableError::Conflict("actual test exit operand")),
                                    ) => {}
                                    _ => panic!("secondary actual exit was replaced"),
                                }
                            }
                            _ => panic!("body/exit outcomes collapsed"),
                        }
                        Ok(())
                    },
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn assessment_completion_latches_before_original_cancelled_and_sticky_exits() {
        for cancellation in [false, true] {
            let deadline = Instant::now() + Duration::from_secs(30);
            let cancelled = AtomicBool::new(false);
            let mut original = operation(deadline);
            let result = with_original_pwe_session_durable(
                &mut original,
                limits(deadline),
                &cancelled,
                |payer, same| {
                    let reservation = AssessmentFrameReservation::<()>::admit(payer, same)?;
                    let native = if cancellation {
                        AssessmentRefusal::Cancelled
                    } else {
                        AssessmentRefusal::Budget
                    };
                    let pending = reservation.capture(Err(PgAssessmentEngineError::Native(native)));
                    if cancellation {
                        cancelled.store(true, Ordering::Relaxed);
                    }
                    // Real original-payer/SAME Session exit refusal, not a PG
                    // authority fixture or a fabricated successful fence.
                    let exited = check_session(same, payer);
                    match (cancellation, pending.complete(exited)) {
                        (
                            true,
                            Err(PgAssessmentFrameFailure {
                                original:
                                    PgAssessmentEngineError::Native(AssessmentRefusal::Cancelled),
                                frame_exit:
                                    Some(DurableError::Source(SourceCommandError::Denied(_))),
                            }),
                        ) => {}
                        (
                            false,
                            Err(PgAssessmentFrameFailure {
                                original: PgAssessmentEngineError::Native(AssessmentRefusal::Budget),
                                frame_exit:
                                    Some(DurableError::Source(SourceCommandError::Unsupported(_))),
                            }),
                        ) => {}
                        _ => panic!("original native cause or actual refused exit was lost"),
                    }
                    Ok(()) // Deliberately swallowed: original finish must refuse.
                },
            );
            assert!(
                result.is_err(),
                "original sticky/cancelled finish cannot become success"
            );
        }
    }

    #[test]
    fn assessment_completion_preserves_original_source_error_before_later_sticky_exit() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = operation(deadline);
        let result = with_original_pwe_session_durable(
            &mut original,
            limits(deadline),
            &cancelled,
            |payer, same| {
                let reservation = AssessmentFrameReservation::<()>::admit(payer, same)?;
                let first = same.admit_source_work(65).unwrap_err();
                let actual = DurableError::Source(payer.error(first));
                let pending = reservation.capture(Err(PgAssessmentEngineError::Source(actual)));
                let exited = check_session(same, payer);
                assert!(matches!(
                    pending.complete(exited),
                    Err(PgAssessmentFrameFailure {
                        original: PgAssessmentEngineError::Source(DurableError::Source(
                            SourceCommandError::Unsupported("PWE Assessment Session exhausted")
                        )),
                        frame_exit: Some(DurableError::Source(SourceCommandError::Unsupported(
                            "original Assessment payer resource refusal persists"
                        ))),
                    })
                ));
                Ok(()) // Original payer finish must retain the refusal.
            },
        );
        assert!(result.is_err());
    }
}
