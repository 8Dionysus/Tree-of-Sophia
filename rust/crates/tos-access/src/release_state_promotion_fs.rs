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
    CanonicalProfile, Digest256, JsonLimits, JsonMode, JsonValue, canonical_bytes_v1, parse_json,
};

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
        let file = directory(&parent, &leaf, deadline)?;
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
        let root = Root::acquire(root, deadline)?;
        let lock = open_at(
            &root.file,
            OsStr::new(".release.lock"),
            libc::O_RDWR | libc::O_CREAT,
        )?;
        regular(&lock)?;
        loop {
            active(deadline)?;
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
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
        let pairs = directory(&root.file, OsStr::new("pairs"), deadline)?;
        let bindings = directory(&root.file, OsStr::new("bindings"), deadline)?;
        let revocations = directory(&root.file, OsStr::new("revocations"), deadline)?;
        let revoked_data = directory(&revocations, OsStr::new("data"), deadline)?;
        let revoked_corpus = directory(&revocations, OsStr::new("corpus"), deadline)?;
        let revoked_software = directory(&revocations, OsStr::new("software"), deadline)?;
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
fn read_optional(dir: &File, leaf: &str, cap: usize, deadline: Instant) -> Result<Option<Vec<u8>>> {
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
    let mut raw = Vec::with_capacity(before.size as usize);
    let mut block = [0u8; 8192];
    loop {
        active(deadline)?;
        let count = file
            .read(&mut block)
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
fn canonical(raw: &[u8], limits: JsonLimits) -> Result<JsonValue> {
    let value = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| "release metadata JSON invalid")?
        .into_root();
    if canonical_bytes_v1(&value, CanonicalProfile::CorpusSnapshotV1, limits)
        .map_err(|_| "release metadata canonicalization failed")?
        != raw
    {
        return Err("release metadata is not canonical".into());
    }
    Ok(value)
}
fn bindings(raw: &[u8], limits: JsonLimits, deadline: Instant) -> Result<JsonValue> {
    let value = canonical(raw, limits)?;
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
) -> Result<Option<(String, Option<String>)>> {
    read_optional(&store.root.file, "current.json", limits.max_bytes, deadline)?
        .map(|raw| {
            let value = canonical(&raw, limits)?;
            state::validate_release_pointer(&value).map_err(|error| error.to_string())
        })
        .transpose()
}
fn stored_pair(store: &Store, id: &str, limits: JsonLimits, deadline: Instant) -> Result<()> {
    let leaf = format!("{id}.json");
    let raw = read_optional(&store.pairs, &leaf, limits.max_bytes, deadline)?
        .ok_or("current immutable pair absent")?;
    let value = canonical(&raw, limits)?;
    if state::validate_release_pair(&value, &raw).map_err(|error| error.to_string())? != id {
        return Err("current pair filename digest differs".into());
    }
    let raw = read_optional(&store.bindings, &leaf, limits.max_bytes, deadline)?
        .ok_or("current immutable bindings absent")?;
    bindings(&raw, limits, deadline)?;
    Ok(())
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
fn temporary<'a>(dir: &'a File, raw: &[u8], deadline: Instant) -> Result<Temporary<'a>> {
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
) -> Result<()> {
    if let Some(existing) = read_optional(dir, leaf, limits.max_bytes, deadline)? {
        if existing != raw {
            return Err("existing immutable release record differs".into());
        }
        let held = tos_fd_open::open_regular_at(dir, Path::new(leaf))
            .map_err(|_| "identical immutable record unavailable for sync")?;
        regular(&held)?;
        sync(&held, deadline)?;
        return sync(dir, deadline);
    }
    let mut temp = temporary(dir, raw, deadline)?;
    active(deadline)?;
    temp.check()?;
    let src = name(OsStr::new(&temp.leaf))?;
    let dst = name(OsStr::new(leaf))?;
    if unsafe {
        libc::linkat(
            dir.as_raw_fd(),
            src.as_ptr(),
            dir.as_raw_fd(),
            dst.as_ptr(),
            0,
        )
    } != 0
    {
        if std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            return Err(failure("immutable release hardlink failed"));
        }
    }
    // Remove only our held temporary name before enforcing final nlink=1.
    temp.remove()?;
    temp.linked = true;
    let existing = read_optional(dir, leaf, limits.max_bytes, deadline)?
        .ok_or("immutable publication name absent")?;
    if existing != raw {
        return Err("immutable release publication collision".into());
    }
    sync(dir, deadline)
}
fn pointer_raw(pair_id: &str, previous: Option<&str>, limits: JsonLimits) -> Result<Vec<u8>> {
    let encoded=serde_json::to_vec(&serde_json::json!({"schema_version":"tos_access_release_pointer_v1","current":pair_id,"previous":previous}))
        .map_err(|_| "release pointer encoding failed")?;
    let value = parse_json(&encoded, JsonMode::PublishedStrict, limits)
        .map_err(|_| "release pointer JSON invalid")?
        .into_root();
    state::validate_release_pointer(&value).map_err(|error| error.to_string())?;
    canonical_bytes_v1(&value, CanonicalProfile::CorpusSnapshotV1, limits)
        .map_err(|_| "release pointer canonicalization failed".into())
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
) -> Result<Publication> {
    active(deadline)?;
    if max_metadata_bytes == 0 || max_metadata_bytes > state::METADATA_LIMITS.max_bytes {
        return Err("release metadata budget invalid".into());
    }
    let limits = JsonLimits {
        max_bytes: max_metadata_bytes,
        ..state::METADATA_LIMITS
    };
    let pair = canonical(pair_raw, limits)?;
    let pair_id =
        state::validate_release_pair(&pair, pair_raw).map_err(|error| error.to_string())?;
    bindings(bindings_raw, limits, deadline)?;
    if let Some(expected) = expected_current {
        Digest256::from_hex(expected).map_err(|_| "expected_current digest invalid")?;
    }
    let store = Store::acquire(root, deadline)?;
    let original = pointer(&store, limits, deadline)?;
    let current = original.as_ref().map(|p| p.0.as_str());
    if current != expected_current {
        return Err("expected_current does not match current release".into());
    }
    if let Some(current) = current {
        stored_pair(&store, current, limits, deadline)?;
    }
    let leaf = format!("{pair_id}.json");
    // Detect both collisions before creating either immutable record.
    for (dir, raw) in [(&store.pairs, pair_raw), (&store.bindings, bindings_raw)] {
        if let Some(existing) = read_optional(dir, &leaf, limits.max_bytes, deadline)? {
            if existing != raw {
                return Err("existing immutable release pair/bindings differs".into());
            }
        }
    }
    store.available(&pair, deadline)?;
    if current == Some(pair_id.as_str()) {
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
            if pointer(&store, limits, deadline)? != original {
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
    immutable(&store.pairs, &leaf, pair_raw, limits, deadline)?;
    immutable(&store.bindings, &leaf, bindings_raw, limits, deadline)?;
    store.check(deadline)?;
    if pointer(&store, limits, deadline)? != original {
        return Err("expected_current changed before promotion".into());
    }
    store.available(&pair, deadline)?;
    let previous = current.map(str::to_owned);
    let raw = pointer_raw(&pair_id, previous.as_deref(), limits)?;
    let mut temp = temporary(&store.root.file, &raw, deadline)?;
    store.check(deadline)?;
    temp.check()?;
    if pointer(&store, limits, deadline)? != original {
        return Err("expected_current changed before atomic publication".into());
    }
    for (dir, expected) in [(&store.pairs, pair_raw), (&store.bindings, bindings_raw)] {
        if read_optional(dir, &leaf, limits.max_bytes, deadline)?.as_deref() != Some(expected) {
            return Err("immutable pair changed before pointer commit".into());
        }
    }
    store.available(&pair, deadline)?;
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
        let actual = read_optional(&store.root.file, "current.json", limits.max_bytes, deadline)?
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
        encode(serde_json::json!({
            "schema_version":"tos_access_release_pair_v1",
            "software_sha256":Digest256::of_bytes(label.as_bytes()).to_hex(),
            "data_revision":Digest256::of_bytes(b"test data").to_hex(),
            "data_manifest_sha256":Digest256::of_bytes(b"test manifest").to_hex(),
            "corpus_revision":Digest256::of_bytes(b"test corpus").to_hex(),
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
        )
    }
    fn write_private(path: &Path, bytes: &[u8]) {
        std::fs::write(path, bytes).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn fresh_and_owned_python_layout_publish_with_exact_cas() {
        let fixture = Fixture::new();
        let root = fixture.root();
        let a = pair("a");
        let bindings = binding("a");
        let first = publish(&root, &a, &bindings, None).unwrap();
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
        let value = canonical(&raw, state::METADATA_LIMITS).unwrap();
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
}
