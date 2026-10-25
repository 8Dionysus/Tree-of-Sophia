//! Lossless OWNED transport for the new closed PG Assessment producer only.
//! No permission/report/target constructor, public rendering, native evaluator
//! callback, replacement ledger or legacy compatibility mapping lives here.
use super::OriginalAssessmentPayer;
use crate::durable_private_packages::{PrivateOwnerReadFailure, PrivateOwnerReadResult};
use crate::{DurableError, DurableResult};
use tos_validation::executor::ExecutorFailure;
use tos_validation::{assessment::AssessmentRefusal, item_rules::ItemRefusal};

/// Preserve the exact owned source or native body error; no cloning, string
/// formatting, compatibility_category or conversion to a coarse category.
#[derive(Debug)]
pub(crate) enum PgAssessmentEngineError {
    Source(DurableError),
    Native(AssessmentRefusal),
}
pub(crate) type PgAssessmentEngineResult<T> = Result<T, PgAssessmentEngineError>;
#[derive(Debug)]
pub(crate) struct PgAssessmentFrameFailure {
    pub(crate) original: PgAssessmentEngineError,
    pub(crate) frame_exit: Option<DurableError>,
}
pub(crate) type PgAssessmentFrameResult<T> = Result<T, PgAssessmentFrameFailure>;

/// Distinct stage types prevent a second generic finisher from overwriting an
/// earlier frame exit. The actual Common owner adds its own paid exit once.
#[derive(Debug)]
pub(crate) enum PgAssessmentOwnerBodyError {
    Owner(DurableError),
    Frame(PgAssessmentFrameFailure),
}
#[derive(Debug)]
pub(crate) struct PgAssessmentProducerFailure {
    pub(crate) original: PgAssessmentOwnerBodyError,
    pub(crate) owner_exit: Option<DurableError>,
}
pub(crate) type PgAssessmentProducerResult<T> = Result<T, PgAssessmentProducerFailure>;

