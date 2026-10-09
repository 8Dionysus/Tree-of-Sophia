//! Maintained, bounded producer entry for a fresh ManagedLocal native Original
//! pair. This writes only a private data candidate and a compact witness; the
//! installed software owner performs pair verification and promotion later.

use crate::source_creation_store::IsolatedCreationRoot;
use crate::source_current_cut::foundation_capture::{
    AuthoredDiagnosticCapture, AuthoredDiagnosticCaptureLimits,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64},
    },
    time::{Duration, Instant},
};
use tos_compiler::{
    ColdOpenLimits, DedicatedSessionSqliteHeap, ImmutableKnowledgeCustody,
    KnowledgeSelectedExpectation, LinuxFsVerityCustody, NativeFsVerityMeasurement,
    NativeKnowledgeSelection, NativeProcessLimits, NativeSelectionPaths, PublicCapture,
    RuntimeCaptureCreationUsage, RuntimeCaptureOwnedBudget,
    native_cold_resources::LinuxCgroupColdOpenResourceHold, native_snapshot,
    native_snapshot_manifest as manifest, private_tmpfs_stage::PrivateTmpfsStageIsolation,
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, RelativePath, SourceRevision, parse_json,
};
use tos_ops_mechanics_plan::route_cards::RouteSources;
use tos_source_store::{
    CaptureGitRequest, CaptureRestoreLimits, CorpusReader, CutReadLimits, GitCaptureLimits,
    ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
};
use tos_source_store::{CorpusCutReader, SoftwareComponentSelectionV1};
use tos_validation::FormatProfile;
use tos_validation::executor::{
    BatchBudget, BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget,
};
use tos_validation::source_cut::CutWorkerLimits;

const REQUEST_SCHEMA: &str = "tos_native_managed_original_produce_request_v2";
const CORPUS_BUILD_REQUEST_SCHEMA: &str = "tos_native_corpus_build_request_v1";
const CORPUS_PROJECTION_CHECK_REQUEST_SCHEMA: &str =
    "tos_native_corpus_projection_check_request_v1";
const RESULT_SCHEMA: &str = "tos_native_managed_original_produce_result_v2";
const QUERY_VOCABULARY_PATH: &str = "ToS/doctrine/semantic-interchange/query-vocabulary.v1.json";
const SCHEMA_WORKER_SOURCE_PATH: &str = "rust/crates/tos-validation/src/bin/tos-schema-worker.rs";
const COLD_SCHEMA: &str = "tos_native_managed_original_cold_witness_v1";
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;
const MAX_SELECTION_BYTES: usize = 1024 * 1024;
const MAX_EVIDENCE_REF_BYTES: u64 = 4 * 1024 * 1024;
const MAX_BUILD_SECONDS: u64 = 2 * 60 * 60;
const MAX_COLD_VM_STEPS: u64 = 50_000_000_000;
// Logical byte/visitor work across capture, normalization, indexing and seal.
// A packed model can expand to many times its physical size during checked
// reads. These ceilings do not increase memory, file, VM or deadline limits.
const MAX_PRODUCER_WORK_BYTES: u64 = 256 * 1024 * 1024 * 1024;
const MAX_COLD_WORK_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_COLD_ROWS: u64 = 10_000_000;
const MAX_COLD_ROW_BYTES: usize = 8 * 1024 * 1024;
const MAX_COLD_METADATA_BYTES: usize = 1024 * 1024;
const MAX_COLD_SOURCES: usize = 4096;

/// Explicit resource envelope for direct source-root projection. The route
/// reuses the authored-source capture owner and existing composer; these
/// values narrow that operation and grant no source admission/publication.
#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectRepositoryProjectionLimits {
    pub tmpfs_quota_bytes: u64,
    pub tmpfs_inode_limit: u64,
    pub working_ram_bytes: u64,
    pub max_build_seconds: u64,
    pub max_source_members: u64,
    pub max_source_bytes: u64,
    pub max_member_bytes: u64,
    pub max_manifest_bytes: usize,
    pub max_capture_member_read_bytes: usize,
    pub max_capture_write_bytes: usize,
    pub max_recheck_read_bytes: usize,
    pub max_callback_and_fence_state_bytes: usize,
    pub callback_owned_heap_state_upper_bound_bytes: usize,
    pub final_fence_workspace_upper_bound_bytes: usize,
    pub max_state_bytes: usize,
    pub max_json_visits: usize,
    pub max_work_bytes: u64,
    pub max_output_bytes: u64,
    pub max_schema_receipts: usize,
    pub max_schema_receipt_bytes: usize,
    pub worker_cpu_seconds: u64,
    pub worker_address_space_bytes: u64,
    pub max_worker_image_bytes: u64,
    pub cold_open: ColdOpenLimits,
    pub process_limits: NativeProcessLimits,
}

impl DirectRepositoryProjectionLimits {
    fn repo_validation_v1() -> Self {
        Self {
            tmpfs_quota_bytes: manifest::NATIVE_PRODUCER_MIN_TMPFS_QUOTA_BYTES,
            tmpfs_inode_limit: 65_536,
            working_ram_bytes: 2 * 1024 * 1024 * 1024,
            max_build_seconds: 30 * 60,
            max_source_members: manifest::NATIVE_PRODUCER_MAX_MEMBERS as u64,
            max_source_bytes: manifest::NATIVE_PRODUCER_MAX_SOURCE_CLOSURE_BYTES,
            max_member_bytes: 8 * 1024 * 1024,
            max_manifest_bytes: 8 * 1024 * 1024,
            max_capture_member_read_bytes: 64 * 1024 * 1024,
            max_capture_write_bytes: 256 * 1024 * 1024,
            max_recheck_read_bytes: 1024 * 1024 * 1024,
            max_callback_and_fence_state_bytes: 1024 * 1024 * 1024,
            callback_owned_heap_state_upper_bound_bytes: 512 * 1024 * 1024,
            final_fence_workspace_upper_bound_bytes: 256 * 1024 * 1024,
            max_state_bytes: 512 * 1024 * 1024,
            max_json_visits: 8_000_000,
            max_work_bytes: 8 * 1024 * 1024 * 1024,
            max_output_bytes: manifest::NATIVE_PRODUCER_MAX_DATA_BYTES,
            max_schema_receipts: 4096,
            max_schema_receipt_bytes: 4 * 1024 * 1024,
            worker_cpu_seconds: 1800,
            worker_address_space_bytes: 1024 * 1024 * 1024,
            max_worker_image_bytes: 512 * 1024 * 1024,
            cold_open: ColdOpenLimits {
                max_file_bytes: manifest::NATIVE_PRODUCER_MAX_MODEL_BYTES,
                max_vm_steps: MAX_COLD_VM_STEPS,
                sqlite_cache_kib: 64 * 1024,
                max_rows: MAX_COLD_ROWS,
                max_work_bytes: MAX_COLD_WORK_BYTES,
                max_row_bytes: MAX_COLD_ROW_BYTES,
                max_metadata_bytes: MAX_COLD_METADATA_BYTES,
                max_sources: MAX_COLD_SOURCES,
            },
            process_limits: NativeProcessLimits {
                address_space_bytes: 2 * 1024 * 1024 * 1024,
                file_size_bytes: manifest::NATIVE_PRODUCER_MAX_DATA_BYTES,
            },
        }
    }
}

/// One direct repository source cut plus a separately selected executable
/// image. Software components are captured from the exact Git identity;
/// authored ToS bytes are captured from the held live source root.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectRepositoryProjectionRequest {
    pub repository_root: PathBuf,
    pub software_git_commit: String,
    pub software_git_tree: String,
    pub schema_worker_absolute_path: PathBuf,
    pub schema_worker_sha256: String,
    pub limits: DirectRepositoryProjectionLimits,
}

pub(crate) struct DirectRepositoryProjectionContext<'a> {
    pub cut: &'a CorpusCutReader,
    pub software: &'a SoftwareCaptureReader,
    pub components: &'a SoftwareComponentSelectionV1,
    pub recheck: &'a dyn Fn() -> tos_compiler::Result<()>,
    pub deadline: Instant,
    pub cancelled: &'a AtomicBool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectRepositoryProjectionCheckInput {
    schema_version: String,
    projection: DirectRepositoryProjectionRequest,
}

const ASSESSED_CANDIDATE_REQUEST_SCHEMA: &str = "tos_native_assessed_candidate_request_v1";
const ASSESSED_CANDIDATE_RESULT_SCHEMA: &str = "tos_native_assessed_candidate_result_v1";
const MAX_ASSESSED_CANDIDATE_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssessedCandidateRequest {
    schema_version: String,
    operation: String,
    product: String,
    projection: DirectRepositoryProjectionRequest,
    native_invocation: PinnedAssessmentInvocation,
    assessed_form_ids: Vec<String>,
    output: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedAssessmentInvocation {
    path: PathBuf,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AssessedCarrierSelection {
    form_ref: Value,
    subject_ref: Value,
    source_path: String,
    form_path: String,
}

struct SelectedAssessmentInvocation {
    path: PathBuf,
    raw: Vec<u8>,
    sha256: Digest256,
    stamp: Stamp,
    value: Value,
    store: CorpusReader,
    software: SoftwareCaptureReader,
    components: SoftwareComponentSelectionV1,
    source_revision: String,
}

#[derive(Debug)]
struct Refusal(&'static str);
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl StdError for Refusal {}
pub(crate) type Result<T> = std::result::Result<T, Box<dyn StdError>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    comparison_root: Option<String>,
    #[serde(default)]
    selected_snapshot: Option<manifest::NativeSelectedSnapshotProfile>,
    #[serde(default)]
    source_only: Option<NativeSourceOnlyRequest>,
    tmpfs_quota_bytes: u64,
    tmpfs_inode_limit: u64,
    working_ram_bytes: u64,
    /// Original simultaneous producer Rust/SQLite state, separate from file caps.
    max_state_bytes: usize,
    max_json_visits: usize,
    /// One cumulative capture/build work counter; old requests retain 16 GiB.
    #[serde(default = "default_producer_work_bytes")]
    max_work_bytes: u64,
    persistent_write_cap_bytes: u64,
    max_build_seconds: u64,
    cold_open: ColdOpenLimits,
    process_limits: NativeProcessLimits,
    data_directory: String,
    private_release_directory: String,
    #[serde(default)]
    evidence_refs: Vec<EvidenceRefInput>,
    #[serde(default)]
    previous_native_snapshot: Option<manifest::NativePreviousDataSnapshotProfile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSourceOnlyRequest {
    corpus_store: String,
    source_revision: String,
    max_revisions: usize,
    max_members: u64,
    max_total_bytes: u64,
    max_member_bytes: u64,
    software_capture: String,
    software_restored_root: String,
    source_git_commit: String,
    source_git_tree: String,
    capture_manifest_sha256: String,
    software_components: Vec<String>,
    schema_worker_path: String,
    schema_worker_absolute_path: String,
    schema_worker_sha256: String,
    max_schema_receipts: usize,
    max_schema_receipt_bytes: usize,
    worker_cpu_seconds: u64,
    worker_address_space_bytes: u64,
}

impl NativeSourceOnlyRequest {
    fn retained_state_upper_bound(&self) -> Result<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.corpus_store.capacity())
            .and_then(|n| n.checked_add(self.source_revision.capacity()))
            .and_then(|n| n.checked_add(self.software_capture.capacity()))
            .and_then(|n| n.checked_add(self.software_restored_root.capacity()))
            .and_then(|n| n.checked_add(self.source_git_commit.capacity()))
            .and_then(|n| n.checked_add(self.source_git_tree.capacity()))
            .and_then(|n| n.checked_add(self.capture_manifest_sha256.capacity()))
            .and_then(|n| {
                n.checked_add(self.software_components.capacity() * std::mem::size_of::<String>())
            })
            .and_then(|n| n.checked_add(self.schema_worker_path.capacity()))
            .and_then(|n| n.checked_add(self.schema_worker_absolute_path.capacity()))
            .and_then(|n| n.checked_add(self.schema_worker_sha256.capacity()))
            .ok_or(Refusal(
                "native source-only request retained slots overflow",
            ))?;
        for component in &self.software_components {
            bytes = bytes.checked_add(component.capacity()).ok_or(Refusal(
                "native source-only request retained strings overflow",
            ))?;
        }
        Ok(bytes)
    }
}

fn default_producer_work_bytes() -> u64 {
    tos_compiler::Limits::default().max_work_bytes
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceRefInput {
    kind: String,
    path: String,
    sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stamp {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
    size: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
impl From<&fs::Metadata> for Stamp {
    fn from(value: &fs::Metadata) -> Self {
        Self {
            dev: value.dev(),
            ino: value.ino(),
            uid: value.uid(),
            mode: value.mode(),
            size: value.len(),
            mtime: value.mtime(),
            mtime_nsec: value.mtime_nsec(),
            ctime: value.ctime(),
            ctime_nsec: value.ctime_nsec(),
        }
    }
}

#[derive(Clone, Copy)]
struct PinObservation {
    stamp: Stamp,
    producer_elapsed_ns: u64,
}

#[derive(Default)]
struct PinObservationState {
    first: Option<PinObservation>,
    last: Option<PinObservation>,
    verify_calls: u8,
}

/// Observe metadata on the exact descriptor passed to the maintained custody
/// hook while delegating all actual fs-verity and resource enforcement.
struct ObservingNativeCustody {
    inner: LinuxFsVerityCustody,
    producer_started: Instant,
    observations: Mutex<PinObservationState>,
}

impl ObservingNativeCustody {
    fn new(inner: LinuxFsVerityCustody, producer_started: Instant) -> Self {
        Self {
            inner,
            producer_started,
            observations: Mutex::new(PinObservationState::default()),
        }
    }

    fn snapshot(&self) -> Result<(PinObservation, PinObservation, u8)> {
        let state = self
            .observations
            .lock()
            .map_err(|_| Refusal("native custody observation lock poisoned"))?;
        Ok((
            state
                .first
                .ok_or(Refusal("native held-FD first observation absent"))?,
            state
                .last
                .ok_or(Refusal("native held-FD last observation absent"))?,
            state.verify_calls,
        ))
    }
}

impl ImmutableKnowledgeCustody for ObservingNativeCustody {
    fn verify(
        &self,
        pinned: &File,
        expected: &KnowledgeSelectedExpectation,
    ) -> std::result::Result<(), tos_compiler::Error> {
        self.inner.verify(pinned, expected)?;
        let observation = PinObservation {
            stamp: Stamp::from(&pinned.metadata()?),
            producer_elapsed_ns: elapsed_ns(self.producer_started),
        };
        let mut state = self
            .observations
            .lock()
            .map_err(|_| tos_compiler::Error::Invalid("native custody observation lock"))?;
        if state.verify_calls >= 8 {
            return Err(tos_compiler::Error::Budget(
                "native custody observation count",
            ));
        }
        if state.first.is_none() {
            state.first = Some(observation);
        }
        state.last = Some(observation);
        state.verify_calls += 1;
        Ok(())
    }

    fn verify_cold_resources(
        &self,
        limits: ColdOpenLimits,
    ) -> std::result::Result<(), tos_compiler::Error> {
        self.inner.verify_cold_resources(limits)
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    match u64::try_from(started.elapsed().as_nanos()) {
        Ok(value) => value,
        Err(_) => u64::MAX,
    }
}

fn stamp_json(stamp: Stamp) -> Value {
    json!({
        "device": stamp.dev,
        "inode": stamp.ino,
        "uid": stamp.uid,
        "mode": stamp.mode,
        "size_bytes": stamp.size,
        "mtime_seconds": stamp.mtime,
        "mtime_nanoseconds": stamp.mtime_nsec,
        "ctime_seconds": stamp.ctime,
        "ctime_nanoseconds": stamp.ctime_nsec,
    })
}

fn private_model_path_stamp(path: &Path, uid: u32, expected_size: u64) -> Result<Stamp> {
    no_symlink_path(path)?;
    let metadata = fs::symlink_metadata(path)?;
    let stamp = Stamp::from(&metadata);
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || stamp.uid != uid
        || stamp.mode & 0o077 != 0
        || stamp.size != expected_size
    {
        return Err(Refusal("native cold-open named model custody differs").into());
    }
    Ok(stamp)
}

struct HeldEvidenceRef {
    kind: String,
    path: PathBuf,
    file: File,
    stamp: Stamp,
    sha256: String,
}
impl HeldEvidenceRef {
    fn output(&self) -> Value {
        json!({
            "kind": self.kind,
            "path": self.path,
            "size_bytes": self.stamp.size,
            "sha256": self.sha256,
            "authority": "explicit external reference; not interpreted as an admission",
        })
    }
    fn recheck(&mut self, deadline: Instant) -> Result<()> {
        active(deadline)?;
        if Stamp::from(&self.file.metadata()?) != self.stamp
            || Stamp::from(&fs::symlink_metadata(&self.path)?) != self.stamp
        {
            return Err(Refusal("external evidence reference custody changed").into());
        }
        self.file.seek(SeekFrom::Start(0))?;
        let mut hasher = Digest256Hasher::new();
        let mut total = 0u64;
        let mut block = [0u8; 64 * 1024];
        loop {
            active(deadline)?;
            let count = self.file.read(&mut block)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|value| *value <= MAX_EVIDENCE_REF_BYTES)
                .ok_or(Refusal("external evidence reference byte ceiling"))?;
            hasher.update(&block[..count]);
        }
        if total != self.stamp.size
            || hasher.finalize().to_hex() != self.sha256
            || Stamp::from(&self.file.metadata()?) != self.stamp
            || Stamp::from(&fs::symlink_metadata(&self.path)?) != self.stamp
        {
            return Err(Refusal("external evidence reference changed after hold").into());
        }
        active(deadline)
    }
}

struct TempStagePaths {
    capture: PathBuf,
    native: PathBuf,
}
impl Drop for TempStagePaths {
    fn drop(&mut self) {
        for path in [&self.capture, &self.native] {
            for suffix in ["-journal", "-wal", "-shm", ""] {
                let mut name = path.as_os_str().to_os_string();
                name.push(suffix);
                let _ = fs::remove_file(PathBuf::from(name));
            }
        }
    }
}

struct OutputTree {
    root_path: PathBuf,
    root: File,
    store: File,
    byte_cap: u64,
    written_bytes: u64,
    uid: u32,
}
impl OutputTree {
    fn reserve(&mut self, bytes: u64) -> Result<()> {
        self.written_bytes = self
            .written_bytes
            .checked_add(bytes)
            .filter(|value| *value <= self.byte_cap)
            .ok_or(Refusal("persistent candidate write cap exceeded"))?;
        Ok(())
    }

    fn write_member(&mut self, relative: &str, raw: &[u8], deadline: Instant) -> Result<()> {
        let size = u64::try_from(raw.len())?;
        self.reserve(size)?;
        let (parent, leaf) = self.parent_for_member(relative)?;
        let mut output: File = rustix::fs::openat(
            &parent,
            leaf.as_str(),
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(io::Error::from)?;
        let before = output.metadata()?;
        if !before.is_file()
            || before.uid() != self.uid
            || before.mode() & 0o077 != 0
            || before.len() != 0
        {
            return Err(Refusal("private candidate output file custody").into());
        }
        for chunk in raw.chunks(64 * 1024) {
            active(deadline)?;
            output.write_all(chunk)?;
        }
        output.sync_all()?;
        if output.metadata()?.len() != size {
            return Err(Refusal("private candidate output size differs").into());
        }
        parent.sync_all()?;
        Ok(())
    }

    fn copy_model_once(
        &mut self,
        source_path: &Path,
        source_digest: &str,
        expected_size: u64,
        deadline: Instant,
    ) -> Result<()> {
        if expected_size == 0 || expected_size > manifest::NATIVE_PRODUCER_MAX_MODEL_BYTES {
            return Err(Refusal("native model output size ceiling").into());
        }
        self.reserve(expected_size)?;
        no_symlink_path(source_path)?;
        let mut source = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(source_path)?;
        let source_before = source.metadata()?;
        let named_before = fs::symlink_metadata(source_path)?;
        let source_stamp = Stamp::from(&source_before);
        if !source_before.is_file()
            || named_before.file_type().is_symlink()
            || Stamp::from(&named_before) != source_stamp
            || source_before.len() != expected_size
        {
            return Err(Refusal("completed native model custody differs").into());
        }
        let (parent, leaf) = self.parent_for_member(manifest::NATIVE_MODEL_PATH)?;
        let mut output: File = rustix::fs::openat(
            &parent,
            leaf.as_str(),
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(io::Error::from)?;
        let mut hash = Digest256Hasher::new();
        let mut total = 0u64;
        let mut block = [0u8; 64 * 1024];
        loop {
            active(deadline)?;
            let count = source.read(&mut block)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|value| *value <= expected_size)
                .ok_or(Refusal("completed native model read ceiling"))?;
            output.write_all(&block[..count])?;
            hash.update(&block[..count]);
        }
        output.sync_all()?;
        let digest = hash.finalize().to_hex();
        let output_metadata = output.metadata()?;
        if total != expected_size
            || digest != source_digest
            || Stamp::from(&source.metadata()?) != source_stamp
            || Stamp::from(&fs::symlink_metadata(source_path)?) != source_stamp
            || !output_metadata.is_file()
            || output_metadata.uid() != self.uid
            || output_metadata.mode() & 0o077 != 0
            || output_metadata.len() != expected_size
        {
            return Err(Refusal("completed native model changed during copy").into());
        }
        parent.sync_all()?;
        Ok(())
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn verify_root(&self) -> Result<()> {
        let held = self.root.metadata()?;
        let named = fs::symlink_metadata(&self.root_path)?;
        if !held.is_dir()
            || held.uid() != self.uid
            || held.mode() & 0o077 != 0
            || Stamp::from(&held) != Stamp::from(&named)
            || named.file_type().is_symlink()
        {
            return Err(Refusal("private output root custody changed").into());
        }
        Ok(())
    }

    fn parent_for_member(&self, relative: &str) -> Result<(File, String)> {
        let rel = RelativePath::parse(relative)
            .map_err(|_| Refusal("native candidate relative member path"))?;
        let parts = rel.as_str().split('/').collect::<Vec<_>>();
        if parts.len() < 2 || parts[0] != "data" || parts.len() > 16 {
            return Err(Refusal("native candidate member depth or root").into());
        }
        let mut directory = rustix::fs::openat(
            &self.root,
            "data",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(io::Error::from)?;
        for &component in &parts[1..parts.len() - 1] {
            match rustix::fs::mkdirat(
                &directory,
                component,
                rustix::fs::Mode::from_raw_mode(0o700),
            ) {
                Ok(()) => directory.sync_all()?,
                Err(rustix::io::Errno::EXIST) => (),
                Err(error) => return Err(io::Error::from(error).into()),
            }
            let next: File = rustix::fs::openat(
                &directory,
                component,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(io::Error::from)?;
            let metadata = next.metadata()?;
            if !metadata.is_dir() || metadata.uid() != self.uid || metadata.mode() & 0o077 != 0 {
                return Err(Refusal("private candidate nested directory custody").into());
            }
            directory = next;
        }
        Ok((directory, parts[parts.len() - 1].to_owned()))
    }
}

fn active(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        return Err(Refusal("whole producer deadline expired").into());
    }
    Ok(())
}

fn no_symlink_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(Refusal("producer path must be absolute and normalized").into());
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part.as_os_str());
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(Refusal("producer path contains symlink").into());
        }
    }
    Ok(())
}

fn json_exact(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or(Refusal("assessed candidate selected JSON object"))?;
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err(Refusal("assessed candidate selected JSON fields differ").into());
    }
    Ok(())
}

fn json_text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Refusal("assessed candidate selected text field").into())
}

