//! Original native ledgers around the maintained foundation codecs. Prefix
//! admission precedes count/emit/parse work; successful charges are never reset.
use super::{MAX_BYTES, invalid, unsupported};
use crate::source_claims::ClaimCallBudget;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_native_cli::private_assessment::OperationBudget;
use std::{cell::RefCell, rc::Rc, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode, JsonLimits, JsonMode,
    JsonValue, canonical_bytes_v1_with_state_budget_and_visits_and_admission,
    parse_json_with_state_budget_and_admission,
};

pub(super) struct CanonicalLedger<'a> {
    budget: &'a mut dyn NativeBudgetPort,
    whole: Option<&'a Rc<RefCell<ClaimCallBudget>>>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl<'a> CanonicalLedger<'a> {
    pub(super) fn new(
        budget: &'a mut OperationBudget,
        whole: &'a Rc<RefCell<ClaimCallBudget>>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self::new_selected(budget, Some(whole), deadline, cancelled)
    }
    pub(super) fn new_selected(
        budget: &'a mut dyn NativeBudgetPort,
        whole: Option<&'a Rc<RefCell<ClaimCallBudget>>>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Self {
        Self {
            budget,
            whole,
            deadline,
            cancelled,
        }
    }
    pub(super) fn remaining_live(&mut self) -> SourceCommandResult<usize> {
        match self.whole {
            Some(whole) => whole
                .try_borrow()
                .map_err(|_| invalid("PWE whole-call budget in use"))?
                .remaining_live(),
            None => self.budget.remaining_bytes(),
        }
    }
    fn limits(&mut self, mut limits: JsonLimits) -> SourceCommandResult<JsonLimits> {
        active(self.deadline, self.cancelled)?;
        let remaining = self.budget.limits()?;
        let read = match self.whole {
            Some(whole) => whole
                .try_borrow()
                .map_err(|_| invalid("PWE whole-call budget in use"))?
                .remaining_read()?,
            None => remaining.max_input_bytes as u64,
        };
        let bytes = limits
            .max_bytes
            .min(remaining.max_input_bytes)
            .min(usize::try_from(read).unwrap_or(usize::MAX));
        // Tighten the particular existing route, never enlarge its ceilings.
        limits.max_bytes = bytes;
        limits.max_visits = limits.max_visits.min(remaining.max_work);
        JsonLimits::new(
            limits.max_bytes,
            limits.max_depth,
            limits.max_visits,
            limits.max_integer_digits,
        )
        .map_err(|_| unsupported("PWE original codec allowance exhausted"))
    }
    /// Both traversals use the foundation's existing visitor. Its count pass
    /// checks output + scratch against the ORIGINAL remaining live allowance
    /// before try_reserve_exact; emission checks actual capacity against it too.
    /// Claim callbacks debit read/work; their returned allocation joins the
    /// separate retained holders before escape. Assessment reserves workspace
    /// first, since input and retained state debit one original byte currency.
    pub(super) fn canonical(&mut self, value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
        let limits = JsonLimits::new(MAX_BYTES, 96, 1_000_000, 4096)
            .map_err(|_| invalid("PWE canonical limits"))?;
        self.encode(value, CanonicalProfile::SourceRecordDigestV1, limits)
    }
    /// Preserve cmd::same's existing input profile and ceilings, now with both
    /// held ledgers and simultaneous canonical holders admitted prospectively.
    pub(super) fn same(
        &mut self,
        left: &JsonValue,
        right: &JsonValue,
    ) -> SourceCommandResult<bool> {
        let limits = JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        };
        let left = self.encode(left, CanonicalProfile::SourceCommandInputV1, limits)?;
        let right = self.encode(right, CanonicalProfile::SourceCommandInputV1, limits)?;
        let work = left
            .len()
            .checked_add(right.len())
            .ok_or(unsupported("PWE canonical comparison overflow"))?;
        self.inspect_access(work, 1)?;
        Ok(left == right)
    }
    fn encode(
        &mut self,
        value: &JsonValue,
        profile: CanonicalProfile,
        limits: JsonLimits,
    ) -> SourceCommandResult<Vec<u8>> {
        // Preserve the legacy Claim route's first-error ordering.
        let claim_limits = if self.whole.is_some() {
            Some(self.limits(limits)?)
        } else {
            None
        };
        let state = if self.whole.is_none() {
            let fixed = std::mem::size_of::<JsonValue>()
                .checked_add(std::mem::size_of::<tos_foundation::JsonString>())
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<serde_json::Value>()
                            + std::mem::size_of::<String>()
                            + 4 * std::mem::size_of::<usize>(),
                    )
                })
                .and_then(|n| n.checked_mul(16 * 97))
                .and_then(|n| n.checked_add(1024))
                .ok_or(unsupported("PWE Assessment codec frame holder"))?;
            self.reserve_access(fixed)?;
            let mut nodes = 0;
            let output = native_envelope(self, value, 0, limits.max_depth.min(96), &mut nodes)?;
            let workspace = nodes
                .checked_mul(std::mem::size_of::<(
                    usize,
                    &(tos_foundation::JsonString, JsonValue),
                )>())
                .and_then(|n| n.checked_add(output))
                .and_then(|n| n.checked_add(fixed))
                .ok_or(unsupported("PWE Assessment codec workspace"))?;
            // ONE byte currency: reserve overlap BEFORE count/sort/emission.
            self.reserve_access(workspace)?;
            workspace
        } else {
            self.remaining_live()?
        };
        let limits = match claim_limits {
            Some(limits) => limits,
            None => self.limits(limits)?,
        };
        let deadline = self.deadline;
        let cancelled = self.cancelled;
        let mut admission_error = None;
        let encoded = {
            let mut check = || cutoff(deadline, cancelled);
            let budget = &mut *self.budget;
            let whole = self.whole;
            let mut admit = |bytes, visits| {
                admit_prefix(budget, whole, deadline, cancelled, bytes, visits).map_err(|error| {
                    admission_error = Some(error);
                    codec_refusal()
                })
            };
            canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                value, profile, limits, state, &mut check, &mut admit,
            )
        };
        let (bytes, _) =
            encoded.map_err(|error| codec_error(error, admission_error, deadline, cancelled))?;
        // Claim state was bounded prospectively by the visitor and has a
        // separate live currency. Assessment already paid its overlapping
        // workspace before the codec and cannot retain-after-encode.
        if self.whole.is_some() {
            self.reserve_access(bytes.capacity())?;
        }
        Ok(bytes)
    }
    pub(super) fn hash_bytes(&mut self, bytes: &[u8]) -> SourceCommandResult<Digest256> {
        self.inspect_bytes(bytes.len())?;
        Ok(Digest256::of_bytes(bytes))
    }
    pub(super) fn digest(&mut self, value: &JsonValue) -> SourceCommandResult<Digest256> {
        let bytes = self.canonical(value)?;
        self.hash_bytes(&bytes)
    }
    pub(super) fn domain_value(
        &mut self,
        value: &JsonValue,
    ) -> SourceCommandResult<serde_json::Value> {
        let bytes = self.canonical(value)?;
        self.decode_domain(&bytes)
    }
    pub(super) fn decode_domain(&mut self, bytes: &[u8]) -> SourceCommandResult<serde_json::Value> {
        self.inspect_bytes(bytes.len())?;
        // Original logical conversion-holder fee, paid BEFORE deserialization.
        // Canonical input allocation is already held by the same whole ledger.
        self.reserve_value(bytes.len())?;
        serde_json::from_slice(bytes).map_err(|_| invalid("PWE native/domain conversion"))
    }
    pub(super) fn parse_selected(&mut self, bytes: &[u8]) -> SourceCommandResult<JsonValue> {
        let limits = self.limits(
            JsonLimits::new(MAX_BYTES, 96, 1_000_000, 4096)
                .map_err(|_| invalid("PWE parser limits"))?,
        )?;
        // Reserve the returned native tree before its parser allocates. The
        // parser's actual state cap is this reservation, not a new allowance.
        let reserved = self.reserve_value(bytes.len())?;
        let deadline = self.deadline;
        let cancelled = self.cancelled;
        let mut admission_error = None;
        let parsed = {
            let mut check = || cutoff(deadline, cancelled);
            let budget = &mut *self.budget;
            let whole = self.whole;
            let mut admit = |bytes, visits| {
                admit_prefix(budget, whole, deadline, cancelled, bytes, visits).map_err(|error| {
                    admission_error = Some(error);
                    codec_refusal()
                })
            };
            parse_json_with_state_budget_and_admission(
                bytes,
                JsonMode::PublishedStrict,
                limits,
                reserved,
                &mut check,
                &mut admit,
            )
        };
        parsed
            .map(|document| document.into_root())
            .map_err(|error| codec_error(error, admission_error, deadline, cancelled))
    }
    /// Preserve the existing serde compact-size measure. Its full borrowed
    /// value walk, strings, keys and numeric lexemes are admitted BEFORE the
    /// serializer counts them; the counter retains no output body.
    pub(super) fn encoded_len(&mut self, value: &serde_json::Value) -> SourceCommandResult<usize> {
        let mut visits = 0;
        self.admit_serde(value, 0, &mut visits)?;
        struct Counter(usize);
        impl std::io::Write for Counter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 = self
                    .0
                    .checked_add(bytes.len())
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or_else(|| std::io::Error::other("PWE encoded byte cap"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut counter = Counter(0);
        serde_json::to_writer(&mut counter, value)
            .map_err(|_| unsupported("PWE encoded value byte cap"))?;
        Ok(counter.0)
    }
    fn admit_serde(
        &mut self,
        value: &serde_json::Value,
        depth: usize,
        visits: &mut usize,
    ) -> SourceCommandResult<()> {
        active(self.deadline, self.cancelled)?;
        self.budget.charge_work(1)?;
        if depth > 96 || *visits >= 1_000_000 {
            return Err(unsupported("PWE size visitor limit"));
        }
        *visits += 1;
        match value {
            serde_json::Value::Null | serde_json::Value::Bool(_) => self.admit_text(5)?,
            // arbitrary_precision is already enabled by tos-command; as_str
            // borrows the lexeme without scanning, formatting or copying it.
            serde_json::Value::Number(number) => self.admit_text(number.as_str().len())?,
            serde_json::Value::String(text) => self.admit_text(text.len())?,
            serde_json::Value::Array(values) => {
                self.admit_text(
                    values
                        .len()
                        .checked_add(2)
                        .ok_or(unsupported("PWE size array overflow"))?,
                )?;
                for value in values {
                    self.admit_serde(value, depth + 1, visits)?;
                }
            }
            serde_json::Value::Object(values) => {
                let punctuation = values
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(unsupported("PWE size object overflow"))?;
                self.admit_text(punctuation)?;
                for (key, value) in values {
                    self.admit_text(key.len())?;
                    self.admit_serde(value, depth + 1, visits)?;
                }
            }
        }
        Ok(())
    }
    fn admit_text(&mut self, bytes: usize) -> SourceCommandResult<()> {
        if bytes > MAX_BYTES {
            return Err(unsupported("PWE size input cap"));
        }
        // Each UTF8 input byte may become at most six JSON escape bytes.
        // Reserve that serialization plus its input/count work and delimiters.
        let work = bytes
            .checked_mul(8)
            .and_then(|n| n.checked_add(2))
            .ok_or(unsupported("PWE size work overflow"))?;
        admit_prefix(
            self.budget,
            self.whole,
            self.deadline,
            self.cancelled,
            bytes,
            work,
        )
    }
    pub(super) fn inspect_access(
        &mut self,
        bytes: usize,
        visits: usize,
    ) -> SourceCommandResult<()> {
        admit_prefix(
            self.budget,
            self.whole,
            self.deadline,
            self.cancelled,
            bytes,
            visits,
        )
    }
    pub(super) fn account_copy_input(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.budget.charge_bytes(bytes)?;
        if let Some(whole) = self.whole {
            whole
                .try_borrow_mut()
                .map_err(|_| invalid("PWE whole-call budget in use"))?
                .read(u64::try_from(bytes).map_err(|_| invalid("PWE source size conversion"))?)?;
        }
        Ok(())
    }
    pub(super) fn reserve_access(&mut self, bytes: usize) -> SourceCommandResult<()> {
        match self.whole {
            Some(whole) => whole
                .try_borrow_mut()
                .map_err(|_| invalid("PWE whole-call budget in use"))?
                .retain(bytes),
            None => self.budget.charge_bytes(bytes),
        }
    }
    fn inspect_bytes(&mut self, bytes: usize) -> SourceCommandResult<()> {
        if bytes > MAX_BYTES {
            return Err(unsupported("PWE codec byte cap"));
        }
        active(self.deadline, self.cancelled)?;
        self.budget.charge_work(bytes)?;
        self.budget.charge_bytes(bytes)?;
        if let Some(whole) = self.whole {
            whole
                .try_borrow_mut()
                .map_err(|_| invalid("PWE whole-call budget in use"))?
                .read(
                    u64::try_from(bytes).map_err(|_| unsupported("PWE codec byte conversion"))?,
                )?;
        }
        Ok(())
    }
    fn reserve_value(&mut self, bytes: usize) -> SourceCommandResult<usize> {
        let reserved = bytes
            .checked_mul(64)
            .and_then(|n| n.checked_add(256))
            .ok_or(unsupported("PWE conversion holder overflow"))?;
        self.reserve_access(reserved)?;
        Ok(reserved)
    }
}
fn admit_prefix(
    budget: &mut dyn NativeBudgetPort,
    whole: Option<&Rc<RefCell<ClaimCallBudget>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
    bytes: usize,
    visits: usize,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let work = bytes
        .checked_add(visits)
        .ok_or(unsupported("PWE codec work overflow"))?;
    // Ordered successful debits survive failure of any later debit or codec
    // prefix. No local copy, rollback, refill or ceiling increase is possible.
    budget.charge_work(work)?;
    budget.charge_bytes(bytes)?;
    if let Some(whole) = whole {
        whole
            .try_borrow_mut()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .read(u64::try_from(bytes).map_err(|_| unsupported("PWE codec byte conversion"))?)?;
    }
    Ok(())
}
fn codec_refusal() -> FoundationError {
    FoundationError::new(FoundationErrorCode::BudgetExceeded, "PWE codec admission")
}
fn cutoff(deadline: Instant, cancelled: &AtomicBool) -> tos_foundation::Result<()> {
    active(deadline, cancelled).map_err(|_| codec_refusal())
}
fn codec_error(
    error: FoundationError,
    admission_error: Option<SourceCommandError>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandError {
    if let Some(error) = admission_error {
        return error;
    }
    if let Err(error) = active(deadline, cancelled) {
        return error;
    }
    if error.code == FoundationErrorCode::BudgetExceeded {
        unsupported("PWE original codec resource allowance")
    } else {
        invalid("PWE source codec rejected document")
    }
}

// This erased resource port has ONE concrete implementation. Private frame
// constructors accept the actual OriginalAssessmentPayer before erasure.
pub(super) trait AssessmentOperationLoan: super::source_budget::OriginalPayer {
    fn lend_payer(
        &self,
        run: &mut dyn FnMut(
            &crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer<
                '_,
                '_,
            >,
        ) -> SourceCommandResult<()>,
    ) -> SourceCommandResult<()>;
    fn lend(
        &self,
        run: &mut dyn FnMut(&mut OperationBudget) -> SourceCommandResult<()>,
    ) -> SourceCommandResult<()>;
}
impl AssessmentOperationLoan
    for crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer<
        '_,
        '_,
    >
{
    fn lend_payer(
        &self,
        run: &mut dyn FnMut(
            &crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer<
                '_,
                '_,
            >,
        ) -> SourceCommandResult<()>,
    ) -> SourceCommandResult<()> {
        run(self)
    }
    fn lend(
        &self,
        run: &mut dyn FnMut(&mut OperationBudget) -> SourceCommandResult<()>,
    ) -> SourceCommandResult<()> {
        self.with_operation(|operation| run(operation))
    }
}
pub(super) enum FrameBudget<'a> {
    Claim(&'a mut OperationBudget),
    Assessment(&'a dyn AssessmentOperationLoan),
}
impl FrameBudget<'_> {
    pub(super) fn with_operation<T>(
        &mut self,
        run: impl FnOnce(&mut OperationBudget) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        match self {
            Self::Claim(operation) => run(operation),
            Self::Assessment(payer) => {
                let mut run = Some(run);
                let mut value = None;
                payer.lend(&mut |operation| {
                    let body = run
                        .take()
                        .ok_or(invalid("PWE repeated original operation loan"))?;
                    value = Some(body(operation)?);
                    Ok(())
                })?;
                value.ok_or(invalid("PWE original operation loan did not run"))
            }
        }
    }
    pub(super) fn with_assessment_payer<T>(
        &self,
        run: impl FnOnce(
            &crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer<
                '_,
                '_,
            >,
        ) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        let Self::Assessment(payer) = self else {
            return Err(invalid("PWE Assessment payer loan on Claim route"));
        };
        let mut body = Some(run);
        let mut result = None;
        payer.lend_payer(&mut |actual| {
            let body = body
                .take()
                .ok_or(invalid("PWE repeated native source payer loan"))?;
            result = Some(body(actual)?);
            Ok(())
        })?;
        result.ok_or(invalid("PWE native source payer loan absent"))
    }
    pub(super) fn charge_work(&mut self, work: usize) -> SourceCommandResult<()> {
        self.with_operation(|operation| operation.charge_work(work))
    }
    pub(super) fn charge_bytes(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.with_operation(|operation| operation.charge_bytes(bytes))
    }
    pub(super) fn limits(
        &mut self,
    ) -> SourceCommandResult<tos_validation::assessment::AssessmentLimits> {
        self.with_operation(|operation| operation.limits())
    }
    pub(super) fn matches_assessment(
        &self,
        payer: &dyn super::source_budget::OriginalPayer,
    ) -> bool {
        match self {
            Self::Assessment(actual) => std::ptr::addr_eq(actual.admission(), payer.admission()),
            Self::Claim(_) => false,
        }
    }
}

pub(super) fn native_envelope<M: super::access::AccessMeter>(
    meter: &mut M,
    value: &JsonValue,
    depth: usize,
    depth_limit: usize,
    nodes: &mut usize,
) -> SourceCommandResult<usize> {
    meter.inspect_access(0, 1)?;
    if depth > depth_limit || *nodes >= 1_000_000 {
        return Err(unsupported("PWE source bound visitor limit"));
    }
    *nodes += 1;
    let add = |a: usize, b: usize| {
        a.checked_add(b)
            .ok_or(unsupported("PWE source canonical bound overflow"))
    };
    match value {
        JsonValue::Null | JsonValue::Bool(_) => Ok(5),
        // SourceRecordDigestV1 accepts bounded numeric spellings. This
        // upper envelope also covers fixed decimal/float formatting work.
        JsonValue::Number(n) => n
            .lexeme
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(4096))
            .ok_or(unsupported("PWE source number bound")),
        JsonValue::String(s) => s
            .units()
            .len()
            .checked_mul(6)
            .and_then(|n| n.checked_add(2))
            .ok_or(unsupported("PWE source string bound")),
        JsonValue::Array(items) => {
            let mut bytes = add(items.len(), 2)?;
            for child in items {
                bytes = add(
                    bytes,
                    native_envelope(meter, child, depth + 1, depth_limit, nodes)?,
                )?;
            }
            Ok(bytes)
        }
        JsonValue::Object(entries) => {
            let mut bytes = 2;
            for (key, child) in entries {
                meter.inspect_access(0, 1)?;
                let key = key
                    .units()
                    .len()
                    .checked_mul(6)
                    .and_then(|n| n.checked_add(4))
                    .ok_or(unsupported("PWE source key bound"))?;
                bytes = add(
                    add(bytes, key)?,
                    native_envelope(meter, child, depth + 1, depth_limit, nodes)?,
                )?;
            }
            Ok(bytes)
        }
    }
}

pub(super) trait OperationAccess {
    fn lend_operation(
        &mut self,
        run: &mut dyn FnMut(&mut OperationBudget) -> SourceCommandResult<()>,
    ) -> SourceCommandResult<()>;
}
impl OperationAccess for FrameBudget<'_> {
    fn lend_operation(
        &mut self,
        run: &mut dyn FnMut(&mut OperationBudget) -> SourceCommandResult<()>,
    ) -> SourceCommandResult<()> {
        self.with_operation(|operation| run(operation))
    }
}
pub(super) fn with_operation_access<T>(
    access: &mut dyn OperationAccess,
    run: impl FnOnce(&mut OperationBudget) -> SourceCommandResult<T>,
) -> SourceCommandResult<T> {
    let mut body = Some(run);
    let mut result = None;
    access.lend_operation(&mut |operation| {
        let body = body
            .take()
            .ok_or(invalid("PWE repeated closed operation loan"))?;
        result = Some(body(operation)?);
        Ok(())
    })?;
    result.ok_or(invalid("PWE closed operation loan absent"))
}