pub(crate) fn complete_frame<T>(
    body: PgAssessmentEngineResult<T>,
    exited: DurableResult<()>,
) -> PgAssessmentFrameResult<T> {
    match (body, exited) {
        (Err(original), exited) => Err(PgAssessmentFrameFailure {
            original,
            frame_exit: exited.err(),
        }),
        (Ok(_), Err(original)) => Err(PgAssessmentFrameFailure {
            original: PgAssessmentEngineError::Source(original),
            frame_exit: None,
        }),
        (Ok(value), Ok(())) => Ok(value),
    }
}
pub(crate) fn complete_owner<T>(
    body: Result<T, PgAssessmentOwnerBodyError>,
    exited: DurableResult<()>,
) -> PgAssessmentProducerResult<T> {
    match (body, exited) {
        (Err(original), exited) => Err(PgAssessmentProducerFailure {
            original,
            owner_exit: exited.err(),
        }),
        (Ok(_), Err(original)) => Err(PgAssessmentProducerFailure {
            original: PgAssessmentOwnerBodyError::Owner(original),
            owner_exit: None,
        }),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// Outer joint guard keeps the complete Operation/Release/OperationAndRelease
/// value. A producer error, retained BEFORE returning the internal Err relay,
/// stays primary; the actual private completion is additionally retained.
#[derive(Debug)]
pub(crate) enum PgAssessmentHeldFailure {
    Producer {
        original: PgAssessmentProducerFailure,
        private_completion: Option<PrivateOwnerReadFailure>,
    },
    PrivateRead(PrivateOwnerReadFailure),
}
pub(crate) fn complete_private<T>(
    guarded: PrivateOwnerReadResult<Option<T>>,
    retained: Option<PgAssessmentProducerFailure>,
) -> Result<T, PgAssessmentHeldFailure> {
    match (guarded, retained) {
        (guarded, Some(original)) => Err(PgAssessmentHeldFailure::Producer {
            original,
            private_completion: guarded.err(),
        }),
        (Err(original), None) => Err(PgAssessmentHeldFailure::PrivateRead(original)),
        (Ok(Some(value)), None) => Ok(value),
        (Ok(None), None) => Err(PgAssessmentHeldFailure::PrivateRead(
            PrivateOwnerReadFailure::Operation(DurableError::Corrupt(
                "PG assessment owned error relay is absent",
            )),
        )),
    }
}

fn native_resource_refusal(error: &AssessmentRefusal) -> bool {
    match error {
        AssessmentRefusal::Budget | AssessmentRefusal::Cancelled | AssessmentRefusal::Deadline => {
            true
        }
        AssessmentRefusal::Schema(error) => schema_resource_refusal(error),
        _ => false,
    }
}
fn schema_resource_refusal(error: &ItemRefusal) -> bool {
    match error {
        ItemRefusal::Budget | ItemRefusal::BudgetCheck { .. } | ItemRefusal::Deadline => true,
        ItemRefusal::Executor(evidence) => matches!(
            evidence.reason,
            ExecutorFailure::InputBudget
                | ExecutorFailure::ResourceLimitUnknown
                | ExecutorFailure::Timeout
                | ExecutorFailure::Cancelled
                | ExecutorFailure::CpuLimit
        ),
        _ => false,
    }
}
impl OriginalAssessmentPayer<'_, '_> {
    /// Borrowed source/schema visitor counterpart of the existing Assessment
    /// latch. Classification is shared exactly; no cloning/wrapping/consuming
    /// the original ItemRefusal and no error-category transport substitution.
    pub(crate) fn latch_native_schema_refusal(&self, original: &ItemRefusal) {
        if schema_resource_refusal(original) {
            self.failed.set(true);
        }
    }
    /// Monotonic latch ONLY: does not consume stored refusal, map/replace an
    /// error, manufacture a budget failure, debit/refund, borrow OperationBudget
    /// or touch source/permission state. The actual caller retains the owned E
    /// and calls this before paid exits. Its control/Err enclosure must already
    /// be prospectively funded. Genuine negative Ok(report) never enters here.
    pub(crate) fn latch_native_assessment_refusal(&self, original: &AssessmentRefusal) {
        if native_resource_refusal(original) {
            self.failed.set(true);
        }
    }
}

// Deliberately no Display/Error/Serialize/public summary implementation. Safe
// bounded public rendering is a separate owner boundary; it must never replace
// this internal owned primary or discard the detailed diagnostic evidence.

#[cfg(test)]
mod tests {
    use super::super::with_original_pwe_session;
    use super::*;
    use crate::source_command::SourceCommandError;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    use tos_compiler::pwe_native::Limits;

    fn operation(deadline: Instant) -> OperationBudget {
        OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap()
    }
    fn limits(deadline: Instant) -> Limits {
        Limits {
            max_input_bytes: 64,
            max_nodes: 16,
            max_work: 64,
            max_refs: 8,
            max_depth: 8,
            max_items: 8,
            max_state_bytes: 4096,
            deadline,
        }
    }
    #[test]
    fn borrowed_schema_latch_keeps_owned_detail_and_does_not_poison_semantic_source_error() {
        for (original, resource) in [
            (
                ItemRefusal::BudgetCheck {
                    check: "actual visitor bound",
                    used: Some(11),
                    limit: Some(10),
                },
                true,
            ),
            (ItemRefusal::Deadline, true),
            (
                ItemRefusal::Source("original owned visitor detail".to_owned()),
                false,
            ),
        ] {
            let d = Instant::now() + Duration::from_secs(30);
            let c = AtomicBool::new(false);
            let mut operation = operation(d);
            let pointer = if let ItemRefusal::Source(text) = &original {
                Some(text.as_ptr())
            } else {
                None
            };
            let result =
                with_original_pwe_session(&mut operation, limits(d), &c, |payer, _same| {
                    payer.latch_native_schema_refusal(&original);
                    assert_eq!(payer.failed.get(), resource);
                    Ok(())
                });
            assert_eq!(result.is_err(), resource);
            if let ItemRefusal::Source(text) = original {
                assert_eq!(Some(text.as_ptr()), pointer);
            }
        }
    }
    #[test]
    fn owned_native_budgetcheck_keeps_details_through_frame_and_owner_exit() {
        let native = AssessmentRefusal::Schema(ItemRefusal::BudgetCheck {
            check: "native declared work",
            used: Some(11),
            limit: Some(10),
        });
        let frame: PgAssessmentFrameResult<()> = complete_frame(
            Err(PgAssessmentEngineError::Native(native)),
            Err(DurableError::Refused("later frame exit")),
        );
        let finished = complete_owner(
            frame.map_err(PgAssessmentOwnerBodyError::Frame),
            Err(DurableError::Conflict("later owner exit")),
        );
        match finished {
            Err(PgAssessmentProducerFailure {
                original:
                    PgAssessmentOwnerBodyError::Frame(PgAssessmentFrameFailure {
                        original:
                            PgAssessmentEngineError::Native(AssessmentRefusal::Schema(
                                ItemRefusal::BudgetCheck {
                                    check: "native declared work",
                                    used: Some(11),
                                    limit: Some(10),
                                },
                            )),
                        frame_exit: Some(DurableError::Refused("later frame exit")),
                    }),
                owner_exit: Some(DurableError::Conflict("later owner exit")),
            }) => {}
            _ => panic!("owned primary/details or an exit fault were replaced"),
        }
    }
    #[test]
    fn native_owned_string_and_executor_evidence_are_moved_without_coarsening() {
        let text = String::from("original invalid detail; not a public rendering");
        let address = text.as_ptr();
        let result: PgAssessmentFrameResult<()> = complete_frame(
            Err(PgAssessmentEngineError::Native(
                AssessmentRefusal::InvalidInput(text),
            )),
            Ok(()),
        );
        match result {
            Err(PgAssessmentFrameFailure {
                original: PgAssessmentEngineError::Native(AssessmentRefusal::InvalidInput(text)),
                frame_exit: None,
            }) => assert_eq!(text.as_ptr(), address),
            _ => panic!("owned native string was mapped/replaced"),
        }
        let evidence = tos_validation::item_rules::ItemExecutorRefusal {
            stage: "actual-test-stage",
            reason: ExecutorFailure::CpuLimit,
            exchange: None,
            quota: None,
            batch_completed_count: Some(7),
        };
        let body: PgAssessmentEngineResult<()> = Err(PgAssessmentEngineError::Native(
            AssessmentRefusal::Schema(ItemRefusal::Executor(Box::new(evidence))),
        ));
        match complete_frame(body, Ok(())) {
            Err(PgAssessmentFrameFailure {
                original:
                    PgAssessmentEngineError::Native(AssessmentRefusal::Schema(ItemRefusal::Executor(e))),
                frame_exit: None,
            }) => {
                assert_eq!(e.stage, "actual-test-stage");
                assert_eq!(e.batch_completed_count, Some(7));
                assert!(matches!(e.reason, ExecutorFailure::CpuLimit));
            }
            _ => panic!("typed executor evidence was projected to a compatibility category"),
        }
    }
    #[test]
    fn latch_does_not_consume_original_source_refusal_and_finish_stays_refused() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut op = operation(deadline);
        let result: Result<(), SourceCommandError> =
            with_original_pwe_session(&mut op, limits(deadline), &cancelled, |payer, _same| {
                // Deliberate TEST source refusal to exercise stored-first-error
                // custody. Production latch does not perform such a debit.
                let source = payer.debit(usize::MAX).unwrap_err();
                payer.latch_native_assessment_refusal(&AssessmentRefusal::Budget);
                assert!(
                    payer.refusal.borrow().is_some(),
                    "latch-only must not take the existing source cause"
                );
                Err(payer.error(source))
            });
        assert!(matches!(
            result,
            Err(SourceCommandError::Invalid(
                "assessment cumulative byte budget"
            ))
        ));
        let mut op = operation(deadline);
        let swallowed: Result<(), SourceCommandError> =
            with_original_pwe_session(&mut op, limits(deadline), &cancelled, |payer, _same| {
                payer.latch_native_assessment_refusal(&AssessmentRefusal::Budget);
                Ok(())
            });
        assert!(
            swallowed.is_err(),
            "swallowing a native resource error cannot authorize finish"
        );
    }
    #[test]
    fn native_resource_classification_uses_typed_variants_not_message_text() {
        assert!(native_resource_refusal(&AssessmentRefusal::Cancelled));
        assert!(native_resource_refusal(&AssessmentRefusal::Deadline));
        assert!(native_resource_refusal(&AssessmentRefusal::Schema(
            ItemRefusal::BudgetCheck {
                check: "typed budget",
                used: None,
                limit: None,
            }
        )));
        for error in [
            AssessmentRefusal::Unsupported("budget cancelled deadline".to_owned()),
            AssessmentRefusal::InvalidInput("budget".to_owned()),
            AssessmentRefusal::Schema(ItemRefusal::Source("deadline".to_owned())),
        ] {
            assert!(!native_resource_refusal(&error));
            let deadline = Instant::now() + Duration::from_secs(30);
            let cancel = AtomicBool::new(false);
            let mut op = operation(deadline);
            with_original_pwe_session(&mut op, limits(deadline), &cancel, |payer, _same| {
                payer.latch_native_assessment_refusal(&error);
                Ok(())
            })
            .unwrap();
        }
        // Unit outcomes here exercise the resource adapter only, not a fake
        // mechanics report, semantic positive or application admission fixture.
    }
    #[test]
    fn private_completion_keeps_operation_and_release_with_native_primary() {
        let engine: PgAssessmentEngineResult<()> = Err(PgAssessmentEngineError::Native(
            AssessmentRefusal::Unsupported("owned native primary".to_owned()),
        ));
        let frame = complete_frame(engine, Ok(()));
        let failure =
            complete_owner(frame.map_err(PgAssessmentOwnerBodyError::Frame), Ok(())).unwrap_err();
        let completion = PrivateOwnerReadFailure::OperationAndRelease {
            operation: DurableError::Source(SourceCommandError::Unsupported(
                "internal owned producer relay",
            )),
            release: DurableError::Corrupt("actual-test-release-fault"),
        };
        let result: Result<Option<()>, PgAssessmentHeldFailure> =
            complete_private(Err(completion), Some(failure));
        assert!(matches!(
            result,
            Err(PgAssessmentHeldFailure::Producer {
                original: PgAssessmentProducerFailure {
                    original: PgAssessmentOwnerBodyError::Frame(PgAssessmentFrameFailure {
                        original: PgAssessmentEngineError::Native(AssessmentRefusal::Unsupported(
                            _
                        )),
                        ..
                    }),
                    ..
                },
                private_completion: Some(PrivateOwnerReadFailure::OperationAndRelease { .. }),
            })
        ));
        let missing: Result<(), PgAssessmentHeldFailure> = complete_private(Ok(None), None);
        assert!(
            missing.is_err(),
            "missing owned outcome cannot become successful output"
        );
    }
}
