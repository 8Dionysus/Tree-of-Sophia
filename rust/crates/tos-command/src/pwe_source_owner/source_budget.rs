//! Borrows the original SOURCE payer and one caller-held cumulative Session.
//! No assessment budget, source selection, grant or authority is constructed.
use super::access::AccessMeter;
use super::{MAX_BYTES, invalid, unsupported};
use crate::source_claims::ClaimCallBudget;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use std::{cell::RefCell, mem::size_of, rc::Rc, sync::atomic::AtomicBool, time::Instant};
use tos_compiler::pwe_native::{Error, Session, SourceAdmission};
use tos_foundation::{
    CanonicalProfile, FoundationError, FoundationErrorCode, JsonLimits, JsonString, JsonValue,
    canonical_bytes_v1_with_state_budget_and_visits_and_admission,
};

pub(crate) struct SourcePayer<'a> {
    whole: &'a Rc<RefCell<ClaimCallBudget>>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    refusal: RefCell<Option<SourceCommandError>>,
}
impl<'a> SourcePayer<'a> {
    pub(crate) fn new(
        whole: &'a Rc<RefCell<ClaimCallBudget>>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        whole
            .try_borrow()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .remaining_live()?;
        Ok(Self {
            whole,
            deadline,
            cancelled,
            refusal: RefCell::new(None),
        })
    }
    fn debit(&self, retained: bool, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        let result = (|| {
            active(self.deadline, self.cancelled)?;
            let mut whole = self
                .whole
                .try_borrow_mut()
                .map_err(|_| invalid("PWE whole-call budget in use"))?;
            if retained {
                whole.retain(bytes)
            } else {
                whole.read(
                    u64::try_from(bytes).map_err(|_| unsupported("PWE source byte conversion"))?,
                )
            }
        })();
        result.map_err(|error| {
            let mut first = self.refusal.borrow_mut();
            if first.is_none() {
                *first = Some(error);
            }
            Error::Upstream("original source payer refused")
        })
    }
    pub(crate) fn error(&self, error: Error, reason: &'static str) -> SourceCommandError {
        if let Some(original) = self.refusal.borrow_mut().take() {
            return original;
        }
        match error {
            Error::Cancelled => SourceCommandError::Denied("PWE original Session cancelled"),
            Error::Deadline => SourceCommandError::Denied("PWE original Session deadline"),
            Error::ResourceLimit => unsupported("PWE cumulative source Session allowance"),
            Error::NeedsBinding(_) => unsupported("PWE original source payer binding absent"),
            _ => invalid(reason),
        }
    }
    fn remaining(&self) -> SourceCommandResult<(usize, usize)> {
        active(self.deadline, self.cancelled)?;
        let whole = self
            .whole
            .try_borrow()
            .map_err(|_| invalid("PWE whole-call budget in use"))?;
        Ok((
            usize::try_from(whole.remaining_read()?).unwrap_or(usize::MAX),
            whole.remaining_live()?,
        ))
    }
    pub(super) fn current(&self) -> SourceCommandResult<()> {
        active(self.deadline, self.cancelled)
    }
}
impl SourceAdmission for SourcePayer<'_> {
    fn inspect(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        self.debit(false, bytes)
    }
    fn retain(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        self.debit(true, bytes)
    }
}