fn json_positive(value: &Value, key: &str, max: u64) -> Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0 && *value <= max)
        .ok_or_else(|| Refusal("assessed candidate selected budget").into())
}

fn selected_assessment_invocation(
    pinned: &PinnedAssessmentInvocation,
    projection: &DirectRepositoryProjectionRequest,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SelectedAssessmentInvocation> {
    let uid = rustix::process::getuid().as_raw();
    let path_text = pinned
        .path
        .to_str()
        .ok_or(Refusal("assessed candidate invocation path UTF-8"))?;
    if !absolute_bounded(path_text) {
        return Err(Refusal("assessed candidate invocation path invalid").into());
    }
    let raw = crate::source_text_owner::read_absolute(
        &pinned.path,
        uid,
        true,
        1_048_576,
        deadline,
        cancelled,
    )?;
    let (mut file, stamp) = open_regular(&pinned.path, 1_048_576, uid)?;
    let mut held_raw = Vec::with_capacity(raw.len());
    (&mut file).take(1_048_577).read_to_end(&mut held_raw)?;
    if held_raw != raw
        || Stamp::from(&file.metadata()?) != stamp
        || Stamp::from(&fs::symlink_metadata(&pinned.path)?) != stamp
    {
        return Err(Refusal("assessed candidate invocation custody differs").into());
    }
    let sha256 = Digest256::from_prefixed(&pinned.sha256)
        .map_err(|_| Refusal("assessed candidate invocation SHA-256 invalid"))?;
    if Digest256::of_bytes(&raw) != sha256 {
        return Err(Refusal("assessed candidate invocation pin differs").into());
    }
    let parsed = parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 1_048_576,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Refusal("assessed candidate invocation JSON"))?;
    let value: Value = serde_json::from_slice(&raw)
        .map_err(|_| Refusal("assessed candidate invocation transport JSON"))?;
    let _ = parsed;
    json_exact(
        &value,
        &[
            "schema_version",
            "owner_config",
            "native_executable",
            "native_executable_sha256",
            "corpus_store",
            "source_revision",
            "original_source_revision",
            "software_capture",
            "software_restored_root",
            "software_selection",
            "software_components",
            "schema_worker",
            "assessment_schema_worker",
            "budgets",
        ],
    )?;
    if json_text(&value, "schema_version")? != "tos_local_native_assessment_read_invocation_v1" {
        return Err(
            Refusal("assessed candidate requires the native assessment read invocation").into(),
        );
    }
    for field in [
        "owner_config",
        "native_executable",
        "corpus_store",
        "software_capture",
        "software_restored_root",
    ] {
        let path = Path::new(json_text(&value, field)?);
        if !path.is_absolute() || path.as_os_str().len() > 4096 {
            return Err(Refusal("assessed candidate invocation path field").into());
        }
    }
    let source_revision = json_text(&value, "source_revision")?.to_owned();
    let revision = Digest256::from_prefixed(&source_revision)
        .map_err(|_| Refusal("assessed candidate invocation source revision"))?;
    if json_text(&value, "original_source_revision")? != source_revision {
        return Err(Refusal("assessed candidate original source selection differs").into());
    }
    let selection = value
        .get("software_selection")
        .ok_or(Refusal("assessed candidate invocation software selection"))?;
    json_exact(
        selection,
        &[
            "source_git_commit",
            "source_git_tree",
            "capture_manifest_sha256",
        ],
    )?;
    if json_text(selection, "source_git_commit")? != projection.software_git_commit
        || json_text(selection, "source_git_tree")? != projection.software_git_tree
    {
        return Err(Refusal("assessed candidate native software selection differs").into());
    }
    Digest256::from_prefixed(json_text(selection, "capture_manifest_sha256")?)
        .map_err(|_| Refusal("assessed candidate software manifest digest"))?;
    for field in ["schema_worker", "assessment_schema_worker"] {
        let worker = value
            .get(field)
            .ok_or(Refusal("assessed candidate selected schema worker"))?;
        json_exact(worker, &["absolute_path", "sha256"])?;
        let worker_path = Path::new(json_text(worker, "absolute_path")?);
        if !worker_path.is_absolute() || worker_path.as_os_str().len() > 4096 {
            return Err(Refusal("assessed candidate schema worker path").into());
        }
        Digest256::from_prefixed(json_text(worker, "sha256")?)
            .map_err(|_| Refusal("assessed candidate schema worker digest"))?;
    }
    let budgets = value
        .get("budgets")
        .ok_or(Refusal("assessed candidate invocation budgets"))?;
    json_exact(
        budgets,
        &[
            "max_revisions",
            "max_members",
            "max_total_bytes",
            "max_member_bytes",
            "max_schema_receipts",
            "max_schema_receipt_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
        ],
    )?;
    for (field, max) in [
        ("max_revisions", 4),
        ("max_members", 2048),
        ("max_total_bytes", 33_554_432),
        ("max_member_bytes", 8_388_608),
        ("max_schema_receipts", 128),
        ("max_schema_receipt_bytes", 262_144),
        ("worker_cpu_seconds", 3),
        ("worker_address_space_bytes", 1_073_741_824),
    ] {
        json_positive(budgets, field, max)?;
    }
    if crate::source_serialization::executable(deadline, cancelled)?
        != Digest256::from_prefixed(json_text(&value, "native_executable_sha256")?)
            .map_err(|_| Refusal("assessed candidate native executable digest"))?
    {
        return Err(Refusal(
            "assessed candidate invocation does not pin the running native command",
        )
        .into());
    }
    let components_raw = value
        .get("software_components")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= 128)
        .ok_or(Refusal("assessed candidate invocation software components"))?;
    let mut component_paths = Vec::with_capacity(components_raw.len());
    let mut seen_components = BTreeSet::new();
    for item in components_raw {
        let path = item
            .as_str()
            .ok_or(Refusal("assessed candidate software component path"))?;
        let relative = RelativePath::parse(path)
            .map_err(|_| Refusal("assessed candidate software component path"))?;
        if !seen_components.insert(path.to_owned()) {
            return Err(Refusal("assessed candidate duplicate software component").into());
        }
        component_paths.push(relative);
    }
    let read_limits = ReadLimits {
        max_manifest_bytes: 4_194_304,
        max_manifest_entries: 2048,
        max_selected_object_bytes: 8_388_608,
        json: JsonLimits::default(),
    };
    let store =
        CorpusReader::open_existing(Path::new(json_text(&value, "corpus_store")?), read_limits)
            .map_err(|_| Refusal("assessed candidate native corpus store"))?;
    let current = store
        .select_current()
        .map_err(|_| Refusal("assessed candidate native corpus current pointer"))?
        .ok_or(Refusal(
            "assessed candidate native corpus current revision absent",
        ))?;
    if current.0 != revision {
        return Err(Refusal("assessed candidate native corpus revision changed").into());
    }
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: json_text(selection, "source_git_commit")?.to_owned(),
        source_git_tree: json_text(selection, "source_git_tree")?.to_owned(),
        capture_manifest_sha256: Digest256::from_prefixed(json_text(
            selection,
            "capture_manifest_sha256",
        )?)
        .map_err(|_| Refusal("assessed candidate software manifest digest"))?,
    };
    let software = SoftwareCaptureReader::open(
        Path::new(json_text(&value, "software_capture")?),
        Path::new(json_text(&value, "software_restored_root")?),
        selection,
        read_limits,
        deadline,
        cancelled,
    )
    .map_err(|_| Refusal("assessed candidate selected software capture"))?;
    let components = software
        .select_components(&component_paths)
        .map_err(|_| Refusal("assessed candidate selected software components"))?;
    Ok(SelectedAssessmentInvocation {
        path: pinned.path.clone(),
        raw,
        sha256,
        stamp,
        value,
        store,
        software,
        components,
        source_revision,
    })
}

impl SelectedAssessmentInvocation {
    fn verify_current(&self, deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        let uid = rustix::process::getuid().as_raw();
        let raw = crate::source_text_owner::read_absolute(
            &self.path, uid, true, 1_048_576, deadline, cancelled,
        )?;
        let (_, stamp) = open_regular(&self.path, 1_048_576, uid)?;
        let revision = Digest256::from_prefixed(&self.source_revision)
            .map_err(|_| Refusal("assessed candidate source revision pin"))?;
        let current = self
            .store
            .select_current()
            .map_err(|_| Refusal("assessed candidate native corpus current pointer"))?
            .ok_or(Refusal(
                "assessed candidate native corpus current revision absent",
            ))?;
        if raw != self.raw
            || Digest256::of_bytes(&raw) != self.sha256
            || stamp != self.stamp
            || current.0 != revision
            || crate::source_serialization::executable(deadline, cancelled)?
                != Digest256::from_prefixed(json_text(&self.value, "native_executable_sha256")?)
                    .map_err(|_| Refusal("assessed candidate native executable pin"))?
        {
            return Err(Refusal("assessed candidate invocation or selected source changed").into());
        }
        Ok(())
    }
}

fn read_assessed_candidate_request(mut input: impl Read) -> Result<AssessedCandidateRequest> {
    let mut raw = Vec::new();
    input
        .by_ref()
        .take((MAX_ASSESSED_CANDIDATE_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut raw)?;
    if raw.len() > MAX_ASSESSED_CANDIDATE_REQUEST_BYTES {
        return Err(Refusal("assessed candidate request byte limit").into());
    }
    parse_assessed_candidate_request(&raw)
}

fn parse_assessed_candidate_request(raw: &[u8]) -> Result<AssessedCandidateRequest> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: MAX_ASSESSED_CANDIDATE_REQUEST_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Refusal("assessed candidate request JSON"))?;
    let request: AssessedCandidateRequest = serde_json::from_slice(raw)
        .map_err(|_| Refusal("assessed candidate request transport JSON"))?;
    if request.schema_version != ASSESSED_CANDIDATE_REQUEST_SCHEMA
        || !matches!(request.operation.as_str(), "publish" | "check")
        || !matches!(
            request.product.as_str(),
            "bibliographic" | "corpus" | "paired"
        )
        || request.assessed_form_ids.is_empty()
        || request.assessed_form_ids.len() > 256
        || !absolute_bounded(
            request
                .output
                .to_str()
                .ok_or(Refusal("assessed candidate output path UTF-8"))?,
        )
    {
        return Err(Refusal("assessed candidate request selection differs").into());
    }
    let mut seen = BTreeSet::new();
    for identity in &request.assessed_form_ids {
        let mut parts = identity.split('.');
        if parts.next() != Some("tos")
            || parts.next() != Some("form")
            || parts.next().is_none()
            || identity.len() > 256
            || identity.bytes().any(|byte| {
                !(byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || byte == b'.'
                    || byte == b'-')
            })
            || identity.split('.').any(str::is_empty)
            || !seen.insert(identity.clone())
        {
            return Err(Refusal(
                "assessed candidate form IDs must be distinct ToS form identities",
            )
            .into());
        }
    }
    validate_direct_projection_request(&request.projection)?;
    Ok(request)
}

fn read_assessed_candidate_request_file(path: &Path) -> Result<AssessedCandidateRequest> {
    if !path.is_absolute() {
        return Err(Refusal("assessed candidate request file must be absolute").into());
    }
    let uid = rustix::process::getuid().as_raw();
    let (mut file, stamp) = open_regular(path, MAX_ASSESSED_CANDIDATE_REQUEST_BYTES as u64, uid)?;
    let mut raw = Vec::with_capacity(
        usize::try_from(stamp.size)
            .map_err(|_| Refusal("assessed candidate request size range"))?,
    );
    (&mut file)
        .take((MAX_ASSESSED_CANDIDATE_REQUEST_BYTES as u64).saturating_add(1))
        .read_to_end(&mut raw)?;
    if raw.len() as u64 != stamp.size
        || Stamp::from(&file.metadata()?) != stamp
        || Stamp::from(&fs::symlink_metadata(path)?) != stamp
    {
        return Err(Refusal("assessed candidate request file changed while reading").into());
    }
    parse_assessed_candidate_request(&raw)
}

fn validate_candidate_target(target: &Path, repository_root: &Path) -> Result<()> {
    let text = target
        .to_str()
        .ok_or(Refusal("assessed candidate target path UTF-8"))?;
    if !absolute_bounded(text)
        || target.extension().and_then(|value| value.to_str()) != Some("json")
        || target.file_name().is_none()
    {
        return Err(Refusal("assessed candidate target must be normalized absolute JSON").into());
    }
    if let Ok(relative) = target.strip_prefix(repository_root) {
        if relative
            .components()
            .next()
            .and_then(|part| part.as_os_str().to_str())
            != Some(".git")
        {
            return Err(Refusal(
                "assessed candidate target must be outside authored repository sources",
            )
            .into());
        }
    }
    if target == repository_root.join(manifest::CORPUS_INDEX_PATH)
        || target == repository_root.join(manifest::CLAIM_GRAPH_PATH)
    {
        return Err(Refusal("assessed candidate cannot replace a standard product").into());
    }
    Ok(())
}

fn validate_record_reference(reference: &Value) -> Result<()> {
    json_exact(reference, &["id", "version", "digest"])?;
    if json_text(reference, "id")?.is_empty() {
        return Err(Refusal("assessed candidate record reference identity").into());
    }
    let digest = json_text(reference, "digest")?;
    if Digest256::from_prefixed(digest).is_err() {
        return Err(Refusal("assessed candidate record reference digest").into());
    }
    if !reference["version"].is_string() && !reference["version"].is_number() {
        return Err(Refusal("assessed candidate record reference version").into());
    }
    Ok(())
}

