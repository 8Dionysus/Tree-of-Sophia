use serde_json::Value;
use std::{
    mem::size_of,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode, JsonLimits, JsonMode,
    JsonString, JsonValue, canonical_bytes_v1_with_state_budget_and_visits_and_admission,
    parse_json_with_state_budget_and_admission,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed(&'static str),
    NeedsBinding(&'static str),
    Unsupported(&'static str),
    ExactMismatch,
    QualificationMismatch(&'static str),
    ResourceLimit,
    Cancelled,
    Deadline,
    Denied,
    Upstream(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;

/// Incremental PWE charges. Upstream parser/source/schema worker holders remain
/// caller-owned. Zero limits refuse; there is no process-global default budget.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_nodes: usize,
    pub max_work: usize,
    pub max_refs: usize,
    pub max_depth: usize,
    pub max_items: usize,
    pub max_state_bytes: usize,
    pub deadline: Instant,
}
/// Caller-owned source inspection/holder accounting. This is a resource port,
/// not evidence, selection, source authority or a successful closure verifier.
pub trait SourceAdmission {
    fn inspect(&self, bytes: usize) -> Result<()>;
    fn retain(&self, bytes: usize) -> Result<()>;
}
#[derive(Clone, Copy, Debug)]
pub struct SourceAllowance {
    pub input_bytes: usize,
    pub nodes: usize,
    pub work: usize,
    pub state_bytes: usize,
    pub depth: usize,
}
pub struct Session<'a> {
    pub(super) limits: Limits,
    cancelled: &'a AtomicBool,
    bytes: usize,
    nodes: usize,
    work: usize,
    refs: usize,
    state: usize,
    source_admission: Option<&'a dyn SourceAdmission>,
}
fn add(n: &mut usize, d: usize, cap: usize) -> Result<()> {
    let next = n.checked_add(d).ok_or(Error::ResourceLimit)?;
    if next > cap {
        return Err(Error::ResourceLimit);
    }
    *n = next;
    Ok(())
}
impl<'a> Session<'a> {
    pub fn new(limits: Limits, cancelled: &'a AtomicBool) -> Result<Self> {
        if [
            limits.max_input_bytes,
            limits.max_nodes,
            limits.max_work,
            limits.max_refs,
            limits.max_items,
            limits.max_state_bytes,
            limits.max_depth,
        ]
        .contains(&0)
            || limits.max_depth > 96
        {
            return Err(Error::ResourceLimit);
        }
        let s = Self {
            limits,
            cancelled,
            bytes: 0,
            nodes: 0,
            work: 0,
            refs: 0,
            state: 0,
            source_admission: None,
        };
        s.check()?;
        Ok(s)
    }
    /// Construct ONCE at the real source-operation boundary. All later reader,
    /// revision and dependency calls borrow this same cumulative session.
    pub fn new_source(
        limits: Limits,
        cancelled: &'a AtomicBool,
        payer: &'a dyn SourceAdmission,
    ) -> Result<Self> {
        let mut session = Self::new(limits, cancelled)?;
        session.source_admission = Some(payer);
        Ok(session)
    }
    pub fn has_source_payer(&self, payer: &dyn SourceAdmission) -> bool {
        self.source_admission
            .is_some_and(|actual| std::ptr::addr_eq(actual, payer))
    }
    pub(super) fn source_mode(&self) -> bool {
        self.source_admission.is_some()
    }
    /// Only the current remainder is exposed; no setter, reset or new payer.
    pub fn source_allowance(&self) -> Result<SourceAllowance> {
        self.check()?;
        if self.source_admission.is_none() {
            return Err(Error::NeedsBinding("original source payer"));
        }
        Ok(SourceAllowance {
            input_bytes: self
                .limits
                .max_input_bytes
                .checked_sub(self.bytes)
                .ok_or(Error::ResourceLimit)?,
            nodes: self
                .limits
                .max_nodes
                .checked_sub(self.nodes)
                .ok_or(Error::ResourceLimit)?,
            work: self
                .limits
                .max_work
                .checked_sub(self.work)
                .ok_or(Error::ResourceLimit)?,
            state_bytes: self
                .limits
                .max_state_bytes
                .checked_sub(self.state)
                .ok_or(Error::ResourceLimit)?,
            depth: self.limits.max_depth,
        })
    }
    pub fn admit_source_prefix(&mut self, bytes: usize, visits: usize) -> Result<()> {
        self.source_allowance()?;
        self.tick(bytes.checked_add(visits).ok_or(Error::ResourceLimit)?)?;
        self.input(bytes)?;
        add(&mut self.nodes, visits, self.limits.max_nodes)
    }
    /// Work only on this cumulative SOURCE Session. Original work funding is
    /// caller-owned; this never converts work into input/state/nodes/refs or
    /// invokes a byte-admission callback. The caller preserves refusal through
    /// its original payer mapping, as for the other Session operations.
    pub fn admit_source_work(&mut self, work: usize) -> Result<()> {
        self.source_allowance()?;
        self.tick(work)
    }
    pub fn retain_source_state(&mut self, bytes: usize) -> Result<()> {
        self.source_allowance()?;
        self.state(bytes)
    }
    pub fn charges(&self) -> (usize, usize, usize, usize, usize) {
        (self.bytes, self.nodes, self.work, self.refs, self.state)
    }
    pub(super) fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        if Instant::now() >= self.limits.deadline {
            return Err(Error::Deadline);
        }
        Ok(())
    }
    pub(super) fn tick(&mut self, n: usize) -> Result<()> {
        self.check()?;
        add(&mut self.work, n, self.limits.max_work)
    }
    pub(super) fn state(&mut self, n: usize) -> Result<()> {
        self.check()?;
        add(&mut self.state, n, self.limits.max_state_bytes)?;
        if let Some(payer) = self.source_admission {
            payer.retain(n)?;
        }
        Ok(())
    }
    fn source_read(&self, n: usize) -> Result<()> {
        self.check()?;
        if let Some(payer) = self.source_admission {
            payer.inspect(n)?;
        }
        Ok(())
    }
    fn input(&mut self, n: usize) -> Result<()> {
        self.source_read(n)?;
        add(&mut self.bytes, n, self.limits.max_input_bytes)
    }
    pub(super) fn prepay_k0(&mut self, l: tos_foundation::OperationBindingLimits) -> Result<()> {
        self.tick(l.max_nodes)?;
        self.input(l.max_envelope_bytes)?;
        add(&mut self.nodes, l.max_nodes, self.limits.max_nodes)?;
        add(&mut self.refs, l.max_refs, self.limits.max_refs)?;
        if l.max_depth > self.limits.max_depth {
            return Err(Error::ResourceLimit);
        }
        Ok(())
    }
    pub(super) fn vec<T>(&mut self, count: usize) -> Result<Vec<T>> {
        if count > self.limits.max_items {
            return Err(Error::ResourceLimit);
        }
        self.state(
            size_of::<Vec<T>>()
                .checked_add(
                    count
                        .checked_mul(size_of::<T>())
                        .ok_or(Error::ResourceLimit)?,
                )
                .ok_or(Error::ResourceLimit)?,
        )?;
        let mut out = Vec::new();
        out.try_reserve_exact(count)
            .map_err(|_| Error::ResourceLimit)?;
        Ok(out)
    }
    pub(super) fn push<T>(&mut self, v: &mut Vec<T>, x: T) -> Result<()> {
        if v.len() >= self.limits.max_items {
            return Err(Error::ResourceLimit);
        }
        self.state(size_of::<T>())?;
        v.try_reserve_exact(1).map_err(|_| Error::ResourceLimit)?;
        v.push(x);
        Ok(())
    }
    pub(super) fn equal(&mut self, a: &Value, b: &Value) -> Result<bool> {
        self.tick(1)?;
        match (a, b) {
            (Value::String(a), Value::String(b)) => self.same_text(a, b),
            (Value::Array(a), Value::Array(b)) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (a, b) in a.iter().zip(b) {
                    if !self.equal(a, b)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Value::Object(a), Value::Object(b)) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (k, v) in a {
                    self.tick(k.len().checked_mul(b.len()).ok_or(Error::ResourceLimit)?)?;
                    self.source_read(
                        k.len()
                            .checked_mul(b.len())
                            .and_then(|n| n.checked_mul(2))
                            .ok_or(Error::ResourceLimit)?,
                    )?;
                    let Some(w) = b.get(k) else { return Ok(false) };
                    if !self.equal(v, w)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Value::Number(left), Value::Number(right)) => {
                if self.source_admission.is_some() {
                    let n = left
                        .as_str()
                        .len()
                        .checked_add(right.as_str().len())
                        .ok_or(Error::ResourceLimit)?;
                    self.admit_source_prefix(n, 1)?;
                    // Pinned arbitrary_precision Number equality compares its
                    // stored strings. Keep serde's existing equality semantics.
                    return Ok(a == b);
                }
                for x in [a, b] {
                    if let Value::Number(number) = x {
                        self.source_read(number.as_str().len())?;
                    }
                    self.tick(
                        self.limits
                            .max_input_bytes
                            .checked_mul(8)
                            .ok_or(Error::ResourceLimit)?,
                    )?;
                    let n = crate::source_bibliographic_render::encoded_size(
                        x,
                        self.limits.max_input_bytes,
                    )
                    .map_err(|_| Error::ResourceLimit)?;
                    self.tick(n)?;
                }
                Ok(a == b)
            }
            _ => Ok(a == b),
        }
    }
    /// serde Value cannot contain duplicate keys. Supplied source owners must use
    /// their PublishedStrict parser before this borrowed-value API, as for the
    /// maintained metadata facade. We do not retroactively certify raw bytes.
    pub(super) fn admit(&mut self, v: &Value) -> Result<()> {
        self.walk(v, 0)
    }
    fn walk(&mut self, v: &Value, depth: usize) -> Result<()> {
        if depth > self.limits.max_depth {
            return Err(Error::ResourceLimit);
        }
        self.tick(1)?;
        add(&mut self.nodes, 1, self.limits.max_nodes)?;
        match v {
            Value::Null => self.input(4)?,
            Value::Bool(_) => self.input(5)?,
            Value::Number(number) => {
                if self.source_admission.is_some() {
                    // serde_json 1.0.151 arbitrary_precision: as_str borrows
                    // the exact serialized number spelling, including its
                    // already-normalized exponent. No counting codec needed.
                    let n = number.as_str().len();
                    self.admit_source_prefix(n, 1)?;
                    return Ok(());
                }
                self.source_read(number.as_str().len())?;
                self.tick(
                    self.limits
                        .max_input_bytes
                        .checked_mul(8)
                        .ok_or(Error::ResourceLimit)?,
                )?;
                let n = crate::source_bibliographic_render::encoded_size(
                    v,
                    self.limits.max_input_bytes,
                )
                .map_err(|_| Error::ResourceLimit)?;
                self.tick(n)?;
                self.input(n)?;
            }
            Value::String(s) => {
                self.tick(s.len())?;
                let n = s
                    .len()
                    .checked_mul(6)
                    .and_then(|n| n.checked_add(2))
                    .ok_or(Error::ResourceLimit)?;
                self.input(n)?;
            }
            Value::Array(xs) => {
                self.input(xs.len().checked_add(2).ok_or(Error::ResourceLimit)?)?;
                for x in xs {
                    self.walk(x, depth + 1)?;
                }
            }
            Value::Object(xs) => {
                self.input(2)?;
                for (k, x) in xs {
                    self.tick(k.len())?;
                    let n = k
                        .len()
                        .checked_mul(6)
                        .and_then(|n| n.checked_add(4))
                        .ok_or(Error::ResourceLimit)?;
                    self.input(n)?;
                    self.walk(x, depth + 1)?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn field<'v>(&mut self, v: &'v Value, k: &str) -> Result<&'v Value> {
        let obj = v.as_object().ok_or(Error::Malformed("object"))?;
        // Charge an upper bound for the map lookup, independent of tree shape.
        for key in obj.keys() {
            self.tick(
                key.len()
                    .checked_add(k.len() + 1)
                    .ok_or(Error::ResourceLimit)?,
            )?;
            self.source_read(
                key.len()
                    .checked_add(k.len() + 1)
                    .ok_or(Error::ResourceLimit)?,
            )?;
        }
        obj.get(k).ok_or(Error::Malformed("missing field"))
    }
    pub(super) fn text<'v>(&mut self, v: &'v Value) -> Result<&'v str> {
        let s = v.as_str().ok_or(Error::Malformed("UTF-8 string"))?;
        self.tick(s.len())?;
        self.source_read(s.len())?;
        if s.is_empty() {
            return Err(Error::Malformed("empty string"));
        }
        Ok(s)
    }
    pub(super) fn str_field<'v>(&mut self, v: &'v Value, k: &str) -> Result<&'v str> {
        let x = self.field(v, k)?;
        self.text(x)
    }
    pub(super) fn array<'v>(&mut self, v: &'v Value) -> Result<&'v [Value]> {
        let x = v.as_array().ok_or(Error::Malformed("array"))?;
        self.tick(x.len())?;
        if x.len() > self.limits.max_items {
            return Err(Error::ResourceLimit);
        }
        Ok(x)
    }
    pub(super) fn same_text(&mut self, a: &str, b: &str) -> Result<bool> {
        self.tick(a.len().checked_add(b.len()).ok_or(Error::ResourceLimit)?)?;
        self.source_read(a.len().checked_add(b.len()).ok_or(Error::ResourceLimit)?)?;
        Ok(a == b)
    }
    pub(super) fn exact<'v>(&mut self, v: &'v Value) -> Result<PweRef<'v>> {
        add(&mut self.refs, 1, self.limits.max_refs)?;
        if v.as_object().map(|x| x.len()) != Some(3) {
            return Err(Error::Malformed("exact ref shape"));
        }
        let id = self.str_field(v, "id")?;
        if id.chars().any(char::is_control) {
            return Err(Error::Malformed("stable id"));
        }
        let ver = self
            .u64_field(v, "version")?
            .filter(|n| *n > 0)
            .ok_or(Error::Malformed("positive u64 version"))?;
        let d = self.str_field(v, "digest")?;
        let digest =
            Digest256::from_prefixed(d).map_err(|_| Error::Malformed("canonical digest"))?;
        Ok(PweRef {
            id,
            version: ver,
            digest,
        })
    }
    fn u64_field(&mut self, value: &Value, key: &str) -> Result<Option<u64>> {
        let value = self.field(value, key)?;
        if self.source_admission.is_some() {
            if let Value::Number(number) = value {
                self.admit_source_prefix(number.as_str().len(), 1)?;
            }
        }
        Ok(value.as_u64())
    }
    pub(super) fn typed<'v>(&mut self, v: &'v Value) -> Result<(PweRef<'v>, PweRef<'v>)> {
        if v.as_object().map(|x| x.len()) != Some(2) {
            return Err(Error::Malformed("TypedRef"));
        }
        let k = self.field(v, "kind_binding")?;
        let r = self.field(v, "record")?;
        Ok((self.exact(k)?, self.exact(r)?))
    }
    pub(super) fn bound<'v>(&mut self, v: &'v Value, slot: &'static str) -> Result<&'v Value> {
        match self.str_field(v, "state")? {
            "bound" => {
                let x = self.field(v, "value")?;
                self.typed(x)?;
                Ok(x)
            }
            "unresolved" => Err(Error::NeedsBinding(slot)),
            _ => Err(Error::Malformed("binding state")),
        }
    }
    pub(super) fn digest(&mut self, v: &Value) -> Result<Digest256> {
        if self.source_admission.is_some() {
            let raw = self.source_canonical(v)?;
            self.admit_source_prefix(raw.len(), 1)?;
            return Ok(Digest256::of_bytes(&raw));
        }
        // Existing bounded canonical owner codec; its temporary work/holders are
        // prepaid by this explicit conservative logical helper envelope. This is
        // not an allocator/RSS claim, and it does not include the retained input.
        let cap = self.limits.max_input_bytes;
        self.tick(cap.checked_mul(8).ok_or(Error::ResourceLimit)?)?;
        self.source_read(cap)?;
        let n = crate::source_bibliographic_render::encoded_size(v, cap)
            .map_err(|_| Error::ResourceLimit)?;
        let slots = size_of::<tos_foundation::JsonValue>()
            .checked_add(size_of::<tos_foundation::JsonString>())
            .and_then(|x| x.checked_add(size_of::<Value>()))
            .and_then(|x| x.checked_add(size_of::<(String, Value)>()))
            .and_then(|x| x.checked_mul(8))
            .and_then(|x| x.checked_add(64))
            .ok_or(Error::ResourceLimit)?;
        self.state(n.checked_mul(slots).ok_or(Error::ResourceLimit)?)?;
        self.tick(
            n.checked_mul(n)
                .and_then(|x| x.checked_mul(8))
                .and_then(|x| x.checked_add(4_000_000))
                .ok_or(Error::ResourceLimit)?,
        )?;
        self.source_read(n.checked_mul(2).ok_or(Error::ResourceLimit)?)?;
        let raw = crate::source_bibliographic_render::encode(v, cap)
            .map_err(|_| Error::Upstream("canonical owner codec"))?;
        self.check()?;
        Ok(Digest256::of_bytes(&raw))
    }
    // SOURCE-only bounded compact wire measurement. All operations here use
    // length headers or charged visits; no string contents are inspected by
    // this prepass. Eight work units per UTF8 input byte +32 per string/node
    // reserve the pinned compact serializer's scan/escaping/copy work BEFORE
    // serialization. A number is already a stored JSON lexeme (pinned serde).
    fn source_text_bound(&mut self, text: &str) -> Result<usize> {
        let n = text.len();
        self.tick(
            n.checked_mul(7)
                .and_then(|n| n.checked_add(32))
                .ok_or(Error::ResourceLimit)?,
        )?;
        self.admit_source_prefix(n, 1)?;
        n.checked_mul(6)
            .and_then(|n| n.checked_add(2))
            .ok_or(Error::ResourceLimit)
    }
    fn source_wire_bound(
        &mut self,
        value: &Value,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<usize> {
        self.admit_source_prefix(0, 1)?;
        if depth > self.limits.max_depth || *nodes >= 1_000_000 {
            return Err(Error::ResourceLimit);
        }
        *nodes += 1;
        self.tick(32)?;
        match value {
            Value::Null | Value::Bool(_) => Ok(5),
            Value::Number(number) => {
                let n = number.as_str().len();
                self.tick(64)?; // pinned private Number token/serializer dispatch
                self.admit_source_prefix(n, 1)?;
                // The compact wire uses n. The canonical Python float spelling
                // may expand it; 32 also bounds the pinned finite-f64 spelling.
                n.checked_add(32).ok_or(Error::ResourceLimit)
            }
            Value::String(text) => self.source_text_bound(text),
            Value::Array(values) => {
                let mut bound = values.len().checked_add(2).ok_or(Error::ResourceLimit)?;
                for value in values {
                    bound = bound
                        .checked_add(self.source_wire_bound(value, depth + 1, nodes)?)
                        .ok_or(Error::ResourceLimit)?;
                }
                Ok(bound)
            }
            Value::Object(values) => {
                let mut bound = 2usize;
                for (key, value) in values {
                    bound = bound
                        .checked_add(self.source_text_bound(key)?)
                        .and_then(|n| n.checked_add(2))
                        .ok_or(Error::ResourceLimit)?;
                    bound = bound
                        .checked_add(self.source_wire_bound(value, depth + 1, nodes)?)
                        .ok_or(Error::ResourceLimit)?;
                }
                Ok(bound)
            }
        }
    }
    /// Borrow this SAME Session for each maintained codec prefix. Cutoff and
    /// admission errors retain their original identity; no callback ledger is
    /// constructed and no successfully spent prefix is refunded.
    fn source_codec<T>(
        &mut self,
        run: impl FnOnce(
            &mut dyn FnMut() -> tos_foundation::Result<()>,
            &mut dyn FnMut(usize, usize) -> tos_foundation::Result<()>,
        ) -> tos_foundation::Result<T>,
    ) -> Result<T> {
        let cancelled = self.cancelled;
        let deadline = self.limits.deadline;
        let first = std::cell::RefCell::new(None);
        let result = {
            let mut check = || {
                let error = if cancelled.load(Ordering::Relaxed) {
                    Some(Error::Cancelled)
                } else if Instant::now() >= deadline {
                    Some(Error::Deadline)
                } else {
                    None
                };
                if let Some(error) = error {
                    if first.borrow().is_none() {
                        *first.borrow_mut() = Some(error);
                    }
                    Err(FoundationError::new(
                        FoundationErrorCode::BudgetExceeded,
                        "PWE original SOURCE cutoff",
                    ))
                } else {
                    Ok(())
                }
            };
            let mut admit = |bytes, visits| {
                self.admit_source_prefix(bytes, visits).map_err(|error| {
                    if first.borrow().is_none() {
                        *first.borrow_mut() = Some(error);
                    }
                    FoundationError::new(
                        FoundationErrorCode::BudgetExceeded,
                        "PWE original SOURCE admission",
                    )
                })
            };
            run(&mut check, &mut admit)
        };
        if let Some(error) = first.into_inner() {
            return Err(error);
        }
        self.check()?;
        result.map_err(|error| {
            if error.code == FoundationErrorCode::BudgetExceeded {
                Error::ResourceLimit
            } else {
                Error::Upstream("pinned SOURCE canonical codec")
            }
        })
    }
    fn source_codec_limits(&self) -> Result<JsonLimits> {
        let remaining = self.source_allowance()?;
        JsonLimits::new(
            remaining.input_bytes,
            remaining.depth.min(96),
            remaining.nodes.min(remaining.work).min(1_000_000),
            4096,
        )
        .map_err(|_| Error::ResourceLimit)
    }
    /// Same compact serde wire -> PublishedStrict parse -> SourceRecordDigestV1
    /// composition as the maintained owner renderer. Only SOURCE accounting is
    /// refined; no alternate numeric/identity grammar or canonical spelling.
    pub(super) fn source_canonical(&mut self, value: &Value) -> Result<Vec<u8>> {
        self.source_allowance()?;
        let depth = self.limits.max_depth.min(96);
        // Conservative public-ABI frame envelope also covers the prepass
        // recursion before it starts. The maintained codecs enforce their
        // own exact state requirements inside the prepaid reservations.
        let fixed = size_of::<JsonValue>()
            .checked_add(size_of::<JsonString>())
            .and_then(|n| {
                n.checked_add(size_of::<Value>() + size_of::<String>() + 4 * size_of::<usize>())
            })
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| n.checked_mul(depth + 1))
            .and_then(|n| n.checked_add(1024))
            .ok_or(Error::ResourceLimit)?;
        self.state(fixed)?;
        let mut nodes = 0;
        let bound = self.source_wire_bound(value, 0, &mut nodes)?;
        self.state(
            bound
                .checked_add(size_of::<Vec<u8>>())
                .ok_or(Error::ResourceLimit)?,
        )?;
        // Reserve all compact output/scan work before either allocation or
        // emission; the sink cannot grow or write beyond that exact backing.
        self.tick(bound)?;
        self.admit_source_prefix(bound, 1)?;
        let mut wire = Vec::new();
        wire.try_reserve_exact(bound)
            .map_err(|_| Error::ResourceLimit)?;
        if wire.capacity() > bound {
            return Err(Error::ResourceLimit);
        }
        let cutoff = std::cell::RefCell::new(None);
        struct Sink<'w> {
            wire: &'w mut Vec<u8>,
            bound: usize,
            cancelled: &'w AtomicBool,
            deadline: Instant,
            cutoff: &'w std::cell::RefCell<Option<Error>>,
        }
        impl std::io::Write for Sink<'_> {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self
                    .wire
                    .len()
                    .checked_add(bytes.len())
                    .is_none_or(|n| n > self.bound || n > self.wire.capacity())
                {
                    *self.cutoff.borrow_mut() = Some(Error::ResourceLimit);
                    return Err(std::io::Error::other("PWE prepaid compact wire"));
                }
                for chunk in bytes.chunks(65_536) {
                    let error = if self.cancelled.load(Ordering::Relaxed) {
                        Some(Error::Cancelled)
                    } else if Instant::now() >= self.deadline {
                        Some(Error::Deadline)
                    } else {
                        None
                    };
                    if let Some(error) = error {
                        *self.cutoff.borrow_mut() = Some(error);
                        return Err(std::io::Error::other("PWE original compact cutoff"));
                    }
                    self.wire.extend_from_slice(chunk);
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let serialized = serde_json::to_writer(
            &mut Sink {
                wire: &mut wire,
                bound,
                cancelled: self.cancelled,
                deadline: self.limits.deadline,
                cutoff: &cutoff,
            },
            value,
        );
        if let Some(error) = cutoff.into_inner() {
            return Err(error);
        }
        serialized.map_err(|_| Error::Upstream("pinned compact serde codec"))?;
        self.check()?;
        let stack = (depth + 1)
            .checked_mul(
                size_of::<JsonValue>()
                    + size_of::<JsonString>()
                    + size_of::<std::collections::HashMap<Vec<u16>, usize>>()
                    + 2 * size_of::<Vec<JsonValue>>(),
            )
            .ok_or(Error::ResourceLimit)?;
        let parse_state = wire
            .len()
            .checked_mul(64)
            .and_then(|n| n.checked_add(1024))
            .and_then(|n| n.checked_add(stack))
            .ok_or(Error::ResourceLimit)?;
        self.state(parse_state)?;
        let limits = self.source_codec_limits()?;
        let document = self.source_codec(|check, admit| {
            parse_json_with_state_budget_and_admission(
                &wire,
                JsonMode::PublishedStrict,
                limits,
                parse_state,
                check,
                admit,
            )
        })?;
        let canonical_state = bound
            .checked_add(fixed)
            .and_then(|n| {
                n.checked_add(nodes.checked_mul(size_of::<(usize, &(JsonString, JsonValue))>())?)
            })
            .ok_or(Error::ResourceLimit)?;
        self.state(canonical_state)?;
        let limits = self.source_codec_limits()?;
        let (raw, _) = self.source_codec(|check, admit| {
            canonical_bytes_v1_with_state_budget_and_visits_and_admission(
                document.root(),
                CanonicalProfile::SourceRecordDigestV1,
                limits,
                canonical_state,
                check,
                admit,
            )
        })?;
        Ok(raw)
    }
    pub(super) fn record_matches(
        &mut self,
        record: &Value,
        r: PweRef<'_>,
        id: &str,
        version: &str,
    ) -> Result<()> {
        let actual = self.str_field(record, id)?;
        if !self.same_text(actual, r.id)?
            || self.u64_field(record, version)? != Some(r.version)
            || self.digest(record)? != r.digest
        {
            return Err(Error::ExactMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PweRef<'a> {
    pub(super) id: &'a str,
    pub(super) version: u64,
    pub(super) digest: Digest256,
}
impl<'a> PweRef<'a> {
    pub fn id(self) -> &'a str {
        self.id
    }
    pub fn version(self) -> u64 {
        self.version
    }
    pub fn digest(self) -> Digest256 {
        self.digest
    }
}

#[cfg(test)]
mod source_work_tests {
    use super::*;
    use std::{cell::Cell, time::Duration};

    // Resource observations only; no native owner, record or authority.
    #[derive(Default)]
    struct ByteProbe {
        input: Cell<usize>,
        state: Cell<usize>,
    }
    impl SourceAdmission for ByteProbe {
        fn inspect(&self, n: usize) -> Result<()> {
            self.input.set(self.input.get() + n);
            Ok(())
        }
        fn retain(&self, n: usize) -> Result<()> {
            self.state.set(self.state.get() + n);
            Ok(())
        }
    }
    fn limits() -> Limits {
        Limits {
            max_input_bytes: 8, max_nodes: 1, max_work: 16, max_refs: 1,
            max_depth: 8, max_items: 1, max_state_bytes: 4,
            deadline: Instant::now() + Duration::from_secs(30),
        }
    }

    #[test]
    fn work_only_keeps_currencies_and_cumulative_prefix_separate() {
        let cancelled = AtomicBool::new(false);
        let payer = ByteProbe::default();
        let mut s = Session::new_source(limits(), &cancelled, &payer).unwrap();
        s.admit_source_prefix(8, 1).unwrap(); // input and semantic nodes exhausted
        s.retain_source_state(4).unwrap();
        s.refs = 1; // private test setup: exhaust independent reference currency
        let before = s.charges();
        s.admit_source_work(5).unwrap();
        s.admit_source_work(2).unwrap();
        s.admit_source_work(0).unwrap();
        assert_eq!(s.charges(), (before.0, before.1, before.2 + 7, before.3, before.4));
        assert_eq!((payer.input.get(), payer.state.get()), (8, 4));
        let spent = s.charges();
        assert_eq!(s.admit_source_work(1), Err(Error::ResourceLimit));
        assert_eq!(s.admit_source_work(usize::MAX), Err(Error::ResourceLimit));
        assert_eq!(s.charges(), spent); // neither refund nor partial failed add
        assert_eq!((payer.input.get(), payer.state.get()), (8, 4));
    }

    #[test]
    fn work_only_requires_source_mode_and_both_cutoffs_even_for_zero() {
        let cancelled = AtomicBool::new(false);
        let mut plain = Session::new(limits(), &cancelled).unwrap();
        let before = plain.charges();
        assert_eq!(plain.admit_source_work(0), Err(Error::NeedsBinding("original source payer")));
        assert_eq!(plain.charges(), before);
        let payer = ByteProbe::default();
        let mut s = Session::new_source(limits(), &cancelled, &payer).unwrap();
        cancelled.store(true, Ordering::Relaxed);
        assert_eq!(s.admit_source_work(0), Err(Error::Cancelled));
        assert_eq!(s.charges(), (0, 0, 0, 0, 0));
        // Independent expired-deadline case, without a sleep or a public reset.
        let other_cancelled = AtomicBool::new(false);
        let mut expired = Session::new_source(limits(), &other_cancelled, &payer).unwrap();
        expired.limits.deadline = Instant::now();
        assert_eq!(expired.admit_source_work(0), Err(Error::Deadline));
        assert_eq!(expired.charges(), (0, 0, 0, 0, 0));
        assert_eq!((payer.input.get(), payer.state.get()), (0, 0));
    }

    #[test]
    fn work_only_cannot_fit_strictly_more_than_four_times_one_mib() {
        // Numeric capacity bound only; no frame allocation or production profile.
        let cancelled = AtomicBool::new(false);
        let payer = ByteProbe::default();
        let n = 1_048_576usize;
        let mut selected = limits();
        selected.max_work = 4_194_304;
        let mut s = Session::new_source(selected, &cancelled, &payer).unwrap();
        assert_eq!(s.admit_source_work(4 * n + 1), Err(Error::ResourceLimit));
        assert_eq!(s.charges(), (0, 0, 0, 0, 0));
        s.admit_source_work(4 * n).unwrap();
        assert_eq!(s.admit_source_work(1), Err(Error::ResourceLimit));
        assert_eq!(s.charges(), (0, 0, 4_194_304, 0, 0));
        assert_eq!((payer.input.get(), payer.state.get()), (0, 0));
    }
}
