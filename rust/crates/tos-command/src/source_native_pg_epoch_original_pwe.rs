//! Source epoch over an ALREADY EXISTING original payer/Session. This avoids
//! holding payer.with_operation() across a callback that reenters that payer.
//! No new allowance, ledger, Session, read permission or native-fit receipt.
use super::{DurablePgCoordinator, SourceRevisionReadSession, SourceRevisionReadSet};
use crate::source_command::SourceCommandResult;
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
use crate::{DurableError, DurableResult};
use std::{
    mem::{size_of, size_of_val},
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_compiler::pwe_native::Session;
use tos_segment_store::SegmentStore;

fn complete_query(
    result: SourceCommandResult<Option<()>>,
    foreign: Option<DurableError>,
) -> DurableResult<()> {
    match (result, foreign) {
        // Query's original non-Source failure was retained BEFORE a later
        // payer exit failure. Keep both resource refusal state and original
        // owned error; no error-string/category test or error reconstruction.
        (_, Some(original)) => Err(original),
        (Err(original), None) => Err(DurableError::Source(original)),
        (Ok(Some(())), None) => Ok(()),
        (Ok(None), None) => Err(DurableError::Invalid(
            "source PWE epoch exit relay inconsistent",
        )),
    }
}
impl DurablePgCoordinator {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_source_revision_session_with_pwe_exit_data_admission<
        'budget,
        'cancel,
        'pure,
        T,
    >(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        payer: &OriginalAssessmentPayer<'budget, 'cancel>,
        same: &mut Session<'pure>,
        operation: impl FnOnce(
            &mut SourceRevisionReadSession<'_>,
            &OriginalAssessmentPayer<'budget, 'cancel>,
            &mut Session<'pure>,
        ) -> DurableResult<T>,
    ) -> DurableResult<(T, SourceRevisionReadSet)> {
        let (actual_deadline, actual_cancelled) = payer.source_cutoff();
        if deadline != actual_deadline || !std::ptr::eq(cancelled, actual_cancelled) {
            return Err(DurableError::Conflict(
                "source PWE epoch cutoff differs from original payer",
            ));
        }
        // Local resources/callback/result holders BEFORE construction. The
        // existing source-session constructor and query/native/driver opening
        // costs retain their explicit debt; this is an exit DATA port only.
        let fixed = size_of::<(&OriginalAssessmentPayer<'_, '_>, &mut Session<'_>)>()
            .checked_add(size_of_val(&operation))
            .and_then(|n| n.checked_add(size_of::<DurableResult<(T, SourceRevisionReadSet)>>()))
            .and_then(|n| n.checked_add(size_of::<DurableResult<T>>()))
            .and_then(|n| n.checked_add(size_of::<Option<DurableError>>()))
            .and_then(|n| n.checked_add(size_of::<SourceCommandResult<Option<()>>>()))
            .and_then(|n| n.checked_add(2 * size_of::<DurableResult<()>>()))
            .ok_or(DurableError::Refused(
                "source PWE epoch control holders overflow",
            ))?;
        payer
            .charge_sto_source_measures(same, 0, fixed, 0, 0, 1)
            .map_err(DurableError::Source)?;
        let mut resources = (payer, same);
        self.with_source_revision_session_resources(
            store,
            domain,
            job_id,
            job_fence,
            deadline,
            cancelled,
            &mut resources,
            |epoch, resources| operation(epoch, resources.0, &mut *resources.1),
            |epoch, resources| {
                // Short original-budget loan ONLY during the actual final
                // freshness query, after the body/host/worker loan ended.
                let mut foreign = None;
                let result = resources.0.with_operation(|budget| {
                    match epoch.verify_fresh_with_query_data_admission(budget, deadline, cancelled)
                    {
                        Ok(()) => Ok(Some(())),
                        Err(DurableError::Source(original)) => Err(original),
                        Err(original) => {
                            foreign = Some(original);
                            Ok(None)
                        }
                    }
                });
                complete_query(result, foreign)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command::SourceCommandError;
    #[test]
    fn pwe_source_epoch_query_relay_preserves_foreign_body_before_late_payer_error() {
        let original = DurableError::Refused("original real query refusal");
        assert!(matches!(
            complete_query(
                Err(SourceCommandError::Denied(
                    "later original payer exit refused"
                )),
                Some(original)
            ),
            Err(DurableError::Refused("original real query refusal"))
        ));
        assert!(matches!(
            complete_query(
                Ok(Some(())),
                Some(DurableError::Conflict("original query conflict"))
            ),
            Err(DurableError::Conflict("original query conflict"))
        ));
    }
    #[test]
    fn pwe_source_epoch_query_relay_requires_actual_success_and_keeps_source_error() {
        assert!(complete_query(Ok(Some(())), None).is_ok());
        assert!(matches!(
            complete_query(Ok(None), None),
            Err(DurableError::Invalid(
                "source PWE epoch exit relay inconsistent"
            ))
        ));
        assert!(matches!(
            complete_query(
                Err(SourceCommandError::Invalid(
                    "original detailed source error"
                )),
                None
            ),
            Err(DurableError::Source(SourceCommandError::Invalid(
                "original detailed source error"
            )))
        ));
    }
}
