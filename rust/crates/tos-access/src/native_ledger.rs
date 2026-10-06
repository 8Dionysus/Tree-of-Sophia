//! Original counters retained by one serial lazy selected-owner session.
use super::*;
use std::cell::Cell;

pub(super) struct Ledger {
    pub retained: Cell<usize>,
    pub state_limit: usize,
    pub(super) reserved: Cell<usize>,
    pub work: Arc<std::sync::atomic::AtomicU64>,
    pub work_limit: u64,
    pub sql_vm: Arc<std::sync::atomic::AtomicU64>,
    pub sql_vm_limit: u64,
    pub store_steps: Arc<std::sync::atomic::AtomicU64>,
    pub store_sql_vm: Arc<std::sync::atomic::AtomicU64>,
    pub visits: Arc<std::sync::atomic::AtomicUsize>,
    pub visit_limit: usize,
    pub rows: Cell<u64>,
    pub input_bytes: Cell<u64>,
}
impl Ledger {
    pub fn remaining(&self, additional: usize) -> Result<usize> {
        self.state_limit
            .checked_sub(self.retained.get())
            .and_then(|n| n.checked_sub(self.reserved.get()))
            .and_then(|n| n.checked_sub(additional))
            .ok_or_else(|| {
                // Numeric owner counters only; source data and request text
                // never enter the private bounded child diagnostic stream.
                eprintln!(
                    "Core state refusal: limit={} retained={} reserved={} additional={}",
                    self.state_limit,
                    self.retained.get(),
                    self.reserved.get(),
                    additional,
                );
                "Core lazy original simultaneous state"
            })
    }
    pub fn reserve(&self, bytes: usize) -> Result<Reservation<'_>> {
        if bytes > self.remaining(0)? {
            return Err("Core lazy original remaining state");
        }
        let n = self
            .reserved
            .get()
            .checked_add(bytes)
            .ok_or("Core lazy reservation overflow")?;
        self.reserved.set(n);
        Ok(Reservation(self, bytes))
    }
    pub fn charge_work(&self, bytes: u64) -> Result<()> {
        self.work
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes).filter(|n| *n <= self.work_limit)
            })
            .map(|_| ())
            .map_err(|_| "Core lazy original aggregate byte work")
    }
    pub fn remaining_work(&self) -> Result<u64> {
        self.work_limit
            .checked_sub(self.work.load(Ordering::Relaxed))
            .ok_or("Core lazy original remaining byte work")
    }
    pub fn remaining_visits(&self) -> Result<usize> {
        self.visit_limit
            .checked_sub(self.visits.load(Ordering::Relaxed))
            .ok_or("Core lazy remaining original JSON visits")
    }
    pub fn charge_visits(&self, n: usize) -> Result<()> {
        self.visits
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(n).filter(|n| *n <= self.visit_limit)
            })
            .map(|_| ())
            .map_err(|_| "Core lazy original JSON visits exhausted")
    }
    /// Serial owned parser may settle only its unused preadmitted ceiling.
    pub fn settle_unused_visits(&self, n: usize) -> Result<()> {
        self.visits
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_sub(n)
            })
            .map(|_| ())
            .map_err(|_| "Core lazy original JSON settlement")
    }
    /// Store debits shared JSON/work/VM itself; caller settles only attempted
    /// rows/input, even on terminal failure. No double JSON debit.
    pub fn debit_store_usage(
        &self,
        usage: &tos_query::source_diagnostic::StoreUsage,
    ) -> Result<()> {
        let rows = self.rows.get().checked_sub(usage.rows);
        let input = self.input_bytes.get().checked_sub(usage.input_bytes);
        self.rows.set(rows.unwrap_or(0));
        self.input_bytes.set(input.unwrap_or(0));
        if rows.is_none() || input.is_none() {
            return Err("Core lazy original Store usage exhausted");
        }
        Ok(())
    }
    /// Usage is observed by its owner before action, including terminal failure.
    /// Saturation records exhaustion rather than granting another failing action.
    pub fn debit_carrier_usage(
        &self,
        usage: tos_compiler::native_snapshot_carriers::CapturedCarrierUsage,
    ) -> Result<()> {
        let rows = self.rows.get().checked_sub(usage.rows);
        let input = self.input_bytes.get().checked_sub(usage.input_bytes);
        let visits = self.charge_visits(usage.json_visits);
        self.rows.set(rows.unwrap_or(0));
        self.input_bytes.set(input.unwrap_or(0));
        if rows.is_none() || input.is_none() || visits.is_err() {
            return Err("Core lazy original aggregate carrier usage exhausted");
        }
        Ok(())
    }
}
pub(super) struct Reservation<'a>(&'a Ledger, usize);
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        self.0.reserved.set(
            self.0
                .reserved
                .get()
                .checked_sub(self.1)
                .expect("owned lazy state reservation"),
        );
    }
}
pub(super) struct Workspace<'a>(pub &'a Ledger);
impl session_transport::Workspace for Workspace<'_> {
    fn reserve(&mut self, bytes: usize) -> Result<()> {
        let n = self
            .0
            .reserved
            .get()
            .checked_add(bytes)
            .ok_or("Core lazy transport reservation overflow")?;
        if bytes > self.0.remaining(0)? {
            return Err("Core lazy transport remaining state");
        }
        self.0.reserved.set(n);
        Ok(())
    }
    fn release(&mut self, bytes: usize) {
        self.0.reserved.set(
            self.0
                .reserved
                .get()
                .checked_sub(bytes)
                .expect("owned lazy transport reservation"),
        );
    }
    fn charge_work(&mut self, bytes: u64) -> Result<()> {
        self.0.charge_work(bytes)
    }
}