fn selections_from_nodes(
    nodes: &[Value],
    wanted: &BTreeSet<String>,
) -> Result<BTreeMap<String, AssessedCarrierSelection>> {
    let mut selected = BTreeMap::new();
    for node in nodes {
        let properties = node
            .get("properties")
            .and_then(Value::as_object)
            .ok_or(Refusal("assessed candidate node properties"))?;
        let Some(forms) = properties.get("human_forms").and_then(Value::as_array) else {
            continue;
        };
        for packet in forms {
            let Some(identity) = packet
                .get("form")
                .and_then(|form| form.get("id"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            if !wanted.contains(identity) {
                continue;
            }
            let has_source = properties.contains_key("source_record");
            let has_claim = properties.contains_key("source_claim");
            if has_source == has_claim {
                return Err(
                    Refusal("assessed candidate form requires one exact source carrier").into(),
                );
            }
            let form_ref = packet
                .get("form")
                .ok_or(Refusal("assessed candidate form reference"))?;
            let subject_ref = packet
                .get("subject")
                .ok_or(Refusal("assessed candidate subject reference"))?;
            validate_record_reference(form_ref)?;
            validate_record_reference(subject_ref)?;
            if json_text(form_ref, "id")? != identity {
                return Err(Refusal("assessed candidate form identity differs").into());
            }
            let source_path = node
                .get("source_ref")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(Refusal("assessed candidate source path"))?
                .to_owned();
            let form_path = properties
                .get("human_forms_source_ref")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(Refusal("assessed candidate form-set path"))?
                .to_owned();
            let subject_digest = json_text(subject_ref, "digest")?
                .strip_prefix("sha256:")
                .ok_or(Refusal("assessed candidate subject digest prefix"))?;
            let carrier_digest = node
                .get("source_sha256")
                .or_else(|| properties.get("source_sha256"))
                .and_then(Value::as_str)
                .ok_or(Refusal("assessed candidate source digest"))?;
            if carrier_digest != subject_digest {
                return Err(
                    Refusal("assessed candidate source body and subject digest disagree").into(),
                );
            }
            let selection = AssessedCarrierSelection {
                form_ref: form_ref.clone(),
                subject_ref: subject_ref.clone(),
                source_path,
                form_path,
            };
            if selected.insert(identity.to_owned(), selection).is_some() {
                return Err(
                    Refusal("assessed candidate form must have one exact source carrier").into(),
                );
            }
        }
    }
    Ok(selected)
}

fn native_assessed_batch_request(selections: &[AssessedCarrierSelection]) -> Result<Vec<u8>> {
    let request = json!({
        "schema_version":"tos_local_assessed_forms_materialization_request_v1",
        "operation":"materialize_assessed_forms",
        "selections": selections.iter().map(|selection| json!({
            "form_ref": selection.form_ref,
            "subject_ref": selection.subject_ref,
            "source_path": selection.source_path,
            "form_path": selection.form_path,
        })).collect::<Vec<_>>(),
    });
    let raw = serde_json::to_vec(&request)?;
    if raw.is_empty() || raw.len() > 1_048_576 {
        return Err(Refusal("assessed candidate native batch request budget").into());
    }
    Ok(raw)
}

fn assessed_packets(
    batch: &Value,
    expected: &[AssessedCarrierSelection],
) -> Result<BTreeMap<String, Value>> {
    if batch.get("schema_version").and_then(Value::as_str)
        != Some("tos_local_assessed_forms_materialization_result_v1")
    {
        return Err(Refusal("assessed candidate native batch result schema").into());
    }
    let snapshot = json_text(batch, "owner_snapshot")?.to_owned();
    Digest256::from_prefixed(&snapshot)
        .map_err(|_| Refusal("assessed candidate owner snapshot digest"))?;
    let replies = batch
        .get("replies")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == expected.len())
        .ok_or(Refusal("assessed candidate native batch reply count"))?;
    let mut packets = BTreeMap::new();
    for (selection, reply) in expected.iter().zip(replies) {
        if reply.get("schema_version").and_then(Value::as_str)
            != Some("tos_local_assessment_result_v1")
            || reply.get("owner_snapshot").and_then(Value::as_str) != Some(snapshot.as_str())
            || reply.get("authentication").and_then(Value::as_str) != Some("local-unix-account")
        {
            return Err(Refusal("assessed candidate native reply envelope").into());
        }
        let result = reply
            .get("result")
            .ok_or(Refusal("assessed candidate native reply result"))?;
        let packet = result
            .get("materialization")
            .filter(|packet| {
                packet.get("schema_version").and_then(Value::as_str)
                    == Some("tos_human_form_materialization_v1")
            })
            .ok_or(Refusal("assessed candidate materialization packet"))?;
        if packet.get("form") != Some(&selection.form_ref)
            || packet.get("subject") != Some(&selection.subject_ref)
        {
            return Err(
                Refusal("assessed candidate returned materialization binding differs").into(),
            );
        }
        let mut candidate_packet = packet.clone();
        let mut assessment_snapshot = Map::new();
        assessment_snapshot.insert("owner_snapshot".into(), Value::String(snapshot.clone()));
        assessment_snapshot.insert(
            "journal_revision".into(),
            result
                .get("revision")
                .cloned()
                .ok_or(Refusal("assessed candidate journal revision"))?,
        );
        assessment_snapshot.insert(
            "journal_batches".into(),
            result
                .get("batch_count")
                .cloned()
                .ok_or(Refusal("assessed candidate journal batch count"))?,
        );
        assessment_snapshot.insert("publication_authorized".into(), Value::Bool(false));
        assessment_snapshot.insert("current_runtime_grant".into(), Value::Bool(false));
        if candidate_packet.get("subject_assessment").is_some() {
            assessment_snapshot.insert("subject_assessment_required".into(), Value::Bool(true));
        }
        candidate_packet
            .as_object_mut()
            .ok_or(Refusal("assessed candidate materialization object"))?
            .insert(
                "assessment_snapshot".into(),
                Value::Object(assessment_snapshot),
            );
        if serde_json::to_vec(&candidate_packet)?.len() > 65_536 {
            return Err(Refusal("assessed candidate materialization packet byte limit").into());
        }
        if packets
            .insert(
                json_text(&selection.form_ref, "id")?.to_owned(),
                candidate_packet,
            )
            .is_some()
        {
            return Err(Refusal("assessed candidate duplicate returned form").into());
        }
    }
    Ok(packets)
}

fn apply_assessed_packets(
    nodes: &mut [Value],
    packets: &BTreeMap<String, Value>,
    max_bytes: usize,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for node in nodes {
        let Some(forms) = node
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .and_then(|properties| properties.get_mut("human_forms"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for packet in forms.iter_mut() {
            let Some(identity) = packet
                .get("form")
                .and_then(|form| form.get("id"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            if let Some(replacement) = packets.get(identity) {
                if !seen.insert(identity.to_owned()) {
                    return Err(Refusal(
                        "assessed candidate duplicate form carrier during rendering",
                    )
                    .into());
                }
                *packet = replacement.clone();
            }
        }
        let encoded = serde_json::to_vec(forms)?;
        if encoded.len() > max_bytes {
            return Err(Refusal("assessed candidate complete form set byte limit").into());
        }
    }
    if seen.len() != packets.len() {
        return Err(Refusal("assessed candidate rendered form coverage differs").into());
    }
    Ok(())
}

fn render_candidate(value: &Value, max_bytes: u64) -> Result<Vec<u8>> {
    let mut raw = serde_json::to_vec_pretty(value)?;
    raw.push(b'\n');
    if raw.is_empty() || raw.len() as u64 > max_bytes {
        return Err(Refusal("assessed candidate output byte limit").into());
    }
    Ok(raw)
}

struct CandidatePublication {
    directory: File,
    leaf: String,
    target: PathBuf,
    device: u64,
    inode: u64,
    committed: bool,
}

impl CandidatePublication {
    fn commit(mut self) -> PathBuf {
        self.committed = true;
        self.target.clone()
    }
}

impl Drop for CandidatePublication {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let opened: std::io::Result<File> = rustix::fs::openat(
            &self.directory,
            self.leaf.as_str(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(io::Error::from);
        if opened.is_ok_and(|file| {
            file.metadata()
                .is_ok_and(|metadata| metadata.dev() == self.device && metadata.ino() == self.inode)
        }) {
            let _ = rustix::fs::unlinkat(
                &self.directory,
                self.leaf.as_str(),
                rustix::fs::AtFlags::empty(),
            );
            let _ = self.directory.sync_all();
        }
    }
}

/// Test-only access to the real staged/no-replace candidate writer. The callback
/// runs after staging fsync and before visibility, exactly as the production
/// currentness guard does; no source or assessment authority is created.
#[cfg(feature = "conformance-owner-local-source-resolver")]
pub fn conformance_write_assessed_candidate(
    target: &Path,
    rendered: &[u8],
    after_stage_fsync: impl FnOnce() -> std::result::Result<(), Box<dyn StdError>>,
) -> std::result::Result<(), Box<dyn StdError>> {
    let publication = write_assessed_candidate_with_hook(target, rendered, after_stage_fsync)?;
    drop(publication);
    Ok(())
}

pub(crate) fn write_assessed_candidate_with_hook(
    target: &Path,
    rendered: &[u8],
    after_stage_fsync: impl FnOnce() -> Result<()>,
) -> Result<CandidatePublication> {
    if !target.is_absolute()
        || target
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        || target.extension().and_then(|value| value.to_str()) != Some("json")
        || rendered.is_empty()
        || rendered.len() > 512 * 1024 * 1024
    {
        return Err(Refusal("assessed candidate target or output bytes invalid").into());
    }
    let parent = target
        .parent()
        .ok_or(Refusal("assessed candidate target parent absent"))?;
    fs::create_dir_all(parent)?;
    no_symlink_path(parent)?;
    let directory = tos_fd_open::open_absolute_directory(parent)
        .map_err(|_| Refusal("assessed candidate target parent custody"))?;
    let directory_metadata = directory.metadata()?;
    if !directory_metadata.is_dir()
        || directory_metadata.uid() != rustix::process::getuid().as_raw()
    {
        return Err(Refusal("assessed candidate target parent ownership").into());
    }
    let leaf = target
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or(Refusal("assessed candidate target filename"))?
        .to_owned();
    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let temporary = format!(
        ".tos-assessed-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let mut stage: File = rustix::fs::openat(
        &directory,
        temporary.as_str(),
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(io::Error::from)?;
    struct StageCleanup<'a> {
        directory: &'a File,
        name: &'a str,
    }
    impl Drop for StageCleanup<'_> {
        fn drop(&mut self) {
            let _ = rustix::fs::unlinkat(self.directory, self.name, rustix::fs::AtFlags::empty());
            let _ = self.directory.sync_all();
        }
    }
    let _cleanup = StageCleanup {
        directory: &directory,
        name: &temporary,
    };
    let stage_metadata = stage.metadata()?;
    if !stage_metadata.is_file()
        || stage_metadata.uid() != rustix::process::getuid().as_raw()
        || stage_metadata.mode() & 0o077 != 0
        || stage_metadata.len() != 0
    {
        return Err(Refusal("assessed candidate staging file custody").into());
    }
    for chunk in rendered.chunks(64 * 1024) {
        stage.write_all(chunk)?;
    }
    stage.sync_all()?;
    if stage.metadata()?.len() != rendered.len() as u64 {
        return Err(Refusal("assessed candidate staging byte count differs").into());
    }
    after_stage_fsync()?;
    let (device, inode) = {
        let metadata = stage.metadata()?;
        (metadata.dev(), metadata.ino())
    };
    rustix::fs::linkat(
        &directory,
        temporary.as_str(),
        &directory,
        leaf.as_str(),
        rustix::fs::AtFlags::empty(),
    )
    .map_err(|error| {
        if error == rustix::io::Errno::EXIST {
            Refusal("assessed candidate output already exists; refusing replacement")
        } else {
            Refusal("assessed candidate atomic no-replace link failed")
        }
    })?;
    let publication = CandidatePublication {
        directory: directory.try_clone()?,
        leaf,
        target: target.to_path_buf(),
        device,
        inode,
        committed: false,
    };
    publication.directory.sync_all()?;
    drop(stage);
    rustix::fs::unlinkat(
        &publication.directory,
        temporary.as_str(),
        rustix::fs::AtFlags::empty(),
    )
    .map_err(|_| Refusal("assessed candidate staging cleanup failed"))?;
    publication.directory.sync_all()?;
    Ok(publication)
}

fn open_regular(path: &Path, cap: u64, uid: u32) -> Result<(File, Stamp)> {
    no_symlink_path(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let held = file.metadata()?;
    let named = fs::symlink_metadata(path)?;
    let stamp = Stamp::from(&held);
    if !held.is_file()
        || named.file_type().is_symlink()
        || Stamp::from(&named) != stamp
        || stamp.uid != uid
        || stamp.mode & 0o6000 != 0
        || stamp.size > cap
    {
        return Err(Refusal("producer held regular file custody or size").into());
    }
    Ok((file, stamp))
}

fn hold_evidence_refs(
    inputs: Vec<EvidenceRefInput>,
    deadline: Instant,
    uid: u32,
) -> Result<Vec<HeldEvidenceRef>> {
    if inputs.len() != 3 {
        return Err(
            Refusal("exact admission, built and verified evidence references required").into(),
        );
    }
    let required = BTreeSet::from([
        "admission".to_owned(),
        "built".to_owned(),
        "verified".to_owned(),
    ]);
    let mut seen = BTreeSet::new();
    let mut held = Vec::with_capacity(3);
    for input in inputs {
        active(deadline)?;
        if !required.contains(&input.kind) || !seen.insert(input.kind.clone()) {
            return Err(Refusal("external evidence reference labels differ").into());
        }
        let digest = Digest256::from_hex(&input.sha256)
            .map_err(|_| Refusal("external evidence reference SHA-256 invalid"))?
            .to_hex();
        let path = PathBuf::from(&input.path);
        let (mut file, stamp) = open_regular(&path, MAX_EVIDENCE_REF_BYTES, uid)?;
        let mut hash = Digest256Hasher::new();
        let mut total = 0u64;
        let mut block = [0u8; 64 * 1024];
        loop {
            active(deadline)?;
            let count = file.read(&mut block)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|value| *value <= MAX_EVIDENCE_REF_BYTES)
                .ok_or(Refusal("external evidence reference byte ceiling"))?;
            hash.update(&block[..count]);
        }
        if total != stamp.size
            || hash.finalize().to_hex() != digest
            || Stamp::from(&file.metadata()?) != stamp
            || Stamp::from(&fs::symlink_metadata(&path)?) != stamp
        {
            return Err(Refusal("external evidence reference changed or digest differs").into());
        }
        held.push(HeldEvidenceRef {
            kind: input.kind,
            path,
            file,
            stamp,
            sha256: digest,
        });
    }
    if seen != required {
        return Err(Refusal("external evidence reference set incomplete").into());
    }
    Ok(held)
}

fn validate_request(request: &Request) -> Result<()> {
    let legacy_request = request.schema_version == REQUEST_SCHEMA
        && request.selected_snapshot.is_some()
        && request.source_only.is_none()
        && request.mode.is_none()
        && request.comparison_root.is_none();
    let source_only_request = request.schema_version == CORPUS_BUILD_REQUEST_SCHEMA
        && request.selected_snapshot.is_none()
        && request.source_only.is_some()
        && matches!(request.mode.as_deref(), Some("build" | "check"))
        && match (request.mode.as_deref(), request.comparison_root.as_deref()) {
            (Some("build"), None) => true,
            (Some("check"), Some(root)) => absolute_bounded(root),
            _ => false,
        };
    if !(legacy_request || source_only_request)
        || request.tmpfs_quota_bytes < manifest::NATIVE_PRODUCER_MIN_TMPFS_QUOTA_BYTES
        || request.tmpfs_inode_limit < 4096
        || request.working_ram_bytes == 0
        || request.max_state_bytes < 131072
        || request.max_json_visits == 0
        || request.max_work_bytes == 0
        || request.max_work_bytes > MAX_PRODUCER_WORK_BYTES
        || request.max_state_bytes as u64 > request.process_limits.address_space_bytes
        || request.persistent_write_cap_bytes == 0
        || request.persistent_write_cap_bytes > manifest::NATIVE_PRODUCER_MAX_DATA_BYTES
        || request.max_build_seconds == 0
        || request.max_build_seconds > MAX_BUILD_SECONDS
        || !valid_child(&request.data_directory)
        || !valid_child(&request.private_release_directory)
        || request.data_directory == request.private_release_directory
    {
        return Err(Refusal("native Original producer request envelope invalid").into());
    }
    if let Some(profile) = &request.selected_snapshot {
        profile.validate()?;
    }
    if let Some(source) = &request.source_only {
        validate_source_only_request(source, request)?;
    }
    if let Some(previous) = &request.previous_native_snapshot {
        previous.validate()?;
    }
    let cold = request.cold_open;
    let process = request.process_limits;
    if cold.max_file_bytes == 0
        || cold.max_file_bytes > manifest::NATIVE_PRODUCER_MAX_MODEL_BYTES
        || cold.max_vm_steps == 0
        || cold.max_vm_steps > MAX_COLD_VM_STEPS
        || cold.sqlite_cache_kib == 0
        || cold.max_rows == 0
        || cold.max_rows > MAX_COLD_ROWS
        || cold.max_work_bytes == 0
        || cold.max_work_bytes > MAX_COLD_WORK_BYTES
        || cold.max_row_bytes == 0
        || cold.max_row_bytes > MAX_COLD_ROW_BYTES
        || cold.max_metadata_bytes == 0
        || cold.max_metadata_bytes > MAX_COLD_METADATA_BYTES
        || cold.max_sources == 0
        || cold.max_sources > MAX_COLD_SOURCES
        || process.address_space_bytes == 0
        || process.address_space_bytes > request.working_ram_bytes
        || process.file_size_bytes == 0
        || process.file_size_bytes > manifest::NATIVE_PRODUCER_MAX_DATA_BYTES
    {
        return Err(Refusal("native cold-open/process envelope invalid").into());
    }
    Ok(())
}

fn absolute_bounded(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && Path::new(value).is_absolute()
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

fn validate_source_only_request(source: &NativeSourceOnlyRequest, request: &Request) -> Result<()> {
    if !absolute_bounded(&source.corpus_store)
        || !absolute_bounded(&source.software_capture)
        || !absolute_bounded(&source.software_restored_root)
        || source.max_revisions == 0
        || source.max_revisions > 8
        || source.max_members == 0
        || source.max_members > manifest::NATIVE_PRODUCER_MAX_MEMBERS as u64
        || source.max_total_bytes == 0
        || source.max_total_bytes > manifest::NATIVE_PRODUCER_MAX_SOURCE_CLOSURE_BYTES
        || source.max_member_bytes == 0
        || source.max_member_bytes > source.max_total_bytes
        || source.max_member_bytes > MAX_COLD_ROW_BYTES as u64
        || source.software_components.is_empty()
        || source.software_components.len() > 128
        || source.max_schema_receipts == 0
        || source.max_schema_receipts > 4096
        || source.max_schema_receipt_bytes == 0
        || source.max_schema_receipt_bytes > 4 * 1024 * 1024
        || source.worker_cpu_seconds == 0
        || source.worker_cpu_seconds
            > tos_validation::executor::ExecutorBudget::MAX_SCALAR_CPU_SECONDS
        || source.worker_address_space_bytes < 64 * 1024 * 1024
        || source.worker_address_space_bytes > request.working_ram_bytes
        || source.schema_worker_absolute_path.len() > 4096
        || !Path::new(&source.schema_worker_absolute_path).is_absolute()
    {
        return Err(Refusal("native source-only selection or budget invalid").into());
    }
    Digest256::from_hex(&source.source_revision)
        .map_err(|_| Refusal("native selected source revision invalid"))?;
    Digest256::from_hex(&source.capture_manifest_sha256)
        .map_err(|_| Refusal("native selected software manifest digest invalid"))?;
    Digest256::from_hex(&source.schema_worker_sha256)
        .map_err(|_| Refusal("native selected schema worker digest invalid"))?;
    RelativePath::parse(&source.schema_worker_path)
        .map_err(|_| Refusal("native selected schema worker relative path invalid"))?;
    let mut components = BTreeSet::new();
    for path in &source.software_components {
        let relative = RelativePath::parse(path)
            .map_err(|_| Refusal("native selected software component path invalid"))?;
        if !components.insert(relative.as_str().to_owned()) {
            return Err(Refusal("native selected software component path repeated").into());
        }
    }
    if !components.contains(source.schema_worker_path.as_str())
        || !components.contains("ToS/doctrine/semantic-interchange/query-vocabulary.v1.json")
    {
        return Err(Refusal("native source-only producer component closure incomplete").into());
    }
    if !matches!(source.source_git_commit.len(), 40 | 64)
        || !matches!(source.source_git_tree.len(), 40 | 64)
        || !source
            .source_git_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !source
            .source_git_tree
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Refusal("native selected software Git identity invalid").into());
    }
    Ok(())
}

fn valid_child(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn bounded_input(mut input: impl Read) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut raw)?;
    if raw.len() > MAX_REQUEST_BYTES {
        return Err(Refusal("native Original request byte ceiling").into());
    }
    let json_limits = JsonLimits::new(MAX_REQUEST_BYTES, 32, 20_000, 4300)?;
    parse_json(&raw, JsonMode::PublishedStrict, json_limits)?;
    Ok(raw)
}

fn create_output_tree(
    store_path: &Path,
    store: &File,
    data_name: &str,
    uid: u32,
    cap: u64,
) -> Result<OutputTree> {
    match rustix::fs::statat(store, data_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => (),
        Ok(_) => return Err(Refusal("native data destination must be absent").into()),
        Err(error) => return Err(io::Error::from(error).into()),
    }
    rustix::fs::mkdirat(store, data_name, rustix::fs::Mode::from_raw_mode(0o700))
        .map_err(io::Error::from)?;
    let root: File = rustix::fs::openat(
        store,
        data_name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)?;
    let metadata = root.metadata()?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err(Refusal("native data destination private directory custody").into());
    }
    rustix::fs::mkdirat(&root, "data", rustix::fs::Mode::from_raw_mode(0o700))
        .map_err(io::Error::from)?;
    let data: File = rustix::fs::openat(
        &root,
        "data",
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)?;
    let data_metadata = data.metadata()?;
    if !data_metadata.is_dir() || data_metadata.uid() != uid || data_metadata.mode() & 0o077 != 0 {
        return Err(Refusal("native data member root custody").into());
    }
    data.sync_all()?;
    root.sync_all()?;
    Ok(OutputTree {
        root_path: store_path.join(data_name),
        root,
        store: store.try_clone()?,
        byte_cap: cap,
        written_bytes: 0,
        uid,
    })
}

fn data_member_paths(members: &[manifest::NativeCapturedMember]) -> Vec<String> {
    let mut paths = members
        .iter()
        .map(|member| format!("data/{}", member.source_path))
        .collect::<Vec<_>>();
    paths.push(manifest::NATIVE_MODEL_PATH.to_owned());
    paths.push(manifest::NATIVE_SELECTION_PATH.to_owned());
    paths.sort();
    paths
}

fn runtime_source_bindings(
    selected: &manifest::NativeSelectedSnapshotCensus,
    profile: &manifest::NativeSelectedSnapshotProfile,
) -> BTreeMap<String, String> {
    let mut result = selected.source_bindings().clone();
    result.insert(
        manifest::RUNTIME_DATA_DECLARATION_PATH.to_owned(),
        Digest256::of_bytes(manifest::RUNTIME_DATA_DECLARATION).to_hex(),
    );
    result.insert(
        manifest::EVIDENCE_SCENES_PATH.to_owned(),
        profile.evidence_scenes_sha256.clone(),
    );
    result
}

fn source_bindings(
    corpus: &tos_compiler::CorpusOriginalReceipt,
    captured_members: &[manifest::NativeCapturedMember],
    selected_bindings: &BTreeMap<String, String>,
    deadline: Instant,
) -> Result<BTreeMap<String, String>> {
    let result = selected_bindings.clone();
    if corpus.origin.profile != "captured-runtime-projection-v1"
        || corpus.origin.source_path != manifest::CORPUS_INDEX_PATH
        || result.get(&corpus.origin.source_path).map(String::as_str)
            != Some(corpus.origin.source_sha256.as_str())
    {
        return Err(Refusal("native Original root top binding differs").into());
    }
    let max_original_members = manifest::NATIVE_PRODUCER_MAX_MEMBERS.saturating_sub(2);
    if corpus.origin.members.is_empty() || corpus.origin.members.len() > max_original_members {
        return Err(Refusal("native Original source binding member count ceiling").into());
    }
    let captured = captured_members
        .iter()
        .map(|member| (member.source_path.as_str(), member))
        .collect::<BTreeMap<_, _>>();
    if captured.len() != captured_members.len() {
        return Err(Refusal("native captured source member duplicate").into());
    }
    let mut seen = BTreeSet::new();
    for member in &corpus.origin.members {
        active(deadline)?;
        RelativePath::parse(&member.path)
            .map_err(|_| Refusal("native Original source binding path invalid"))?;
        if !seen.insert(member.path.as_str()) {
            return Err(Refusal("native Original source binding duplicate").into());
        }
        let canonical_digest = Digest256::from_hex(&member.sha256)
            .map_err(|_| Refusal("native Original source binding digest invalid"))?
            .to_hex();
        let captured_member = captured.get(member.path.as_str()).ok_or(Refusal(
            "native Original member absent from captured closure",
        ))?;
        if canonical_digest != member.sha256
            || captured_member.sha256 != member.sha256
            || captured_member.size_bytes != member.size_bytes
        {
            return Err(Refusal("native Original member differs from captured closure").into());
        }
        // The current ManagedLocal reader treats the declared source root as
        // the TOP binding for this captured-runtime profile. Every member is
        // still checked here against the held capture closure, and the
        // receiving reader rechecks every declared member against its held
        // DataGuard. Per-part TOP bindings would exceed this reader's 1 MiB
        // manifest ceiling for a large selected traversal.
    }
    if result.len() > manifest::NATIVE_PRODUCER_MAX_MEMBERS {
        return Err(Refusal("native source binding map member ceiling").into());
    }
    Ok(result)
}

fn compiler_json(fingerprint: &manifest::NativeCompilerFingerprint) -> Value {
    json!({
        "compiler_sha256": fingerprint.compiler_sha256(),
        "compiler_paths": fingerprint.compiler_paths(),
        "input_bindings": fingerprint.input_bindings(),
        "selected_compiler_config_and_image_bytes": fingerprint.code_bytes(),
    })
}

fn json_object(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let mut object = serde_json::Map::new();
    for (key, value) in fields {
        object.insert(key.to_owned(), value);
    }
    Value::Object(object)
}

struct SourceOnlyRuntime {
    cut: CorpusCutReader,
    software: SoftwareCaptureReader,
    components: SoftwareComponentSelectionV1,
    profile: manifest::NativeSelectedRuntimeSourceProfile,
    source_root: PathBuf,
    worker: ExactWorkerIdentity,
    member_count: usize,
    source_bytes: u64,
}

enum SelectedSource {
    Historical {
        profile: manifest::NativeSelectedSnapshotProfile,
        census: manifest::NativeSelectedSnapshotCensus,
    },
    Current {
        runtime: SourceOnlyRuntime,
        census: manifest::NativeSourceOnlySnapshotCensus,
    },
}

impl SelectedSource {
    fn source_root(&self) -> &Path {
        match self {
            Self::Historical { census, .. } => census.source_root(),
            Self::Current { runtime, .. } => &runtime.source_root,
        }
    }

    fn retained_state_upper_bound(&self) -> Result<usize> {
        match self {
            Self::Historical { profile, census } => profile
                .retained_state_upper_bound()?
                .checked_add(census.retained_state_upper_bound()?)
                .ok_or_else(|| Refusal("selected historical source census overflow").into()),
            Self::Current { runtime, census } => runtime
                .retained_state_upper_bound()?
                .checked_add(census.retained_state_upper_bound()?)
                .ok_or_else(|| Refusal("selected current source census overflow").into()),
        }
    }

    fn validate_capture_closure(
        &self,
        capture: &PublicCapture,
        deadline: Instant,
    ) -> Result<Vec<manifest::NativeCapturedMember>> {
        match self {
            Self::Historical { census, .. } => {
                Ok(census.validate_capture_closure(capture, deadline)?)
            }
            Self::Current { census, .. } => Ok(census.validate_capture_closure(capture, deadline)?),
        }
    }

    fn recheck(
        &mut self,
        request: &Request,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        match self {
            Self::Historical { census, .. } => Ok(census.recheck_manifest(deadline)?),
            Self::Current { runtime, .. } => runtime.recheck(
                request
                    .source_only
                    .as_ref()
                    .ok_or(Refusal("native source-only request absent"))?,
                deadline,
                cancelled,
            ),
        }
    }

    fn source_bindings(
        &self,
        corpus: &tos_compiler::CorpusOriginalReceipt,
        captured_members: &[manifest::NativeCapturedMember],
        deadline: Instant,
    ) -> Result<BTreeMap<String, String>> {
        let bindings = match self {
            Self::Historical { census, profile } => runtime_source_bindings(census, profile),
            Self::Current { census, .. } => census.source_bindings().clone(),
        };
        source_bindings(corpus, captured_members, &bindings, deadline)
    }

    fn raw_source_bindings(&self) -> BTreeMap<String, String> {
        match self {
            Self::Historical { census, profile } => runtime_source_bindings(census, profile),
            Self::Current { census, .. } => census.source_bindings().clone(),
        }
    }

    fn manifest_input<'a>(
        &'a self,
        model_abi: &'a str,
        compiler: &'a manifest::NativeCompilerFingerprint,
        source_bindings: &'a BTreeMap<String, String>,
        member_paths: &'a [String],
    ) -> manifest::NativeDataSnapshotManifestInput<'a> {
        match self {
            Self::Historical { profile, census } => manifest::NativeDataSnapshotManifestInput {
                corpus_revision: &profile.corpus_revision,
                selected_profile: Some(profile),
                selected_census: Some(census),
                selected_runtime_source: None,
                source_only_census: None,
                model_abi,
                compiler,
                source_bindings,
                member_paths,
                native_selection: manifest::NATIVE_SELECTION_PATH,
            },
            Self::Current { census, .. } => manifest::NativeDataSnapshotManifestInput {
                corpus_revision: &census.profile().source_revision,
                selected_profile: None,
                selected_census: None,
                selected_runtime_source: Some(census.profile()),
                source_only_census: Some(census),
                model_abi,
                compiler,
                source_bindings,
                member_paths,
                native_selection: manifest::NATIVE_SELECTION_PATH,
            },
        }
    }

    fn member_count(&self) -> usize {
        match self {
            Self::Historical { census, .. } => census.member_count(),
            Self::Current { census, .. } => census.member_count(),
        }
    }

    fn member_bytes(&self) -> u64 {
        match self {
            Self::Historical { census, .. } => census.member_bytes(),
            Self::Current { census, .. } => census.member_bytes(),
        }
    }

    fn selected_source_json(&self, captured_count: usize, captured_bytes: u64) -> Value {
        match self {
            Self::Historical { profile, census } => {
                let excluded = &profile.excluded_compiled_model;
                json_object([
                    ("profile_schema_version", json!(profile.schema_version)),
                    ("source_manifest_path", json!(profile.manifest_path)),
                    ("source_data_revision", json!(profile.data_revision)),
                    ("runtime_data_root", json!(profile.runtime_data_root)),
                    ("source_manifest_sha256", json!(census.manifest_sha256())),
                    ("corpus_revision", json!(profile.corpus_revision)),
                    (
                        "source_members_excluding_old_compiled_sqlite",
                        json!(census.member_count()),
                    ),
                    (
                        "source_bytes_excluding_old_compiled_sqlite",
                        json!(census.member_bytes()),
                    ),
                    ("excluded_old_compiled_sqlite", json!(excluded.path)),
                    (
                        "excluded_old_compiled_sqlite_sha256",
                        json!(excluded.sha256),
                    ),
                    (
                        "excluded_old_compiled_sqlite_bytes",
                        json!(excluded.size_bytes),
                    ),
                    ("capture_member_count", json!(captured_count)),
                    ("capture_member_bytes", json!(captured_bytes)),
                ])
            }
            Self::Current { runtime, census } => json_object([
                (
                    "profile_schema_version",
                    json!(manifest::NativeSelectedRuntimeSourceProfile::SCHEMA_VERSION),
                ),
                ("source_revision", json!(census.profile().source_revision)),
                ("source_members", json!(runtime.member_count)),
                ("source_bytes", json!(runtime.source_bytes)),
                (
                    "source_members_excluding_generated_products",
                    json!(runtime.member_count),
                ),
                (
                    "source_bytes_excluding_generated_products",
                    json!(runtime.source_bytes),
                ),
                (
                    "runtime_product_count",
                    json!(census.profile().products.len()),
                ),
                ("runtime_profile_sha256", json!(census.profile_sha256())),
                ("capture_member_count", json!(captured_count)),
                ("capture_member_bytes", json!(captured_bytes)),
            ]),
        }
    }
}