// Native codec work borrows this actual resource port. Only FrameBudget is
// supplied by protected frame constructors; this port confers no authority.
pub(super) trait NativeBudgetPort {
    fn charge_work(&mut self, work: usize) -> SourceCommandResult<()>;
    fn charge_bytes(&mut self, bytes: usize) -> SourceCommandResult<()>;
    fn limits(&mut self) -> SourceCommandResult<tos_validation::assessment::AssessmentLimits>;
    fn remaining_bytes(&self) -> SourceCommandResult<usize>;
}
impl NativeBudgetPort for FrameBudget<'_> {
    fn charge_work(&mut self, work: usize) -> SourceCommandResult<()> {
        FrameBudget::charge_work(self, work)
    }
    fn charge_bytes(&mut self, bytes: usize) -> SourceCommandResult<()> {
        FrameBudget::charge_bytes(self, bytes)
    }
    fn limits(&mut self) -> SourceCommandResult<tos_validation::assessment::AssessmentLimits> {
        FrameBudget::limits(self)
    }
    fn remaining_bytes(&self) -> SourceCommandResult<usize> {
        match self {
            FrameBudget::Assessment(payer) => Ok(payer.remaining()?.0),
            FrameBudget::Claim(_) => Err(invalid("PWE Claim state must use its actual wallet")),
        }
    }
}

impl NativeBudgetPort for OperationBudget {
    fn charge_work(&mut self, work: usize) -> SourceCommandResult<()> {
        OperationBudget::charge_work(self, work)
    }
    fn charge_bytes(&mut self, bytes: usize) -> SourceCommandResult<()> {
        OperationBudget::charge_bytes(self, bytes)
    }
    fn limits(&mut self) -> SourceCommandResult<tos_validation::assessment::AssessmentLimits> {
        OperationBudget::limits(self)
    }
    fn remaining_bytes(&self) -> SourceCommandResult<usize> {
        // Raw legacy reader construction always has its genuine Claim wallet.
        Err(invalid(
            "PWE raw operation requires a selected state-currency port",
        ))
    }
}
