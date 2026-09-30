//! Operation-owned, fail-closed isolated JSON Schema worker boundary.
//!
//! This module does not create a validation trace or admission attestation.
//! The caller must pin the dedicated worker's exact ELF digest. Linux copies
//! that ELF to a sealed executable memfd before launching it; the selected-cut
//! adapter retains that immutable image within one bounded operation, so a path change
//! after verification cannot change the worker image. One bounded child retains
//! only its exact selected closure/profile and finite selector cache until explicit
//! finalization; uncertain exchanges poison it permanently.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tos_foundation::Digest256;

use crate::{FormatProfile, SchemaResource};

const MAX_URI_BYTES: usize = 4096;
const MAX_FRAME_BYTES: usize = 36 * 1024 * 1024;
const BATCH_REQUEST_MAGIC: &[u8; 8] = b"TOSV2BQ1";
const BATCH_ACK_MAGIC: &[u8; 8] = b"TOSV2BA1";
const BATCH_UNIT_MAGIC: &[u8; 8] = b"TOSV2BU1";
const BATCH_ACK_BYTES: usize = 8 + 32 + 32 + 4;
const BATCH_UNIT_BYTES: usize = 8 + 8 + 32 + 2;
const MAX_BATCH_UNITS: usize = 64;
const MAX_BATCH_RAW_BYTES: usize = 32 * 1024 * 1024;
const MAX_BATCH_FRAME_BYTES: usize = 68 * 1024 * 1024;
const OPERATION_REQUEST_MAGIC: &[u8; 8] = b"TOSV2OP1";
const OPERATION_END_MAGIC: &[u8; 8] = b"TOSV2OE1";
const OPERATION_END_BYTES: usize = 8 + 32 + 32 + 4;
const OPERATION_CLOSE_MAGIC: &[u8; 8] = b"TOSV2OC1";
const OPERATION_FINAL_MAGIC: &[u8; 8] = b"TOSV2OF1";
const OPERATION_FINAL_BYTES: usize = 8 + 32 + 32 + 8;
// nonce, selected schema digest/profile, finite aggregate envelope.
const OPERATION_HEADER_BYTES: usize = 8 + 16 + 32 + 1 + 8 * 8;
const MAX_MEMBER_ID_BYTES: usize = 512;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone)]
pub struct ExactWorkerIdentity {
    pub absolute_path: PathBuf,
    pub sha256: Digest256,
}

/// One exact sealed image for related adapters within a single bounded caller
/// operation. It carries no schema receipt, child state, or global cache entry.
/// Only successful image verification can construct this handle.
pub struct VerifiedWorkerImageHandle {
    identity: ExactWorkerIdentity,
    operation_deadline: Instant,
    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    file: std::fs::File,
}

impl VerifiedWorkerImageHandle {
    pub fn prepare(
        worker: ExactWorkerIdentity,
        budget: ExecutorBudget,
        operation_deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::VerifiedWorkerImage::prepare(&worker, budget, operation_deadline, cancelled)
                .map(native::VerifiedWorkerImage::into_handle)
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (worker, budget, operation_deadline, cancelled);
            Err(ExecutorFailure::UnsupportedHost)
        }
    }

    pub fn identity(&self) -> &ExactWorkerIdentity {
        &self.identity
    }

    pub(crate) fn operation_deadline(&self) -> Instant {
        self.operation_deadline
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutorBudget {
    /// Deadline for image verification, fork, transfer and worker execution.
    pub execution_wall: Duration,
    /// Additional, caller-visible allowance for SIGKILL and WNOHANG reap.
    pub cleanup_grace: Duration,
    pub cpu_seconds: u64,
    pub address_space_bytes: u64,
}

impl ExecutorBudget {
    pub fn laboratory() -> Self {
        Self {
            execution_wall: Duration::from_secs(5),
            cleanup_grace: Duration::from_millis(200),
            cpu_seconds: 3,
            address_space_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionIdentity {
    pub worker_sha256: Digest256,
    pub request_sha256: Digest256,
    pub schema_set_sha256: Digest256,
    pub instance_sha256: Digest256,
    pub profile: FormatProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorFailure {
    UnsupportedHost,
    WorkerIdentity,
    InputBudget,
    ResourceLimitUnknown,
    Spawn,
    Timeout,
    Cancelled,
    CpuLimit,
    CrashSignal(i32),
    CrashExit(i32),
    ReapPending(i32),
    Protocol,
    Backend,
    ParseRejected,
    CoverageMismatch,
}

/// A child status observed before any parent cleanup signal. This is not an
/// inference from elapsed time or a status caused by parent-directed SIGKILL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildTermination {
    Exited(i32),
    Signalled(i32),
}

/// Context for the actual failed exchange guard; no acceptance is carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExchangeFailureContext {
    pub boundary: &'static str,
    pub failure: ExecutorFailure,
    pub natural_termination: Option<ChildTermination>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorOutcome {
    SchemaValid(ExecutionIdentity),
    SchemaInvalid(ExecutionIdentity),
    InputRejected(ExecutionIdentity),
    Indeterminate {
        reason: ExecutorFailure,
        identity: Option<ExecutionIdentity>,
        exchange: Option<ExchangeFailureContext>,
    },
}

/// A bounded transport unit. The path is an exact identity label only; the
/// worker never opens it. Source membership and ownership remain external.
#[derive(Debug, Clone)]
pub struct BatchUnit {
    pub ordinal: u64,
    pub member_id: String,
    pub relative_path: String,
    pub root_uri: String,
    pub raw_instance: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct BatchCoverageExpectation {
    pub count: u64,
    pub ordered_manifest_sha256: Digest256,
}

#[derive(Debug, Clone, Copy)]
pub struct BatchBudget {
    pub total_execution_wall: Duration,
    pub startup_wall: Duration,
    pub per_unit_wall: Duration,
    pub cleanup_grace: Duration,
    pub cpu_seconds: u64,
    pub address_space_bytes: u64,
    /// Caller may lower these ceilings; the implementation never exceeds 64
    /// units or 32 MiB raw instances in one request frame.
    pub max_units: usize,
    pub max_total_raw_bytes: usize,
}

impl BatchBudget {
    pub(crate) fn validate(self) -> Result<(), ExecutorFailure> {
        let budget = self;
        if budget.total_execution_wall.is_zero()
            || budget.total_execution_wall > Duration::from_secs(3600)
            || budget.startup_wall.is_zero()
            || budget.startup_wall > budget.total_execution_wall
            || budget.per_unit_wall.is_zero()
            || budget.per_unit_wall > budget.total_execution_wall
            || budget.cleanup_grace > Duration::from_secs(1)
            || budget.cpu_seconds == 0
            || budget.cpu_seconds > 3600
            || budget.address_space_bytes < 64 * 1024 * 1024
            || budget.address_space_bytes > 8 * 1024 * 1024 * 1024
            || budget.max_units == 0
            || budget.max_units > MAX_BATCH_UNITS
            || budget.max_total_raw_bytes == 0
            || budget.max_total_raw_bytes > MAX_BATCH_RAW_BYTES
        {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(())
    }
    pub const MAX_UNITS: usize = MAX_BATCH_UNITS;
    pub const MAX_RAW_BYTES: usize = MAX_BATCH_RAW_BYTES;
    pub fn laboratory() -> Self {
        Self {
            total_execution_wall: Duration::from_secs(60),
            startup_wall: Duration::from_secs(10),
            per_unit_wall: Duration::from_secs(5),
            cleanup_grace: Duration::from_millis(200),
            cpu_seconds: 30,
            address_space_bytes: 1024 * 1024 * 1024,
            max_units: MAX_BATCH_UNITS,
            max_total_raw_bytes: MAX_BATCH_RAW_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchUnitVerdict {
    SchemaValid,
    SchemaInvalid,
    InputRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchUnitReceipt {
    pub ordinal: u64,
    pub member_id: String,
    pub relative_path: String,
    pub root_uri: String,
    pub raw_sha256: Digest256,
    pub unit_sha256: Digest256,
    pub verdict: BatchUnitVerdict,
}

/// This proves only exact transport coverage of the caller-supplied manifest.
/// It is not a corpus membership root or a validation attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchCoverageCheckpoint {
    pub worker_sha256: Digest256,
    pub request_sha256: Digest256,
    pub profile: FormatProfile,
    pub schema_set_sha256: Digest256,
    pub ordered_manifest_sha256: Digest256,
    pub completed_count: u64,
    pub result_stream_sha256: Digest256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchOutcome {
    Complete {
        receipts: Vec<BatchUnitReceipt>,
        checkpoint: BatchCoverageCheckpoint,
    },
    Incomplete {
        receipts: Vec<BatchUnitReceipt>,
        checkpoint: BatchCoverageCheckpoint,
        reason: ExecutorFailure,
        exchange: Option<ExchangeFailureContext>,
    },
}

/// Finite transport profile for a sequence of disposable batch processes.
/// It is not a source-universe limit or source-membership authority.
#[derive(Debug, Clone, Copy)]
pub struct BatchStreamBudget {
    pub batch: BatchBudget,
    pub max_chunks: u64,
    pub max_total_units: u64,
    pub max_total_raw_bytes: u64,
    pub total_execution_wall: Duration,
    /// Cumulative limits for one operation-owned isolated child, not per frame.
    pub operation_cpu_seconds: u64,
    pub operation_address_space_bytes: u64,
    pub max_total_wire_bytes: u64,
    pub max_distinct_selectors: usize,
}

impl BatchStreamBudget {
    pub(crate) fn validate(self) -> Result<(), ExecutorFailure> {
        let budget = self;
        budget.batch.validate()?;
        if budget.max_chunks == 0
            || budget.max_total_units == 0
            || budget.max_total_raw_bytes == 0
            || budget.max_total_wire_bytes == 0
            || budget.max_distinct_selectors == 0
            || budget.max_distinct_selectors == usize::MAX
            || budget.max_chunks == u64::MAX
            || budget.max_total_units == u64::MAX
            || budget.max_total_raw_bytes == u64::MAX
            || budget.max_total_wire_bytes == u64::MAX
            || budget.total_execution_wall.is_zero()
            || budget.total_execution_wall > Duration::from_secs(3600)
            || budget.operation_cpu_seconds == 0
            || budget.operation_cpu_seconds > 3600
            || budget.operation_address_space_bytes < 64 * 1024 * 1024
            || budget.operation_address_space_bytes > 8 * 1024 * 1024 * 1024
        {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(())
    }
    pub fn laboratory() -> Self {
        Self {
            batch: BatchBudget::laboratory(),
            max_chunks: 64,
            max_total_units: 4096,
            max_total_raw_bytes: 128 * 1024 * 1024,
            total_execution_wall: Duration::from_secs(300),
            operation_cpu_seconds: 30,
            operation_address_space_bytes: 1024 * 1024 * 1024,
            max_total_wire_bytes: 2 * 128 * 1024 * 1024 + 32 * 1024 * 1024,
            max_distinct_selectors: MAX_BATCH_UNITS,
        }
    }
}

pub(crate) fn validate_batch_unit(unit: &BatchUnit) -> Result<(), ExecutorFailure> {
    if unit.member_id.is_empty()
        || unit.member_id.len() > MAX_MEMBER_ID_BYTES
        || unit.relative_path.is_empty()
        || unit.relative_path.len() > MAX_PATH_BYTES
        || unit.relative_path.starts_with('/')
        || unit.relative_path.split('/').any(|part| part == "..")
        || unit.root_uri.len() > MAX_URI_BYTES
        || unit.raw_instance.len() > crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
    {
        return Err(ExecutorFailure::InputBudget);
    }
    Ok(())
}

pub(crate) fn batch_unit_digest(unit: &BatchUnit) -> Result<Digest256, ExecutorFailure> {
    validate_batch_unit(unit)?;
    let mut digest = tos_foundation::Digest256Hasher::new();
    digest.update(b"tos-val2-batch-unit-v1\0");
    digest.update(&unit.ordinal.to_be_bytes());
    for value in [
        unit.member_id.as_bytes(),
        unit.relative_path.as_bytes(),
        unit.root_uri.as_bytes(),
        unit.raw_instance.as_slice(),
    ] {
        digest.update(&(value.len() as u32).to_be_bytes());
        digest.update(value);
    }
    Ok(digest.finalize())
}

impl BatchCoverageExpectation {
    /// Exact transport manifest only; this does not establish source coverage.
    pub fn from_units(units: &[BatchUnit]) -> Result<Self, ExecutorFailure> {
        if units.is_empty() || units.len() > MAX_BATCH_UNITS {
            return Err(ExecutorFailure::InputBudget);
        }
        let mut manifest = tos_foundation::Digest256Hasher::new();
        manifest.update(b"tos-val2-batch-manifest-v1\0");
        for (ordinal, unit) in units.iter().enumerate() {
            if unit.ordinal != ordinal as u64 {
                return Err(ExecutorFailure::CoverageMismatch);
            }
            manifest.update(batch_unit_digest(unit)?.as_bytes());
        }
        Ok(Self {
            count: units.len() as u64,
            ordered_manifest_sha256: manifest.finalize(),
        })
    }
}

fn unknown(reason: ExecutorFailure, identity: Option<ExecutionIdentity>) -> ExecutorOutcome {
    ExecutorOutcome::Indeterminate {
        reason,
        identity,
        exchange: None,
    }
}

pub struct BoundedSchemaExecutor;

impl BoundedSchemaExecutor {
    pub fn evaluate(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
    ) -> ExecutorOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate(
                worker,
                resources,
                profile,
                root_uri,
                raw_instance,
                budget,
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (worker, resources, profile, root_uri, raw_instance, budget);
            unknown(ExecutorFailure::UnsupportedHost, None)
        }
    }

    /// Cooperative cancellation checked before and after image verification
    /// and at every nonblocking parent poll. It cannot interrupt a blocked
    /// host filesystem read; cleanup retains the explicit grace budget.
    pub fn evaluate_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
        cancelled: &AtomicBool,
    ) -> ExecutorOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate(
                worker,
                resources,
                profile,
                root_uri,
                raw_instance,
                budget,
                Some(cancelled),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                worker,
                resources,
                profile,
                root_uri,
                raw_instance,
                budget,
                cancelled,
            );
            unknown(ExecutorFailure::UnsupportedHost, None)
        }
    }

    pub fn evaluate_batch(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
    ) -> BatchOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch(worker, resources, profile, units, expected, budget, None)
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (worker, resources, profile, units, expected, budget);
            BatchOutcome::Incomplete {
                receipts: Vec::new(),
                checkpoint: BatchCoverageCheckpoint {
                    worker_sha256: worker.sha256,
                    request_sha256: Digest256::of_bytes(b""),
                    profile,
                    schema_set_sha256: Digest256::of_bytes(b""),
                    ordered_manifest_sha256: Digest256::of_bytes(b""),
                    completed_count: 0,
                    result_stream_sha256: Digest256::of_bytes(b""),
                },
                reason: ExecutorFailure::UnsupportedHost,
                exchange: None,
            }
        }
    }

    /// Same finite protocol, with cooperative cancellation during parent polls.
    pub fn evaluate_batch_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        cancelled: &AtomicBool,
    ) -> BatchOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch(
                worker,
                resources,
                profile,
                units,
                expected,
                budget,
                Some(cancelled),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                worker, resources, profile, units, expected, budget, cancelled,
            );
            BatchOutcome::Incomplete {
                receipts: Vec::new(),
                checkpoint: BatchCoverageCheckpoint {
                    worker_sha256: worker.sha256,
                    request_sha256: Digest256::of_bytes(b""),
                    profile,
                    schema_set_sha256: Digest256::of_bytes(b""),
                    ordered_manifest_sha256: Digest256::of_bytes(b""),
                    completed_count: 0,
                    result_stream_sha256: Digest256::of_bytes(b""),
                },
                reason: ExecutorFailure::UnsupportedHost,
                exchange: None,
            }
        }
    }
}

/// Called only by the dedicated executable. Its process limits are imposed by
/// the parent before `exec`; a direct call to this function has no such limit.
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub fn worker_once() -> std::io::Result<()> {
    native::worker_once()
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub fn worker_once() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "bounded schema worker requires Linux process limits",
    ))
}