impl SourceOnlyRuntime {
    fn retained_state_upper_bound(&self) -> Result<usize> {
        let component_bytes = self.components.members().try_fold(0usize, |sum, member| {
            sum.checked_add(member.path.as_str().len())
                .and_then(|n| n.checked_add(std::mem::size_of::<Digest256>()))
                .and_then(|n| {
                    n.checked_add(std::mem::size_of::<u64>() + 2 * std::mem::size_of::<usize>())
                })
                .ok_or(Refusal(
                    "selected software component retained state overflow",
                ))
        })?;
        self.profile
            .retained_state_upper_bound()?
            .checked_add(component_bytes)
            .and_then(|n| n.checked_add(self.source_root.as_os_str().len()))
            .and_then(|n| n.checked_add(self.worker.absolute_path.as_os_str().len()))
            .ok_or_else(|| Refusal("native source-only retained state overflow").into())
    }

    fn recheck(
        &self,
        source: &NativeSourceOnlyRequest,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        active(deadline)?;
        let revision = SourceRevision(
            Digest256::from_hex(&source.source_revision)
                .map_err(|_| Refusal("native selected source revision invalid"))?,
        );
        if self.cut.current().revision() != revision
            || self
                .cut
                .stream(revision)
                .map_err(|_| Refusal("native selected source cut changed"))?
                .expectation()
                .digest
                .to_hex()
                != self.profile.membership_root
            || self.software.selection().source_git_commit != source.source_git_commit
            || self.software.selection().source_git_tree != source.source_git_tree
            || self.software.selection().capture_manifest_sha256.to_hex()
                != source.capture_manifest_sha256
            || self.components.members().count() != source.software_components.len()
        {
            return Err(Refusal("native source/software selection changed").into());
        }
        for member in self.components.members() {
            active(deadline)?;
            let raw = self.software.read_selected_component(
                &self.components,
                &member.path,
                source.max_member_bytes,
                deadline,
                cancelled,
            )?;
            if raw.len() as u64 != member.size_bytes
                || Digest256::of_bytes(&raw).to_hex() != member.sha256.to_hex()
            {
                return Err(Refusal("selected software component changed").into());
            }
        }
        let (mut worker_file, stamp) = open_regular(
            &self.worker.absolute_path,
            source.max_member_bytes,
            rustix::process::getuid().as_raw(),
        )?;
        let mut worker_bytes = Vec::new();
        worker_file.read_to_end(&mut worker_bytes)?;
        if Stamp::from(&worker_file.metadata()?) != stamp
            || Stamp::from(&fs::symlink_metadata(&self.worker.absolute_path)?) != stamp
            || Digest256::of_bytes(&worker_bytes) != self.worker.sha256
        {
            return Err(
                Refusal("selected schema worker changed during native corpus build").into(),
            );
        }
        Ok(())
    }
}

fn source_projection_limits(
    source: &NativeSourceOnlyRequest,
    request: &Request,
    deadline: Instant,
) -> Result<crate::source_corpus_index_projection::CorpusIndexProjectionLimits> {
    use tos_compiler::source_bibliographic::BibliographicLimits;
    use tos_compiler::source_witness_catalog::SourceCatalogLimits;
    let sqlite_cache_kib = u32::try_from(request.cold_open.sqlite_cache_kib)
        .map_err(|_| Refusal("native SQLite cache cap conversion"))?;
    let source_file_cap = usize::try_from(source.max_member_bytes.min(8 * 1024 * 1024))
        .map_err(|_| Refusal("native source projection file cap conversion"))?
        .max(1);
    let source_row_cap = source_file_cap.min(1024 * 1024).max(1);
    let plan_cap = (request.max_state_bytes / 4).clamp(1, 64 * 1024 * 1024);
    let output_cap = request
        .persistent_write_cap_bytes
        .min(manifest::NATIVE_PRODUCER_MAX_DATA_BYTES)
        .min(512 * 1024 * 1024);
    let work_cap = request.max_work_bytes.min(320 * 1024 * 1024).max(1);
    let row_count = request.cold_open.max_rows.max(1);
    let output_row_cap = source_row_cap.min(4 * 1024 * 1024).max(1);
    let claim_rows = usize::try_from(row_count.min(16_384))
        .unwrap_or(16_384)
        .max(1);
    let claim_row_cap = output_row_cap
        .min((128 * 1024 * 1024usize / claim_rows).max(1))
        .max(1);
    let catalog = SourceCatalogLimits {
        max_files: source.max_members.min(65_536),
        max_rows: row_count,
        max_file_bytes: source_file_cap.min(16 * 1024 * 1024),
        max_row_bytes: source_row_cap,
        max_contract_bytes: source_file_cap.min(16 * 1024 * 1024),
        max_output_row_bytes: output_row_cap,
    };
    let bibliographic = BibliographicLimits {
        catalog,
        max_claim_cohort_rows: claim_rows,
        max_claim_cohort_bytes: (claim_rows * claim_row_cap).min(128 * 1024 * 1024),
        max_output_rows: row_count,
        max_output_bytes: output_cap,
        deadline,
    };
    let worker_budget = ExecutorBudget {
        execution_wall: deadline.saturating_duration_since(Instant::now()),
        cleanup_grace: Duration::from_millis(200),
        cpu_seconds: source.worker_cpu_seconds,
        address_space_bytes: source.worker_address_space_bytes,
    };
    let worker_limits = CutWorkerLimits {
        max_receipts: source.max_schema_receipts,
        max_receipt_bytes: source.max_schema_receipt_bytes,
    };
    let batch_wall = worker_budget.execution_wall.min(Duration::from_secs(3600));
    if batch_wall.is_zero() {
        return Err(Refusal("native source projection deadline exhausted").into());
    }
    let batch_units = source
        .max_schema_receipts
        .min(BatchBudget::MAX_UNITS)
        .max(1);
    let batch_raw = usize::try_from(
        source
            .max_member_bytes
            .min(BatchBudget::MAX_RAW_BYTES as u64),
    )
    .map_err(|_| Refusal("native source worker raw cap conversion"))?
    .max(1);
    let batch = BatchBudget {
        total_execution_wall: batch_wall,
        startup_wall: batch_wall.min(Duration::from_secs(10)),
        per_unit_wall: batch_wall.min(Duration::from_secs(5)),
        cleanup_grace: Duration::from_millis(200),
        cpu_seconds: source.worker_cpu_seconds,
        address_space_bytes: source.worker_address_space_bytes,
        max_units: batch_units,
        max_total_raw_bytes: batch_raw,
    };
    let schema_work = BatchStreamBudget {
        batch,
        max_chunks: source.max_schema_receipts as u64,
        max_total_units: source.max_members.max(1),
        max_total_raw_bytes: source.max_total_bytes.max(1),
        total_execution_wall: batch_wall,
        operation_cpu_seconds: source
            .worker_cpu_seconds
            .saturating_mul(source.max_schema_receipts as u64)
            .min(3600)
            .max(1),
        operation_address_space_bytes: source.worker_address_space_bytes,
        max_total_wire_bytes: (source.max_schema_receipts as u64)
            .saturating_mul(source.max_schema_receipt_bytes as u64)
            .min(320 * 1024 * 1024)
            .max(1),
        max_distinct_selectors: source.max_members.min(BatchBudget::MAX_UNITS as u64) as usize,
    };
    Ok(
        crate::source_corpus_index_projection::CorpusIndexProjectionLimits {
            catalog_input: tos_compiler::SourceCatalogInputLimits {
                max_manifest_members: source.max_members,
                max_selected_members: source.max_members.min(4096) as usize,
                max_plan_bytes: plan_cap,
                max_work_bytes: work_cap,
            },
            bibliographic,
            repository: tos_compiler::knowledge_repository_source::RepositorySourceLimits {
                max_inventory_members: source.max_members.min(65_536),
                max_source_bytes: source_file_cap,
                max_row_bytes: source_row_cap,
                max_plan_bytes: plan_cap,
                max_work_bytes: work_cap,
            },
            canon: tos_compiler::knowledge_canon_source::CanonSourceLimits {
                max_manifest_members: source.max_members,
                max_selected_members: source.max_members,
                max_nodes: row_count,
                max_packs: row_count,
                max_edges: row_count,
                max_source_bytes: source_file_cap,
                max_raw_row_bytes: source_row_cap,
                max_csv_fields: 1024,
                max_csv_record_bytes: source_file_cap,
                max_forms: 256,
                max_forms_output_bytes: 256 * 1024,
                max_page_rows: 1024,
                max_page_bytes: 64 * 1024 * 1024,
                max_work_bytes: work_cap,
            },
            stage: tos_compiler::knowledge_stage::StageLimits {
                sqlite: tos_compiler::Limits {
                    max_rows: row_count,
                    max_row_bytes: request.cold_open.max_row_bytes,
                    max_output_bytes: output_cap,
                    max_work_bytes: work_cap,
                    sqlite_cache_kib,
                    max_sql_vm_steps: request.cold_open.max_vm_steps,
                },
                max_temp_bytes: request.tmpfs_quota_bytes.min(64 * 1024 * 1024).max(1),
                max_seek_rows: row_count.min(1024) as usize,
                max_seek_bytes: output_cap.min(64 * 1024 * 1024),
            },
            schema_worker: worker_budget,
            schema_worker_limits: worker_limits,
            schema_work,
            originals: tos_compiler::NavigationOriginalLimits {
                max_rows: row_count,
                max_row_bytes: request.cold_open.max_row_bytes,
                max_total_bytes: request.cold_open.max_work_bytes,
            },
            max_canon_input_bytes: source.max_total_bytes.min(8 * 1024 * 1024),
            max_output_bytes: output_cap,
            max_work_bytes: work_cap,
        },
    )
}

fn write_source_file(root: &Path, relative: &str, raw: &[u8], mode: u32) -> Result<()> {
    let relative =
        RelativePath::parse(relative).map_err(|_| Refusal("native source staging path invalid"))?;
    let path = root.join(relative.as_str());
    let parent = path
        .parent()
        .ok_or(Refusal("native source staging parent absent"))?;
    fs::create_dir_all(parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .mode(mode & 0o777)
        .open(&path)?;
    file.write_all(raw)?;
    file.sync_all()?;
    Ok(())
}

fn materialize_source_cut(
    source: &NativeSourceOnlyRequest,
    cut: &CorpusCutReader,
    source_root: &Path,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(usize, u64)> {
    const GENERATED: &[&str] = &[
        manifest::CORPUS_INDEX_PATH,
        tos_compiler::source_philosophy_views::ATLAS_REF,
        tos_compiler::source_philosophy_graph::VIEWS_REF,
        manifest::PHILOSOPHY_GRAPH_PATH,
        manifest::CLAIM_GRAPH_PATH,
        manifest::EVIDENCE_PROJECTION_PATH,
    ];
    if source_root.exists() || source_root.is_symlink() {
        return Err(Refusal("native source staging root must be new").into());
    }
    fs::create_dir(source_root)?;
    let mut count = 0usize;
    let mut total_bytes = 0u64;
    for member in cut.current().members() {
        active(deadline)?;
        if GENERATED.contains(&member.path.as_str()) {
            continue;
        }
        count = count
            .checked_add(1)
            .filter(|value| *value <= source.max_members as usize)
            .ok_or(Refusal("native source cut member count ceiling"))?;
        total_bytes = total_bytes
            .checked_add(member.size_bytes)
            .filter(|value| *value <= source.max_total_bytes)
            .ok_or(Refusal("native source cut byte ceiling"))?;
        if member.size_bytes > source.max_member_bytes {
            return Err(Refusal("native source cut member byte ceiling").into());
        }
        let path = RelativePath::parse(member.path.as_str())
            .map_err(|_| Refusal("native source cut path invalid"))?;
        let raw = cut.read_member(
            cut.current().revision(),
            &path,
            source.max_member_bytes,
            deadline,
            cancelled,
        )?;
        if raw.raw.len() as u64 != member.size_bytes
            || Digest256::of_bytes(&raw.raw) != member.sha256
        {
            return Err(Refusal("native source cut member changed while staging").into());
        }
        write_source_file(source_root, member.path.as_str(), &raw.raw, member.mode)?;
    }
    Ok((count, total_bytes))
}

fn prepare_source_only_runtime(
    source: &NativeSourceOnlyRequest,
    request: &Request,
    isolation: &PrivateTmpfsStageIsolation,
    deadline: Instant,
    cancelled: &Arc<AtomicBool>,
) -> Result<SourceOnlyRuntime> {
    let read_limits = ReadLimits {
        max_manifest_bytes: 4_194_304,
        max_manifest_entries: source.max_members.min(4096) as usize,
        max_selected_object_bytes: source.max_total_bytes,
        json: JsonLimits::default(),
    };
    let store = CorpusReader::open_existing(Path::new(&source.corpus_store), read_limits)?;
    let revision = SourceRevision(
        Digest256::from_hex(&source.source_revision)
            .map_err(|_| Refusal("native selected source revision invalid"))?,
    );
    let cut = store.open_source_cut(
        revision,
        CutReadLimits {
            max_revisions: source.max_revisions,
            max_members: source.max_members,
            max_total_bytes: source.max_total_bytes,
            max_member_bytes: source.max_member_bytes,
        },
        deadline,
        cancelled,
    )?;
    if cut.current().revision() != revision {
        return Err(Refusal("native selected source revision differs from opened cut").into());
    }
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: source.source_git_commit.clone(),
        source_git_tree: source.source_git_tree.clone(),
        capture_manifest_sha256: Digest256::from_hex(&source.capture_manifest_sha256)
            .map_err(|_| Refusal("native selected software manifest digest invalid"))?,
    };
    let software = SoftwareCaptureReader::open(
        Path::new(&source.software_capture),
        Path::new(&source.software_restored_root),
        selection,
        read_limits,
        deadline,
        cancelled,
    )?;
    let component_paths = source
        .software_components
        .iter()
        .map(|path| -> Result<RelativePath> {
            Ok(RelativePath::parse(path)
                .map_err(|_| Refusal("native selected software component path invalid"))?)
        })
        .collect::<Result<Vec<_>>>()?;
    let components = software.select_components(&component_paths)?;
    let worker_relative = RelativePath::parse(&source.schema_worker_path)
        .map_err(|_| Refusal("native selected schema worker path invalid"))?;
    let worker = ExactWorkerIdentity {
        absolute_path: PathBuf::from(&source.schema_worker_absolute_path),
        sha256: Digest256::from_hex(&source.schema_worker_sha256)
            .map_err(|_| Refusal("native selected schema worker SHA invalid"))?,
    };
    // Verify captured worker source independently; `worker` names the exact
    // selected executable image that the schema executor pins and measures.
    drop(software.read_selected_component(
        &components,
        &worker_relative,
        source.max_member_bytes,
        deadline,
        cancelled,
    )?);
    let software_binding = manifest::NativeSelectedSoftwareBinding::from_capture(
        &software,
        &components,
        &source.schema_worker_path,
        &source.schema_worker_sha256,
    )?;
    let profile =
        manifest::NativeSelectedRuntimeSourceProfile::from_current_cut(&cut, software_binding)?;
    let source_root = isolation.root().join("tos-native-corpus-source");
    let (member_count, source_bytes) =
        materialize_source_cut(source, &cut, &source_root, deadline, cancelled)?;
    let source_home_path = RelativePath::parse("ToS/source_home.manifest.json")
        .map_err(|_| Refusal("native source-home manifest path invalid"))?;
    let source_home = cut.read_member(
        revision,
        &source_home_path,
        source.max_member_bytes,
        deadline,
        cancelled,
    )?;
    let source_home_value: Value = serde_json::from_slice(&source_home.raw)?;
    let identity_id = source_home_value
        .get("owner_repo")
        .and_then(Value::as_str)
        .filter(|value| *value == "Tree-of-Sophia")
        .ok_or(Refusal("native selected source-home identity invalid"))?;
    let source_home_sha = Digest256::of_bytes(&source_home.raw).to_hex();
    let binding = profile.source_binding();
    let root_input = tos_compiler::RepositoryRootInput {
        source_cut: &binding.source_cut,
        material: &source_home.raw,
        material_sha256: &source_home_sha,
        identity_id,
    };
    let projection_limits = source_projection_limits(source, request, deadline)?;
    let worker_image = worker.clone();
    let recheck = || -> tos_compiler::Result<()> {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed)
            || Instant::now() >= deadline
            || cut.current().revision() != revision
            || cut
                .stream(revision)
                .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
                .expectation()
                .digest
                .to_hex()
                != profile.membership_root
            || software.selection().capture_manifest_sha256.to_hex()
                != source.capture_manifest_sha256
        {
            return Err(tos_compiler::Error::Invalid(
                "native source selection changed",
            ));
        }
        Ok(())
    };
    let products = crate::source_corpus_index_projection::project(
        &cut,
        &software,
        &binding,
        root_input,
        worker_image,
        projection_limits,
        isolation,
        isolation.root(),
        &recheck,
        deadline,
        cancelled,
    )?;
    write_source_file(
        &source_root,
        manifest::CORPUS_INDEX_PATH,
        products.corpus.output_bytes(),
        0o644,
    )?;
    write_source_file(
        &source_root,
        manifest::CLAIM_GRAPH_PATH,
        &products.bibliographic_claims,
        0o644,
    )?;
    let philosophy_args = vec![
        "--philosophy-product".to_owned(),
        "corpus".to_owned(),
        "--mode".to_owned(),
        "build".to_owned(),
        "--source-root".to_owned(),
        source_root.display().to_string(),
        "--output-root".to_owned(),
        source_root.display().to_string(),
        "--max-seconds".to_owned(),
        deadline
            .saturating_duration_since(Instant::now())
            .as_secs()
            .max(1)
            .min(3600)
            .to_string(),
        "--scratch-bytes".to_owned(),
        request.tmpfs_quota_bytes.to_string(),
    ];
    tos_ops_mechanics_plan::philosophy_products::run(&philosophy_args, Arc::clone(cancelled))
        .map_err(|_| Refusal("native source philosophy products failed"))?;
    let evidence_stage = isolation.root().join("tos-native-evidence-stage.sqlite3");
    let mut evidence_limits =
        manifest::portable_native_snapshot_limits(request.max_build_seconds)?.capture;
    let sqlite_cache_kib = u32::try_from(request.cold_open.sqlite_cache_kib)
        .map_err(|_| Refusal("native SQLite cache cap conversion"))?;
    evidence_limits.max_input_bytes = source.max_total_bytes;
    evidence_limits.max_rows = request.cold_open.max_rows;
    evidence_limits.max_staging_bytes = request.tmpfs_quota_bytes;
    evidence_limits.max_work_bytes = request.max_work_bytes;
    evidence_limits.max_sql_vm_steps = request.cold_open.max_vm_steps;
    evidence_limits.sqlite_cache_kib = sqlite_cache_kib;
    let evidence = tos_compiler::epistemic_evidence::build(
        &source_root,
        &evidence_stage,
        evidence_limits,
        deadline,
    )?;
    write_source_file(
        &source_root,
        manifest::EVIDENCE_PROJECTION_PATH,
        &evidence,
        0o644,
    )?;
    let product_paths = manifest::required_native_runtime_product_paths();
    let mut product_rows = Vec::with_capacity(product_paths.len());
    for path in product_paths {
        active(deadline)?;
        let full_path = source_root.join(path);
        let (mut file, stamp) = open_regular(
            &full_path,
            request.persistent_write_cap_bytes,
            rustix::process::getuid().as_raw(),
        )?;
        let mut raw = Vec::new();
        (&mut file)
            .take(request.persistent_write_cap_bytes.saturating_add(1))
            .read_to_end(&mut raw)?;
        if raw.len() as u64 > request.persistent_write_cap_bytes
            || Stamp::from(&file.metadata()?) != stamp
            || Stamp::from(&fs::symlink_metadata(&full_path)?) != stamp
        {
            return Err(Refusal("native generated runtime product changed or exceeded cap").into());
        }
        product_rows.push(manifest::NativeSourceMemberBinding {
            path: path.to_owned(),
            mode: stamp.mode & 0o777,
            size_bytes: raw.len() as u64,
            sha256: Digest256::of_bytes(&raw).to_hex(),
        });
    }
    product_rows.sort_by(|left, right| left.path.cmp(&right.path));
    let profile = profile.with_runtime_products(product_rows)?;
    let runtime = SourceOnlyRuntime {
        cut,
        software,
        components,
        profile,
        source_root,
        worker,
        member_count,
        source_bytes,
    };
    runtime.recheck(source, deadline, cancelled)?;
    Ok(runtime)
}

