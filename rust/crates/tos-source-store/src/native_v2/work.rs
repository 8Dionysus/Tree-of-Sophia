//! Shared invocation ledger; clones never restart a budget.
use std::{
    cell::Cell,
    io,
    mem::size_of,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
pub fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}
pub fn active(deadline: Instant, cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "source admission cancelled",
        ));
    }
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "source admission deadline exceeded",
        ));
    }
    Ok(())
}
#[derive(Clone)]
pub struct AdmissionWorkBudget {
    inner: Rc<AdmissionWorkState>,
}

struct AdmissionWorkState {
    used: Cell<u64>,
    maximum: Cell<u64>,
}

impl AdmissionWorkBudget {
    pub fn new(maximum: u64) -> io::Result<Self> {
        if maximum == 0 || maximum == u64::MAX {
            return Err(invalid("selected source work bound is not finite"));
        }
        Ok(Self {
            inner: Rc::new(AdmissionWorkState {
                used: Cell::new(0),
                maximum: Cell::new(maximum),
            }),
        })
    }

    pub fn charge(&self, _kind: impl Copy) -> io::Result<()> {
        self.charge_many(1)
    }

    pub fn charge_many(&self, count: u64) -> io::Result<()> {
        let used = self
            .inner
            .used
            .get()
            .checked_add(count)
            .filter(|used| *used <= self.inner.maximum.get())
            .ok_or_else(|| invalid("selected source work ceiling exhausted"))?;
        self.inner.used.set(used);
        Ok(())
    }

    pub fn used(&self) -> u64 {
        self.inner.used.get()
    }

    pub fn remaining(&self) -> io::Result<u64> {
        self.inner
            .maximum
            .get()
            .checked_sub(self.inner.used.get())
            .ok_or_else(|| invalid("selected source work meter regressed"))
    }

    pub fn maximum(&self) -> u64 {
        self.inner.maximum.get()
    }

    /// Narrow the original shared operation ceiling after identity preparation.
    /// Already charged preparation remains charged on every retained clone.
    pub fn lower_maximum(&self, maximum: u64) -> io::Result<()> {
        if maximum == 0
            || maximum == u64::MAX
            || maximum > self.inner.maximum.get()
            || maximum < self.inner.used.get()
        {
            return Err(invalid("selected source work ceiling cannot be narrowed"));
        }
        self.inner.maximum.set(maximum);
        Ok(())
    }

    pub fn retained_allocation_upper_bound_bytes() -> usize {
        size_of::<AdmissionWorkState>() + 2 * size_of::<usize>()
    }
}
