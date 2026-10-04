//! Connection-local SQLite work limits. External sorter and rollback files
//! require the separate host quota contract described in the crate README.

use crate::{Error, Limits, Result};
use rusqlite::Connection;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

const MAX_PROGRESS_INTERVAL: u64 = 1_000;

pub(crate) fn effective_vm_cap(limits: Limits) -> u64 {
    let interval = limits.max_sql_vm_steps.min(MAX_PROGRESS_INTERVAL);
    (limits.max_sql_vm_steps / interval) * interval
}

pub(crate) fn install_progress(db: &Connection, limits: Limits, used: Arc<AtomicU64>) {
    install_progress_inner(db, limits, used, None);
}

pub(crate) fn install_progress_until(
    db: &Connection,
    limits: Limits,
    used: Arc<AtomicU64>,
    deadline: Instant,
) {
    install_progress_inner(db, limits, used, Some(deadline));
}

fn install_progress_inner(
    db: &Connection,
    limits: Limits,
    used: Arc<AtomicU64>,
    deadline: Option<Instant>,
) {
    let interval = limits.max_sql_vm_steps.min(MAX_PROGRESS_INTERVAL);
    let effective_cap = effective_vm_cap(limits);
    db.progress_handler(
        interval as i32,
        Some(move || {
            used.fetch_add(interval, Ordering::Relaxed)
                .saturating_add(interval)
                >= effective_cap
                || deadline.is_some_and(|limit| Instant::now() >= limit)
        }),
    );
}

pub(crate) fn configure(db: &Connection, limits: Limits) -> Result<Arc<AtomicU64>> {
    let used = Arc::new(AtomicU64::new(0));
    configure_with_counter(db, limits, Arc::clone(&used))?;
    Ok(used)
}

pub(crate) fn configure_with_counter(
    db: &Connection,
    limits: Limits,
    used: Arc<AtomicU64>,
) -> Result<()> {
    install_progress(db, limits, used);
    configure_limits(db, limits)
}

pub(crate) fn configure_with_counter_until(
    db: &Connection,
    limits: Limits,
    used: Arc<AtomicU64>,
    deadline: Instant,
) -> Result<()> {
    install_progress_until(db, limits, used, deadline);
    configure_limits(db, limits)
}

fn configure_limits(db: &Connection, limits: Limits) -> Result<()> {
    db.execute_batch(
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE;",
    )?;
    db.pragma_update(None, "cache_size", -(limits.sqlite_cache_kib as i64))?;
    let cache_size: i64 = db.query_row("PRAGMA cache_size", [], |r| r.get(0))?;
    let temp_store: i64 = db.query_row("PRAGMA temp_store", [], |r| r.get(0))?;
    if cache_size != -(limits.sqlite_cache_kib as i64) || temp_store != 1 {
        return Err(Error::Invalid("SQLite cache/temp policy not applied"));
    }
    let page_size: u64 = db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    if limits.max_output_bytes < page_size {
        return Err(Error::Budget("output smaller than SQLite page"));
    }
    let page_cap = limits.max_output_bytes / page_size;
    db.pragma_update(None, "max_page_count", page_cap)?;
    let effective_page_cap: u64 = db.query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
    if effective_page_cap > page_cap {
        return Err(Error::Invalid("SQLite output page cap not applied"));
    }
    Ok(())
}

/// One prepaid window on the original serial owner's VM ledger. Unused
/// partial windows remain charged when a connection is closed or refuses.
/// This is conservative VM-window admission, not an exact CPU measurement.
pub(crate) struct SharedVmWindow {
    used: Arc<AtomicU64>,
    cap: u64,
    interval: u64,
}
fn reserve_vm_window(used: &AtomicU64, cap: u64, interval: u64) -> Result<()> {
    let mut current = used.load(Ordering::Acquire);
    loop {
        let next = current.checked_add(interval).filter(|n| *n <= cap)
            .ok_or(Error::Budget("shared SQLite VM window"))?;
        match used.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(value) => current = value,
        }
    }
}
impl SharedVmWindow {
    /// Call before opening/using the new connection; this does not create one.
    pub(crate) fn reserve(used: Arc<AtomicU64>, cap: u64) -> Result<Self> {
        if cap == 0 { return Err(Error::Budget("shared SQLite VM ceiling")); }
        // A new statement can reset SQLite's progress phase. One instruction
        // per window avoids an uncharged short-statement tail; old sampled
        // compatibility APIs retain their existing interval policy.
        let interval = 1;
        reserve_vm_window(&used, cap, interval)?;
        Ok(Self { used, cap, interval })
    }
    pub(crate) fn install(self, db: &Connection, deadline: Instant,
        cancelled: Arc<std::sync::atomic::AtomicBool>) {
        let interval = self.interval;
        db.progress_handler(interval as i32, Some(move || {
            if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
                return true;
            }
            // Prior window has completed. Reserve the next before continuing;
            // refusal preserves the original counter and stops this statement.
            reserve_vm_window(&self.used, self.cap, interval).is_err()
        }));
    }
}