fn check_source_products(
    runtime: &SourceOnlyRuntime,
    comparison_root: &Path,
    deadline: Instant,
) -> Result<Value> {
    no_symlink_path(comparison_root)?;
    let root_metadata = fs::symlink_metadata(comparison_root)?;
    if !root_metadata.is_dir() || root_metadata.uid() != rustix::process::getuid().as_raw() {
        return Err(Refusal("native corpus comparison root must be an owned directory").into());
    }
    let mut products = Vec::with_capacity(runtime.profile.products.len());
    for expected in &runtime.profile.products {
        active(deadline)?;
        let path = comparison_root.join(&expected.path);
        let (mut file, stamp) = open_regular(
            &path,
            expected.size_bytes,
            rustix::process::getuid().as_raw(),
        )?;
        let mut raw = Vec::new();
        (&mut file)
            .take(expected.size_bytes.saturating_add(1))
            .read_to_end(&mut raw)?;
        if raw.len() as u64 != expected.size_bytes
            || Digest256::of_bytes(&raw).to_hex() != expected.sha256
            || Stamp::from(&file.metadata()?) != stamp
            || Stamp::from(&fs::symlink_metadata(&path)?) != stamp
        {
            return Err(Refusal("native corpus runtime product parity differs").into());
        }
        products.push(json_object([
            ("path", json!(expected.path)),
            ("size_bytes", json!(expected.size_bytes)),
            ("sha256", json!(expected.sha256)),
            ("matches", json!(true)),
        ]));
    }
    Ok(json_object([
        ("schema_version", json!("tos_native_corpus_build_result_v1")),
        ("mode", json!("check")),
        ("outcome", json!("native-runtime-products-match")),
        ("comparison_root", json!(comparison_root)),
        ("source_revision", json!(runtime.profile.source_revision)),
        (
            "runtime_profile_sha256",
            json!(runtime.profile.profile_sha256),
        ),
        ("persistent_write_performed", json!(false)),
        ("products", Value::Array(products)),
    ]))
}

pub(crate) fn validate_direct_projection_request(
    request: &DirectRepositoryProjectionRequest,
) -> Result<()> {
    let limits = request.limits;
    let root = request
        .repository_root
        .to_str()
        .ok_or(Refusal("native projection repository path must be UTF-8"))?;
    if !absolute_bounded(root)
        || request.software_git_commit.len() != 40
        || !request
            .software_git_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || request.software_git_tree.len() != 40
        || !request
            .software_git_tree
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !request.schema_worker_absolute_path.is_absolute()
        || request.schema_worker_absolute_path.as_os_str().len() > 4096
        || Digest256::from_hex(&request.schema_worker_sha256).is_err()
        || limits.tmpfs_quota_bytes < manifest::NATIVE_PRODUCER_MIN_TMPFS_QUOTA_BYTES
        || limits.tmpfs_inode_limit < 4096
        || limits.working_ram_bytes == 0
        || limits.max_build_seconds == 0
        || limits.max_build_seconds > MAX_BUILD_SECONDS
        || limits.max_source_members == 0
        || limits.max_source_members > manifest::NATIVE_PRODUCER_MAX_MEMBERS as u64
        || limits.max_source_bytes == 0
        || limits.max_source_bytes > manifest::NATIVE_PRODUCER_MAX_SOURCE_CLOSURE_BYTES
        || limits.max_member_bytes == 0
        || limits.max_member_bytes > limits.max_source_bytes
        || limits.max_member_bytes > MAX_COLD_ROW_BYTES as u64
        || limits.max_manifest_bytes < 4096
        || limits.max_manifest_bytes == usize::MAX
        || limits.max_capture_member_read_bytes == 0
        || limits.max_capture_member_read_bytes == usize::MAX
        || limits.max_capture_write_bytes == 0
        || limits.max_capture_write_bytes == usize::MAX
        || limits.max_recheck_read_bytes == 0
        || limits.max_recheck_read_bytes == usize::MAX
        || limits.max_callback_and_fence_state_bytes == 0
        || limits.max_callback_and_fence_state_bytes == usize::MAX
        || limits.callback_owned_heap_state_upper_bound_bytes == 0
        || limits.callback_owned_heap_state_upper_bound_bytes == usize::MAX
        || limits.final_fence_workspace_upper_bound_bytes == 0
        || limits.final_fence_workspace_upper_bound_bytes == usize::MAX
        || limits.max_state_bytes < 131_072
        || limits.max_json_visits == 0
        || limits.max_work_bytes == 0
        || limits.max_work_bytes > MAX_PRODUCER_WORK_BYTES
        || limits.max_output_bytes == 0
        || limits.max_output_bytes > manifest::NATIVE_PRODUCER_MAX_DATA_BYTES
        || limits.max_schema_receipts == 0
        || limits.max_schema_receipts > 4096
        || limits.max_schema_receipt_bytes == 0
        || limits.max_schema_receipt_bytes > 4 * 1024 * 1024
        || limits.worker_cpu_seconds == 0
        || limits.worker_cpu_seconds > ExecutorBudget::MAX_SCALAR_CPU_SECONDS
        || limits.worker_address_space_bytes < 64 * 1024 * 1024
        || limits.worker_address_space_bytes > limits.working_ram_bytes
        || limits.max_worker_image_bytes == 0
        || limits.max_worker_image_bytes > 512 * 1024 * 1024
        || limits.max_state_bytes as u64 > limits.process_limits.address_space_bytes
        || limits.process_limits.address_space_bytes > limits.working_ram_bytes
        || limits.process_limits.file_size_bytes == 0
        || limits.process_limits.file_size_bytes > manifest::NATIVE_PRODUCER_MAX_DATA_BYTES
        || limits.cold_open.max_file_bytes == 0
        || limits.cold_open.max_file_bytes > manifest::NATIVE_PRODUCER_MAX_MODEL_BYTES
        || limits.cold_open.max_vm_steps == 0
        || limits.cold_open.max_vm_steps > MAX_COLD_VM_STEPS
        || limits.cold_open.sqlite_cache_kib == 0
        || u32::try_from(limits.cold_open.sqlite_cache_kib).is_err()
        || limits.cold_open.max_rows == 0
        || limits.cold_open.max_rows > MAX_COLD_ROWS
        || limits.cold_open.max_work_bytes == 0
        || limits.cold_open.max_work_bytes > MAX_COLD_WORK_BYTES
        || limits.cold_open.max_row_bytes == 0
        || limits.cold_open.max_row_bytes > MAX_COLD_ROW_BYTES
        || limits.cold_open.max_metadata_bytes == 0
        || limits.cold_open.max_metadata_bytes > MAX_COLD_METADATA_BYTES
        || limits.cold_open.max_sources == 0
        || limits.cold_open.max_sources > MAX_COLD_SOURCES
    {
        return Err(Refusal("native direct projection request limits invalid").into());
    }
    Ok(())
}

fn verify_direct_worker_image(worker: &ExactWorkerIdentity, max_bytes: u64) -> Result<()> {
    if measure_direct_worker_image(&worker.absolute_path, max_bytes)? != worker.sha256 {
        return Err(Refusal("selected schema worker image changed or digest differs").into());
    }
    Ok(())
}

fn measure_direct_worker_image(path: &Path, max_bytes: u64) -> Result<Digest256> {
    let uid = rustix::process::getuid().as_raw();
    let (mut file, stamp) = open_regular(path, max_bytes, uid)?;
    let mut hasher = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .filter(|bytes| *bytes <= max_bytes)
            .ok_or(Refusal("selected schema worker image cap exceeded"))?;
        hasher.update(&buffer[..count]);
    }
    let digest = hasher.finalize();
    if total != stamp.size
        || Stamp::from(&file.metadata()?) != stamp
        || Stamp::from(&fs::symlink_metadata(path)?) != stamp
    {
        return Err(Refusal("selected schema worker image changed during measurement").into());
    }
    Ok(digest)
}

fn lowercase_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn git_revision_output(raw: &[u8]) -> Result<String> {
    let value = std::str::from_utf8(raw)
        .map_err(|_| Refusal("native selected software revision is not UTF-8"))?;
    let value = value.strip_suffix('\n').unwrap_or(value);
    if !lowercase_hex(value, 40) {
        return Err(Refusal("native selected software revision output invalid").into());
    }
    Ok(value.to_owned())
}

fn resolve_software_git_identity(
    repository_root: &Path,
    selected_commit: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(String, String)> {
    if selected_commit != "HEAD" && !lowercase_hex(selected_commit, 40) {
        return Err(Refusal("native selected software commit must be HEAD or exact hex").into());
    }
    let commit_expression = format!("{selected_commit}^{{commit}}");
    let commit_output = crate::source_text_owner_ocr::bounded_process(
        "git",
        &["rev-parse", "--verify", &commit_expression],
        Some(repository_root),
        128,
        deadline,
        cancelled,
    )
    .map_err(|_| Refusal("native selected software commit could not be resolved"))?;
    let commit = git_revision_output(&commit_output)?;
    if selected_commit != "HEAD" && commit != selected_commit {
        return Err(Refusal("native selected software commit changed").into());
    }
    let tree_expression = format!("{commit}^{{tree}}");
    let tree_output = crate::source_text_owner_ocr::bounded_process(
        "git",
        &["rev-parse", "--verify", &tree_expression],
        Some(repository_root),
        128,
        deadline,
        cancelled,
    )
    .map_err(|_| Refusal("native selected software tree could not be resolved"))?;
    let tree = git_revision_output(&tree_output)?;
    Ok((commit, tree))
}

fn direct_projection_request_from_argv(
    repository_root: &str,
    selected_commit: &str,
    worker_environment: &str,
    limits_profile: &str,
) -> Result<(DirectRepositoryProjectionRequest, &'static str)> {
    if limits_profile != "repo-validation-v1"
        || worker_environment.is_empty()
        || worker_environment.len() > 128
        || !worker_environment.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_uppercase() || (index > 0 && byte.is_ascii_digit())
        })
    {
        return Err(Refusal("native direct projection argv selection invalid").into());
    }
    let repository_root = PathBuf::from(repository_root);
    if !repository_root.is_absolute() {
        return Err(Refusal("native direct projection repository root must be absolute").into());
    }
    no_symlink_path(&repository_root)?;
    let repository_root = repository_root.canonicalize()?;
    let limits = DirectRepositoryProjectionLimits::repo_validation_v1();
    let root_utf8 = repository_root
        .to_str()
        .ok_or(Refusal("native projection repository path must be UTF-8"))?;
    if !absolute_bounded(root_utf8) {
        return Err(Refusal("native direct projection repository path invalid").into());
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(limits.max_build_seconds))
        .ok_or(Refusal("native direct projection deadline overflow"))?;
    let cancelled = AtomicBool::new(false);
    let (software_git_commit, software_git_tree) =
        resolve_software_git_identity(&repository_root, selected_commit, deadline, &cancelled)?;
    let worker_path = std::env::var_os(worker_environment)
        .map(PathBuf::from)
        .ok_or(Refusal(
            "native selected schema worker environment is unset",
        ))?;
    if !worker_path.is_absolute() {
        return Err(Refusal("native selected schema worker path must be absolute").into());
    }
    no_symlink_path(&worker_path)?;
    let worker_path = worker_path.canonicalize()?;
    let worker_utf8 = worker_path
        .to_str()
        .ok_or(Refusal("native selected schema worker path must be UTF-8"))?;
    if !absolute_bounded(worker_utf8) {
        return Err(Refusal("native selected schema worker path invalid").into());
    }
    let schema_worker_sha256 =
        measure_direct_worker_image(&worker_path, limits.max_worker_image_bytes)?.to_hex();
    let request = DirectRepositoryProjectionRequest {
        repository_root,
        software_git_commit,
        software_git_tree,
        schema_worker_absolute_path: worker_path,
        schema_worker_sha256,
        limits,
    };
    validate_direct_projection_request(&request)?;
    Ok((request, "repo-validation-v1"))
}

fn compare_direct_projection_product(
    repository_root: &Path,
    path: &str,
    candidate: &[u8],
    max_bytes: u64,
) -> Result<Value> {
    let path = repository_root.join(path);
    no_symlink_path(&path)?;
    let (mut file, stamp) = open_regular(&path, max_bytes, rustix::process::getuid().as_raw())?;
    let mut actual = Vec::new();
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut actual)?;
    if actual.len() as u64 != stamp.size
        || actual != candidate
        || Stamp::from(&file.metadata()?) != stamp
        || Stamp::from(&fs::symlink_metadata(&path)?) != stamp
    {
        return Err(Refusal("native direct projection byte parity differs").into());
    }
    Ok(json_object([
        (
            "path",
            json!(path.strip_prefix(repository_root)?.to_string_lossy()),
        ),
        ("size_bytes", json!(stamp.size)),
        ("sha256", json!(Digest256::of_bytes(candidate).to_hex())),
        ("matches", json!(true)),
    ]))
}