// Resource-only interface over the two actual original payer implementations.
// It cannot select a source, validate a role, create a Session, or grant access.
pub(super) trait OriginalPayer: SourceAdmission {
    fn current(&self) -> SourceCommandResult<()>;
    fn remaining(&self) -> SourceCommandResult<(usize, usize)>;
    fn error(&self, error: Error, reason: &'static str) -> SourceCommandError;
    fn admission(&self) -> &dyn SourceAdmission;
    fn claim_wallet(&self) -> Option<&Rc<RefCell<ClaimCallBudget>>>;
    fn check_cutoff(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()>;
}
impl OriginalPayer for SourcePayer<'_> {
    fn admission(&self) -> &dyn SourceAdmission {
        self
    }
    fn claim_wallet(&self) -> Option<&Rc<RefCell<ClaimCallBudget>>> {
        Some(self.whole)
    }
    fn check_cutoff(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        if self.deadline != deadline || !std::ptr::eq(self.cancelled, cancelled) {
            return Err(SourceCommandError::Conflict(
                "PWE original Claim cutoff differs",
            ));
        }
        self.current()
    }
    fn current(&self) -> SourceCommandResult<()> {
        SourcePayer::current(self)
    }
    fn remaining(&self) -> SourceCommandResult<(usize, usize)> {
        SourcePayer::remaining(self)
    }
    fn error(&self, error: Error, reason: &'static str) -> SourceCommandError {
        SourcePayer::error(self, error, reason)
    }
}
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
impl OriginalPayer for OriginalAssessmentPayer<'_, '_> {
    fn admission(&self) -> &dyn SourceAdmission {
        self
    }
    fn claim_wallet(&self) -> Option<&Rc<RefCell<ClaimCallBudget>>> {
        None
    }
    fn check_cutoff(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        self.current()
    }
    fn current(&self) -> SourceCommandResult<()> {
        self.with_operation(|_| Ok(()))
    }
    fn remaining(&self) -> SourceCommandResult<(usize, usize)> {
        let remaining = OriginalAssessmentPayer::remaining(self)?.bytes;
        // ONE cumulative currency, not the per-report input ceiling.
        Ok((remaining, remaining))
    }
    fn error(&self, error: Error, _reason: &'static str) -> SourceCommandError {
        OriginalAssessmentPayer::error(self, error)
    }
}

pub(super) struct SourceLedger<'s, 'p, P: OriginalPayer> {
    session: &'s mut Session<'p>,
    payer: &'s P,
}
impl<'s, 'p, P: OriginalPayer> SourceLedger<'s, 'p, P> {
    pub(super) fn new(session: &'s mut Session<'p>, payer: &'s P) -> SourceCommandResult<Self> {
        payer.current()?;
        if !session.has_source_payer(payer) {
            return Err(unsupported(
                "PWE Session is not bound to this original source payer",
            ));
        }
        Ok(Self { session, payer })
    }
    pub(super) fn reserve(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.reserve_access(bytes)
    }
    pub(super) fn inspect(&mut self, bytes: usize, visits: usize) -> SourceCommandResult<()> {
        self.inspect_access(bytes, visits)
    }
    pub(super) fn domain_value(
        &mut self,
        value: &JsonValue,
    ) -> SourceCommandResult<serde_json::Value> {
        let raw = self.canonical(value)?;
        self.inspect_access(raw.len(), 1)?;
        let holder = raw
            .len()
            .checked_mul(64)
            .and_then(|n| n.checked_add(256))
            .ok_or(unsupported("PWE source conversion holder"))?;
        self.reserve_access(holder)?;
        serde_json::from_slice(&raw).map_err(|_| invalid("PWE source native/domain conversion"))
    }
    /// A length-only borrowed prepass pays every node before inspection. It
    /// neither sorts, validates numbers nor pretends to be schema acceptance.
    fn canonical(&mut self, value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
        // Conservative ABI-sized stack/descriptor envelope, paid before the
        // borrowed prepass recurses and again for the independent codec frame.
        let fixed = size_of::<JsonValue>()
            .checked_add(size_of::<JsonString>())
            .and_then(|n| n.checked_add(size_of::<serde_json::Value>()))
            .and_then(|n| n.checked_add(size_of::<String>() + 4 * size_of::<usize>()))
            .and_then(|n| n.checked_mul(16 * 97))
            .and_then(|n| n.checked_add(1024))
            .ok_or(unsupported("PWE source codec fixed holder"))?;
        self.reserve_access(fixed)?;
        let mut nodes = 0;
        let depth = self
            .session
            .source_allowance()
            .map_err(|e| self.payer.error(e, "PWE source depth allowance"))?
            .depth
            .min(96);
        let output = super::budget::native_envelope(self, value, 0, depth, &mut nodes)?;
        let workspace = nodes
            .checked_mul(size_of::<(usize, &(JsonString, JsonValue))>())
            .and_then(|n| n.checked_add(output))
            .and_then(|n| n.checked_add(fixed))
            .ok_or(unsupported("PWE source codec workspace"))?;
        // Spend on ORIGINAL ledgers before any codec count/sort/allocation.
        self.reserve_access(workspace)?;
        let remaining = self
            .session
            .source_allowance()
            .map_err(|e| self.payer.error(e, "PWE source codec allowance"))?;
        let (read, _) = self.payer.remaining()?;
        let limits = JsonLimits::new(
            MAX_BYTES.min(remaining.input_bytes).min(read),
            depth,
            1_000_000.min(remaining.nodes).min(remaining.work),
            4096,
        )
        .map_err(|_| unsupported("PWE original source codec allowance exhausted"))?;
        let payer = self.payer;
        let first = RefCell::new(None);
        let encoded = {
            let session = &mut *self.session;
            let mut check = || {
                payer.current().map_err(|error| {
                    let mut original = first.borrow_mut();
                    if original.is_none() {
                        *original = Some(error);
                    }
                    FoundationError::new(
                        FoundationErrorCode::BudgetExceeded,
                        "PWE source deadline/cancel",
                    )
                })
            };
            let mut admit = |bytes, visits| {
                session.admit_source_prefix(bytes, visits).map_err(|error| {
                    let mut original = first.borrow_mut();
                    if original.is_none() {
                        *original = Some(payer.error(error, "PWE source codec admission"));
                    }
                    FoundationError::new(
                        FoundationErrorCode::BudgetExceeded,
                        "PWE source codec admission",
                    )
                })
            };
            canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                value,
                CanonicalProfile::SourceRecordDigestV1,
                limits,
                workspace,
                &mut check,
                &mut admit,
            )
        };
        match (encoded, first.into_inner()) {
            (_, Some(original)) => Err(original),
            (Ok((bytes, _)), None) => {
                self.session
                    .source_allowance()
                    .map_err(|e| payer.error(e, "PWE source codec final currentness"))?;
                payer.current()?;
                Ok(bytes)
            }
            (Err(error), None) => {
                payer.current()?;
                Err(if error.code == FoundationErrorCode::BudgetExceeded {
                    unsupported("PWE source codec original allowance")
                } else {
                    invalid("PWE source canonical codec")
                })
            }
        }
    }
}
impl<P: OriginalPayer> AccessMeter for SourceLedger<'_, '_, P> {
    fn inspect_access(&mut self, bytes: usize, visits: usize) -> SourceCommandResult<()> {
        self.payer.current()?;
        self.session
            .admit_source_prefix(bytes, visits)
            .map_err(|e| self.payer.error(e, "PWE source access admission"))
    }
    fn reserve_access(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.payer.current()?;
        self.session
            .retain_source_state(bytes)
            .map_err(|e| self.payer.error(e, "PWE source holder admission"))
    }
}

