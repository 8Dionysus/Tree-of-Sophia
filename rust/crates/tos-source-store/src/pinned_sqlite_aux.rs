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
    pub read_attempted_bytes: u64,
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
}

#[derive(Debug)]
struct IoState {
    max_read: u64,
    max_write: u64,
    read_attempted: AtomicU64,
    read_permitted: AtomicU64,
    read_returned: AtomicU64,
    write_attempted: AtomicU64,
    write_permitted: AtomicU64,
    write_returned: AtomicU64,
    failure: AtomicU8,
}

/// Cloneable cumulative logical I/O authority shared by every participating
/// source reader, SQLite pager and caller-owned output step.
#[derive(Clone, Debug)]
pub struct PinnedSqliteIoBudget(Arc<IoState>);

impl PinnedSqliteIoBudget {
    pub fn new(max_read_bytes: u64, max_write_bytes: u64) -> Result<Self> {
        if max_read_bytes == 0
            || max_write_bytes == 0
            || max_read_bytes == u64::MAX
            || max_write_bytes == u64::MAX
        {
            return Err(budget_error("SQLite logical I/O limits must be nonzero"));
        }
        Ok(Self(Arc::new(IoState {
            max_read: max_read_bytes,
            max_write: max_write_bytes,
            read_attempted: AtomicU64::new(0),
            read_permitted: AtomicU64::new(0),
            read_returned: AtomicU64::new(0),
            write_attempted: AtomicU64::new(0),
            write_permitted: AtomicU64::new(0),
            write_returned: AtomicU64::new(0),
            failure: AtomicU8::new(0),
        })))
    }

    pub fn charge_read(&self, bytes: u64) -> Result<()> {
        if self.0.failure.load(Ordering::Acquire) != 0 {
            saturating_add(&self.0.read_attempted, bytes);
            return Err(budget_error(
                "SQLite logical I/O ledger has a prior failure",
            ));
        }
        charge(
            &self.0.read_attempted,
            &self.0.read_permitted,
            self.0.max_read,
            bytes,
        )
        .map_err(|_| {
            self.fail(PinnedSqliteIoFailure::ReadLimit);
            budget_error("SQLite cumulative read budget exceeded")
        })
    }

    pub fn charge_write(&self, bytes: u64) -> Result<()> {
        if self.0.failure.load(Ordering::Acquire) != 0 {
            saturating_add(&self.0.write_attempted, bytes);
            return Err(budget_error(
                "SQLite logical I/O ledger has a prior failure",
            ));
        }
        charge(
            &self.0.write_attempted,
            &self.0.write_permitted,
            self.0.max_write,
            bytes,
        )
        .map_err(|_| {
            self.fail(PinnedSqliteIoFailure::WriteLimit);
            budget_error("SQLite cumulative write budget exceeded")
        })
    }

    pub fn record_read_returned(&self, bytes: u64) -> Result<()> {
        record_returned(&self.0.read_returned, &self.0.read_permitted, bytes).map_err(|_| {
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite read return exceeded permitted bytes")
        })
    }

    pub fn record_write_returned(&self, bytes: u64) -> Result<()> {
        record_returned(&self.0.write_returned, &self.0.write_permitted, bytes).map_err(|_| {
            self.fail(PinnedSqliteIoFailure::Io);
            budget_error("SQLite write return exceeded permitted bytes")
        })
    }

    pub fn snapshot(&self) -> PinnedSqliteIoSnapshot {
        PinnedSqliteIoSnapshot {
            read_attempted_bytes: self.0.read_attempted.load(Ordering::Acquire),
            read_permitted_bytes: self.0.read_permitted.load(Ordering::Acquire),
            read_returned_bytes: self.0.read_returned.load(Ordering::Acquire),
            write_attempted_bytes: self.0.write_attempted.load(Ordering::Acquire),
            write_permitted_bytes: self.0.write_permitted.load(Ordering::Acquire),
            write_returned_bytes: self.0.write_returned.load(Ordering::Acquire),
            failure: decode_failure(self.0.failure.load(Ordering::Acquire)),
        }
    }

    pub(super) fn fail(&self, reason: PinnedSqliteIoFailure) {
        let _ =
            self.0
                .failure
                .compare_exchange(0, reason as u8, Ordering::AcqRel, Ordering::Acquire);
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
    let allowed = permitted.load(Ordering::Acquire);
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
            self.set_failure(PinnedSqliteIoFailure::FileLimit);
            return false;
        }
        if let Some(other) = &self.other {
            if other.reserve_logical(previous, target).is_err() {
                self.set_failure(PinnedSqliteIoFailure::FileLimit);
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
            self.set_failure(PinnedSqliteIoFailure::FileLimit);
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
            self.io_budget.fail(PinnedSqliteIoFailure::FileLimit);
            return Err(budget_error("SQLite auxiliary file class is disabled"));
        }
        let counted_live = class != AuxClass::Main;
        if counted_live {
            let current = self.live_aux.fetch_add(1, Ordering::AcqRel);
            if current >= self.limits.max_live_aux {
                self.live_aux.fetch_sub(1, Ordering::AcqRel);
                self.io_budget.fail(PinnedSqliteIoFailure::FileLimit);
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
            self.request
                .io_budget
                .fail(PinnedSqliteIoFailure::FileLimit);
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
