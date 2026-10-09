//! Private durable publication after the parent has verified a native pair.
//! This module owns filesystem custody/CAS only, never candidate admission.
use super::super as state;
use std::{
    ffi::{CString, OsStr},
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, OwnedState, canonical_bytes_v1,
    parse_json_with_state_budget,
};

pub(super) trait Charges {
    fn io(&mut self, bytes: usize) -> std::result::Result<(), String>;
    fn state(&mut self, bytes: usize) -> std::result::Result<(), String>;
    fn available_state(&self) -> usize;
    fn verify_inputs(&mut self) -> Result<()>;
}
type Result<T> = std::result::Result<T, String>;
#[derive(Debug)]
pub(super) struct Publication {
    pub(super) pair_id: String,
    pub(super) previous: Option<String>,
    pub(super) committed: bool,
    pub(super) durable: bool,
    pub(super) failure: Option<String>,
}
fn active(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err("release publication original deadline exceeded".into())
    } else {
        Ok(())
    }
}
fn failure(label: &str) -> String {
    format!("{label}: {}", std::io::Error::last_os_error())
}
fn name(leaf: &OsStr) -> Result<CString> {
    CString::new(leaf.as_bytes()).map_err(|_| "release leaf contains NUL".into())
}
fn inode(file: &File) -> Result<(u64, u64)> {
    let m = file.metadata().map_err(|_| "release inode unavailable")?;
    Ok((m.dev(), m.ino()))
}
fn private_dir(file: &File, parent: bool) -> Result<()> {
    let m = file
        .metadata()
        .map_err(|_| "release directory metadata unavailable")?;
    if !m.is_dir()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o022 != 0
        || (parent && m.mode() & 0o777 != 0o700)
    {
        return Err("release directory owner/private mode invalid".into());
    }
    Ok(())
}
fn regular(file: &File) -> Result<()> {
    let m = file
        .metadata()
        .map_err(|_| "release regular metadata unavailable")?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o777 != 0o600
        || m.nlink() != 1
    {
        return Err("release metadata owner/mode/link count invalid".into());
    }
    Ok(())
}
fn exists(dir: &File, leaf: &OsStr) -> Result<bool> {
    let leaf = name(leaf)?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let result = unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            leaf.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result == 0 {
        return Ok(true);
    }
    if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
        Ok(false)
    } else {
        Err(failure("release name metadata unavailable"))
    }
}
fn open_at(dir: &File, leaf: &OsStr, flags: i32) -> Result<File> {
    let leaf = name(leaf)?;
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            leaf.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        Err(failure("release descriptor open failed"))
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
fn sync(file: &File, deadline: Instant) -> Result<()> {
    active(deadline)?;
    file.sync_all().map_err(|_| "release fsync failed")?;
    active(deadline)
}
fn directory(dir: &File, leaf: &OsStr, deadline: Instant) -> Result<File> {
    active(deadline)?;
    if !exists(dir, leaf)? {
        let c = name(leaf)?;
        if unsafe { libc::mkdirat(dir.as_raw_fd(), c.as_ptr(), 0o700) } != 0
            && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
        {
            return Err(failure("release directory creation failed"));
        }
        sync(dir, deadline)?;
    }
    let file = tos_fd_open::open_directory_at(dir, Path::new(leaf))
        .map_err(|_| "release child directory unsafe")?;
    private_dir(&file, false)?;
    Ok(file)
}
fn normalized(path: &Path) -> Result<&str> {
    let raw = path.to_str().ok_or("release absolute path UTF8 invalid")?;
    if raw.len() > 4096 || !state::safe_reference_absolute_path(raw, path) {
        return Err("release normalized absolute path invalid".into());
    }
    if raw.split('/').count() > 129 {
        return Err("release path depth exceeded".into());
    }
    Ok(raw)
}
// Check all existing components without following symlinks. Missing selected
// artifact paths stay missing: this does not discover or create bound artifacts.
fn binding_path(raw: &str, deadline: Instant) -> Result<()> {
    normalized(Path::new(raw))?;
    let mut dir = File::open("/").map_err(|_| "release root anchor unavailable")?;
    let mut parts = raw.split('/').skip(1).peekable();
    while let Some(part) = parts.next() {
        active(deadline)?;
        if part.is_empty() {
            continue;
        }
        if !exists(&dir, OsStr::new(part))? {
            return Ok(());
        }
        let file = open_at(&dir, OsStr::new(part), libc::O_PATH)?;
        let m = file
            .metadata()
            .map_err(|_| "release binding component metadata unavailable")?;
        if m.file_type().is_symlink() || (parts.peek().is_some() && !m.is_dir()) {
            return Err("release binding has unsafe component".into());
        }
        dir = file;
    }
    Ok(())
}
struct Root {
    path: PathBuf,
    parent_path: PathBuf,
    parent: File,
    leaf: std::ffi::OsString,
    file: File,
}
impl Root {
    fn acquire(path: &Path, deadline: Instant) -> Result<Self> {
        Self::open(path, deadline, true)
    }
    fn open_existing(path: &Path, deadline: Instant) -> Result<Self> {
        Self::open(path, deadline, false)
    }
    fn open(path: &Path, deadline: Instant, create: bool) -> Result<Self> {
        active(deadline)?;
        normalized(path)?;
        let parent_path = path
            .parent()
            .ok_or("release root parent absent")?
            .to_path_buf();
        let leaf = path
            .file_name()
            .ok_or("release root cannot be filesystem root")?
            .to_owned();
        let parent = tos_fd_open::open_absolute_directory(&parent_path)
            .map_err(|_| "release private parent unavailable")?;
        private_dir(&parent, true)?;
        let file = if create {
            directory(&parent, &leaf, deadline)?
        } else {
            let file = tos_fd_open::open_directory_at(&parent, Path::new(&leaf))
                .map_err(|_| "release private root unavailable")?;
            private_dir(&file, false)?;
            file
        };
        let root = Self {
            path: path.to_path_buf(),
            parent_path,
            parent,
            leaf,
            file,
        };
        root.check(deadline)?;
        Ok(root)
    }
    fn check(&self, deadline: Instant) -> Result<()> {
        active(deadline)?;
        private_dir(&self.parent, true)?;
        private_dir(&self.file, false)?;
        let named_parent = tos_fd_open::open_absolute_directory(&self.parent_path)
            .map_err(|_| "release parent name changed")?;
        if inode(&named_parent)? != inode(&self.parent)? {
            return Err("release parent custody changed".into());
        }
        let named = tos_fd_open::open_directory_at(&self.parent, Path::new(&self.leaf))
            .map_err(|_| "release root name changed")?;
        if inode(&named)? != inode(&self.file)? {
            return Err("release root custody changed".into());
        }
        // Re-open the complete explicit name too; no alternate root is selected.
        let absolute = tos_fd_open::open_absolute_directory(&self.path)
            .map_err(|_| "release explicit root name changed")?;
        if inode(&absolute)? != inode(&self.file)? {
            return Err("release root absolute custody changed".into());
        }
        Ok(())
    }
}
struct Store {
    root: Root,
    pairs: File,
    bindings: File,
    revocations: File,
    revoked_data: File,
    revoked_corpus: File,
    revoked_software: File,
    lock: File,
}
impl Store {
    fn acquire(root: &Path, deadline: Instant) -> Result<Self> {
        Self::open(root, deadline, true)
    }
    fn open_existing(root: &Path, deadline: Instant) -> Result<Self> {
        Self::open(root, deadline, false)
    }
    fn open(root: &Path, deadline: Instant, create: bool) -> Result<Self> {
        let root = if create {
            Root::acquire(root, deadline)?
        } else {
            Root::open_existing(root, deadline)?
        };
        let lock = open_at(
            &root.file,
            OsStr::new(".release.lock"),
            if create { libc::O_RDWR | libc::O_CREAT } else { libc::O_RDONLY },
        )?;
        regular(&lock)?;
        let lock_mode = if create { libc::LOCK_EX } else { libc::LOCK_SH };
        loop {
            active(deadline)?;
            if unsafe { libc::flock(lock.as_raw_fd(), lock_mode | libc::LOCK_NB) } == 0 {
                break;
            }
            let error = std::io::Error::last_os_error().raw_os_error();
            if !matches!(error, Some(libc::EWOULDBLOCK) | Some(libc::EINTR)) {
                return Err(failure("release lock failed"));
            }
            std::thread::sleep(
                Duration::from_millis(2).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        let open_dir = |parent: &File, leaf: &str| {
            if create {
                directory(parent, OsStr::new(leaf), deadline)
            } else {
                let dir = tos_fd_open::open_directory_at(parent, Path::new(leaf))
                    .map_err(|_| "release managed directory unavailable")?;
                private_dir(&dir, false)?;
                Ok(dir)
            }
        };
        let pairs = open_dir(&root.file, "pairs")?;
        let bindings = open_dir(&root.file, "bindings")?;
        let revocations = open_dir(&root.file, "revocations")?;
        let revoked_data = open_dir(&revocations, "data")?;
        let revoked_corpus = open_dir(&revocations, "corpus")?;
        let revoked_software = open_dir(&revocations, "software")?;
        let store = Self {
            root,
            pairs,
            bindings,
            revocations,
            revoked_data,
            revoked_corpus,
            revoked_software,
            lock,
        };
        store.check(deadline)?;
        Ok(store)
    }
    fn check(&self, deadline: Instant) -> Result<()> {
        self.root.check(deadline)?;
        regular(&self.lock)?;
        let named = open_at(&self.root.file, OsStr::new(".release.lock"), libc::O_RDONLY)?;
        if inode(&named)? != inode(&self.lock)? {
            return Err("release lock custody changed".into());
        }
        for (parent, leaf, held) in [
            (&self.root.file, "pairs", &self.pairs),
            (&self.root.file, "bindings", &self.bindings),
            (&self.root.file, "revocations", &self.revocations),
            (&self.revocations, "data", &self.revoked_data),
            (&self.revocations, "corpus", &self.revoked_corpus),
            (&self.revocations, "software", &self.revoked_software),
        ] {
            active(deadline)?;
            private_dir(held, false)?;
            let named = tos_fd_open::open_directory_at(parent, Path::new(leaf))
                .map_err(|_| "release layout name unsafe")?;
            if inode(&named)? != inode(held)? {
                return Err("release layout custody changed".into());
            }
        }
        Ok(())
    }
    fn available(&self, pair: &JsonValue, deadline: Instant) -> Result<()> {
        for (dir, field) in [
            (&self.revoked_data, "data_revision"),
            (&self.revoked_corpus, "corpus_revision"),
            (&self.revoked_software, "software_sha256"),
        ] {
            active(deadline)?;
            let digest = state::digest(pair, field)
                .map_err(|error| error.to_string())?
                .to_hex();
            if exists(dir, OsStr::new(&format!("{digest}.json")))? {
                return Err("selected release revoked or revocation state unsafe".into());
            }
        }
        Ok(())
    }
}
#[derive(Eq, PartialEq)]
struct Stamp {
    dev: u64,
    ino: u64,
    size: u64,
    mode: u32,
    uid: u32,
    nlink: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}
fn stamp(file: &File) -> Result<Stamp> {
    let m = file
        .metadata()
        .map_err(|_| "release metadata stamp unavailable")?;
    Ok(Stamp {
        dev: m.dev(),
        ino: m.ino(),
        size: m.len(),
        mode: m.mode(),
        uid: m.uid(),
        nlink: m.nlink(),
        mtime: m.mtime(),
        mtime_nsec: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_nsec: m.ctime_nsec(),
    })
}
fn read_optional(
    dir: &File,
    leaf: &str,
    cap: usize,
    deadline: Instant,
    charges: &mut dyn Charges,
) -> Result<Option<Vec<u8>>> {
    active(deadline)?;
    if !exists(dir, OsStr::new(leaf))? {
        return Ok(None);
    }
    let mut file = tos_fd_open::open_regular_at(dir, Path::new(leaf))
        .map_err(|_| "release metadata member unsafe")?;
    regular(&file)?;
    let before = stamp(&file)?;
    if before.size > cap as u64 {
        return Err("release metadata byte budget exceeded".into());
    }
    charges.state(before.size as usize)?;
    let mut raw = Vec::with_capacity(before.size as usize);
    let mut block = [0u8; 8192];
    loop {
        active(deadline)?;
        let allowance = block.len().min(
            before
                .size
                .saturating_sub(raw.len() as u64)
                .saturating_add(1) as usize,
        );
        charges.io(allowance)?;
        let count = file
            .read(&mut block[..allowance])
            .map_err(|_| "release metadata read failed")?;
        if count == 0 {
            break;
        }
        if raw.len().checked_add(count).is_none_or(|n| n > cap) {
            return Err("release metadata byte budget exceeded".into());
        }
        raw.extend_from_slice(&block[..count]);
    }
    let named = tos_fd_open::open_regular_at(dir, Path::new(leaf))
        .map_err(|_| "release metadata name changed")?;
    if before != stamp(&file)? || before != stamp(&named)? || raw.len() as u64 != before.size {
        return Err("release metadata custody changed during read".into());
    }
    active(deadline)?;
    Ok(Some(raw))
}
fn canonical(raw: &[u8], limits: JsonLimits, charges: &mut dyn Charges) -> Result<JsonValue> {
    let value = parse_json_with_state_budget(
        raw,
        JsonMode::PublishedStrict,
        limits,
        charges.available_state(),
    )
    .map_err(|_| "release metadata JSON invalid")?
    .into_root();
    charges.state(value.retained_state_bytes().map_err(|e| e.to_string())?)?;
    let canonical = canonical_bytes_v1(
        &value,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: limits.max_bytes.min(charges.available_state()),
            ..limits
        },
    )
    .map_err(|_| "release metadata canonicalization failed")?;
    charges.state(canonical.len())?;
    if canonical != raw {
        return Err("release metadata is not canonical".into());
    }
    Ok(value)
}
fn bindings(
    raw: &[u8],
    limits: JsonLimits,
    deadline: Instant,
    charges: &mut dyn Charges,
) -> Result<JsonValue> {
    let value = canonical(raw, limits, charges)?;
    state::validate_release_bindings(&value).map_err(|error| error.to_string())?;
    for field in ["data_root", "software_archive"] {
        binding_path(
            state::text(&value, field).map_err(|error| error.to_string())?,
            deadline,
        )?;
    }
    Ok(value)
}
fn pointer(
    store: &Store,
    limits: JsonLimits,
    deadline: Instant,
    charges: &mut dyn Charges,
) -> Result<Option<(String, Option<String>)>> {
    read_optional(
        &store.root.file,
        "current.json",
        limits.max_bytes,
        deadline,
        charges,
    )?
    .map(|raw| {
        let value = canonical(&raw, limits, charges)?;
        state::validate_release_pointer(&value).map_err(|error| error.to_string())
    })
    .transpose()
}
fn stored_pair(
    store: &Store,
    id: &str,
    limits: JsonLimits,
    deadline: Instant,
    charges: &mut dyn Charges,
) -> Result<()> {
    let leaf = format!("{id}.json");
    let raw = read_optional(&store.pairs, &leaf, limits.max_bytes, deadline, charges)?
        .ok_or("current immutable pair absent")?;
    let value = canonical(&raw, limits, charges)?;
    if state::validate_release_pair(&value, &raw).map_err(|error| error.to_string())? != id {
        return Err("current pair filename digest differs".into());
    }
    let raw = read_optional(&store.bindings, &leaf, limits.max_bytes, deadline, charges)?
        .ok_or("current immutable bindings absent")?;
    bindings(&raw, limits, deadline, charges)?;
    Ok(())
}
fn stored_records(
    store: &Store,
    id: &str,
    limits: JsonLimits,
    deadline: Instant,
    charges: &mut dyn Charges,
) -> Result<(JsonValue, Vec<u8>, Vec<u8>)> {
    let leaf = format!("{id}.json");
    let pair_raw = read_optional(&store.pairs, &leaf, limits.max_bytes, deadline, charges)?
        .ok_or("current immutable pair absent")?;
    let pair = canonical(&pair_raw, limits, charges)?;
    if state::validate_release_pair(&pair, &pair_raw).map_err(|error| error.to_string())? != id {
        return Err("current pair filename digest differs".into());
    }
    let bindings_raw =
        read_optional(&store.bindings, &leaf, limits.max_bytes, deadline, charges)?
            .ok_or("current immutable bindings absent")?;
    bindings(&bindings_raw, limits, deadline, charges)?;
    Ok((pair, pair_raw, bindings_raw))
}

pub(super) struct PreviousRelease {
    pub(super) current: String,
    pub(super) previous: String,
    pub(super) pair_raw: Vec<u8>,
    pub(super) bindings_raw: Vec<u8>,
}

/// Read exactly the stored previous pair under the private writer lock. The
/// caller releases this lock before the expensive owner verification; the
/// rollback commit later requires the same target still be the pointer's
/// previous pair and rechecks the full pointer CAS.
pub(super) fn previous_release(
    root: &Path,
    expected_current: &str,
    deadline: Instant,
    max_metadata_bytes: usize,
    charges: &mut dyn Charges,
) -> Result<PreviousRelease> {
    active(deadline)?;
    Digest256::from_hex(expected_current).map_err(|_| "expected_current digest invalid")?;
    if max_metadata_bytes == 0 || max_metadata_bytes > state::METADATA_LIMITS.max_bytes {
        return Err("release metadata budget invalid".into());
    }
    let limits = JsonLimits {
        max_bytes: max_metadata_bytes,
        ..state::METADATA_LIMITS
    };
    charges.state(
        std::mem::size_of::<Store>()
            + root.as_os_str().len()
            + root.parent().ok_or("release parent absent")?.as_os_str().len()
            + root.file_name().ok_or("release leaf absent")?.len(),
    )?;
    let store = Store::acquire(root, deadline)?;
    charges.verify_inputs()?;
    let original = pointer(&store, limits, deadline, charges)?
        .ok_or("cannot rollback without a current release pointer")?;
    let current = original.0.as_str();
    if current != expected_current {
        return Err("expected_current does not match current release".into());
    }
    let previous = original
        .1
        .as_deref()
        .ok_or("current release has no previous pair to roll back to")?;
    // As in the Python owner, corrupt current state blocks rollback even though
    // it is the pair being replaced.
    stored_records(&store, current, limits, deadline, charges)?;
    let (_, pair_raw, bindings_raw) =
        stored_records(&store, previous, limits, deadline, charges)?;
    store.check(deadline)?;
    if pointer(&store, limits, deadline, charges)? != Some(original) {
        return Err("release pointer changed while reading rollback target".into());
    }
    Ok(PreviousRelease {
        current: current.to_owned(),
        previous: previous.to_owned(),
        pair_raw,
        bindings_raw,
    })
}
pub(super) struct CurrentRelease {
    pub(super) pair_id: String,
    pub(super) pair_raw: Vec<u8>,
    pub(super) bindings_raw: Vec<u8>,
    pub(super) previous: Option<String>,
}

/// Read status without creating the store or its lock/layout. The shared lock
/// keeps pair, bindings, pointer and revocations from changing during the read.
pub(super) fn current_release(
    root: &Path,
    deadline: Instant,
    max_metadata_bytes: usize,
    charges: &mut dyn Charges,
) -> Result<CurrentRelease> {
    active(deadline)?;
    if max_metadata_bytes == 0 || max_metadata_bytes > state::METADATA_LIMITS.max_bytes {
        return Err("release metadata budget invalid".into());
    }
    let limits = JsonLimits {
        max_bytes: max_metadata_bytes,
        ..state::METADATA_LIMITS
    };
    charges.state(
        std::mem::size_of::<Store>()
            + root.as_os_str().len()
            + root.parent().ok_or("release parent absent")?.as_os_str().len()
            + root.file_name().ok_or("release leaf absent")?.len(),
    )?;
    let store = Store::open_existing(root, deadline)?;
    charges.verify_inputs()?;
    let original = pointer(&store, limits, deadline, charges)?
        .ok_or("no current release pointer exists")?;
    let pair_id = original.0.clone();
    let (pair, pair_raw, bindings_raw) = stored_records(&store, &pair_id, limits, deadline, charges)?;
    store.available(&pair, deadline)?;
    store.check(deadline)?;
    if pointer(&store, limits, deadline, charges)? != Some(original.clone()) {
        return Err("release pointer changed while reading status".into());
    }
    Ok(CurrentRelease {
        pair_id,
        pair_raw,
        bindings_raw,
        previous: original.1,
    })
}

struct Temporary<'a> {
    dir: &'a File,
    leaf: String,
    file: File,
    linked: bool,
}
impl Temporary<'_> {
    fn check(&self) -> Result<()> {
        let named = open_at(self.dir, OsStr::new(&self.leaf), libc::O_RDONLY)?;
        if inode(&named)? != inode(&self.file)? {
            return Err("release temporary custody changed".into());
        }
        Ok(())
    }
    fn remove(&self) -> Result<()> {
        self.check()?;
        let leaf = name(OsStr::new(&self.leaf))?;
        if unsafe { libc::unlinkat(self.dir.as_raw_fd(), leaf.as_ptr(), 0) } != 0 {
            return Err(failure("release temporary cleanup failed"));
        }
        Ok(())
    }
}
impl Drop for Temporary<'_> {
    fn drop(&mut self) {
        if !self.linked {
            let _ = self.remove();
        }
    }
}
fn temporary<'a>(
    dir: &'a File,
    raw: &[u8],
    deadline: Instant,
    charges: &mut dyn Charges,
) -> Result<Temporary<'a>> {
    for _ in 0..8 {
        active(deadline)?;
        let mut nonce = [0u8; 16];
        if unsafe { libc::getrandom(nonce.as_mut_ptr().cast(), nonce.len(), libc::GRND_NONBLOCK) }
            != nonce.len() as isize
        {
            return Err(failure("release temporary entropy unavailable"));
        }
        let leaf = format!(
            ".native-publish-{}",
            nonce.iter().map(|n| format!("{n:02x}")).collect::<String>()
        );
        let c = name(OsStr::new(&leaf))?;
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                c.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EEXIST) {
                continue;
            }
            return Err(failure("release temporary creation failed"));
        }
        let mut temp = Temporary {
            dir,
            leaf,
            file: unsafe { File::from_raw_fd(fd) },
            linked: false,
        };
        regular(&temp.file)?;
        for block in raw.chunks(8192) {
            active(deadline)?;
            charges.io(block.len())?;
            temp.file
                .write_all(block)
                .map_err(|_| "release temporary write failed")?;
        }
        sync(&temp.file, deadline)?;
        temp.check()?;
        return Ok(temp);
    }
    Err("release temporary collision bound exceeded".into())
}
fn immutable(
    dir: &File,
    leaf: &str,
    raw: &[u8],
    limits: JsonLimits,
    deadline: Instant,
    charges: &mut dyn Charges,
) -> Result<()> {
    if let Some(existing) = read_optional(dir, leaf, limits.max_bytes, deadline, charges)? {
        if existing != raw {
            return Err("existing immutable release record differs".into());
        }
        let held = tos_fd_open::open_regular_at(dir, Path::new(leaf))
            .map_err(|_| "identical immutable record unavailable for sync")?;
        regular(&held)?;
        sync(&held, deadline)?;
        return sync(dir, deadline);
    }
    let mut temp = temporary(dir, raw, deadline, charges)?;
    active(deadline)?;
    temp.check()?;
    let src = name(OsStr::new(&temp.leaf))?;
    let dst = name(OsStr::new(leaf))?;
    if unsafe {
        libc::renameat2(
            dir.as_raw_fd(),
            src.as_ptr(),
            dir.as_raw_fd(),
            dst.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        if std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            return Err(failure(
                "immutable atomic no-clobber rename unsupported or failed",
            ));
        }
    } else {
        // Atomic no-clobber name transfer keeps nlink=1 even across SIGKILL.
        // Unsupported filesystems fail closed; there is no hardlink fallback.
        temp.linked = true;
    }
    let existing = read_optional(dir, leaf, limits.max_bytes, deadline, charges)?
        .ok_or("immutable publication name absent")?;
    if existing != raw {
        return Err("immutable release publication collision".into());
    }
    let held = tos_fd_open::open_regular_at(dir, Path::new(leaf))
        .map_err(|_| "immutable published file unavailable for sync")?;
    regular(&held)?;
    let named = tos_fd_open::open_regular_at(dir, Path::new(leaf))
        .map_err(|_| "immutable published name unavailable for sync")?;
    if inode(&held)? != inode(&named)? {
        return Err("immutable published sync identity changed".into());
    }
    sync(&held, deadline)?;
    sync(dir, deadline)
}
fn pointer_raw(
    pair_id: &str,
    previous: Option<&str>,
    limits: JsonLimits,
    charges: &mut dyn Charges,
) -> Result<Vec<u8>> {
    let encoded=serde_json::to_vec(&serde_json::json!({"schema_version":"tos_access_release_pointer_v1","current":pair_id,"previous":previous}))
        .map_err(|_| "release pointer encoding failed")?;
    let value = parse_json_with_state_budget(
        &encoded,
        JsonMode::PublishedStrict,
        limits,
        charges.available_state(),
    )
    .map_err(|_| "release pointer JSON invalid")?
    .into_root();
    state::validate_release_pointer(&value).map_err(|error| error.to_string())?;
    charges.state(
        encoded
            .len()
            .checked_add(value.retained_state_bytes().map_err(|e| e.to_string())?)
            .ok_or("pointer state overflow")?,
    )?;
    let raw = canonical_bytes_v1(
        &value,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: limits.max_bytes.min(charges.available_state()),
            ..limits
        },
    )
    .map_err(|_| "release pointer canonicalization failed")?;
    charges.state(raw.len())?;
    Ok(raw)
}
fn revocation_raw(
    kind: &str,
    digest: &str,
    reason: &str,
    owner_ref: &str,
    limits: JsonLimits,
    charges: &mut dyn Charges,
) -> Result<Vec<u8>> {
    if !matches!(kind, "data" | "corpus" | "software") {
        return Err("release revocation kind must be data, corpus, or software".into());
    }
    let encoded = serde_json::to_vec(&serde_json::json!({
        "schema_version":"tos_access_release_revocation_v1",
        "kind":kind,
        "digest":digest,
        "reason":reason,
        "owner_ref":owner_ref
    }))
    .map_err(|_| "release revocation encoding failed")?;
    let value = parse_json_with_state_budget(
        &encoded,
        JsonMode::PublishedStrict,
        limits,
        charges.available_state(),
    )
    .map_err(|_| "release revocation JSON invalid")?
    .into_root();
    state::validate_release_revocation(&value, kind, digest).map_err(|error| error.to_string())?;
    charges.state(
        encoded
            .len()
            .checked_add(value.retained_state_bytes().map_err(|e| e.to_string())?)
            .ok_or("release revocation state overflow")?,
    )?;
    let raw = canonical_bytes_v1(
        &value,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: limits.max_bytes.min(charges.available_state()),
            ..limits
        },
    )
    .map_err(|_| "release revocation canonicalization failed")?;
    charges.state(raw.len())?;
    Ok(raw)
}

