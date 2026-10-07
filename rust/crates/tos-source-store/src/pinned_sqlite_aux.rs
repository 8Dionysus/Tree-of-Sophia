//! Scope-bounded unnamed SQLite scratch and shared logical-I/O ledgers.
//!
//! The public budgets are deliberately independent of SQLite so callers can
//! reserve their output/staging lifetime against the same declared envelope.

use crate::pinned_sqlite::FdIoPolicy;
use crate::{Result, StoreError, StoreErrorCode};
use rusqlite::ffi;
use std::fs::OpenOptions;
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_void},
    fs::File,
    io::ErrorKind,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    },
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PinnedSqliteIoFailure {
    ReadLimit = 1,
    WriteLimit = 2,
    Deadline = 3,
    Cancelled = 4,
    FileLimit = 5,
    SpaceLimit = 6,
    Io = 7,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PinnedSqliteIoSnapshot {
    /// The current finite ceiling and the caller that last lowered it. Missing
    /// values mean the limit lock could not be observed, never an unlimited grant.
    pub read_limit_bytes: Option<u64>,
    pub read_limit_origin: Option<&'static std::panic::Location<'static>>,
    pub read_attempted_bytes: u64,
    /// Admitted logical guard envelopes included in attempted reads. These
    /// are upper bounds, not measured payload or kernel transfer bytes.
    pub read_upper_bound_attempted_bytes: u64,
    pub read_permitted_bytes: u64,
    pub read_returned_bytes: u64,
    /// Includes bytes submitted to `xWrite` and logical extension bytes
    /// requested through `xTruncate`; no refund occurs for denied attempts.
    pub write_attempted_bytes: u64,
    pub write_permitted_bytes: u64,
    /// Actual payload bytes accepted by `xWrite`. `xTruncate` extensions have
    /// no transfer payload and therefore do not inflate this field.
    pub write_returned_bytes: u64,
    /// First bounded failure observed by any clone. This keeps a SQLite
    /// `IOERR` attributable to its originating envelope or deadline.
    pub failure: Option<PinnedSqliteIoFailure>,
    /// Local failure before any failure inherited from the shared write owner.
    pub local_failure: Option<PinnedSqliteIoFailure>,
    /// First numeric file-limit observation, without paths or payloads. The
    /// check names the units; it does not alter failure or admission authority.
    pub file_limit_observation: Option<(&'static str, u64, u64)>,
}

#[derive(Debug)]
struct IoLimits {
    max_read: u64,
    max_write: u64,
    read_origin: &'static std::panic::Location<'static>,
}

#[derive(Debug)]
struct IoState {
    // Charge and restriction share one linearization point. An atomic ceiling
    // alone would permit a charge to commit against a stale, larger limit.
    limits: Mutex<IoLimits>,
    // One immutable root authority, never another child. Existing write-only
    // children retain local reads; shared-IO children debit both root ceilings.
    aggregate_write: Option<PinnedSqliteIoBudget>,
    share_reads: bool,
    read_attempted: AtomicU64,
    read_upper_bound_attempted: AtomicU64,
    read_upper_bound_permitted: AtomicU64,
    read_permitted: AtomicU64,
    read_returned: AtomicU64,
    write_attempted: AtomicU64,
    write_permitted: AtomicU64,
    write_returned: AtomicU64,
    failure: AtomicU8,
    file_limit_observation: Mutex<Option<(&'static str, u64, u64)>>,
}

/// Cloneable cumulative logical I/O authority shared by every participating
/// source reader, SQLite pager and caller-owned output step.
#[derive(Clone, Debug)]
pub struct PinnedSqliteIoBudget(Arc<IoState>);

impl PinnedSqliteIoBudget {
    /// Upper bound for this owner's one shared Arc allocation, including its
    /// state and Arc reference counters. The caller accounts for its own handle.
    pub fn shared_state_upper_bound() -> usize {
        std::mem::size_of::<IoState>()
            + 2 * std::mem::size_of::<AtomicUsize>()
            + std::mem::align_of::<IoState>()
    }

    /// Identity of the existing ledger, never numeric-limit equivalence.
    pub fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[track_caller]
    pub fn new(max_read_bytes: u64, max_write_bytes: u64) -> Result<Self> {
        if max_read_bytes == 0
            || max_write_bytes == 0
            || max_read_bytes == u64::MAX
            || max_write_bytes == u64::MAX
        {
            return Err(budget_error("SQLite logical I/O limits must be nonzero"));
        }
        Ok(Self(Arc::new(IoState {
            limits: Mutex::new(IoLimits {
                max_read: max_read_bytes,
                max_write: max_write_bytes,
                read_origin: std::panic::Location::caller(),
            }),
            aggregate_write: None,
            share_reads: false,
            read_attempted: AtomicU64::new(0),
            read_upper_bound_attempted: AtomicU64::new(0),
            read_upper_bound_permitted: AtomicU64::new(0),
            read_permitted: AtomicU64::new(0),
            read_returned: AtomicU64::new(0),
            write_attempted: AtomicU64::new(0),
            write_permitted: AtomicU64::new(0),
            write_returned: AtomicU64::new(0),
            failure: AtomicU8::new(0),
            file_limit_observation: Mutex::new(None),
        })))
    }

    /// Create a local read/write ledger backed by one existing cumulative
    /// write authority. Local restrictions and counters remain independent;
    /// every permitted write must fit both ceilings before IO begins.
    /// Only a root may back children: this is one shared pool, not a hierarchy.
    #[track_caller]
    pub fn new_with_shared_write_authority(
        max_read_bytes: u64,
        max_local_write_bytes: u64,
        aggregate_write: Self,
    ) -> Result<Self> {
        if aggregate_write.0.aggregate_write.is_some()
            || aggregate_write.failure_code() != 0
            || aggregate_write
                .0
                .limits
                .lock()
                .map_err(|_| {
                    aggregate_write.fail(PinnedSqliteIoFailure::Io);
                    budget_error("SQLite aggregate write limit lock is poisoned")
                })?
                .max_write
                == 0
        {
            return Err(budget_error(
                "SQLite shared write authority must be a live writable root",
            ));
        }
        let mut local = Self::new(max_read_bytes, max_local_write_bytes)?;
        // `new` returned the sole local Arc, and the private parent link is
        // assigned only here before clones can escape. Cycles cannot form.
        Arc::get_mut(&mut local.0)
            .expect("new local IO ledger has one owner")
            .aggregate_write = Some(aggregate_write);
        Ok(local)
    }

    /// Keep local producer counters while admitting every read and write
    /// against the same original root. This permits sequential phases to use
    /// unused read capacity without granting either phase a second pool.
    #[track_caller]
    pub fn new_with_shared_io_authority(
        max_local_read_bytes: u64,
        max_local_write_bytes: u64,
        root: Self,
    ) -> Result<Self> {
        let mut local = Self::new_with_shared_write_authority(
            max_local_read_bytes,
            max_local_write_bytes,
            root,
        )?;
        Arc::get_mut(&mut local.0)
            .expect("new local IO ledger has one owner")
            .share_reads = true;
        Ok(local)
    }

    /// Exact identity of the shared write pool, including root/child pairs.
    pub fn shares_write_authority_with(&self, other: &Self) -> bool {
        let root = self.0.aggregate_write.as_ref().unwrap_or(self);
        let other_root = other.0.aggregate_write.as_ref().unwrap_or(other);
        root.shares_with(other_root)
    }

    /// The aggregate census is observed separately, never added to the local
    /// write sum as a third producer. Only shared-IO children charge root reads.
    pub fn shared_write_snapshot(&self) -> PinnedSqliteIoSnapshot {
        self.0.aggregate_write.as_ref().unwrap_or(self).snapshot()
    }

    fn failure_code(&self) -> u8 {
        let local = self.0.failure.load(Ordering::Acquire);
        if local != 0 {
            local
        } else {
            self.0
                .aggregate_write
                .as_ref()
                .map_or(0, |root| root.0.failure.load(Ordering::Acquire))
        }
    }

    /// Create one cumulative read-only logical-I/O ledger. Zero writes are a
    /// denied capability, not an artificial one-byte allowance. Reads and
    /// upper-bound guard charges retain the same identity and accounting law
    /// as a regular request ledger.
    #[track_caller]
    pub fn new_read_only(max_read_bytes: u64) -> Result<Self> {
        if max_read_bytes == 0 || max_read_bytes == u64::MAX {
            return Err(budget_error(
                "SQLite read-only limit must be finite and nonzero",
            ));
        }
        Ok(Self(Arc::new(IoState {
            limits: Mutex::new(IoLimits {
                max_read: max_read_bytes,
                max_write: 0,
                read_origin: std::panic::Location::caller(),
            }),
            aggregate_write: None,
            share_reads: false,
            read_attempted: AtomicU64::new(0),
            read_upper_bound_attempted: AtomicU64::new(0),
            read_upper_bound_permitted: AtomicU64::new(0),
            read_permitted: AtomicU64::new(0),
            read_returned: AtomicU64::new(0),
            write_attempted: AtomicU64::new(0),
            write_permitted: AtomicU64::new(0),
            write_returned: AtomicU64::new(0),
            failure: AtomicU8::new(0),
            file_limit_observation: Mutex::new(None),
        })))
    }

    pub fn charge_read(&self, bytes: u64) -> Result<()> {
        self.charge_read_classified(bytes, false)
    }

    /// Debit a source-owned conservative metadata/guard envelope against the
    /// same cumulative read ceiling. Keep its classification through denied
    /// attempts; callers must not report this amount as returned payload.
    pub fn charge_read_upper_bound(&self, bytes: u64) -> Result<()> {
        self.charge_read_classified(bytes, true)
    }

    fn charge_read_classified(&self, bytes: u64, upper_bound: bool) -> Result<()> {
        if self.0.share_reads {
            if let Some(root) = self.0.aggregate_write.as_ref() {
                return self.charge_shared_read(root, bytes, upper_bound);
            }
        }
        let limits = self.0.limits.lock().map_err(|_| {
            if upper_bound {
                saturating_add(&self.0.read_upper_bound_attempted, bytes);
            }
            saturating_add(&self.0.read_attempted, bytes);
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite logical I/O limit lock is poisoned")
        })?;
        if upper_bound {
            saturating_add(&self.0.read_upper_bound_attempted, bytes);
        }
        if self.failure_code() != 0 {
            saturating_add(&self.0.read_attempted, bytes);
            return Err(budget_error(
                "SQLite logical I/O ledger has a prior failure",
            ));
        }
        charge(
            &self.0.read_attempted,
            &self.0.read_permitted,
            limits.max_read,
            bytes,
        )
        .map_err(|_| {
            self.fail(PinnedSqliteIoFailure::ReadLimit);
            budget_error("SQLite cumulative read budget exceeded")
        })?;
        if upper_bound {
            // The same limit lock protects total and tagged permits, so a
            // payload return cannot borrow a metadata-only envelope.
            saturating_add(&self.0.read_upper_bound_permitted, bytes);
        }
        Ok(())
    }

    fn charge_shared_read(&self, root: &Self, bytes: u64, upper_bound: bool) -> Result<()> {
        // Match the write lock order: local then root. No root locks a child.
        let record_attempt = || {
            for ledger in [self, root] {
                saturating_add(&ledger.0.read_attempted, bytes);
                if upper_bound {
                    saturating_add(&ledger.0.read_upper_bound_attempted, bytes);
                }
            }
        };
        let local_limits = self.0.limits.lock().map_err(|_| {
            record_attempt();
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite local read limit lock is poisoned")
        })?;
        let root_limits = root.0.limits.lock().map_err(|_| {
            record_attempt();
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite aggregate read limit lock is poisoned")
        })?;
        record_attempt();
        if self.failure_code() != 0 {
            return Err(budget_error(
                "SQLite shared read ledger has a prior failure",
            ));
        }
        let local_next = self
            .0
            .read_permitted
            .load(Ordering::Acquire)
            .checked_add(bytes)
            .filter(|n| *n <= local_limits.max_read);
        let root_next = root
            .0
            .read_permitted
            .load(Ordering::Acquire)
            .checked_add(bytes)
            .filter(|n| *n <= root_limits.max_read);
        let (Some(local_next), Some(root_next)) = (local_next, root_next) else {
            self.fail(PinnedSqliteIoFailure::ReadLimit);
            return Err(budget_error(
                "SQLite local or aggregate cumulative read budget exceeded",
            ));
        };
        self.0.read_permitted.store(local_next, Ordering::Release);
        root.0.read_permitted.store(root_next, Ordering::Release);
        if upper_bound {
            saturating_add(&self.0.read_upper_bound_permitted, bytes);
            saturating_add(&root.0.read_upper_bound_permitted, bytes);
        }
        Ok(())
    }

    pub fn charge_write(&self, bytes: u64) -> Result<()> {
        if let Some(root) = self.0.aggregate_write.as_ref() {
            return self.charge_shared_write(root, bytes);
        }
        let limits = self.0.limits.lock().map_err(|_| {
            saturating_add(&self.0.write_attempted, bytes);
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite logical I/O limit lock is poisoned")
        })?;
        if self.failure_code() != 0 {
            saturating_add(&self.0.write_attempted, bytes);
            return Err(budget_error(
                "SQLite logical I/O ledger has a prior failure",
            ));
        }
        charge(
            &self.0.write_attempted,
            &self.0.write_permitted,
            limits.max_write,
            bytes,
        )
        .map_err(|_| {
            self.fail(PinnedSqliteIoFailure::WriteLimit);
            budget_error("SQLite cumulative write budget exceeded")
        })
    }

    fn charge_shared_write(&self, root: &Self, bytes: u64) -> Result<()> {
        // All children acquire local then root; roots never acquire a child.
        // Hold both restriction locks until both permits are committed.
        let local_limits = self.0.limits.lock().map_err(|_| {
            saturating_add(&self.0.write_attempted, bytes);
            saturating_add(&root.0.write_attempted, bytes);
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite local write limit lock is poisoned")
        })?;
        let root_limits = root.0.limits.lock().map_err(|_| {
            saturating_add(&self.0.write_attempted, bytes);
            saturating_add(&root.0.write_attempted, bytes);
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite aggregate write limit lock is poisoned")
        })?;
        saturating_add(&self.0.write_attempted, bytes);
        saturating_add(&root.0.write_attempted, bytes);
        if self.failure_code() != 0 {
            return Err(budget_error(
                "SQLite shared write ledger has a prior failure",
            ));
        }
        let local_next = self
            .0
            .write_permitted
            .load(Ordering::Acquire)
            .checked_add(bytes)
            .filter(|next| *next <= local_limits.max_write);
        let root_next = root
            .0
            .write_permitted
            .load(Ordering::Acquire)
            .checked_add(bytes)
            .filter(|next| *next <= root_limits.max_write);
        let (Some(local_next), Some(root_next)) = (local_next, root_next) else {
            self.fail(PinnedSqliteIoFailure::WriteLimit);
            return Err(budget_error(
                "SQLite local or aggregate cumulative write budget exceeded",
            ));
        };
        self.0.write_permitted.store(local_next, Ordering::Release);
        root.0.write_permitted.store(root_next, Ordering::Release);
        Ok(())
    }

    pub fn record_read_returned(&self, bytes: u64) -> Result<()> {
        let _limits = self.0.limits.lock().map_err(|_| {
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite logical I/O limit lock is poisoned")
        })?;
        let allowed = self
            .0
            .read_permitted
            .load(Ordering::Acquire)
            .checked_sub(self.0.read_upper_bound_permitted.load(Ordering::Acquire))
            .ok_or_else(|| {
                self.fail(PinnedSqliteIoFailure::Io);
                budget_error("SQLite read permit classification regressed")
            })?;
        record_returned_up_to(&self.0.read_returned, allowed, bytes).map_err(|_| {
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite read return exceeded payload permits")
        })?;
        if self.0.share_reads {
            if let Some(root) = self.0.aggregate_write.as_ref() {
                root.record_read_returned(bytes)?;
            }
        }
        Ok(())
    }

    /// Narrow this same cumulative ledger to the caller's genuinely remaining
    /// outer read/write slices before further IO. Existing attempts, permits,
    /// returns and failures are never reset or refunded. Already issued IO may
    /// finish; its entire permit is included in the used baseline.
    ///
    /// The caller must finish/debit external IO before selecting these slices
    /// and prevent concurrent external spending of that remaining outer pool.
    /// This mechanical restriction neither verifies that outer accounting nor
    /// grants a larger limit: repeated calls can only retain or lower ceilings.
    /// Zero remaining bytes are valid. No new Arc/request identity is created.
    #[track_caller]
    pub fn restrict_remaining_io(
        &self,
        remaining_read_bytes: u64,
        remaining_write_bytes: u64,
    ) -> Result<()> {
        let mut limits = self.0.limits.lock().map_err(|_| {
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite logical I/O limit lock is poisoned")
        })?;
        if self.failure_code() != 0 {
            return Err(budget_error(
                "SQLite logical I/O ledger has a prior failure",
            ));
        }
        if remaining_read_bytes == u64::MAX || remaining_write_bytes == u64::MAX {
            return Err(budget_error("SQLite remaining I/O slice is unbounded"));
        }
        let used_read = self
            .0
            .read_attempted
            .load(Ordering::Acquire)
            .max(self.0.read_permitted.load(Ordering::Acquire))
            .max(self.0.read_returned.load(Ordering::Acquire));
        let used_write = self
            .0
            .write_attempted
            .load(Ordering::Acquire)
            .max(self.0.write_permitted.load(Ordering::Acquire))
            .max(self.0.write_returned.load(Ordering::Acquire));
        // Validate both additions before mutating either ceiling.
        let max_read = used_read
            .checked_add(remaining_read_bytes)
            .ok_or_else(|| budget_error("SQLite remaining read slice overflow"))?;
        let max_write = used_write
            .checked_add(remaining_write_bytes)
            .ok_or_else(|| budget_error("SQLite remaining write slice overflow"))?;
        if max_read < limits.max_read {
            limits.max_read = max_read;
            limits.read_origin = std::panic::Location::caller();
        }
        limits.max_write = limits.max_write.min(max_write);
        Ok(())
    }

    pub fn record_write_returned(&self, bytes: u64) -> Result<()> {
        if let Some(root) = self.0.aggregate_write.as_ref() {
            // Already permitted IO may return after a different attempt failed.
            // Validate the local permit first so no child borrows another's.
            record_returned(&self.0.write_returned, &self.0.write_permitted, bytes).map_err(
                |_| {
                    self.fail(PinnedSqliteIoFailure::Io);
                    budget_error("SQLite local write return exceeded permitted bytes")
                },
            )?;
            return root.record_write_returned(bytes).map_err(|error| {
                self.fail(PinnedSqliteIoFailure::Io);
                error
            });
        }
        record_returned(&self.0.write_returned, &self.0.write_permitted, bytes).map_err(|_| {
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite write return exceeded permitted bytes")
        })
    }

    pub fn snapshot(&self) -> PinnedSqliteIoSnapshot {
        let limits = self.0.limits.lock().ok();
        PinnedSqliteIoSnapshot {
            read_limit_bytes: limits.as_ref().map(|limits| limits.max_read),
            read_limit_origin: limits.as_ref().map(|limits| limits.read_origin),
            read_attempted_bytes: self.0.read_attempted.load(Ordering::Acquire),
            read_upper_bound_attempted_bytes: self
                .0
                .read_upper_bound_attempted
                .load(Ordering::Acquire),
            read_permitted_bytes: self.0.read_permitted.load(Ordering::Acquire),
            read_returned_bytes: self.0.read_returned.load(Ordering::Acquire),
            write_attempted_bytes: self.0.write_attempted.load(Ordering::Acquire),
            write_permitted_bytes: self.0.write_permitted.load(Ordering::Acquire),
            write_returned_bytes: self.0.write_returned.load(Ordering::Acquire),
            failure: decode_failure(self.failure_code()),
            local_failure: decode_failure(self.0.failure.load(Ordering::Acquire)),
            file_limit_observation: self.0.file_limit_observation.lock().ok().and_then(|v| *v),
        }
    }

    fn fail_file_limit(&self, check: &'static str, attempted: u64, limit: u64) {
        if let Ok(mut observation) = self.0.file_limit_observation.lock() {
            observation.get_or_insert((check, attempted, limit));
        }
        if let Some(root) = self.0.aggregate_write.as_ref() {
            root.fail_file_limit(check, attempted, limit);
        }
        self.fail(PinnedSqliteIoFailure::FileLimit);
    }

    pub(super) fn fail(&self, reason: PinnedSqliteIoFailure) {
        let _ =
            self.0
                .failure
                .compare_exchange(0, reason as u8, Ordering::AcqRel, Ordering::Acquire);
        if let Some(root) = self.0.aggregate_write.as_ref() {
            root.fail(reason);
        }
    }
}

fn charge(attempted: &AtomicU64, permitted: &AtomicU64, max: u64, bytes: u64) -> Result<()> {
    saturating_add(attempted, bytes);
    let mut current = permitted.load(Ordering::Acquire);
    loop {
        let next = current
            .checked_add(bytes)
            .filter(|value| *value <= max)
            .ok_or_else(|| budget_error("SQLite cumulative logical I/O limit exceeded"))?;
        match permitted.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn record_returned(returned: &AtomicU64, permitted: &AtomicU64, bytes: u64) -> Result<()> {
    record_returned_up_to(returned, permitted.load(Ordering::Acquire), bytes)
}

fn record_returned_up_to(returned: &AtomicU64, allowed: u64, bytes: u64) -> Result<()> {
    let mut current = returned.load(Ordering::Acquire);
    loop {
        let next = current
            .checked_add(bytes)
            .filter(|value| *value <= allowed)
            .ok_or_else(|| budget_error("SQLite returned more logical I/O than permitted"))?;
        match returned.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn saturating_add(counter: &AtomicU64, bytes: u64) {
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
        Some(old.saturating_add(bytes))
    });
}

fn decode_failure(raw: u8) -> Option<PinnedSqliteIoFailure> {
    match raw {
        1 => Some(PinnedSqliteIoFailure::ReadLimit),
        2 => Some(PinnedSqliteIoFailure::WriteLimit),
        3 => Some(PinnedSqliteIoFailure::Deadline),
        4 => Some(PinnedSqliteIoFailure::Cancelled),
        5 => Some(PinnedSqliteIoFailure::FileLimit),
        6 => Some(PinnedSqliteIoFailure::SpaceLimit),
        7 => Some(PinnedSqliteIoFailure::Io),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PinnedSqliteSpaceSnapshot {
    pub declared_available_bytes: u64,
    pub reserved_current_bytes: u64,
    pub reserved_high_water_bytes: u64,
    pub actual_observed_current_bytes: u64,
    pub actual_observed_high_water_bytes: u64,
    pub allocation_anomalies: u64,
    /// False means a mutex was poisoned; counters are diagnostic only.
    pub ledger_consistent: bool,
}

#[derive(Debug, Default)]
struct SpaceState {
    reserved_current: u64,
    reserved_high_water: u64,
    actual_current: u64,
    actual_high_water: u64,
    allocation_anomalies: u64,
}

#[derive(Debug)]
struct SpaceLedger {
    declared: u64,
    state: Mutex<SpaceState>,
}

/// Cloneable physical-space envelope. It reserves caller-declared allocation
/// ceilings; it does not claim that a filesystem has granted those bytes.
#[derive(Clone, Debug)]
pub struct PinnedSqliteSpaceBudget(Arc<SpaceLedger>);

impl PinnedSqliteSpaceBudget {
    /// Upper bound for this owner's one shared Arc allocation, including its
    /// state and Arc reference counters. The caller accounts for its own handle.
    pub fn shared_state_upper_bound() -> usize {
        std::mem::size_of::<SpaceLedger>()
            + 2 * std::mem::size_of::<AtomicUsize>()
            + std::mem::align_of::<SpaceLedger>()
    }

    /// Identity of the existing ledger; this creates no reservation or grant.
    pub fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn new(available_declared_bytes: u64) -> Result<Self> {
        if available_declared_bytes == 0 || available_declared_bytes == u64::MAX {
            return Err(budget_error(
                "SQLite declared physical space must be nonzero",
            ));
        }
        Ok(Self(Arc::new(SpaceLedger {
            declared: available_declared_bytes,
            state: Mutex::new(SpaceState::default()),
        })))
    }

    pub fn reserve(&self, bytes: u64) -> Result<PinnedSqliteSpaceReservation> {
        if bytes == 0 {
            return Err(budget_error("SQLite physical reservation must be nonzero"));
        }
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_| budget_error("SQLite physical-space ledger is poisoned"))?;
        if state.allocation_anomalies != 0 {
            return Err(budget_error(
                "SQLite physical-space ledger has an allocation anomaly",
            ));
        }
        let next = state
            .reserved_current
            .checked_add(bytes)
            .filter(|value| *value <= self.0.declared)
            .ok_or_else(|| budget_error("SQLite declared physical-space envelope exceeded"))?;
        state.reserved_current = next;
        state.reserved_high_water = state.reserved_high_water.max(next);
        drop(state);
        Ok(PinnedSqliteSpaceReservation {
            inner: Arc::new(ReservationInner {
                ledger: self.0.clone(),
                reserved: bytes,
                actual: AtomicU64::new(0),
            }),
        })
    }

    pub fn snapshot(&self) -> PinnedSqliteSpaceSnapshot {
        let (state, ledger_consistent) = match self.0.state.lock() {
            Ok(state) => (state, true),
            Err(poisoned) => (poisoned.into_inner(), false),
        };
        PinnedSqliteSpaceSnapshot {
            declared_available_bytes: self.0.declared,
            reserved_current_bytes: state.reserved_current,
            reserved_high_water_bytes: state.reserved_high_water,
            actual_observed_current_bytes: state.actual_current,
            actual_observed_high_water_bytes: state.actual_high_water,
            allocation_anomalies: state.allocation_anomalies,
            ledger_consistent,
        }
    }
}

#[derive(Debug)]
struct ReservationInner {
    ledger: Arc<SpaceLedger>,
    reserved: u64,
    actual: AtomicU64,
}

impl Drop for ReservationInner {
    fn drop(&mut self) {
        let actual = self.actual.load(Ordering::Acquire);
        let mut state = self
            .ledger
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.reserved_current = state.reserved_current.saturating_sub(self.reserved);
        state.actual_current = state.actual_current.saturating_sub(actual);
    }
}

#[derive(Debug)]
pub struct PinnedSqliteSpaceReservation {
    inner: Arc<ReservationInner>,
}

impl PinnedSqliteSpaceReservation {
    pub fn update_actual_allocated(&self, bytes: u64) -> Result<()> {
        let mut state = self
            .inner
            .ledger
            .state
            .lock()
            .map_err(|_| budget_error("SQLite physical-space ledger is poisoned"))?;
        let previous = self.inner.actual.swap(bytes, Ordering::AcqRel);
        state.actual_current = state
            .actual_current
            .saturating_sub(previous)
            .saturating_add(bytes);
        state.actual_high_water = state.actual_high_water.max(state.actual_current);
        if bytes > self.inner.reserved {
            state.allocation_anomalies = state.allocation_anomalies.saturating_add(1);
            Err(budget_error(
                "SQLite observed allocation exceeded its reservation",
            ))
        } else if state.allocation_anomalies != 0 {
            Err(budget_error(
                "SQLite physical-space ledger has an allocation anomaly",
            ))
        } else {
            Ok(())
        }
    }

    pub fn reserved_bytes(&self) -> u64 {
        self.inner.reserved
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PinnedSqliteAuxLimits {
    pub main_logical_bytes: u64,
    pub main_allocated_bytes: u64,
    pub temp_db_logical_bytes: u64,
    pub temp_db_allocated_bytes: u64,
    pub main_journal_logical_bytes: u64,
    pub main_journal_allocated_bytes: u64,
    pub temp_journal_logical_bytes: u64,
    pub temp_journal_allocated_bytes: u64,
    pub other_aux_aggregate_logical_bytes: u64,
    pub other_aux_aggregate_allocated_bytes: u64,
    pub max_live_aux: usize,
}

pub struct PinnedSqliteAuxRequest {
    pub limits: PinnedSqliteAuxLimits,
    pub io_budget: PinnedSqliteIoBudget,
    pub space_budget: PinnedSqliteSpaceBudget,
    pub deadline: Instant,
    pub cancelled: Arc<AtomicBool>,
}

impl PinnedSqliteAuxRequest {
    fn validate(&self) -> Result<()> {
        let l = self.limits;
        if l.main_logical_bytes == 0
            || l.main_allocated_bytes == 0
            || l.other_aux_aggregate_logical_bytes == 0
            || l.other_aux_aggregate_allocated_bytes == 0
            || l.max_live_aux == 0
            || l.max_live_aux == usize::MAX
            || self.deadline <= Instant::now()
            || self.cancelled.load(Ordering::Acquire)
            || self.io_budget.snapshot().failure.is_some()
            || [
                l.main_logical_bytes,
                l.main_allocated_bytes,
                l.temp_db_logical_bytes,
                l.temp_db_allocated_bytes,
                l.main_journal_logical_bytes,
                l.main_journal_allocated_bytes,
                l.temp_journal_logical_bytes,
                l.temp_journal_allocated_bytes,
                l.other_aux_aggregate_logical_bytes,
                l.other_aux_aggregate_allocated_bytes,
            ]
            .contains(&u64::MAX)
            || (l.temp_db_logical_bytes == 0) != (l.temp_db_allocated_bytes == 0)
            || (l.main_journal_logical_bytes == 0) != (l.main_journal_allocated_bytes == 0)
            || (l.temp_journal_logical_bytes == 0) != (l.temp_journal_allocated_bytes == 0)
        {
            return Err(budget_error("SQLite auxiliary request envelope is invalid"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuxClass {
    Main,
    MainJournal,
    TempDb,
    TempJournal,
    Other,
}

impl AuxClass {
    fn ceilings(self, limits: PinnedSqliteAuxLimits) -> (u64, u64) {
        match self {
            Self::Main => (limits.main_logical_bytes, limits.main_allocated_bytes),
            Self::MainJournal => (
                limits.main_journal_logical_bytes,
                limits.main_journal_allocated_bytes,
            ),
            Self::TempDb => (limits.temp_db_logical_bytes, limits.temp_db_allocated_bytes),
            Self::TempJournal => (
                limits.temp_journal_logical_bytes,
                limits.temp_journal_allocated_bytes,
            ),
            Self::Other => (
                limits.other_aux_aggregate_logical_bytes,
                limits.other_aux_aggregate_allocated_bytes,
            ),
        }
    }
}

#[derive(Debug, Default)]
struct OtherState {
    logical_current: u64,
    allocated_current: u64,
}

#[derive(Debug)]
struct OtherAggregate {
    logical_cap: u64,
    allocated_cap: u64,
    reservation: PinnedSqliteSpaceReservation,
    state: Mutex<OtherState>,
}

impl OtherAggregate {
    fn reserve_logical(&self, previous: u64, target: u64) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| budget_error("SQLite auxiliary aggregate ledger is poisoned"))?;
        let next = state
            .logical_current
            .saturating_sub(previous)
            .checked_add(target)
            .filter(|value| *value <= self.logical_cap)
            .ok_or_else(|| budget_error("SQLite aggregate auxiliary logical ceiling exceeded"))?;
        state.logical_current = next;
        Ok(())
    }

    fn update_inode(
        &self,
        previous_logical: u64,
        logical: u64,
        previous_allocated: u64,
        allocated: u64,
    ) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| budget_error("SQLite auxiliary aggregate ledger is poisoned"))?;
        let next_logical = state
            .logical_current
            .saturating_sub(previous_logical)
            .saturating_add(logical);
        let next_allocated = state
            .allocated_current
            .saturating_sub(previous_allocated)
            .saturating_add(allocated);
        state.logical_current = next_logical;
        state.allocated_current = next_allocated;
        let logical_ok = next_logical <= self.logical_cap;
        let allocated_ok = self
            .reservation
            .update_actual_allocated(next_allocated)
            .is_ok()
            && next_allocated <= self.allocated_cap;
        if logical_ok && allocated_ok {
            Ok(())
        } else {
            Err(budget_error("SQLite aggregate auxiliary ceiling exceeded"))
        }
    }

    fn remove_inode(&self, logical: u64, allocated: u64) {
        if let Ok(mut state) = self.state.lock() {
            state.logical_current = state.logical_current.saturating_sub(logical);
            state.allocated_current = state.allocated_current.saturating_sub(allocated);
            let _ = self
                .reservation
                .update_actual_allocated(state.allocated_current);
        }
    }
}

#[derive(Debug)]
struct AuxState {
    class: AuxClass,
    logical_cap: u64,
    allocated_cap: u64,
    reservation: Option<PinnedSqliteSpaceReservation>,
    other: Option<Arc<OtherAggregate>>,
    io_budget: PinnedSqliteIoBudget,
    space_budget: PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    live_aux: Arc<AtomicUsize>,
    logical_current: AtomicU64,
    allocated_current: AtomicU64,
    counted_live: bool,
    open_handles: AtomicUsize,
}

impl AuxState {
    fn check_running(&self) -> bool {
        if self.io_budget.snapshot().failure.is_some() {
            return false;
        }
        if Instant::now() >= self.deadline {
            self.io_budget.fail(PinnedSqliteIoFailure::Deadline);
            false
        } else if self.cancelled.load(Ordering::Acquire) {
            self.io_budget.fail(PinnedSqliteIoFailure::Cancelled);
            false
        } else {
            true
        }
    }

    fn check_space_health(&self) -> bool {
        let snapshot = self.space_budget.snapshot();
        if !snapshot.ledger_consistent || snapshot.allocation_anomalies != 0 {
            self.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
            false
        } else {
            true
        }
    }

    fn set_failure(&self, failure: PinnedSqliteIoFailure) {
        self.io_budget.fail(failure);
    }

    fn reserve_logical(&self, target: u64) -> bool {
        let previous = self.logical_current.load(Ordering::Acquire);
        if target > self.logical_cap {
            self.io_budget.fail_file_limit("logical_growth_bytes", target, self.logical_cap);
            return false;
        }
        if let Some(other) = &self.other {
            if other.reserve_logical(previous, target).is_err() {
                self.io_budget.fail_file_limit("other_member_target_vs_aggregate_bytes", target, other.logical_cap);
                return false;
            }
        }
        self.logical_current.store(target, Ordering::Release);
        true
    }

    fn observe(&self, file: &File) -> bool {
        let Ok(metadata) = file.metadata() else {
            self.set_failure(PinnedSqliteIoFailure::Io);
            return false;
        };
        let logical = metadata.len();
        let allocated = metadata.blocks().saturating_mul(512);
        let previous_logical = self.logical_current.load(Ordering::Acquire);
        let previous_allocated = self.allocated_current.load(Ordering::Acquire);
        let physical_result = if let Some(other) = &self.other {
            other.update_inode(previous_logical, logical, previous_allocated, allocated)
        } else {
            self.reservation.as_ref().map_or(Ok(()), |reservation| {
                reservation.update_actual_allocated(allocated)
            })
        };
        self.logical_current.store(logical, Ordering::Release);
        self.allocated_current.store(allocated, Ordering::Release);
        if logical > self.logical_cap {
            self.io_budget.fail_file_limit("observed_logical_bytes", logical, self.logical_cap);
            return false;
        }
        if physical_result.is_err() || allocated > self.allocated_cap {
            self.set_failure(PinnedSqliteIoFailure::SpaceLimit);
            false
        } else {
            true
        }
    }
}

impl Drop for AuxState {
    fn drop(&mut self) {
        if let Some(other) = &self.other {
            other.remove_inode(
                self.logical_current.load(Ordering::Acquire),
                self.allocated_current.load(Ordering::Acquire),
            );
        }
        if self.counted_live {
            self.live_aux.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

#[derive(Debug)]
struct AuxInode {
    file: File,
    state: Arc<AuxState>,
}

#[derive(Debug)]
struct AuxContext {
    id: u64,
    workspace: File,
    limits: PinnedSqliteAuxLimits,
    io_budget: PinnedSqliteIoBudget,
    space_budget: PinnedSqliteSpaceBudget,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    map: Mutex<HashMap<Vec<u8>, Arc<AuxInode>>>,
    main: Mutex<Option<Arc<AuxInode>>>,
    main_path: Mutex<Option<Vec<u8>>>,
    live_aux: Arc<AtomicUsize>,
    next_key: AtomicU64,
    other_aggregate: Mutex<Weak<OtherAggregate>>,
    closed: AtomicBool,
}

struct VfsAppData {
    context: Arc<AuxContext>,
    base: *mut ffi::sqlite3_vfs,
}
unsafe impl Send for VfsAppData {}
unsafe impl Sync for VfsAppData {}

impl AuxContext {
    fn new(
        id: u64,
        workspace: File,
        main: Arc<AuxInode>,
        request: &PinnedSqliteAuxRequest,
    ) -> Self {
        Self {
            id,
            workspace,
            limits: request.limits,
            io_budget: request.io_budget.clone(),
            space_budget: request.space_budget.clone(),
            deadline: request.deadline,
            cancelled: request.cancelled.clone(),
            map: Mutex::new(HashMap::new()),
            main: Mutex::new(Some(main)),
            main_path: Mutex::new(None),
            live_aux: Arc::new(AtomicUsize::new(0)),
            next_key: AtomicU64::new(1),
            other_aggregate: Mutex::new(Weak::new()),
            closed: AtomicBool::new(false),
        }
    }

    fn main_path(&self) -> Result<Option<Vec<u8>>> {
        self.main_path
            .lock()
            .map(|path| path.clone())
            .map_err(|_| budget_error("SQLite auxiliary main-path ledger is poisoned"))
    }

    /// SQLite's pager probes for an existing WAL even when DELETE/OFF mode is
    /// selected. Admit only this exact existence check as absent; it does not
    /// make the WAL pathname openable or creatable.
    fn probe_absent_main_wal(&self, key: &[u8]) -> Result<bool> {
        if self.closed.load(Ordering::Acquire) {
            return Err(budget_error("SQLite auxiliary scope is already closed"));
        }
        let Some(main_path) = self.main_path()? else {
            return Ok(false);
        };
        let mut wal_path = main_path.clone();
        wal_path.extend_from_slice(b"-wal");
        if key != wal_path.as_slice() {
            return Ok(false);
        }

        let main = self
            .main
            .lock()
            .map_err(|_| budget_error("SQLite auxiliary context is poisoned"))?
            .as_ref()
            .cloned()
            .ok_or_else(|| budget_error("SQLite auxiliary main inode is no longer live"))?;
        if main.state.class != AuxClass::Main
            || !main.state.check_running()
            || !main.state.check_space_health()
        {
            return Err(budget_error(
                "SQLite main-WAL absence probe failed its live-main guard",
            ));
        }
        let current = self
            .get(&main_path)?
            .ok_or_else(|| budget_error("SQLite auxiliary main mapping is no longer live"))?;
        if !Arc::ptr_eq(&main, &current) {
            return Err(budget_error("SQLite auxiliary main mapping changed"));
        }
        Ok(true)
    }

    fn set_main_path(&self, key: Vec<u8>) -> Result<()> {
        let main = self
            .main
            .lock()
            .map_err(|_| budget_error("SQLite auxiliary context is poisoned"))?
            .as_ref()
            .cloned()
            .ok_or_else(|| budget_error("SQLite auxiliary main inode already closed"))?;
        self.map
            .lock()
            .map_err(|_| budget_error("SQLite auxiliary context is poisoned"))?
            .insert(key.clone(), main);
        *self
            .main_path
            .lock()
            .map_err(|_| budget_error("SQLite auxiliary context is poisoned"))? = Some(key);
        Ok(())
    }

    fn key(&self) -> Vec<u8> {
        format!(
            "tosaux-{}-tmp-{}",
            self.id,
            self.next_key.fetch_add(1, Ordering::Relaxed)
        )
        .into_bytes()
    }

    fn get(&self, key: &[u8]) -> Result<Option<Arc<AuxInode>>> {
        self.map
            .lock()
            .map(|map| map.get(key).cloned())
            .map_err(|_| budget_error("SQLite auxiliary inode map is poisoned"))
    }

    fn remove(&self, key: &[u8]) -> Result<Option<Arc<AuxInode>>> {
        self.map
            .lock()
            .map(|mut map| map.remove(key))
            .map_err(|_| budget_error("SQLite auxiliary inode map is poisoned"))
    }

    fn other_ledger(&self) -> Result<Arc<OtherAggregate>> {
        let mut slot = self
            .other_aggregate
            .lock()
            .map_err(|_| budget_error("SQLite auxiliary context is poisoned"))?;
        if let Some(existing) = slot.upgrade() {
            return Ok(existing);
        }
        let reservation = self
            .space_budget
            .reserve(self.limits.other_aux_aggregate_allocated_bytes)?;
        let ledger = Arc::new(OtherAggregate {
            logical_cap: self.limits.other_aux_aggregate_logical_bytes,
            allocated_cap: self.limits.other_aux_aggregate_allocated_bytes,
            reservation,
            state: Mutex::new(OtherState::default()),
        });
        *slot = Arc::downgrade(&ledger);
        Ok(ledger)
    }

    fn create_inode(self: &Arc<Self>, class: AuxClass) -> Result<Arc<AuxInode>> {
        if Instant::now() >= self.deadline {
            self.io_budget.fail(PinnedSqliteIoFailure::Deadline);
            return Err(budget_error("SQLite auxiliary scope deadline exceeded"));
        }
        if self.cancelled.load(Ordering::Acquire) {
            self.io_budget.fail(PinnedSqliteIoFailure::Cancelled);
            return Err(budget_error("SQLite auxiliary scope cancelled"));
        }
        let (logical_cap, allocated_cap) = class.ceilings(self.limits);
        if logical_cap == 0 || allocated_cap == 0 {
            self.io_budget.fail_file_limit("disabled_file_class_bytes", 1, logical_cap.min(allocated_cap));
            return Err(budget_error("SQLite auxiliary file class is disabled"));
        }
        let counted_live = class != AuxClass::Main;
        if counted_live {
            let current = self.live_aux.fetch_add(1, Ordering::AcqRel);
            if current >= self.limits.max_live_aux {
                self.live_aux.fetch_sub(1, Ordering::AcqRel);
                self.io_budget.fail_file_limit("live_aux_count", current.saturating_add(1) as u64, self.limits.max_live_aux as u64);
                return Err(budget_error("SQLite live auxiliary inode limit exceeded"));
            }
        }
        let other = if class == AuxClass::Other {
            match self.other_ledger() {
                Ok(value) => Some(value),
                Err(error) => {
                    if counted_live {
                        self.live_aux.fetch_sub(1, Ordering::AcqRel);
                    }
                    self.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
                    return Err(error);
                }
            }
        } else {
            None
        };
        let reservation = if other.is_none() {
            match self.space_budget.reserve(allocated_cap) {
                Ok(value) => Some(value),
                Err(error) => {
                    if counted_live {
                        self.live_aux.fetch_sub(1, Ordering::AcqRel);
                    }
                    self.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
                    return Err(error);
                }
            }
        } else {
            None
        };
        let file = match create_unnamed(&self.workspace) {
            Ok(file) => file,
            Err(error) => {
                if counted_live {
                    self.live_aux.fetch_sub(1, Ordering::AcqRel);
                }
                self.io_budget.fail(PinnedSqliteIoFailure::Io);
                return Err(error);
            }
        };
        let metadata = match file.metadata() {
            Ok(value) => value,
            Err(_) => {
                if counted_live {
                    self.live_aux.fetch_sub(1, Ordering::AcqRel);
                }
                self.io_budget.fail(PinnedSqliteIoFailure::Io);
                return Err(invalid("SQLite auxiliary inode metadata"));
            }
        };
        if let Err(error) = validate_unnamed_file(&metadata) {
            if counted_live {
                self.live_aux.fetch_sub(1, Ordering::AcqRel);
            }
            self.io_budget.fail(PinnedSqliteIoFailure::Io);
            return Err(error);
        }
        let actual = metadata.blocks().saturating_mul(512);
        if let Some(reservation) = &reservation {
            if let Err(error) = reservation.update_actual_allocated(actual) {
                self.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
                if counted_live {
                    self.live_aux.fetch_sub(1, Ordering::AcqRel);
                }
                return Err(error);
            }
        }
        if let Some(other) = &other {
            if let Err(error) = other.update_inode(0, metadata.len(), 0, actual) {
                self.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
                // Keep the observed allocation charged until the inode itself
                // is gone, then remove that inode's aggregate contribution.
                drop(file);
                other.remove_inode(metadata.len(), actual);
                if counted_live {
                    self.live_aux.fetch_sub(1, Ordering::AcqRel);
                }
                return Err(error);
            }
        }
        let state = Arc::new(AuxState {
            class,
            logical_cap,
            allocated_cap,
            reservation,
            other,
            io_budget: self.io_budget.clone(),
            space_budget: self.space_budget.clone(),
            deadline: self.deadline,
            cancelled: self.cancelled.clone(),
            live_aux: self.live_aux.clone(),
            logical_current: AtomicU64::new(0),
            allocated_current: AtomicU64::new(actual),
            counted_live,
            open_handles: AtomicUsize::new(0),
        });
        Ok(Arc::new(AuxInode { file, state }))
    }

    fn virtual_key(&self, key: &[u8]) -> bool {
        let prefix = format!("tosaux-{}-tmp-", self.id);
        let Some(suffix) = key.strip_prefix(prefix.as_bytes()) else {
            return false;
        };
        !suffix.is_empty() && suffix.len() <= 20 && suffix.iter().all(u8::is_ascii_digit)
    }

    fn accepted_path(&self, key: &[u8]) -> Result<bool> {
        if self.virtual_key(key) {
            return Ok(self.get(key)?.is_some());
        }
        if let Some(main) = self.main_path()? {
            if key == main.as_slice() || key == [main.as_slice(), b"-journal"].concat() {
                return Ok(true);
            }
        }
        if self
            .get(key)?
            .is_some_and(|inode| inode.state.class == AuxClass::TempJournal)
        {
            return Ok(true);
        }
        if let Some(base) = key.strip_suffix(b"-journal") {
            return Ok(self.get(base)?.is_some_and(|inode| {
                inode.state.class == AuxClass::TempDb && self.virtual_key(base)
            }));
        }
        Ok(false)
    }
}

// Reuse the auxiliary ledger policy for a strict MAIN-only VFS. This holds a
// descriptor until before its reservation drops; it does not grant aux opens.
struct MainOnlyPolicy {
    _file: File,
    inner: AuxPolicy,
}
impl crate::pinned_sqlite::FdIoPolicy for MainOnlyPolicy {
    fn begin_read(&self, bytes: u64) -> bool {
        self.inner.begin_read(bytes)
    }
    fn record_read(&self, bytes: u64) -> bool {
        self.inner.record_read(bytes)
    }
    fn before_write(&self, file: &File, offset: u64, bytes: u64) -> bool {
        self.inner.before_write(file, offset, bytes)
    }
    fn record_write(&self, bytes: u64) -> bool {
        self.inner.record_write(bytes)
    }
    fn before_truncate(&self, file: &File, size: u64) -> bool {
        self.inner.before_truncate(file, size)
    }
    fn after_mutation(&self, file: &File) -> bool {
        self.inner.after_mutation(file)
    }
    fn check_operation(&self) -> bool {
        self.inner.check_operation()
    }
    fn failed_io(&self) {
        self.inner.failed_io();
    }
}

pub(super) fn strict_main_policy(
    file: &File,
    io_budget: PinnedSqliteIoBudget,
    space_budget: PinnedSqliteSpaceBudget,
    logical_cap: u64,
    allocated_cap: u64,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
) -> Result<Arc<dyn crate::pinned_sqlite::FdIoPolicy>> {
    if logical_cap == 0
        || logical_cap == u64::MAX
        || allocated_cap == 0
        || allocated_cap == u64::MAX
    {
        return Err(budget_error("strict SQLite main envelope is invalid"));
    }
    if cancelled.load(Ordering::Acquire) {
        io_budget.fail(PinnedSqliteIoFailure::Cancelled);
    } else if deadline <= Instant::now() {
        io_budget.fail(PinnedSqliteIoFailure::Deadline);
    }
    if io_budget.snapshot().failure.is_some() {
        return Err(budget_error("strict SQLite main request is stopped"));
    }
    let metadata = file
        .metadata()
        .map_err(|_| invalid("strict SQLite main metadata"))?;
    validate_unnamed_file(&metadata)?;
    if metadata.len() != 0 {
        return Err(invalid("strict SQLite main must be fresh"));
    }
    let reservation = space_budget.reserve(allocated_cap).map_err(|error| {
        io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
        error
    })?;
    let actual = metadata.blocks().checked_mul(512).ok_or_else(|| {
        io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
        budget_error("strict SQLite allocation overflow")
    })?;
    reservation
        .update_actual_allocated(actual)
        .map_err(|error| {
            io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
            error
        })?;
    let owned = file
        .try_clone()
        .map_err(|_| invalid("strict SQLite main descriptor clone"))?;
    let state = Arc::new(AuxState {
        class: AuxClass::Main,
        logical_cap,
        allocated_cap,
        reservation: Some(reservation),
        other: None,
        io_budget,
        space_budget,
        deadline,
        cancelled,
        live_aux: Arc::new(AtomicUsize::new(0)),
        logical_current: AtomicU64::new(0),
        allocated_current: AtomicU64::new(actual),
        counted_live: false,
        open_handles: AtomicUsize::new(0),
    });
    Ok(Arc::new(MainOnlyPolicy {
        _file: owned,
        inner: AuxPolicy {
            state,
            context: Weak::new(),
            key: Vec::new(),
            delete_on_close: false,
        },
    }))
}

/// Fixed allocations made by strict_main_policy, excluding the shared request
/// ledgers, SQLite/VFS allocations and allocator overhead. Arc headers are
/// charged conservatively alongside their concrete payloads.
pub(super) fn strict_main_declared_custody_bytes() -> Result<usize> {
    [
        std::mem::size_of::<MainOnlyPolicy>(),
        std::mem::size_of::<AuxState>(),
        std::mem::size_of::<AtomicUsize>(),
        6 * std::mem::size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
    .ok_or_else(|| budget_error("strict SQLite declared custody state overflow"))
}

#[derive(Debug)]
struct AuxPolicy {
    state: Arc<AuxState>,
    context: Weak<AuxContext>,
    key: Vec<u8>,
    delete_on_close: bool,
}

impl crate::pinned_sqlite::FdIoPolicy for AuxPolicy {
    fn begin_read(&self, bytes: u64) -> bool {
        self.state.io_budget.charge_read(bytes).is_ok() && self.state.check_running()
    }
    fn record_read(&self, bytes: u64) -> bool {
        self.state.io_budget.record_read_returned(bytes).is_ok() && self.state.check_running()
    }
    fn before_write(&self, file: &File, offset: u64, bytes: u64) -> bool {
        if self.state.io_budget.charge_write(bytes).is_err() {
            return false;
        }
        if !self.state.check_running()
            || !self.state.observe(file)
            || !self.state.check_space_health()
        {
            return false;
        }
        let Ok(metadata) = file.metadata() else {
            self.state.set_failure(PinnedSqliteIoFailure::Io);
            return false;
        };
        let Some(end) = offset.checked_add(bytes) else {
            self.state.set_failure(PinnedSqliteIoFailure::FileLimit);
            return false;
        };
        self.state.reserve_logical(metadata.len().max(end))
    }
    fn record_write(&self, bytes: u64) -> bool {
        self.state.io_budget.record_write_returned(bytes).is_ok()
    }
    fn before_truncate(&self, file: &File, size: u64) -> bool {
        let Ok(current) = file.metadata().map(|metadata| metadata.len()) else {
            self.state.set_failure(PinnedSqliteIoFailure::Io);
            return false;
        };
        if size > current && self.state.io_budget.charge_write(size - current).is_err() {
            return false;
        }
        if !self.state.check_running()
            || !self.state.observe(file)
            || !self.state.check_space_health()
        {
            return false;
        }
        if size > current {
            self.state.reserve_logical(size)
        } else {
            true
        }
    }
    fn after_mutation(&self, file: &File) -> bool {
        let observed = self.state.observe(file);
        let running = self.state.check_running();
        let space_healthy = self.state.check_space_health();
        observed && running && space_healthy
    }
    fn check_operation(&self) -> bool {
        self.state.check_running() && self.state.check_space_health()
    }
    fn failed_io(&self) {
        self.state.set_failure(PinnedSqliteIoFailure::Io);
    }
    fn file_closed(&self) {
        let previous = self.state.open_handles.fetch_sub(1, Ordering::AcqRel);
        if self.delete_on_close && previous == 1 {
            if let Some(context) = self.context.upgrade() {
                if context.remove(&self.key).is_err() {
                    self.state.set_failure(PinnedSqliteIoFailure::Io);
                }
            }
        }
    }
}

/// Scope-owned VFS registration. The SQLite VFS app-data Arc remains live
/// through a connection even when callers drop their scope first.
pub(super) struct AuxVfsLease {
    name: CString,
    vfs: *mut ffi::sqlite3_vfs,
    context: Arc<AuxContext>,
    closed: AtomicBool,
}

unsafe impl Send for AuxVfsLease {}
unsafe impl Sync for AuxVfsLease {}

impl std::fmt::Debug for AuxVfsLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuxVfsLease")
            .field("name", &self.name)
            .field("closed", &self.closed.load(Ordering::Acquire))
            .finish()
    }
}

impl AuxVfsLease {
    fn register(context: Arc<AuxContext>) -> Result<Arc<Self>> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let name = CString::new(format!("tos-pinned-aux-{id}"))
            .map_err(|_| invalid("SQLite auxiliary VFS name"))?;
        unsafe {
            let base = ffi::sqlite3_vfs_find(c"unix".as_ptr());
            if base.is_null() {
                return Err(invalid("SQLite unix VFS unavailable"));
            }
            let mut vfs = Box::new(*base);
            vfs.pNext = std::ptr::null_mut();
            vfs.zName = name.as_ptr();
            let app_data = Arc::into_raw(Arc::new(VfsAppData {
                context: context.clone(),
                base,
            }));
            vfs.pAppData = app_data as *mut c_void;
            vfs.szOsFile = crate::pinned_sqlite::sqlite_file_size() as i32;
            vfs.xFullPathname = Some(aux_full_path);
            vfs.xOpen = Some(aux_open);
            vfs.xDelete = Some(aux_delete);
            vfs.xAccess = Some(aux_access);
            vfs.xDlOpen = None;
            vfs.xDlError = None;
            vfs.xDlSym = None;
            vfs.xDlClose = None;
            vfs.xRandomness = Some(aux_randomness);
            vfs.xSleep = Some(aux_sleep);
            vfs.xCurrentTime = Some(aux_current_time);
            vfs.xGetLastError = Some(aux_last_error);
            vfs.xCurrentTimeInt64 = Some(aux_current_time_int64);
            vfs.xSetSystemCall = None;
            vfs.xGetSystemCall = None;
            vfs.xNextSystemCall = None;
            let pointer = Box::into_raw(vfs);
            let code = ffi::sqlite3_vfs_register(pointer, 0);
            if code != ffi::SQLITE_OK {
                drop(Arc::from_raw((*pointer).pAppData.cast::<VfsAppData>()));
                drop(Box::from_raw(pointer));
                return Err(invalid("SQLite auxiliary VFS registration failed"));
            }
            Ok(Arc::new(Self {
                name,
                vfs: pointer,
                context,
                closed: AtomicBool::new(false),
            }))
        }
    }

    pub(super) fn name(&self) -> &CStr {
        &self.name
    }

    pub(super) fn set_main_path(&self, path: Vec<u8>) -> Result<()> {
        self.context.set_main_path(path)
    }

    pub(super) fn mark_closed(&self) {
        self.closed.store(true, Ordering::Release);
        self.context.closed.store(true, Ordering::Release);
        if let Ok(mut map) = self.context.map.lock() {
            map.clear();
        } else {
            self.context.io_budget.fail(PinnedSqliteIoFailure::Io);
        }
        if let Ok(mut main) = self.context.main.lock() {
            main.take();
        } else {
            self.context.io_budget.fail(PinnedSqliteIoFailure::Io);
        }
        if let Ok(mut path) = self.context.main_path.lock() {
            path.take();
        } else {
            self.context.io_budget.fail(PinnedSqliteIoFailure::Io);
        }
    }
}

impl Drop for AuxVfsLease {
    fn drop(&mut self) {
        unsafe {
            let _ = ffi::sqlite3_vfs_unregister(self.vfs);
            let app_data = (*self.vfs).pAppData.cast::<VfsAppData>();
            if !app_data.is_null() {
                drop(Arc::from_raw(app_data));
            }
            drop(Box::from_raw(self.vfs));
        }
    }
}

static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);

/// One unnamed-inode SQLite scratch scope. Each scope permits one connection
/// attempt and one capped post-close extraction of the main database bytes.
pub struct PinnedSqliteAuxScope {
    _workspace: File,
    request: PinnedSqliteAuxRequest,
    context: Arc<AuxContext>,
    lease: Arc<AuxVfsLease>,
    main: Option<Arc<AuxInode>>,
    connection_attempted: bool,
    main_read_attempted: bool,
}

impl PinnedSqliteAuxScope {
    pub fn new(workspace_dir: File, request: PinnedSqliteAuxRequest) -> Result<Self> {
        request.validate()?;
        let metadata = workspace_dir
            .metadata()
            .map_err(|_| invalid("SQLite auxiliary workspace metadata"))?;
        if !metadata.is_dir()
            || metadata.mode() & 0o7777 != 0o700
            || metadata.uid() != crate::pinned_sqlite::current_fs_uid()?
        {
            return Err(invalid(
                "SQLite auxiliary workspace must be private and process-owned",
            ));
        }

        if Instant::now() >= request.deadline || request.cancelled.load(Ordering::Acquire) {
            request
                .io_budget
                .fail(if Instant::now() >= request.deadline {
                    PinnedSqliteIoFailure::Deadline
                } else {
                    PinnedSqliteIoFailure::Cancelled
                });
            return Err(budget_error(
                "SQLite auxiliary scope expired before inode creation",
            ));
        }
        let main_reservation = request
            .space_budget
            .reserve(request.limits.main_allocated_bytes)
            .map_err(|error| {
                request.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
                error
            })?;
        let main_file = create_unnamed(&workspace_dir).map_err(|error| {
            request.io_budget.fail(PinnedSqliteIoFailure::Io);
            error
        })?;
        let main_metadata = main_file
            .metadata()
            .map_err(|_| invalid("SQLite auxiliary main inode metadata"))?;
        validate_unnamed_file(&main_metadata)?;
        let actual = main_metadata.blocks().saturating_mul(512);
        if main_reservation.update_actual_allocated(actual).is_err() {
            request.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
            return Err(budget_error(
                "SQLite auxiliary main inode exceeded physical reservation",
            ));
        }
        let state = Arc::new(AuxState {
            class: AuxClass::Main,
            logical_cap: request.limits.main_logical_bytes,
            allocated_cap: request.limits.main_allocated_bytes,
            reservation: Some(main_reservation),
            other: None,
            io_budget: request.io_budget.clone(),
            space_budget: request.space_budget.clone(),
            deadline: request.deadline,
            cancelled: request.cancelled.clone(),
            live_aux: Arc::new(AtomicUsize::new(0)),
            logical_current: AtomicU64::new(0),
            allocated_current: AtomicU64::new(actual),
            counted_live: false,
            open_handles: AtomicUsize::new(0),
        });
        let main = Arc::new(AuxInode {
            file: main_file,
            state,
        });
        let id = NEXT_SCOPE.fetch_add(1, Ordering::Relaxed);
        let context = Arc::new(AuxContext::new(
            id,
            workspace_dir
                .try_clone()
                .map_err(|_| invalid("SQLite auxiliary workspace clone"))?,
            main.clone(),
            &request,
        ));
        let lease = AuxVfsLease::register(context.clone())?;
        Ok(Self {
            _workspace: workspace_dir,
            request,
            context,
            lease,
            main: Some(main),
            connection_attempted: false,
            main_read_attempted: false,
        })
    }

    pub fn open_connection(&mut self) -> Result<crate::PinnedSqliteConnection> {
        if self.connection_attempted || self.lease.closed.load(Ordering::Acquire) {
            return Err(budget_error(
                "SQLite auxiliary scope connection attempt already consumed",
            ));
        }
        self.connection_attempted = true;
        if !self.running()
            || !self.request.space_budget.snapshot().ledger_consistent
            || self.request.space_budget.snapshot().allocation_anomalies != 0
            || self.request.io_budget.snapshot().failure.is_some()
        {
            return Err(budget_error(
                "SQLite auxiliary scope expired, cancelled, or failed",
            ));
        }
        let main = self
            .main
            .as_ref()
            .ok_or_else(|| budget_error("SQLite auxiliary main inode already released"))?;
        match crate::pinned_sqlite::PinnedSqliteConnection::open_auxiliary(
            &main.file,
            self.lease.clone(),
        ) {
            Ok(connection) if self.request.io_budget.snapshot().failure.is_none() => Ok(connection),
            Ok(connection) => match connection.close() {
                Ok(()) => Err(budget_error(
                    "SQLite auxiliary connection open recorded a bounded failure",
                )),
                Err((connection, _)) => {
                    drop(connection);
                    Err(budget_error(
                        "SQLite auxiliary connection could not close after open failure",
                    ))
                }
            },
            Err(error) if self.request.io_budget.snapshot().failure.is_some() => Err(budget_error(
                "SQLite auxiliary connection open refused by its shared budget or deadline",
            )),
            Err(error) => Err(error),
        }
    }

    pub fn read_main_bytes(&mut self, max_bytes: u64) -> Result<Vec<u8>> {
        if self.main_read_attempted {
            return Err(budget_error(
                "SQLite auxiliary main extraction already attempted",
            ));
        }
        self.main_read_attempted = true;
        if !self.connection_attempted || !self.lease.closed.load(Ordering::Acquire) {
            self.main.take();
            return Err(budget_error(
                "SQLite auxiliary database must close successfully before extraction",
            ));
        }
        let Some(main) = self.main.take() else {
            return Err(budget_error("SQLite auxiliary main inode already released"));
        };
        let space_snapshot = self.request.space_budget.snapshot();
        if !self.running()
            || !main.state.check_space_health()
            || !space_snapshot.ledger_consistent
            || space_snapshot.allocation_anomalies != 0
            || self.request.io_budget.snapshot().failure.is_some()
        {
            return Err(budget_error(
                "SQLite auxiliary main extraction stopped by scope failure",
            ));
        }
        let metadata = main
            .file
            .metadata()
            .map_err(|_| invalid("SQLite auxiliary main inode metadata"))?;
        let len = metadata.len();
        if max_bytes == 0
            || len > max_bytes
            || len > main.state.logical_cap
            || len > usize::MAX as u64
        {
            self.request.io_budget.fail_file_limit("main_extraction_bytes", len, max_bytes.min(main.state.logical_cap));
            return Err(budget_error("SQLite auxiliary main extraction exceeds cap"));
        }
        let policy = AuxPolicy {
            state: main.state.clone(),
            context: Arc::downgrade(&self.context),
            key: Vec::new(),
            delete_on_close: false,
        };
        if !crate::pinned_sqlite::FdIoPolicy::check_operation(&policy)
            || (len > 0 && !crate::pinned_sqlite::FdIoPolicy::begin_read(&policy, len))
        {
            return Err(budget_error(
                "SQLite auxiliary main extraction budget refused",
            ));
        }
        let mut output = vec![0u8; len as usize];
        let mut at = 0usize;
        while at < output.len() {
            match main.file.read_at(&mut output[at..], at as u64) {
                Ok(0) => {
                    self.request.io_budget.fail(PinnedSqliteIoFailure::Io);
                    return Err(StoreError::new(
                        StoreErrorCode::CorruptSelectedObject,
                        "SQLite auxiliary main inode shortened during extraction",
                    ));
                }
                Ok(count) => {
                    at += count;
                    if !crate::pinned_sqlite::FdIoPolicy::record_read(&policy, count as u64) {
                        return Err(budget_error(
                            "SQLite auxiliary main extraction budget refused",
                        ));
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => {
                    self.request.io_budget.fail(PinnedSqliteIoFailure::Io);
                    return Err(StoreError::io(
                        "SQLite auxiliary main extraction failed",
                        error,
                    ));
                }
            }
        }
        let after = main
            .file
            .metadata()
            .map_err(|_| invalid("SQLite auxiliary main inode recheck"))?;
        if before_main_changed(&metadata, &after)
            || !main.state.observe(&main.file)
            || !crate::pinned_sqlite::FdIoPolicy::check_operation(&policy)
            || self.request.io_budget.snapshot().failure.is_some()
        {
            self.request.io_budget.fail(PinnedSqliteIoFailure::Io);
            return Err(budget_error(
                "SQLite auxiliary main inode changed or scope failed during extraction",
            ));
        }
        drop(main);
        let final_space = self.request.space_budget.snapshot();
        if !self.running()
            || !final_space.ledger_consistent
            || final_space.allocation_anomalies != 0
        {
            return Err(budget_error(
                "SQLite auxiliary main extraction missed its final scope gate",
            ));
        }
        if self.request.io_budget.snapshot().failure.is_some() {
            return Err(budget_error(
                "SQLite auxiliary main extraction ended with a bounded failure",
            ));
        }
        Ok(output)
    }

    fn running(&self) -> bool {
        if Instant::now() >= self.request.deadline {
            self.request.io_budget.fail(PinnedSqliteIoFailure::Deadline);
            false
        } else if self.request.cancelled.load(Ordering::Acquire) {
            self.request
                .io_budget
                .fail(PinnedSqliteIoFailure::Cancelled);
            false
        } else {
            true
        }
    }
}

const O_TMPFILE: i32 = (1 << 22) | (1 << 16);
const OPEN_READONLY: i32 = 0x0000_0001;
const OPEN_READWRITE: i32 = 0x0000_0002;
const OPEN_CREATE: i32 = 0x0000_0004;
const OPEN_DELETEONCLOSE: i32 = 0x0000_0008;
const OPEN_EXCLUSIVE: i32 = 0x0000_0010;
const OPEN_URI: i32 = 0x0000_0040;
const OPEN_MAIN_DB: i32 = 0x0000_0100;
const OPEN_TEMP_DB: i32 = 0x0000_0200;
const OPEN_MAIN_JOURNAL: i32 = 0x0000_0800;
const OPEN_TEMP_JOURNAL: i32 = 0x0000_1000;
const OPEN_SUBJOURNAL: i32 = 0x0000_2000;
const OPEN_TRANSIENT_DB: i32 = 0x0000_0400;
const OPEN_NOMUTEX: i32 = 0x0000_8000;
const OPEN_FULLMUTEX: i32 = 0x0001_0000;
const OPEN_SHAREDCACHE: i32 = 0x0002_0000;
const OPEN_PRIVATECACHE: i32 = 0x0004_0000;
const OPEN_NOFOLLOW: i32 = 0x0100_0000;
const OPEN_EXRESCODE: i32 = 0x0200_0000;
const OPEN_ALLOWED: i32 = OPEN_READONLY
    | OPEN_READWRITE
    | OPEN_CREATE
    | OPEN_DELETEONCLOSE
    | OPEN_EXCLUSIVE
    | OPEN_URI
    | OPEN_MAIN_DB
    | OPEN_TEMP_DB
    | OPEN_MAIN_JOURNAL
    | OPEN_TEMP_JOURNAL
    | OPEN_SUBJOURNAL
    | OPEN_TRANSIENT_DB
    | OPEN_NOMUTEX
    | OPEN_FULLMUTEX
    | OPEN_SHAREDCACHE
    | OPEN_PRIVATECACHE
    | OPEN_NOFOLLOW
    | OPEN_EXRESCODE;
const OPEN_TYPE_MASK: i32 = OPEN_MAIN_DB
    | OPEN_TEMP_DB
    | OPEN_MAIN_JOURNAL
    | OPEN_TEMP_JOURNAL
    | OPEN_SUBJOURNAL
    | OPEN_TRANSIENT_DB;

fn create_unnamed(directory: &File) -> Result<File> {
    let metadata = directory
        .metadata()
        .map_err(|_| invalid("SQLite auxiliary workspace recheck"))?;
    if !metadata.is_dir()
        || metadata.mode() & 0o7777 != 0o700
        || metadata.uid() != crate::pinned_sqlite::current_fs_uid()?
    {
        return Err(invalid("SQLite auxiliary workspace lost private ownership"));
    }
    let path = format!("/proc/self/fd/{}/.", directory.as_raw_fd());
    OpenOptions::new()
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(O_TMPFILE)
        .open(path)
        .map_err(|error| StoreError::io("cannot create unnamed SQLite auxiliary inode", error))
}

fn validate_unnamed_file(metadata: &std::fs::Metadata) -> Result<()> {
    if !metadata.is_file()
        || metadata.nlink() != 0
        || metadata.len() != 0
        || metadata.mode() & 0o7777 != 0o600
        || metadata.uid() != crate::pinned_sqlite::current_fs_uid()?
    {
        return Err(invalid(
            "SQLite auxiliary inode must be fresh, unnamed, and private",
        ));
    }
    Ok(())
}

fn before_main_changed(before: &std::fs::Metadata, after: &std::fs::Metadata) -> bool {
    before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.uid() != after.uid()
        || before.mode() != after.mode()
        || before.nlink() != after.nlink()
        || before.len() != after.len()
}

unsafe fn app_data_from_vfs(vfs: *mut ffi::sqlite3_vfs) -> Option<Arc<VfsAppData>> {
    if vfs.is_null() {
        return None;
    }
    let pointer = unsafe { (*vfs).pAppData.cast::<VfsAppData>() };
    if pointer.is_null() {
        return None;
    }
    unsafe {
        Arc::increment_strong_count(pointer);
    }
    Some(unsafe { Arc::from_raw(pointer) })
}

unsafe fn context_from_vfs(vfs: *mut ffi::sqlite3_vfs) -> Option<Arc<AuxContext>> {
    unsafe { app_data_from_vfs(vfs) }.map(|app_data| app_data.context.clone())
}

unsafe extern "C" fn aux_randomness(
    vfs: *mut ffi::sqlite3_vfs,
    bytes: i32,
    output: *mut std::ffi::c_char,
) -> i32 {
    let Some(app) = (unsafe { app_data_from_vfs(vfs) }) else {
        return 0;
    };
    let callback = unsafe { (*app.base).xRandomness };
    callback.map_or(0, |callback| unsafe { callback(app.base, bytes, output) })
}

unsafe extern "C" fn aux_sleep(vfs: *mut ffi::sqlite3_vfs, microseconds: i32) -> i32 {
    let Some(app) = (unsafe { app_data_from_vfs(vfs) }) else {
        return 0;
    };
    let callback = unsafe { (*app.base).xSleep };
    callback.map_or(0, |callback| unsafe { callback(app.base, microseconds) })
}

unsafe extern "C" fn aux_current_time(vfs: *mut ffi::sqlite3_vfs, output: *mut f64) -> i32 {
    let Some(app) = (unsafe { app_data_from_vfs(vfs) }) else {
        return ffi::SQLITE_ERROR;
    };
    let callback = unsafe { (*app.base).xCurrentTime };
    callback.map_or(ffi::SQLITE_ERROR, |callback| unsafe {
        callback(app.base, output)
    })
}

unsafe extern "C" fn aux_current_time_int64(vfs: *mut ffi::sqlite3_vfs, output: *mut i64) -> i32 {
    let Some(app) = (unsafe { app_data_from_vfs(vfs) }) else {
        return ffi::SQLITE_ERROR;
    };
    let callback = unsafe { (*app.base).xCurrentTimeInt64 };
    callback.map_or(ffi::SQLITE_ERROR, |callback| unsafe {
        callback(app.base, output)
    })
}

unsafe extern "C" fn aux_last_error(
    vfs: *mut ffi::sqlite3_vfs,
    bytes: i32,
    output: *mut std::ffi::c_char,
) -> i32 {
    let Some(app) = (unsafe { app_data_from_vfs(vfs) }) else {
        return 0;
    };
    let callback = unsafe { (*app.base).xGetLastError };
    callback.map_or(0, |callback| unsafe { callback(app.base, bytes, output) })
}

unsafe extern "C" fn aux_full_path(
    vfs: *mut ffi::sqlite3_vfs,
    name: *const std::ffi::c_char,
    count: i32,
    output: *mut std::ffi::c_char,
) -> i32 {
    let Some(context) = (unsafe { context_from_vfs(vfs) }) else {
        return ffi::SQLITE_CANTOPEN;
    };
    if name.is_null() || output.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    let raw = unsafe { CStr::from_ptr(name) }.to_bytes();
    match context.accepted_path(raw) {
        Ok(true) => {}
        Ok(false) => return ffi::SQLITE_CANTOPEN,
        Err(_) => return ffi::SQLITE_IOERR,
    }
    let Some(terminated_len) = raw.len().checked_add(1) else {
        return ffi::SQLITE_CANTOPEN;
    };
    if count <= 0 || terminated_len >= count as usize {
        return ffi::SQLITE_CANTOPEN;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(name.cast::<u8>(), output.cast::<u8>(), terminated_len);
    }
    ffi::SQLITE_OK
}

unsafe extern "C" fn aux_access(
    vfs: *mut ffi::sqlite3_vfs,
    name: *const std::ffi::c_char,
    flags: i32,
    output: *mut i32,
) -> i32 {
    let Some(context) = (unsafe { context_from_vfs(vfs) }) else {
        return ffi::SQLITE_IOERR_ACCESS;
    };
    if name.is_null() || output.is_null() {
        return ffi::SQLITE_IOERR_ACCESS;
    }
    let key = unsafe { CStr::from_ptr(name) }.to_bytes();
    match context.probe_absent_main_wal(key) {
        Ok(true) if flags == 0 => {
            // SQLITE_ACCESS_EXISTS is zero. WAL open/fullpath/create remain
            // rejected by their separate, stricter pathname gates.
            unsafe {
                *output = 0;
            }
            return ffi::SQLITE_OK;
        }
        Ok(true) => return ffi::SQLITE_CANTOPEN,
        Ok(false) => {}
        Err(_) => return ffi::SQLITE_IOERR_ACCESS,
    }
    match context.accepted_path(key) {
        Ok(true) => {}
        Ok(false) => return ffi::SQLITE_CANTOPEN,
        Err(_) => return ffi::SQLITE_IOERR_ACCESS,
    }
    let exists = match context.get(key) {
        Ok(value) => value.is_some(),
        Err(_) => return ffi::SQLITE_IOERR_ACCESS,
    };
    unsafe {
        *output = i32::from(exists);
    }
    ffi::SQLITE_OK
}

unsafe extern "C" fn aux_delete(
    vfs: *mut ffi::sqlite3_vfs,
    name: *const std::ffi::c_char,
    _sync_dir: i32,
) -> i32 {
    let Some(context) = (unsafe { context_from_vfs(vfs) }) else {
        return ffi::SQLITE_IOERR_DELETE;
    };
    if name.is_null() {
        return ffi::SQLITE_IOERR_DELETE;
    }
    let key = unsafe { CStr::from_ptr(name) }.to_bytes();
    let main_path = match context.main_path() {
        Ok(Some(path)) => path,
        Ok(None) | Err(_) => {
            context.io_budget.fail(PinnedSqliteIoFailure::Io);
            return ffi::SQLITE_IOERR_DELETE;
        }
    };
    if key == main_path {
        return ffi::SQLITE_IOERR_DELETE;
    }
    match context.accepted_path(key) {
        Ok(true) => {}
        Ok(false) => return ffi::SQLITE_IOERR_DELETE,
        Err(_) => {
            context.io_budget.fail(PinnedSqliteIoFailure::Io);
            return ffi::SQLITE_IOERR_DELETE;
        }
    }
    match context.get(key) {
        Err(_) => {
            context.io_budget.fail(PinnedSqliteIoFailure::Io);
            return ffi::SQLITE_IOERR_DELETE;
        }
        Ok(Some(inode)) => {
            if inode.state.class == AuxClass::Main {
                return ffi::SQLITE_IOERR_DELETE;
            }
            if context.remove(key).is_err() {
                context.io_budget.fail(PinnedSqliteIoFailure::Io);
                return ffi::SQLITE_IOERR_DELETE;
            }
        }
        Ok(None) => {}
    }
    ffi::SQLITE_OK
}

unsafe extern "C" fn aux_open(
    vfs: *mut ffi::sqlite3_vfs,
    name: *const std::ffi::c_char,
    file: *mut ffi::sqlite3_file,
    flags: i32,
    output_flags: *mut i32,
) -> i32 {
    let Some(context) = (unsafe { context_from_vfs(vfs) }) else {
        return ffi::SQLITE_CANTOPEN;
    };
    if file.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    unsafe {
        (*file).pMethods = std::ptr::null();
    }
    if context.closed.load(Ordering::Acquire) || flags & !OPEN_ALLOWED != 0 {
        return ffi::SQLITE_CANTOPEN;
    }
    let type_flags = flags & OPEN_TYPE_MASK;
    let readonly = flags & OPEN_READONLY != 0;
    let readwrite = flags & OPEN_READWRITE != 0;
    if type_flags.count_ones() != 1
        || readonly == readwrite
        || flags & OPEN_FULLMUTEX != 0 && flags & OPEN_NOMUTEX != 0
    {
        return ffi::SQLITE_CANTOPEN;
    }
    if readonly && type_flags != OPEN_MAIN_JOURNAL {
        return ffi::SQLITE_CANTOPEN;
    }
    if readonly && flags & (OPEN_CREATE | OPEN_DELETEONCLOSE | OPEN_EXCLUSIVE) != 0 {
        return ffi::SQLITE_CANTOPEN;
    }
    let raw_name = if name.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(name) }.to_bytes().to_vec())
    };
    let main_path = match context.main_path() {
        Ok(Some(path)) => path,
        Ok(None) => return ffi::SQLITE_CANTOPEN,
        Err(_) => {
            context.io_budget.fail(PinnedSqliteIoFailure::Io);
            return ffi::SQLITE_IOERR;
        }
    };
    let (class, key, unnamed) = match type_flags {
        OPEN_MAIN_DB => {
            if raw_name.as_deref() != Some(main_path.as_slice())
                || flags & OPEN_CREATE != 0
                || readonly
            {
                return ffi::SQLITE_CANTOPEN;
            }
            (AuxClass::Main, main_path.clone(), false)
        }
        OPEN_MAIN_JOURNAL => {
            let mut expected = main_path.clone();
            expected.extend_from_slice(b"-journal");
            if raw_name.as_deref() != Some(expected.as_slice()) {
                return ffi::SQLITE_CANTOPEN;
            }
            (AuxClass::MainJournal, expected, false)
        }
        OPEN_TEMP_DB => match raw_name {
            Some(key) if context.virtual_key(&key) => (AuxClass::TempDb, key, false),
            Some(_) => return ffi::SQLITE_CANTOPEN,
            None => (AuxClass::TempDb, context.key(), true),
        },
        OPEN_TEMP_JOURNAL => match raw_name {
            None => (AuxClass::TempJournal, context.key(), true),
            Some(key) => {
                let Some(base) = key.strip_suffix(b"-journal") else {
                    return ffi::SQLITE_CANTOPEN;
                };
                let parent = match context.get(base) {
                    Ok(Some(parent)) => parent,
                    Ok(None) => return ffi::SQLITE_CANTOPEN,
                    Err(_) => {
                        context.io_budget.fail(PinnedSqliteIoFailure::Io);
                        return ffi::SQLITE_IOERR;
                    }
                };
                if parent.state.class != AuxClass::TempDb || !context.virtual_key(base) {
                    return ffi::SQLITE_CANTOPEN;
                }
                (AuxClass::TempJournal, key, false)
            }
        },
        OPEN_SUBJOURNAL => match raw_name {
            None => (AuxClass::Other, context.key(), true),
            Some(key) if context.virtual_key(&key) => (AuxClass::Other, key, false),
            Some(_) => return ffi::SQLITE_CANTOPEN,
        },
        OPEN_TRANSIENT_DB => match raw_name {
            None => (AuxClass::Other, context.key(), true),
            Some(key) if context.virtual_key(&key) => (AuxClass::Other, key, false),
            Some(_) => return ffi::SQLITE_CANTOPEN,
        },
        _ => return ffi::SQLITE_CANTOPEN,
    };
    let inode = if readonly {
        let existing = match context.get(&key) {
            Ok(Some(existing)) => existing,
            Ok(None) => return ffi::SQLITE_CANTOPEN,
            Err(_) => {
                context.io_budget.fail(PinnedSqliteIoFailure::Io);
                return ffi::SQLITE_IOERR;
            }
        };
        if existing.state.class != class {
            return ffi::SQLITE_CANTOPEN;
        }
        existing
    } else if unnamed {
        match context.create_inode(class) {
            Ok(inode) => inode,
            Err(_) => {
                context.io_budget.fail(PinnedSqliteIoFailure::SpaceLimit);
                return ffi::SQLITE_FULL;
            }
        }
    } else if let Some(existing) = match context.get(&key) {
        Ok(existing) => existing,
        Err(_) => {
            context.io_budget.fail(PinnedSqliteIoFailure::Io);
            return ffi::SQLITE_IOERR;
        }
    } {
        if existing.state.class != class {
            return ffi::SQLITE_CANTOPEN;
        }
        existing
    } else {
        if flags & OPEN_CREATE == 0 {
            return ffi::SQLITE_CANTOPEN;
        }
        let created = match context.create_inode(class) {
            Ok(inode) => inode,
            Err(_) => {
                return ffi::SQLITE_FULL;
            }
        };
        match context.map.lock() {
            Ok(mut map) => map
                .entry(key.clone())
                .or_insert_with(|| created.clone())
                .clone(),
            Err(_) => {
                context.io_budget.fail(PinnedSqliteIoFailure::Io);
                return ffi::SQLITE_IOERR;
            }
        }
    };
    if !inode.state.check_running() || !inode.state.check_space_health() {
        return ffi::SQLITE_FULL;
    }
    let expected = match inode.file.metadata() {
        Ok(value) => value,
        Err(_) => return ffi::SQLITE_CANTOPEN,
    };
    let path = format!("/proc/self/fd/{}", inode.file.as_raw_fd());
    let actual = match OpenOptions::new().read(true).write(!readonly).open(path) {
        Ok(file) => file,
        Err(_) => {
            context.io_budget.fail(PinnedSqliteIoFailure::Io);
            return ffi::SQLITE_CANTOPEN;
        }
    };
    let observed = match actual.metadata() {
        Ok(value) => value,
        Err(_) => return ffi::SQLITE_CANTOPEN,
    };
    if !observed.is_file()
        || expected.dev() != observed.dev()
        || expected.ino() != observed.ino()
        || expected.uid() != observed.uid()
        || expected.mode() != observed.mode()
        || expected.nlink() != observed.nlink()
    {
        return ffi::SQLITE_CANTOPEN;
    }
    let delete_on_close = flags & OPEN_DELETEONCLOSE != 0 || unnamed;
    let policy: Arc<dyn crate::pinned_sqlite::FdIoPolicy> = Arc::new(AuxPolicy {
        state: inode.state.clone(),
        context: Arc::downgrade(&context),
        key: key.clone(),
        delete_on_close,
    });
    inode.state.open_handles.fetch_add(1, Ordering::AcqRel);
    let installed =
        unsafe { crate::pinned_sqlite::install_fd_file(file, actual, readonly, Some(policy)) };
    if installed != ffi::SQLITE_OK {
        inode.state.open_handles.fetch_sub(1, Ordering::AcqRel);
        return installed;
    }
    if !output_flags.is_null() {
        unsafe {
            *output_flags = flags;
        }
    }
    ffi::SQLITE_OK
}

fn budget_error(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::BudgetExceeded, detail)
}

fn invalid(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::DescriptorMismatch, detail)
}

#[cfg(test)]
mod io_restriction_tests {
    use super::*;

    #[test]
    fn shared_write_pool_precharges_two_local_producers_and_preserves_actual_returns() {
        let root = PinnedSqliteIoBudget::new(1, 10).unwrap();
        let a =
            PinnedSqliteIoBudget::new_with_shared_write_authority(20, 10, root.clone()).unwrap();
        let b =
            PinnedSqliteIoBudget::new_with_shared_write_authority(30, 10, root.clone()).unwrap();
        assert!(!a.shares_with(&b));
        assert!(a.shares_write_authority_with(&b));
        assert!(a.shares_write_authority_with(&root));
        a.charge_write(6).unwrap();
        b.charge_write(4).unwrap();
        assert!(b.charge_write(1).is_err());
        assert_eq!(
            a.snapshot().failure,
            Some(PinnedSqliteIoFailure::WriteLimit)
        );
        assert_eq!(b.snapshot().write_permitted_bytes, 4);
        // Both real transfers were permitted before the refused attempt.
        a.record_write_returned(6).unwrap();
        b.record_write_returned(4).unwrap();
        let whole = root.snapshot();
        assert_eq!(whole.write_attempted_bytes, 11);
        assert_eq!(whole.write_permitted_bytes, 10);
        assert_eq!(whole.write_returned_bytes, 10);
        assert_eq!(whole.read_attempted_bytes, 0);
        assert_eq!(a.shared_write_snapshot(), whole);
        assert_eq!(
            a.snapshot().write_returned_bytes + b.snapshot().write_returned_bytes,
            10
        );
        assert!(a.charge_write(1).is_err());
        assert_eq!(root.snapshot().write_permitted_bytes, 10);
        assert_eq!(root.snapshot().write_attempted_bytes, 12);
    }

    #[test]
    fn shared_write_children_restrict_locally_without_narrowing_peer_reads_or_writes() {
        let root = PinnedSqliteIoBudget::new(1, 20).unwrap();
        let a =
            PinnedSqliteIoBudget::new_with_shared_write_authority(10, 20, root.clone()).unwrap();
        let b =
            PinnedSqliteIoBudget::new_with_shared_write_authority(10, 20, root.clone()).unwrap();
        a.restrict_remaining_io(0, 0).unwrap();
        b.charge_read(10).unwrap();
        b.record_read_returned(10).unwrap();
        b.charge_write(20).unwrap();
        b.record_write_returned(20).unwrap();
        assert_eq!(a.snapshot().read_attempted_bytes, 0);
        assert_eq!(a.snapshot().write_attempted_bytes, 0);
        assert_eq!(b.snapshot().read_returned_bytes, 10);
        assert_eq!(root.snapshot().read_attempted_bytes, 0);
        assert_eq!(root.snapshot().write_returned_bytes, 20);
    }

    #[test]
    fn local_write_refusal_never_grants_aggregate_permits_or_borrows_peer_returns() {
        let root = PinnedSqliteIoBudget::new(1, 20).unwrap();
        let a = PinnedSqliteIoBudget::new_with_shared_write_authority(10, 3, root.clone()).unwrap();
        let b =
            PinnedSqliteIoBudget::new_with_shared_write_authority(10, 20, root.clone()).unwrap();
        b.charge_write(5).unwrap();
        assert!(a.charge_write(4).is_err());
        assert_eq!(root.snapshot().write_attempted_bytes, 9);
        assert_eq!(root.snapshot().write_permitted_bytes, 5);
        assert_eq!(a.snapshot().write_permitted_bytes, 0);
        assert!(a.record_write_returned(1).is_err());
        assert_eq!(root.snapshot().write_returned_bytes, 0);
        b.record_write_returned(5).unwrap();
        assert_eq!(root.snapshot().write_returned_bytes, 5);
        assert!(b.charge_write(1).is_err());
    }

    #[test]
    fn shared_write_authority_rejects_children_as_roots_and_distinguishes_equal_pools() {
        let root = PinnedSqliteIoBudget::new(1, 10).unwrap();
        let other_root = PinnedSqliteIoBudget::new(1, 10).unwrap();
        let child = PinnedSqliteIoBudget::new_with_shared_write_authority(10, 10, root).unwrap();
        assert!(!child.shares_write_authority_with(&other_root));
        assert!(PinnedSqliteIoBudget::new_with_shared_write_authority(10, 10, child).is_err());
        assert!(
            PinnedSqliteIoBudget::new_with_shared_write_authority(
                10,
                10,
                PinnedSqliteIoBudget::new_read_only(10).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn guard_upper_shares_ceiling_without_becoming_payload() {
        let io = PinnedSqliteIoBudget::new(10, 10).unwrap();
        let clone = io.clone();
        clone.charge_read_upper_bound(3).unwrap();
        io.charge_read(7).unwrap();
        io.record_read_returned(7).unwrap();
        assert!(clone.charge_read_upper_bound(1).is_err());
        let seen = io.snapshot();
        assert_eq!(seen.read_attempted_bytes, 11);
        assert_eq!(seen.read_upper_bound_attempted_bytes, 4);
        assert_eq!(seen.read_permitted_bytes, 10);
        assert_eq!(seen.read_returned_bytes, 7);
        assert_eq!(seen.failure, Some(PinnedSqliteIoFailure::ReadLimit));

        let only_guard = PinnedSqliteIoBudget::new(10, 10).unwrap();
        only_guard.charge_read_upper_bound(3).unwrap();
        assert!(only_guard.record_read_returned(1).is_err());
        assert_eq!(only_guard.snapshot().read_returned_bytes, 0);
    }

    #[test]
    fn narrowing_keeps_identity_counters_and_inflight_permits() {
        let io = PinnedSqliteIoBudget::new(100, 100).unwrap();
        let clone = io.clone();
        io.charge_read(7).unwrap();
        io.charge_write(11).unwrap();
        let before = io.snapshot();
        clone.restrict_remaining_io(0, 0).unwrap();
        assert!(clone.shares_with(&io));
        assert_eq!(io.snapshot(), before);
        io.record_read_returned(7).unwrap();
        io.record_write_returned(11).unwrap();
        // A larger later slice cannot restore either exhausted ceiling.
        clone.restrict_remaining_io(100, 100).unwrap();
        assert!(io.charge_read(1).is_err());
        assert_eq!(io.snapshot().read_permitted_bytes, 7);
        assert_eq!(io.snapshot().read_returned_bytes, 7);
        assert_eq!(
            io.snapshot().failure,
            Some(PinnedSqliteIoFailure::ReadLimit)
        );
        assert!(clone.restrict_remaining_io(100, 100).is_err());
    }

    #[test]
    fn narrowing_write_limit_is_shared_and_invalid_pair_does_not_partially_apply() {
        let io = PinnedSqliteIoBudget::new(10, 20).unwrap();
        io.charge_read(1).unwrap();
        io.charge_write(2).unwrap();
        assert!(io.restrict_remaining_io(0, u64::MAX - 1).is_err());
        // Overflow in the second slice must leave the first ceiling intact.
        io.charge_read(9).unwrap();
        io.restrict_remaining_io(0, 3).unwrap();
        let clone = io.clone();
        clone.charge_write(3).unwrap();
        assert!(io.charge_write(1).is_err());
        assert_eq!(clone.snapshot().write_permitted_bytes, 5);
        assert_eq!(
            clone.snapshot().failure,
            Some(PinnedSqliteIoFailure::WriteLimit)
        );
    }

    #[test]
    fn read_only_ledger_denies_every_nonzero_write_without_a_synthetic_write_cap() {
        let io = PinnedSqliteIoBudget::new_read_only(19).unwrap();
        assert!(io.shares_with(&io.clone()));
        io.charge_read_upper_bound(4).unwrap();
        io.charge_read(15).unwrap();
        io.record_read_returned(15).unwrap();
        assert!(io.charge_write(1).is_err());
        let seen = io.snapshot();
        assert_eq!(seen.read_upper_bound_attempted_bytes, 4);
        assert_eq!(seen.read_returned_bytes, 15);
        assert_eq!(seen.write_attempted_bytes, 1);
        assert_eq!(seen.write_permitted_bytes, 0);
        assert_eq!(seen.failure, Some(PinnedSqliteIoFailure::WriteLimit));
        assert!(PinnedSqliteIoBudget::new_read_only(0).is_err());
    }
}
