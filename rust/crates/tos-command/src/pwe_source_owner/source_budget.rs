//! Borrows the original SOURCE payer and one caller-held cumulative Session.
//! No assessment budget, source selection, grant or authority is constructed.
use super::access::AccessMeter;
use super::{MAX_BYTES, invalid, unsupported};
use crate::source_claims::ClaimCallBudget;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use std::{cell::RefCell, mem::size_of, rc::Rc, sync::atomic::AtomicBool, time::Instant};
use tos_compiler::pwe_native::{Error, Session, SourceAdmission};
use tos_foundation::{CanonicalProfile, FoundationError, FoundationErrorCode, JsonLimits, JsonString, JsonValue, canonical_bytes_v1_with_state_budget_and_visits_and_admission};

pub(crate) struct SourcePayer<'a> {
    whole: &'a Rc<RefCell<ClaimCallBudget>>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    refusal: RefCell<Option<SourceCommandError>>,
}
impl<'a> SourcePayer<'a> {
    pub(crate) fn new(whole: &'a Rc<RefCell<ClaimCallBudget>>, deadline: Instant, cancelled: &'a AtomicBool) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        whole.try_borrow().map_err(|_| invalid("PWE whole-call budget in use"))?.remaining_live()?;
        Ok(Self { whole, deadline, cancelled, refusal: RefCell::new(None) })
    }
    fn debit(&self, retained: bool, bytes: usize) -> tos_compiler::pwe_native::Result<()> {
        let result = (|| {
            active(self.deadline, self.cancelled)?;
            let mut whole = self.whole.try_borrow_mut().map_err(|_| invalid("PWE whole-call budget in use"))?;
            if retained { whole.retain(bytes) } else { whole.read(u64::try_from(bytes).map_err(|_| unsupported("PWE source byte conversion"))?) }
        })();
        result.map_err(|error| {
            let mut first = self.refusal.borrow_mut();
            if first.is_none() { *first = Some(error); }
            Error::Upstream("original source payer refused")
        })
    }
    pub(super) fn error(&self, error: Error, reason: &'static str) -> SourceCommandError {
        if let Some(original) = self.refusal.borrow_mut().take() { return original; }
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
        let whole = self.whole.try_borrow().map_err(|_| invalid("PWE whole-call budget in use"))?;
        Ok((usize::try_from(whole.remaining_read()?).unwrap_or(usize::MAX), whole.remaining_live()?))
    }
    pub(super) fn current(&self) -> SourceCommandResult<()> { active(self.deadline, self.cancelled) }
}
impl SourceAdmission for SourcePayer<'_> {
    fn inspect(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> { self.debit(false, bytes) }
    fn retain(&self, bytes: usize) -> tos_compiler::pwe_native::Result<()> { self.debit(true, bytes) }
}