/// Write one permanent revocation record. An identical retry succeeds, while
/// different metadata for an existing digest is a collision and never replaces
/// the earlier owner decision.
pub(super) fn revoke_digest(
    root: &Path,
    kind: &str,
    digest: &str,
    reason: &str,
    owner_ref: &str,
    deadline: Instant,
    max_metadata_bytes: usize,
    charges: &mut dyn Charges,
) -> Result<()> {
    active(deadline)?;
    if max_metadata_bytes == 0 || max_metadata_bytes > state::METADATA_LIMITS.max_bytes {
        return Err("release metadata budget invalid".into());
    }
    let limits = JsonLimits {
        max_bytes: max_metadata_bytes,
        ..state::METADATA_LIMITS
    };
    let raw = revocation_raw(kind, digest, reason, owner_ref, limits, charges)?;
    let store = Store::acquire(root, deadline)?;
    let directory = match kind {
        "data" => &store.revoked_data,
        "corpus" => &store.revoked_corpus,
        "software" => &store.revoked_software,
        _ => return Err("release revocation kind must be data, corpus, or software".into()),
    };
    let leaf = format!("{digest}.json");
    immutable(directory, &leaf, &raw, limits, deadline, charges)?;
    store.check(deadline)?;
    Ok(())
}

/// The parent has already authenticated the genuine native/software/cold pair.
/// An Err precedes pointer commit. Once rename succeeds, failure stays in the
/// structured outcome with committed=true; no rollback or foreign deletion.
pub(super) fn publish_verified_pair(
    root: &Path,
    pair_raw: &[u8],
    bindings_raw: &[u8],
    expected_current: Option<&str>,
    deadline: Instant,
    max_metadata_bytes: usize,
    charges: &mut dyn Charges,
) -> Result<Publication> {
    publish_verified_pair_inner(
        root,
        pair_raw,
        bindings_raw,
        expected_current,
        false,
        deadline,
        max_metadata_bytes,
        charges,
    )
}

