use crate::{DomainError, DomainLimits};
use std::{mem::size_of, sync::atomic::{AtomicBool, Ordering}, time::Instant};
use tos_foundation::{CanonicalProfile, Digest256, ExactRecordRef, FoundationError,
    FoundationErrorCode, JsonDocument, JsonLimits,
    canonical_bytes_v1_with_state_budget_and_visits_and_admission};
use super::record::{NativeIdentityFields, NativeTsRecord, finish_binding};

/// One original operation allowance. Logical requested state excludes allocator
/// excess/RSS and the caller's preexisting held source/parser/schema frames.
#[derive(Clone, Copy, Debug)]
pub struct NativeTsLimits {
    pub domain: DomainLimits,
    pub max_source_files: usize,
    pub max_source_bytes: usize,
    pub canonical: JsonLimits,
    pub canonical_state_bytes: usize,
    pub deadline: Instant,
}
#[derive(Clone, Copy, Default)]
struct Counts { work: usize, bytes: usize, items: usize }
#[derive(Clone, Copy, Debug)]
pub struct NativeTsUsage {
    pub work: usize, pub requested_bytes: usize, pub items: usize,
    pub source_files: usize, pub source_bytes: usize,
}
pub struct NativeTsSession<'a> {
    limits: NativeTsLimits,
    cancelled: &'a AtomicBool,
    used: Counts,
    source_files: usize,
    source_bytes: usize,
    call: Option<(DomainLimits, Counts)>,
}
fn add(n: &mut usize, d: usize, cap: usize) -> Result<(), DomainError> {
    *n = n.checked_add(d).filter(|v| *v <= cap).ok_or(DomainError::ResourceLimit)?;
    Ok(())
}
impl<'a> NativeTsSession<'a> {
    pub fn new(limits: NativeTsLimits, cancelled: &'a AtomicBool) -> Result<Self, DomainError> {
        if limits.domain.max_work == 0 || limits.domain.max_requested_bytes == 0
            || limits.domain.max_items == 0 || limits.max_source_files == 0
            || limits.max_source_bytes == 0 || limits.canonical_state_bytes == 0 {
            return Err(DomainError::ResourceLimit);
        }
        let out = Self { limits, cancelled, used: Counts::default(), source_files: 0,
            source_bytes: 0, call: None };
        out.check()?; Ok(out)
    }
    pub fn check(&self) -> Result<(), DomainError> {
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.limits.deadline {
            return Err(DomainError::ResourceLimit);
        }
        Ok(())
    }
    pub fn usage(&self) -> NativeTsUsage {
        NativeTsUsage { work:self.used.work,requested_bytes:self.used.bytes,
            items:self.used.items,source_files:self.source_files,source_bytes:self.source_bytes }
    }
    pub fn work(&mut self, n: usize) -> Result<(), DomainError> {
        self.check()?;
        add(&mut self.used.work, n, self.limits.domain.max_work)?;
        if let Some((l,c)) = &mut self.call { add(&mut c.work, n, l.max_work)?; }
        Ok(())
    }
    pub fn requested_bytes(&mut self, n: usize) -> Result<(), DomainError> {
        self.check()?;
        add(&mut self.used.bytes, n, self.limits.domain.max_requested_bytes)?;
        if let Some((l,c)) = &mut self.call { add(&mut c.bytes, n, l.max_requested_bytes)?; }
        Ok(())
    }
    pub fn item(&mut self) -> Result<(), DomainError> {
        self.items(1)
    }
    fn items(&mut self, n: usize) -> Result<(), DomainError> {
        self.check()?;
        add(&mut self.used.items, n, self.limits.domain.max_items)?;
        if let Some((l,c)) = &mut self.call { add(&mut c.items, n, l.max_items)?; }
        Ok(())
    }
    /// The accepted pure checker owns a finite local budget, without exposing
    /// its spent counters. Reserve that entire allowance before calling it;
    /// no refund or reset can mint a second original allowance.
    pub(crate) fn reserve_checker(&mut self, limits: DomainLimits) -> Result<(), DomainError> {
        if limits.max_work == 0 || limits.max_requested_bytes == 0 || limits.max_items == 0 {
            return Err(DomainError::ResourceLimit);
        }
        self.work(limits.max_work)?;
        self.requested_bytes(limits.max_requested_bytes)?;
        self.items(limits.max_items)
    }
    /// Native owner calls this before source I/O; partial/chunk reads retain all
    /// charges (files=0 may charge subsequent chunks of an already opened file).
    pub fn source_read(&mut self, files: usize, bytes: usize) -> Result<(), DomainError> {
        self.check()?;
        add(&mut self.source_files, files, self.limits.max_source_files)?;
        add(&mut self.source_bytes, bytes, self.limits.max_source_bytes)?;
        self.work(bytes)
    }
    pub(crate) fn exact(&mut self, r: &ExactRecordRef) -> Result<(), DomainError> {
        self.work(r.id.as_str().len().checked_add(40).ok_or(DomainError::ResourceLimit)?)
    }
    pub(crate) fn reserve<T>(&mut self, n: usize) -> Result<(), DomainError> {
        self.requested_bytes(n.checked_mul(size_of::<T>()).ok_or(DomainError::ResourceLimit)?)
    }
    pub(crate) fn call<T>(&mut self, limits: DomainLimits,
        f: impl FnOnce(&mut Self) -> Result<T, DomainError>) -> Result<T, DomainError> {
        self.check()?;
        if self.call.is_some() { return Err(DomainError::Invalid("reentrant native TS resolution")); }
        if limits.max_work == 0 || limits.max_requested_bytes == 0 || limits.max_items == 0 {
            return Err(DomainError::ResourceLimit);
        }
        self.call = Some((limits, Counts::default()));
        let result = f(self);
        self.call = None; // Only the local cap resets; original charges never do.
        result
    }
    pub(crate) fn bind<'b>(&mut self, doc: &'b JsonDocument, exact: &'b ExactRecordRef)
        -> Result<NativeTsRecord<'b>, DomainError> {
        self.exact(exact)?;
        if doc.mode() != tos_foundation::JsonMode::PublishedStrict {
            return Err(DomainError::Invalid("native TS source parse profile"));
        }
        // Pre-reserve a finite common-codec workspace against the original
        // operation. Unused reservation is not refunded or reused as new budget.
        let available = self.limits.canonical_state_bytes;
        self.requested_bytes(available)?;
        let limits = self.limits.canonical;
        let deadline = self.limits.deadline;
        let cancelled = self.cancelled;
        let mut check = || {
            if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                Err(FoundationError::new(FoundationErrorCode::BudgetExceeded, "native TS cutoff"))
            } else { Ok(()) }
        };
        let mut admit = |bytes: usize, visits: usize| {
            let n = bytes.checked_add(visits).ok_or_else(|| FoundationError::new(
                FoundationErrorCode::BudgetExceeded, "native TS codec work overflow"))?;
            self.work(n).map_err(|_| FoundationError::new(
                FoundationErrorCode::BudgetExceeded, "native TS codec work allowance"))
        };
        let (bytes, _) = canonical_bytes_v1_with_state_budget_and_visits_and_admission(
            doc.root(), CanonicalProfile::SourceRecordDigestV1, limits, available,
            &mut check, &mut admit).map_err(super::record::codec_error)?;
        self.work(bytes.len())?; // The following digest traversal is additional.
        if Digest256::of_bytes(&bytes) != exact.digest {
            return Err(DomainError::Incompatible("native TS whole-record canonical digest"));
        }
        let entries = doc.root().as_object().ok_or(DomainError::Invalid("native TS Record object"))?;
        for (key,value) in entries {
            self.work(key.units().len().checked_add(1).ok_or(DomainError::ResourceLimit)?)?;
            if key.as_str() == Some("record_id") {
                if let Some(id) = value.as_str() { self.work(id.len())?; }
            }
            if key.as_str() == Some("record_version") {
                if let tos_foundation::JsonValue::Number(n) = value { self.work(n.lexeme.len())?; }
            }
        }
        finish_binding(doc, exact, NativeIdentityFields::Record)
    }
}
pub(crate) fn push<T>(s: &mut NativeTsSession<'_>, v: &mut Vec<T>, x: T) -> Result<(), DomainError> {
    s.item()?; s.reserve::<T>(1)?;
    if v.len() == v.capacity() {
        s.work(v.len().checked_add(1).ok_or(DomainError::ResourceLimit)?)?;
        v.try_reserve_exact(1).map_err(|_| DomainError::ResourceLimit)?;
    }
    v.push(x); Ok(())
}
