//! AUTHORED / UNRUN native fixture consumers. These functions require actual
//! privately-issued Common PG roles/current owner and actual native reports.
//! No tests fabricate records, permissions, profiles, workers, budgets or reports.
//! They are compiled by the combined --lib --tests gate after registration;
//! the actual runner must supply the separately enumerated source cases.
use super::*;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
struct ActualCounters {
    charges: (usize, usize, usize, usize, usize),
    original_bytes: usize,
    original_work: usize,
}

// Test-only inspection of the retained ORIGINAL loan, never a public host,
// worker, payer or Session accessor. Snapshot carriers/visits are paid before
// the read on those same ledgers; diagnostic assertions are fixture code.
fn actual_counters(
    application: &mut PgPublicApplication<'_, '_, '_, '_, '_, '_, '_, '_>,
) -> DurableResult<ActualCounters> {
    application.current.with_source_epoch(|_, payer, same| {
        {
            let mut ledger = source(SourceLedger::new(same, payer))?;
            source(ledger.reserve(
                size_of::<ActualCounters>() + size_of::<DurableResult<ActualCounters>>(),
            ))?;
            source(ledger.inspect(0, 8))?;
        }
        let original = source(payer.remaining())?;
        Ok(ActualCounters {
            charges: same.charges(),
            original_bytes: original.bytes,
            original_work: original.work,
        })
    })
}

fn copy_coordinates<'a>(
    application: &mut PgPublicApplication<'_, '_, '_, '_, '_, '_, '_, '_>,
    coordinates: &PgReportCoordinates<'a>,
) -> DurableResult<PgReportCoordinates<'a>> {
    application.current.with_source_epoch(|_, payer, same| {
        let mut ledger = source(SourceLedger::new(same, payer))?;
        source(ledger.reserve(size_of::<PgReportCoordinates<'_>>()))?;
        source(ledger.inspect(0, 4))?;
        Ok(PgReportCoordinates {
            access_scope: coordinates.access_scope,
            assessment_policy: coordinates.assessment_policy,
            observed_now: coordinates.observed_now,
            evaluator_input_sha256: coordinates.evaluator_input_sha256,
        })
    })
}

fn assert_spent_prefix(before: &ActualCounters, after: &ActualCounters, report_work: usize) {
    assert!(after.charges.0 >= before.charges.0);
    assert!(after.charges.1 >= before.charges.1);
    assert!(after.charges.2.checked_sub(before.charges.2).unwrap() >= report_work);
    assert!(after.charges.3 >= before.charges.3);
    assert!(after.charges.4 >= before.charges.4);
    assert!(after.original_bytes < before.original_bytes);
    assert!(after.original_work < before.original_work);
}

/// AUTHORED/UNRUN. Actual ready native report/Claim/roles only. Measures include
/// the priced inspection/owner guards, so inequalities are lower bounds rather
/// than a claim that comparison costs exactly report.work_used(). The Common
/// runner must additionally inspect its actual outer-operation final counters.
pub(crate) fn fixture_cumulative_comparison_twice(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    report: &AssessmentMechanicsReport,
    coordinates: PgReportCoordinates<'_>,
) -> DurableResult<()> {
    with_pg_public_application(current, claim, exact, roles, |mut application| {
        let before = actual_counters(&mut application)?;
        assert!(
            before.charges.2 > 0,
            "prepare/check/qualify already spent original A"
        );
        let first_coordinates = copy_coordinates(&mut application, &coordinates)?;
        assert_eq!(
            application.compare_native_report(report, first_coordinates)?,
            PgReportComparison::MatchingMechanics
        );
        let first = actual_counters(&mut application)?;
        assert_spent_prefix(&before, &first, report.work_used());
        assert_eq!(
            application.compare_native_report(report, coordinates)?,
            PgReportComparison::MatchingMechanics
        );
        let second = actual_counters(&mut application)?;
        assert_spent_prefix(&first, &second, report.work_used());
        Ok(())
    })
}

/// Test-only phase observation; never source/currentness/admission evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OriginalWorkDrainPhase {
    NotEntered,
    Entered,
    Drained,
    ScopeExitErr,
}