/// Produce both source-derived products from one direct checkout capture.
/// The callback runs before the source capture's final live-content and
/// publication-epoch recheck, so parity and query consumers stay inside the
/// same bounded source window.
pub(crate) fn with_direct_repository_projection<T>(
    request: &DirectRepositoryProjectionRequest,
    consume: impl FnOnce(
        &crate::source_corpus_index_projection::NativeCorpusIndexProducts,
        DirectRepositoryProjectionContext<'_>,
    ) -> Result<T>,
) -> Result<(
    crate::source_corpus_index_projection::NativeCorpusIndexProducts,
    T,
)> {
    validate_direct_projection_request(request)?;
    let limits = request.limits;
    let started = Instant::now();
    let deadline = started
        .checked_add(Duration::from_secs(limits.max_build_seconds))
        .ok_or(Refusal("native direct projection deadline overflow"))?;
    let uid = rustix::process::getuid().as_raw();
    if rustix::process::geteuid().as_raw() != uid {
        return Err(Refusal("native direct projection refuses setuid execution").into());
    }
    no_symlink_path(&request.repository_root)?;
    let isolation = PrivateTmpfsStageIsolation::select_from_environment(
        limits.tmpfs_quota_bytes,
        limits.tmpfs_inode_limit,
        limits.working_ram_bytes,
    )?;
    let workspace = tempfile::Builder::new()
        .prefix("tos-corpus-projection-check-")
        .tempdir_in(isolation.root())?;
    let workspace_metadata = fs::symlink_metadata(workspace.path())?;
    if !workspace_metadata.is_dir()
        || workspace_metadata.file_type().is_symlink()
        || workspace_metadata.uid() != uid
        || workspace_metadata.mode() & 0o777 != 0o700
    {
        return Err(Refusal("native direct projection workspace is not private").into());
    }
    let cancelled = AtomicBool::new(false);
    let isolated = IsolatedCreationRoot::create(workspace.path(), deadline, &cancelled)?;
    let mut sources = RouteSources::new_until_with_operation_limit(
        &request.repository_root,
        deadline,
        crate::source_current_cut::foundation_capture::MAX_CAPTURE_DISCOVERY_ENTRIES,
    )?;

    let software_capture_path = workspace.path().join("software-capture");
    let software_restored_root = workspace.path().join("software-restored");
    let software_include = vec![
        QUERY_VOCABULARY_PATH.to_owned(),
        SCHEMA_WORKER_SOURCE_PATH.to_owned(),
    ];
    const SOFTWARE_MEMBER_BYTES: u64 = 65_536;
    const SOFTWARE_SOURCE_BYTES: u64 = 131_072;
    const SOFTWARE_METADATA_BYTES: usize = 65_536;
    const SOFTWARE_ARCHIVE_BYTES: u64 = 262_144;
    const SOFTWARE_TREE_BYTES: u64 = 131_072;
    let capture = tos_source_store::capture_git(
        CaptureGitRequest {
            repository: &request.repository_root,
            commit: &request.software_git_commit,
            include_prefixes: &software_include,
            exclude_prefixes: &[],
            exclude_path_parts: &[],
            output: &software_capture_path,
        },
        GitCaptureLimits {
            max_members: software_include.len(),
            max_member_bytes: SOFTWARE_MEMBER_BYTES,
            max_source_bytes: SOFTWARE_SOURCE_BYTES,
            max_metadata_bytes: SOFTWARE_METADATA_BYTES,
            max_tree_bytes: SOFTWARE_TREE_BYTES,
            max_archive_bytes: SOFTWARE_ARCHIVE_BYTES,
        },
        deadline,
        &cancelled,
    )?;
    if capture
        .manifest
        .object_get("source_git_tree")
        .and_then(tos_foundation::JsonValue::as_str)
        != Some(request.software_git_tree.as_str())
    {
        return Err(Refusal("native selected software tree differs from commit").into());
    }
    let software_selection = SoftwareCaptureSelectionV1 {
        source_git_commit: request.software_git_commit.clone(),
        source_git_tree: request.software_git_tree.clone(),
        capture_manifest_sha256: capture.manifest_sha256,
    };
    let mut software_json = JsonLimits::default();
    software_json.max_bytes = SOFTWARE_METADATA_BYTES;
    software_json.max_visits = limits.max_json_visits.min(software_json.max_visits);
    let software_read_limits = ReadLimits {
        max_manifest_bytes: SOFTWARE_METADATA_BYTES,
        max_manifest_entries: software_include.len(),
        max_selected_object_bytes: SOFTWARE_SOURCE_BYTES,
        json: software_json,
    };
    tos_source_store::restore_capture(
        &software_capture_path,
        &software_restored_root,
        &software_selection,
        CaptureRestoreLimits {
            metadata: software_read_limits,
            max_archive_bytes: SOFTWARE_ARCHIVE_BYTES,
            max_decoded_bytes: SOFTWARE_SOURCE_BYTES,
            max_source_bytes: SOFTWARE_SOURCE_BYTES,
        },
        deadline,
        &cancelled,
    )?;
    let software = SoftwareCaptureReader::open(
        &software_capture_path,
        &software_restored_root,
        software_selection,
        software_read_limits,
        deadline,
        &cancelled,
    )?;
    let component_paths = [QUERY_VOCABULARY_PATH, SCHEMA_WORKER_SOURCE_PATH]
        .into_iter()
        .map(|path| {
            RelativePath::parse(path).map_err(|_| -> Box<dyn StdError> {
                Box::new(Refusal("native direct projection component path invalid"))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let components = software.select_components(&component_paths)?;
    let worker_source_path = RelativePath::parse(SCHEMA_WORKER_SOURCE_PATH)
        .map_err(|_| Refusal("native schema worker source path invalid"))?;
    let worker_source = software.read_selected_component(
        &components,
        &worker_source_path,
        SOFTWARE_MEMBER_BYTES,
        deadline,
        &cancelled,
    )?;
    drop(worker_source);
    let worker = ExactWorkerIdentity {
        absolute_path: request.schema_worker_absolute_path.clone(),
        sha256: Digest256::from_hex(&request.schema_worker_sha256)
            .map_err(|_| Refusal("native direct schema worker SHA invalid"))?,
    };
    verify_direct_worker_image(&worker, limits.max_worker_image_bytes)?;

    let mut json_limits = JsonLimits::default();
    json_limits.max_bytes = limits.max_manifest_bytes;
    json_limits.max_visits = limits.max_json_visits;
    let member_limit = usize::try_from(limits.max_source_members)
        .map_err(|_| Refusal("native direct source member count range"))?;
    let read_limits = ReadLimits {
        max_manifest_bytes: limits.max_manifest_bytes,
        max_manifest_entries: member_limit,
        max_selected_object_bytes: limits.max_source_bytes,
        json: json_limits,
    };
    let cut_limits = CutReadLimits {
        max_revisions: 1,
        max_members: limits.max_source_members,
        max_total_bytes: limits.max_source_bytes,
        max_member_bytes: limits.max_member_bytes,
    };
    let capture_limits = AuthoredDiagnosticCaptureLimits {
        read_limits,
        cut_limits,
        max_capture_member_read_bytes: limits.max_capture_member_read_bytes,
        max_capture_write_bytes: limits.max_capture_write_bytes,
        max_recheck_read_bytes: limits.max_recheck_read_bytes,
        max_callback_and_fence_state_bytes: limits.max_callback_and_fence_state_bytes,
        callback_owned_heap_state_upper_bound_bytes: limits
            .callback_owned_heap_state_upper_bound_bytes,
        final_fence_workspace_upper_bound_bytes: limits.final_fence_workspace_upper_bound_bytes,
    };
    let worker_image_digest = worker.sha256;
    let ((products, consumed), _) =
        crate::source_current_cut::foundation_capture::with_authored_diagnostic_capture(
            &mut sources,
            &isolated,
            worker_image_digest,
            capture_limits,
            deadline,
            &cancelled,
            |captured: &AuthoredDiagnosticCapture<'_>| {
                let callback_result = (|| -> Result<_> {
                    let cut = captured.cut();
                    if cut.current().member_count() as u64 > limits.max_source_members {
                        return Err(Refusal("native direct authored member cap exceeded").into());
                    }
                    let source_bytes = cut.current().members().try_fold(0u64, |sum, member| {
                        sum.checked_add(member.size_bytes)
                            .filter(|bytes| *bytes <= limits.max_source_bytes)
                            .ok_or(Refusal("native direct authored byte cap exceeded"))
                    })?;
                    if source_bytes > limits.max_source_bytes
                        || cut
                            .current()
                            .members()
                            .any(|member| member.size_bytes > limits.max_member_bytes)
                    {
                        return Err(Refusal("native direct authored member budget exceeded").into());
                    }
                    let vocabulary_path = RelativePath::parse(QUERY_VOCABULARY_PATH)
                        .map_err(|_| Refusal("native query vocabulary path invalid"))?;
                    let source_vocabulary = cut
                        .current()
                        .member(&vocabulary_path)
                        .ok_or(Refusal("native direct source vocabulary absent"))?;
                    let captured_vocabulary = components
                        .member(&vocabulary_path)
                        .ok_or(Refusal("native selected software vocabulary absent"))?;
                    if source_vocabulary.sha256 != captured_vocabulary.sha256
                        || source_vocabulary.size_bytes != captured_vocabulary.size_bytes
                        || source_vocabulary.mode != captured_vocabulary.mode
                    {
                        return Err(Refusal("native source and software vocabulary differ").into());
                    }
                    let software_binding = manifest::NativeSelectedSoftwareBinding::from_capture(
                        &software,
                        &components,
                        SCHEMA_WORKER_SOURCE_PATH,
                        &request.schema_worker_sha256,
                    )?;
                    let profile = manifest::NativeSelectedRuntimeSourceProfile::from_current_cut(
                        cut,
                        software_binding,
                    )?;
                    let revision = cut.current().revision();
                    let source_home_path = RelativePath::parse("ToS/source_home.manifest.json")
                        .map_err(|_| Refusal("native source-home manifest path invalid"))?;
                    let source_home = cut.read_member(
                        revision,
                        &source_home_path,
                        limits.max_member_bytes,
                        deadline,
                        &cancelled,
                    )?;
                    let source_home_value: Value = serde_json::from_slice(&source_home.raw)?;
                    let identity_id = source_home_value
                        .get("owner_repo")
                        .and_then(Value::as_str)
                        .filter(|value| *value == "Tree-of-Sophia")
                        .ok_or(Refusal("native direct source-home identity invalid"))?;
                    let source_home_sha = Digest256::of_bytes(&source_home.raw).to_hex();
                    let binding = profile.source_binding();
                    let root_input = tos_compiler::RepositoryRootInput {
                        source_cut: &binding.source_cut,
                        material: &source_home.raw,
                        material_sha256: &source_home_sha,
                        identity_id,
                    };
                    let projection_source = NativeSourceOnlyRequest {
                        corpus_store: String::new(),
                        source_revision: profile.source_revision.clone(),
                        max_revisions: 1,
                        max_members: limits.max_source_members,
                        max_total_bytes: limits.max_source_bytes,
                        max_member_bytes: limits.max_member_bytes,
                        software_capture: software_capture_path.display().to_string(),
                        software_restored_root: software_restored_root.display().to_string(),
                        source_git_commit: request.software_git_commit.clone(),
                        source_git_tree: request.software_git_tree.clone(),
                        capture_manifest_sha256: capture.manifest_sha256.to_hex(),
                        software_components: vec![
                            QUERY_VOCABULARY_PATH.to_owned(),
                            SCHEMA_WORKER_SOURCE_PATH.to_owned(),
                        ],
                        schema_worker_path: SCHEMA_WORKER_SOURCE_PATH.to_owned(),
                        schema_worker_absolute_path: request
                            .schema_worker_absolute_path
                            .display()
                            .to_string(),
                        schema_worker_sha256: request.schema_worker_sha256.clone(),
                        max_schema_receipts: limits.max_schema_receipts,
                        max_schema_receipt_bytes: limits.max_schema_receipt_bytes,
                        worker_cpu_seconds: limits.worker_cpu_seconds,
                        worker_address_space_bytes: limits.worker_address_space_bytes,
                    };
                    let projection_request = Request {
                        schema_version: CORPUS_PROJECTION_CHECK_REQUEST_SCHEMA.to_owned(),
                        mode: Some("check".to_owned()),
                        comparison_root: Some(request.repository_root.display().to_string()),
                        selected_snapshot: None,
                        source_only: None,
                        tmpfs_quota_bytes: limits.tmpfs_quota_bytes,
                        tmpfs_inode_limit: limits.tmpfs_inode_limit,
                        working_ram_bytes: limits.working_ram_bytes,
                        max_state_bytes: limits.max_state_bytes,
                        max_json_visits: limits.max_json_visits,
                        max_work_bytes: limits.max_work_bytes,
                        persistent_write_cap_bytes: limits.max_output_bytes,
                        max_build_seconds: limits.max_build_seconds,
                        cold_open: limits.cold_open,
                        process_limits: limits.process_limits,
                        data_directory: "projection-check-data".to_owned(),
                        private_release_directory: "projection-check-release".to_owned(),
                        evidence_refs: Vec::new(),
                        previous_native_snapshot: None,
                    };
                    let projection_limits = source_projection_limits(
                        &projection_source,
                        &projection_request,
                        deadline,
                    )?;
                    let recheck = || -> tos_compiler::Result<()> {
                        if cancelled.load(std::sync::atomic::Ordering::Relaxed)
                            || Instant::now() >= deadline
                            || cut.current().revision() != revision
                            || cut
                                .stream(revision)
                                .map_err(|error| tos_compiler::Error::Source(error.to_string()))?
                                .expectation()
                                .digest
                                .to_hex()
                                != profile.membership_root
                            || software.selection().source_git_commit != request.software_git_commit
                            || software.selection().source_git_tree != request.software_git_tree
                            || software.selection().capture_manifest_sha256
                                != capture.manifest_sha256
                        {
                            return Err(tos_compiler::Error::Invalid(
                                "native direct repository source selection changed",
                            ));
                        }
                        captured.recheck().map_err(|_| {
                            tos_compiler::Error::Invalid(
                                "native direct repository authored source changed",
                            )
                        })?;
                        for component in components.members() {
                            let bytes = software
                                .read_selected_component(
                                    &components,
                                    &component.path,
                                    SOFTWARE_SOURCE_BYTES,
                                    deadline,
                                    &cancelled,
                                )
                                .map_err(|_| {
                                    tos_compiler::Error::Invalid(
                                        "native direct selected software component changed",
                                    )
                                })?;
                            if bytes.len() as u64 != component.size_bytes
                                || Digest256::of_bytes(&bytes) != component.sha256
                            {
                                return Err(tos_compiler::Error::Invalid(
                                    "native direct selected software fixity changed",
                                ));
                            }
                        }
                        verify_direct_worker_image(&worker, limits.max_worker_image_bytes)
                            .map_err(|_| {
                                tos_compiler::Error::Invalid(
                                    "native direct selected schema worker image changed",
                                )
                            })?;
                        Ok(())
                    };
                    let stage_root = workspace.path();
                    let products = crate::source_corpus_index_projection::project(
                        cut,
                        &software,
                        &binding,
                        root_input,
                        worker.clone(),
                        projection_limits,
                        &isolation,
                        stage_root,
                        &recheck,
                        deadline,
                        &cancelled,
                    )?;
                    let consumed = consume(
                        &products,
                        DirectRepositoryProjectionContext {
                            cut,
                            software: &software,
                            components: &components,
                            recheck: &recheck,
                            deadline,
                            cancelled: &cancelled,
                        },
                    )?;
                    Ok((products, consumed))
                })();
                callback_result.map_err(|error| {
                    crate::source_command::SourceCommandError::DeniedWithReason(format!(
                        "native direct projection failed: {error}"
                    ))
                })
            },
        )?;
    Ok((products, consumed))
}

pub(crate) fn compose_direct_repository_projection(
    request: &DirectRepositoryProjectionRequest,
) -> Result<crate::source_corpus_index_projection::NativeCorpusIndexProducts> {
    with_direct_repository_projection(request, |_, _| Ok(())).map(|(products, ())| products)
}

pub(crate) fn check_direct_projection_products(
    request: &DirectRepositoryProjectionRequest,
    products: &crate::source_corpus_index_projection::NativeCorpusIndexProducts,
) -> Result<Value> {
    validate_direct_projection_request(request)?;
    let corpus = compare_direct_projection_product(
        &request.repository_root,
        manifest::CORPUS_INDEX_PATH,
        products.corpus.output_bytes(),
        request.limits.max_output_bytes,
    )?;
    let bibliographic = compare_direct_projection_product(
        &request.repository_root,
        manifest::CLAIM_GRAPH_PATH,
        &products.bibliographic_claims,
        request.limits.max_output_bytes,
    )?;
    Ok(json_object([
        ("corpus_index", corpus),
        ("bibliographic_claim_graph", bibliographic),
    ]))
}

fn check_direct_repository_projection(
    request: &DirectRepositoryProjectionRequest,
) -> Result<Value> {
    let (products, value) = with_direct_repository_projection(request, |products, _| {
        check_direct_projection_products(request, products)
    })?;
    Ok(json_object([
        (
            "schema_version",
            json!("tos_native_corpus_projection_check_result_v1"),
        ),
        (
            "comparison_root",
            json!(request.repository_root.display().to_string()),
        ),
        (
            "outcome",
            json!("corpus-index-and-bibliographic-graph-match"),
        ),
        ("software_git_commit", json!(request.software_git_commit)),
        ("software_git_tree", json!(request.software_git_tree)),
        (
            "schema_worker_path",
            json!(request.schema_worker_absolute_path.display().to_string()),
        ),
        ("schema_worker_sha256", json!(request.schema_worker_sha256)),
        (
            "source_revision",
            json!(products.bibliographic_receipt.source_revision),
        ),
        (
            "source_membership_sha256",
            json!(products.bibliographic_receipt.source_membership_sha256),
        ),
        ("persistent_write_performed", json!(false)),
        ("grants_source_admission", json!(false)),
        ("products", value),
    ]))
}

fn graph_nodes_mut(value: &mut Value) -> Result<&mut [Value]> {
    value
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .map(Vec::as_mut_slice)
        .ok_or_else(|| Refusal("assessed candidate graph nodes are absent").into())
}

fn graph_nodes(value: &Value) -> Result<&[Value]> {
    value
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| Refusal("assessed candidate graph nodes are absent").into())
}

fn requested_selections(
    product: &str,
    wanted: &BTreeSet<String>,
    corpus: &Value,
    bibliographic: &Value,
) -> Result<(
    BTreeMap<String, AssessedCarrierSelection>,
    BTreeMap<String, AssessedCarrierSelection>,
    Vec<AssessedCarrierSelection>,
)> {
    let corpus_selected = selections_from_nodes(graph_nodes(corpus)?, wanted)?;
    let bibliographic_selected = selections_from_nodes(graph_nodes(bibliographic)?, wanted)?;
    let require_all = |selected: &BTreeMap<String, AssessedCarrierSelection>| -> Result<()> {
        if selected.len() != wanted.len() {
            return Err(
                Refusal("assessed candidate form ID is absent from its product carriers").into(),
            );
        }
        Ok(())
    };
    let mut union = BTreeMap::new();
    match product {
        "corpus" => {
            require_all(&corpus_selected)?;
            union.extend(corpus_selected.clone());
        }
        "bibliographic" => {
            require_all(&bibliographic_selected)?;
            union.extend(bibliographic_selected.clone());
        }
        "paired" => {
            if corpus_selected.is_empty() && bibliographic_selected.is_empty() {
                return Err(
                    Refusal("assessed candidate forms are absent from paired products").into(),
                );
            }
            for (identity, selection) in &corpus_selected {
                union.insert(identity.clone(), selection.clone());
            }
            for (identity, selection) in &bibliographic_selected {
                if let Some(corpus_selection) = union.get(identity) {
                    if corpus_selection != selection {
                        return Err(Refusal("paired assessed form carrier bindings differ").into());
                    }
                } else {
                    union.insert(identity.clone(), selection.clone());
                }
            }
            if union.len() != wanted.len() {
                return Err(
                    Refusal("assessed candidate form ID is absent from paired carriers").into(),
                );
            }
        }
        _ => return Err(Refusal("assessed candidate product selection differs").into()),
    }
    let expected = union.into_values().collect::<Vec<_>>();
    Ok((corpus_selected, bibliographic_selected, expected))
}

fn read_candidate_bytes(target: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let (mut file, stamp) = open_regular(target, max_bytes, rustix::process::getuid().as_raw())?;
    if stamp.mode & 0o077 != 0 {
        return Err(Refusal("assessed candidate existing output is not private").into());
    }
    let mut raw = Vec::with_capacity(
        usize::try_from(stamp.size).map_err(|_| Refusal("assessed candidate output size range"))?,
    );
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut raw)?;
    if raw.len() as u64 != stamp.size
        || Stamp::from(&file.metadata()?) != stamp
        || Stamp::from(&fs::symlink_metadata(target)?) != stamp
    {
        return Err(Refusal("assessed candidate existing output changed while reading").into());
    }
    Ok(raw)
}

fn execute_assessed_candidate(request: AssessedCandidateRequest) -> Result<Value> {
    validate_direct_projection_request(&request.projection)?;
    let canonical_root = request.projection.repository_root.canonicalize()?;
    if canonical_root != request.projection.repository_root {
        return Err(Refusal("assessed candidate repository root must be canonical").into());
    }
    validate_candidate_target(&request.output, &canonical_root)?;
    let wanted = request
        .assessed_form_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let (products, prepared) = with_direct_repository_projection(
        &request.projection,
        |products, context| {
            let source_revision = context.cut.current().revision().0.to_prefixed();
            let selected = selected_assessment_invocation(
                &request.native_invocation,
                &request.projection,
                context.deadline,
                context.cancelled,
            )?;
            if selected.source_revision != source_revision {
                return Err(Refusal(
                    "assessed candidate invocation source revision differs from direct capture",
                )
                .into());
            }
            let mut corpus_value = products.corpus.value().clone();
            let mut bibliographic_value: Value =
                serde_json::from_slice(&products.bibliographic_claims)
                    .map_err(|_| Refusal("assessed candidate bibliographic graph JSON"))?;
            let (corpus_selected, bibliographic_selected, expected) = requested_selections(
                &request.product,
                &wanted,
                &corpus_value,
                &bibliographic_value,
            )?;
            if expected.is_empty() || expected.len() > 256 {
                return Err(Refusal("assessed candidate selected form count differs").into());
            }
            let batch_request = native_assessed_batch_request(&expected)?;
            selected.verify_current(context.deadline, context.cancelled)?;
            let batch = crate::source_native_cli::private_assessment::run_public_v2_batch(
                &selected.value,
                &batch_request,
                &selected.store,
                context.cut,
                &selected.software,
                &selected.components,
                Some(&request.projection.repository_root),
                context.deadline,
                context.cancelled,
            )
            .map_err(|_| Refusal("selected native assessment batch refused"))?;
            let packets = assessed_packets(&batch, &expected)?;
            let corpus_packets = packets
                .iter()
                .filter(|(identity, _)| corpus_selected.contains_key(*identity))
                .map(|(identity, packet)| (identity.clone(), packet.clone()))
                .collect::<BTreeMap<_, _>>();
            let bibliographic_packets = packets
                .iter()
                .filter(|(identity, _)| bibliographic_selected.contains_key(*identity))
                .map(|(identity, packet)| (identity.clone(), packet.clone()))
                .collect::<BTreeMap<_, _>>();
            if matches!(request.product.as_str(), "corpus" | "paired") {
                apply_assessed_packets(
                    graph_nodes_mut(&mut corpus_value)?,
                    &corpus_packets,
                    256 * 1024,
                )?;
            }
            if matches!(request.product.as_str(), "bibliographic" | "paired") {
                apply_assessed_packets(
                    graph_nodes_mut(&mut bibliographic_value)?,
                    &bibliographic_packets,
                    256 * 1024,
                )?;
            }
            let candidate = match request.product.as_str() {
                "corpus" => corpus_value,
                "bibliographic" => bibliographic_value,
                "paired" => json!({
                    "schema_version": "tos_native_assessed_paired_candidate_v1",
                    "source_revision": source_revision,
                    "source_membership_sha256": products.bibliographic_receipt.source_membership_sha256,
                    "corpus_index": corpus_value,
                    "bibliographic_claim_graph": bibliographic_value,
                }),
                _ => return Err(Refusal("assessed candidate product selection differs").into()),
            };
            let rendered =
                render_candidate(&candidate, request.projection.limits.max_output_bytes)?;
            let output_sha256 = Digest256::of_bytes(&rendered).to_hex();
            let snapshot = json_text(&batch, "owner_snapshot")?.to_owned();
            let revision = json_text(&batch, "owner_snapshot")?.to_owned();
            let check = request.operation == "check";
            if check {
                let existing = read_candidate_bytes(
                    &request.output,
                    request.projection.limits.max_output_bytes,
                )?;
                if existing != rendered {
                    return Err(Refusal(
                        "assessed candidate existing output does not match current projection",
                    )
                    .into());
                }
                verify_assessed_replay(
                    &selected,
                    &batch_request,
                    &batch,
                    context.cut,
                    context.software,
                    context.components,
                    &request.projection.repository_root,
                    context.recheck,
                    context.deadline,
                    context.cancelled,
                )?;
                return Ok((
                    json!({
                        "schema_version": ASSESSED_CANDIDATE_RESULT_SCHEMA,
                        "operation": "check",
                        "product": request.product,
                        "output": request.output,
                        "output_bytes": rendered.len(),
                        "output_sha256": output_sha256,
                        "source_revision": source_revision,
                        "source_membership_sha256": products.bibliographic_receipt.source_membership_sha256,
                        "owner_snapshot": snapshot,
                        "assessed_form_ids": request.assessed_form_ids,
                        "matches": true,
                        "persistent_write_performed": false,
                        "grants_source_admission": false,
                    }),
                    None,
                ));
            }
            let publication =
                write_assessed_candidate_with_hook(&request.output, &rendered, || {
                    verify_assessed_replay(
                        &selected,
                        &batch_request,
                        &batch,
                        context.cut,
                        context.software,
                        context.components,
                        &request.projection.repository_root,
                        context.recheck,
                        context.deadline,
                        context.cancelled,
                    )
                })?;
            let receipt = json!({
                "schema_version": ASSESSED_CANDIDATE_RESULT_SCHEMA,
                "operation": "publish",
                "product": request.product,
                "output": request.output,
                "output_bytes": rendered.len(),
                "output_sha256": output_sha256,
                "source_revision": source_revision,
                "source_membership_sha256": products.bibliographic_receipt.source_membership_sha256,
                "owner_snapshot": snapshot,
                "assessed_form_ids": request.assessed_form_ids,
                "persistent_write_performed": true,
                "grants_source_admission": false,
            });
            let _ = revision;
            Ok((receipt, Some(publication)))
        },
    )?;
    let _ = products;
    if let Some(publication) = prepared.1 {
        publication.commit();
    }
    Ok(prepared.0)
}

fn verify_assessed_replay(
    selected: &SelectedAssessmentInvocation,
    batch_request: &[u8],
    expected: &Value,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    expected_source_root: &Path,
    recheck: &dyn Fn() -> tos_compiler::Result<()>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    selected.verify_current(deadline, cancelled)?;
    let replay = crate::source_native_cli::private_assessment::run_public_v2_batch(
        &selected.value,
        batch_request,
        &selected.store,
        cut,
        &selected.software,
        &selected.components,
        Some(expected_source_root),
        deadline,
        cancelled,
    )
    .map_err(|_| Refusal("selected native assessment replay refused"))?;
    if replay != *expected {
        return Err(Refusal(
            "selected native assessment replay changed before candidate publication",
        )
        .into());
    }
    selected.verify_current(deadline, cancelled)?;
    for component in components.members() {
        let raw = software
            .read_selected_component(&components, &component.path, 131_072, deadline, cancelled)
            .map_err(|_| {
                Refusal("direct selected software component changed before candidate publication")
            })?;
        if raw.len() as u64 != component.size_bytes || Digest256::of_bytes(&raw) != component.sha256
        {
            return Err(Refusal(
                "direct selected software component changed before candidate publication",
            )
            .into());
        }
    }
    recheck()
        .map_err(|_| Refusal("direct captured source changed before candidate publication"))?;
    Ok(())
}

fn report_assessed_candidate(
    result: Result<Value>,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    match result {
        Ok(value) => match serde_json::to_vec(&value) {
            Ok(raw) if raw.len() <= MAX_RESULT_BYTES => {
                if output.write_all(&raw).is_err() || output.write_all(b"\n").is_err() {
                    let _ = diagnostics.write_all(b"native assessed candidate output failed\n");
                    2
                } else {
                    0
                }
            }
            _ => {
                let _ = diagnostics
                    .write_all(b"native assessed candidate receipt exceeded output cap\n");
                2
            }
        },
        Err(error) => {
            let _ = writeln!(diagnostics, "native assessed candidate refused: {error}");
            2
        }
    }
}

/// Build or byte-check a private assessed graph candidate from one captured
/// source projection and the explicitly pinned native public-v2 assessment
/// invocation. `check` is read-only; `publish` commits by no-replace link only
/// after the staged file is synced and the full assessment/source fence passes.
pub fn run_corpus_assessed_candidate(
    input: impl Read,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    let result = read_assessed_candidate_request(input).and_then(execute_assessed_candidate);
    report_assessed_candidate(result, output, diagnostics)
}

pub fn run_corpus_assessed_candidate_args(
    args: &[OsString],
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    if args.len() == 1 && matches!(args[0].to_str(), Some("--help" | "-h")) {
        let usage = b"usage: tos-native-owner-command corpus-assessed-candidate --request ABS_JSON\n       tos-native-owner-command corpus-assessed-candidate < REQUEST_JSON\n\nRequest operation is publish or check; product is bibliographic, corpus or paired. The request includes the source projection and an absolute path+SHA-256 pin to the native assessment-read invocation. Publish creates one private no-replace candidate; check reads and compares an existing candidate without writing. Neither route admits source or publishes a standard export.\n";
        return if output.write_all(usage).is_ok() {
            0
        } else {
            2
        };
    }
    let result = if args.is_empty() {
        read_assessed_candidate_request(std::io::stdin().lock())
    } else if args.len() == 2 && args[0].to_str() == Some("--request") {
        let path = args[1]
            .to_str()
            .filter(|value| absolute_bounded(value))
            .map(Path::new)
            .ok_or(Refusal(
                "assessed candidate request path must be absolute UTF-8",
            ));
        path.map_err(|error| -> Box<dyn StdError> { Box::new(error) })
            .and_then(read_assessed_candidate_request_file)
    } else {
        Err(Refusal("assessed candidate CLI requires --request ABS_JSON or stdin").into())
    }
    .and_then(execute_assessed_candidate);
    report_assessed_candidate(result, output, diagnostics)
}

fn execute(mut request: Request) -> Result<Value> {
    validate_request(&request)?;
    let started = Instant::now();
    let deadline = started
        .checked_add(Duration::from_secs(request.max_build_seconds))
        .ok_or(Refusal("native Original producer deadline arithmetic"))?;
    let uid = rustix::process::getuid().as_raw();
    if rustix::process::geteuid().as_raw() != uid {
        return Err(Refusal("native Original producer refuses setuid execution").into());
    }

    // This exact host ticket must be selected before any source capture or
    // writer. Its selected persistent filesystem is the only output root.
    let isolation = PrivateTmpfsStageIsolation::select_from_environment(
        request.tmpfs_quota_bytes,
        request.tmpfs_inode_limit,
        request.working_ram_bytes,
    )?;
    let persistent_store_path = isolation
        .persistent_store()
        .ok_or(Refusal(
            "private tmpfs ticket has no persistent output store",
        ))?
        .to_owned();
    isolation.verify_persistent_store(&persistent_store_path)?;
    no_symlink_path(&persistent_store_path)?;
    let store_metadata = fs::symlink_metadata(&persistent_store_path)?;
    if !store_metadata.is_dir() || store_metadata.uid() != uid || store_metadata.mode() & 0o077 != 0
    {
        return Err(Refusal("persistent candidate store must be owner-private").into());
    }
    let store = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(&persistent_store_path)?;
    let held_store = store.metadata()?;
    if !held_store.is_dir()
        || held_store.uid() != uid
        || held_store.dev() != store_metadata.dev()
        || held_store.ino() != store_metadata.ino()
    {
        return Err(Refusal("persistent store held/name identity changed").into());
    }
    let data_root = persistent_store_path.join(&request.data_directory);
    let private_release_root = persistent_store_path.join(&request.private_release_directory);
    if data_root == private_release_root
        || data_root.starts_with(&private_release_root)
        || private_release_root.starts_with(&data_root)
    {
        return Err(Refusal(
            "candidate data and private release namespaces must be fresh and disjoint",
        )
        .into());
    }
    match rustix::fs::statat(
        &store,
        request.data_directory.as_str(),
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(rustix::io::Errno::NOENT) => (),
        Ok(_) => {
            return Err(Refusal("native data destination must be absent before production").into());
        }
        Err(error) => return Err(io::Error::from(error).into()),
    }
    match rustix::fs::statat(
        &store,
        request.private_release_directory.as_str(),
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(rustix::io::Errno::NOENT) => (),
        Ok(_) => return Err(Refusal("private release destination must remain absent").into()),
        Err(error) => return Err(io::Error::from(error).into()),
    }

    let fingerprint_before = manifest::fingerprint_native_compiler_source(deadline)?;
    let mut limits = manifest::portable_native_snapshot_limits(request.max_build_seconds)?;
    limits.capture.max_work_bytes = request.max_work_bytes;
    // Search participates in the same original cumulative work counter. Its
    // local ceiling must not silently retain a smaller generic default.
    limits.full.search.max_work_bytes = request.max_work_bytes;
    // Each stored posting consumes physical space in its bounded block/row.
    // Keep a finite, conservative count envelope tied to the selected cold
    // byte cap; actual MAIN, TEMP and final cold-size checks remain decisive.
    limits.full.search.max_postings = request.cold_open.max_file_bytes;
    // The private stage's live main and TEMP databases use the maintained
    // compiler limits covered by the tmpfs/RAM hold. The caller's cold-file
    // ceiling applies to the finished selected model after compaction.
    let cancelled = Arc::new(AtomicBool::new(false));
    let resources = LinuxCgroupColdOpenResourceHold::acquire_original_stage(
        request.working_ram_bytes,
        request.tmpfs_quota_bytes,
        deadline,
        Arc::clone(&cancelled),
    )?;
    let historical = request
        .selected_snapshot
        .as_ref()
        .map(|profile| manifest::census_selected_runtime_closure(profile, deadline))
        .transpose()?;
    let source_runtime = request
        .source_only
        .as_ref()
        .map(|source| {
            prepare_source_only_runtime(source, &request, &isolation, deadline, &cancelled)
        })
        .transpose()?;
    if request.mode.as_deref() == Some("check") {
        let runtime = source_runtime
            .as_ref()
            .ok_or(Refusal("native corpus check has no source-only runtime"))?;
        runtime.recheck(
            request
                .source_only
                .as_ref()
                .ok_or(Refusal("native corpus check source selection absent"))?,
            deadline,
            cancelled.as_ref(),
        )?;
        let fingerprint_after = manifest::fingerprint_native_compiler_source(deadline)?;
        manifest::require_stable_native_compiler_source(&fingerprint_before, &fingerprint_after)?;
        let comparison_root = request
            .comparison_root
            .as_deref()
            .ok_or(Refusal("native corpus check comparison root absent"))?;
        return check_source_products(runtime, Path::new(comparison_root), deadline);
    }
    let mut evidence_refs =
        hold_evidence_refs(std::mem::take(&mut request.evidence_refs), deadline, uid)?;
    let selected_source_bytes = match (&historical, &source_runtime, &request.selected_snapshot) {
        (Some(census), _, Some(_)) => census.retained_state_upper_bound()?,
        (_, Some(runtime), None) => runtime.retained_state_upper_bound()?,
        _ => return Err(Refusal("native producer selected source mode differs").into()),
    };
    let mut caller_bytes = isolation
        .retained_state_upper_bound()?
        .checked_add(resources.retained_state_upper_bound()?)
        .and_then(|n| n.checked_add(selected_source_bytes))
        .and_then(|n| n.checked_add(fingerprint_before.retained_state_upper_bound().ok()?))
        .ok_or(Refusal("native Original retained owner census overflow"))?;
    caller_bytes = caller_bytes
        .checked_add(evidence_refs.capacity() * std::mem::size_of::<HeldEvidenceRef>())
        .ok_or(Refusal("native Original evidence slots overflow"))?;
    for evidence in &evidence_refs {
        caller_bytes = caller_bytes
            .checked_add(
                evidence.kind.capacity() + evidence.path.capacity() + evidence.sha256.capacity(),
            )
            .ok_or(Refusal("native Original evidence owner census overflow"))?;
    }
    caller_bytes = caller_bytes
        .checked_add(
            std::mem::size_of::<Request>()
                + request
                    .selected_snapshot
                    .as_ref()
                    .map(manifest::NativeSelectedSnapshotProfile::retained_state_upper_bound)
                    .transpose()?
                    .unwrap_or(0)
                + request
                    .source_only
                    .as_ref()
                    .map(NativeSourceOnlyRequest::retained_state_upper_bound)
                    .transpose()?
                    .unwrap_or(0)
                + request
                    .previous_native_snapshot
                    .as_ref()
                    .map(manifest::NativePreviousDataSnapshotProfile::retained_state_upper_bound)
                    .transpose()?
                    .unwrap_or(0)
                + request.data_directory.capacity()
                + request.private_release_directory.capacity()
                + persistent_store_path.capacity()
                + data_root.capacity()
                + private_release_root.capacity(),
        )
        .ok_or(Refusal("native Original request owner census overflow"))?;
    let retained = Cell::new(caller_bytes);
    let remaining = |extra: usize| {
        request
            .max_state_bytes
            .checked_sub(retained.get())
            .and_then(|n| n.checked_sub(extra))
            .ok_or(tos_compiler::Error::Budget(
                "native Original simultaneous state",
            ))
    };
    let heap = DedicatedSessionSqliteHeap::establish(
        tos_compiler::dedicated_session_heap_bytes(request.max_state_bytes)?,
        &remaining,
        deadline,
        cancelled.as_ref(),
    )?;
    retained.set(
        retained
            .get()
            .checked_add(heap.reserved_state_bytes())
            .ok_or(Refusal("native Original SQLite owner census overflow"))?,
    );
    let work = Arc::new(AtomicU64::new(0));
    let vm = Arc::new(AtomicU64::new(0));
    let mut capture_usage = RuntimeCaptureCreationUsage::default();
    let temp_paths = TempStagePaths {
        capture: isolation.root().join("tos-native-original-capture.sqlite3"),
        native: isolation.root().join("tos-native-original-model.sqlite3"),
    };
    retained.set(
        retained
            .get()
            .checked_add(
                std::mem::size_of::<TempStagePaths>()
                    + temp_paths.capture.capacity()
                    + temp_paths.native.capacity(),
            )
            .ok_or(Refusal("native Original stage paths overflow"))?,
    );
    remaining(0)?;
    for path in [&temp_paths.capture, &temp_paths.native] {
        if path.exists() || path.is_symlink() {
            return Err(Refusal("native Original private tmpfs candidates must be fresh").into());
        }
    }
    let capture_source_root = match (&historical, &source_runtime) {
        (Some(census), None) => census.source_root(),
        (None, Some(runtime)) => runtime.source_root.as_path(),
        _ => return Err(Refusal("native producer source root mode differs").into()),
    };
    let capture = PublicCapture::create_runtime_with_owned_budget(
        capture_source_root,
        &temp_paths.capture,
        limits.capture,
        deadline,
        Arc::clone(&cancelled),
        RuntimeCaptureOwnedBudget {
            remaining_after_retained: &remaining,
            original_work: work,
            original_work_limit: limits.capture.max_work_bytes,
            creation_work_allowance: limits.capture.max_work_bytes,
            original_sql_vm: vm,
            original_sql_vm_limit: limits.capture.max_sql_vm_steps,
            original_sqlite_heap: Arc::clone(&heap),
            max_creation_json_visits: request.max_json_visits,
            creation_deadline: deadline,
        },
        &mut capture_usage,
    )?;
    retained.set(
        retained
            .get()
            .checked_add(capture.retained_state_upper_bound()?)
            .ok_or(Refusal("native Original capture owner census overflow"))?,
    );
    let mut selected_source = match (
        request.selected_snapshot.as_ref(),
        historical,
        source_runtime,
    ) {
        (Some(profile), Some(census), None) => SelectedSource::Historical {
            profile: profile.clone(),
            census,
        },
        (None, None, Some(runtime)) => {
            let census = manifest::NativeSourceOnlySnapshotCensus::open(
                runtime.profile.clone(),
                &capture,
                deadline,
            )?;
            retained.set(
                retained
                    .get()
                    .checked_add(census.retained_state_upper_bound()?)
                    .ok_or(Refusal("native source-only census retained state overflow"))?,
            );
            remaining(0)?;
            SelectedSource::Current { runtime, census }
        }
        _ => return Err(Refusal("native producer selected source mode differs").into()),
    };
    let captured_members = selected_source.validate_capture_closure(&capture, deadline)?;
    if captured_members.len() < selected_source.member_count()
        || captured_members.len() > manifest::NATIVE_PRODUCER_MAX_MEMBERS
    {
        return Err(Refusal("native source closure census identity differs").into());
    }
    capture.verify_inputs(limits.capture)?;
    selected_source.recheck(&request, deadline, cancelled.as_ref())?;

    // All original source bytes are selected from this retained capture. The
    // old Python SQLite member was excluded during metadata census and is
    // neither read nor copied.
    let source_bytes = captured_members
        .iter()
        .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
        .ok_or(Refusal("native source closure byte arithmetic"))?;
    if source_bytes > manifest::NATIVE_PRODUCER_MAX_SOURCE_CLOSURE_BYTES {
        return Err(Refusal("native source closure byte ceiling").into());
    }
    let payload_layout = tos_compiler::knowledge_stage::KnowledgePayloadLayout::CarrierOnceV3;
    let model_abi = payload_layout
        .carrier_model_abi()
        .ok_or(Refusal("native producer model ABI absent"))?;
    let member_paths = data_member_paths(&captured_members);
    let provisional_bindings = selected_source.raw_source_bindings();
    let cache_input = selected_source.manifest_input(
        model_abi,
        &fingerprint_before,
        &provisional_bindings,
        &member_paths,
    );
    let data_cap = request
        .persistent_write_cap_bytes
        .min(manifest::NATIVE_PRODUCER_MAX_DATA_BYTES);
    let manifest_limits = manifest::NativeDataManifestLimits {
        max_manifest_bytes: MAX_SELECTION_BYTES,
        max_members: manifest::NATIVE_PRODUCER_MAX_MEMBERS,
        max_member_bytes: data_cap,
        max_total_data_bytes: data_cap,
        deadline,
    };
    let expected_source_revision = capture.core_source_revision()?;
    let native_cache = match &request.previous_native_snapshot {
        Some(profile) => manifest::verify_previous_native_data_snapshot(
            profile,
            &cache_input,
            &expected_source_revision,
            manifest_limits,
        )?,
        None => None,
    };
    if let Some(cache) = &native_cache {
        retained.set(
            retained
                .get()
                .checked_add(cache.retained_state_upper_bound()?)
                .ok_or(Refusal("native cached selection owner census overflow"))?,
        );
        remaining(0)?;
    }
    let remaining_visits = request
        .max_json_visits
        .checked_sub(capture_usage.json_visits)
        .filter(|n| *n > 0)
        .ok_or(Refusal("native Original capture exhausted JSON owner"))?;
    let mut producer_usage = native_snapshot::NativeSnapshotCreationUsage::default();
    let mut result = None;
    let mut completion_error = None;
    let consume = |snapshot_output: &native_snapshot::NativeSnapshotOutput<'_>,
                   loan: native_snapshot::NativeSnapshotOwnedReadLoan<'_, '_>| {
        let mut finish = || -> Result<Value> {
            let captured_members = selected_source.validate_capture_closure(&capture, deadline)?;
            let corpus_original = snapshot_output
                .corpus_original()
                .ok_or(Refusal("native corpus Original receipt absent"))?;
            let source_bindings =
                selected_source.source_bindings(corpus_original, &captured_members, deadline)?;
            capture.verify_inputs(limits.capture)?;
            selected_source.recheck(&request, deadline, cancelled.as_ref())?;
            let fingerprint_after_build = manifest::fingerprint_native_compiler_source(deadline)?;
            manifest::require_stable_native_compiler_source(
                &fingerprint_before,
                &fingerprint_after_build,
            )?;

            let model_bytes = snapshot_output.stage().sqlite_size_bytes;
            if model_bytes == 0
                || model_bytes > manifest::NATIVE_PRODUCER_MAX_MODEL_BYTES
                || model_bytes > request.cold_open.max_file_bytes
            {
                return Err(Refusal("completed native model exceeds selected ceiling").into());
            }
            let manifest_input = selected_source.manifest_input(
                &snapshot_output.expectation().model_abi,
                &fingerprint_before,
                &source_bindings,
                &member_paths,
            );
            let manifest_metadata_upper =
                manifest::preflight_native_data_manifest(&manifest_input, manifest_limits)? as u64;
            let persistent_candidate_upper = source_bytes
                .checked_add(model_bytes)
                .and_then(|sum| sum.checked_add(MAX_SELECTION_BYTES as u64))
                .and_then(|sum| sum.checked_add(manifest_metadata_upper))
                .filter(|sum| *sum <= data_cap)
                .ok_or(Refusal(
                    "512 MiB persistent candidate ceiling refuses before model copy",
                ))?;
            if persistent_candidate_upper > request.persistent_write_cap_bytes {
                return Err(
                    Refusal("persistent write reservation is below candidate upper bound").into(),
                );
            }
            active(deadline)?;
            isolation.verify_persistent_store(&persistent_store_path)?;
            let mut output = create_output_tree(
                &persistent_store_path,
                &store,
                &request.data_directory,
                uid,
                data_cap,
            )?;
            for member in &captured_members {
                active(deadline)?;
                let raw = capture.read_retained_input(
                    &member.source_path,
                    manifest::NATIVE_PRODUCER_MEMBER_READ_CAP_BYTES,
                )?;
                if raw.len() as u64 != member.size_bytes
                    || Digest256::of_bytes(&raw).to_hex() != member.sha256
                {
                    return Err(Refusal("captured member changed before private copy").into());
                }
                output.write_member(&format!("data/{}", member.source_path), &raw, deadline)?;
            }
            output.copy_model_once(
                snapshot_output.artifact_path(),
                &snapshot_output.stage().sqlite_sha256,
                model_bytes,
                deadline,
            )?;

            let selection_paths = NativeSelectionPaths {
                model: manifest::NATIVE_MODEL_PATH.to_owned(),
                descriptor: "data/ToS/doctrine/semantic-interchange/query-vocabulary.v1.json"
                    .into(),
                entity_registry: "data/ToS/doctrine/semantic-interchange/entity-types.v1.json"
                    .into(),
                relation_registry: "data/ToS/doctrine/semantic-interchange/relation-types.v1.json"
                    .into(),
            };
            let model_path = output.root_path().join(&selection_paths.model);
            let selection = match snapshot_output {
                native_snapshot::NativeSnapshotOutput::Built(completed) => completed
                    .selection_for_copied_model(
                        &model_path,
                        selection_paths,
                        request.cold_open,
                        request.process_limits,
                        MAX_SELECTION_BYTES,
                    )?,
                native_snapshot::NativeSnapshotOutput::Reused(reused) => reused
                    .selection_for_copied_model(
                        &model_path,
                        request.cold_open,
                        request.process_limits,
                        MAX_SELECTION_BYTES,
                    )?,
            };
            if selection.producer().corpus_original.is_none()
                || selection.producer().philosophy_original.is_none()
                || selection.producer().managed_source.is_some()
                || selection.producer().managed_source_v2.is_some()
            {
                return Err(Refusal(
                    "completed native selection does not carry the exact Original pair",
                )
                .into());
            }
            let selection_raw = selection.encode(MAX_SELECTION_BYTES)?;
            let selection_digest = Digest256::of_bytes(&selection_raw).to_hex();
            let selection_value: Value = serde_json::from_slice(&selection_raw)?;
            output.write_member(manifest::NATIVE_SELECTION_PATH, &selection_raw, deadline)?;
            let estimated_output = output
                .written_bytes
                .checked_add(manifest_metadata_upper)
                .filter(|sum| *sum <= data_cap)
                .ok_or(Refusal("candidate data plus manifest metadata ceiling"))?;
            let output_file_list_bytes = output.written_bytes;
            if output_file_list_bytes > data_cap
                || estimated_output > request.persistent_write_cap_bytes
            {
                return Err(Refusal("candidate data premanifest ceiling").into());
            }

            // This is real controlled cold admission of the copied fs-verity model
            // under the same state/heap/work/VM owners as the Original writer.
            // The returned metrics describe this invocation only; the resulting
            // receipt conveys mechanics, not rights, canon or source semantic approval.
            let expectation: KnowledgeSelectedExpectation = selection.expectation().clone();
            let fs_verity: NativeFsVerityMeasurement = selection.fs_verity().clone();
            let named_model_before = private_model_path_stamp(&model_path, uid, model_bytes)?;
            let cold_open_start_elapsed_ns = elapsed_ns(started);
            let custody_observer = Arc::new(ObservingNativeCustody::new(
                LinuxFsVerityCustody::new(fs_verity.clone(), request.process_limits)?,
                started,
            ));
            let custody: Arc<dyn ImmutableKnowledgeCustody> = custody_observer.clone();
            let mut pinned_model = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&model_path)?;
            let mut cold_receipt = None;
            let cold_check = |model: &mut tos_compiler::ControlledKnowledgeModel<'_, '_, '_>| {
                if model.corpus_original_receipt().is_none()
                    || model.philosophy_original_receipt().is_none()
                    || model.navigation_original_receipt().is_none()
                {
                    return Err(tos_compiler::Error::Invalid(
                        "native controlled cold Original roots absent",
                    ));
                }
                model.check_pin()?;
                let source_basis = serde_json::to_value(model.source_basis()).map_err(|_| {
                    tos_compiler::Error::Invalid("native controlled cold source basis encoding")
                })?;
                cold_receipt = Some((
                    model.cold_digest_read_bytes(),
                    model.cold_validation_charged_bytes(),
                    model.open_vm_steps(),
                    source_basis,
                ));
                Ok(())
            };
            match snapshot_output {
                native_snapshot::NativeSnapshotOutput::Built(completed) => {
                    completed.with_controlled_copied_knowledge_model(
                        &loan,
                        &isolation,
                        &mut pinned_model,
                        custody.as_ref(),
                        request.cold_open,
                        request.process_limits,
                        request.working_ram_bytes,
                        &resources,
                        deadline,
                        cold_check,
                    )?;
                }
                native_snapshot::NativeSnapshotOutput::Reused(_) => {
                    loan.with_controlled_selected_knowledge_model(
                        &mut pinned_model,
                        selection.expectation(),
                        custody.as_ref(),
                        request.cold_open,
                        request.process_limits,
                        request.working_ram_bytes,
                        &resources,
                        deadline,
                        cold_check,
                    )?;
                }
            }
            drop(pinned_model);
            let (
                cold_digest_read_bytes,
                cold_validation_charged_bytes,
                open_vm_steps,
                source_basis,
            ) = cold_receipt.ok_or(Refusal("native controlled cold receipt absent"))?;
            let cold_source_revision = snapshot_output.source_revision().to_owned();
            let corpus_original =
                serde_json::to_value(selection.producer().corpus_original.as_ref())?;
            let philosophy_original =
                serde_json::to_value(selection.producer().philosophy_original.as_ref())?;
            let named_model_after = private_model_path_stamp(&model_path, uid, model_bytes)?;
            let (held_fd_first, held_fd_last, custody_verify_calls) =
                custody_observer.snapshot()?;
            if custody_verify_calls < 2
                || named_model_before != held_fd_first.stamp
                || held_fd_first.stamp != held_fd_last.stamp
                || held_fd_last.stamp != named_model_after
                || held_fd_first.producer_elapsed_ns < cold_open_start_elapsed_ns
            {
                return Err(
                    Refusal("cold selected model held/name custody interval differs").into(),
                );
            }
            let cold_open_end_elapsed_ns = elapsed_ns(started);
            if held_fd_last.producer_elapsed_ns > cold_open_end_elapsed_ns {
                return Err(
                    Refusal("cold selected model observation clock ordering differs").into(),
                );
            }

            let manifest_receipt = match snapshot_output {
                native_snapshot::NativeSnapshotOutput::Built(completed) => {
                    manifest::write_completed_native_data_manifest(
                        completed,
                        output.root_path(),
                        &manifest_input,
                        &selection,
                        manifest_limits,
                    )?
                }
                native_snapshot::NativeSnapshotOutput::Reused(reused) => {
                    manifest::write_reused_native_data_manifest(
                        reused,
                        output.root_path(),
                        &manifest_input,
                        &selection,
                        manifest_limits,
                    )?
                }
            };
            let total_output_bytes = output
                .written_bytes
                .checked_add(manifest_receipt.manifest_bytes)
                .ok_or(Refusal("native candidate total output arithmetic"))?;
            if total_output_bytes > data_cap
                || total_output_bytes > request.persistent_write_cap_bytes
            {
                return Err(Refusal("candidate manifest exceeds persistent-write ceiling").into());
            }
            output.root.sync_all()?;
            output.store.sync_all()?;
            isolation.verify_persistent_store(&persistent_store_path)?;
            match rustix::fs::statat(
                &store,
                request.private_release_directory.as_str(),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Err(rustix::io::Errno::NOENT) => (),
                Ok(_) => {
                    return Err(Refusal(
                        "private release destination was created during production",
                    )
                    .into());
                }
                Err(error) => return Err(io::Error::from(error).into()),
            }
            output.verify_root()?;
            capture.verify_inputs(limits.capture)?;
            selected_source.recheck(&request, deadline, cancelled.as_ref())?;
            let fingerprint_after_cold = manifest::fingerprint_native_compiler_source(deadline)?;
            manifest::require_stable_native_compiler_source(
                &fingerprint_before,
                &fingerprint_after_cold,
            )?;
            for evidence in &mut evidence_refs {
                evidence.recheck(deadline)?;
            }
            active(deadline)?;

            let evidence_outputs = evidence_refs
                .iter()
                .map(HeldEvidenceRef::output)
                .collect::<Vec<_>>();
            let source_cut = snapshot_output.expectation().source_cut.clone();
            let authority = json_object([
                ("source_admission", json!(false)),
                ("rights_admission", json!(false)),
                ("canon_acceptance", json!(false)),
                ("semantic_acceptance", json!(false)),
                ("publication", json!(false)),
            ]);
            let data_manifest = json_object([
                ("path", json!(output.root_path().join("data/manifest.json"))),
                ("sha256", json!(manifest_receipt.manifest_sha256)),
                ("data_revision", json!(manifest_receipt.data_revision)),
                ("bytes", json!(manifest_receipt.manifest_bytes)),
                ("member_count", json!(manifest_receipt.member_count)),
                ("member_bytes", json!(manifest_receipt.member_bytes)),
                (
                    "native_selection_path",
                    json!(manifest::NATIVE_SELECTION_PATH),
                ),
                ("native_selection_sha256", json!(selection_digest)),
            ]);
            let mut selected_source_record =
                selected_source.selected_source_json(captured_members.len(), source_bytes);
            let selected_source_object = selected_source_record
                .as_object_mut()
                .ok_or(Refusal("native selected source receipt object"))?;
            selected_source_object.insert(
                "native_projection_source_revision".into(),
                json!(snapshot_output.source_revision()),
            );
            selected_source_object.insert("native_projection_source_cut".into(), json!(source_cut));
            let evidence_scenes_sha256 = source_bindings
                .get(manifest::EVIDENCE_SCENES_PATH)
                .ok_or(Refusal("native evidence-scene source binding absent"))?;
            let evidence_lens_scene = json_object([
                ("path", json!(manifest::EVIDENCE_SCENES_PATH)),
                ("sha256", json!(evidence_scenes_sha256)),
                (
                    "custody",
                    json!("embedded configuration input in this exact executing image"),
                ),
                ("copied_as_runtime_member", json!(false)),
            ]);
            let held_model_fd_first = json_object([
                ("stamp", stamp_json(held_fd_first.stamp)),
                (
                    "producer_elapsed_ns",
                    json!(held_fd_first.producer_elapsed_ns),
                ),
            ]);
            let held_model_fd_last = json_object([
                ("stamp", stamp_json(held_fd_last.stamp)),
                (
                    "producer_elapsed_ns",
                    json!(held_fd_last.producer_elapsed_ns),
                ),
            ]);
            let clock = json_object([
                (
                    "basis",
                    json!("monotonic elapsed nanoseconds relative to producer entry Instant"),
                ),
                (
                    "cold_open_start_elapsed_ns",
                    json!(cold_open_start_elapsed_ns),
                ),
                ("cold_open_end_elapsed_ns", json!(cold_open_end_elapsed_ns)),
            ]);
            let cold_witness = json_object([
                ("schema_version", json!(COLD_SCHEMA)),
                ("actual_cold_open_completed", json!(true)),
                (
                    "native_selection_schema",
                    json!("tos_access_native_knowledge_selection_v3"),
                ),
                ("native_fs_verity_measurement", json!(fs_verity)),
                ("expectation", json!(expectation)),
                ("source_basis", source_basis),
                ("cold_source_revision", json!(cold_source_revision)),
                ("cold_digest_read_bytes", json!(cold_digest_read_bytes)),
                (
                    "cold_validation_charged_bytes",
                    json!(cold_validation_charged_bytes),
                ),
                ("cold_open_vm_steps", json!(open_vm_steps)),
                ("corpus_original_available", json!(true)),
                ("philosophy_original_available", json!(true)),
                ("corpus_original_receipt", corpus_original),
                ("philosophy_original_receipt", philosophy_original),
                ("process_limits", json!(request.process_limits)),
                ("cold_open_limits", json!(request.cold_open)),
                ("model_path", json!(model_path)),
                ("named_model_before", stamp_json(named_model_before)),
                ("held_model_fd_first", held_model_fd_first),
                ("held_model_fd_last", held_model_fd_last),
                ("named_model_after", stamp_json(named_model_after)),
                ("custody_verify_calls", json!(custody_verify_calls)),
                ("clock", clock),
                ("named_and_held_stamps_agree", json!(true)),
            ]);
            let resource_envelope = json_object([
                ("tmpfs_quota_bytes", json!(request.tmpfs_quota_bytes)),
                (
                    "minimum_composed_tmpfs_quota_bytes",
                    json!(manifest::NATIVE_PRODUCER_MIN_TMPFS_QUOTA_BYTES),
                ),
                ("working_ram_bytes", json!(request.working_ram_bytes)),
                (
                    "original_kernel_memory_max_bytes",
                    json!(resources.original_kernel_memory_max()),
                ),
                ("producer_max_state_bytes", json!(request.max_state_bytes)),
                ("producer_max_json_visits", json!(request.max_json_visits)),
                ("producer_max_work_bytes", json!(request.max_work_bytes)),
                (
                    "capture_build_observed_work_bytes",
                    json!(capture.work_bytes()),
                ),
                (
                    "writer_model_max_file_bytes",
                    json!(limits.stage.sqlite.max_output_bytes),
                ),
                (
                    "writer_temp_max_file_bytes",
                    json!(limits.stage.max_temp_bytes),
                ),
                ("payload_layout", json!("CarrierOnceV3")),
                (
                    "persistent_write_cap_bytes",
                    json!(request.persistent_write_cap_bytes),
                ),
                ("candidate_data_cap_bytes", json!(data_cap)),
                (
                    "conservative_data_upper_bound_bytes",
                    json!(persistent_candidate_upper),
                ),
                ("pre_manifest_output_bytes", json!(output.written_bytes)),
                ("whole_deadline_seconds", json!(request.max_build_seconds)),
                ("elapsed_seconds", json!(started.elapsed().as_secs())),
                (
                    "limit_interpretation",
                    json!(
                        "original state/JSON/physical ceilings and refusal bounds; actual completed file size and cold counters are separate evidence, not peak-RAM measurement"
                    ),
                ),
            ]);
            let result = json_object([
                ("schema_version", json!(RESULT_SCHEMA)),
                (
                    "outcome",
                    json!("private-native-data-candidate-ready-for-external-pair-verifier"),
                ),
                ("current_release_promoted", json!(false)),
                ("installed_access_mcp_accepted", json!(false)),
                (
                    "native_model_reused",
                    json!(snapshot_output.reused().is_some()),
                ),
                ("authority", authority),
                ("data_root", json!(output.root_path())),
                ("private_release_root", json!(private_release_root)),
                ("private_release_root_created", json!(false)),
                ("persistent_store", json!(persistent_store_path)),
                ("data_manifest", data_manifest),
                ("selected_source", selected_source_record),
                ("source_bindings", json!(source_bindings)),
                (
                    "embedded_producer_fingerprint",
                    compiler_json(&fingerprint_before),
                ),
                ("evidence_lens_scene", evidence_lens_scene),
                ("external_evidence_refs", json!(evidence_outputs)),
                ("producer_selection", selection_value),
                ("cold_witness", cold_witness),
                ("resource_envelope", resource_envelope),
            ]);
            let raw_result = serde_json::to_vec(&result)?;
            if raw_result.len() > MAX_RESULT_BYTES {
                return Err(Refusal("native Original result byte ceiling").into());
            }
            active(deadline)?;
            Ok(result)
        };
        match finish() {
            Ok(value) => {
                result = Some(value);
                Ok(())
            }
            Err(error) => {
                completion_error = Some(error);
                Err(tos_compiler::Error::Invalid(
                    "native Original consumer refused",
                ))
            }
        }
    };
    let produced = match native_cache {
        Some(cache) => {
            let reused = native_snapshot::reuse_native_snapshot_from_verified_cache(
                &capture,
                cache,
                &expected_source_revision,
                manifest::RUNTIME_DATA_DECLARATION,
                model_abi,
            )?;
            native_snapshot::with_reused_native_snapshot_from_capture_with_owned_budget_and_layout(
                &capture,
                &reused,
                deadline,
                cancelled.as_ref(),
                native_snapshot::NativeSnapshotOwnedBudget {
                    remaining_after_retained: &remaining,
                    original_sqlite_heap: &heap,
                    max_creation_json_visits: remaining_visits,
                    creation_deadline: deadline,
                },
                &mut producer_usage,
                payload_layout,
                consume,
            )
        }
        None => {
            native_snapshot::with_native_original_snapshot_from_capture_with_owned_budget_and_layout(
                &capture,
                &temp_paths.native,
                manifest::RUNTIME_DATA_DECLARATION,
                &isolation,
                limits,
                deadline,
                cancelled.as_ref(),
                native_snapshot::NativeSnapshotOwnedBudget {
                    remaining_after_retained: &remaining,
                    original_sqlite_heap: &heap,
                    max_creation_json_visits: remaining_visits,
                    creation_deadline: deadline,
                },
                &mut producer_usage,
                payload_layout,
                consume,
            )
        }
    };
    if let Some(error) = completion_error {
        return Err(error);
    }
    produced?;
    result.ok_or_else(|| Refusal("native Original completion receipt absent").into())
}