/// Roll back only to the exact stored previous pair. The caller supplies the
/// same sealed verification loan as promotion, so the previous candidate is
/// deeply rechecked before the writer's exact-pointer CAS.
pub(super) fn rollback_verified_previous(
    root: &Path,
    pair_raw: &[u8],
    bindings_raw: &[u8],
    expected_current: &str,
    deadline: Instant,
    max_metadata_bytes: usize,
    charges: &mut dyn Charges,
) -> Result<Publication> {
    publish_verified_pair_inner(
        root,
        pair_raw,
        bindings_raw,
        Some(expected_current),
        true,
        deadline,
        max_metadata_bytes,
        charges,
    )
}

fn publish_verified_pair_inner(
    root: &Path,
    pair_raw: &[u8],
    bindings_raw: &[u8],
    expected_current: Option<&str>,
    rollback_to_previous: bool,
    deadline: Instant,
    max_metadata_bytes: usize,
    charges: &mut dyn Charges,
) -> Result<Publication> {
    active(deadline)?;
    if max_metadata_bytes == 0 || max_metadata_bytes > state::METADATA_LIMITS.max_bytes {
        return Err("release metadata budget invalid".into());
    }
    let limits = JsonLimits {
        max_bytes: max_metadata_bytes,
        ..state::METADATA_LIMITS
    };
    let pair = canonical(pair_raw, limits, charges)?;
    let pair_id =
        state::validate_release_pair(&pair, pair_raw).map_err(|error| error.to_string())?;
    bindings(bindings_raw, limits, deadline, charges)?;
    if let Some(expected) = expected_current {
        Digest256::from_hex(expected).map_err(|_| "expected_current digest invalid")?;
    }
    charges.state(
        std::mem::size_of::<Store>()
            + root.as_os_str().len()
            + root
                .parent()
                .ok_or("release parent absent")?
                .as_os_str()
                .len()
            + root.file_name().ok_or("release leaf absent")?.len(),
    )?;
    let store = Store::acquire(root, deadline)?;
    charges.verify_inputs()?;
    let original = pointer(&store, limits, deadline, charges)?;
    let current = original.as_ref().map(|p| p.0.as_str());
    if current != expected_current {
        return Err("expected_current does not match current release".into());
    }
    if rollback_to_previous
        && original
            .as_ref()
            .and_then(|pointer| pointer.1.as_deref())
            != Some(pair_id.as_str())
    {
        return Err("rollback target is no longer the stored previous pair".into());
    }
    if let Some(current) = current {
        stored_pair(&store, current, limits, deadline, charges)?;
    }
    let leaf = format!("{pair_id}.json");
    // Detect both collisions before creating either immutable record.
    for (dir, raw) in [(&store.pairs, pair_raw), (&store.bindings, bindings_raw)] {
        if let Some(existing) = read_optional(dir, &leaf, limits.max_bytes, deadline, charges)? {
            if existing != raw {
                return Err("existing immutable release pair/bindings differs".into());
            }
        }
    }
    store.available(&pair, deadline)?;
    if current == Some(pair_id.as_str()) {
        charges.verify_inputs()?;
        let previous = original.as_ref().and_then(|p| p.1.clone());
        let durability = (|| {
            store.check(deadline)?;
            sync(&store.pairs, deadline)?;
            sync(&store.bindings, deadline)?;
            let current = open_at(&store.root.file, OsStr::new("current.json"), libc::O_RDONLY)?;
            sync(&current, deadline)?;
            sync(&store.root.file, deadline)?;
            store.check(deadline)?;
            store.available(&pair, deadline)?;
            if pointer(&store, limits, deadline, charges)? != original {
                return Err("already-current pointer changed during durability check".into());
            }
            Ok(())
        })();
        return Ok(Publication {
            pair_id,
            previous,
            committed: true,
            durable: durability.is_ok(),
            failure: durability
                .err()
                .map(|message: String| message.chars().take(512).collect()),
        });
    }
    immutable(&store.pairs, &leaf, pair_raw, limits, deadline, charges)?;
    immutable(
        &store.bindings,
        &leaf,
        bindings_raw,
        limits,
        deadline,
        charges,
    )?;
    store.check(deadline)?;
    if pointer(&store, limits, deadline, charges)? != original {
        return Err("expected_current changed before promotion".into());
    }
    store.available(&pair, deadline)?;
    let previous = current.map(str::to_owned);
    let raw = pointer_raw(&pair_id, previous.as_deref(), limits, charges)?;
    let mut temp = temporary(&store.root.file, &raw, deadline, charges)?;
    store.check(deadline)?;
    temp.check()?;
    if pointer(&store, limits, deadline, charges)? != original {
        return Err("expected_current changed before atomic publication".into());
    }
    for (dir, expected) in [(&store.pairs, pair_raw), (&store.bindings, bindings_raw)] {
        if read_optional(dir, &leaf, limits.max_bytes, deadline, charges)?.as_deref()
            != Some(expected)
        {
            return Err("immutable pair changed before pointer commit".into());
        }
    }
    store.available(&pair, deadline)?;
    charges.verify_inputs()?;
    active(deadline)?;
    let source = name(OsStr::new(&temp.leaf))?;
    let destination = name(OsStr::new("current.json"))?;
    if unsafe {
        libc::renameat(
            store.root.file.as_raw_fd(),
            source.as_ptr(),
            store.root.file.as_raw_fd(),
            destination.as_ptr(),
        )
    } != 0
    {
        return Err(failure("release current pointer rename failed"));
    }
    temp.linked = true; // The name moved; Drop must never undo the committed pointer.
    let durability = (|| {
        sync(&store.root.file, deadline)?;
        store.check(deadline)?;
        store.available(&pair, deadline)?;
        let named = open_at(&store.root.file, OsStr::new("current.json"), libc::O_RDONLY)?;
        regular(&named)?;
        if inode(&named)? != inode(&temp.file)? {
            return Err("committed pointer inode differs from held publication".into());
        }
        let actual = read_optional(
            &store.root.file,
            "current.json",
            limits.max_bytes,
            deadline,
            charges,
        )?
        .ok_or("committed pointer absent")?;
        if actual != raw {
            return Err("committed pointer custody/readback differs".into());
        }
        Ok(())
    })();
    Ok(Publication {
        pair_id,
        previous,
        committed: true,
        durable: durability.is_ok(),
        failure: durability
            .err()
            .map(|message: String| message.chars().take(512).collect()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::parse_json;
    struct TestCharges {
        io: usize,
        state: usize,
    }
    impl Charges for TestCharges {
        fn io(&mut self, n: usize) -> Result<()> {
            self.io = self
                .io
                .checked_add(n)
                .filter(|x| *x <= 64 * 1024 * 1024)
                .ok_or("test IO allowance")?;
            Ok(())
        }
        fn state(&mut self, n: usize) -> Result<()> {
            self.state = self
                .state
                .checked_add(n)
                .filter(|x| *x <= 64 * 1024 * 1024)
                .ok_or("test state allowance")?;
            Ok(())
        }
        fn available_state(&self) -> usize {
            64 * 1024 * 1024 - self.state
        }
        fn verify_inputs(&mut self) -> Result<()> {
            Ok(())
        }
    }
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut nonce = [0u8; 16];
            assert_eq!(
                unsafe { libc::getrandom(nonce.as_mut_ptr().cast(), nonce.len(), 0) },
                nonce.len() as isize
            );
            let suffix = nonce.iter().map(|v| format!("{v:02x}")).collect::<String>();
            // The actual test owner sets TMPDIR to its admitted artifact scope.
            let path = std::env::temp_dir().join(format!("native-release-store-{suffix}"));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            Self(path)
        }
        fn root(&self) -> PathBuf {
            self.0.join("release-store")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn encode(value: serde_json::Value) -> Vec<u8> {
        let raw = serde_json::to_vec(&value).unwrap();
        let value = parse_json(&raw, JsonMode::PublishedStrict, state::METADATA_LIMITS)
            .unwrap()
            .into_root();
        canonical_bytes_v1(
            &value,
            CanonicalProfile::CorpusSnapshotV1,
            state::METADATA_LIMITS,
        )
        .unwrap()
    }
    fn pair(label: &str) -> Vec<u8> {
        let data = format!("test data {label}");
        let manifest = format!("test manifest {label}");
        let corpus = format!("test corpus {label}");
        encode(serde_json::json!({
            "schema_version":"tos_access_release_pair_v1",
            "software_sha256":Digest256::of_bytes(label.as_bytes()).to_hex(),
            "data_revision":Digest256::of_bytes(data.as_bytes()).to_hex(),
            "data_manifest_sha256":Digest256::of_bytes(manifest.as_bytes()).to_hex(),
            "corpus_revision":Digest256::of_bytes(corpus.as_bytes()).to_hex(),
            "query_schema":"test query", "compiler_version":"test compiler"
        }))
    }
    fn binding(label: &str) -> Vec<u8> {
        encode(
            serde_json::json!({"data_root":format!("/test-not-present-{label}/data"),
            "software_archive":format!("/test-not-present-{label}/software.tar")}),
        )
    }
    fn publish(
        root: &Path,
        pair: &[u8],
        bindings: &[u8],
        expected: Option<&str>,
    ) -> Result<Publication> {
        publish_verified_pair(
            root,
            pair,
            bindings,
            expected,
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut TestCharges { io: 0, state: 0 },
        )
    }
    fn write_private(path: &Path, bytes: &[u8]) {
        std::fs::write(path, bytes).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn status_reads_the_exact_available_pair_without_creating_store_state() {
        let fixture = Fixture::new();
        let root = fixture.root();
        assert!(
            current_release(
                &root,
                Instant::now() + Duration::from_secs(30),
                state::METADATA_LIMITS.max_bytes,
                &mut TestCharges { io: 0, state: 0 },
            )
            .is_err()
        );
        assert!(!root.exists());

        let first_pair = pair("status-a");
        let first_bindings = binding("status-a");
        let first = publish(&root, &first_pair, &first_bindings, None).unwrap();
        let second = publish(
            &root,
            &pair("status-b"),
            &binding("status-b"),
            Some(&first.pair_id),
        )
        .unwrap();
        let selected = current_release(
            &root,
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        assert_eq!(selected.pair_id, second.pair_id);
        assert_eq!(selected.pair_raw, pair("status-b"));
        assert_eq!(selected.bindings_raw, binding("status-b"));
        assert_eq!(selected.previous.as_deref(), Some(first.pair_id.as_str()));

        let current = canonical(
            &selected.pair_raw,
            state::METADATA_LIMITS,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        let revoked = state::digest(&current, "software_sha256").unwrap().to_hex();
        revoke_digest(
            &root,
            "software",
            &revoked,
            "status refuses withdrawn selection",
            "test:release-status",
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        assert!(
            current_release(
                &root,
                Instant::now() + Duration::from_secs(30),
                state::METADATA_LIMITS.max_bytes,
                &mut TestCharges { io: 0, state: 0 },
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_revocation_metadata_is_refused_before_store_creation() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let uppercase = "A".repeat(64);
        let lowercase = "a".repeat(64);
        for (kind, digest, reason, owner) in [
            ("other", "a", "reason", "owner"),
            ("data", uppercase.as_str(), "reason", "owner"),
            ("data", lowercase.as_str(), "", "owner"),
            ("data", lowercase.as_str(), "bad\nreason", "owner"),
            ("data", lowercase.as_str(), "reason", "\t"),
        ] {
            assert!(
                revoke_digest(
                    &root,
                    kind,
                    digest,
                    reason,
                    owner,
                    Instant::now() + Duration::from_secs(30),
                    state::METADATA_LIMITS.max_bytes,
                    &mut TestCharges { io: 0, state: 0 },
                )
                .is_err()
            );
            assert!(!root.exists());
        }
    }

    #[test]
    fn fresh_and_owned_python_layout_publish_with_exact_cas() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let a = pair("a");
        let bindings = binding("a");
        let first = publish(&root, &a, &bindings, None).unwrap();
        for kind in ["pairs", "bindings"] {
            assert_eq!(
                std::fs::metadata(root.join(kind).join(format!("{}.json", first.pair_id)))
                    .unwrap()
                    .nlink(),
                1
            );
        }
        assert!(first.committed && first.durable && first.failure.is_none());
        assert_eq!(first.previous, None);
        let pointer_before = std::fs::read(root.join("current.json")).unwrap();
        assert!(publish(&root, &pair("b"), &binding("b"), None).is_err());
        assert_eq!(
            std::fs::read(root.join("current.json")).unwrap(),
            pointer_before
        );
        // Python's mkdir defaults may be0755 under a checked0700 privateparent.
        for path in [&root, &root.join("pairs"), &root.join("bindings")] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let repeated = publish(&root, &a, &bindings, Some(&first.pair_id)).unwrap();
        assert!(repeated.committed && repeated.durable && repeated.failure.is_none());
        assert_eq!(
            std::fs::read(root.join("current.json")).unwrap(),
            pointer_before
        );
        let second = publish(&root, &pair("b"), &binding("b"), Some(&first.pair_id)).unwrap();
        assert_eq!(second.previous.as_deref(), Some(first.pair_id.as_str()));
        assert!(second.committed && second.durable);
    }
    #[test]
    fn noncanonical_and_unsafe_bindings_refuse_before_store_creation() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let mut raw = pair("a");
        raw.push(b' ');
        assert!(publish(&root, &raw, &binding("a"), None).is_err());
        assert!(!root.exists());
        let unsafe_binding =
            encode(serde_json::json!({"data_root":"/tmp/../data","software_archive":"/software"}));
        assert!(publish(&root, &pair("a"), &unsafe_binding, None).is_err());
        assert!(!root.exists());
        let outside = fixture.0.join("target");
        std::fs::create_dir(&outside).unwrap();
        let alias = fixture.0.join("alias");
        std::os::unix::fs::symlink(outside, &alias).unwrap();
        let symlink_binding =
            encode(serde_json::json!({"data_root":alias,"software_archive":"/software"}));
        assert!(publish(&root, &pair("a"), &symlink_binding, None).is_err());
        assert!(!root.exists());
    }
    #[test]
    fn immutable_binding_collision_never_overwrites_or_advances_pointer() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let raw = pair("a");
        let bindings = binding("a");
        let first = publish(&root, &raw, &bindings, None).unwrap();
        let path = root
            .join("bindings")
            .join(format!("{}.json", first.pair_id));
        let collision = binding("collision");
        write_private(&path, &collision);
        let pointer = std::fs::read(root.join("current.json")).unwrap();
        assert!(publish(&root, &raw, &bindings, Some(&first.pair_id)).is_err());
        assert_eq!(std::fs::read(path).unwrap(), collision);
        assert_eq!(std::fs::read(root.join("current.json")).unwrap(), pointer);
    }
    #[test]
    fn every_revocation_kind_refuses_without_publishing_candidate() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let first = publish(&root, &pair("a"), &binding("a"), None).unwrap();
        let raw = pair("b");
        let value = canonical(
            &raw,
            state::METADATA_LIMITS,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        let id = state::validate_release_pair(&value, &raw).unwrap();
        let pointer = std::fs::read(root.join("current.json")).unwrap();
        for (kind, field) in [
            ("data", "data_revision"),
            ("corpus", "corpus_revision"),
            ("software", "software_sha256"),
        ] {
            let digest = state::digest(&value, field).unwrap().to_hex();
            let path = root
                .join("revocations")
                .join(kind)
                .join(format!("{digest}.json"));
            // Malformed/foreign revocation metadata also fails closed.
            write_private(&path, b"not a trusted revocation record");
            assert!(publish(&root, &raw, &binding("b"), Some(&first.pair_id)).is_err());
            assert_eq!(std::fs::read(root.join("current.json")).unwrap(), pointer);
            assert!(!root.join("pairs").join(format!("{id}.json")).exists());
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn publication_io_refusal_never_commits_pointer() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let a = pair("a");
        let b = binding("a");
        struct Refuse;
        impl Charges for Refuse {
            fn io(&mut self, _: usize) -> Result<()> {
                Err("original IO exhausted".into())
            }
            fn state(&mut self, _: usize) -> Result<()> {
                Ok(())
            }
            fn available_state(&self) -> usize {
                64 * 1024 * 1024
            }
            fn verify_inputs(&mut self) -> Result<()> {
                Ok(())
            }
        }
        let result = publish_verified_pair(
            &root,
            &a,
            &b,
            None,
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut Refuse,
        );
        assert!(result.is_err());
        assert!(!root.join("current.json").exists());
    }
    #[test]
    fn expired_verified_input_guard_cannot_advance_current() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let a = pair("a");
        let b = binding("a");
        struct Drift(TestCharges);
        impl Charges for Drift {
            fn io(&mut self, n: usize) -> Result<()> {
                self.0.io(n)
            }
            fn state(&mut self, n: usize) -> Result<()> {
                self.0.state(n)
            }
            fn available_state(&self) -> usize {
                self.0.available_state()
            }
            fn verify_inputs(&mut self) -> Result<()> {
                Err("verified candidate identity changed".into())
            }
        }
        let result = publish_verified_pair(
            &root,
            &a,
            &b,
            None,
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut Drift(TestCharges { io: 0, state: 0 }),
        );
        assert!(result.is_err());
        assert!(!root.join("current.json").exists());
        assert_eq!(std::fs::read_dir(root.join("pairs")).unwrap().count(), 0);
    }
    #[test]
    fn rollback_reverifies_exact_previous_and_preserves_pointer_on_refusal() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let first_pair = pair("rollback-a");
        let first_bindings = binding("rollback-a");
        let first = publish(&root, &first_pair, &first_bindings, None).unwrap();
        let second = publish(
            &root,
            &pair("rollback-b"),
            &binding("rollback-b"),
            Some(&first.pair_id),
        )
        .unwrap();
        let original_pointer = std::fs::read(root.join("current.json")).unwrap();
        let previous = previous_release(
            &root,
            &second.pair_id,
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        assert_eq!(previous.current, second.pair_id);
        assert_eq!(previous.previous, first.pair_id);
        assert_eq!(previous.pair_raw, first_pair);
        assert_eq!(previous.bindings_raw, first_bindings);

        struct RefuseVerification(TestCharges);
        impl Charges for RefuseVerification {
            fn io(&mut self, n: usize) -> Result<()> {
                self.0.io(n)
            }
            fn state(&mut self, n: usize) -> Result<()> {
                self.0.state(n)
            }
            fn available_state(&self) -> usize {
                self.0.available_state()
            }
            fn verify_inputs(&mut self) -> Result<()> {
                Err("previous pair verification refused".into())
            }
        }
        assert!(
            rollback_verified_previous(
                &root,
                &first_pair,
                &first_bindings,
                &second.pair_id,
                Instant::now() + Duration::from_secs(30),
                state::METADATA_LIMITS.max_bytes,
                &mut RefuseVerification(TestCharges { io: 0, state: 0 }),
            )
            .is_err()
        );
        assert_eq!(std::fs::read(root.join("current.json")).unwrap(), original_pointer);

        let rolled_back = rollback_verified_previous(
            &root,
            &first_pair,
            &first_bindings,
            &second.pair_id,
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        assert!(rolled_back.committed && rolled_back.durable);
        assert_eq!(rolled_back.pair_id, first.pair_id);
        assert_eq!(rolled_back.previous.as_deref(), Some(second.pair_id.as_str()));
        let pointer: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("current.json")).unwrap()).unwrap();
        assert_eq!(pointer["current"], first.pair_id);
        assert_eq!(pointer["previous"], second.pair_id);
    }

    #[test]
    fn revocation_is_immutable_and_rollback_never_resurrects_it() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let first_pair = pair("revoke-a");
        let first_bindings = binding("revoke-a");
        let first = publish(&root, &first_pair, &first_bindings, None).unwrap();
        let second_pair = pair("revoke-b");
        let second_bindings = binding("revoke-b");
        let second =
            publish(&root, &second_pair, &second_bindings, Some(&first.pair_id)).unwrap();
        let second_value = canonical(
            &second_pair,
            state::METADATA_LIMITS,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        let digest = state::digest(&second_value, "software_sha256")
            .unwrap()
            .to_hex();
        let revoke = |reason: &str| {
            revoke_digest(
                &root,
                "software",
                &digest,
                reason,
                "test:release-state",
                Instant::now() + Duration::from_secs(30),
                state::METADATA_LIMITS.max_bytes,
                &mut TestCharges { io: 0, state: 0 },
            )
        };
        revoke("current release revoked").unwrap();
        let path = root
            .join("revocations/software")
            .join(format!("{digest}.json"));
        let original_record = std::fs::read(&path).unwrap();
        assert!(revoke("different reason").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original_record);

        // A revoked current pair can be replaced by a verified previous pair;
        // rollback neither deletes nor rewrites the current pair's revocation.
        let rollback = rollback_verified_previous(
            &root,
            &first_pair,
            &first_bindings,
            &second.pair_id,
            Instant::now() + Duration::from_secs(30),
            state::METADATA_LIMITS.max_bytes,
            &mut TestCharges { io: 0, state: 0 },
        )
        .unwrap();
        assert!(rollback.committed && rollback.durable);
        assert_eq!(std::fs::read(&path).unwrap(), original_record);
        let pointer_before_refused_rollback = std::fs::read(root.join("current.json")).unwrap();

        // The same revoked pair can never be restored later as the previous.
        assert!(
            rollback_verified_previous(
                &root,
                &second_pair,
                &second_bindings,
                &first.pair_id,
                Instant::now() + Duration::from_secs(30),
                state::METADATA_LIMITS.max_bytes,
                &mut TestCharges { io: 0, state: 0 },
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(root.join("current.json")).unwrap(),
            pointer_before_refused_rollback
        );
        assert_eq!(std::fs::read(&path).unwrap(), original_record);
    }

    #[test]
    fn each_revoked_previous_component_blocks_rollback_without_pointer_change() {
        for (kind, field) in [
            ("data", "data_revision"),
            ("corpus", "corpus_revision"),
            ("software", "software_sha256"),
        ] {
            let fixture = Fixture::new();
            let root = fixture.root();
            let target_pair = pair(&format!("rollback-target-{kind}"));
            let target_bindings = binding(&format!("rollback-target-{kind}"));
            let target =
                publish(&root, &target_pair, &target_bindings, None).unwrap();
            let current = publish(
                &root,
                &pair(&format!("rollback-current-{kind}")),
                &binding(&format!("rollback-current-{kind}")),
                Some(&target.pair_id),
            )
            .unwrap();
            let target_value = canonical(
                &target_pair,
                state::METADATA_LIMITS,
                &mut TestCharges { io: 0, state: 0 },
            )
            .unwrap();
            let digest = state::digest(&target_value, field).unwrap().to_hex();
            revoke_digest(
                &root,
                kind,
                &digest,
                "previous component revoked",
                "test:release-state",
                Instant::now() + Duration::from_secs(30),
                state::METADATA_LIMITS.max_bytes,
                &mut TestCharges { io: 0, state: 0 },
            )
            .unwrap();
            let pointer_before = std::fs::read(root.join("current.json")).unwrap();
            assert!(
                rollback_verified_previous(
                    &root,
                    &target_pair,
                    &target_bindings,
                    &current.pair_id,
                    Instant::now() + Duration::from_secs(30),
                    state::METADATA_LIMITS.max_bytes,
                    &mut TestCharges { io: 0, state: 0 },
                )
                .is_err(),
                "rollback restored revoked {kind} component"
            );
            assert_eq!(std::fs::read(root.join("current.json")).unwrap(), pointer_before);
            assert!(
                root.join("revocations")
                    .join(kind)
                    .join(format!("{digest}.json"))
                    .is_file()
            );
        }
    }
}