/// Borrow the existing original SOURCE codec from Common caller integration.
/// No payer, session, source selection or authority is constructed here.
pub(crate) fn source_canonical(
    value: &JsonValue,
    session: &mut Session<'_>,
    payer: &SourcePayer<'_>,
) -> SourceCommandResult<Vec<u8>> {
    SourceLedger::new(session, payer)?.canonical(value)
}

/// Same admitted native codec on the actual original Assessment payer. This
/// lends resources only; native PG identity/currentness/role gates stay owned.
pub(crate) fn assessment_canonical(
    value: &JsonValue,
    session: &mut Session<'_>,
    payer: &OriginalAssessmentPayer<'_, '_>,
) -> SourceCommandResult<Vec<u8>> {
    SourceLedger::new(session, payer)?.canonical(value)
}
pub(crate) fn assessment_domain_value(
    value: &JsonValue,
    session: &mut Session<'_>,
    payer: &OriginalAssessmentPayer<'_, '_>,
) -> SourceCommandResult<serde_json::Value> {
    SourceLedger::new(session, payer)?.domain_value(value)
}

#[cfg(test)]
mod assessment_payer_tests {
    use super::*;
    use crate::source_native_cli::private_assessment::{
        OperationBudget, original_payer::with_original_pwe_session,
    };
    use std::time::Duration;
    use tos_compiler::pwe_native::Limits;