/// AUTHORED/UNRUN. Retargeted after v27 reachability RETURN: the actual work
/// drain must fail at a PAID scope exit before another comparison is invoked.
/// ScopeExitErr means only body-drain Ok followed by a scope-exit Err. Keep
/// the actual first error/currency: it does not identify an exact debit site.
/// Separate comparison-entry/report-work exhaustion remains OPEN.
pub(crate) fn fixture_scope_exit_after_original_work_drain(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    report: &AssessmentMechanicsReport,
    coordinates: PgReportCoordinates<'_>,
    phase: &Cell<OriginalWorkDrainPhase>,
) -> DurableResult<()> {
    // Reset only diagnostic reachability, never Session/ledger/cutoff state.
    // A reused Cell must not turn a pre-entry refusal into a reached target.
    phase.set(OriginalWorkDrainPhase::NotEntered);
    with_pg_public_application(current, claim, exact, roles, |mut application| {
        let first_coordinates = copy_coordinates(&mut application, &coordinates)?;
        assert_eq!(
            application.compare_native_report(report, first_coordinates)?,
            PgReportComparison::MatchingMechanics
        );
        let drained_scope = application.current.with_source_epoch(|_, payer, same| {
            phase.set(OriginalWorkDrainPhase::Entered);
            let before = same.charges();
            let remaining = source(same.source_allowance().map_err(|e| payer.error(e)))?.work;
            source(
                same.admit_source_work(remaining)
                    .map_err(|e| payer.error(e)),
            )?;
            let after = same.charges();
            assert_eq!(
                (after.0, after.1, after.3, after.4),
                (before.0, before.1, before.3, before.4)
            );
            assert_eq!(after.2.checked_sub(before.2), Some(remaining));
            phase.set(OriginalWorkDrainPhase::Drained);
            Ok(())
        });
        match drained_scope {
            Err(original) => {
                if phase.get() == OriginalWorkDrainPhase::Drained {
                    phase.set(OriginalWorkDrainPhase::ScopeExitErr);
                }
                Err(original) // preserve exact scope-exit or earlier body error
            }
            Ok(()) => panic!("full work drain cannot make protected owner exits free"),
        }
    })
}

/// AUTHORED/UNRUN. `actual_cancelled` MUST be the flag actually used by the
/// runner's original operation/epoch, not a fresh flag. The caller constructed
/// that route and retains its flag; this helper creates no replacement cutoff.
pub(crate) fn fixture_cancelled_comparison(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    report: &AssessmentMechanicsReport,
    coordinates: PgReportCoordinates<'_>,
    actual_cancelled: &AtomicBool,
) -> DurableResult<()> {
    with_pg_public_application(current, claim, exact, roles, |mut application| {
        let first_coordinates = copy_coordinates(&mut application, &coordinates)?;
        assert_eq!(
            application.compare_native_report(report, first_coordinates)?,
            PgReportComparison::MatchingMechanics
        );
        actual_cancelled.store(true, Ordering::Relaxed);
        match application.compare_native_report(report, coordinates) {
            Err(original) => Err(original),
            Ok(_) => panic!("actual cancelled original owner must refuse comparison"),
        }
    })
}

/// AUTHORED/UNRUN. Executes the actual admitted native parser on intentionally
/// malformed fixture bytes. Its original source error must survive an actual
/// cancelled outer exit and original payer finish; no report/role is issued.
pub(crate) fn fixture_body_error_and_cancelled_exit(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    actual_cancelled: &AtomicBool,
    observed_strict_parser_failure: &Cell<bool>,
) -> DurableResult<()> {
    let mut original_detail = None;
    let result = with_pg_public_application(current, claim, exact, roles, |mut application| {
        let parsed = application.current.with_source_epoch(|_, payer, _| {
            let (deadline, cancelled) = payer.source_cutoff();
            source(payer.with_operation(|budget| {
                crate::source_native_roles::admitted_document(b"{", budget, deadline, cancelled)
                    .map(|_| ())
            }))
        });
        let original = parsed.expect_err("actual admitted parser must reject malformed bytes");
        if !matches!(
            &original,
            DurableError::Source(SourceCommandError::Invalid(
                "native role strict source document"
            ))
        ) {
            return Err(original); // preserve a genuinely earlier funding/source failure
        }
        observed_strict_parser_failure.set(true); // test reachability only, never authority
        original_detail = Some(format!("{original:?}"));
        actual_cancelled.store(true, Ordering::Relaxed);
        Err(original)
    });
    match result {
        Err(error) => {
            if let Some(original) = original_detail {
                assert_eq!(
                    format!("{error:?}"),
                    original,
                    "body Err must survive paid Err exit"
                );
            }
            // An earlier prepare/source refusal remains the real dependency;
            // it is not evidence that the intended body-error case executed.
            Err(error)
        }
        Ok(()) => panic!("body error or actual cancelled exit must refuse"),
    }
}

