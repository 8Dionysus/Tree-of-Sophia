//! Maintained, bounded producer entry for a fresh ManagedLocal native Original
//! pair. This writes only a private data candidate and a compact witness; the
//! installed software owner performs pair verification and promotion later.

use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
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
use tos_source_store::{CorpusCutReader, SoftwareComponentSelectionV1};
use tos_source_store::{
    CorpusReader, CutReadLimits, ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
};
use tos_validation::FormatProfile;
use tos_validation::executor::{
    BatchBudget, BatchStreamBudget, ExactWorkerIdentity, ExecutorBudget,
};
use tos_validation::source_cut::CutWorkerLimits;

const REQUEST_SCHEMA: &str = "tos_native_managed_original_produce_request_v2";
const CORPUS_BUILD_REQUEST_SCHEMA: &str = "tos_native_corpus_build_request_v1";
const RESULT_SCHEMA: &str = "tos_native_managed_original_produce_result_v2";
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

#[derive(Debug)]
struct Refusal(&'static str);
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl StdError for Refusal {}
type Result<T> = std::result::Result<T, Box<dyn StdError>>;

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
    let root_input = tos_compiler::RepositoryRootInput {
        source_cut: &profile.source_revision,
        material: &source_home.raw,
        material_sha256: &source_home_sha,
        identity_id,
    };
    let projection_limits = source_projection_limits(source, request, deadline)?;
    let binding = profile.source_binding();
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