// This owner is used ONLY by the dedicated oneDriver native-session child.
// Its SourceFrame contract places every SQLite user (carrier, Store, Whole,
// reader and final fences) under this same retained original state owner.
static DEDICATED_HEAP_ESTABLISHED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Stable technical suballocation of the original whole state, not a new
/// grant or a claim that the selected model fits. Actual remaining state must
/// independently admit this pool and its holder before establishment.
pub fn dedicated_session_heap_bytes(original_whole_state_bytes: usize) -> Result<usize> {
    let bytes = original_whole_state_bytes / 2;
    if bytes < 65536 || bytes > i64::MAX as usize {
        return Err(Error::Budget("dedicated SQLite heap profile"));
    }
    Ok(bytes)
}

/// One monotonically bounded process-wide SQLite pool in the dedicated native
/// session. Never establish this in Python/shared legacy processes. There is
/// no Drop restore: process exit ends the pool, dropping a role cannot refund
/// it. All SQLite connections must start after this holder is established.
pub struct DedicatedSessionSqliteHeap {
    reserved_bytes: usize,
    effective_limit: i64,
}
impl DedicatedSessionSqliteHeap {
    pub fn establish(
        heap_bytes: usize,
        remaining_after_retained: &dyn Fn(usize) -> Result<usize>,
        deadline: Instant,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Arc<Self>> {
        let active = || {
            if Instant::now() >= deadline || cancelled.load(Ordering::Acquire) {
                Err(Error::Budget("dedicated SQLite heap cutoff/cancellation"))
            } else { Ok(()) }
        };
        active()?;
        if !cfg!(target_os = "linux") {
            return Err(Error::Invalid("dedicated SQLite initializer requires Linux"));
        }
        if heap_bytes < 65536 || heap_bytes > i64::MAX as usize {
            return Err(Error::Budget("dedicated SQLite heap bytes"));
        }
        let holder_bytes = std::mem::size_of::<Self>()
            .checked_add(2 * std::mem::size_of::<std::sync::atomic::AtomicUsize>())
            .ok_or(Error::Budget("dedicated SQLite heap holder"))?;
        let reserved_bytes = heap_bytes.checked_add(holder_bytes)
            .ok_or(Error::Budget("dedicated SQLite heap state"))?;
        // Before the setter's bundled initialization and before Arc allocation.
        remaining_after_retained(reserved_bytes)?;
        if DEDICATED_HEAP_ESTABLISHED.compare_exchange(false, true,
            Ordering::AcqRel, Ordering::Acquire).is_err() {
            return Err(Error::Invalid("dedicated SQLite heap already established"));
        }
        // These compile-option/status queries use static metadata and status
        // counters, not a connection. Reject a pre-existing allocator owner.
        // The pinned bundled/Linux initializer uses its default static pcache/
        // VFS plus temporary recursive mutex/10-byte OS probe; the >=64KiB
        // reserved pool covers that finite initialization before hardLimit is
        // installed. Custom allocator/VFS initialization is outside this mode.
        unsafe {
            // Configuration succeeds only before SQLite initialization. This
            // dedicated route explicitly enables tracked allocator accounting;
            // memory_used()==0 by itself cannot establish either property.
            // The dedicated child callgraph supplies the separate default
            // allocator/VFS/no-external-pool ownership invariant.
            if rusqlite::ffi::sqlite3_config(rusqlite::ffi::SQLITE_CONFIG_MEMSTATUS, 1i32)
                != rusqlite::ffi::SQLITE_OK {
                return Err(Error::Invalid("SQLite initialized before dedicated heap owner"));
            }
            if rusqlite::ffi::sqlite3_memory_used() != 0 {
                return Err(Error::Invalid("SQLite tracked heap before dedicated heap owner"));
            }
            if rusqlite::ffi::sqlite3_compileoption_used(c"OMIT_WSD".as_ptr()) != 0
                || rusqlite::ffi::sqlite3_compileoption_used(c"MEMDEBUG".as_ptr()) != 0 {
                return Err(Error::Invalid("unsupported dedicated SQLite initializer"));
            }
            let previous = rusqlite::ffi::sqlite3_hard_heap_limit64(-1);
            if previous < 0 {
                return Err(Error::Invalid("dedicated SQLite initialization failed"));
            }
            let requested = heap_bytes as i64;
            let effective_limit = if previous > 0 { previous.min(requested) } else { requested };
            rusqlite::ffi::sqlite3_hard_heap_limit64(effective_limit);
            if rusqlite::ffi::sqlite3_hard_heap_limit64(-1) != effective_limit
                || rusqlite::ffi::sqlite3_memory_used() > effective_limit {
                return Err(Error::Budget("dedicated SQLite heap enforcement"));
            }
            active()?;
            Ok(Arc::new(Self { reserved_bytes, effective_limit }))
        }
    }
    /// Include this whole pool once in the original session's retained ledger.
    pub fn reserved_state_bytes(&self) -> usize { self.reserved_bytes }
    pub fn effective_heap_bytes(&self) -> usize { self.effective_limit as usize }
    pub fn verify_current(&self) -> Result<()> {
        let current = unsafe { rusqlite::ffi::sqlite3_hard_heap_limit64(-1) };
        if current <= 0 || current > self.effective_limit {
            return Err(Error::Invalid("dedicated SQLite heap widened"));
        }
        Ok(())
    }
}