pub(super) struct SourceLedger<'s, 'p, 'c> {
    session: &'s mut Session<'p>,
    payer: &'s SourcePayer<'c>,
}
impl<'s, 'p, 'c> SourceLedger<'s, 'p, 'c> {
    pub(super) fn new(session: &'s mut Session<'p>, payer: &'s SourcePayer<'c>) -> SourceCommandResult<Self> {
        payer.current()?;
        if !session.has_source_payer(payer) { return Err(unsupported("PWE Session is not bound to this original source payer")); }
        Ok(Self { session, payer })
    }
    pub(super) fn reserve(&mut self, bytes: usize) -> SourceCommandResult<()> { self.reserve_access(bytes) }
    pub(super) fn inspect(&mut self, bytes: usize, visits: usize) -> SourceCommandResult<()> { self.inspect_access(bytes, visits) }
    pub(super) fn domain_value(&mut self, value: &JsonValue) -> SourceCommandResult<serde_json::Value> {
        let raw = self.canonical(value)?;
        self.inspect_access(raw.len(), 1)?;
        let holder = raw.len().checked_mul(64).and_then(|n| n.checked_add(256)).ok_or(unsupported("PWE source conversion holder"))?;
        self.reserve_access(holder)?;
        serde_json::from_slice(&raw).map_err(|_| invalid("PWE source native/domain conversion"))
    }
    /// A length-only borrowed prepass pays every node before inspection. It
    /// neither sorts, validates numbers nor pretends to be schema acceptance.
    fn bound(&mut self, value: &JsonValue, depth: usize, depth_limit: usize, nodes: &mut usize) -> SourceCommandResult<usize> {
        self.inspect_access(0, 1)?;
        if depth > depth_limit || *nodes >= 1_000_000 { return Err(unsupported("PWE source bound visitor limit")); }
        *nodes += 1;
        let add = |a: usize, b: usize| a.checked_add(b).ok_or(unsupported("PWE source canonical bound overflow"));
        match value {
            JsonValue::Null | JsonValue::Bool(_) => Ok(5),
            // SourceRecordDigestV1 accepts bounded numeric spellings. This
            // upper envelope also covers fixed decimal/float formatting work.
            JsonValue::Number(n) => n.lexeme.len().checked_mul(8).and_then(|n| n.checked_add(4096)).ok_or(unsupported("PWE source number bound")),
            JsonValue::String(s) => s.units().len().checked_mul(6).and_then(|n| n.checked_add(2)).ok_or(unsupported("PWE source string bound")),
            JsonValue::Array(items) => {
                let mut bytes = add(items.len(), 2)?;
                for child in items { bytes = add(bytes, self.bound(child, depth + 1, depth_limit, nodes)?)?; }
                Ok(bytes)
            }
            JsonValue::Object(entries) => {
                let mut bytes = 2;
                for (key, child) in entries {
                    self.inspect_access(0, 1)?;
                    let key = key.units().len().checked_mul(6).and_then(|n| n.checked_add(4)).ok_or(unsupported("PWE source key bound"))?;
                    bytes = add(add(bytes, key)?, self.bound(child, depth + 1, depth_limit, nodes)?)?;
                }
                Ok(bytes)
            }
        }
    }
    fn canonical(&mut self, value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
        // Conservative ABI-sized stack/descriptor envelope, paid before the
        // borrowed prepass recurses and again for the independent codec frame.
        let fixed = size_of::<JsonValue>().checked_add(size_of::<JsonString>()).and_then(|n| n.checked_add(size_of::<serde_json::Value>())).and_then(|n| n.checked_add(size_of::<String>() + 4 * size_of::<usize>())).and_then(|n| n.checked_mul(16 * 97)).and_then(|n| n.checked_add(1024)).ok_or(unsupported("PWE source codec fixed holder"))?;
        self.reserve_access(fixed)?;
        let mut nodes = 0;
        let depth = self.session.source_allowance().map_err(|e| self.payer.error(e, "PWE source depth allowance"))?.depth.min(96);
        let output = self.bound(value, 0, depth, &mut nodes)?;
        let workspace = nodes.checked_mul(size_of::<(usize, &(JsonString, JsonValue))>()).and_then(|n| n.checked_add(output)).and_then(|n| n.checked_add(fixed)).ok_or(unsupported("PWE source codec workspace"))?;
        // Spend on ORIGINAL ledgers before any codec count/sort/allocation.
        self.reserve_access(workspace)?;
        let remaining = self.session.source_allowance().map_err(|e| self.payer.error(e, "PWE source codec allowance"))?;
        let (read, _) = self.payer.remaining()?;
        let limits = JsonLimits::new(MAX_BYTES.min(remaining.input_bytes).min(read), depth, 1_000_000.min(remaining.nodes).min(remaining.work), 4096).map_err(|_| unsupported("PWE original source codec allowance exhausted"))?;
        let payer = self.payer;
        let first = RefCell::new(None);
        let encoded = {
            let session = &mut *self.session;
            let mut check = || payer.current().map_err(|error| {
                let mut original = first.borrow_mut();
                if original.is_none() { *original = Some(error); }
                FoundationError::new(FoundationErrorCode::BudgetExceeded, "PWE source deadline/cancel")
            });
            let mut admit = |bytes, visits| session.admit_source_prefix(bytes, visits).map_err(|error| {
                let mut original = first.borrow_mut();
                if original.is_none() { *original = Some(payer.error(error, "PWE source codec admission")); }
                FoundationError::new(FoundationErrorCode::BudgetExceeded, "PWE source codec admission")
            });
            canonical_bytes_v1_with_state_budget_and_visits_and_admission(value, CanonicalProfile::SourceRecordDigestV1, limits, workspace, &mut check, &mut admit)
        };
        match (encoded, first.into_inner()) {
            (_, Some(original)) => Err(original),
            (Ok((bytes, _)), None) => {
                self.session.source_allowance().map_err(|e| payer.error(e, "PWE source codec final currentness"))?;
                payer.current()?;
                Ok(bytes)
            },
            (Err(error), None) => {
                payer.current()?;
                Err(if error.code == FoundationErrorCode::BudgetExceeded { unsupported("PWE source codec original allowance") } else { invalid("PWE source canonical codec") })
            },
        }
    }
}
impl AccessMeter for SourceLedger<'_, '_, '_> {
    fn inspect_access(&mut self, bytes: usize, visits: usize) -> SourceCommandResult<()> {
        self.payer.current()?;
        self.session.admit_source_prefix(bytes, visits).map_err(|e| self.payer.error(e, "PWE source access admission"))
    }
    fn reserve_access(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.payer.current()?;
        self.session.retain_source_state(bytes).map_err(|e| self.payer.error(e, "PWE source holder admission"))
    }
}