    #[test]
    fn native_codec_borrows_original_assessment_session_and_keeps_error_prefix() {
        // Actual numeric OperationBudget, never a native owner, grant, role,
        // report or source receipt. No production Session profile is selected.
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut original = OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        original.lower_remaining(4_194_304, 1_048_576).unwrap();
        let limits = Limits {
            max_input_bytes: 3_145_728,
            max_work: 65_536,
            max_state_bytes: 3_145_728,
            max_nodes: 65_536,
            max_depth: 96,
            max_refs: 128,
            max_items: 128,
            deadline,
        };
        let value =
            crate::source_command::parse(b"{\"version\":2,\"id\":\"resource-test\"}").unwrap();
        let expected = tos_foundation::canonical_raw_bytes_v1(
            b"{\"version\":2,\"id\":\"resource-test\"}",
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default(),
        )
        .unwrap();
        let result =
            with_original_pwe_session(&mut original, limits, &cancelled, |payer, session| {
                let before = payer
                    .with_operation(|operation| operation.limits())
                    .unwrap();
                let mut last = session.charges();
                for _ in 0..2 {
                    assert_eq!(
                        assessment_canonical(&value, session, payer).unwrap(),
                        expected
                    );
                    assert!(session.charges().2 > last.2 && session.charges().4 > last.4);
                    last = session.charges();
                }
                let after = payer
                    .with_operation(|operation| operation.limits())
                    .unwrap();
                assert!(after.max_input_bytes < before.max_input_bytes);
                assert_eq!(after.max_work, before.max_work); // already prepaid ONCE
                // Native frame codecs use the same actual operation directly;
                // there is no fabricated Claim wallet or second pure Session.
                let native_before = payer.remaining()?;
                let pure_before = session.charges();
                let mut native_budget = super::super::budget::FrameBudget::Assessment(payer);
                let actual = super::super::budget::CanonicalLedger::new_selected(
                    &mut native_budget,
                    None,
                    deadline,
                    &cancelled,
                )
                .canonical(&value)?;
                assert_eq!(actual, expected);
                let native_after = payer.remaining()?;
                assert!(
                    native_after.bytes < native_before.bytes
                        && native_after.work < native_before.work
                );
                assert_eq!(session.charges(), pure_before);
                payer.with_operation(|operation| {
                    let work = operation.limits()?.max_work;
                    operation.lower_remaining(1, work)
                })?;
                let spent = session.charges();
                let error = assessment_canonical(&value, session, payer).unwrap_err();
                assert!(matches!(
                    error,
                    SourceCommandError::Invalid("assessment cumulative byte budget")
                ));
                assert!(session.charges().4 > spent.4);
                Err::<(), _>(error)
            });
        assert!(matches!(
            result,
            Err(SourceCommandError::Invalid(
                "assessment cumulative byte budget"
            ))
        ));
        // The actual candidate now latches resource failure; probing limits
        // cannot turn that refused operation into a successful continuation.
        assert!(original.limits().is_err());
    }

    #[test]
    fn work_only_uses_prepaid_work_and_keeps_original_error_and_sticky_finish() {
        for lane in 0..3 {
            let deadline = Instant::now() + Duration::from_secs(30);
            let cancelled = AtomicBool::new(false);
            let mut original = OperationBudget::new(
                &serde_json::json!({"budgets": {
                    "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
                }}),
                deadline,
            )
            .unwrap();
            let selected = Limits {
                max_input_bytes: 128,
                max_nodes: 16,
                max_work: 64,
                max_refs: 8,
                max_depth: 8,
                max_items: 8,
                max_state_bytes: 128,
                deadline,
            };
            let result =
                with_original_pwe_session(&mut original, selected, &cancelled, |payer, session| {
                    let before = payer.remaining()?;
                    session.admit_source_work(3).map_err(|e| payer.error(e))?;
                    session.admit_source_work(5).map_err(|e| payer.error(e))?;
                    assert_eq!(session.charges(), (0, 0, 8, 0, 0));
                    let after = payer.remaining()?;
                    // Work was already reserved ONCE at the real operation entry.
                    assert_eq!((after.bytes, after.work), (before.bytes, before.work));
                    let error = if lane == 2 {
                        payer.with_operation(|operation| {
                            let work = operation.limits()?.max_work;
                            operation.lower_remaining(1, work)
                        })?;
                        // Actual byte refusal supplies an original error by value;
                        // preceding work-only calls must not consume or replace it.
                        session.admit_source_prefix(2, 1).unwrap_err()
                    } else {
                        let spent = session.charges();
                        let error = session.admit_source_work(57).unwrap_err();
                        assert_eq!(error, Error::ResourceLimit);
                        assert_eq!(session.charges(), spent);
                        error
                    };
                    let mapped = payer.error(error);
                    if lane == 1 {
                        drop(mapped); // swallowed mapped work refusal stays sticky
                        Ok(())
                    } else {
                        Err::<(), _>(mapped) // actual finish preserves original body Err
                    }
                });
            match lane {
                0 => assert!(matches!(
                    result,
                    Err(SourceCommandError::Unsupported(
                        "PWE Assessment Session exhausted"
                    ))
                )),
                1 => assert!(matches!(
                    result,
                    Err(SourceCommandError::Unsupported(
                        "original Assessment payer resource refusal persists"
                    ))
                )),
                _ => assert!(matches!(
                    result,
                    Err(SourceCommandError::Invalid(
                        "assessment cumulative byte budget"
                    ))
                )),
            }
        }
    }
}
