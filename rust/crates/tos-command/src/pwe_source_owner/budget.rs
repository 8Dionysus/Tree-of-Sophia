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
    budget: &'a mut OperationBudget,
    whole: &'a Rc<RefCell<ClaimCallBudget>>,
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
        Self {
            budget,
            whole,
            deadline,
            cancelled,
        }
    }
    fn limits(&self, mut limits: JsonLimits) -> SourceCommandResult<JsonLimits> {
        active(self.deadline, self.cancelled)?;
        let remaining = self.budget.limits()?;
        let read = self
            .whole
            .try_borrow()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .remaining_read()?;
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
    /// During the codec callback only read/work counters change, not live state.
    /// The returned allocation joins retained holders before it escapes here.
    pub(super) fn canonical(&mut self, value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
        let limits = JsonLimits::new(MAX_BYTES, 96, 1_000_000, 4096)
            .map_err(|_| invalid("PWE canonical limits"))?;
        self.encode(value, CanonicalProfile::SourceRecordDigestV1, limits)
    }
    /// Preserve cmd::same's existing input profile and ceilings, now with both
    /// held ledgers and simultaneous canonical holders admitted prospectively.
    pub(super) fn same(&mut self, left: &JsonValue, right: &JsonValue) -> SourceCommandResult<bool> {
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
        self.budget.charge_work(work)?;
        self.whole
            .try_borrow_mut()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .read(u64::try_from(work).map_err(|_| unsupported("PWE comparison byte conversion"))?)?;
        Ok(left == right)
    }
    fn encode(
        &mut self,
        value: &JsonValue,
        profile: CanonicalProfile,
        limits: JsonLimits,
    ) -> SourceCommandResult<Vec<u8>> {
        let limits = self.limits(limits)?;
        let available = self
            .whole
            .try_borrow()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .remaining_live()?;
        let deadline = self.deadline;
        let cancelled = self.cancelled;
        let mut admission_error = None;
        let encoded = {
            let mut check = || cutoff(deadline, cancelled);
            let budget = &mut *self.budget;
            let whole = self.whole;
            let mut admit = |bytes, visits| {
                admit_prefix(budget, whole, deadline, cancelled, bytes, visits)
                    .map_err(|error| {
                        admission_error = Some(error);
                        codec_refusal()
                    })
            };
            canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                value,
                profile,
                limits,
                available,
                &mut check,
                &mut admit,
            )
        };
        let (bytes, _) = encoded
            .map_err(|error| codec_error(error, admission_error, deadline, cancelled))?;
        // The same allowance was checked prospectively by the visitor. No
        // other allocation/owner callback can interleave with this synchronous
        // count/emit call. Account its actual retained capacity before use.
        self.whole
            .try_borrow_mut()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .retain(bytes.capacity())?;
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
    pub(super) fn decode_domain(
        &mut self,
        bytes: &[u8],
    ) -> SourceCommandResult<serde_json::Value> {
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
                admit_prefix(budget, whole, deadline, cancelled, bytes, visits)
                    .map_err(|error| {
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
    pub(super) fn encoded_len(
        &mut self,
        value: &serde_json::Value,
    ) -> SourceCommandResult<usize> {
        let mut visits = 0;
        self.admit_serde(value, 0, &mut visits)?;
        struct Counter(usize);
        impl std::io::Write for Counter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 = self.0
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
                    values.len().checked_add(2).ok_or(unsupported("PWE size array overflow"))?,
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
            self.budget, self.whole, self.deadline, self.cancelled, bytes, work,
        )
    }
    fn inspect_bytes(&mut self, bytes: usize) -> SourceCommandResult<()> {
        if bytes > MAX_BYTES {
            return Err(unsupported("PWE codec byte cap"));
        }
        active(self.deadline, self.cancelled)?;
        self.budget.charge_work(bytes)?;
        self.budget.charge_bytes(bytes)?;
        self.whole
            .try_borrow_mut()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .read(u64::try_from(bytes).map_err(|_| unsupported("PWE codec byte conversion"))?)
    }
    fn reserve_value(&mut self, bytes: usize) -> SourceCommandResult<usize> {
        let reserved = bytes
            .checked_mul(64)
            .and_then(|n| n.checked_add(256))
            .ok_or(unsupported("PWE conversion holder overflow"))?;
        self.whole
            .try_borrow_mut()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .retain(reserved)?;
        Ok(reserved)
    }
}
fn admit_prefix(
    budget: &mut OperationBudget,
    whole: &Rc<RefCell<ClaimCallBudget>>,
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
    whole
        .try_borrow_mut()
        .map_err(|_| invalid("PWE whole-call budget in use"))?
        .read(u64::try_from(bytes).map_err(|_| unsupported("PWE codec byte conversion"))?)
}
fn codec_refusal() -> FoundationError {
    FoundationError::new(
        FoundationErrorCode::BudgetExceeded,
        "PWE codec admission",
    )
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