/// Existing `tos-native-owner-command native-original-produce` route. It reads
/// one bounded JSON request and emits one bounded JSON receipt, with no argv
/// path traversal, hidden defaults, installed-pointer write or MCP call.
pub fn run(input: impl Read, output: &mut impl Write, diagnostics: &mut impl Write) -> i32 {
    run_request(REQUEST_SCHEMA, input, output, diagnostics)
}

/// Native source-cut entry. It shares the managed producer/writer but accepts
/// only the source-only request schema; no historical snapshot profile is a
/// required input or fallback.
pub fn run_corpus_build(
    input: impl Read,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    run_request(CORPUS_BUILD_REQUEST_SCHEMA, input, output, diagnostics)
}

/// Read and validate the shared direct source projection request envelope.
/// Query callers can embed the deserializable
/// `DirectRepositoryProjectionRequest` in their own bounded request and call
/// `validate_direct_projection_request` before using the same composer route.
pub(crate) fn read_direct_repository_projection_request(
    input: impl Read,
) -> Result<DirectRepositoryProjectionRequest> {
    let raw = bounded_input(input)?;
    parse_direct_repository_projection_request(&raw)
}

fn parse_direct_repository_projection_request(
    raw: &[u8],
) -> Result<DirectRepositoryProjectionRequest> {
    let request: DirectRepositoryProjectionCheckInput = serde_json::from_slice(&raw)?;
    if request.schema_version != CORPUS_PROJECTION_CHECK_REQUEST_SCHEMA {
        return Err(Refusal("native direct projection request schema differs").into());
    }
    validate_direct_projection_request(&request.projection)?;
    Ok(request.projection)
}

