//! Maintained CorpusStore.restore for an explicit immutable v1 revision.
//! Restored bytes confer no admission, semantic, rights, or publication grant.
use super::source_admission::{active, invalid};
use super::source_admission_store::AdmissionStore;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, Permissions},
    io::{self, Read},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, SourceRevision};
use tos_source_store::{ReadLimits, Snapshot};

#[derive(Clone, Copy)]
pub struct RestoreLimits {
    pub reader: ReadLimits,
    pub max_read_bytes: u64,
    pub max_write_bytes: u64,
    pub max_directories: usize,
    /// Derived directory/index/leaf tracking payload. The selected reader's
    /// manifest/JSON limits separately bound its parsed Snapshot and the
    /// returned serde Value; both DOMs, allocator overhead and descriptor state
    /// must be included in the caller's whole-operation RSS/storage admission.
    pub max_state_bytes: usize,
}
impl RestoreLimits {
    pub fn validate(self) -> io::Result<Self> {
        self.reader.validate().map_err(invalid)?;
        if self.max_read_bytes == 0
            || self.max_read_bytes == u64::MAX
            || self.max_write_bytes == 0
            || self.max_write_bytes == u64::MAX
            || self.max_directories == 0
            || self.max_directories == usize::MAX
            || self.max_state_bytes == 0
            || self.max_state_bytes == usize::MAX
        {
            return Err(invalid("invalid source restore operation limits"));
        }
        Ok(self)
    }
}
fn identity(file: &File) -> io::Result<(u64, u64)> {
    let m = file.metadata()?;
    Ok((m.dev(), m.ino()))
}
fn stamp(file: &File) -> io::Result<(u64, u64, u64, u32, i64, i64, i64, i64)> {
    let m = file.metadata()?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
fn plus(total: &mut u64, n: u64, cap: u64) -> io::Result<()> {
    *total = total
        .checked_add(n)
        .filter(|v| *v <= cap)
        .ok_or_else(|| invalid("source restore cumulative byte bound exceeded"))?;
    Ok(())
}
fn original(snapshot: &Snapshot) -> Value {
    let identities = snapshot
        .indexed_identities()
        .map(|(id, p)| (id.to_owned(), p.as_str().to_owned()))
        .collect::<BTreeMap<_, _>>();
    let dependencies = snapshot
        .members()
        .filter_map(|m| {
            snapshot.indexed_dependencies(&m.path).map(|v| {
                (
                    m.path.as_str().to_owned(),
                    v.iter().map(|p| p.as_str().to_owned()).collect::<Vec<_>>(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let files=snapshot.members().map(|m|json!({"path":m.path.as_str(),"sha256":m.sha256.to_hex(),"size_bytes":m.size_bytes,"mode":m.mode})).collect::<Vec<_>>();
    let retirements=snapshot.retirements().iter().map(|r|json!({"path":r.path.as_str(),"sha256":r.sha256.to_hex(),"event_ref":r.event_ref.as_str(),"event_sha256":r.event_sha256.to_hex(),"event_size_bytes":r.event_size_bytes})).collect::<Vec<_>>();
    json!({"schema_version":"tos_corpus_snapshot_v1","revision":snapshot.revision().0.to_hex(),"base_revision":snapshot.base_revision().map(|r|r.0.to_hex()),"validator_sha256":snapshot.validator_sha256().to_hex(),"files":files,"identities":identities,"dependencies":dependencies,"retirements":retirements})
}
struct Directory {
    parent: usize,
    name: String,
    held: File,
}
struct Leaf {
    parent: usize,
    name: String,
    stamp: (u64, u64, u64, u32, i64, i64, i64, i64),
}
struct Stage {
    parent: File,
    name: String,
    root: File,
    dirs: Vec<Directory>,
    leaves: Vec<Leaf>,
    published: bool,
}
impl Stage {
    fn directory(&self, index: usize) -> &File {
        if index == 0 {
            &self.root
        } else {
            &self.dirs[index - 1].held
        }
    }
    fn verify(&self, deadline: Instant, cancel: &AtomicBool) -> io::Result<()> {
        active(deadline, cancel)?;
        let selected =
            tos_fd_open::open_directory_at(&self.parent, Path::new(&self.name)).map_err(invalid)?;
        if identity(&selected)? != identity(&self.root)? {
            return Err(invalid("restore stage replaced"));
        }
        for dir in &self.dirs {
            active(deadline, cancel)?;
            let selected =
                tos_fd_open::open_directory_at(self.directory(dir.parent), Path::new(&dir.name))
                    .map_err(invalid)?;
            if identity(&selected)? != identity(&dir.held)? {
                return Err(invalid("restore directory replaced"));
            }
        }
        for leaf in &self.leaves {
            active(deadline, cancel)?;
            let selected =
                tos_fd_open::open_regular_at(self.directory(leaf.parent), Path::new(&leaf.name))
                    .map_err(invalid)?;
            if stamp(&selected)? != leaf.stamp {
                return Err(invalid("restored file changed before commit"));
            }
        }
        Ok(())
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        // Only explicitly created inode-bound names, never recursive deletion.
        // Unexpected content/namespace substitution is preserved for its owner.
        for leaf in self.leaves.iter().rev() {
            if let Ok(current) =
                tos_fd_open::open_regular_at(self.directory(leaf.parent), Path::new(&leaf.name))
            {
                if identity(&current).is_ok_and(|id| id == (leaf.stamp.0, leaf.stamp.1)) {
                    let _ = rustix::fs::unlinkat(
                        self.directory(leaf.parent),
                        leaf.name.as_str(),
                        AtFlags::empty(),
                    );
                }
            }
        }
        for dir in self.dirs.iter().rev() {
            if let Ok(current) =
                tos_fd_open::open_directory_at(self.directory(dir.parent), Path::new(&dir.name))
            {
                if matches!((identity(&current),identity(&dir.held)),(Ok(a),Ok(b)) if a==b) {
                    let _ = rustix::fs::unlinkat(
                        self.directory(dir.parent),
                        dir.name.as_str(),
                        AtFlags::REMOVEDIR,
                    );
                }
            }
        }
        if let Ok(current) = tos_fd_open::open_directory_at(&self.parent, Path::new(&self.name)) {
            if matches!((identity(&current),identity(&self.root)),(Ok(a),Ok(b)) if a==b) {
                let _ = rustix::fs::unlinkat(&self.parent, self.name.as_str(), AtFlags::REMOVEDIR);
            }
        }
    }
}
fn output_path(output: &Path) -> io::Result<(PathBuf, String, usize)> {
    if !output.is_absolute() {
        return Err(invalid("restore output must be absolute"));
    }
    let mut normalized = PathBuf::from("/");
    let mut depth = 0usize;
    for component in output.components() {
        match component {
            Component::RootDir => (),
            Component::Normal(p) => {
                normalized.push(p);
                depth += 1;
            }
            _ => return Err(invalid("restore output must be a new, non-symlink path")),
        }
    }
    if normalized.as_os_str().as_bytes() != output.as_os_str().as_bytes() {
        return Err(invalid("restore output must be normalized"));
    }
    let parent = normalized
        .parent()
        .ok_or_else(|| invalid("restore output parent absent"))?
        .to_owned();
    let name = normalized
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| invalid("restore output name absent"))?
        .to_owned();
    Ok((parent, name, depth.saturating_sub(1)))
}
fn parent(path: &Path, deadline: Instant, cancel: &AtomicBool) -> io::Result<File> {
    let mut held = tos_fd_open::open_absolute_directory(Path::new("/")).map_err(invalid)?;
    for component in path.components() {
        active(deadline, cancel)?;
        match component {
            Component::RootDir => (),
            Component::Normal(p) => {
                match rustix::fs::mkdirat(&held, p, Mode::from_raw_mode(0o700)) {
                    Ok(()) => held.sync_all()?,
                    Err(Errno::EXIST) => (),
                    Err(e) => return Err(e.into()),
                }
                held = tos_fd_open::open_directory_at(&held, Path::new(p)).map_err(invalid)?;
            }
            _ => return Err(invalid("restore output parent is not normalized")),
        }
    }
    Ok(held)
}
fn existing_ancestor(path: &Path, deadline: Instant, cancel: &AtomicBool) -> io::Result<File> {
    let mut held = tos_fd_open::open_absolute_directory(Path::new("/")).map_err(invalid)?;
    for part in path.components() {
        active(deadline, cancel)?;
        if let Component::Normal(part) = part {
            match tos_fd_open::open_directory_at(&held, Path::new(part)) {
                Ok(next) => held = next,
                Err(e)
                    if e.source
                        .as_ref()
                        .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
                {
                    return Ok(held);
                }
                Err(e) => return Err(invalid(e)),
            }
        }
    }
    Ok(held)
}
fn fresh(parent: &File, name: &str) -> io::Result<()> {
    match rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => Ok(()),
        Ok(_) => Err(invalid("restore output must be a new, non-symlink path")),
        Err(e) => Err(e.into()),
    }
}
fn readback(
    file: &mut File,
    size: u64,
    digest: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    use std::io::{Seek, SeekFrom};
    let before = stamp(file)?;
    file.seek(SeekFrom::Start(0))?;
    let mut count = 0u64;
    let mut hash = Digest256Hasher::new();
    let mut chunk = [0; 65536];
    loop {
        active(deadline, cancel)?;
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        plus(&mut count, n as u64, size)?;
        hash.update(&chunk[..n]);
    }
    if count != size || hash.finalize() != digest || stamp(file)? != before {
        return Err(invalid("corpus object changed during restore"));
    }
    Ok(())
}

/// Actual existing-store recovery entrypoint. It neither creates a store nor
/// chooses current; the caller explicitly binds one immutable revision.
pub fn restore_revision(
    store_path: &Path,
    revision: Digest256,
    output: &Path,
    limits: RestoreLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<Value> {
    let limits = limits.validate()?;
    active(deadline, cancel)?;
    let store = AdmissionStore::open_existing(store_path, deadline, cancel)?;
    restore(&store, revision, output, limits, deadline, cancel)
}
pub(crate) fn restore(
    store: &AdmissionStore,
    revision: Digest256,
    output: &Path,
    limits: RestoreLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<Value> {
    restore_guarded(
        store,
        revision,
        output,
        limits,
        deadline,
        cancel,
        &|| Ok(()),
    )
}

#[derive(Debug)]
pub(crate) struct RestoreCommittedRefusal {
    pub revision: Digest256,
    pub output: PathBuf,
    pub output_dev: u64,
    pub output_ino: u64,
    pub manifest_sha256: Digest256,
    pub restored_files: usize,
    pub reason: &'static str,
}
impl std::fmt::Display for RestoreCommittedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "restore output committed; {}", self.reason)
    }
}
impl std::error::Error for RestoreCommittedRefusal {}

pub(crate) fn restore_guarded(
    store: &AdmissionStore,
    revision: Digest256,
    output: &Path,
    limits: RestoreLimits,
    deadline: Instant,
    cancel: &AtomicBool,
    fence: &dyn Fn() -> io::Result<()>,
) -> io::Result<Value> {
    let limits = limits.validate()?;
    active(deadline, cancel)?;
    let (parent_path, name, parent_depth) = output_path(output)?;
    let reader = store.reader(limits.reader)?;
    let mut reads = 0u64;
    let mut writes = 0u64;
    plus(
        &mut reads,
        limits.reader.max_manifest_bytes as u64,
        limits.max_read_bytes,
    )?;
    let snapshot = reader
        .load_exact(SourceRevision(revision))
        .map_err(invalid)?;
    let manifest = original(&snapshot);
    let manifest_sha256 = Digest256::of_bytes(&super::source_admission_candidate::canonical(
        &manifest,
        limits.reader.json,
    )?);
    let restored_files = snapshot.member_count();
    active(deadline, cancel)?;
    let mut directories = BTreeSet::new();
    let mut live = 0u64;
    let mut state = 0u64;
    if parent_depth
        .checked_add(1)
        .is_none_or(|n| n > limits.max_directories)
    {
        return Err(invalid("restore directory bound exceeded"));
    }
    for member in snapshot.members() {
        active(deadline, cancel)?;
        if member.size_bytes > limits.reader.max_selected_object_bytes {
            return Err(invalid("restore member exceeds selected byte bound"));
        }
        plus(&mut live, member.size_bytes, limits.max_write_bytes)?;
        plus(
            &mut state,
            member.path.as_str().len() as u64 + 128,
            limits.max_state_bytes as u64,
        )?;
        let mut prefix = String::new();
        let parent = member.path.as_str().rsplit_once('/').map_or("", |(p, _)| p);
        for part in parent.split('/').filter(|p| !p.is_empty()) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if !directories.contains(&prefix) {
                if directories
                    .len()
                    .checked_add(parent_depth)
                    .and_then(|n| n.checked_add(2))
                    .is_none_or(|n| n > limits.max_directories)
                {
                    return Err(invalid("restore directory bound exceeded"));
                }
                plus(
                    &mut state,
                    (prefix.len() as u64).saturating_mul(2).saturating_add(192),
                    limits.max_state_bytes as u64,
                )?;
                directories.insert(prefix.clone());
            }
        }
        if directories
            .len()
            .checked_add(parent_depth)
            .and_then(|n| n.checked_add(1))
            .is_none_or(|n| n > limits.max_directories)
        {
            return Err(invalid("restore directory bound exceeded"));
        }
    }
    // Whole live verification + copy + destination readback, before any writes.
    plus(
        &mut reads,
        live.checked_mul(3)
            .ok_or_else(|| invalid("restore read cost overflow"))?,
        limits.max_read_bytes,
    )?;
    plus(&mut writes, live, limits.max_write_bytes)?;
    let mut retired = Vec::new();
    for event in snapshot.retirements() {
        active(deadline, cancel)?;
        let size = store.object_size(
            event.sha256,
            limits.reader.max_selected_object_bytes,
            deadline,
            cancel,
        )?;
        plus(&mut reads, size, limits.max_read_bytes)?;
        plus(&mut reads, event.event_size_bytes, limits.max_read_bytes)?;
        plus(&mut state, 80, limits.max_state_bytes as u64)?;
        retired.push((
            event.sha256,
            size,
            event.event_sha256,
            event.event_size_bytes,
        ));
    }
    // Preserve maintained load(verify_objects=True), including historical
    // objects not materialized as live files in the restored source tree.
    for member in snapshot.members() {
        store.verify_object(member.sha256, member.size_bytes, deadline, cancel)?;
    }
    for (digest, size, event_digest, event_size) in retired {
        store.verify_object(digest, size, deadline, cancel)?;
        store.verify_object(event_digest, event_size, deadline, cancel)?;
    }
    let ancestor = existing_ancestor(&parent_path, deadline, cancel)?;
    let stat = rustix::fs::fstatvfs(&ancestor)?;
    let block = stat.f_frsize.max(stat.f_bsize).max(1);
    let mut disk = 0u64;
    for member in snapshot.members() {
        let blocks = member
            .size_bytes
            .checked_add(block - 1)
            .ok_or_else(|| invalid("restore disk cost overflow"))?
            / block;
        disk = disk
            .checked_add(
                blocks
                    .checked_mul(block)
                    .ok_or_else(|| invalid("restore disk cost overflow"))?,
            )
            .ok_or_else(|| invalid("restore disk cost overflow"))?;
    }
    disk = disk
        .checked_add(
            (directories.len() as u64
                + parent_depth as u64
                + snapshot.members().count() as u64
                + 1)
            .checked_mul(block)
            .ok_or_else(|| invalid("restore disk cost overflow"))?,
        )
        .ok_or_else(|| invalid("restore disk cost overflow"))?;
    if disk > stat.f_bavail.saturating_mul(stat.f_frsize.max(1)) {
        return Err(invalid("insufficient restore disk space"));
    }
    let parent = parent(&parent_path, deadline, cancel)?;
    fresh(&parent, &name)?;
    let actual = rustix::fs::fstatvfs(&parent)?;
    if disk > actual.f_bavail.saturating_mul(actual.f_frsize.max(1)) {
        return Err(invalid("insufficient restore disk space"));
    }
    let mut random = [0; 24];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let stage_name = format!(".corpus-restore-{}", Digest256::of_bytes(&random).to_hex());
    rustix::fs::mkdirat(&parent, stage_name.as_str(), Mode::from_raw_mode(0o700))?;
    let root = tos_fd_open::open_directory_at(&parent, Path::new(&stage_name)).map_err(invalid)?;
    let mut stage = Stage {
        parent,
        name: stage_name,
        root,
        dirs: Vec::new(),
        leaves: Vec::new(),
        published: false,
    };
    let mut directory_index = BTreeMap::from([(String::new(), 0usize)]);
    for path in &directories {
        active(deadline, cancel)?;
        let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
        let index = *directory_index
            .get(parent)
            .ok_or_else(|| invalid("restore directory prefix absent"))?;
        rustix::fs::mkdirat(stage.directory(index), name, Mode::from_raw_mode(0o700))?;
        let held = tos_fd_open::open_directory_at(stage.directory(index), Path::new(name))
            .map_err(invalid)?;
        stage.dirs.push(Directory {
            parent: index,
            name: name.to_owned(),
            held,
        });
        directory_index.insert(path.clone(), stage.dirs.len());
    }
    for member in snapshot.members() {
        active(deadline, cancel)?;
        let (parent, name) = member
            .path
            .as_str()
            .rsplit_once('/')
            .unwrap_or(("", member.path.as_str()));
        let index = *directory_index
            .get(parent)
            .ok_or_else(|| invalid("restore file prefix absent"))?;
        let mut file = File::from(rustix::fs::openat(
            stage.directory(index),
            name,
            OFlags::CREATE | OFlags::EXCL | OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        let initial = stamp(&file)?;
        stage.leaves.push(Leaf {
            parent: index,
            name: name.to_owned(),
            stamp: initial,
        });
        store.copy_object(
            member.sha256,
            member.size_bytes,
            &mut file,
            deadline,
            cancel,
        )?;
        file.set_permissions(Permissions::from_mode(member.mode))?;
        file.sync_all()?;
        readback(
            &mut file,
            member.size_bytes,
            member.sha256,
            deadline,
            cancel,
        )?;
        stage
            .leaves
            .last_mut()
            .ok_or_else(|| invalid("restore leaf tracking absent"))?
            .stamp = stamp(&file)?;
    }
    for dir in stage.dirs.iter().rev() {
        active(deadline, cancel)?;
        dir.held.sync_all()?;
    }
    stage.root.sync_all()?;
    stage.verify(deadline, cancel)?;
    let selected_parent = tos_fd_open::open_absolute_directory(&parent_path).map_err(invalid)?;
    if identity(&selected_parent)? != identity(&stage.parent)? {
        return Err(invalid("restore output parent replaced"));
    }
    fresh(&stage.parent, &name)?;
    active(deadline, cancel)?;
    let output_identity = identity(&stage.root)?;
    store.verify_layout()?;
    fence()?;
    rustix::fs::renameat_with(
        &stage.parent,
        stage.name.as_str(),
        &stage.parent,
        name.as_str(),
        RenameFlags::NOREPLACE,
    )?;
    stage.published = true;
    let terminal = (|| -> io::Result<()> {
        stage.parent.sync_all()?;
        let installed =
            tos_fd_open::open_directory_at(&stage.parent, Path::new(&name)).map_err(invalid)?;
        if identity(&installed)? != output_identity {
            return Err(invalid("restore output replaced during publication"));
        }
        let selected_parent =
            tos_fd_open::open_absolute_directory(&parent_path).map_err(invalid)?;
        if identity(&selected_parent)? != identity(&stage.parent)? {
            return Err(invalid("restore output parent replaced during publication"));
        }
        store.verify_layout()?;
        active(deadline, cancel)?;
        fence()?;
        Ok(())
    })();
    if terminal.is_err() {
        return Err(io::Error::other(RestoreCommittedRefusal {
            revision,
            output: output.to_owned(),
            output_dev: output_identity.0,
            output_ino: output_identity.1,
            manifest_sha256,
            restored_files,
            reason: "terminal sync/layout/current/invocation/executable custody refused; preserve output",
        }));
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_admission_candidate::canonical;
    use std::{fs, time::Duration};

    /// Physical restore/control fixture only, never source admission evidence.
    #[test]
    fn restore_exact_live_bytes_modes_and_historical_custody_without_pointer_change() {
        let temp = tempfile::tempdir().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let store_path = temp.path().join("store");
        let store = AdmissionStore::create(&store_path, deadline, &cancel).unwrap();
        let json = tos_foundation::JsonLimits::new(16384, 32, 4096, 4300).unwrap();
        let reader = ReadLimits {
            max_manifest_bytes: 16384,
            max_manifest_entries: 32,
            max_selected_object_bytes: 4096,
            json,
        };
        let limits = RestoreLimits {
            reader,
            max_read_bytes: 65536,
            max_write_bytes: 4096,
            max_directories: 32,
            max_state_bytes: 16384,
        };
        let live = b"#!/bin/sh\nexit 0\n";
        let live_digest = Digest256::of_bytes(live);
        let old_digest = Digest256::of_bytes(b"old");
        let event_digest = Digest256::of_bytes(b"{}");
        for (n, bytes, digest) in [
            ("live", live.as_slice(), live_digest),
            ("old", b"old".as_slice(), old_digest),
            ("event", b"{}".as_slice(), event_digest),
        ] {
            let input = temp.path().join(n);
            fs::write(&input, bytes).unwrap();
            store
                .ingest(
                    &mut File::open(&input).unwrap(),
                    bytes.len() as u64,
                    digest,
                    deadline,
                    &cancel,
                )
                .unwrap();
        }
        store.sync_objects(deadline, &cancel).unwrap();
        let mut value = json!({"schema_version":"tos_corpus_snapshot_v1","base_revision":null,"validator_sha256":Digest256::of_bytes(b"physical fixture").to_hex(),"files":[{"path":"ToS/a/run.sh","sha256":live_digest.to_hex(),"size_bytes":live.len(),"mode":493}],"identities":{},"dependencies":{},"retirements":[{"path":"ToS/source-witnesses/old.md","sha256":old_digest.to_hex(),"event_ref":"ToS/source-witnesses/retirements/old.json","event_sha256":event_digest.to_hex(),"event_size_bytes":2}]});
        let revision = Digest256::of_bytes(&canonical(&value, json).unwrap());
        value["revision"] = revision.to_hex().into();
        store
            .publish(
                None,
                revision,
                &canonical(&value, json).unwrap(),
                reader,
                deadline,
                &cancel,
                &|_| Ok(()),
            )
            .unwrap();
        let absent = temp.path().join("absent/store");
        assert!(AdmissionStore::open_existing(&absent, deadline, &cancel).is_err());
        assert!(!absent.parent().unwrap().exists());
        let store = AdmissionStore::open_existing(&store_path, deadline, &cancel).unwrap();
        let output = temp.path().join("missing/deep/source");
        assert_eq!(
            restore_revision(&store_path, revision, &output, limits, deadline, &cancel).unwrap(),
            value
        );
        assert_eq!(fs::read(output.join("ToS/a/run.sh")).unwrap(), live);
        assert_eq!(
            fs::metadata(output.join("ToS/a/run.sh")).unwrap().mode() & 0o777,
            0o755
        );
        assert!(restore(&store, revision, &output, limits, deadline, &cancel).is_err());
        let symlink = temp.path().join("linked");
        std::os::unix::fs::symlink(temp.path().join("missing"), &symlink).unwrap();
        assert!(
            restore(
                &store,
                revision,
                &symlink.join("fresh"),
                limits,
                deadline,
                &cancel
            )
            .is_err()
        );
        let quota = temp.path().join("quota/source");
        assert!(
            restore(
                &store,
                revision,
                &quota,
                RestoreLimits {
                    max_write_bytes: 1,
                    ..limits
                },
                deadline,
                &cancel
            )
            .is_err()
        );
        assert!(!quota.parent().unwrap().exists());
        let old = store_path.join("objects").join(old_digest.to_hex());
        fs::set_permissions(&old, Permissions::from_mode(0o600)).unwrap();
        fs::write(&old, b"bad").unwrap();
        let bad = temp.path().join("bad/source");
        assert!(restore(&store, revision, &bad, limits, deadline, &cancel).is_err());
        assert!(!bad.parent().unwrap().exists());
        assert_eq!(
            store.reader(reader).unwrap().select_current().unwrap(),
            Some(SourceRevision(revision))
        );
        assert!(fs::read_dir(output.parent().unwrap()).unwrap().all(|e| {
            !e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".corpus-restore-")
        }));
        // Failed-stage cleanup removes only captured owned inodes. Unknown
        // content prevents root removal and is left for its actual owner.
        let parent = tos_fd_open::open_absolute_directory(temp.path()).unwrap();
        fs::create_dir(temp.path().join("owned-stage")).unwrap();
        let root = tos_fd_open::open_directory_at(&parent, Path::new("owned-stage")).unwrap();
        fs::write(temp.path().join("owned-stage/ours"), b"partial").unwrap();
        let ours = tos_fd_open::open_regular_at(&root, Path::new("ours")).unwrap();
        fs::write(temp.path().join("owned-stage/foreign"), b"preserve").unwrap();
        let stage = Stage {
            parent,
            name: "owned-stage".to_owned(),
            root,
            dirs: vec![],
            leaves: vec![Leaf {
                parent: 0,
                name: "ours".to_owned(),
                stamp: stamp(&ours).unwrap(),
            }],
            published: false,
        };
        drop(stage);
        assert!(!temp.path().join("owned-stage/ours").exists());
        assert_eq!(
            fs::read(temp.path().join("owned-stage/foreign")).unwrap(),
            b"preserve"
        );
    }
}
