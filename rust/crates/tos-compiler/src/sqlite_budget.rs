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