fn read_direct_repository_projection_request_file(
    path: &Path,
) -> Result<DirectRepositoryProjectionRequest> {
    if !path.is_absolute() {
        return Err(Refusal("native projection request file must be absolute").into());
    }
    let uid = rustix::process::getuid().as_raw();
    let (mut file, stamp) = open_regular(path, MAX_REQUEST_BYTES as u64, uid)?;
    let capacity = usize::try_from(stamp.size)
        .map_err(|_| Refusal("native projection request file size range"))?;
    let mut raw = Vec::with_capacity(capacity);
    (&mut file)
        .take(stamp.size.saturating_add(1))
        .read_to_end(&mut raw)?;
    if raw.len() as u64 != stamp.size
        || Stamp::from(&file.metadata()?) != stamp
        || Stamp::from(&fs::symlink_metadata(path)?) != stamp
    {
        return Err(Refusal("native projection request file changed while reading").into());
    }
    parse_direct_repository_projection_request(&raw)
}

fn direct_projection_request_from_args(
    args: &[OsString],
) -> Result<(DirectRepositoryProjectionRequest, Option<&'static str>)> {
    if args.len() == 2 && args[0].to_str() == Some("--request") {
        let path = args[1]
            .to_str()
            .ok_or(Refusal("native projection request path must be UTF-8"))?;
        let path = Path::new(path);
        if !absolute_bounded(
            path.to_str()
                .ok_or(Refusal("native projection request path must be UTF-8"))?,
        ) {
            return Err(Refusal("native projection request path invalid").into());
        }
        return Ok((read_direct_repository_projection_request_file(path)?, None));
    }
    if args.len() != 8 {
        return Err(
            Refusal("native projection argv requires four exact option/value pairs").into(),
        );
    }
    let mut values = BTreeMap::<String, String>::new();
    for pair in args.chunks_exact(2) {
        let name = pair[0]
            .to_str()
            .ok_or(Refusal("native projection argv option must be UTF-8"))?;
        let value = pair[1]
            .to_str()
            .ok_or(Refusal("native projection argv value must be UTF-8"))?;
        if !matches!(
            name,
            "--repo-root" | "--software-commit" | "--schema-worker-env" | "--limits-profile"
        ) || values.insert(name.to_owned(), value.to_owned()).is_some()
        {
            return Err(Refusal("native projection argv options differ or repeat").into());
        }
    }
    if values.len() != 4 {
        return Err(Refusal("native projection argv fields incomplete").into());
    }
    let request = direct_projection_request_from_argv(
        values
            .get("--repo-root")
            .ok_or(Refusal("native projection repository root absent"))?,
        values
            .get("--software-commit")
            .ok_or(Refusal("native projection software commit absent"))?,
        values
            .get("--schema-worker-env")
            .ok_or(Refusal("native projection worker environment absent"))?,
        values
            .get("--limits-profile")
            .ok_or(Refusal("native projection limits profile absent"))?,
    )?;
    Ok((request.0, Some(request.1)))
}

/// Native source-root parity route for the maintained corpus index and
/// bibliographic-claims composers. It performs no persistent write.
pub fn run_corpus_projection_check(
    input: impl Read,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    let result = (|| -> Result<Value> {
        let request = read_direct_repository_projection_request(input)?;
        check_direct_repository_projection(&request)
    })();
    match result {
        Ok(value) => match serde_json::to_vec(&value) {
            Ok(raw) if raw.len() <= MAX_RESULT_BYTES => {
                if output.write_all(&raw).is_err() || output.write_all(b"\n").is_err() {
                    let _ = diagnostics.write_all(b"native direct projection output failed\n");
                    2
                } else {
                    0
                }
            }
            _ => {
                let _ =
                    diagnostics.write_all(b"native direct projection result exceeded output cap\n");
                2
            }
        },
        Err(error) => {
            let _ = writeln!(diagnostics, "native direct projection refused: {error}");
            2
        }
    }
}

pub fn run_corpus_projection_check_args(
    args: &[OsString],
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    if args.len() == 1 && args[0].to_str() == Some("--help") {
        let usage = b"usage: tos-native-owner-command corpus-projection-check --repo-root ABS --software-commit HEAD|HEX40 --schema-worker-env ENV_NAME --limits-profile repo-validation-v1\n       tos-native-owner-command corpus-projection-check --request ABS_JSON\n";
        return if output.write_all(usage).is_ok() {
            0
        } else {
            2
        };
    }
    let result = (|| -> Result<Value> {
        let (request, profile) = direct_projection_request_from_args(args)?;
        let mut value = check_direct_repository_projection(&request)?;
        if let Some(profile) = profile {
            value
                .as_object_mut()
                .ok_or(Refusal("native projection result object invalid"))?
                .insert("limits_profile".to_owned(), json!(profile));
        }
        Ok(value)
    })();
    match result {
        Ok(value) => match serde_json::to_vec(&value) {
            Ok(raw) if raw.len() <= MAX_RESULT_BYTES => {
                if output.write_all(&raw).is_err() || output.write_all(b"\n").is_err() {
                    let _ = diagnostics.write_all(b"native direct projection output failed\n");
                    2
                } else {
                    0
                }
            }
            _ => {
                let _ =
                    diagnostics.write_all(b"native direct projection result exceeded output cap\n");
                2
            }
        },
        Err(error) => {
            let _ = writeln!(diagnostics, "native direct projection refused: {error}");
            2
        }
    }
}

fn run_request(
    expected_schema: &str,
    input: impl Read,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    let result = (|| -> Result<Value> {
        let raw = bounded_input(input)?;
        let request: Request = serde_json::from_slice(&raw)?;
        if request.schema_version != expected_schema {
            return Err(Refusal("native producer request schema differs").into());
        }
        execute(request)
    })();
    match result {
        Ok(value) => match serde_json::to_vec(&value) {
            Ok(raw) if raw.len() <= MAX_RESULT_BYTES => {
                if output.write_all(&raw).is_err() || output.write_all(b"\n").is_err() {
                    let _ = diagnostics.write_all(b"native Original producer output failed\n");
                    2
                } else {
                    0
                }
            }
            _ => {
                let _ =
                    diagnostics.write_all(b"native Original producer result exceeded output cap\n");
                2
            }
        },
        Err(error) => {
            let _ = writeln!(diagnostics, "native Original producer refused: {error}");
            2
        }
    }
}