/// Use once per genuine ready source Claim (name/responsibility/identity).
/// The fixture, not this adapter, owns source schema/route/basis admission.
fn actual_public_application_and_report(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    report: &AssessmentMechanicsReport,
    coordinates: PgReportCoordinates<'_>,
) -> DurableResult<()> {
    with_pg_public_application(current, claim, exact, roles, |mut application| {
        assert_eq!(application.exact(), exact);
        assert!(std::ptr::eq(
            application.claim().get("object").unwrap(),
            application.qualified().value()
        ));
        assert_eq!(
            application.compare_native_report(report, coordinates)?,
            PgReportComparison::MatchingMechanics
        );
        // Matching mechanics is intentionally NOT converted to NativeAccount,
        // an application ticket, parent witness, assessment or can_use grant.
        Ok(())
    })
}

/// Real fixture must select the appropriate DIFFERENT source operand, e.g.
/// a different exact role version, role/kind/registry, actual epoch or public
/// selection. Alternatively supply a schema-valid Claim with one changed
/// application pin/argument/qualification. Never mutate a private-issued role.
/// Assert the specific expected upstream/semantic error outside the frame too.
fn actual_reject_before_application_callback(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    expected: impl FnOnce(&DurableError) -> bool,
) {
    let entered = Cell::new(false);
    let result = with_pg_public_application(current, claim, exact, roles, |_| {
        entered.set(true);
        Ok(())
    });
    let error = result.expect_err("actual incompatible public input must refuse");
    assert!(
        !entered.get(),
        "failed public consistency must not invoke application body"
    );
    assert!(expected(&error), "wrong original/semantic error: {error:?}");
}

/// Wrong scope/policy/use/time/subject/source/worker must fail as Err. Test
/// each coordinate independently, using an actual alternate native report or
/// genuine requested plan operand. False/not-ready admission instead returns
/// NegativeMechanics. A real admitted-with-limits report must refuse until its
/// limits interpreter is selected. No arbitrary caller bool becomes admission.
fn actual_report_comparison_case(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    report: &AssessmentMechanicsReport,
    coordinates: PgReportCoordinates<'_>,
    expected: impl FnOnce(&DurableResult<PgReportComparison>) -> bool,
) -> DurableResult<()> {
    with_pg_public_application(current, claim, exact, roles, |mut application| {
        let result = application.compare_native_report(report, coordinates);
        assert!(
            expected(&result),
            "wrong actual report comparison: {result:?}"
        );
        // Preserve the original Err through the real outer Err fence/finish.
        result.map(|_| ())
    })
}

/// The real runner inserts an actual source/rights/job change, cancellation or
/// original-resource exhaustion in the BODY. Even a successful public stage
/// must then fail at its exit; on a body Err its original detail takes priority.
/// Observe the source/Session debit prefix OUTSIDE the actual whole operation.
fn actual_body_and_exit_error_case(
    current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>,
    claim: &JsonValue,
    exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    change: impl FnOnce() -> DurableResult<()>,
    expected: impl FnOnce(&DurableError) -> bool,
) {
    let result = with_pg_public_application(current, claim, exact, roles, |_| change());
    let error = result.expect_err("actual changed owner/cutoff must refuse at exit");
    assert!(
        expected(&error),
        "original body/exit error changed: {error:?}"
    );
}
