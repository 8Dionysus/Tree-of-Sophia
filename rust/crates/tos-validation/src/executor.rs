//! Disposable, fail-closed native boundary for one-shot or bounded batch JSON Schema probes.
//!
//! This module does not create a validation trace or admission attestation.
//! The caller must pin the dedicated worker's exact ELF digest. Linux copies
//! that ELF to a sealed executable memfd before launching it; the selected-cut
//! adapter retains that immutable image within one bounded operation, so a path change
//! after verification cannot change the worker image.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tos_foundation::Digest256;

use crate::{FormatProfile, SchemaResource};

const REQUEST_MAGIC: &[u8; 8] = b"TOSV2RQ1";
const RESPONSE_MAGIC: &[u8; 8] = b"TOSV2RS1";
const MAX_URI_BYTES: usize = 4096;
const MAX_FRAME_BYTES: usize = 36 * 1024 * 1024;
const RESPONSE_BYTES: usize = 8 + 32 + 32 + 32 + 2;
const BATCH_REQUEST_MAGIC: &[u8; 8] = b"TOSV2BQ1";
const BATCH_ACK_MAGIC: &[u8; 8] = b"TOSV2BA1";
const BATCH_UNIT_MAGIC: &[u8; 8] = b"TOSV2BU1";
const BATCH_ACK_BYTES: usize = 8 + 32 + 32 + 4;
const BATCH_UNIT_BYTES: usize = 8 + 8 + 32 + 2;
const MAX_BATCH_UNITS: usize = 64;
const MAX_BATCH_RAW_BYTES: usize = 32 * 1024 * 1024;
const MAX_BATCH_FRAME_BYTES: usize = 68 * 1024 * 1024;
const MAX_MEMBER_ID_BYTES: usize = 512;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone)]
pub struct ExactWorkerIdentity {
    pub absolute_path: PathBuf,
    pub sha256: Digest256,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorOutcome {
    SchemaValid(ExecutionIdentity),
    SchemaInvalid(ExecutionIdentity),
    InputRejected(ExecutionIdentity),
    Indeterminate {
        reason: ExecutorFailure,
        identity: Option<ExecutionIdentity>,
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
    /// units or 32 MiB raw instances in one disposable process.
    pub max_units: usize,
    pub max_total_raw_bytes: usize,
}

impl BatchBudget {
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
}

impl BatchStreamBudget {
    pub fn laboratory() -> Self {
        Self {
            batch: BatchBudget::laboratory(),
            max_chunks: 64,
            max_total_units: 4096,
            max_total_raw_bytes: 128 * 1024 * 1024,
            total_execution_wall: Duration::from_secs(300),
        }
    }
}

fn batch_unit_digest(unit: &BatchUnit) -> Result<Digest256, ExecutorFailure> {
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
    ExecutorOutcome::Indeterminate { reason, identity }
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
    pub(crate) fn prepare(
        _: &ExactWorkerIdentity,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn matches(&self, _: &ExactWorkerIdentity) -> bool {
        false
    }
    pub(crate) fn evaluate(
        &self,
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
    pub(crate) fn evaluate(
        &self,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> ExecutorOutcome {
        unknown(ExecutorFailure::UnsupportedHost, None)
    }
    pub(crate) fn evaluate_batch(
        &self,
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

    fn parse_profile(value: u8) -> Option<FormatProfile> {
        match value {
            1 => Some(FormatProfile::LegacyPythonObserved20260923),
            2 => Some(FormatProfile::AssertedSourceCandidateV1),
            _ => None,
        }
    }

    fn put_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExecutorFailure> {
        let size = u32::try_from(value.len()).map_err(|_| ExecutorFailure::InputBudget)?;
        output.extend_from_slice(&size.to_be_bytes());
        output.extend_from_slice(value);
        if output.len() > MAX_FRAME_BYTES {
            return Err(ExecutorFailure::InputBudget);
        }
        Ok(())
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

    struct BatchPrepared {
        frame: Vec<u8>,
        units: Vec<BatchUnitMeta>,
        worker_sha256: Digest256,
        profile: FormatProfile,
        schema_set_sha256: Digest256,
        request_sha256: Digest256,
        ordered_manifest_sha256: Digest256,
    }

    fn empty_batch_checkpoint(
        worker: Digest256,
        profile: FormatProfile,
        schema: Digest256,
        manifest: Digest256,
    ) -> BatchCoverageCheckpoint {
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        BatchCoverageCheckpoint {
            worker_sha256: worker,
            request_sha256: Digest256::of_bytes(b""),
            profile,
            schema_set_sha256: schema,
            ordered_manifest_sha256: manifest,
            completed_count: 0,
            result_stream_sha256: results.finalize(),
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
            put_bytes(&mut encoded, resource.uri.as_bytes())?;
            put_bytes(&mut encoded, &resource.raw)?;
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
            scalar_budget(budget)?;
            let deadline = Instant::now()
                .checked_add(budget.execution_wall)
                .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                .min(operation_deadline);
            let file = sealed_worker_checked(worker, Some(deadline), Some(cancelled))?;
            preparation_check(Some(deadline), Some(cancelled))?;
            Ok(Self {
                file,
                identity: worker.clone(),
                operation_deadline,
            })
        }

        pub(crate) fn matches(&self, worker: &ExactWorkerIdentity) -> bool {
            self.identity.sha256 == worker.sha256
                && self.identity.absolute_path == worker.absolute_path
        }

        pub(crate) fn evaluate(
            &self,
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
            &self,
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
            let (request, request_sha256, schema_set_sha256, instance_sha256) =
                match make_request_encoded(
                    encoded_resources,
                    schema_set_sha256,
                    profile,
                    root_uri,
                    raw_instance,
                ) {
                    Ok(value) => value,
                    Err(reason) => return unknown(reason, None),
                };
            let identity = ExecutionIdentity {
                worker_sha256: self.identity.sha256,
                request_sha256,
                schema_set_sha256,
                instance_sha256,
                profile,
            };
            if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                return unknown(reason, Some(identity));
            }
            if start.elapsed() >= budget.execution_wall {
                return unknown(ExecutorFailure::Timeout, Some(identity));
            }
            let argv = [
                c"tos-schema-worker".as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            run_image(
                &self.file,
                request,
                identity,
                budget,
                start,
                &argv,
                Some(cancelled),
            )
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
            scalar_budget(budget)?;
            let deadline = Instant::now()
                .checked_add(budget.execution_wall)
                .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                .min(operation_deadline);
            preparation_check(Some(deadline), Some(cancelled))?;
            let (encoded_resources, schema_set_sha256) = encode_resources(resources)?;
            preparation_check(Some(deadline), Some(cancelled))?;
            let image = VerifiedWorkerImage::prepare(
                worker,
                ExecutorBudget {
                    execution_wall: deadline.saturating_duration_since(Instant::now()),
                    ..budget
                },
                operation_deadline,
                cancelled,
            )?;
            preparation_check(Some(deadline), Some(cancelled))?;
            Ok(Self {
                image,
                encoded_resources,
                schema_set_sha256,
                profile,
            })
        }

        pub(crate) fn evaluate(
            &self,
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
            &self,
            units: &[BatchUnit],
            expected: BatchCoverageExpectation,
            mut budget: BatchBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> BatchOutcome {
            let start = Instant::now();
            let deadline = deadline.min(self.image.operation_deadline);
            let prepared = match preparation_check(Some(deadline), Some(cancelled)).and_then(|()| {
                make_batch_request_encoded(
                    self.image.identity.sha256,
                    &self.encoded_resources,
                    self.schema_set_sha256,
                    self.profile,
                    units,
                    budget,
                )
            }) {
                Ok(prepared) => prepared,
                Err(reason) => {
                    return BatchOutcome::Incomplete {
                        receipts: Vec::new(),
                        checkpoint: empty_batch_checkpoint(
                            self.image.identity.sha256,
                            self.profile,
                            self.schema_set_sha256,
                            expected.ordered_manifest_sha256,
                        ),
                        reason,
                    };
                }
            };
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            if expected.count != prepared.units.len() as u64
                || expected.ordered_manifest_sha256 != prepared.ordered_manifest_sha256
            {
                return batch_incomplete(
                    prepared,
                    Vec::new(),
                    results,
                    ExecutorFailure::CoverageMismatch,
                );
            }
            if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                return batch_incomplete(prepared, Vec::new(), results, reason);
            }
            budget.total_execution_wall = budget
                .total_execution_wall
                .min(deadline.saturating_duration_since(start));
            budget.startup_wall = budget.startup_wall.min(budget.total_execution_wall);
            budget.per_unit_wall = budget.per_unit_wall.min(budget.total_execution_wall);
            if start.elapsed() >= budget.total_execution_wall
                || start.elapsed() >= budget.startup_wall
            {
                return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Timeout);
            }
            let argv = [
                c"tos-schema-worker".as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            run_batch_image_cancellable(
                &self.image.file,
                prepared,
                results,
                budget,
                start,
                &argv,
                Some(cancelled),
            )
        }
    }

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
            if metas.len() >= budget.max_units
                || unit.ordinal != metas.len() as u64
                || unit.member_id.is_empty()
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
        let start = Instant::now();
        let empty_digest = Digest256::of_bytes(b"");
        let prepared = match if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            Err(ExecutorFailure::Cancelled)
        } else {
            make_batch_request(worker.sha256, resources, profile, units, budget)
        } {
            Ok(prepared) => prepared,
            Err(reason) => {
                return BatchOutcome::Incomplete {
                    receipts: Vec::new(),
                    checkpoint: empty_batch_checkpoint(
                        worker.sha256,
                        profile,
                        empty_digest,
                        empty_digest,
                    ),
                    reason,
                };
            }
        };
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        if expected.count != prepared.units.len() as u64
            || expected.ordered_manifest_sha256 != prepared.ordered_manifest_sha256
        {
            return batch_incomplete(
                prepared,
                Vec::new(),
                results,
                ExecutorFailure::CoverageMismatch,
            );
        }
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Cancelled);
        }
        let image = match sealed_worker(worker) {
            Ok(image) => image,
            Err(reason) => return batch_incomplete(prepared, Vec::new(), results, reason),
        };
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Cancelled);
        }
        if start.elapsed() >= budget.total_execution_wall || start.elapsed() >= budget.startup_wall
        {
            return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Timeout);
        }
        let argv = [
            c"tos-schema-worker".as_ptr() as *mut libc::c_char,
            std::ptr::null_mut(),
        ];
        run_batch_image_cancellable(&image, prepared, results, budget, start, &argv, cancelled)
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
        }
    }

    fn make_request(
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
    ) -> Result<(Vec<u8>, Digest256, Digest256, Digest256), ExecutorFailure> {
        let (encoded, schema_digest) = encode_resources(resources)?;
        make_request_encoded(&encoded, schema_digest, profile, root_uri, raw_instance)
    }

    fn make_request_encoded(
        encoded_resources: &[u8],
        schema_digest: Digest256,
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
    ) -> Result<(Vec<u8>, Digest256, Digest256, Digest256), ExecutorFailure> {
        if raw_instance.len() > crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
            || root_uri.len() > MAX_URI_BYTES
        {
            return Err(ExecutorFailure::InputBudget);
        }
        let instance_digest = Digest256::of_bytes(raw_instance);
        let mut nonce = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut nonce))
            .map_err(|_| ExecutorFailure::Spawn)?;
        let mut frame = Vec::new();
        frame.extend_from_slice(REQUEST_MAGIC);
        frame.extend_from_slice(&nonce);
        frame.push(profile_byte(profile));
        frame.extend_from_slice(encoded_resources);
        put_bytes(&mut frame, root_uri.as_bytes())?;
        put_bytes(&mut frame, raw_instance)?;
        let request_digest = Digest256::of_bytes(&frame);
        Ok((frame, request_digest, schema_digest, instance_digest))
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
        cleanup_grace: Duration,
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
        let reap_deadline = Instant::now() + cleanup_grace;
        while status.is_none() && Instant::now() < reap_deadline {
            poll_exit(pid, status)?;
            if status.is_none() {
                thread::sleep(Duration::from_millis(1));
            }
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
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return unknown(ExecutorFailure::Cancelled, None);
        }
        if let Err(reason) = scalar_budget(budget) {
            return unknown(reason, None);
        }
        let (request, request_sha256, schema_set_sha256, instance_sha256) =
            match make_request(resources, profile, root_uri, raw_instance) {
                Ok(value) => value,
                Err(reason) => return unknown(reason, None),
            };
        let identity = ExecutionIdentity {
            worker_sha256: worker.sha256,
            request_sha256,
            schema_set_sha256,
            instance_sha256,
            profile,
        };
        let start = Instant::now();
        let image = match sealed_worker(worker) {
            Ok(image) => image,
            Err(reason) => return unknown(reason, Some(identity)),
        };
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return unknown(ExecutorFailure::Cancelled, Some(identity));
        }
        if start.elapsed() >= budget.execution_wall {
            return unknown(ExecutorFailure::Timeout, Some(identity));
        }
        let argv = [
            c"tos-schema-worker".as_ptr() as *mut libc::c_char,
            std::ptr::null_mut(),
        ];
        run_image(&image, request, identity, budget, start, &argv, cancelled)
    }

    fn run_image(
        image: &File,
        request: Vec<u8>,
        identity: ExecutionIdentity,
        budget: ExecutorBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
    ) -> ExecutorOutcome {
        let (input_parent, input_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => return unknown(ExecutorFailure::Spawn, Some(identity)),
        };
        let (output_parent, output_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => return unknown(ExecutorFailure::Spawn, Some(identity)),
        };
        #[cfg(test)]
        TEST_CHILD_STDOUT_INODE.with(|cell| cell.set(output_child.metadata().unwrap().ino()));
        let null = match OpenOptions::new().write(true).open("/dev/null") {
            Ok(file) => file,
            Err(_) => return unknown(ExecutorFailure::Spawn, Some(identity)),
        };
        let parent_pid = unsafe { libc::getpid() };
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return unknown(ExecutorFailure::Spawn, Some(identity));
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
        let mut input = Some(input_parent);
        let output = output_parent;
        let mut written = 0usize;
        let mut response = Vec::with_capacity(RESPONSE_BYTES + 1);
        let mut output_eof = false;
        let mut status = None;
        let mut failure = None;
        while !output_eof || written != request.len() || status.is_none() {
            if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                failure = Some(ExecutorFailure::Cancelled);
                break;
            }
            if start.elapsed() >= budget.execution_wall {
                failure = Some(ExecutorFailure::Timeout);
                break;
            }
            if let Err(reason) = poll_exit(pid, &mut status) {
                failure = Some(reason);
                break;
            }
            if written == request.len() {
                if let Some(fd) = input.take() {
                    unsafe { libc::shutdown(fd.as_raw_fd(), libc::SHUT_WR) };
                }
            }
            let mut fds = [
                libc::pollfd {
                    fd: input.as_ref().map_or(-1, AsRawFd::as_raw_fd),
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
                    failure = Some(ExecutorFailure::Protocol);
                    break;
                }
                continue;
            }
            if fds[0].revents & libc::POLLOUT != 0 {
                let count = unsafe {
                    libc::send(
                        fds[0].fd,
                        request[written..].as_ptr().cast(),
                        request.len() - written,
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if count > 0 {
                    written += count as usize;
                } else if count == 0
                    || (count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock)
                {
                    failure = Some(ExecutorFailure::Protocol);
                    break;
                }
            }
            if fds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let mut buffer = [0u8; 256];
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
                    if response.len() > RESPONSE_BYTES {
                        failure = Some(ExecutorFailure::Protocol);
                        break;
                    }
                } else if io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                    failure = Some(ExecutorFailure::Protocol);
                    break;
                }
            }
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
            {
                failure = Some(ExecutorFailure::Protocol);
                break;
            }
        }
        if let Some(reason) = failure {
            let observed_failure = status.and_then(status_failure);
            let result = kill_and_reap(pid, &mut status, budget.cleanup_grace);
            return unknown(
                result
                    .err()
                    .unwrap_or_else(|| observed_failure.unwrap_or(reason)),
                Some(identity),
            );
        }
        let Some(status) = status else {
            return unknown(ExecutorFailure::ResourceLimitUnknown, Some(identity));
        };
        if let Some(reason) = status_failure(status) {
            return unknown(reason, Some(identity));
        }
        interpret_response(&response, identity)
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

    fn run_batch_image_cancellable(
        image: &File,
        prepared: BatchPrepared,
        mut results: Digest256Hasher,
        budget: BatchBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
    ) -> BatchOutcome {
        let (input_parent, input_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => {
                return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Spawn);
            }
        };
        let (output_parent, output_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => {
                return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Spawn);
            }
        };
        let null = match OpenOptions::new().write(true).open("/dev/null") {
            Ok(file) => file,
            Err(_) => {
                return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Spawn);
            }
        };
        let parent_pid = unsafe { libc::getpid() };
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return batch_incomplete(prepared, Vec::new(), results, ExecutorFailure::Spawn);
        }
        if pid == 0 {
            unsafe {
                child_exec(
                    image.as_raw_fd(),
                    input_child.as_raw_fd(),
                    output_child.as_raw_fd(),
                    null.as_raw_fd(),
                    ExecutorBudget {
                        execution_wall: budget.total_execution_wall,
                        cleanup_grace: budget.cleanup_grace,
                        cpu_seconds: budget.cpu_seconds,
                        address_space_bytes: budget.address_space_bytes,
                    },
                    parent_pid,
                    argv.as_ptr(),
                )
            }
        }
        drop(input_child);
        drop(output_child);
        drop(null);

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
        let mut status = None;
        let mut failure = None;
        while !output_eof || status.is_none() || receipts.len() != prepared.units.len() {
            if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                failure = Some(ExecutorFailure::Cancelled);
                break;
            }
            let now = Instant::now();
            if now.duration_since(start) >= budget.total_execution_wall
                || (!ack && now.duration_since(start) >= budget.startup_wall)
                || unit_deadline.is_some_and(|deadline| now >= deadline)
            {
                failure = Some(ExecutorFailure::Timeout);
                break;
            }
            if let Err(reason) = poll_exit(pid, &mut status) {
                failure = Some(reason);
                break;
            }
            if written == prepared.frame.len() {
                if let Some(fd) = input.take() {
                    unsafe { libc::shutdown(fd.as_raw_fd(), libc::SHUT_WR) };
                }
            }
            let mut fds = [
                libc::pollfd {
                    fd: input.as_ref().map_or(-1, AsRawFd::as_raw_fd),
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
                    failure = Some(ExecutorFailure::Protocol);
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
                    failure = Some(ExecutorFailure::Protocol);
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
                    if response.len() > BATCH_ACK_BYTES + prepared.units.len() * BATCH_UNIT_BYTES {
                        failure = Some(ExecutorFailure::Protocol);
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
                                failure = Some(ExecutorFailure::Protocol);
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
                                failure = Some(ExecutorFailure::Protocol);
                                break;
                            }
                            let verdict = match (bytes[48], bytes[49]) {
                                (0, 0) => BatchUnitVerdict::SchemaValid,
                                (1, 0) => BatchUnitVerdict::SchemaInvalid,
                                (2, 1) => BatchUnitVerdict::InputRejected,
                                (3, 1) => {
                                    failure = Some(ExecutorFailure::InputBudget);
                                    break;
                                }
                                (3, 2) => {
                                    failure = Some(ExecutorFailure::Backend);
                                    break;
                                }
                                _ => {
                                    failure = Some(ExecutorFailure::Protocol);
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
                                failure = Some(ExecutorFailure::ParseRejected);
                                break;
                            }
                            unit_deadline = Some(Instant::now() + budget.per_unit_wall);
                        } else {
                            failure = Some(ExecutorFailure::Protocol);
                            break;
                        }
                    }
                    if failure.is_some() {
                        break;
                    }
                } else if io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                    failure = Some(ExecutorFailure::Protocol);
                    break;
                }
            }
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
            {
                failure = Some(ExecutorFailure::Protocol);
                break;
            }
            if output_eof
                && (parsed != response.len() || !ack || receipts.len() != prepared.units.len())
            {
                failure = Some(ExecutorFailure::Protocol);
                break;
            }
        }
        if let Some(reason) = failure {
            let observed_failure = status.and_then(status_failure);
            let result = kill_and_reap(pid, &mut status, budget.cleanup_grace);
            return batch_incomplete(
                prepared,
                receipts,
                results,
                result
                    .err()
                    .unwrap_or_else(|| observed_failure.unwrap_or(reason)),
            );
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

    fn interpret_response(response: &[u8], identity: ExecutionIdentity) -> ExecutorOutcome {
        if response.len() != RESPONSE_BYTES
            || &response[..8] != RESPONSE_MAGIC
            || &response[8..40] != identity.request_sha256.as_bytes()
            || &response[40..72] != identity.schema_set_sha256.as_bytes()
            || &response[72..104] != identity.instance_sha256.as_bytes()
        {
            return unknown(ExecutorFailure::Protocol, Some(identity));
        }
        match (response[104], response[105]) {
            (0, 0) => ExecutorOutcome::SchemaValid(identity),
            (1, 0) => ExecutorOutcome::SchemaInvalid(identity),
            (2, 1) => ExecutorOutcome::InputRejected(identity),
            (3, 1) => unknown(ExecutorFailure::InputBudget, Some(identity)),
            (3, 2) => unknown(ExecutorFailure::Backend, Some(identity)),
            _ => unknown(ExecutorFailure::Protocol, Some(identity)),
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
        if &magic == BATCH_REQUEST_MAGIC {
            return batch_worker_once(stdin, io::stdout(), magic);
        }
        let mut frame = magic.to_vec();
        stdin
            .take((MAX_FRAME_BYTES - 8 + 1) as u64)
            .read_to_end(&mut frame)?;
        if frame.len() > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversize request",
            ));
        }
        let request_sha256 = Digest256::of_bytes(&frame);
        let mut cursor = Cursor {
            bytes: &frame,
            offset: 0,
        };
        if cursor.take(8)? != REQUEST_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "protocol version",
            ));
        }
        let _nonce = cursor.take(16)?;
        let profile = parse_profile(cursor.take(1)?[0])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "format profile"))?;
        let count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()) as usize;
        if count > crate::SchemaBackendProbe::MAX_RESOURCES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "resource count"));
        }
        let mut total = 0usize;
        let mut resources = Vec::with_capacity(count);
        for _ in 0..count {
            let uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "resource uri"))?;
            let raw = cursor.bytes(crate::SchemaBackendProbe::MAX_RESOURCE_BYTES)?;
            total = total
                .checked_add(raw.len())
                .filter(|total| *total <= crate::SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "resource bytes"))?;
            resources.push(SchemaResource {
                uri: uri.to_owned(),
                raw: raw.to_vec(),
            });
        }
        let root_uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "root uri"))?;
        let raw_instance = cursor.bytes(crate::SchemaBackendProbe::MAX_INSTANCE_BYTES)?;
        if cursor.offset != frame.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "trailing bytes"));
        }
        let schema_set_sha256 = schema_set_digest(&resources)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "duplicate resource"))?;
        let instance_sha256 = Digest256::of_bytes(raw_instance);
        let result = match crate::SchemaBackendProbe::new(resources, profile) {
            Ok(probe) => {
                if probe.schema_set_digest() != schema_set_sha256 {
                    (3, 2)
                } else {
                    match probe.is_valid_raw(root_uri, raw_instance) {
                        Ok(true) => (0, 0),
                        Ok(false) => (1, 0),
                        Err(crate::SchemaProbeError::InvalidPublishedJson(_))
                        | Err(crate::SchemaProbeError::InvalidJson) => (2, 1),
                        Err(crate::SchemaProbeError::BudgetExceeded) => (3, 1),
                        Err(_) => (3, 2),
                    }
                }
            }
            Err(crate::SchemaProbeError::BudgetExceeded) => (3, 1),
            Err(_) => (3, 2),
        };
        let mut response = Vec::with_capacity(RESPONSE_BYTES);
        response.extend_from_slice(RESPONSE_MAGIC);
        response.extend_from_slice(request_sha256.as_bytes());
        response.extend_from_slice(schema_set_sha256.as_bytes());
        response.extend_from_slice(instance_sha256.as_bytes());
        response.push(result.0);
        response.push(result.1);
        io::stdout().write_all(&response)
    }

    struct BatchParsedUnit<'a> {
        ordinal: u64,
        root_uri: &'a str,
        raw: &'a [u8],
        unit_sha256: Digest256,
    }

    fn batch_worker_once(
        stdin: impl Read,
        mut stdout: impl Write,
        magic: [u8; 8],
    ) -> io::Result<()> {
        use jsonschema::{Draft, Registry, Validator};

        let mut frame = magic.to_vec();
        stdin
            .take((MAX_BATCH_FRAME_BYTES - 8 + 1) as u64)
            .read_to_end(&mut frame)?;
        if frame.len() > MAX_BATCH_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "batch frame cap",
            ));
        }
        let request_sha256 = Digest256::of_bytes(&frame);
        let mut cursor = Cursor {
            bytes: &frame,
            offset: 0,
        };
        if cursor.take(8)? != BATCH_REQUEST_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "batch version"));
        }
        let _nonce = cursor.take(16)?;
        let profile = parse_profile(cursor.take(1)?[0])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "batch profile"))?;
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
            digest.update(&frame[start..cursor.offset]);
            units.push(BatchParsedUnit {
                ordinal: observed_ordinal,
                root_uri,
                raw,
                unit_sha256: digest.finalize(),
            });
        }
        if cursor.offset != frame.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "batch trailing bytes",
            ));
        }
        let schema_set_sha256 = schema_set_digest(&resources)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "duplicate schema"))?;
        let probe = crate::SchemaBackendProbe::new(resources, profile)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "schema setup"))?;
        if probe.schema_set_digest() != schema_set_sha256 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "schema digest"));
        }
        let registry = Registry::new()
            .extend(
                probe
                    .resources
                    .iter()
                    .map(|(uri, value)| (uri.as_str(), value.clone())),
            )
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "registry"))?
            .prepare()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "registry closure"))?;
        let mut validators: BTreeMap<&str, Validator> = BTreeMap::new();
        for unit in &units {
            if validators.contains_key(unit.root_uri) {
                continue;
            }
            let schema = probe
                .selected_schema(unit.root_uri)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "missing root selector"))?;
            let mut options = jsonschema::options()
                .with_draft(Draft::Draft202012)
                .with_registry(&registry)
                .offline()
                .should_validate_formats(true)
                .should_ignore_unknown_formats(false);
            if profile == FormatProfile::LegacyPythonObserved20260923 {
                options = options
                    .with_format("date-time", |_| true)
                    .with_format("uri", |_| true)
                    .with_format("uri-reference", |_| true);
            }
            let validator = options
                .build(schema.as_ref())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "schema compile"))?;
            validators.insert(unit.root_uri, validator);
        }
        let mut ack = Vec::with_capacity(BATCH_ACK_BYTES);
        ack.extend_from_slice(BATCH_ACK_MAGIC);
        ack.extend_from_slice(request_sha256.as_bytes());
        ack.extend_from_slice(schema_set_sha256.as_bytes());
        ack.extend_from_slice(&(unit_count as u32).to_be_bytes());
        stdout.write_all(&ack)?;
        for unit in units {
            let result = match crate::published_value(
                unit.raw,
                crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
            ) {
                Ok(instance) => {
                    if validators.get(unit.root_uri).unwrap().is_valid(&instance) {
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
            stdout.write_all(&response)?;
            if result != (0, 0) && result != (1, 0) {
                break;
            }
        }
        Ok(())
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
            batch_worker_once(
                std::io::Cursor::new(&prepared.frame[8..]),
                &mut output,
                *BATCH_REQUEST_MAGIC,
            )
            .unwrap();
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
        fn response_requires_exact_request_and_complete_clean_frame() {
            let identity = ExecutionIdentity {
                worker_sha256: Digest256::of_bytes(b"worker"),
                request_sha256: Digest256::of_bytes(b"request"),
                schema_set_sha256: Digest256::of_bytes(b"schemas"),
                instance_sha256: Digest256::of_bytes(b"instance"),
                profile: FormatProfile::AssertedSourceCandidateV1,
            };
            let mut response = Vec::new();
            response.extend_from_slice(RESPONSE_MAGIC);
            response.extend_from_slice(identity.request_sha256.as_bytes());
            response.extend_from_slice(identity.schema_set_sha256.as_bytes());
            response.extend_from_slice(identity.instance_sha256.as_bytes());
            response.extend_from_slice(&[0, 0]);
            assert_eq!(
                interpret_response(&response, identity),
                ExecutorOutcome::SchemaValid(identity)
            );
            for index in [0, 8, 40, 72, 105] {
                let mut forged = response.clone();
                forged[index] ^= 1;
                assert!(matches!(
                    interpret_response(&forged, identity),
                    ExecutorOutcome::Indeterminate {
                        reason: ExecutorFailure::Protocol,
                        ..
                    }
                ));
            }
            response.push(0);
            assert!(matches!(
                interpret_response(&response, identity),
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Protocol,
                    ..
                }
            ));
        }

        #[test]
        fn oversized_and_duplicate_inputs_never_launch_worker() {
            let one = SchemaResource {
                uri: "https://example.invalid/schema".to_owned(),
                raw: b"{}".to_vec(),
            };
            assert!(matches!(
                make_request(
                    &[one.clone()],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    &vec![0; crate::SchemaBackendProbe::MAX_INSTANCE_BYTES + 1]
                ),
                Err(ExecutorFailure::InputBudget)
            ));
            assert!(matches!(
                make_request(
                    &[one.clone(), one],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    b"null"
                ),
                Err(ExecutorFailure::Backend)
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
            assert_eq!(first.len(), BATCH_ACK_BYTES + 3 * BATCH_UNIT_BYTES);
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
            batch_worker_once(
                std::io::Cursor::new(&prepared.frame[8..]),
                &mut output,
                *BATCH_REQUEST_MAGIC,
            )
            .unwrap();
            assert_eq!(output.len(), BATCH_ACK_BYTES + 3 * BATCH_UNIT_BYTES);
            assert_eq!(&output[8..40], prepared.request_sha256.as_bytes());
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
                    batch_worker_once(
                        std::io::Cursor::new(&changed[8..]),
                        &mut output,
                        *BATCH_REQUEST_MAGIC,
                    )
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
                let outcome = run_batch_image(
                    image.try_clone().unwrap(),
                    prepared,
                    results,
                    budget,
                    Instant::now(),
                    &argv,
                );
                assert!(matches!(
                    outcome,
                    BatchOutcome::Incomplete {
                        reason: ExecutorFailure::Protocol,
                        ..
                    }
                ));
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
                outcome,
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::Timeout,
                    receipts,
                    ..
                } if receipts.is_empty()
            ));
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