// Operation-local custody, used by the selected-cut and record-plan adapters.
// No path lookup or image preparation is repeated after successful creation.
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(crate) use native::{PreparedSchemaWorker, VerifiedWorkerImage};

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub(crate) struct VerifiedWorkerImage;
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
impl VerifiedWorkerImage {
    pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
        None
    }
    pub(crate) fn preflight(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn poison(&mut self, reason: ExecutorFailure) -> ExecutorFailure {
        reason
    }
    pub(crate) fn set_operation_budget(
        &mut self,
        _: BatchStreamBudget,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn finish(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_operation_origin(&mut self, _: Instant) {}

    pub(crate) fn prepare(
        _: &ExactWorkerIdentity,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn matches(&mut self, _: &ExactWorkerIdentity) -> bool {
        false
    }
    pub(crate) fn evaluate(
        &mut self,
        _: &[SchemaResource],
        _: FormatProfile,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> ExecutorOutcome {
        unknown(ExecutorFailure::UnsupportedHost, None)
    }
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub(crate) struct PreparedSchemaWorker;
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
impl PreparedSchemaWorker {
    pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
        None
    }
    pub(crate) fn preflight(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn operation_budget(&self) -> BatchStreamBudget {
        BatchStreamBudget::laboratory()
    }
    pub(crate) fn release_child(
        &mut self,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn wire_cost(
        &self,
        _: usize,
        _: usize,
        _: usize,
        _: usize,
    ) -> Result<(u64, u64, u64), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_operation_budget(
        &mut self,
        _: BatchStreamBudget,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn finish(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_operation_origin(&mut self, _: Instant) {}

    pub(crate) fn prepare(
        _: &ExactWorkerIdentity,
        _: &[SchemaResource],
        _: FormatProfile,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn prepare_with_image(
        _: &VerifiedWorkerImageHandle,
        _: &[SchemaResource],
        _: FormatProfile,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }

    pub(crate) fn evaluate(
        &mut self,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> ExecutorOutcome {
        unknown(ExecutorFailure::UnsupportedHost, None)
    }
    pub(crate) fn evaluate_batch(
        &mut self,
        _: &[BatchUnit],
        _: BatchCoverageExpectation,
        _: BatchBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> BatchOutcome {
        BatchOutcome::Incomplete {
            receipts: Vec::new(),
            checkpoint: BatchCoverageCheckpoint {
                worker_sha256: Digest256::of_bytes(b""),
                request_sha256: Digest256::of_bytes(b""),
                profile: FormatProfile::AssertedSourceCandidateV1,
                schema_set_sha256: Digest256::of_bytes(b""),
                ordered_manifest_sha256: Digest256::of_bytes(b""),
                completed_count: 0,
                result_stream_sha256: Digest256::of_bytes(b""),
            },
            reason: ExecutorFailure::UnsupportedHost,
            exchange: None,
        }
    }
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod native {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::thread;
    use std::time::Instant;
    use tos_foundation::Digest256Hasher;

    // Linux UAPI MFD_EXEC. Requiring this flag fails closed on older kernels
    // or hosts that refuse executable anonymous files.
    const MFD_EXEC_FLAG: u32 = 0x0010;
    const MAX_WORKER_BYTES: u64 = 128 * 1024 * 1024;

    #[cfg(test)]
    thread_local! {
        static TEST_CHILD_STDOUT_INODE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }

    fn profile_byte(profile: FormatProfile) -> u8 {
        match profile {
            FormatProfile::LegacyPythonObserved20260923 => 1,
            FormatProfile::AssertedSourceCandidateV1 => 2,
        }
    }

    fn operation_header(
        nonce: [u8; 16],
        schema_set: Digest256,
        profile: FormatProfile,
        limits: [u64; 8],
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(OPERATION_HEADER_BYTES);
        header.extend_from_slice(OPERATION_REQUEST_MAGIC);
        header.extend_from_slice(&nonce);
        header.extend_from_slice(schema_set.as_bytes());
        header.push(profile_byte(profile));
        for n in limits {
            header.extend_from_slice(&n.to_be_bytes());
        }
        header
    }

    fn parse_profile(value: u8) -> Option<FormatProfile> {
        match value {
            1 => Some(FormatProfile::LegacyPythonObserved20260923),
            2 => Some(FormatProfile::AssertedSourceCandidateV1),
            _ => None,
        }
    }

    fn put_batch_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExecutorFailure> {
        let size = u32::try_from(value.len()).map_err(|_| ExecutorFailure::InputBudget)?;
        if output
            .len()
            .checked_add(4)
            .and_then(|len| len.checked_add(value.len()))
            .filter(|len| *len <= MAX_BATCH_FRAME_BYTES)
            .is_none()
        {
            return Err(ExecutorFailure::InputBudget);
        }
        output.extend_from_slice(&size.to_be_bytes());
        output.extend_from_slice(value);
        Ok(())
    }

    fn schema_set_digest(resources: &[SchemaResource]) -> Result<Digest256, ExecutorFailure> {
        let mut members = BTreeMap::new();
        for resource in resources {
            if members
                .insert(resource.uri.as_str(), Digest256::of_bytes(&resource.raw))
                .is_some()
            {
                return Err(ExecutorFailure::Backend);
            }
        }
        let mut digest = Digest256Hasher::new();
        digest.update(b"tos-schema-set-v1\0");
        for (uri, raw_digest) in members {
            digest.update(&(uri.len() as u64).to_be_bytes());
            digest.update(uri.as_bytes());
            digest.update(raw_digest.as_bytes());
        }
        Ok(digest.finalize())
    }

    #[derive(Clone)]
    struct BatchUnitMeta {
        ordinal: u64,
        member_id: String,
        relative_path: String,
        root_uri: String,
        raw_sha256: Digest256,
        unit_sha256: Digest256,
    }

    #[derive(Clone)]
    struct BatchPrepared {
        frame: Vec<u8>,
        units: Vec<BatchUnitMeta>,
        worker_sha256: Digest256,
        profile: FormatProfile,
        schema_set_sha256: Digest256,
        request_sha256: Digest256,
        ordered_manifest_sha256: Digest256,
    }

    fn empty_batch_outcome(
        worker: Digest256,
        profile: FormatProfile,
        schema: Digest256,
        reason: ExecutorFailure,
    ) -> BatchOutcome {
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        BatchOutcome::Incomplete {
            receipts: Vec::new(),
            reason,
            exchange: None,
            checkpoint: BatchCoverageCheckpoint {
                worker_sha256: worker,
                request_sha256: Digest256::of_bytes(b""),
                profile,
                schema_set_sha256: schema,
                ordered_manifest_sha256: Digest256::of_bytes(b""),
                completed_count: 0,
                result_stream_sha256: results.finalize(),
            },
        }
    }

    fn encode_resources(
        resources: &[SchemaResource],
    ) -> Result<(Vec<u8>, Digest256), ExecutorFailure> {
        if resources.len() > crate::SchemaBackendProbe::MAX_RESOURCES {
            return Err(ExecutorFailure::InputBudget);
        }
        let mut total = 0usize;
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&(resources.len() as u32).to_be_bytes());
        for resource in resources {
            total = total
                .checked_add(resource.raw.len())
                .ok_or(ExecutorFailure::InputBudget)?;
            if resource.uri.len() > MAX_URI_BYTES
                || resource.raw.len() > crate::SchemaBackendProbe::MAX_RESOURCE_BYTES
                || total > crate::SchemaBackendProbe::MAX_TOTAL_BYTES
            {
                return Err(ExecutorFailure::InputBudget);
            }
            put_batch_bytes(&mut encoded, resource.uri.as_bytes())?;
            put_batch_bytes(&mut encoded, &resource.raw)?;
        }
        Ok((encoded, schema_set_digest(resources)?))
    }

    /// Exact immutable image owned for one bounded operation; no global cache.
    /// The same FD may launch independent disposable children with different
    /// schema plans. Resource and request identities remain separate.
    pub(crate) struct VerifiedWorkerImage {
        file: File,
        identity: ExactWorkerIdentity,
        operation_deadline: Instant,
        operation_budget: BatchStreamBudget,
        session: Option<OwnedSchemaSession>,
        poisoned: Option<ExecutorFailure>,
        poison_exchange: Option<ExchangeFailureContext>,
        operation_started: Option<Instant>,
        used_frames: u64,
        used_units: u64,
        used_raw: u64,
        used_wire: u64,
        used_cpu_micros: u64,
        selectors: std::collections::BTreeSet<(String, String)>,
        selected_profile: Option<FormatProfile>,
    }

    struct OwnedSchemaSession {
        child: OperationChild,
        header: Vec<u8>,
        schema_set: Digest256,
        sequence: u64,
    }

    /// One exact encoded resource closure used by the selected-cut adapter.
    pub(crate) struct PreparedSchemaWorker {
        image: VerifiedWorkerImage,
        encoded_resources: Vec<u8>,
        schema_set_sha256: Digest256,
        profile: FormatProfile,
    }

    fn scalar_budget(budget: ExecutorBudget) -> Result<(), ExecutorFailure> {
        if budget.execution_wall.is_zero()
            || budget.cleanup_grace > Duration::from_secs(1)
            || budget.cpu_seconds == 0
            || budget.cpu_seconds > 60
            || budget.address_space_bytes < 64 * 1024 * 1024
            || budget.address_space_bytes > 8 * 1024 * 1024 * 1024
        {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(())
    }

    impl VerifiedWorkerImage {
        pub(crate) fn prepare(
            worker: &ExactWorkerIdentity,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            let operation_started = Instant::now();
            scalar_budget(budget)?;
            let deadline = Instant::now()
                .checked_add(budget.execution_wall)
                .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                .min(operation_deadline);
            let file = sealed_worker_checked(worker, Some(deadline), Some(cancelled))?;
            preparation_check(Some(deadline), Some(cancelled))?;
            Self::from_file(
                file,
                worker.clone(),
                budget,
                operation_deadline,
                operation_started,
            )
        }

        pub(super) fn into_handle(self) -> VerifiedWorkerImageHandle {
            VerifiedWorkerImageHandle {
                file: self.file,
                identity: self.identity,
                operation_deadline: self.operation_deadline,
            }
        }

        fn from_handle(
            handle: &VerifiedWorkerImageHandle,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            scalar_budget(budget)?;
            let operation_deadline = operation_deadline.min(handle.operation_deadline);
            preparation_check(Some(operation_deadline), Some(cancelled))?;
            let file = handle
                .file
                .try_clone()
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
            preparation_check(Some(operation_deadline), Some(cancelled))?;
            Self::from_file(
                file,
                handle.identity.clone(),
                budget,
                operation_deadline,
                Instant::now(),
            )
        }

        fn from_file(
            file: File,
            identity: ExactWorkerIdentity,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            operation_started: Instant,
        ) -> Result<Self, ExecutorFailure> {
            let mut operation_budget = BatchStreamBudget::laboratory();
            operation_budget.batch.total_execution_wall = budget.execution_wall;
            operation_budget.batch.startup_wall = budget.execution_wall;
            operation_budget.batch.per_unit_wall = budget.execution_wall;
            operation_budget.batch.cleanup_grace = budget.cleanup_grace;
            operation_budget.operation_cpu_seconds = budget.cpu_seconds;
            operation_budget.operation_address_space_bytes = budget.address_space_bytes;
            Ok(Self {
                file,
                identity,
                operation_deadline,
                operation_budget,
                session: None,
                poisoned: None,
                poison_exchange: None,
                operation_started: Some(operation_started),
                used_frames: 0,
                used_units: 0,
                used_raw: 0,
                used_wire: 0,
                used_cpu_micros: 0,
                selectors: std::collections::BTreeSet::new(),
                selected_profile: None,
            })
        }

        pub(crate) fn set_operation_budget(
            &mut self,
            budget: BatchStreamBudget,
        ) -> Result<(), ExecutorFailure> {
            if self.used_frames != 0 || self.session.is_some() || self.poisoned.is_some() {
                return Err(ExecutorFailure::ResourceLimitUnknown);
            }
            budget.validate()?;
            self.operation_budget = budget;
            Ok(())
        }
        pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
            self.poison_exchange
        }
        pub(crate) fn poison(&mut self, mut reason: ExecutorFailure) -> ExecutorFailure {
            if let Some(mut session) = self.session.take() {
                if let Err(cleanup) = session.child.cleanup() {
                    reason = cleanup;
                }
            }
            self.poisoned = Some(reason);
            reason
        }
        fn finish_session_inner(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            let deadline = deadline.min(self.operation_deadline).min(
                self.operation_started
                    .and_then(|origin| {
                        origin.checked_add(self.operation_budget.total_execution_wall)
                    })
                    .ok_or(ExecutorFailure::ResourceLimitUnknown)?,
            );
            preparation_check(Some(deadline), Some(cancelled))?;
            let Some(mut session) = self.session.take() else {
                return Ok(());
            };
            let result = (|| {
                let header_sha = Digest256::of_bytes(&session.header);
                let mut close = Vec::new();
                close.extend_from_slice(&session.sequence.to_be_bytes());
                close.extend_from_slice(OPERATION_CLOSE_MAGIC);
                close.extend_from_slice(header_sha.as_bytes());
                let mut hash = Digest256Hasher::new();
                hash.update(&session.header);
                hash.update(&close);
                let close_sha = hash.finalize();
                let mut request = Vec::new();
                request.extend_from_slice(&(close.len() as u32).to_be_bytes());
                request.extend_from_slice(&close);
                self.used_wire = self
                    .used_wire
                    .checked_add((request.len() + OPERATION_FINAL_BYTES) as u64)
                    .filter(|n| *n <= self.operation_budget.max_total_wire_bytes)
                    .ok_or(ExecutorFailure::InputBudget)?;
                let mut written = 0usize;
                let mut response = Vec::new();
                let mut eof = false;
                loop {
                    if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                        return Err(self.poison(reason));
                    }
                    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
                    if session.child.status.is_none() {
                        let mut status = 0;
                        let result = unsafe {
                            libc::wait4(session.child.pid, &mut status, libc::WNOHANG, &mut usage)
                        };
                        if result == session.child.pid {
                            session.child.status = Some(status);
                            let micros = |v: libc::timeval| -> Option<u64> {
                                u64::try_from(v.tv_sec)
                                    .ok()?
                                    .checked_mul(1_000_000)?
                                    .checked_add(u64::try_from(v.tv_usec).ok()?)
                            };
                            self.used_cpu_micros = self
                                .used_cpu_micros
                                .checked_add(
                                    micros(usage.ru_utime)
                                        .and_then(|n| n.checked_add(micros(usage.ru_stime)?))
                                        .ok_or(ExecutorFailure::ResourceLimitUnknown)?,
                                )
                                .ok_or(ExecutorFailure::ResourceLimitUnknown)?;
                            if self.used_cpu_micros
                                > self
                                    .operation_budget
                                    .operation_cpu_seconds
                                    .checked_mul(1_000_000)
                                    .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                            {
                                return Err(self.poison(ExecutorFailure::CpuLimit));
                            }
                        } else if result < 0
                            && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
                        {
                            return Err(self.poison(ExecutorFailure::ResourceLimitUnknown));
                        }
                    }
                    if written < request.len() {
                        let n = unsafe {
                            libc::send(
                                session.child.input.as_raw_fd(),
                                request[written..].as_ptr().cast(),
                                request.len() - written,
                                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                            )
                        };
                        if n > 0 {
                            written += n as usize;
                        } else if n == 0
                            || io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock
                        {
                            return Err(self.poison(ExecutorFailure::Protocol));
                        }
                    }
                    let mut extra = [0u8; OPERATION_FINAL_BYTES + 1];
                    let count = unsafe {
                        libc::recv(
                            session.child.output.as_raw_fd(),
                            extra.as_mut_ptr().cast(),
                            extra.len(),
                            libc::MSG_DONTWAIT,
                        )
                    };
                    if count > 0 {
                        response.extend_from_slice(&extra[..count as usize]);
                        if response.len() > OPERATION_FINAL_BYTES {
                            return Err(self.poison(ExecutorFailure::Protocol));
                        }
                    }
                    if count == 0 {
                        eof = true;
                    }
                    if count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                        return Err(self.poison(ExecutorFailure::Protocol));
                    }
                    if eof && session.child.status.is_some() {
                        if let Some(reason) = session.child.status.and_then(status_failure) {
                            return Err(self.poison(reason));
                        }
                        if written != request.len()
                            || response.len() != OPERATION_FINAL_BYTES
                            || &response[..8] != OPERATION_FINAL_MAGIC
                            || &response[8..40] != header_sha.as_bytes()
                            || &response[40..72] != close_sha.as_bytes()
                            || u64::from_be_bytes(response[72..80].try_into().unwrap())
                                != session.sequence
                        {
                            return Err(self.poison(ExecutorFailure::Protocol));
                        }
                        return Ok(());
                    }
                    let mut fd = libc::pollfd {
                        fd: session.child.output.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    unsafe {
                        libc::poll(&mut fd, 1, 2);
                    }
                }
            })();
            if let Err(reason) = result {
                let reason = session.child.cleanup().err().unwrap_or(reason);
                return Err(self.poison(reason));
            }
            result
        }
        fn finish_session(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            match self.finish_session_inner(deadline, cancelled) {
                Ok(()) => Ok(()),
                Err(reason) => Err(self.poison(reason)),
            }
        }
        pub(crate) fn finish(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            if let Some(reason) = self.poisoned {
                return Err(reason);
            }
            let result = self.finish_session(deadline, cancelled);
            // Finalized operations cannot accept another request or extend their envelope.
            if result.is_ok() {
                self.poisoned = Some(ExecutorFailure::CoverageMismatch);
            }
            result
        }
        fn exchange(
            &mut self,
            encoded: &[u8],
            schema_set: Digest256,
            profile: FormatProfile,
            units: &[BatchUnit],
            expected: BatchCoverageExpectation,
            mut budget: BatchBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> BatchOutcome {
            let start = Instant::now();
            budget.max_units = budget.max_units.min(self.operation_budget.batch.max_units);
            budget.max_total_raw_bytes = budget
                .max_total_raw_bytes
                .min(self.operation_budget.batch.max_total_raw_bytes);
            budget.total_execution_wall = budget
                .total_execution_wall
                .min(self.operation_budget.batch.total_execution_wall);
            budget.startup_wall = budget
                .startup_wall
                .min(self.operation_budget.batch.startup_wall)
                .min(budget.total_execution_wall);
            budget.per_unit_wall = budget
                .per_unit_wall
                .min(self.operation_budget.batch.per_unit_wall)
                .min(budget.total_execution_wall);
            budget.cleanup_grace = budget
                .cleanup_grace
                .min(self.operation_budget.batch.cleanup_grace);
            let empty_resources = 0u32.to_be_bytes();
            let resources = if self
                .session
                .as_ref()
                .is_some_and(|s| s.schema_set == schema_set)
            {
                &empty_resources[..]
            } else {
                encoded
            };
            let prepared = match make_batch_request_encoded(
                self.identity.sha256,
                resources,
                schema_set,
                profile,
                units,
                budget,
            ) {
                Ok(x) => x,
                Err(reason) => {
                    return empty_batch_outcome(self.identity.sha256, profile, schema_set, reason);
                }
            };
            self.exchange_prepared(
                prepared, units, expected, budget, deadline, cancelled, start,
            )
        }

        fn exchange_prepared(
            &mut self,
            mut prepared: BatchPrepared,
            units: &[BatchUnit],
            expected: BatchCoverageExpectation,
            mut budget: BatchBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
            start: Instant,
        ) -> BatchOutcome {
            let schema_set = prepared.schema_set_sha256;
            let profile = prepared.profile;
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let deadline = deadline.min(self.operation_deadline).min(
                self.operation_started
                    .get_or_insert(start)
                    .checked_add(self.operation_budget.total_execution_wall)
                    .unwrap_or(start),
            );
            let refusal = |p, r, why| batch_incomplete(p, Vec::new(), r, why);
            if let Some(reason) = self.poisoned {
                let mut outcome = refusal(prepared, results, reason);
                if let BatchOutcome::Incomplete { exchange, .. } = &mut outcome {
                    *exchange = self.poison_exchange;
                }
                return outcome;
            }
            if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                let why = self.poison(reason);
                return refusal(prepared, results, why);
            }
            if prepared.worker_sha256 != self.identity.sha256 {
                return refusal(
                    prepared,
                    results,
                    self.poison(ExecutorFailure::WorkerIdentity),
                );
            }
            if expected.count != prepared.units.len() as u64
                || expected.ordered_manifest_sha256 != prepared.ordered_manifest_sha256
            {
                return refusal(
                    prepared,
                    results,
                    self.poison(ExecutorFailure::CoverageMismatch),
                );
            }
            if self.selected_profile.is_some_and(|p| p != profile) {
                return refusal(prepared, results, self.poison(ExecutorFailure::Protocol));
            }
            self.selected_profile = Some(profile);
            let raw = units
                .iter()
                .try_fold(0u64, |sum, u| sum.checked_add(u.raw_instance.len() as u64));
            let Some(next_frames) = self
                .used_frames
                .checked_add(1)
                .filter(|n| *n <= self.operation_budget.max_chunks)
            else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            let Some(next_units) = self
                .used_units
                .checked_add(units.len() as u64)
                .filter(|n| *n <= self.operation_budget.max_total_units)
            else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            let Some(next_raw) = raw
                .and_then(|n| self.used_raw.checked_add(n))
                .filter(|n| *n <= self.operation_budget.max_total_raw_bytes)
            else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            for unit in units {
                self.selectors
                    .insert((schema_set.to_hex(), unit.root_uri.clone()));
                if self.selectors.len() > self.operation_budget.max_distinct_selectors {
                    return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
                }
            }
            if self
                .session
                .as_ref()
                .is_some_and(|s| s.schema_set != schema_set)
            {
                if let Err(reason) = self.finish_session(deadline, cancelled) {
                    return refusal(prepared, results, reason);
                }
            }
            let first = self.session.is_none();
            let amount = prepared.frame.len() as u64
                + 8
                + 4
                + if first {
                    OPERATION_HEADER_BYTES as u64
                } else {
                    0
                }
                + (BATCH_ACK_BYTES + prepared.units.len() * BATCH_UNIT_BYTES + OPERATION_END_BYTES)
                    as u64;
            let Some(next_wire) = self.used_wire.checked_add(amount).filter(|n| {
                n.checked_add((4 + 48 + OPERATION_FINAL_BYTES) as u64)
                    .is_some_and(|reserved| reserved <= self.operation_budget.max_total_wire_bytes)
            }) else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            if first {
                let mut nonce = [0u8; 16];
                if File::open("/dev/urandom")
                    .and_then(|mut f| f.read_exact(&mut nonce))
                    .is_err()
                {
                    return refusal(prepared, results, self.poison(ExecutorFailure::Spawn));
                }
                let remaining_cpu = self
                    .operation_budget
                    .operation_cpu_seconds
                    .saturating_sub(self.used_cpu_micros.div_ceil(1_000_000));
                if remaining_cpu == 0 {
                    return refusal(prepared, results, self.poison(ExecutorFailure::CpuLimit));
                }
                let header = operation_header(
                    nonce,
                    schema_set,
                    profile,
                    [
                        self.operation_budget.max_chunks - self.used_frames,
                        self.operation_budget.max_total_units - self.used_units,
                        self.operation_budget.max_total_raw_bytes - self.used_raw,
                        self.operation_budget
                            .max_total_wire_bytes
                            .saturating_sub(self.used_wire),
                        self.operation_budget.max_distinct_selectors as u64,
                        remaining_cpu,
                        self.operation_budget.operation_address_space_bytes,
                        deadline
                            .saturating_duration_since(start)
                            .as_nanos()
                            .min(u64::MAX as u128) as u64,
                    ],
                );
                let argv = [
                    c"tos-schema-worker".as_ptr() as *mut libc::c_char,
                    std::ptr::null_mut(),
                ];
                let child = match spawn_operation_child(
                    &self.file,
                    ExecutorBudget {
                        execution_wall: deadline.saturating_duration_since(start),
                        cleanup_grace: budget.cleanup_grace,
                        cpu_seconds: remaining_cpu,
                        address_space_bytes: self.operation_budget.operation_address_space_bytes,
                    },
                    &argv,
                ) {
                    Ok(x) => x,
                    Err(reason) => return refusal(prepared, results, self.poison(reason)),
                };
                self.session = Some(OwnedSchemaSession {
                    child,
                    header,
                    schema_set,
                    sequence: 0,
                });
            }
            let session = self.session.as_mut().unwrap();
            // Add the operation framing in place instead of keeping full
            // batch/body/wire copies of every instance at the same time.
            let batch_len = prepared.frame.len();
            let header_len = if first { session.header.len() } else { 0 };
            let prefix = header_len + 4 + 8;
            prepared.frame.reserve(prefix);
            prepared.frame.resize(batch_len + prefix, 0);
            prepared.frame.copy_within(0..batch_len, prefix);
            if first {
                prepared.frame[..header_len].copy_from_slice(&session.header);
            }
            prepared.frame[header_len..header_len + 4]
                .copy_from_slice(&((batch_len + 8) as u32).to_be_bytes());
            prepared.frame[header_len + 4..prefix].copy_from_slice(&session.sequence.to_be_bytes());
            let mut request = Digest256Hasher::new();
            request.update(&session.header);
            request.update(&prepared.frame[header_len + 4..]);
            prepared.request_sha256 = request.finalize();
            self.used_frames = next_frames;
            self.used_units = next_units;
            self.used_raw = next_raw;
            self.used_wire = next_wire;
            budget.total_execution_wall = budget
                .total_execution_wall
                .min(deadline.saturating_duration_since(start));
            budget.startup_wall = budget.startup_wall.min(budget.total_execution_wall);
            budget.per_unit_wall = budget.per_unit_wall.min(budget.total_execution_wall);
            let outcome = run_batch_exchange(
                &mut session.child,
                prepared,
                results,
                budget,
                start,
                Some(cancelled),
                true,
            );
            match &outcome {
                BatchOutcome::Complete { .. } => session.sequence += 1,
                BatchOutcome::Incomplete {
                    reason, exchange, ..
                } => {
                    // A later preflight/finish refuses this poisoned operation,
                    // but must not replace its first actual exchange origin.
                    if self.poison_exchange.is_none() {
                        self.poison_exchange = *exchange;
                    }
                    let why = *reason;
                    self.poison(why);
                }
            }
            outcome
        }

        pub(crate) fn preflight(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            if let Some(reason) = self.poisoned {
                return Err(reason);
            }
            let deadline = deadline.min(self.operation_deadline).min(
                self.operation_started
                    .and_then(|origin| {
                        origin.checked_add(self.operation_budget.total_execution_wall)
                    })
                    .ok_or(ExecutorFailure::ResourceLimitUnknown)?,
            );
            preparation_check(Some(deadline), Some(cancelled)).map_err(|reason| self.poison(reason))
        }

        pub(crate) fn matches(&self, worker: &ExactWorkerIdentity) -> bool {
            self.identity.sha256 == worker.sha256
                && self.identity.absolute_path == worker.absolute_path
        }

        pub(crate) fn evaluate(
            &mut self,
            resources: &[SchemaResource],
            profile: FormatProfile,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> ExecutorOutcome {
            let start = Instant::now();
            let deadline = deadline.min(self.operation_deadline);
            if let Err(reason) = scalar_budget(budget)
                .and_then(|()| preparation_check(Some(deadline), Some(cancelled)))
            {
                return unknown(reason, None);
            }
            let (encoded, digest) = match encode_resources(resources) {
                Ok(value) => value,
                Err(reason) => return unknown(reason, None),
            };
            // Encoding belongs to this invocation's original wall budget.
            let remaining = budget.execution_wall.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return unknown(ExecutorFailure::Timeout, None);
            }
            self.evaluate_encoded(
                &encoded,
                digest,
                profile,
                root_uri,
                raw_instance,
                ExecutorBudget {
                    execution_wall: remaining,
                    ..budget
                },
                deadline,
                cancelled,
            )
        }

        fn evaluate_encoded(
            &mut self,
            encoded_resources: &[u8],
            schema_set_sha256: Digest256,
            profile: FormatProfile,
            root_uri: &str,
            raw_instance: &[u8],
            mut budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> ExecutorOutcome {
            let start = Instant::now();
            let deadline = deadline.min(self.operation_deadline);
            if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                return unknown(reason, None);
            }
            if let Err(reason) = scalar_budget(budget) {
                return unknown(reason, None);
            }
            budget.execution_wall = budget
                .execution_wall
                .min(deadline.saturating_duration_since(start));
            let units = [BatchUnit {
                ordinal: 0,
                member_id: "scalar-instance".into(),
                relative_path: "scalar-instance".into(),
                root_uri: root_uri.into(),
                raw_instance: raw_instance.to_vec(),
            }];
            let expected = match BatchCoverageExpectation::from_units(&units) {
                Ok(x) => x,
                Err(reason) => return unknown(reason, None),
            };
            let batch = BatchBudget {
                total_execution_wall: budget.execution_wall,
                startup_wall: budget.execution_wall,
                per_unit_wall: budget.execution_wall,
                cleanup_grace: budget.cleanup_grace,
                cpu_seconds: budget.cpu_seconds,
                address_space_bytes: budget.address_space_bytes,
                max_units: 1,
                max_total_raw_bytes: crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
            };
            match self.exchange(
                encoded_resources,
                schema_set_sha256,
                profile,
                &units,
                expected,
                batch,
                deadline,
                cancelled,
            ) {
                BatchOutcome::Complete {
                    receipts,
                    checkpoint,
                } => {
                    let identity = ExecutionIdentity {
                        worker_sha256: checkpoint.worker_sha256,
                        request_sha256: checkpoint.request_sha256,
                        schema_set_sha256,
                        instance_sha256: Digest256::of_bytes(raw_instance),
                        profile,
                    };
                    match receipts[0].verdict {
                        BatchUnitVerdict::SchemaValid => ExecutorOutcome::SchemaValid(identity),
                        BatchUnitVerdict::SchemaInvalid => ExecutorOutcome::SchemaInvalid(identity),
                        BatchUnitVerdict::InputRejected => ExecutorOutcome::InputRejected(identity),
                    }
                }
                BatchOutcome::Incomplete {
                    reason,
                    checkpoint,
                    exchange,
                    ..
                } => {
                    let identity = ExecutionIdentity {
                        worker_sha256: checkpoint.worker_sha256,
                        request_sha256: checkpoint.request_sha256,
                        schema_set_sha256,
                        instance_sha256: Digest256::of_bytes(raw_instance),
                        profile,
                    };
                    if reason == ExecutorFailure::ParseRejected {
                        ExecutorOutcome::InputRejected(identity)
                    } else {
                        ExecutorOutcome::Indeterminate {
                            reason,
                            identity: Some(identity),
                            exchange,
                        }
                    }
                }
            }
        }
    }

    impl PreparedSchemaWorker {
        pub(crate) fn prepare(
            worker: &ExactWorkerIdentity,
            resources: &[SchemaResource],
            profile: FormatProfile,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            Self::prepare_inner(
                worker,
                None,
                resources,
                profile,
                budget,
                operation_deadline,
                cancelled,
            )
        }

        pub(crate) fn prepare_with_image(
            handle: &VerifiedWorkerImageHandle,
            resources: &[SchemaResource],
            profile: FormatProfile,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            Self::prepare_inner(
                handle.identity(),
                Some(handle),
                resources,
                profile,
                budget,
                operation_deadline.min(handle.operation_deadline),
                cancelled,
            )
        }

        fn prepare_inner(
            worker: &ExactWorkerIdentity,
            handle: Option<&VerifiedWorkerImageHandle>,
            resources: &[SchemaResource],
            profile: FormatProfile,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            scalar_budget(budget)?;
            let deadline = Instant::now()
                .checked_add(budget.execution_wall)
                .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                .min(operation_deadline);
            preparation_check(Some(deadline), Some(cancelled))?;
            let (encoded_resources, schema_set_sha256) = encode_resources(resources)?;
            preparation_check(Some(deadline), Some(cancelled))?;
            let remaining = ExecutorBudget {
                execution_wall: deadline.saturating_duration_since(Instant::now()),
                ..budget
            };
            let image = match handle {
                Some(handle) => VerifiedWorkerImage::from_handle(
                    handle,
                    remaining,
                    operation_deadline,
                    cancelled,
                ),
                None => {
                    VerifiedWorkerImage::prepare(worker, remaining, operation_deadline, cancelled)
                }
            }?;
            preparation_check(Some(deadline), Some(cancelled))?;
            Ok(Self {
                image,
                encoded_resources,
                schema_set_sha256,
                profile,
            })
        }

        pub(crate) fn set_operation_budget(
            &mut self,
            budget: BatchStreamBudget,
        ) -> Result<(), ExecutorFailure> {
            self.image.set_operation_budget(budget)
        }
        pub(crate) fn operation_budget(&self) -> BatchStreamBudget {
            self.image.operation_budget
        }
        pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
            self.image.exchange_failure()
        }
        pub(crate) fn preflight(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            self.image.preflight(deadline, cancelled)
        }

        pub(crate) fn release_child(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            if let Some(reason) = self.image.poisoned {
                return Err(reason);
            }
            self.image.finish_session(deadline, cancelled)
        }

        pub(crate) fn finish(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            self.image.finish(deadline, cancelled)
        }
        pub(crate) fn set_operation_origin(&mut self, origin: Instant) {
            self.image.operation_started = Some(origin);
        }
        pub(crate) fn wire_cost(
            &self,
            member_bytes: usize,
            path_bytes: usize,
            selector_bytes: usize,
            instance_bytes: usize,
        ) -> Result<(u64, u64, u64), ExecutorFailure> {
            let operation = (OPERATION_HEADER_BYTES + self.encoded_resources.len() - 4
                + 4
                + 48
                + OPERATION_FINAL_BYTES) as u64;
            let frame = (4
                + 8
                + BATCH_REQUEST_MAGIC.len()
                + 16
                + 1
                + 4
                + 4
                + BATCH_ACK_BYTES
                + OPERATION_END_BYTES) as u64;
            let unit = [
                8usize,
                16,
                member_bytes,
                path_bytes,
                selector_bytes,
                instance_bytes,
                BATCH_UNIT_BYTES,
            ]
            .into_iter()
            .try_fold(0usize, |sum, n| sum.checked_add(n))
            .ok_or(ExecutorFailure::InputBudget)?;
            Ok((operation, frame, unit as u64))
        }

        pub(crate) fn evaluate(
            &mut self,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> ExecutorOutcome {
            self.image.evaluate_encoded(
                &self.encoded_resources,
                self.schema_set_sha256,
                self.profile,
                root_uri,
                raw_instance,
                budget,
                deadline,
                cancelled,
            )
        }

        pub(crate) fn evaluate_batch(
            &mut self,
            units: &[BatchUnit],
            expected: BatchCoverageExpectation,
            mut budget: BatchBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> BatchOutcome {
            self.image.exchange(
                &self.encoded_resources,
                self.schema_set_sha256,
                self.profile,
                units,
                expected,
                budget,
                deadline,
                cancelled,
            )
        }
    }

    #[cfg(test)]
    fn make_batch_request(
        worker_sha256: Digest256,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        budget: BatchBudget,
    ) -> Result<BatchPrepared, ExecutorFailure> {
        let (encoded, schema_set_sha256) = encode_resources(resources)?;
        make_batch_request_encoded(
            worker_sha256,
            &encoded,
            schema_set_sha256,
            profile,
            units,
            budget,
        )
    }

    fn make_batch_request_encoded<U: std::borrow::Borrow<BatchUnit>>(
        worker_sha256: Digest256,
        encoded_resources: &[u8],
        schema_set_sha256: Digest256,
        profile: FormatProfile,
        units: impl IntoIterator<Item = U>,
        budget: BatchBudget,
    ) -> Result<BatchPrepared, ExecutorFailure> {
        budget.validate()?;
        let mut frame = Vec::new();
        frame.extend_from_slice(BATCH_REQUEST_MAGIC);
        let mut nonce = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut nonce))
            .map_err(|_| ExecutorFailure::Spawn)?;
        frame.extend_from_slice(&nonce);
        frame.push(profile_byte(profile));
        frame.extend_from_slice(encoded_resources);
        let count_offset = frame.len();
        frame.extend_from_slice(&0u32.to_be_bytes());
        let mut metas = Vec::new();
        let mut raw_total = 0usize;
        let mut manifest = Digest256Hasher::new();
        manifest.update(b"tos-val2-batch-manifest-v1\0");
        for unit in units {
            let unit = unit.borrow();
            if metas.len() >= budget.max_units || unit.ordinal != metas.len() as u64 {
                return Err(ExecutorFailure::InputBudget);
            }
            validate_batch_unit(unit)?;
            raw_total = raw_total
                .checked_add(unit.raw_instance.len())
                .ok_or(ExecutorFailure::InputBudget)?;
            if raw_total > budget.max_total_raw_bytes {
                return Err(ExecutorFailure::InputBudget);
            }
            let start = frame.len();
            frame.extend_from_slice(&unit.ordinal.to_be_bytes());
            put_batch_bytes(&mut frame, unit.member_id.as_bytes())?;
            put_batch_bytes(&mut frame, unit.relative_path.as_bytes())?;
            put_batch_bytes(&mut frame, unit.root_uri.as_bytes())?;
            put_batch_bytes(&mut frame, &unit.raw_instance)?;
            let mut hasher = Digest256Hasher::new();
            hasher.update(b"tos-val2-batch-unit-v1\0");
            hasher.update(&frame[start..]);
            let unit_sha256 = hasher.finalize();
            manifest.update(unit_sha256.as_bytes());
            metas.push(BatchUnitMeta {
                ordinal: unit.ordinal,
                member_id: unit.member_id.clone(),
                relative_path: unit.relative_path.clone(),
                root_uri: unit.root_uri.clone(),
                raw_sha256: Digest256::of_bytes(&unit.raw_instance),
                unit_sha256,
            });
        }
        if metas.is_empty() {
            return Err(ExecutorFailure::InputBudget);
        }
        frame[count_offset..count_offset + 4].copy_from_slice(&(metas.len() as u32).to_be_bytes());
        let request_sha256 = Digest256::of_bytes(&frame);
        Ok(BatchPrepared {
            frame,
            units: metas,
            worker_sha256,
            profile,
            schema_set_sha256,
            request_sha256,
            ordered_manifest_sha256: manifest.finalize(),
        })
    }

    pub(super) fn evaluate_batch(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        cancelled: Option<&AtomicBool>,
    ) -> BatchOutcome {
        let local_cancelled = AtomicBool::new(false);
        let cancelled = cancelled.unwrap_or(&local_cancelled);
        let start = Instant::now();
        let deadline = start
            .checked_add(budget.total_execution_wall)
            .unwrap_or(start);
        let units: Vec<_> = units
            .into_iter()
            .take(MAX_BATCH_UNITS.saturating_add(1))
            .collect();
        // Keep request/manifest preflight before image lookup.
        let (encoded, set) = match encode_resources(resources) {
            Ok(x) => x,
            Err(reason) => {
                return empty_batch_outcome(
                    worker.sha256,
                    profile,
                    Digest256::of_bytes(b""),
                    reason,
                );
            }
        };
        let prepared =
            match make_batch_request_encoded(worker.sha256, &encoded, set, profile, &units, budget)
            {
                Ok(x) => x,
                Err(reason) => return empty_batch_outcome(worker.sha256, profile, set, reason),
            };
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        if prepared.units.len() as u64 != expected.count
            || prepared.ordered_manifest_sha256 != expected.ordered_manifest_sha256
        {
            return batch_incomplete(
                prepared,
                Vec::new(),
                results,
                ExecutorFailure::CoverageMismatch,
            );
        }
        let scalar = ExecutorBudget {
            execution_wall: budget.total_execution_wall,
            cleanup_grace: budget.cleanup_grace,
            cpu_seconds: budget.cpu_seconds.min(60),
            address_space_bytes: budget.address_space_bytes,
        };
        let mut image = match VerifiedWorkerImage::prepare(worker, scalar, deadline, cancelled) {
            Ok(x) => x,
            Err(reason) => return batch_incomplete(prepared, Vec::new(), results, reason),
        };
        let mut work = BatchStreamBudget::laboratory();
        work.batch = budget;
        work.max_chunks = 1;
        work.max_total_units = budget.max_units as u64;
        work.max_total_raw_bytes = budget.max_total_raw_bytes as u64;
        work.max_total_wire_bytes = (MAX_BATCH_FRAME_BYTES
            + BATCH_ACK_BYTES
            + MAX_BATCH_UNITS * BATCH_UNIT_BYTES
            + OPERATION_END_BYTES
            + OPERATION_HEADER_BYTES
            + OPERATION_FINAL_BYTES
            + 128) as u64;
        work.max_distinct_selectors = budget.max_units;
        work.operation_cpu_seconds = budget.cpu_seconds;
        work.operation_address_space_bytes = budget.address_space_bytes;
        work.total_execution_wall = budget.total_execution_wall;
        if let Err(reason) = image.set_operation_budget(work) {
            return batch_incomplete(prepared, Vec::new(), results, reason);
        }
        // The same preflighted immutable frame now enters the owned path;
        // do not encode/hash/copy the whole request a second time.
        let outcome = image.exchange_prepared(
            prepared, &units, expected, budget, deadline, cancelled, start,
        );
        match outcome {
            BatchOutcome::Complete {
                receipts,
                checkpoint,
            } => match image.finish(deadline, cancelled) {
                Ok(()) => BatchOutcome::Complete {
                    receipts,
                    checkpoint,
                },
                Err(reason) => BatchOutcome::Incomplete {
                    receipts: Vec::new(),
                    checkpoint: BatchCoverageCheckpoint {
                        completed_count: 0,
                        ..checkpoint
                    },
                    reason,
                    exchange: None,
                },
            },
            other => other,
        }
    }

    fn batch_checkpoint(
        prepared: &BatchPrepared,
        completed_count: usize,
        results: Digest256Hasher,
    ) -> BatchCoverageCheckpoint {
        BatchCoverageCheckpoint {
            worker_sha256: prepared.worker_sha256,
            request_sha256: prepared.request_sha256,
            profile: prepared.profile,
            schema_set_sha256: prepared.schema_set_sha256,
            ordered_manifest_sha256: prepared.ordered_manifest_sha256,
            completed_count: completed_count as u64,
            result_stream_sha256: results.finalize(),
        }
    }

    fn batch_incomplete(
        prepared: BatchPrepared,
        receipts: Vec<BatchUnitReceipt>,
        results: Digest256Hasher,
        reason: ExecutorFailure,
    ) -> BatchOutcome {
        let checkpoint = batch_checkpoint(&prepared, receipts.len(), results);
        BatchOutcome::Incomplete {
            receipts,
            checkpoint,
            reason,
            exchange: None,
        }
    }

    /// Snapshot the verified worker into an executable, sealed in-memory file.
    /// Hashing the *copy* removes the in-place mutation race of path+hash+exec.
    fn sealed_worker(worker: &ExactWorkerIdentity) -> Result<File, ExecutorFailure> {
        sealed_worker_checked(worker, None, None)
    }

    fn preparation_check(
        deadline: Option<Instant>,
        cancelled: Option<&AtomicBool>,
    ) -> Result<(), ExecutorFailure> {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err(ExecutorFailure::Cancelled);
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(ExecutorFailure::Timeout);
        }
        Ok(())
    }

    fn sealed_worker_checked(
        worker: &ExactWorkerIdentity,
        deadline: Option<Instant>,
        cancelled: Option<&AtomicBool>,
    ) -> Result<File, ExecutorFailure> {
        preparation_check(deadline, cancelled)?;
        if !worker.absolute_path.is_absolute() {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let mut source = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&worker.absolute_path)
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        let metadata = source
            .metadata()
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        if !metadata.file_type().is_file()
            || metadata.permissions().mode() & 0o111 == 0
            || metadata.len() == 0
            || metadata.len() > MAX_WORKER_BYTES
        {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let fd = unsafe {
            libc::memfd_create(
                c"tos-schema-worker".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | MFD_EXEC_FLAG,
            )
        };
        if fd < 0 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let mut sealed = unsafe { File::from_raw_fd(fd) };
        let mut digest = Digest256Hasher::new();
        let mut copied = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            preparation_check(deadline, cancelled)?;
            let count = source
                .read(&mut buffer)
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
            if count == 0 {
                break;
            }
            copied = copied
                .checked_add(count as u64)
                .ok_or(ExecutorFailure::WorkerIdentity)?;
            if copied > MAX_WORKER_BYTES {
                return Err(ExecutorFailure::WorkerIdentity);
            }
            digest.update(&buffer[..count]);
            sealed
                .write_all(&buffer[..count])
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        }
        preparation_check(deadline, cancelled)?;
        if copied != metadata.len() || digest.finalize() != worker.sha256 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        if unsafe { libc::fcntl(sealed.as_raw_fd(), libc::F_ADD_SEALS, seals) } != 0 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let after = source
            .metadata()
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        if metadata.dev() != after.dev() || metadata.ino() != after.ino() {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        Ok(sealed)
    }

    // After fork, use only libc calls until exec. No Rust allocator, lock, or
    // destructor may run in a possibly multithreaded parent process's child.
    unsafe fn child_exec(
        worker_fd: i32,
        input_fd: i32,
        output_fd: i32,
        null_fd: i32,
        budget: ExecutorBudget,
        parent_pid: libc::pid_t,
        argv: *const *mut libc::c_char,
    ) -> ! {
        let as_limit = libc::rlimit {
            rlim_cur: budget.address_space_bytes,
            rlim_max: budget.address_space_bytes,
        };
        let cpu_limit = libc::rlimit {
            rlim_cur: budget.cpu_seconds.saturating_sub(1).max(1),
            rlim_max: budget.cpu_seconds,
        };
        if unsafe { libc::setpgid(0, 0) } != 0
            || unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0
            || unsafe { libc::getppid() } != parent_pid
            || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
            || unsafe { libc::setrlimit(libc::RLIMIT_AS, &as_limit) } != 0
            || unsafe { libc::setrlimit(libc::RLIMIT_CPU, &cpu_limit) } != 0
            || unsafe { libc::chdir(c"/".as_ptr()) } != 0
            || unsafe { libc::dup2(input_fd, 0) } < 0
            || unsafe { libc::dup2(output_fd, 1) } < 0
            || unsafe { libc::dup2(null_fd, 2) } < 0
        {
            unsafe { libc::_exit(126) };
        }
        // The executable fd remains usable for execveat; all ambient fds
        // (including socket peers and the memfd) close on successful exec.
        const CLOSE_RANGE_CLOEXEC_FLAG: libc::c_int = 4;
        if unsafe { libc::close_range(3, u32::MAX, CLOSE_RANGE_CLOEXEC_FLAG) } != 0 {
            unsafe { libc::_exit(126) };
        }
        let env: [*mut libc::c_char; 1] = [std::ptr::null_mut()];
        unsafe {
            libc::execveat(
                worker_fd,
                c"".as_ptr(),
                argv,
                env.as_ptr(),
                libc::AT_EMPTY_PATH,
            );
            libc::_exit(126)
        }
    }

    fn socket_pair() -> io::Result<(File, File)> {
        let mut fds = [-1, -1];
        if unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
    }

    fn poll_exit(pid: i32, status: &mut Option<i32>) -> Result<(), ExecutorFailure> {
        if status.is_some() {
            return Ok(());
        }
        let mut raw = 0;
        let observed = unsafe { libc::waitpid(pid, &mut raw, libc::WNOHANG) };
        if observed == pid {
            *status = Some(raw);
            Ok(())
        } else if observed == 0 {
            Ok(())
        } else {
            Err(ExecutorFailure::ResourceLimitUnknown)
        }
    }

    fn kill_and_reap(
        pid: i32,
        status: &mut Option<i32>,
        reap_deadline: Instant,
    ) -> Result<(), ExecutorFailure> {
        // waitpid already released this PID/PGID for reuse. Never signal it
        // after that point, even if a descendant kept a protocol socket open.
        if status.is_some() {
            return Ok(());
        }
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
            libc::kill(pid, libc::SIGKILL);
        }
        // Always attempt a nonblocking reap after signalling, even when the
        // shared cleanup deadline has just elapsed.
        loop {
            poll_exit(pid, status)?;
            if status.is_some() || Instant::now() >= reap_deadline {
                break;
            }
            thread::sleep(
                Duration::from_millis(1)
                    .min(reap_deadline.saturating_duration_since(Instant::now())),
            );
        }
        if status.is_none() {
            // A kernel-uninterruptible child cannot be reaped on a deadline.
            // Report the exact residual PID to the caller/host supervisor;
            // no unbounded blocking or detached request thread is hidden.
            return Err(ExecutorFailure::ReapPending(pid));
        }
        Ok(())
    }

    fn status_failure(status: i32) -> Option<ExecutorFailure> {
        let signal = status & 0x7f;
        if signal != 0 {
            return Some(if signal == libc::SIGXCPU {
                ExecutorFailure::CpuLimit
            } else {
                ExecutorFailure::CrashSignal(signal)
            });
        }
        let exit_code = (status >> 8) & 0xff;
        (exit_code != 0).then_some(ExecutorFailure::CrashExit(exit_code))
    }

    pub(super) fn evaluate(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
        cancelled: Option<&AtomicBool>,
    ) -> ExecutorOutcome {
        let local_cancelled = AtomicBool::new(false);
        let cancelled = cancelled.unwrap_or(&local_cancelled);
        let start = Instant::now();
        let deadline = start.checked_add(budget.execution_wall).unwrap_or(start);
        if let Err(reason) =
            scalar_budget(budget).and_then(|_| preparation_check(Some(deadline), Some(cancelled)))
        {
            return unknown(reason, None);
        }
        if raw_instance.len() > crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
            || root_uri.len() > MAX_URI_BYTES
        {
            return unknown(ExecutorFailure::InputBudget, None);
        }
        let (encoded, set) = match encode_resources(resources) {
            Ok(x) => x,
            Err(reason) => return unknown(reason, None),
        };
        let mut image = match VerifiedWorkerImage::prepare(worker, budget, deadline, cancelled) {
            Ok(x) => x,
            Err(reason) => return unknown(reason, None),
        };
        let mut work = BatchStreamBudget::laboratory();
        work.batch = BatchBudget {
            total_execution_wall: budget.execution_wall,
            startup_wall: budget.execution_wall,
            per_unit_wall: budget.execution_wall,
            cleanup_grace: budget.cleanup_grace,
            cpu_seconds: budget.cpu_seconds,
            address_space_bytes: budget.address_space_bytes,
            max_units: 1,
            max_total_raw_bytes: crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
        };
        work.max_chunks = 1;
        work.max_total_units = 1;
        work.max_total_raw_bytes = crate::SchemaBackendProbe::MAX_INSTANCE_BYTES as u64;
        work.max_total_wire_bytes = (MAX_FRAME_BYTES
            + MAX_URI_BYTES
            + OPERATION_HEADER_BYTES
            + 2 * OPERATION_FINAL_BYTES
            + 1024) as u64;
        work.max_distinct_selectors = 1;
        work.operation_cpu_seconds = budget.cpu_seconds;
        work.operation_address_space_bytes = budget.address_space_bytes;
        work.total_execution_wall = budget.execution_wall;
        if let Err(reason) = image.set_operation_budget(work) {
            return unknown(reason, None);
        }
        let outcome = image.evaluate_encoded(
            &encoded,
            set,
            profile,
            root_uri,
            raw_instance,
            budget,
            deadline,
            cancelled,
        );
        if matches!(
            outcome,
            ExecutorOutcome::SchemaValid(_) | ExecutorOutcome::SchemaInvalid(_)
        ) {
            if let Err(reason) = image.finish(deadline, cancelled) {
                return unknown(
                    reason,
                    match outcome {
                        ExecutorOutcome::SchemaValid(x) | ExecutorOutcome::SchemaInvalid(x) => {
                            Some(x)
                        }
                        _ => None,
                    },
                );
            }
        }
        outcome
    }

    #[cfg(test)]
    fn run_image(
        image: &File,
        request: Vec<u8>,
        identity: ExecutionIdentity,
        budget: ExecutorBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
    ) -> ExecutorOutcome {
        // Existing process-custody faults use the same parent I/O/cleanup path.
        let prepared = BatchPrepared {
            frame: request,
            units: vec![BatchUnitMeta {
                ordinal: 0,
                member_id: "custody-probe".into(),
                relative_path: "custody-probe".into(),
                root_uri: "custody-probe".into(),
                raw_sha256: identity.instance_sha256,
                unit_sha256: identity.instance_sha256,
            }],
            worker_sha256: identity.worker_sha256,
            profile: identity.profile,
            schema_set_sha256: identity.schema_set_sha256,
            request_sha256: identity.request_sha256,
            ordered_manifest_sha256: identity.instance_sha256,
        };
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        let batch = BatchBudget {
            total_execution_wall: budget.execution_wall,
            startup_wall: budget.execution_wall,
            per_unit_wall: budget.execution_wall,
            cleanup_grace: budget.cleanup_grace,
            cpu_seconds: budget.cpu_seconds,
            address_space_bytes: budget.address_space_bytes,
            max_units: 1,
            max_total_raw_bytes: crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
        };
        match run_batch_image_cancellable(image, prepared, results, batch, start, argv, cancelled) {
            BatchOutcome::Incomplete {
                reason, exchange, ..
            } => ExecutorOutcome::Indeterminate {
                reason,
                identity: Some(identity),
                exchange,
            },
            BatchOutcome::Complete { .. } => unknown(ExecutorFailure::Protocol, Some(identity)),
        }
    }

    #[cfg(test)]
    fn run_batch_image(
        image: &File,
        prepared: BatchPrepared,
        results: Digest256Hasher,
        budget: BatchBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
    ) -> BatchOutcome {
        run_batch_image_cancellable(image, prepared, results, budget, start, argv, None)
    }

    struct OperationChild {
        pid: libc::pid_t,
        input: File,
        output: File,
        status: Option<i32>,
        natural_status: Option<i32>,
        cleanup_grace: Duration,
        cleanup_result: Option<Result<(), ExecutorFailure>>,
    }
    impl OperationChild {
        fn cleanup(&mut self) -> Result<(), ExecutorFailure> {
            self.cleanup_inner(false)
        }
        fn cleanup_after_eof(&mut self) -> Result<(), ExecutorFailure> {
            self.cleanup_inner(true)
        }
        fn cleanup_inner(&mut self, observe_eof_exit: bool) -> Result<(), ExecutorFailure> {
            if let Some(result) = self.cleanup_result {
                return result;
            }
            let started = Instant::now();
            let reap_deadline = started + self.cleanup_grace;
            // EOF can precede a waitable status. Reserve at least half of the
            // same cleanup grace for forced termination/reaping; this is a
            // best-effort natural observation, not a promised cause capture.
            let observation_deadline = started + self.cleanup_grace / 2;
            let mut observed = poll_exit(self.pid, &mut self.status);
            while observe_eof_exit
                && observed.is_ok()
                && self.status.is_none()
                && Instant::now() < observation_deadline
            {
                thread::sleep(
                    Duration::from_millis(1)
                        .min(observation_deadline.saturating_duration_since(Instant::now())),
                );
                observed = poll_exit(self.pid, &mut self.status);
            }
            // Only status collected before any parent kill belongs to origin.
            self.natural_status = self.status;
            let reaped = kill_and_reap(self.pid, &mut self.status, reap_deadline);
            let result = reaped.and(observed);
            self.cleanup_result = Some(result);
            result
        }
    }
    impl Drop for OperationChild {
        fn drop(&mut self) {
            let _ = self.cleanup();
        }
    }
    fn spawn_operation_child(
        image: &File,
        budget: ExecutorBudget,
        argv: &[*mut libc::c_char],
    ) -> Result<OperationChild, ExecutorFailure> {
        let (input_parent, input_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => {
                return Err(ExecutorFailure::Spawn);
            }
        };
        let (output_parent, output_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => {
                return Err(ExecutorFailure::Spawn);
            }
        };
        #[cfg(test)]
        TEST_CHILD_STDOUT_INODE.with(|cell| cell.set(output_child.metadata().unwrap().ino()));
        let null = match OpenOptions::new().write(true).open("/dev/null") {
            Ok(file) => file,
            Err(_) => {
                return Err(ExecutorFailure::Spawn);
            }
        };
        let parent_pid = unsafe { libc::getpid() };
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(ExecutorFailure::Spawn);
        }
        if pid == 0 {
            unsafe {
                child_exec(
                    image.as_raw_fd(),
                    input_child.as_raw_fd(),
                    output_child.as_raw_fd(),
                    null.as_raw_fd(),
                    budget,
                    parent_pid,
                    argv.as_ptr(),
                )
            }
        }
        drop(input_child);
        drop(output_child);
        drop(null);

        Ok(OperationChild {
            pid,
            input: input_parent,
            output: output_parent,
            status: None,
            natural_status: None,
            cleanup_grace: budget.cleanup_grace,
            cleanup_result: None,
        })
    }

    #[cfg(test)]
    fn run_batch_image_cancellable(
        image: &File,
        prepared: BatchPrepared,
        mut results: Digest256Hasher,
        budget: BatchBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
    ) -> BatchOutcome {
        let mut child = match spawn_operation_child(
            image,
            ExecutorBudget {
                execution_wall: budget.total_execution_wall,
                cleanup_grace: budget.cleanup_grace,
                cpu_seconds: budget.cpu_seconds,
                address_space_bytes: budget.address_space_bytes,
            },
            argv,
        ) {
            Ok(child) => child,
            Err(reason) => return batch_incomplete(prepared, Vec::new(), results, reason),
        };
        run_batch_exchange(
            &mut child, prepared, results, budget, start, cancelled, false,
        )
    }

    fn run_batch_exchange(
        child: &mut OperationChild,
        prepared: BatchPrepared,
        mut results: Digest256Hasher,
        budget: BatchBudget,
        start: Instant,
        cancelled: Option<&AtomicBool>,
        retained: bool,
    ) -> BatchOutcome {
        let pid = child.pid;
        let input_parent = &child.input;
        let output_parent = &child.output;
        let mut input = Some(input_parent);
        let output = output_parent;
        let mut written = 0usize;
        let mut response =
            Vec::with_capacity(BATCH_ACK_BYTES + prepared.units.len() * BATCH_UNIT_BYTES);
        let mut parsed = 0usize;
        let mut ack = false;
        let mut unit_deadline = None;
        let mut receipts = Vec::with_capacity(prepared.units.len());
        let mut output_eof = false;
        let mut status = child.status;
        let mut failure = None;
        let mut terminal = false;
        let expected_bytes = BATCH_ACK_BYTES
            + prepared.units.len() * BATCH_UNIT_BYTES
            + if retained { OPERATION_END_BYTES } else { 0 };
        while if retained {
            !terminal
        } else {
            !output_eof || status.is_none() || receipts.len() != prepared.units.len()
        } {
            if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                failure = Some((ExecutorFailure::Cancelled, "cancellation"));
                break;
            }
            let now = Instant::now();
            let timeout_boundary = if now.duration_since(start) >= budget.total_execution_wall {
                Some("exchange-wall")
            } else if !ack && now.duration_since(start) >= budget.startup_wall {
                Some("ack-startup-wall")
            } else if unit_deadline.is_some_and(|deadline| now >= deadline) {
                Some("unit-wall")
            } else {
                None
            };
            if let Some(boundary) = timeout_boundary {
                failure = Some((ExecutorFailure::Timeout, boundary));
                break;
            }
            if let Err(reason) = poll_exit(pid, &mut status) {
                failure = Some((reason, "child-status-poll"));
                break;
            }
            if !retained && written == prepared.frame.len() {
                if let Some(fd) = input.take() {
                    unsafe { libc::shutdown(fd.as_raw_fd(), libc::SHUT_WR) };
                }
            }
            let mut fds = [
                libc::pollfd {
                    fd: if written < prepared.frame.len() {
                        input.as_ref().map_or(-1, |fd| fd.as_raw_fd())
                    } else {
                        -1
                    },
                    events: libc::POLLOUT,
                    revents: 0,
                },
                libc::pollfd {
                    fd: if output_eof { -1 } else { output.as_raw_fd() },
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            if unsafe { libc::poll(fds.as_mut_ptr(), 2, 2) } < 0 {
                if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    failure = Some((ExecutorFailure::Protocol, "poll-system-call"));
                    break;
                }
                continue;
            }
            if fds[0].revents & libc::POLLOUT != 0 {
                let count = unsafe {
                    libc::send(
                        fds[0].fd,
                        prepared.frame[written..].as_ptr().cast(),
                        prepared.frame.len() - written,
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if count > 0 {
                    written += count as usize;
                } else if count == 0
                    || (count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock)
                {
                    failure = Some((ExecutorFailure::Protocol, "request-send"));
                    break;
                }
            }
            if fds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let mut buffer = [0u8; 1024];
                let count = unsafe {
                    libc::recv(
                        fds[1].fd,
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                if count == 0 {
                    output_eof = true;
                } else if count > 0 {
                    response.extend_from_slice(&buffer[..count as usize]);
                    if response.len() > expected_bytes {
                        failure = Some((ExecutorFailure::Protocol, "response-byte-count"));
                        break;
                    }
                    loop {
                        if parsed == response.len() {
                            break;
                        }
                        if !ack {
                            if response.len() - parsed < BATCH_ACK_BYTES {
                                break;
                            }
                            let bytes = &response[parsed..parsed + BATCH_ACK_BYTES];
                            if &bytes[..8] != BATCH_ACK_MAGIC
                                || &bytes[8..40] != prepared.request_sha256.as_bytes()
                                || &bytes[40..72] != prepared.schema_set_sha256.as_bytes()
                                || u32::from_be_bytes(bytes[72..76].try_into().unwrap()) as usize
                                    != prepared.units.len()
                            {
                                failure = Some((ExecutorFailure::Protocol, "ack-identity"));
                                break;
                            }
                            parsed += BATCH_ACK_BYTES;
                            ack = true;
                            unit_deadline = Some(Instant::now() + budget.per_unit_wall);
                        } else if receipts.len() < prepared.units.len() {
                            if response.len() - parsed < BATCH_UNIT_BYTES {
                                break;
                            }
                            let bytes = &response[parsed..parsed + BATCH_UNIT_BYTES];
                            let meta = &prepared.units[receipts.len()];
                            if &bytes[..8] != BATCH_UNIT_MAGIC
                                || u64::from_be_bytes(bytes[8..16].try_into().unwrap())
                                    != meta.ordinal
                                || &bytes[16..48] != meta.unit_sha256.as_bytes()
                            {
                                failure = Some((ExecutorFailure::Protocol, "unit-identity"));
                                break;
                            }
                            let verdict = match (bytes[48], bytes[49]) {
                                (0, 0) => BatchUnitVerdict::SchemaValid,
                                (1, 0) => BatchUnitVerdict::SchemaInvalid,
                                (2, 1) => BatchUnitVerdict::InputRejected,
                                (3, 1) => {
                                    failure = Some((
                                        ExecutorFailure::InputBudget,
                                        "worker-unit-input-budget",
                                    ));
                                    break;
                                }
                                (3, 2) => {
                                    failure =
                                        Some((ExecutorFailure::Backend, "worker-unit-backend"));
                                    break;
                                }
                                _ => {
                                    failure = Some((ExecutorFailure::Protocol, "unit-verdict"));
                                    break;
                                }
                            };
                            parsed += BATCH_UNIT_BYTES;
                            results.update(meta.unit_sha256.as_bytes());
                            results.update(&[bytes[48], bytes[49]]);
                            receipts.push(BatchUnitReceipt {
                                ordinal: meta.ordinal,
                                member_id: meta.member_id.clone(),
                                relative_path: meta.relative_path.clone(),
                                root_uri: meta.root_uri.clone(),
                                raw_sha256: meta.raw_sha256,
                                unit_sha256: meta.unit_sha256,
                                verdict,
                            });
                            if verdict == BatchUnitVerdict::InputRejected {
                                failure =
                                    Some((ExecutorFailure::ParseRejected, "worker-unit-parse"));
                                break;
                            }
                            unit_deadline = Some(Instant::now() + budget.per_unit_wall);
                        } else if retained && !terminal {
                            if response.len() - parsed < OPERATION_END_BYTES {
                                break;
                            }
                            let end = &response[parsed..parsed + OPERATION_END_BYTES];
                            if &end[..8] != OPERATION_END_MAGIC
                                || &end[8..40] != prepared.request_sha256.as_bytes()
                                || &end[40..72] != results.clone().finalize().as_bytes()
                                || u32::from_be_bytes(end[72..76].try_into().unwrap()) as usize
                                    != receipts.len()
                            {
                                failure = Some((ExecutorFailure::Protocol, "terminal-identity"));
                                break;
                            }
                            parsed += OPERATION_END_BYTES;
                            terminal = true;
                        } else {
                            failure = Some((ExecutorFailure::Protocol, "trailing-response"));
                            break;
                        }
                    }
                    if failure.is_some() {
                        break;
                    }
                } else if io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                    failure = Some((ExecutorFailure::Protocol, "response-receive"));
                    break;
                }
            }
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
            {
                failure = Some((ExecutorFailure::Protocol, "socket-events"));
                break;
            }
            if retained && (output_eof || status.is_some()) {
                failure = Some((
                    ExecutorFailure::Protocol,
                    if output_eof {
                        "early-output-eof"
                    } else {
                        "early-child-exit"
                    },
                ));
                break;
            }
            if output_eof
                && (parsed != response.len() || !ack || receipts.len() != prepared.units.len())
            {
                failure = Some((ExecutorFailure::Protocol, "incomplete-eof"));
                break;
            }
        }
        if retained && failure.is_none() && written != prepared.frame.len() {
            failure = Some((ExecutorFailure::Protocol, "incomplete-request-write"));
        }
        if retained && failure.is_none() {
            let mut extra = [0u8; 1];
            let count = unsafe {
                libc::recv(
                    output.as_raw_fd(),
                    extra.as_mut_ptr().cast(),
                    1,
                    libc::MSG_DONTWAIT,
                )
            };
            if count >= 0 || io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                output_eof |= count == 0;
                failure = Some((
                    ExecutorFailure::Protocol,
                    if count == 0 {
                        "post-terminal-eof"
                    } else if count > 0 {
                        "post-terminal-extra-byte"
                    } else {
                        "post-terminal-receive"
                    },
                ));
            }
        }
        child.status = status;
        if let Some((reason, boundary)) = failure {
            // Timeout/cancellation and non-EOF failures keep prompt cleanup.
            let cleanup = if reason == ExecutorFailure::Protocol && output_eof {
                child.cleanup_after_eof()
            } else {
                child.cleanup()
            };
            let observed_failure = child.natural_status.and_then(status_failure);
            let natural_termination = child.natural_status.map(|status| {
                let signal = status & 0x7f;
                if signal == 0 {
                    ChildTermination::Exited((status >> 8) & 0xff)
                } else {
                    ChildTermination::Signalled(signal)
                }
            });
            let mut outcome = batch_incomplete(
                prepared,
                receipts,
                results,
                cleanup
                    .err()
                    .unwrap_or_else(|| observed_failure.unwrap_or(reason)),
            );
            if let BatchOutcome::Incomplete { exchange, .. } = &mut outcome {
                *exchange = Some(ExchangeFailureContext {
                    boundary,
                    failure: reason,
                    natural_termination,
                });
            }
            return outcome;
        }
        if status.and_then(status_failure).is_some() {
            return batch_incomplete(
                prepared,
                receipts,
                results,
                status.and_then(status_failure).unwrap(),
            );
        }
        let checkpoint = batch_checkpoint(&prepared, receipts.len(), results);
        BatchOutcome::Complete {
            receipts,
            checkpoint,
        }
    }

    struct Cursor<'a> {
        bytes: &'a [u8],
        offset: usize,
    }

    impl<'a> Cursor<'a> {
        fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
            let end = self
                .offset
                .checked_add(count)
                .filter(|end| *end <= self.bytes.len())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "short request"))?;
            let part = &self.bytes[self.offset..end];
            self.offset = end;
            Ok(part)
        }

        fn bytes(&mut self, limit: usize) -> io::Result<&'a [u8]> {
            let len = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as usize;
            if len > limit {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "oversize field"));
            }
            self.take(len)
        }
    }

    pub(super) fn worker_once() -> io::Result<()> {
        let mut stdin = io::stdin();
        let mut magic = [0u8; 8];
        stdin.read_exact(&mut magic)?;
        if &magic != OPERATION_REQUEST_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "operation protocol version",
            ));
        }
        operation_worker_once(stdin, io::stdout(), magic)
    }

    struct BatchParsedUnit<'a> {
        ordinal: u64,
        root_uri: &'a str,
        raw: &'a [u8],
        unit_sha256: Digest256,
    }

    fn parse_batch_resources(cursor: &mut Cursor<'_>) -> io::Result<Vec<SchemaResource>> {
        let resource_count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()) as usize;
        if resource_count > crate::SchemaBackendProbe::MAX_RESOURCES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "batch resources",
            ));
        }
        let mut resource_bytes = 0usize;
        let mut resources = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            let uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "schema uri"))?;
            let raw = cursor.bytes(crate::SchemaBackendProbe::MAX_RESOURCE_BYTES)?;
            resource_bytes = resource_bytes
                .checked_add(raw.len())
                .filter(|total| *total <= crate::SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "schema bytes"))?;
            resources.push(SchemaResource {
                uri: uri.to_owned(),
                raw: raw.to_vec(),
            });
        }
        Ok(resources)
    }
    fn parse_batch_units<'a>(
        cursor: &mut Cursor<'a>,
    ) -> io::Result<(Vec<BatchParsedUnit<'a>>, usize)> {
        let unit_count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()) as usize;
        if unit_count == 0 || unit_count > MAX_BATCH_UNITS {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "unit count"));
        }
        let mut units = Vec::with_capacity(unit_count);
        let mut raw_total = 0usize;
        for ordinal in 0..unit_count {
            let start = cursor.offset;
            let observed_ordinal = u64::from_be_bytes(cursor.take(8)?.try_into().unwrap());
            let member = std::str::from_utf8(cursor.bytes(MAX_MEMBER_ID_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "member id"))?;
            let path = std::str::from_utf8(cursor.bytes(MAX_PATH_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "member path"))?;
            let root_uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "root uri"))?;
            let raw = cursor.bytes(crate::SchemaBackendProbe::MAX_INSTANCE_BYTES)?;
            raw_total = raw_total
                .checked_add(raw.len())
                .filter(|total| *total <= MAX_BATCH_RAW_BYTES)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "batch raw bytes"))?;
            if observed_ordinal != ordinal as u64
                || member.is_empty()
                || path.is_empty()
                || path.starts_with('/')
                || path.split('/').any(|part| part == "..")
            {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "unit identity"));
            }
            let mut digest = Digest256Hasher::new();
            digest.update(b"tos-val2-batch-unit-v1\0");
            digest.update(&cursor.bytes[start..cursor.offset]);
            units.push(BatchParsedUnit {
                ordinal: observed_ordinal,
                root_uri,
                raw,
                unit_sha256: digest.finalize(),
            });
        }
        Ok((units, raw_total))
    }

    fn read_operation_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
        let mut length = [0u8; 4];
        if reader.read(&mut length[..1])? == 0 {
            return Ok(None);
        }
        reader.read_exact(&mut length[1..])?;
        let count = u32::from_be_bytes(length) as usize;
        if count < 8 || count > MAX_BATCH_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "operation frame length",
            ));
        }
        let mut frame = vec![0u8; count];
        reader.read_exact(&mut frame)?;
        Ok(Some(frame))
    }

    fn operation_worker_once(
        mut input: impl Read,
        mut output: impl Write,
        magic: [u8; 8],
    ) -> io::Result<()> {
        use jsonschema::{Registry, Validator};
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "operation identity or budget");
        let mut header = vec![0u8; OPERATION_HEADER_BYTES];
        header[..8].copy_from_slice(&magic);
        input.read_exact(&mut header[8..])?;
        let mut h = Cursor {
            bytes: &header,
            offset: 24,
        };
        let expected_schema = h.take(32)?.to_vec();
        let profile = parse_profile(h.take(1)?[0]).ok_or_else(bad)?;
        let mut number =
            || -> io::Result<u64> { Ok(u64::from_be_bytes(h.take(8)?.try_into().unwrap())) };
        let max_frames = number()?;
        let max_units = number()?;
        let max_raw = number()?;
        let max_wire = number()?;
        let max_selectors = usize::try_from(number()?).map_err(|_| bad())?;
        let cpu = number()?;
        let memory = number()?;
        let wall_nanos = number()?;
        if max_frames == 0
            || max_units == 0
            || max_raw == 0
            || max_wire == 0
            || max_selectors == 0
            || cpu == 0
            || cpu > 3600
            || memory < 64 * 1024 * 1024
            || memory > 8 * 1024 * 1024 * 1024
            || wall_nanos == 0
            || wall_nanos > Duration::from_secs(3600).as_nanos() as u64
        {
            return Err(bad());
        }
        let start = Instant::now();
        let mut frame = read_operation_frame(&mut input)?.ok_or_else(bad)?;
        let mut first = Cursor {
            bytes: &frame,
            offset: 8,
        };
        if first.take(8)? != BATCH_REQUEST_MAGIC {
            return Err(bad());
        }
        first.take(16)?;
        if parse_profile(first.take(1)?[0]) != Some(profile) {
            return Err(bad());
        }
        if (header.len()
            + frame.len()
            + 4
            + BATCH_ACK_BYTES
            + OPERATION_END_BYTES
            + 4
            + 48
            + OPERATION_FINAL_BYTES) as u64
            > max_wire
        {
            return Err(bad());
        }
        let resources = parse_batch_resources(&mut first)?;
        let first_units_offset = first.offset;
        let schema_set = schema_set_digest(&resources).map_err(|_| bad())?;
        if expected_schema.as_slice() != schema_set.as_bytes() {
            return Err(bad());
        }
        let probe = crate::SchemaBackendProbe::new(resources, profile).map_err(|_| bad())?;
        if probe.schema_set_digest() != schema_set {
            return Err(bad());
        }
        let registry = Registry::new()
            .extend(
                probe
                    .resources
                    .iter()
                    .map(|(uri, value)| (uri.as_str(), value.clone())),
            )
            .map_err(|_| bad())?
            .prepare()
            .map_err(|_| bad())?;
        let mut validators = BTreeMap::<String, Validator>::new();
        let mut total_units = 0u64;
        let mut total_raw = 0u64;
        let mut wire = header.len() as u64;
        let mut sequence = 0u64;
        loop {
            if start.elapsed().as_nanos() >= wall_nanos as u128 {
                return Err(bad());
            }
            if frame.len() == 48 && &frame[8..16] == OPERATION_CLOSE_MAGIC {
                let header_sha = Digest256::of_bytes(&header);
                if u64::from_be_bytes(frame[..8].try_into().unwrap()) != sequence
                    || &frame[16..48] != header_sha.as_bytes()
                {
                    return Err(bad());
                }
                wire = wire
                    .checked_add((frame.len() + 4 + OPERATION_FINAL_BYTES) as u64)
                    .filter(|n| *n <= max_wire)
                    .ok_or_else(bad)?;
                let mut request = Digest256Hasher::new();
                request.update(&header);
                request.update(&frame);
                let mut final_ack = Vec::with_capacity(OPERATION_FINAL_BYTES);
                final_ack.extend_from_slice(OPERATION_FINAL_MAGIC);
                final_ack.extend_from_slice(header_sha.as_bytes());
                final_ack.extend_from_slice(request.finalize().as_bytes());
                final_ack.extend_from_slice(&sequence.to_be_bytes());
                output.write_all(&final_ack)?;
                output.flush()?;
                return Ok(());
            }

            if sequence >= max_frames || start.elapsed().as_nanos() >= wall_nanos as u128 {
                return Err(bad());
            }
            wire = wire
                .checked_add(frame.len() as u64 + 4)
                .filter(|n| *n <= max_wire)
                .ok_or_else(bad)?;
            let mut cursor = Cursor {
                bytes: &frame,
                offset: 0,
            };
            if u64::from_be_bytes(cursor.take(8)?.try_into().unwrap()) != sequence {
                return Err(bad());
            }
            if sequence == 0 {
                cursor.offset = first_units_offset;
            } else {
                if cursor.take(8)? != BATCH_REQUEST_MAGIC {
                    return Err(bad());
                }
                cursor.take(16)?;
                if parse_profile(cursor.take(1)?[0]) != Some(profile)
                    || !parse_batch_resources(&mut cursor)?.is_empty()
                {
                    return Err(bad());
                }
            }
            let (units, raw_total) = parse_batch_units(&mut cursor)?;
            if cursor.offset != frame.len() {
                return Err(bad());
            }
            total_units = total_units
                .checked_add(units.len() as u64)
                .filter(|n| *n <= max_units)
                .ok_or_else(bad)?;
            total_raw = total_raw
                .checked_add(raw_total as u64)
                .filter(|n| *n <= max_raw)
                .ok_or_else(bad)?;
            let response_bytes =
                BATCH_ACK_BYTES + units.len() * BATCH_UNIT_BYTES + OPERATION_END_BYTES;
            wire = wire
                .checked_add(response_bytes as u64)
                .filter(|n| *n <= max_wire)
                .ok_or_else(bad)?;
            for unit in &units {
                if !validators.contains_key(unit.root_uri) {
                    if validators.len() >= max_selectors {
                        return Err(bad());
                    }
                    validators.insert(
                        unit.root_uri.to_owned(),
                        compile_selected_validator(&probe, &registry, profile, unit.root_uri)?,
                    );
                }
            }
            let mut request = Digest256Hasher::new();
            request.update(&header);
            request.update(&frame);
            let request_sha = request.finalize();
            let mut ack = Vec::with_capacity(BATCH_ACK_BYTES);
            ack.extend_from_slice(BATCH_ACK_MAGIC);
            ack.extend_from_slice(request_sha.as_bytes());
            ack.extend_from_slice(schema_set.as_bytes());
            ack.extend_from_slice(&(units.len() as u32).to_be_bytes());
            output.write_all(&ack)?;
            output.flush()?;
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            for unit in &units {
                let result = match crate::published_value(
                    unit.raw,
                    crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
                ) {
                    Ok(value) => {
                        if validators[unit.root_uri].is_valid(&value) {
                            (0u8, 0u8)
                        } else {
                            (1, 0)
                        }
                    }
                    Err(crate::SchemaProbeError::InvalidPublishedJson(_))
                    | Err(crate::SchemaProbeError::InvalidJson) => (2, 1),
                    Err(crate::SchemaProbeError::BudgetExceeded) => (3, 1),
                    Err(_) => (3, 2),
                };
                let mut response = Vec::with_capacity(BATCH_UNIT_BYTES);
                response.extend_from_slice(BATCH_UNIT_MAGIC);
                response.extend_from_slice(&unit.ordinal.to_be_bytes());
                response.extend_from_slice(unit.unit_sha256.as_bytes());
                response.extend_from_slice(&[result.0, result.1]);
                output.write_all(&response)?;
                output.flush()?;
                if !matches!(result, (0, 0) | (1, 0)) {
                    // The request-bound typed unit refusal remains the failure
                    // reason. Exit without END/FINAL, so coverage cannot pass.
                    return Ok(());
                }
                results.update(unit.unit_sha256.as_bytes());
                results.update(&[result.0, result.1]);
            }
            let mut end = Vec::with_capacity(OPERATION_END_BYTES);
            end.extend_from_slice(OPERATION_END_MAGIC);
            end.extend_from_slice(request_sha.as_bytes());
            end.extend_from_slice(results.finalize().as_bytes());
            end.extend_from_slice(&(units.len() as u32).to_be_bytes());
            output.write_all(&end)?;
            output.flush()?;
            drop(units);
            sequence = sequence.checked_add(1).ok_or_else(bad)?;
            frame = match read_operation_frame(&mut input)? {
                Some(next) => next,
                None => return Err(bad()),
            };
        }
    }

    fn compile_selected_validator<'a>(
        probe: &crate::SchemaBackendProbe,
        registry: &'a jsonschema::Registry<'a>,
        profile: FormatProfile,
        root_uri: &str,
    ) -> io::Result<jsonschema::Validator> {
        let schema = probe
            .selected_schema(root_uri)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "missing root selector"))?;
        let mut options = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .with_registry(registry)
            .offline()
            .should_validate_formats(true)
            .should_ignore_unknown_formats(false);
        if profile == FormatProfile::LegacyPythonObserved20260923 {
            options = options
                .with_format("date-time", |_| true)
                .with_format("uri", |_| true)
                .with_format("uri-reference", |_| true);
        }
        options
            .build(schema.as_ref())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "schema compile"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn batch_schema() -> Vec<SchemaResource> {
            vec![SchemaResource {
                uri: "https://treeofsophia.local/tests/batch-integer.schema.json".to_owned(),
                raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://treeofsophia.local/tests/batch-integer.schema.json","type":"integer"}"#.to_vec(),
            }]
        }

        fn batch_unit(ordinal: u64, raw: &[u8]) -> BatchUnit {
            BatchUnit {
                ordinal,
                member_id: format!("test-member-{ordinal}"),
                relative_path: format!("synthetic/{ordinal}.json"),
                root_uri: batch_schema()[0].uri.clone(),
                raw_instance: raw.to_vec(),
            }
        }

        fn operation_fixture(prepared: &[&BatchPrepared]) -> (Vec<u8>, Vec<Digest256>) {
            let budget = BatchStreamBudget::laboratory();
            let first = prepared[0];
            let header = operation_header(
                [0; 16],
                first.schema_set_sha256,
                first.profile,
                [
                    budget.max_chunks,
                    budget.max_total_units,
                    budget.max_total_raw_bytes,
                    budget.max_total_wire_bytes,
                    budget.max_distinct_selectors as u64,
                    budget.operation_cpu_seconds,
                    budget.operation_address_space_bytes,
                    budget.total_execution_wall.as_nanos() as u64,
                ],
            );
            let mut input = header.clone();
            let mut requests = Vec::new();
            for (sequence, frame) in prepared.iter().enumerate() {
                let mut body = (sequence as u64).to_be_bytes().to_vec();
                body.extend_from_slice(&frame.frame);
                let mut digest = Digest256Hasher::new();
                digest.update(&header);
                digest.update(&body);
                requests.push(digest.finalize());
                input.extend_from_slice(&(body.len() as u32).to_be_bytes());
                input.extend_from_slice(&body);
            }
            let mut close = (prepared.len() as u64).to_be_bytes().to_vec();
            close.extend_from_slice(OPERATION_CLOSE_MAGIC);
            close.extend_from_slice(Digest256::of_bytes(&header).as_bytes());
            input.extend_from_slice(&(close.len() as u32).to_be_bytes());
            input.extend_from_slice(&close);
            (input, requests)
        }

        fn batch_stream_output(units: Vec<BatchUnit>) -> Vec<u8> {
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                units,
                BatchBudget::laboratory(),
            )
            .unwrap();
            let mut output = Vec::new();
            let (input, request_sha) = operation_fixture(&[&prepared]);
            operation_worker_once(
                std::io::Cursor::new(&input[8..]),
                &mut output,
                *OPERATION_REQUEST_MAGIC,
            )
            .unwrap();
            assert_eq!(&output[8..40], request_sha[0].as_bytes());
            output
        }

        fn fixture_image(path: &str) -> File {
            let path = std::fs::canonicalize(path).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            sealed_worker(&ExactWorkerIdentity {
                absolute_path: path,
                sha256: Digest256::of_bytes(&bytes),
            })
            .unwrap()
        }

        fn fixture_identity() -> ExecutionIdentity {
            ExecutionIdentity {
                worker_sha256: Digest256::of_bytes(b"fixture-worker"),
                request_sha256: Digest256::of_bytes(b"fixture-request"),
                schema_set_sha256: Digest256::of_bytes(b"fixture-schemas"),
                instance_sha256: Digest256::of_bytes(b"fixture-instance"),
                profile: FormatProfile::AssertedSourceCandidateV1,
            }
        }

        #[test]
        fn oversized_and_duplicate_inputs_never_launch_worker() {
            let one = SchemaResource {
                uri: "https://example.invalid/schema".to_owned(),
                raw: b"{}".to_vec(),
            };
            let absent = ExactWorkerIdentity {
                absolute_path: "/absent-worker".into(),
                sha256: Digest256::of_bytes(b""),
            };
            assert!(matches!(
                BoundedSchemaExecutor::evaluate(
                    &absent,
                    &[one.clone()],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    &vec![0; crate::SchemaBackendProbe::MAX_INSTANCE_BYTES + 1],
                    ExecutorBudget::laboratory()
                ),
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::InputBudget,
                    ..
                }
            ));
            assert!(matches!(
                BoundedSchemaExecutor::evaluate(
                    &absent,
                    &[one.clone(), one],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    b"null",
                    ExecutorBudget::laboratory()
                ),
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Backend,
                    ..
                }
            ));
        }

        #[test]
        fn worker_that_never_reads_stdin_is_killed_without_waiting_for_a_writer() {
            let image = fixture_image("/usr/bin/sleep");
            let arg = c"2";
            let argv = [
                c"sleep".as_ptr() as *mut libc::c_char,
                arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = ExecutorBudget {
                execution_wall: Duration::from_millis(200),
                cleanup_grace: Duration::from_millis(200),
                cpu_seconds: 2,
                address_space_bytes: 1024 * 1024 * 1024,
            };
            let start = Instant::now();
            let result = run_image(
                &image,
                vec![b'x'; 2 * 1024 * 1024],
                fixture_identity(),
                budget,
                start,
                &argv,
                None,
            );
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Timeout,
                    ..
                }
            ));
            assert!(start.elapsed() < Duration::from_millis(600));
            // Reuse the same immutable image after the first child's reap.
            // Its second isolated invocation must honor live cancellation.
            let cancelled = AtomicBool::new(false);
            let result = std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(20));
                    cancelled.store(true, Ordering::Relaxed);
                });
                run_image(
                    &image,
                    vec![b'x'; 2 * 1024 * 1024],
                    fixture_identity(),
                    ExecutorBudget {
                        execution_wall: Duration::from_secs(2),
                        ..budget
                    },
                    Instant::now(),
                    &argv,
                    Some(&cancelled),
                )
            });
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Cancelled,
                    ..
                }
            ));
        }

        #[test]
        fn escaped_descendant_retaining_stdout_cannot_hold_parent_after_deadline() {
            use std::os::unix::ffi::OsStrExt;
            use std::time::{SystemTime, UNIX_EPOCH};

            let image = fixture_image("/bin/sh");
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("tos-val2-escaped-{}-{unique}", std::process::id()));
            std::fs::create_dir(&dir).unwrap();
            let pid_file = dir.join("child.pid");
            let pid_arg = std::ffi::CString::new(pid_file.as_os_str().as_bytes()).unwrap();
            // The inner shell records its exact PID, then becomes sleep. It
            // has a new session and retains the worker's stdout socket.
            let script = c"/usr/bin/setsid /bin/sh -c 'echo $$ > \"$1\"; exec /usr/bin/sleep 1.2' child \"$1\" & echo ready; wait";
            let argv = [
                c"sh".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                c"fixture".as_ptr() as *mut libc::c_char,
                pid_arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = ExecutorBudget {
                execution_wall: Duration::from_millis(600),
                cleanup_grace: Duration::from_millis(200),
                cpu_seconds: 2,
                address_space_bytes: 1024 * 1024 * 1024,
            };
            let start = Instant::now();
            let result = run_image(
                &image,
                b"request".to_vec(),
                fixture_identity(),
                budget,
                start,
                &argv,
                None,
            );
            let child_pid: i32 = std::fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let pidfd_raw = unsafe { libc::syscall(libc::SYS_pidfd_open, child_pid, 0) as i32 };
            assert!(pidfd_raw >= 0);
            let pidfd = unsafe { File::from_raw_fd(pidfd_raw) };
            let expected_inode = TEST_CHILD_STDOUT_INODE.with(std::cell::Cell::get);
            let fd1 = std::fs::read_link(format!("/proc/{child_pid}/fd/1")).unwrap();
            assert_eq!(fd1.to_string_lossy(), format!("socket:[{expected_inode}]"));
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Timeout,
                    ..
                }
            ));
            assert!(start.elapsed() < Duration::from_millis(950));
            let mut exit_poll = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            assert_eq!(unsafe { libc::poll(&mut exit_poll, 1, 1500) }, 1);
            assert_ne!(exit_poll.revents & libc::POLLIN, 0);
            std::fs::remove_file(pid_file).unwrap();
            std::fs::remove_dir(dir).unwrap();
        }

        #[test]
        fn batch_reuses_compiled_validator_without_prior_instance_state() {
            let first = batch_stream_output(vec![
                batch_unit(0, b"7"),
                batch_unit(1, br#""x""#),
                batch_unit(2, b"7"),
            ]);
            assert_eq!(
                first.len(),
                BATCH_ACK_BYTES
                    + 3 * BATCH_UNIT_BYTES
                    + OPERATION_END_BYTES
                    + OPERATION_FINAL_BYTES
            );
            assert_eq!(first[BATCH_ACK_BYTES + 48], 0);
            assert_eq!(first[BATCH_ACK_BYTES + BATCH_UNIT_BYTES + 48], 1);
            assert_eq!(first[BATCH_ACK_BYTES + 2 * BATCH_UNIT_BYTES + 48], 0);
            let permuted = batch_stream_output(vec![
                batch_unit(0, br#""x""#),
                batch_unit(1, b"7"),
                batch_unit(2, b"7"),
            ]);
            assert_eq!(permuted[BATCH_ACK_BYTES + 48], 1);
            assert_eq!(permuted[BATCH_ACK_BYTES + BATCH_UNIT_BYTES + 48], 0);
            assert_eq!(permuted[BATCH_ACK_BYTES + 2 * BATCH_UNIT_BYTES + 48], 0);
        }

        #[test]
        fn batch_selected_fragments_keep_scope_and_independent_unit_outcomes() {
            let root = "https://treeofsophia.local/tests/root.json";
            let target = "https://treeofsophia.local/tests/nested/types.json";
            let resources = vec![SchemaResource {
                uri: root.into(),
                raw: format!(r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{root}","$defs":{{"scoped":{{"$id":"nested/child.json","properties":{{"a/b~c":{{"$ref":"types.json#/$defs/code"}}}}}},"deny":false}}}}"#).into_bytes(),
            }, SchemaResource {
                uri: target.into(),
                raw: format!(r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{target}","$defs":{{"code":{{"const":"owned"}}}}}}"#).into_bytes(),
            }];
            let mut units = vec![
                batch_unit(0, br#""owned""#),
                batch_unit(1, br#""other""#),
                batch_unit(2, b"3"),
            ];
            units[0].root_uri = format!("{root}#/$defs/scoped/properties/a~1b~0c");
            units[1].root_uri = units[0].root_uri.clone();
            units[2].root_uri = format!("{root}#/$defs/deny");
            let expected = BatchCoverageExpectation::from_units(&units).unwrap();
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &resources,
                FormatProfile::AssertedSourceCandidateV1,
                units,
                BatchBudget::laboratory(),
            )
            .unwrap();
            assert_eq!(
                prepared.ordered_manifest_sha256,
                expected.ordered_manifest_sha256
            );
            let mut output = Vec::new();
            let mut later_unit = batch_unit(0, br#""owned""#);
            later_unit.root_uri = format!("{root}#/$defs/scoped/properties/a~1b~0c");
            let later = make_batch_request_encoded(
                prepared.worker_sha256,
                &0u32.to_be_bytes(),
                prepared.schema_set_sha256,
                prepared.profile,
                [later_unit],
                BatchBudget::laboratory(),
            )
            .unwrap();
            let (input, request_sha) = operation_fixture(&[&prepared, &later]);
            operation_worker_once(
                std::io::Cursor::new(&input[8..]),
                &mut output,
                *OPERATION_REQUEST_MAGIC,
            )
            .unwrap();
            assert_eq!(&output[8..40], request_sha[0].as_bytes());
            assert_eq!(
                output.len(),
                2 * BATCH_ACK_BYTES
                    + 4 * BATCH_UNIT_BYTES
                    + 2 * OPERATION_END_BYTES
                    + OPERATION_FINAL_BYTES
            );
            assert_eq!(&output[8..40], request_sha[0].as_bytes());
            assert_eq!(&output[40..72], prepared.schema_set_sha256.as_bytes());
            for (ordinal, verdict) in [0u8, 1, 1].into_iter().enumerate() {
                let start = BATCH_ACK_BYTES + ordinal * BATCH_UNIT_BYTES;
                assert_eq!(
                    &output[start + 8..start + 16],
                    &(ordinal as u64).to_be_bytes()
                );
                assert_eq!(
                    &output[start + 16..start + 48],
                    prepared.units[ordinal].unit_sha256.as_bytes()
                );
                assert_eq!(output[start + 48], verdict);
            }
            let second = BATCH_ACK_BYTES + 3 * BATCH_UNIT_BYTES + OPERATION_END_BYTES;
            assert_eq!(&output[second + 8..second + 40], request_sha[1].as_bytes());
            assert_ne!(request_sha[0], request_sha[1]);
            assert_eq!(output[second + BATCH_ACK_BYTES + 48], 0);
            assert_eq!(
                &output[output.len() - OPERATION_FINAL_BYTES
                    ..output.len() - OPERATION_FINAL_BYTES + 8],
                OPERATION_FINAL_MAGIC
            );
            // A later malformed frame or omitted CLOSE cannot finalize this operation.
            let second_input = OPERATION_HEADER_BYTES + 4 + 8 + prepared.frame.len();
            let mut out_of_order = input.clone();
            out_of_order[second_input + 4..second_input + 12].copy_from_slice(&0u64.to_be_bytes());
            for broken in [
                out_of_order,
                input[..input.len() - 52].to_vec(),
                input[..second_input + 6].to_vec(),
            ] {
                let mut partial = Vec::new();
                assert!(
                    operation_worker_once(
                        std::io::Cursor::new(&broken[8..]),
                        &mut partial,
                        *OPERATION_REQUEST_MAGIC
                    )
                    .is_err()
                );
                assert!(!partial.windows(8).any(|w| w == OPERATION_FINAL_MAGIC));
            }
        }

        #[test]
        fn shared_verified_image_preserves_seals_deadline_and_independent_operation_state() {
            use std::os::unix::fs::{FileExt, PermissionsExt};
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("fixture-image");
            let raw = b"Synthetic executable custody fixture; no worker execution.";
            std::fs::write(&path, raw).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
            let cancel = AtomicBool::new(false);
            let deadline = Instant::now() + Duration::from_secs(60);
            let handle = VerifiedWorkerImageHandle::prepare(
                ExactWorkerIdentity {
                    absolute_path: path.clone(),
                    sha256: Digest256::of_bytes(raw),
                },
                ExecutorBudget::laboratory(),
                deadline,
                &cancel,
            )
            .unwrap();
            // Subsequent adapters use only this actual sealed image, never a
            // reopened path or a caller-supplied assertion of verification.
            std::fs::remove_file(&path).unwrap();
            let mut first = VerifiedWorkerImage::from_handle(
                &handle,
                ExecutorBudget::laboratory(),
                deadline,
                &cancel,
            )
            .unwrap();
            let second = VerifiedWorkerImage::from_handle(
                &handle,
                ExecutorBudget::laboratory(),
                deadline,
                &cancel,
            )
            .unwrap();
            let mut observed = vec![0; raw.len()];
            second.file.read_exact_at(&mut observed, 0).unwrap();
            assert_eq!(observed, raw);
            let seals = unsafe { libc::fcntl(second.file.as_raw_fd(), libc::F_GET_SEALS) };
            let required =
                libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
            assert_eq!(seals & required, required);
            first.used_frames = 1;
            first.poisoned = Some(ExecutorFailure::Backend);
            assert_eq!(second.used_frames, 0);
            assert!(second.poisoned.is_none() && second.session.is_none());
            assert_eq!(first.identity.sha256, second.identity.sha256);
            assert_eq!(second.operation_deadline, deadline);
            assert!(matches!(
                VerifiedWorkerImage::from_handle(
                    &handle,
                    ExecutorBudget::laboratory(),
                    Instant::now(),
                    &cancel
                ),
                Err(ExecutorFailure::Timeout)
            ));
            assert!(matches!(
                VerifiedWorkerImage::from_handle(
                    &handle,
                    ExecutorBudget::laboratory(),
                    deadline,
                    &AtomicBool::new(true)
                ),
                Err(ExecutorFailure::Cancelled)
            ));
        }

        #[test]
        fn batch_cancellation_before_image_and_during_poll_is_incomplete() {
            // The selected-cut and varying-plan consumers prepare the same
            // image primitive. Cancelled/expired operations refuse before any
            // lookup, rather than turning setup into an unbounded extra phase.
            let absent = ExactWorkerIdentity {
                absolute_path: PathBuf::from("/absent-worker"),
                sha256: Digest256::of_bytes(b""),
            };
            assert!(matches!(
                VerifiedWorkerImage::prepare(
                    &absent,
                    ExecutorBudget::laboratory(),
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(true)
                ),
                Err(ExecutorFailure::Cancelled)
            ));
            assert!(matches!(
                VerifiedWorkerImage::prepare(
                    &absent,
                    ExecutorBudget::laboratory(),
                    Instant::now(),
                    &AtomicBool::new(false)
                ),
                Err(ExecutorFailure::Timeout)
            ));
            let units = vec![batch_unit(0, b"7")];
            let expected = BatchCoverageExpectation::from_units(&units).unwrap();
            let outcome = BoundedSchemaExecutor::evaluate_batch_cancellable(
                &ExactWorkerIdentity {
                    absolute_path: PathBuf::from("/absent-worker"),
                    sha256: Digest256::of_bytes(b""),
                },
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                units,
                expected,
                BatchBudget::laboratory(),
                &AtomicBool::new(true),
            );
            assert!(
                matches!(outcome, BatchOutcome::Incomplete { reason: ExecutorFailure::Cancelled, receipts, .. } if receipts.is_empty())
            );
            let image = fixture_image("/usr/bin/sleep");
            let argv = [
                c"sleep".as_ptr() as *mut libc::c_char,
                c"2".as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = BatchBudget::laboratory();
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let cancelled = AtomicBool::new(false);
            let start = Instant::now();
            let outcome = std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(20));
                    cancelled.store(true, Ordering::Relaxed);
                });
                run_batch_image_cancellable(
                    &image,
                    prepared,
                    results,
                    budget,
                    start,
                    &argv,
                    Some(&cancelled),
                )
            });
            assert!(
                matches!(outcome, BatchOutcome::Incomplete { reason: ExecutorFailure::Cancelled, receipts, .. } if receipts.is_empty())
            );
            assert!(start.elapsed() < Duration::from_millis(700));
        }

        #[test]
        fn independent_fixed_manifest_oracle_catches_unit_framing_drift() {
            let uri = "https://treeofsophia.local/tests/batch-integer.schema.json";
            let units = [
                BatchUnit {
                    ordinal: 0,
                    member_id: "unit-0".to_owned(),
                    relative_path: "synthetic/0.json".to_owned(),
                    root_uri: uri.to_owned(),
                    raw_instance: b"7".to_vec(),
                },
                BatchUnit {
                    ordinal: 1,
                    member_id: "unit-1".to_owned(),
                    relative_path: "synthetic/1.json".to_owned(),
                    root_uri: uri.to_owned(),
                    raw_instance: b"\"x\"".to_vec(),
                },
            ];
            // Values computed independently with Python hashlib+struct using
            // the published binary framing, not `make_batch_request`.
            let expected_units = [
                "71b39a8481983557b3a69c93c8657c166cbc1baeb5f0668bb15089a07e935af6",
                "d934c3e71a992a994c57209d0f39f106e460cb2ccc06d45aa8ceec2ee69e4213",
            ];
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                units,
                BatchBudget::laboratory(),
            )
            .unwrap();
            for (unit, expected) in prepared.units.iter().zip(expected_units) {
                assert_eq!(unit.unit_sha256.to_hex(), expected);
            }
            assert_eq!(
                prepared.ordered_manifest_sha256.to_hex(),
                "fb0a7be8c7c650bf92309ffe2da54cf36ed2f1a492732518353950b30a4945a0"
            );
        }

        #[test]
        fn batch_missing_trailing_and_budget_violations_refuse_without_receipts() {
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                BatchBudget::laboratory(),
            )
            .unwrap();
            for changed in [prepared.frame[..prepared.frame.len() - 1].to_vec(), {
                let mut value = prepared.frame.clone();
                value.push(0);
                value
            }] {
                let mut output = Vec::new();
                assert!(
                    {
                        let changed_prepared = BatchPrepared {
                            frame: changed,
                            ..prepared.clone()
                        };
                        let (input, _) = operation_fixture(&[&changed_prepared]);
                        operation_worker_once(
                            std::io::Cursor::new(&input[8..]),
                            &mut output,
                            *OPERATION_REQUEST_MAGIC,
                        )
                    }
                    .is_err()
                );
                assert!(output.is_empty());
            }
            let mut tiny = BatchBudget::laboratory();
            tiny.max_total_raw_bytes = 1;
            assert!(matches!(
                make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    [batch_unit(0, b"77")],
                    tiny,
                ),
                Err(ExecutorFailure::InputBudget)
            ));
            let too_many = (0..65).map(|ordinal| batch_unit(ordinal, b"7"));
            assert!(matches!(
                make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    too_many,
                    BatchBudget::laboratory(),
                ),
                Err(ExecutorFailure::InputBudget)
            ));
        }

        #[test]
        fn batch_expected_manifest_mismatch_refuses_before_worker_lookup() {
            let outcome = BoundedSchemaExecutor::evaluate_batch(
                &ExactWorkerIdentity {
                    absolute_path: "/does/not/exist/tos-schema-worker".into(),
                    sha256: Digest256::of_bytes(b"fixture-worker"),
                },
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                BatchCoverageExpectation {
                    count: 1,
                    ordered_manifest_sha256: Digest256::of_bytes(b"wrong manifest"),
                },
                BatchBudget::laboratory(),
            );
            assert!(matches!(
                outcome,
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::CoverageMismatch,
                    receipts,
                    ..
                } if receipts.is_empty()
            ));
        }

        #[test]
        fn batch_missing_or_trailing_worker_receipt_refuses_full_coverage() {
            let image = fixture_image("/usr/bin/python3");
            let budget = BatchBudget::laboratory();
            for trailing in [false, true] {
                let prepared = make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    [batch_unit(0, b"7")],
                    budget,
                )
                .unwrap();
                let mut output = Vec::new();
                output.extend_from_slice(BATCH_ACK_MAGIC);
                output.extend_from_slice(prepared.request_sha256.as_bytes());
                output.extend_from_slice(prepared.schema_set_sha256.as_bytes());
                output.extend_from_slice(&1u32.to_be_bytes());
                if trailing {
                    output.extend_from_slice(BATCH_UNIT_MAGIC);
                    output.extend_from_slice(&0u64.to_be_bytes());
                    output.extend_from_slice(prepared.units[0].unit_sha256.as_bytes());
                    output.extend_from_slice(&[0, 0, 0]);
                }
                let output_hex: String = output.iter().map(|byte| format!("{byte:02x}")).collect();
                let output_arg = std::ffi::CString::new(output_hex).unwrap();
                let script = c"import sys; sys.stdout.buffer.write(bytes.fromhex(sys.argv[1])); sys.stdout.buffer.flush()";
                let argv = [
                    c"python3".as_ptr() as *mut libc::c_char,
                    c"-c".as_ptr() as *mut libc::c_char,
                    script.as_ptr() as *mut libc::c_char,
                    output_arg.as_ptr() as *mut libc::c_char,
                    std::ptr::null_mut(),
                ];
                let mut results = Digest256Hasher::new();
                results.update(b"tos-val2-batch-results-v1\0");
                let outcome =
                    run_batch_image(&image, prepared, results, budget, Instant::now(), &argv);
                assert!(matches!(
                    outcome,
                    BatchOutcome::Incomplete {
                        reason: ExecutorFailure::Protocol,
                        ..
                    }
                ));
            }
            // Force EOF while the child is still blocked on input, then let
            // it exit naturally. EOF must not make cleanup immediately kill it.
            let script = c"import os,sys; os.close(1); sys.stdin.buffer.read(1); sys.exit(17)";
            let argv = [
                c"python3".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut child =
                spawn_operation_child(&image, ExecutorBudget::laboratory(), &argv).unwrap();
            let eof_deadline = Instant::now() + Duration::from_millis(700);
            loop {
                let mut byte = [0u8; 1];
                let count = unsafe {
                    libc::recv(
                        child.output.as_raw_fd(),
                        byte.as_mut_ptr().cast(),
                        1,
                        libc::MSG_DONTWAIT,
                    )
                };
                if count == 0 {
                    break;
                }
                assert!(
                    count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock
                );
                assert!(Instant::now() < eof_deadline, "child did not close output");
                thread::sleep(
                    Duration::from_millis(1)
                        .min(eof_deadline.saturating_duration_since(Instant::now())),
                );
            }
            poll_exit(child.pid, &mut child.status).unwrap();
            assert!(
                child.status.is_none(),
                "EOF fixture must still be awaiting input"
            );
            assert_eq!(
                unsafe {
                    libc::send(
                        child.input.as_raw_fd(),
                        b"x".as_ptr().cast(),
                        1,
                        libc::MSG_NOSIGNAL,
                    )
                },
                1
            );
            let cleanup_started = Instant::now();
            child.cleanup_after_eof().unwrap();
            assert_eq!(
                child.natural_status.map(status_failure),
                Some(Some(ExecutorFailure::CrashExit(17)))
            );
            assert_eq!(child.status, child.natural_status);
            assert!(cleanup_started.elapsed() < Duration::from_millis(700));
            child.cleanup().unwrap();
            assert_eq!(child.status, child.natural_status);

            // Closing stdout does not imply process exit. A live EOF child
            // receives only the original cleanup envelope and then is killed;
            // that SIGKILL is cleanup evidence, never natural termination.
            let script = c"import os,time; os.close(1); time.sleep(2)";
            let argv = [
                c"python3".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut child =
                spawn_operation_child(&image, ExecutorBudget::laboratory(), &argv).unwrap();
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let started = Instant::now();
            let outcome =
                run_batch_exchange(&mut child, prepared, results, budget, started, None, true);
            match outcome {
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::Protocol,
                    receipts,
                    exchange: Some(context),
                    ..
                } => {
                    assert!(receipts.is_empty());
                    assert_eq!(context.boundary, "early-output-eof");
                    assert_eq!(context.failure, ExecutorFailure::Protocol);
                    assert_eq!(context.natural_termination, None);
                }
                other => panic!("live EOF child did not preserve failure origin: {other:?}"),
            }
            assert!(started.elapsed() < Duration::from_millis(700));
            assert_eq!(
                child.status.map(status_failure),
                Some(Some(ExecutorFailure::CrashSignal(libc::SIGKILL)))
            );
            assert_eq!(child.natural_status, None);
            child.cleanup_after_eof().unwrap();
            assert_eq!(child.natural_status, None);

            // A status observed before cleanup is source of the termination
            // detail; unlike a cleanup SIGKILL, it may explain missing output.
            for (script, expected) in [
                (c"import sys; sys.exit(17)", ChildTermination::Exited(17)),
                (
                    c"import os,signal; os.kill(os.getpid(),signal.SIGTERM)",
                    ChildTermination::Signalled(libc::SIGTERM),
                ),
            ] {
                let budget = BatchBudget::laboratory();
                let argv = [
                    c"python3".as_ptr() as *mut libc::c_char,
                    c"-c".as_ptr() as *mut libc::c_char,
                    script.as_ptr() as *mut libc::c_char,
                    std::ptr::null_mut(),
                ];
                let mut child =
                    spawn_operation_child(&image, ExecutorBudget::laboratory(), &argv).unwrap();
                let start = Instant::now();
                while child.status.is_none() && start.elapsed() < Duration::from_millis(700) {
                    poll_exit(child.pid, &mut child.status).unwrap();
                    if child.status.is_none() {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
                assert!(
                    child.status.is_some(),
                    "natural child exit was not observed"
                );
                let prepared = make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    [batch_unit(0, b"7")],
                    budget,
                )
                .unwrap();
                let mut results = Digest256Hasher::new();
                results.update(b"tos-val2-batch-results-v1\0");
                let outcome = run_batch_exchange(
                    &mut child,
                    prepared,
                    results,
                    budget,
                    Instant::now(),
                    None,
                    true,
                );
                match outcome {
                    BatchOutcome::Incomplete {
                        reason,
                        receipts,
                        exchange: Some(context),
                        ..
                    } => {
                        assert_eq!(
                            reason,
                            match expected {
                                ChildTermination::Exited(code) => ExecutorFailure::CrashExit(code),
                                ChildTermination::Signalled(signal) =>
                                    ExecutorFailure::CrashSignal(signal),
                            }
                        );
                        assert!(receipts.is_empty());
                        assert_eq!(context.failure, ExecutorFailure::Protocol);
                        assert_eq!(context.natural_termination, Some(expected));
                        assert!(!context.boundary.is_empty());
                    }
                    other => panic!("missing natural termination context: {other:?}"),
                }
                // Latched cleanup cannot overwrite the independently observed
                // original status on a second call or signal a reused PID.
                child.cleanup().unwrap();
                assert_eq!(child.natural_status, child.status);
            }
        }

        #[test]
        fn batch_startup_timeout_kills_real_worker_and_is_incomplete() {
            let image = fixture_image("/usr/bin/sleep");
            let argv = [
                c"sleep".as_ptr() as *mut libc::c_char,
                c"2".as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut budget = BatchBudget::laboratory();
            budget.total_execution_wall = Duration::from_millis(250);
            budget.startup_wall = Duration::from_millis(200);
            budget.per_unit_wall = Duration::from_millis(100);
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let start = Instant::now();
            let outcome = run_batch_image(&image, prepared, results, budget, start, &argv);
            assert!(matches!(
                &outcome,
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::Timeout,
                    receipts,
                    ..
                } if receipts.is_empty()
            ));
            if let BatchOutcome::Incomplete {
                exchange: Some(context),
                ..
            } = outcome
            {
                assert_eq!(context.failure, ExecutorFailure::Timeout);
                assert_eq!(context.boundary, "ack-startup-wall");
                assert_eq!(
                    context.natural_termination, None,
                    "cleanup SIGKILL is not natural termination"
                );
            } else {
                panic!("missing timeout exchange context");
            }
            assert!(start.elapsed() < Duration::from_millis(700));
        }

        #[test]
        fn batch_per_unit_timeout_after_real_ack_kills_and_refuses() {
            let image = fixture_image("/usr/bin/python3");
            let mut budget = BatchBudget::laboratory();
            budget.total_execution_wall = Duration::from_secs(1);
            budget.startup_wall = Duration::from_millis(700);
            budget.per_unit_wall = Duration::from_millis(100);
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut ack = Vec::new();
            ack.extend_from_slice(BATCH_ACK_MAGIC);
            ack.extend_from_slice(prepared.request_sha256.as_bytes());
            ack.extend_from_slice(prepared.schema_set_sha256.as_bytes());
            ack.extend_from_slice(&1u32.to_be_bytes());
            let ack_hex: String = ack.iter().map(|byte| format!("{byte:02x}")).collect();
            let ack_arg = std::ffi::CString::new(ack_hex).unwrap();
            let script = c"import sys,time; sys.stdout.buffer.write(bytes.fromhex(sys.argv[1])); sys.stdout.buffer.flush(); time.sleep(2)";
            let argv = [
                c"python3".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                ack_arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let start = Instant::now();
            let outcome = run_batch_image(&image, prepared, results, budget, start, &argv);
            assert!(matches!(
                outcome,
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::Timeout,
                    receipts,
                    ..
                } if receipts.is_empty()
            ));
            assert!(start.elapsed() < Duration::from_millis(700));
        }
    }
}
