//! Admission for the maintained current-subject/scope selector. This lends the
//! original ledgers; selected data is neither a grant nor an assessment report.
use super::{OperationBudget, OwnerVersion, ParsedRequest, owner_reference};
use crate::source_claims::ClaimCallBudget;
use crate::source_command::{
    self as cmd, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_creation_store::active;
use std::{
    cell::RefCell, collections::BTreeMap, mem::size_of, rc::Rc, sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode, JsonLimits, JsonString,
    JsonValue, canonical_bytes_v1_with_state_budget_and_visits_and_admission,
};

pub(super) struct ScopeAccess<'a> {
    ledger: Option<Ledger<'a>>,
}
struct Ledger<'a> {
    operation: &'a mut OperationBudget,
    whole: Option<&'a Rc<RefCell<ClaimCallBudget>>>,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
fn exhausted() -> Error {
    Error::Unsupported("assessment scope original allowance exhausted")
}
impl ScopeAccess<'_> {
    pub(super) fn plain() -> Self {
        Self { ledger: None }
    }
    pub(super) fn admitted<'a>(
        operation: &'a mut OperationBudget,
        whole: &'a Rc<RefCell<ClaimCallBudget>>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> ScopeAccess<'a> {
        ScopeAccess {
            ledger: Some(Ledger {
                operation,
                whole: Some(whole),
                deadline,
                cancelled,
            }),
        }
    }
    pub(super) fn admitted_operation<'a>(
        operation: &'a mut OperationBudget,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> ScopeAccess<'a> {
        ScopeAccess { ledger: Some(Ledger { operation, whole: None, deadline, cancelled }) }
    }
    pub(super) fn inspect(&mut self, bytes: usize, visits: usize) -> Result<()> {
        if let Some(ledger) = &mut self.ledger {
            active(ledger.deadline, ledger.cancelled)?;
            ledger
                .operation
                .charge_work(bytes.checked_add(visits).ok_or_else(exhausted)?)?;
            ledger.operation.charge_bytes(bytes)?;
            if let Some(whole) = ledger.whole {
                whole.try_borrow_mut().map_err(|_| exhausted())?
                    .read(u64::try_from(bytes).map_err(|_| exhausted())?)?;
            }
        }
        Ok(())
    }
    pub(super) fn retain(&mut self, bytes: usize) -> Result<()> {
        if let Some(ledger) = &mut self.ledger {
            active(ledger.deadline, ledger.cancelled)?;
            if let Some(whole) = ledger.whole {
                whole.try_borrow_mut().map_err(|_| exhausted())?.retain(bytes)?;
            } else {
                // Assessment has ONE original byte currency. Pay holders
                // before construction, not against a fabricated Claim wallet.
                ledger.operation.charge_bytes(bytes)?;
            }
        }
        Ok(())
    }
    pub(super) fn text_equal(&mut self, left: &str, right: &str) -> Result<bool> {
        self.inspect(
            left.len().checked_add(right.len()).ok_or_else(exhausted)?,
            1,
        )?;
        Ok(left == right)
    }
    pub(super) fn current<'v>(
        &mut self,
        all: &'v BTreeMap<String, JsonValue>,
        key: &str,
    ) -> Result<&'v JsonValue> {
        let selected = if self.ledger.is_none() {
            all.get(key)
        } else {
            // Pinned Rust1.98.1 BTreeMap::get descends one path, with at most
            // 11 comparisons per node. A nonempty valid tree of L levels
            // has >= 2^L-1 keys, so bit_width(N) bounds its levels. Each str
            // comparison reads at most key.len() bytes from EITHER operand
            // and both length fields. Pay that whole logical bound BEFORE
            // std lookup, including an absent key; never scan/copy the map.
            // This quotes comparison input, not machine instruction/RSS time.
            let levels = (usize::BITS - all.len().leading_zeros()) as usize;
            let comparisons = all.len().min(levels.checked_mul(11).ok_or_else(exhausted)?);
            let bytes = key.len().checked_add(size_of::<usize>())
                .and_then(|n| n.checked_mul(2))
                .and_then(|n| n.checked_mul(comparisons)).ok_or_else(exhausted)?;
            self.inspect(bytes, comparisons.checked_add(levels)
                .and_then(|n| n.checked_add(1)).ok_or_else(exhausted)?)?;
            all.get(key)
        };
        selected.ok_or(Error::Denied(
            "assessment subject has no exact selected current record",
        ))
    }
    pub(super) fn get<'v>(
        &mut self,
        value: &'v JsonValue,
        key: &str,
    ) -> Result<Option<&'v JsonValue>> {
        if self.ledger.is_none() {
            return Ok(value.object_get(key));
        }
        self.inspect(0, 1)?;
        let Some(fields) = value.as_object() else {
            return Ok(None);
        };
        for (candidate, value) in fields {
            self.inspect(0, 1)?;
            if let Some(candidate) = candidate.as_str() {
                if self.text_equal(candidate, key)? {
                    return Ok(Some(value));
                }
            }
        }
        Ok(None)
    }
    #[track_caller]
    pub(super) fn field<'v>(&mut self, value: &'v JsonValue, key: &str) -> Result<&'v JsonValue> {
        if self.ledger.is_none() {
            return cmd::field(value, key);
        }
        self.get(value, key)?.ok_or_else(|| {
            let location = std::panic::Location::caller();
            Error::InvalidAt {
                reason: "missing object field",
                file: "source_native_private_assessment_scope.rs",
                line: location.line(),
            }
        })
    }
    pub(super) fn text<'v>(&mut self, value: &'v JsonValue, key: &str) -> Result<&'v str> {
        self.field(value, key)?
            .as_str()
            .ok_or(Error::Invalid("expected text field"))
    }
    pub(super) fn exact_keys(&mut self, value: &JsonValue, keys: &[&str]) -> Result<()> {
        if self.ledger.is_none() {
            return cmd::exact_keys(value, keys);
        }
        self.inspect(0, 1)?;
        let fields = value.as_object().ok_or(Error::Invalid("expected object"))?;
        if fields.len() != keys.len() {
            return Err(Error::Invalid("unexpected object fields"));
        }
        for (key, _) in fields {
            let mut found = false;
            for allowed in keys {
                if self.text_equal(key.as_str().unwrap_or(""), allowed)? {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(Error::Invalid("unexpected object fields"));
            }
        }
        Ok(())
    }
    // Pay the actual borrowed source buffers before their future clone/equality.
    // Logical retained holders are conservative source capacities, not RSS.
    fn prepay_value(
        &mut self,
        value: &JsonValue,
        depth: usize,
        visits: &mut usize,
        copied: bool,
    ) -> Result<()> {
        self.inspect(0, 2)?; // this walk and the later clone/equality node
        let limits = JsonLimits::default();
        if depth > limits.max_depth || *visits >= limits.max_visits {
            return Err(exhausted());
        }
        *visits += 1;
        if copied {
            self.retain(size_of::<JsonValue>())?;
        }
        match value {
            JsonValue::Null | JsonValue::Bool(_) => {}
            JsonValue::Number(number) => {
                self.inspect(number.lexeme.len().checked_mul(2).ok_or_else(exhausted)?, 1)?;
                if copied {
                    self.retain(number.lexeme.capacity())?;
                }
            }
            JsonValue::String(text) => self.prepay_string(text, copied)?,
            JsonValue::Array(values) => {
                if copied {
                    self.retain(
                        values
                            .capacity()
                            .checked_mul(size_of::<JsonValue>())
                            .ok_or_else(exhausted)?,
                    )?;
                }
                for value in values {
                    self.prepay_value(value, depth + 1, visits, copied)?;
                }
            }
            JsonValue::Object(fields) => {
                if copied {
                    self.retain(
                        fields
                            .capacity()
                            .checked_mul(size_of::<(JsonString, JsonValue)>())
                            .ok_or_else(exhausted)?,
                    )?;
                }
                for (key, value) in fields {
                    self.prepay_string(key, copied)?;
                    self.prepay_value(value, depth + 1, visits, copied)?;
                }
            }
        }
        Ok(())
    }
    fn prepay_string(&mut self, text: &JsonString, copied: bool) -> Result<()> {
        let bytes = text
            .units()
            .len()
            .checked_mul(size_of::<u16>())
            .and_then(|n| n.checked_add(text.as_str().map_or(0, str::len)))
            .ok_or_else(exhausted)?;
        self.inspect(bytes.checked_mul(2).ok_or_else(exhausted)?, 1)?;
        if copied {
            self.retain(text.retained_storage_bytes().map_err(|_| exhausted())?)?;
        }
        Ok(())
    }
    pub(super) fn copied(&mut self, value: &JsonValue) -> Result<JsonValue> {
        if self.ledger.is_some() {
            self.prepay_value(value, 0, &mut 0, true)?;
        }
        Ok(value.clone())
    }
    pub(super) fn optional_equal(
        &mut self,
        left: Option<&JsonValue>,
        right: Option<&JsonValue>,
    ) -> Result<bool> {
        if self.ledger.is_some() {
            if let Some(value) = left {
                self.prepay_value(value, 0, &mut 0, false)?;
            }
            if let Some(value) = right {
                self.prepay_value(value, 0, &mut 0, false)?;
            }
            self.inspect(0, 1)?;
        }
        Ok(left == right)
    }
    fn canonical(&mut self, value: &JsonValue) -> Result<Vec<u8>> {
        if self.ledger.as_ref().is_some_and(|ledger| ledger.whole.is_none()) {
            return self.canonical_operation(value);
        }
        let Some(ledger) = &mut self.ledger else {
            return cmd::canonical(value);
        };
        active(ledger.deadline, ledger.cancelled)?;
        let remaining = ledger.operation.limits()?;
        let whole = ledger.whole.ok_or_else(exhausted)?;
        let (read, live) = {
            let whole = whole.try_borrow().map_err(|_| exhausted())?;
            (whole.remaining_read()?, whole.remaining_live()?)
        };
        let limits = JsonLimits {
            max_bytes: 8_388_608usize
                .min(remaining.max_input_bytes)
                .min(usize::try_from(read).unwrap_or(usize::MAX)),
            max_visits: JsonLimits::default().max_visits.min(remaining.max_work),
            ..JsonLimits::default()
        };
        let mut refusal = None;
        let deadline = ledger.deadline;
        let cancelled = ledger.cancelled;
        let mut cutoff = || {
            active(deadline, cancelled).map_err(|_| {
                FoundationError::new(FoundationErrorCode::BudgetExceeded, "scope codec cutoff")
            })
        };
        let mut admit = |bytes: usize, visits: usize| {
            let charge = (|| -> Result<()> {
                active(deadline, cancelled)?;
                ledger
                    .operation
                    .charge_work(bytes.checked_add(visits).ok_or_else(exhausted)?)?;
                ledger.operation.charge_bytes(bytes)?;
                whole
                    .try_borrow_mut()
                    .map_err(|_| exhausted())?
                    .read(u64::try_from(bytes).map_err(|_| exhausted())?)
            })();
            charge.map_err(|error| {
                refusal = Some(error);
                FoundationError::new(FoundationErrorCode::BudgetExceeded, "scope codec admission")
            })
        };
        let result = canonical_bytes_v1_with_state_budget_and_visits_and_admission(
            value,
            CanonicalProfile::SourceCommandInputV1,
            limits,
            live,
            &mut cutoff,
            &mut admit,
        );
        let (raw, _) = result.map_err(|_| {
            refusal
                .or_else(|| active(deadline, cancelled).err())
                .unwrap_or_else(exhausted)
        })?;
        whole
            .try_borrow_mut()
            .map_err(|_| exhausted())?
            .retain(raw.capacity())?;
        Ok(raw)
    }
    // Borrowed output envelope only, never a replacement canonical grammar.
    // Every node/key/string/number is paid before its future codec traversal.
    fn operation_envelope(
        &mut self, value: &JsonValue, depth: usize, nodes: &mut usize, numeric_state: &mut usize,
    ) -> Result<usize> {
        self.inspect(0, 1)?;
        let limits = JsonLimits::default();
        if depth > limits.max_depth || *nodes >= limits.max_visits { return Err(exhausted()); }
        *nodes += 1;
        let add = |a: usize, b: usize| a.checked_add(b).ok_or_else(exhausted);
        match value {
            JsonValue::Null | JsonValue::Bool(_) => Ok(5),
            JsonValue::Number(number) => {
                self.inspect(number.lexeme.len(), 1)?;
                *numeric_state = (*numeric_state).max(
                    tos_foundation::canonical_incremental_numeric_state(limits, number.lexeme.len())
                        .map_err(|_| exhausted())?,
                );
                number.lexeme.len().checked_mul(8)
                    .and_then(|n| n.checked_add(limits.max_integer_digits)).ok_or_else(exhausted)
            }
            JsonValue::String(text) => {
                self.prepay_string(text, false)?;
                text.units().len().checked_mul(6).and_then(|n| n.checked_add(2)).ok_or_else(exhausted)
            }
            JsonValue::Array(values) => {
                let mut output = add(values.len(), 2)?;
                for child in values { output = add(output, self.operation_envelope(child, depth + 1, nodes, numeric_state)?)?; }
                Ok(output)
            }
            JsonValue::Object(fields) => {
                let mut output = fields.len().checked_mul(2).and_then(|n| n.checked_add(2)).ok_or_else(exhausted)?;
                for (key, child) in fields {
                    self.prepay_string(key, false)?;
                    let key = key.units().len().checked_mul(6).and_then(|n| n.checked_add(2)).ok_or_else(exhausted)?;
                    output = add(output, add(key, self.operation_envelope(child, depth + 1, nodes, numeric_state)?)?)?;
                }
                Ok(output)
            }
        }
    }
    fn canonical_operation(&mut self, value: &JsonValue) -> Result<Vec<u8>> {
        let defaults = JsonLimits::default();
        // Additive logical frame slots for this borrowed prepass. The actual
        // codec's private ABI-sized fixed scratch is quoted by Foundation.
        type WalkFrame<'a> = (&'a JsonValue, JsonLimits, usize, &'a mut usize, &'a mut usize,
            std::slice::Iter<'a, JsonValue>, std::slice::Iter<'a, (JsonString, JsonValue)>,
            Option<&'a JsonString>, Option<&'a JsonValue>, usize, Result<usize>);
        let prepass = defaults.max_depth.checked_add(1)
            .and_then(|n| n.checked_mul(size_of::<WalkFrame<'_>>())).ok_or_else(exhausted)?;
        self.retain(prepass)?;
        let mut nodes = 0;
        let mut numeric_state = 0;
        let output = self.operation_envelope(value, 0, &mut nodes, &mut numeric_state)?;
        let fixed = tos_foundation::canonical_incremental_fixed_state(defaults).map_err(|_| exhausted())?;
        let state = nodes.checked_mul(size_of::<(usize, &(JsonString, JsonValue))>())
            .and_then(|n| output.checked_mul(2).and_then(|output| n.checked_add(output)))
            .and_then(|n| n.checked_add(fixed))
            .and_then(|n| n.checked_add(numeric_state)).ok_or_else(exhausted)?;
        // ONE currency: reserve overlapping output/sort/numeric scratch BEFORE
        // count/sort/emission, INCLUDING the separate strict parser invoked
        // for each Number. Number validators are sequential; the maximum
        // parser+numeric heap overlaps the output and active sort buffers.
        // Keep it spent on every later Err; never refund.
        self.retain(state)?;
        let ledger = self.ledger.as_mut().ok_or_else(exhausted)?;
        if ledger.whole.is_some() { return Err(exhausted()); }
        active(ledger.deadline, ledger.cancelled)?;
        // Recompute AFTER all holder debits, not from an obsolete allowance.
        let remaining = ledger.operation.limits()?;
        let limits = JsonLimits {
            max_bytes: 8_388_608usize.min(remaining.max_input_bytes),
            max_visits: defaults.max_visits.min(remaining.max_work),
            ..defaults
        };
        let deadline = ledger.deadline;
        let cancelled = ledger.cancelled;
        let mut refusal = None;
        let mut cutoff = || active(deadline, cancelled).map_err(|_| {
            FoundationError::new(FoundationErrorCode::BudgetExceeded, "scope codec cutoff")
        });
        let mut admit = |bytes: usize, visits: usize| {
            let result = (|| -> Result<()> {
                active(deadline, cancelled)?;
                ledger.operation.charge_work(bytes.checked_add(visits).ok_or_else(exhausted)?)?;
                ledger.operation.charge_bytes(bytes)
            })();
            result.map_err(|error| {
                if refusal.is_none() { refusal = Some(error); }
                FoundationError::new(FoundationErrorCode::BudgetExceeded, "scope codec admission")
            })
        };
        canonical_bytes_v1_with_state_budget_and_visits_and_admission(
            value, CanonicalProfile::SourceCommandInputV1, limits, state, &mut cutoff, &mut admit,
        ).map(|(raw, _)| raw).map_err(|_| refusal
            .or_else(|| active(deadline, cancelled).err()).unwrap_or_else(exhausted))
    }
    pub(super) fn same(&mut self, left: &JsonValue, right: &JsonValue) -> Result<bool> {
        if self.ledger.is_none() {
            return cmd::same(left, right);
        }
        let left = self.canonical(left)?;
        let right = self.canonical(right)?;
        self.inspect(
            left.len().checked_add(right.len()).ok_or_else(exhausted)?,
            1,
        )?;
        Ok(left == right)
    }
    pub(super) fn reference(&mut self, row: &JsonValue) -> Result<JsonValue> {
        if self.ledger.is_none() {
            return owner_reference(row);
        }
        self.exact_keys(row, &["id", "version", "payload", "origin_id"])?;
        let id = self.field(row, "id")?;
        let version = self.field(row, "version")?;
        let payload = self.field(row, "payload")?;
        let raw = self.canonical(payload)?;
        self.inspect(raw.len(), 1)?;
        // Hash input, two fixed hex/prefixed String holders, then spelling.
        self.inspect(64 + 71, 2)?;
        self.retain(64 + 71)?;
        let digest = Digest256::of_bytes(&raw).to_prefixed();
        let id = self.copied(id)?;
        let version = self.copied(version)?;
        // Three fixed field slots and ASCII keys; digest spelling has fixed71
        // bytes. Pay before cmd::object/string allocate either native buffer.
        let fixed = size_of::<JsonValue>()
            + 3 * size_of::<(&str, JsonValue)>()
            + 3 * size_of::<(JsonString, JsonValue)>()
            + 3 * (2 + 7 + 6 + 71);
        self.inspect(fixed, 8)?;
        self.retain(fixed)?;
        Ok(cmd::object(vec![
            ("id", id),
            ("version", version),
            ("digest", cmd::string(&digest)),
        ]))
    }
    pub(super) fn version(&mut self, config: &JsonValue) -> Result<OwnerVersion> {
        if self.ledger.is_none() {
            return OwnerVersion::from_config(config);
        }
        let schema = self.text(config, "schema_version")?;
        self.inspect(schema.len().checked_mul(6).ok_or_else(exhausted)?, 6)?;
        OwnerVersion::from_schema(schema)
    }
    pub(super) fn describe(&mut self, request: &ParsedRequest) -> Result<bool> {
        self.text_equal(&request.operation, "describe")
    }
}
