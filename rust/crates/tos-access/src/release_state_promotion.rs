//! Explicit unpublished native candidate verification, then private durable CAS.
//! Cold/archive owner scans retain their finite existing caps. They do not have
//! inner deadline callbacks; the normal external unit owns the hard wall.
use super as state;
#[path = "release_state_promotion_fs.rs"]
mod fs;
use crate::software_archive::{self, installed};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, OwnedState,
    RelativePath, canonical_bytes_v1, parse_json_with_state_budget,
};
type Result<T> = std::result::Result<T, String>;
const SCHEMA: &str = "tos_access_native_release_promote_request_v1";
const BOOTSTRAP_BYTES: usize = 1_048_576;
// Dispatch holder plan: candidate root + four metadata/archive guards + the
// archive's two FDs + model/SQLite holders (9), private store/lock layout (9),
// and six short-lived opens/temp/readback witnesses. This is a conservative
// owned-dispatch plan, not an observation or a bound on SQLite internals/RSS.
const PUBLICATION_FD_RESERVE: usize = 24;
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    max_metadata_bytes: usize,
    max_state_bytes: usize,
    max_io_bytes: u64,
    max_installed_io_bytes: u64,
    max_installed_state_bytes: usize,
    max_held_fds: usize,
    max_archive_bytes: u64,
    max_archive_expanded_bytes: u64,
    max_archive_members: usize,
    max_image_bytes: u64,
    max_candidate_bytes: u64,
    max_candidate_members: usize,
}
impl Limits {
    fn validate(self) -> Result<()> {
        if self.max_metadata_bytes == 0
            || self.max_metadata_bytes > BOOTSTRAP_BYTES
            || self.max_state_bytes == 0
            || self.max_io_bytes == 0
            || self.max_installed_io_bytes == 0
            || self.max_installed_io_bytes > self.max_io_bytes
            || self.max_installed_state_bytes == 0
            || self.max_installed_state_bytes > self.max_state_bytes
            || self.max_held_fds < 32
            || self.max_archive_bytes == 0
            || self.max_archive_expanded_bytes == 0
            || self.max_archive_members == 0
            || self.max_image_bytes == 0
            || self.max_image_bytes > 512 * 1024 * 1024
            || self.max_candidate_bytes == 0
            || self.max_candidate_members == 0
            || self.max_candidate_members > 4090
        {
            return Err("finite native promotion caller limits invalid".into());
        }
        Ok(())
    }
}
struct Budget {
    deadline: Instant,
    limits: Limits,
    io: u64,
    state: usize,
}
impl Budget {
    fn active(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            Err("original promotion deadline exceeded".into())
        } else {
            Ok(())
        }
    }
    fn io(&mut self, bytes: u64) -> Result<()> {
        self.active()?;
        self.io = self
            .io
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_io_bytes)
            .ok_or("promotion cumulative IO/work allowance exceeded")?;
        Ok(())
    }
    fn state(&mut self, bytes: usize) -> Result<()> {
        self.state = self
            .state
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or("promotion cumulative retained-state allowance exceeded")?;
        Ok(())
    }
}
fn monotonic_ns() -> Result<u64> {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0
        || t.tv_sec < 0
        || t.tv_nsec < 0
    {
        return Err("monotonic clock unavailable".into());
    }
    (t.tv_sec as u64)
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(t.tv_nsec as u64))
        .ok_or("monotonic clock overflow".into())
}
fn deadline(ns: u64) -> Result<Instant> {
    let before = Instant::now();
    let remaining = ns
        .checked_sub(monotonic_ns()?)
        .filter(|n| *n > 0 && *n <= 3_600_000_000_000)
        .ok_or("original deadline expired or exceeds 3600-second owner bound")?;
    before
        .checked_add(Duration::from_nanos(remaining))
        .ok_or("original deadline overflow".into())
}
fn absolute(raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    if raw.len() > 4096
        || path.components().count() > 128
        || !state::safe_reference_absolute_path(raw, path)
    {
        return Err("explicit component-safe absolute path required".into());
    }
    Ok(path.to_owned())
}
fn field<'a>(v: &'a JsonValue, k: &str) -> Result<&'a JsonValue> {
    v.object_get(k)
        .ok_or_else(|| format!("request field absent: {k}"))
}
fn text<'a>(v: &'a JsonValue, k: &str) -> Result<&'a str> {
    state::text(v, k).map_err(|e| e.to_string())
}
fn json(raw: &[u8], cap: usize, available_state: usize) -> Result<JsonValue> {
    let limits = JsonLimits {
        max_bytes: cap,
        ..state::METADATA_LIMITS
    };
    let value =
        parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, available_state)
            .map_err(|e| e.to_string())?
            .into_root();
    let remainder = available_state
        .checked_sub(value.retained_state_bytes().map_err(|e| e.to_string())?)
        .ok_or("metadata parser state exceeds allowance")?;
    if canonical_bytes_v1(
        &value,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: limits.max_bytes.min(remainder),
            ..limits
        },
    )
    .map_err(|e| e.to_string())?
        != raw
    {
        return Err("promotion metadata must be exact canonical JSON".into());
    }
    Ok(value)
}
fn canonical(v: &JsonValue, cap: usize) -> Result<Vec<u8>> {
    canonical_bytes_v1(
        v,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: cap,
            ..state::METADATA_LIMITS
        },
    )
    .map_err(|e| e.to_string())
}
type Stamp = (u64, u64, u64, u32, u32, u64, i64, i64, i64, i64);
fn stamp(file: &File) -> Result<Stamp> {
    let m = file.metadata().map_err(|e| e.to_string())?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.uid(),
        m.nlink(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
fn private_root(file: &File) -> Result<Stamp> {
    let m = file.metadata().map_err(|e| e.to_string())?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o777 != 0o700 {
        return Err("native candidate root must be private owned directory".into());
    }
    stamp(file)
}
fn regular(file: &File) -> Result<Stamp> {
    let m = file.metadata().map_err(|e| e.to_string())?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o022 != 0
        || m.nlink() != 1
    {
        return Err(
            "native candidate must retain owned non-writable-to-others regular files".into(),
        );
    }
    stamp(file)
}
struct Held {
    file: File,
    path: PathBuf,
    stamp: Stamp,
    digest: Digest256,
}
impl Held {
    fn open(path: PathBuf, cap: u64, budget: &mut Budget) -> Result<Self> {
        budget.active()?;
        let file = tos_fd_open::open_absolute_regular(&path, cap).map_err(|e| e.to_string())?;
        let before = regular(&file)?;
        if before.2 > cap {
            return Err("held candidate file exceeds owner byte cap".into());
        }
        let mut held = Self {
            file,
            path,
            stamp: before,
            digest: Digest256::of_bytes(&[]),
        };
        held.digest = hash(&mut held.file, before.2, budget)?;
        held.check(budget)?;
        Ok(held)
    }
    fn check(&self, budget: &Budget) -> Result<()> {
        budget.active()?;
        if regular(&self.file)? != self.stamp
            || regular(
                &tos_fd_open::open_absolute_regular(&self.path, self.stamp.2)
                    .map_err(|e| e.to_string())?,
            )? != self.stamp
        {
            return Err("held native candidate identity changed".into());
        }
        Ok(())
    }
    fn raw(&mut self, cap: usize, budget: &mut Budget) -> Result<Vec<u8>> {
        use std::io::Seek;
        if self.stamp.2 > cap as u64 {
            return Err("candidate metadata byte cap exceeded".into());
        }
        budget.state(self.stamp.2 as usize)?;
        self.file.rewind().map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        raw.try_reserve_exact(self.stamp.2 as usize)
            .map_err(|e| e.to_string())?;
        let mut buffer = [0u8; 8192];
        loop {
            budget.io(8192.min(self.stamp.2.saturating_sub(raw.len() as u64) + 1))?;
            let allowance = buffer.len().min(
                self.stamp
                    .2
                    .saturating_sub(raw.len() as u64)
                    .saturating_add(1) as usize,
            );
            let n = self
                .file
                .read(&mut buffer[..allowance])
                .map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if raw.len().checked_add(n).is_none_or(|n| n > cap) {
                return Err("candidate metadata read exceeds cap".into());
            }
            raw.extend_from_slice(&buffer[..n]);
        }
        self.check(budget)?;
        if raw.len() as u64 != self.stamp.2 || Digest256::of_bytes(&raw) != self.digest {
            return Err("held metadata hash changed".into());
        }
        Ok(raw)
    }
}
fn hash(file: &mut File, size: u64, budget: &mut Budget) -> Result<Digest256> {
    use std::io::Seek;
    file.rewind().map_err(|e| e.to_string())?;
    let mut digest = Digest256Hasher::new();
    let mut used = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let allowed = buffer
            .len()
            .min(size.saturating_sub(used).saturating_add(1) as usize);
        budget.io(allowed as u64)?;
        let n = file
            .read(&mut buffer[..allowed])
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        used = used
            .checked_add(n as u64)
            .filter(|n| *n <= size)
            .ok_or("candidate full-EOF size changed")?;
        digest.update(&buffer[..n]);
    }
    if used != size {
        return Err("candidate full-EOF truncated".into());
    }
    Ok(digest.finalize())
}
struct Candidate {
    root: File,
    path: PathBuf,
    identity: Stamp,
    guards: BTreeMap<String, Stamp>,
    directories: BTreeMap<String, Stamp>,
}
impl Candidate {
    fn open(path: PathBuf) -> Result<Self> {
        let root = tos_fd_open::open_absolute_directory(&path).map_err(|e| e.to_string())?;
        let identity = private_root(&root)?;
        Ok(Self {
            root,
            path,
            identity,
            guards: BTreeMap::new(),
            directories: BTreeMap::new(),
        })
    }
    fn check(&self, budget: &Budget) -> Result<()> {
        budget.active()?;
        if private_root(&self.root)? != self.identity
            || private_root(
                &tos_fd_open::open_absolute_directory(&self.path).map_err(|e| e.to_string())?,
            )? != self.identity
        {
            return Err("held candidate root identity changed".into());
        }
        for (path, expected) in &self.directories {
            budget.active()?;
            if directory_stamp(&candidate_directory(&self.root, path)?)? != *expected {
                return Err("candidate directory census identity changed".into());
            }
        }
        for (path, expected) in &self.guards {
            budget.active()?;
            if regular(&state::child(&self.root, path).map_err(|e| e.to_string())?)? != *expected {
                return Err("declared candidate member identity changed".into());
            }
        }
        Ok(())
    }
    fn census(&mut self, files: &BTreeSet<String>, budget: &mut Budget) -> Result<()> {
        let mut dirs = BTreeSet::new();
        for path in files {
            let mut p = Path::new(path).parent();
            while let Some(parent) = p {
                if parent.as_os_str().is_empty() {
                    break;
                }
                let name = parent.to_str().ok_or("candidate directory UTF8 invalid")?;
                if dirs.insert(name.to_owned()) {
                    budget.state(name.len() + 128)?;
                }
                p = parent.parent();
            }
        }
        let mut actual = BTreeSet::new();
        census_directory(
            &self.root,
            "",
            files,
            &dirs,
            &mut actual,
            &mut self.directories,
            0,
            budget,
        )?;
        if &actual != files || self.directories.len() != dirs.len() {
            return Err(
                "candidate complete filesystem census differs from authenticated manifest".into(),
            );
        }
        self.check(budget)
    }
    fn member(
        &mut self,
        path: &str,
        size: u64,
        digest: Digest256,
        budget: &mut Budget,
    ) -> Result<()> {
        RelativePath::parse(path).map_err(|e| e.to_string())?;
        budget.state(path.len() + std::mem::size_of::<Stamp>() + 128)?;
        let mut file = state::child(&self.root, path).map_err(|e| e.to_string())?;
        let before = regular(&file)?;
        if before.2 != size
            || hash(&mut file, size, budget)? != digest
            || regular(&file)? != before
            || regular(&state::child(&self.root, path).map_err(|e| e.to_string())?)? != before
        {
            return Err("declared native candidate exact bytes differ".into());
        }
        self.guards.insert(path.to_owned(), before);
        Ok(())
    }
}
fn directory_stamp(file: &File) -> Result<Stamp> {
    let m = file.metadata().map_err(|e| e.to_string())?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
        return Err("candidate directory custody/mode invalid".into());
    }
    stamp(file)
}
fn candidate_directory(root: &File, path: &str) -> Result<File> {
    RelativePath::parse(path).map_err(|e| e.to_string())?;
    let mut dir = tos_fd_open::reopen_directory(root).map_err(|e| e.to_string())?;
    for part in path.split('/') {
        dir = tos_fd_open::open_directory_at(&dir, Path::new(part)).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}
struct DirectoryStream(*mut libc::DIR);
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn census_directory(
    root: &File,
    prefix: &str,
    files: &BTreeSet<String>,
    dirs: &BTreeSet<String>,
    actual: &mut BTreeSet<String>,
    guards: &mut BTreeMap<String, Stamp>,
    depth: usize,
    budget: &mut Budget,
) -> Result<()> {
    use std::os::fd::IntoRawFd;
    budget.active()?;
    if depth >= 128
        || depth
            .checked_mul(2)
            .and_then(|n| n.checked_add(4))
            .is_none_or(|n| n > budget.limits.max_held_fds)
    {
        return Err("candidate census depth/descriptor ceiling exceeded".into());
    }
    let held = tos_fd_open::reopen_directory(root).map_err(|e| e.to_string())?;
    let before = directory_stamp(&held)?;
    let raw = held.into_raw_fd();
    let dir = unsafe { libc::fdopendir(raw) };
    if dir.is_null() {
        unsafe {
            libc::close(raw);
        }
        return Err("candidate held directory stream unavailable".into());
    }
    let stream = DirectoryStream(dir);
    let mut count = 0usize;
    loop {
        budget.active()?;
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            if unsafe { *libc::__errno_location() } != 0 {
                return Err("candidate held directory census failed".into());
            }
            break;
        }
        let leaf = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }
            .to_str()
            .map_err(|_| "candidate filename is not UTF8")?;
        if leaf == "." || leaf == ".." {
            continue;
        }
        count = count
            .checked_add(1)
            .filter(|n| *n <= files.len().saturating_add(dirs.len()))
            .ok_or("candidate census entry ceiling exceeded")?;
        let name = if prefix.is_empty() {
            leaf.to_owned()
        } else {
            format!("{prefix}/{leaf}")
        };
        if name.len() > 4096 {
            return Err("candidate census path-byte ceiling exceeded".into());
        }
        RelativePath::parse(&name).map_err(|e| e.to_string())?;
        if dirs.contains(&name) {
            let file =
                tos_fd_open::open_directory_at(root, Path::new(leaf)).map_err(|e| e.to_string())?;
            let stamp = directory_stamp(&file)?;
            budget.state(name.len() + std::mem::size_of::<Stamp>() + 128)?;
            guards.insert(name.clone(), stamp);
            census_directory(&file, &name, files, dirs, actual, guards, depth + 1, budget)?;
            let named =
                tos_fd_open::open_directory_at(root, Path::new(leaf)).map_err(|e| e.to_string())?;
            if directory_stamp(&file)? != stamp || directory_stamp(&named)? != stamp {
                return Err("candidate census directory changed".into());
            }
        } else if files.contains(&name) {
            let file =
                tos_fd_open::open_regular_at(root, Path::new(leaf)).map_err(|e| e.to_string())?;
            regular(&file)?;
            budget.state(name.len() + 128)?;
            if !actual.insert(name) {
                return Err("candidate census duplicates file".into());
            }
        } else {
            return Err("candidate census has undeclared entry".into());
        }
    }
    let fd = unsafe { libc::dirfd(stream.0) };
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicate < 0 {
        return Err("candidate census closing identity unavailable".into());
    }
    let file = unsafe { <File as std::os::fd::FromRawFd>::from_raw_fd(duplicate) };
    if directory_stamp(&file)? != before || directory_stamp(root)? != before {
        return Err("candidate census root changed".into());
    }
    Ok(())
}
// Constructed only after full software/archive/source/cold verification.
// This private typed loan cannot be supplied by a request or public callback.
struct SealedInputs<'a> {
    candidate: &'a Candidate,
    manifest: &'a Held,
    selection: &'a Held,
    archive: &'a Held,
    sidecar: &'a Held,
    model: &'a tos_compiler::VerifiedKnowledgeModel<'static>,
    access: &'a installed::VerifiedInstalledAccess,
    owner: &'a installed::VerifiedInstalledAccess,
    installed_budget: &'a mut installed::InstalledAccessBudget,
}
impl SealedInputs<'_> {
    fn verify(&mut self, budget: &Budget) -> Result<()> {
        budget.active()?;
        self.model.check_pin().map_err(|e| e.to_string())?;
        self.candidate.check(budget)?;
        self.manifest.check(budget)?;
        self.selection.check(budget)?;
        self.archive.check(budget)?;
        self.sidecar.check(budget)?;
        let deadline = budget.deadline;
        let mut cancel = || {
            if Instant::now() >= deadline {
                Err("original promotion deadline exceeded".into())
            } else {
                Ok(())
            }
        };
        self.access
            .verify_current_identity(deadline, &mut cancel, self.installed_budget)?;
        self.owner
            .verify_current_identity(deadline, &mut cancel, self.installed_budget)?;
        Ok(())
    }
}
struct PublicationCharges<'a> {
    budget: &'a mut Budget,
    inputs: SealedInputs<'a>,
}
impl fs::Charges for PublicationCharges<'_> {
    fn io(&mut self, n: usize) -> Result<()> {
        self.budget.io(n as u64)
    }
    fn state(&mut self, n: usize) -> Result<()> {
        self.budget.state(n)
    }
    fn available_state(&self) -> usize {
        self.budget
            .limits
            .max_state_bytes
            .saturating_sub(self.budget.state)
    }
    fn verify_inputs(&mut self) -> Result<()> {
        self.inputs.verify(self.budget)
    }
}
fn bootstrap(path: PathBuf, expected: Digest256, deadline: Instant) -> Result<Vec<u8>> {
    let mut budget = Budget {
        deadline,
        limits: Limits {
            max_metadata_bytes: BOOTSTRAP_BYTES,
            max_state_bytes: BOOTSTRAP_BYTES,
            max_io_bytes: (BOOTSTRAP_BYTES * 2 + 2) as u64,
            max_installed_io_bytes: 1,
            max_installed_state_bytes: 1,
            max_held_fds: 24,
            max_archive_bytes: 1,
            max_archive_expanded_bytes: 1,
            max_archive_members: 1,
            max_image_bytes: 1,
            max_candidate_bytes: 1,
            max_candidate_members: 1,
        },
        io: 0,
        state: 0,
    };
    let mut held = Held::open(path, BOOTSTRAP_BYTES as u64, &mut budget)?;
    if held.digest != expected {
        return Err("independently expected request SHA differs".into());
    }
    held.raw(BOOTSTRAP_BYTES, &mut budget)
}
fn execute(raw: &[u8], original_ns: u64, deadline: Instant) -> Result<fs::Publication> {
    let request = json(raw, BOOTSTRAP_BYTES, BOOTSTRAP_BYTES * 64)?;
    state::keys(
        &request,
        &[
            "schema_version",
            "work_deadline_ns",
            "release_root",
            "software_prefix",
            "candidate_pair",
            "bindings",
            "expected_current",
            "candidate_manifest_sha256",
            "candidate_selection_sha256",
            "limits",
        ],
    )
    .map_err(|e| e.to_string())?;
    if text(&request, "schema_version")? != SCHEMA
        || field(&request, "work_deadline_ns")?.as_u64() != Some(original_ns)
    {
        return Err("request schema/original monotonic deadline differs".into());
    }
    let limits: Limits =
        serde_json::from_slice(&canonical(field(&request, "limits")?, BOOTSTRAP_BYTES)?)
            .map_err(|e| e.to_string())?;
    limits.validate()?;
    if raw.len() > limits.max_metadata_bytes {
        return Err("request metadata exceeds declared caller cap".into());
    }
    let mut budget = Budget {
        deadline,
        limits,
        io: 0,
        state: 0,
    };
    budget.io((raw.len() as u64)
        .checked_mul(2)
        .and_then(|n| n.checked_add(2))
        .ok_or("request IO overflow")?)?;
    budget.state(
        raw.len()
            .checked_add(request.retained_state_bytes().map_err(|e| e.to_string())?)
            .ok_or("request state overflow")?,
    )?;
    let pair = field(&request, "candidate_pair")?;
    let bindings = field(&request, "bindings")?;
    let pair_raw = canonical(pair, limits.max_metadata_bytes)?;
    let bindings_raw = canonical(bindings, limits.max_metadata_bytes)?;
    state::validate_release_pair(pair, &pair_raw).map_err(|e| e.to_string())?;
    state::validate_release_bindings(bindings).map_err(|e| e.to_string())?;
    budget.state(pair_raw.len() + bindings_raw.len())?;
    let expected_current = match field(&request, "expected_current")? {
        JsonValue::Null => None,
        JsonValue::String(_) => Some(
            state::digest(&request, "expected_current")
                .map_err(|e| e.to_string())?
                .to_hex(),
        ),
        _ => return Err("expected_current must be digest/null".into()),
    };
    let release_root = absolute(text(&request, "release_root")?)?;
    let prefix = absolute(text(&request, "software_prefix")?)?;
    let data_root = absolute(text(bindings, "data_root")?)?;
    let archive_path = absolute(text(bindings, "software_archive")?)?;
    if release_root.starts_with(&data_root)
        || data_root.starts_with(&release_root)
        || release_root.starts_with(&prefix)
        || prefix.starts_with(&release_root)
        || archive_path.starts_with(&release_root)
    {
        return Err("release publication and candidate/software custody scopes overlap".into());
    }
    let expected_manifest =
        state::digest(&request, "candidate_manifest_sha256").map_err(|e| e.to_string())?;
    if expected_manifest
        != state::digest(pair, "data_manifest_sha256").map_err(|e| e.to_string())?
    {
        return Err("independent manifest expectation differs from pair".into());
    }
    let mut candidate = Candidate::open(data_root.clone())?;
    let mut manifest = Held::open(
        data_root.join("data/manifest.json"),
        limits.max_metadata_bytes as u64,
        &mut budget,
    )?;
    if manifest.digest != expected_manifest {
        return Err("candidate manifest independent SHA differs".into());
    }
    let manifest_raw = manifest.raw(limits.max_metadata_bytes, &mut budget)?;
    let manifest_value = json(
        &manifest_raw,
        limits.max_metadata_bytes,
        limits.max_state_bytes.saturating_sub(budget.state),
    )?;
    budget.state(
        manifest_raw
            .len()
            .checked_mul(4)
            .ok_or("typed manifest state plan overflow")?,
    )?;
    let decoded = state::decode_native_manifest(&manifest_value, &manifest_raw, pair)
        .map_err(|e| e.to_string())?;
    budget.state(
        manifest_value
            .retained_state_bytes()
            .map_err(|e| e.to_string())?,
    )?;
    budget.state(decoded.members.iter().try_fold(0usize, |n, (path, _)| {
        n.checked_add(path.len() + 128)
            .ok_or("candidate member-state overflow")
    })?)?;
    for map in [&decoded.source_bindings, &decoded.compiler_bindings] {
        budget.state(map.iter().try_fold(0usize, |n, (path, _)| {
            n.checked_add(path.len() + 128)
                .ok_or("candidate binding-state overflow")
        })?)?;
    }
    if decoded.members.is_empty() || decoded.members.len() > limits.max_candidate_members {
        return Err("candidate manifest member ceiling exceeded".into());
    }
    let mut total = 0u64;
    for (path, member) in &decoded.members {
        total = total
            .checked_add(member.size)
            .filter(|n| *n <= limits.max_candidate_bytes)
            .ok_or("candidate declared closure byte ceiling exceeded")?;
        candidate.member(path, member.size, member.digest, &mut budget)?;
    }
    if total
        .checked_add(manifest.stamp.2)
        .is_none_or(|n| n > limits.max_candidate_bytes)
    {
        return Err("candidate manifest plus member bytes exceeds caller cap".into());
    }
    candidate.member(
        "data/manifest.json",
        manifest.stamp.2,
        manifest.digest,
        &mut budget,
    )?;
    let mut exact_files = decoded.members.keys().cloned().collect::<BTreeSet<_>>();
    exact_files.insert("data/manifest.json".into());
    budget.state(exact_files.iter().try_fold(0usize, |n, p| {
        n.checked_add(p.len() + 128)
            .ok_or("candidate census expected-state overflow")
    })?)?;
    candidate.census(&exact_files, &mut budget)?;
    candidate.check(&budget)?;
    let mut selection_file = Held::open(
        data_root.join(&decoded.selection_path),
        limits.max_metadata_bytes as u64,
        &mut budget,
    )?;
    if selection_file.digest
        != state::digest(&request, "candidate_selection_sha256").map_err(|e| e.to_string())?
    {
        return Err("candidate selection independent SHA differs".into());
    }
    let selection_raw = selection_file.raw(limits.max_metadata_bytes, &mut budget)?;
    let selection_value = json(
        &selection_raw,
        limits.max_metadata_bytes,
        limits.max_state_bytes.saturating_sub(budget.state),
    )?;
    budget.state(
        selection_value
            .retained_state_bytes()
            .map_err(|e| e.to_string())?,
    )?;
    let paths = field(&selection_value, "paths")?;
    let mut raw_path = |key: &str| -> Result<Vec<u8>> {
        let relative = text(paths, key)?;
        RelativePath::parse(relative).map_err(|e| e.to_string())?;
        let row = decoded
            .members
            .get(relative)
            .ok_or("selection references undeclared candidate member")?;
        let mut held = Held::open(
            data_root.join(relative),
            limits.max_metadata_bytes as u64,
            &mut budget,
        )?;
        if held.stamp.2 != row.size || held.digest != row.digest {
            return Err("selection vocabulary member differs".into());
        }
        held.raw(limits.max_metadata_bytes, &mut budget)
    };
    let descriptor = raw_path("descriptor")?;
    let entity = raw_path("entity_registry")?;
    let relation = raw_path("relation_registry")?;
    // Legacy typed serde/vocabulary owners lack shared state callbacks. The
    // eight-fold source-byte plan reserves packet, receipt/expectation, registry
    // and vocabulary clones/maps. It is an implementation forecast, not an
    // allocator measurement or proof; the external admitted process owns peak.
    budget.state(
        selection_raw
            .len()
            .checked_mul(8)
            .ok_or("typed selection state plan overflow")?,
    )?;
    budget.state(
        descriptor
            .len()
            .checked_add(entity.len())
            .and_then(|n| n.checked_add(relation.len()))
            .and_then(|n| n.checked_mul(8))
            .ok_or("typed vocabulary state plan overflow")?,
    )?;
    let selection = tos_compiler::NativeKnowledgeSelection::decode(
        &selection_raw,
        &descriptor,
        &entity,
        &relation,
        tos_compiler::NATIVE_KNOWLEDGE_ADAPTER_PROFILES,
        limits.max_metadata_bytes,
    )
    .map_err(|e| e.to_string())?;
    if selection.producer().managed_source.is_some()
        || selection.producer().managed_source_v2.is_some()
        || text(pair, "query_schema")? != selection.expectation().model_abi
        || text(pair, "compiler_version")? != tos_compiler::COMPILER_VERSION
    {
        return Err("native candidate selection compiler/ABI/source profile mismatch".into());
    }
    let model_row = decoded
        .members
        .get(&selection.paths().model)
        .ok_or("model not in candidate manifest")?;
    if model_row.size != selection.expectation().model_size_bytes
        || model_row.digest.to_hex() != selection.expectation().model_sha256
    {
        return Err("native selected model independent binding differs".into());
    }
    let mut archive = Held::open(archive_path.clone(), limits.max_archive_bytes, &mut budget)?;
    if archive.digest != state::digest(pair, "software_sha256").map_err(|e| e.to_string())? {
        return Err("candidate archive does not match release pair".into());
    }
    let sidecar_path = PathBuf::from(format!(
        "{}.manifest.json",
        archive_path.to_str().ok_or("archive UTF8 path invalid")?
    ));
    let sidecar = Held::open(sidecar_path, limits.max_metadata_bytes as u64, &mut budget)?;
    // Conservative non-refundable plan for the maintained archive verifier's
    // hash/member scans: four archive-length passes allow initial/final owner
    // hashes plus ZIP range/metadata reads; expanded members have a separate
    // exact owner cap. Three metadata slices allow retained member map and the
    // owner's bounded locator/decoder workspace. These are conservative bills,
    // not measured disk IO/allocator bounds. The external process owns peaks.
    // Its finite owner API has no inner deadline callback.
    budget.io(archive
        .stamp
        .2
        .checked_mul(4)
        .and_then(|n| n.checked_add(limits.max_archive_expanded_bytes))
        .ok_or("archive owner work plan overflow")?)?;
    budget.state(
        limits
            .max_metadata_bytes
            .checked_mul(3)
            .ok_or("archive metadata-state plan overflow")?,
    )?;
    let verified_archive = software_archive::VerifiedArchive::open(
        &archive_path,
        software_archive::ArchiveLimits {
            max_archive_bytes: limits.max_archive_bytes,
            max_total_bytes: limits.max_archive_expanded_bytes,
            max_members: limits.max_archive_members,
            max_metadata_bytes: limits.max_metadata_bytes,
        },
    )?;
    budget.active()?;
    archive.check(&budget)?;
    sidecar.check(&budget)?;
    if hash(&mut archive.file, archive.stamp.2, &mut budget)? != archive.digest {
        return Err("retained archive bytes changed after owner scan".into());
    }
    let mut sidecar = sidecar;
    if hash(&mut sidecar.file, sidecar.stamp.2, &mut budget)? != sidecar.digest {
        return Err("retained sidecar bytes changed after owner scan".into());
    }
    let cold = selection.cold_limits();
    let cold_plan = cold
        .max_work_bytes
        .checked_add(cold.max_file_bytes)
        .ok_or("cold owner work plan overflow")?;
    budget.io(cold_plan)?;
    let cold_state = cold
        .sqlite_cache_kib
        .checked_mul(1024)
        .and_then(|n| n.checked_add((cold.max_metadata_bytes as u64).checked_mul(3)?))
        .and_then(|n| n.checked_add(cold.max_file_bytes))
        .and_then(|n| n.checked_add((cold.max_sources as u64).checked_mul(8192)?))
        .and_then(|n| n.checked_add(cold.max_row_bytes as u64))
        .ok_or("cold retained-state plan overflow")?;
    budget.state(usize::try_from(cold_state).map_err(|_| "cold state plan overflow")?)?;
    budget.io(limits.max_installed_io_bytes)?;
    budget.state(limits.max_installed_state_bytes)?;
    let mut installed_budget =
        installed::InstalledAccessBudget::new(installed::InstalledAccessLimits {
            max_image_bytes: limits.max_image_bytes,
            max_metadata_bytes: limits.max_metadata_bytes,
            max_state_bytes: limits.max_installed_state_bytes,
            max_io_bytes: limits.max_installed_io_bytes,
            // Reserve root/archive/selection/manifest/cold holder and transient FDs.
            max_held_fds: limits.max_held_fds - PUBLICATION_FD_RESERVE,
        })?;
    let mut cancel = || {
        if Instant::now() >= deadline {
            Err("original promotion deadline exceeded".into())
        } else {
            Ok(())
        }
    };
    let mut access =
        installed::verify_installed_access(&prefix, deadline, &mut cancel, &mut installed_budget)?;
    access.verify_running_image(deadline, &mut cancel, &mut installed_budget)?;
    let mut owner = installed::verify_installed_role(
        &prefix,
        installed::SelectedRole::Command("tos-native-owner-command"),
        deadline,
        &mut cancel,
        &mut installed_budget,
    )?;
    if access.manifest() != verified_archive.manifest()
        || owner.manifest() != verified_archive.manifest()
        || access.software_ref() != owner.software_ref()
    {
        return Err(
            "installed Access/Owner and verified archive are different software closures".into(),
        );
    }
    // Public persisted selected-cold owner retains fs-verity and finite VM/work/
    // row/process caps; the original deadline is checked immediately around it.
    budget.active()?;
    let custody: Arc<dyn tos_compiler::ImmutableKnowledgeCustody> = Arc::new(
        tos_compiler::LinuxFsVerityCustody::new(
            selection.fs_verity().clone(),
            selection.process_limits(),
        )
        .map_err(|e| e.to_string())?,
    );
    let model = tos_compiler::open_selected_knowledge_model_owned(
        &data_root.join(&selection.paths().model),
        selection.expectation().clone(),
        custody,
        cold,
    )
    .map_err(|e| e.to_string())?;
    budget.active()?;
    let model_state = model
        .retained_state_upper_bound()
        .map_err(|e| e.to_string())?;
    if model_state > cold_state as usize {
        budget.state(model_state - cold_state as usize)?;
    }
    let corpus = model.corpus_original_receipt().map_err(|e| e.to_string())?;
    state::corpus_source_plan(
        state::CorpusMetadata {
            members: &decoded.members,
            source_bindings: &decoded.source_bindings,
            compiler_bindings: &decoded.compiler_bindings,
            corpus_revision: text(pair, "corpus_revision")?,
        },
        corpus,
        usize::try_from(cold.max_work_bytes).map_err(|_| "cold source work overflow")?,
    )
    .map_err(|e| e.to_string())?;
    tos_query::bind_verified_knowledge(&model, selection.vocabulary(), &descriptor)
        .map_err(|e| e.to_string())?;
    model.check_pin().map_err(|e| e.to_string())?;
    candidate.check(&budget)?;
    manifest.check(&budget)?;
    selection_file.check(&budget)?;
    archive.check(&budget)?;
    sidecar.check(&budget)?;
    access.verify_current(deadline, &mut cancel, &mut installed_budget)?;
    owner.verify_current(deadline, &mut cancel, &mut installed_budget)?;
    // Installed owns a non-refundable globally reserved slice through all
    // post-CAS rechecks; publication cannot consume that same allowance.
    // The private function is reachable only after candidate/source/cold/software
    // verification. No release reader lock or current-root selection exists here.
    budget.active()?;
    let mut publication = {
        let mut charges = PublicationCharges {
            budget: &mut budget,
            inputs: SealedInputs {
                candidate: &candidate,
                manifest: &manifest,
                selection: &selection_file,
                archive: &archive,
                sidecar: &sidecar,
                model: &model,
                access: &access,
                owner: &owner,
                installed_budget: &mut installed_budget,
            },
        };
        fs::publish_verified_pair(
            &release_root,
            &pair_raw,
            &bindings_raw,
            expected_current.as_deref(),
            deadline,
            limits.max_metadata_bytes,
            &mut charges,
        )?
    };
    let post = (|| {
        model.check_pin().map_err(|e| e.to_string())?;
        candidate.check(&budget)?;
        manifest.check(&budget)?;
        selection_file.check(&budget)?;
        archive.check(&budget)?;
        sidecar.check(&budget)?;
        access.verify_current(deadline, &mut cancel, &mut installed_budget)?;
        owner.verify_current(deadline, &mut cancel, &mut installed_budget)?;
        budget.active()
    })()
    .err();
    if let Some(reason) = post {
        publication.durable = false;
        publication.failure = Some(reason.chars().take(512).collect());
    }
    Ok(publication)
}
/// The only public writer entry accepts an independently authenticated bounded
/// request under its caller's original monotonic deadline. No raw publish API.
pub fn run_if_requested(
    args: &[String],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Option<i32> {
    if args.first().map(String::as_str) != Some("native-release-promote") {
        return None;
    }
    let result = (|| -> Result<fs::Publication> {
        if args.len() != 7
            || args[1] != "--request"
            || args[3] != "--request-sha256"
            || args[5] != "--work-deadline-ns"
        {
            return Err("usage: native-release-promote --request ABS --request-sha256 HEX --work-deadline-ns ORIGINAL_NS".into());
        }
        let ns = args[6]
            .parse::<u64>()
            .map_err(|_| "original monotonic deadline invalid")?;
        let original = deadline(ns)?;
        let expected = Digest256::from_hex(&args[4]).map_err(|e| e.to_string())?;
        let raw = bootstrap(absolute(&args[2])?, expected, original)?;
        execute(&raw, ns, original)
    })();
    match result {
        Ok(p) => {
            let code = if p.durable && p.failure.is_none() {
                0
            } else {
                1
            };
            let receipt = serde_json::json!({"schema_version":"tos_access_native_release_promotion_receipt_v1",
                "pair_id":p.pair_id,"previous":p.previous,"committed":p.committed,"durable":p.durable,"failure":p.failure,
                "verification_profile":"native-selected-fsverity-installed-access-owner-v1",
                "owner_scan_deadline":"finite-owner-caps-with-external-normal-unit-hardwall",
                "resource_accounting":"structural-counters-and-conservative-owner-scan-plans; external-peak-caps"});
            if writeln!(stdout, "{receipt}").is_err() {
                return Some(1);
            }
            Some(code)
        }
        Err(reason) => {
            let _ = writeln!(
                stderr,
                "native release promotion: {}",
                reason.chars().take(512).collect::<String>()
            );
            let _ = writeln!(
                stdout,
                "{}",
                serde_json::json!({"schema_version":"tos_access_native_release_promotion_receipt_v1",
                "committed":false,"durable":false,"failure":reason.chars().take(512).collect::<String>()})
            );
            Some(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "tos-promotion-census-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(path.join("data"))
                .unwrap();
            Self(path)
        }
        fn write(&self, path: &str, raw: &[u8]) {
            std::fs::write(self.0.join(path), raw).unwrap();
            std::fs::set_permissions(self.0.join(path), std::fs::Permissions::from_mode(0o600))
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn budget() -> Budget {
        Budget {
            deadline: Instant::now() + Duration::from_secs(30),
            limits: Limits {
                max_metadata_bytes: 1024,
                max_state_bytes: 1048576,
                max_io_bytes: 1048576,
                max_installed_io_bytes: 1024,
                max_installed_state_bytes: 1024,
                max_held_fds: 32,
                max_archive_bytes: 1024,
                max_archive_expanded_bytes: 1024,
                max_archive_members: 4,
                max_image_bytes: 1024,
                max_candidate_bytes: 1024,
                max_candidate_members: 4,
            },
            io: 0,
            state: 0,
        }
    }
    #[test]
    fn complete_candidate_census_refuses_undeclared_names_and_symlinks() {
        let f = Fixture::new();
        f.write("data/a.json", b"authored");
        let mut candidate = Candidate::open(f.0.clone()).unwrap();
        let mut b = budget();
        candidate
            .member("data/a.json", 8, Digest256::of_bytes(b"authored"), &mut b)
            .unwrap();
        let expected = BTreeSet::from(["data/a.json".to_owned()]);
        candidate.census(&expected, &mut b).unwrap();
        candidate.check(&b).unwrap();
        f.write("data/extra.json", b"unowned");
        assert!(candidate.check(&b).is_err());
        let mut c = Candidate::open(f.0.clone()).unwrap();
        assert!(c.census(&expected, &mut budget()).is_err());
        std::fs::remove_file(f.0.join("data/extra.json")).unwrap();
        std::fs::remove_file(f.0.join("data/a.json")).unwrap();
        std::os::unix::fs::symlink("elsewhere", f.0.join("data/a.json")).unwrap();
        let mut c = Candidate::open(f.0.clone()).unwrap();
        assert!(c.census(&expected, &mut budget()).is_err());
    }
    #[test]
    fn original_clock_and_reserved_owner_slices_do_not_renew() {
        assert!(deadline(monotonic_ns().unwrap().saturating_sub(1)).is_err());
        assert!(deadline(monotonic_ns().unwrap().saturating_add(3_601_000_000_000)).is_err());
        let mut b = budget();
        b.limits.max_state_bytes = 2048;
        b.state(b.limits.max_installed_state_bytes).unwrap();
        assert!(b.state(1025).is_err());
        b.deadline = Instant::now();
        assert!(b.io(1).is_err());
    }
}
