//! Physical custody for the maintained immutable corpus store.
//! Installing bytes is not admission. Only the complete native validator's
//! caller may perform the accepted-pointer compare-and-swap.
use super::source_admission::{active, invalid};
use rustix::fs::{AtFlags, FlockOperation, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::{
    fs::{File, Permissions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_source_store::{CorpusReader, ReadLimits};

struct SnapshotStage<'a> {
    parent: &'a File,
    name: String,
    directory: File,
    published: bool,
}
impl Drop for SnapshotStage<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if let Ok(current) = tos_fd_open::open_directory_at(self.parent, Path::new(&self.name)) {
            if matches!((identity(&current), identity(&self.directory)), (Ok(a), Ok(b)) if a == b) {
                let _ = rustix::fs::unlinkat(&self.directory, "snapshot.json", AtFlags::empty());
                let _ = rustix::fs::unlinkat(self.parent, self.name.as_str(), AtFlags::REMOVEDIR);
            }
        }
    }
}

fn identity(file: &File) -> io::Result<(u64, u64)> {
    let m = file.metadata()?;
    Ok((m.dev(), m.ino()))
}
fn owned_directory(file: File) -> io::Result<File> {
    let m = file.metadata()?;
    if !m.is_dir() || m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o022 != 0 {
        return Err(invalid("corpus write directory ownership or mode differs"));
    }
    Ok(file)
}
fn directory(parent: &File, name: &str) -> io::Result<File> {
    match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) => parent.sync_all()?,
        Err(Errno::EXIST) => (),
        Err(error) => return Err(error.into()),
    }
    owned_directory(tos_fd_open::open_directory_at(parent, Path::new(name)).map_err(invalid)?)
}
fn random_name() -> io::Result<String> {
    let mut bytes = [0; 24];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(format!(
        ".admission-{}",
        Digest256::of_bytes(&bytes).to_hex()
    ))
}

struct Temporary<'a> {
    directory: &'a File,
    name: String,
    file: File,
}
impl<'a> Temporary<'a> {
    fn create(directory: &'a File) -> io::Result<Self> {
        let name = random_name()?;
        let file = File::from(rustix::fs::openat(
            directory,
            name.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        Ok(Self {
            directory,
            name,
            file,
        })
    }
}
impl Drop for Temporary<'_> {
    fn drop(&mut self) {
        // Never remove a replaced object at the temporary name.
        if let Ok(current) = tos_fd_open::open_regular_at(self.directory, Path::new(&self.name)) {
            if matches!((identity(&current), identity(&self.file)), (Ok(a), Ok(b)) if a == b) {
                let _ = rustix::fs::unlinkat(self.directory, self.name.as_str(), AtFlags::empty());
            }
        }
    }
}

struct AdmissionLock(File);
impl Drop for AdmissionLock {
    fn drop(&mut self) {
        // Explicit unlock also releases the shared open-file description if an
        // unrelated concurrent process fork briefly inherited this CLOEXEC fd.
        let _ = rustix::fs::flock(&self.0, FlockOperation::Unlock);
    }
}

pub(crate) struct AdmissionStore {
    path: PathBuf,
    root: File,
    objects: File,
    revisions: File,
    staging: File,
}
impl AdmissionStore {
    /// Recovery is an existing-store operation. A typo may not create a
    /// different blank store or mutate its parent namespaces.
    pub(crate) fn open_existing(
        path: &Path,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        active(deadline, cancel)?;
        if !path.is_absolute() {
            return Err(invalid("corpus store must be absolute"));
        }
        let root = owned_directory(tos_fd_open::open_absolute_directory(path).map_err(invalid)?)?;
        let objects = owned_directory(
            tos_fd_open::open_directory_at(&root, Path::new("objects")).map_err(invalid)?,
        )?;
        let revisions = owned_directory(
            tos_fd_open::open_directory_at(&root, Path::new("revisions")).map_err(invalid)?,
        )?;
        let staging = owned_directory(
            tos_fd_open::open_directory_at(&root, Path::new("staging")).map_err(invalid)?,
        )?;
        let store = Self {
            path: path.to_owned(),
            root,
            objects,
            revisions,
            staging,
        };
        store.verify_layout()?;
        active(deadline, cancel)?;
        Ok(store)
    }
    pub(crate) fn reader(&self, limits: ReadLimits) -> io::Result<CorpusReader> {
        self.verify_layout()?;
        let reader = CorpusReader::open_existing(&self.path, limits).map_err(invalid)?;
        self.verify_layout()?;
        Ok(reader)
    }

    /// Compare-and-swap only after the native caller has completed the full
    /// candidate validation. Immutable, unselected revisions may survive a
    /// failed transaction and do not constitute admission.
    pub(crate) fn publish(
        &self,
        expected_base: Option<Digest256>,
        revision: Digest256,
        manifest: &[u8],
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        charge_read: &dyn Fn(u64) -> io::Result<()>,
    ) -> io::Result<()> {
        let limits = limits.validate().map_err(invalid)?;
        active(deadline, cancel)?;
        if manifest.len() > limits.max_manifest_bytes {
            return Err(invalid("corpus manifest byte bound exceeded"));
        }
        self.verify_layout()?;
        let name = revision.to_hex();
        let stage_name = random_name()?;
        rustix::fs::mkdirat(
            &self.staging,
            stage_name.as_str(),
            Mode::from_raw_mode(0o700),
        )?;
        let stage_directory = tos_fd_open::open_directory_at(&self.staging, Path::new(&stage_name))
            .map_err(invalid)?;
        let mut stage = SnapshotStage {
            parent: &self.staging,
            name: stage_name,
            directory: stage_directory,
            published: false,
        };
        let mut file = File::from(rustix::fs::openat(
            &stage.directory,
            "snapshot.json",
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        for block in manifest.chunks(65536) {
            active(deadline, cancel)?;
            file.write_all(block)?;
        }
        file.set_permissions(Permissions::from_mode(0o444))?;
        file.sync_all()?;
        verify_file(
            &mut file,
            manifest.len() as u64,
            Digest256::of_bytes(manifest),
            deadline,
            cancel,
        )?;
        stage.directory.sync_all()?;
        self.verify_layout()?;
        let existed = match rustix::fs::renameat_with(
            &self.staging,
            stage.name.as_str(),
            &self.revisions,
            name.as_str(),
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {
                stage.published = true;
                self.revisions.sync_all()?;
                self.staging.sync_all()?;
                false
            }
            Err(Errno::EXIST) => true,
            Err(error) => return Err(error.into()),
        };
        let directory =
            tos_fd_open::open_directory_at(&self.revisions, Path::new(&name)).map_err(invalid)?;
        let mut installed = tos_fd_open::open_regular_at(&directory, Path::new("snapshot.json"))
            .map_err(invalid)?;
        verify_file(
            &mut installed,
            manifest.len() as u64,
            Digest256::of_bytes(manifest),
            deadline,
            cancel,
        )?;
        let reader = self.reader(limits)?;
        // Use the maintained reader's complete canonical shape, revision,
        // index and retirement checks before this revision can become current.
        let snapshot = reader
            .load_exact(tos_foundation::SourceRevision(revision))
            .map_err(invalid)?;
        if snapshot.base_revision().map(|r| r.0) != expected_base {
            return Err(invalid("corpus manifest base differs from transaction"));
        }
        if existed {
            // Existing revisions must pass the maintained complete object
            // custody check. Do that before taking the pointer lock: no full
            // corpus scan belongs under the shared mutable namespace lock.
            for member in snapshot.members() {
                active(deadline, cancel)?;
                charge_read(member.size_bytes)?;
                self.verify_object(member.sha256, member.size_bytes, deadline, cancel)?;
            }
            for event in snapshot.retirements() {
                active(deadline, cancel)?;
                let name = event.sha256.to_hex();
                let mut old = tos_fd_open::open_regular_at(&self.objects, Path::new(&name))
                    .map_err(invalid)?;
                let size = old.metadata()?.len();
                if size > limits.max_selected_object_bytes {
                    return Err(invalid("retired object exceeds read bound"));
                }
                charge_read(size)?;
                verify_file(&mut old, size, event.sha256, deadline, cancel)?;
                charge_read(event.event_size_bytes)?;
                self.verify_object(event.event_sha256, event.event_size_bytes, deadline, cancel)?;
            }
        }
        active(deadline, cancel)?;
        let lock = self.lock(deadline, cancel)?;
        let current = reader.select_current().map_err(invalid)?.map(|r| r.0);
        if current != expected_base && current != Some(revision) {
            return Err(invalid(
                "accepted base changed; re-admit against current revision",
            ));
        }
        if current == Some(revision) {
            self.verify_layout()?;
            return active(deadline, cancel);
        }
        let value = serde_json::json!({"schema_version":"tos_corpus_pointer_v1",
            "current":revision.to_hex(), "previous":current.map(|d|d.to_hex())});
        let encoded = serde_json::to_vec(&value).map_err(invalid)?;
        let parsed = tos_foundation::parse_json(
            &encoded,
            tos_foundation::JsonMode::PublishedStrict,
            limits.json,
        )
        .map_err(invalid)?;
        let pointer = tos_foundation::canonical_bytes_v1(
            parsed.root(),
            tos_foundation::CanonicalProfile::CorpusSnapshotV1,
            limits.json,
        )
        .map_err(invalid)?;
        let mut pending = Temporary::create(&self.root)?;
        pending.file.write_all(&pointer)?;
        pending.file.sync_all()?;
        verify_file(
            &mut pending.file,
            pointer.len() as u64,
            Digest256::of_bytes(&pointer),
            deadline,
            cancel,
        )?;
        self.verify_layout()?;
        let selected_lock = tos_fd_open::open_regular_at(&self.root, Path::new(".admission.lock"))
            .map_err(invalid)?;
        if identity(&selected_lock)? != identity(&lock.0)? {
            return Err(invalid("corpus admission lock replaced before publication"));
        }
        active(deadline, cancel)?;
        rustix::fs::renameat(
            &self.root,
            pending.name.as_str(),
            &self.root,
            "current.json",
        )?;
        self.root.sync_all()?;
        self.verify_layout()?;
        if reader.select_current().map_err(invalid)?.map(|r| r.0) != Some(revision) {
            return Err(invalid(
                "accepted corpus pointer changed during publication",
            ));
        }
        active(deadline, cancel)
    }

    pub(crate) fn check_current(
        &self,
        expected: Option<Digest256>,
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        let reader = self.reader(limits)?;
        let _lock = self.lock(deadline, cancel)?;
        if reader.select_current().map_err(invalid)?.map(|r| r.0) != expected {
            return Err(invalid(
                "accepted base changed; re-admit against current revision",
            ));
        }
        self.verify_layout()?;
        active(deadline, cancel)
    }

    /// Called only after the selected validator identity matches the batch.
    /// Missing parents are created through held, no-follow directory handles.
    /// Namespace creation is not authorization for any source, rights or review transition.
    pub(crate) fn create(path: &Path, deadline: Instant, cancel: &AtomicBool) -> io::Result<Self> {
        active(deadline, cancel)?;
        if !path.is_absolute() {
            return Err(invalid("corpus store must be absolute"));
        }
        let parent_path = path
            .parent()
            .ok_or_else(|| invalid("corpus store parent absent"))?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalid("corpus store name absent"))?;
        let mut parent = tos_fd_open::open_absolute_directory(Path::new("/")).map_err(invalid)?;
        for component in parent_path.components() {
            active(deadline, cancel)?;
            match component {
                std::path::Component::RootDir => (),
                std::path::Component::Normal(name) => {
                    // Existing ancestors may belong to another owner (e.g. /srv).
                    // Only the actual mutable store namespaces require our uid.
                    match rustix::fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
                        Ok(()) => parent.sync_all()?,
                        Err(Errno::EXIST) => (),
                        Err(error) => return Err(error.into()),
                    }
                    parent = tos_fd_open::open_directory_at(&parent, Path::new(name))
                        .map_err(invalid)?;
                }
                _ => return Err(invalid("corpus store path must be normalized")),
            }
        }
        let root = directory(&parent, name)?;
        let objects = directory(&root, "objects")?;
        let revisions = directory(&root, "revisions")?;
        let staging = directory(&root, "staging")?;
        let store = Self {
            path: path.to_owned(),
            root,
            objects,
            revisions,
            staging,
        };
        store.verify_layout()?;
        active(deadline, cancel)?;
        Ok(store)
    }
    pub(crate) fn verify_layout(&self) -> io::Result<()> {
        let root =
            owned_directory(tos_fd_open::open_absolute_directory(&self.path).map_err(invalid)?)?;
        if identity(&root)? != identity(&self.root)? {
            return Err(invalid("corpus root replaced"));
        }
        for (name, held) in [
            ("objects", &self.objects),
            ("revisions", &self.revisions),
            ("staging", &self.staging),
        ] {
            let selected = owned_directory(
                tos_fd_open::open_directory_at(&root, Path::new(name)).map_err(invalid)?,
            )?;
            if identity(&selected)? != identity(held)? {
                return Err(invalid("corpus namespace replaced"));
            }
        }
        Ok(())
    }
    fn lock(&self, deadline: Instant, cancel: &AtomicBool) -> io::Result<AdmissionLock> {
        let file = File::from(rustix::fs::openat(
            &self.root,
            ".admission.lock",
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::from_raw_mode(0o600),
        )?);
        let m = file.metadata()?;
        if !m.is_file() || m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o077 != 0 {
            return Err(invalid("corpus admission lock ownership or mode differs"));
        }
        loop {
            active(deadline, cancel)?;
            match rustix::fs::flock(&file, FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(Errno::WOULDBLOCK) => std::thread::sleep(
                    Duration::from_millis(10)
                        .min(deadline.saturating_duration_since(Instant::now())),
                ),
                Err(error) => return Err(error.into()),
            }
        }
        let lock = AdmissionLock(file);
        self.verify_layout()?;
        let selected = tos_fd_open::open_regular_at(&self.root, Path::new(".admission.lock"))
            .map_err(invalid)?;
        if identity(&selected)? != identity(&lock.0)? {
            return Err(invalid("corpus admission lock replaced"));
        }
        active(deadline, cancel)?;
        Ok(lock)
    }
    /// The caller charges operation-wide bytes before invoking this stream.
    /// Both a reused object and a newly written object must match exact bytes.
    pub(crate) fn ingest(
        &self,
        source: &mut File,
        size: u64,
        digest: Digest256,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        active(deadline, cancel)?;
        let before = source.metadata()?;
        if !before.is_file() || before.len() != size {
            return Err(invalid("source update size/type differs"));
        }
        let name = digest.to_hex();
        match tos_fd_open::open_regular_at(&self.objects, Path::new(&name)) {
            Ok(mut existing) => {
                verify_file(source, size, digest, deadline, cancel)?;
                verify_file(&mut existing, size, digest, deadline, cancel)?;
                self.verify_layout()?;
                return active(deadline, cancel);
            }
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                ()
            }
            Err(error) => return Err(invalid(error)),
        }
        let mut temporary = Temporary::create(&self.staging)?;
        source.seek(SeekFrom::Start(0))?;
        let mut copied = 0u64;
        let mut hash = Digest256Hasher::new();
        let mut chunk = [0; 65536];
        loop {
            active(deadline, cancel)?;
            let n = source.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            copied = copied
                .checked_add(n as u64)
                .filter(|n| *n <= size)
                .ok_or_else(|| invalid("source grew during admission"))?;
            hash.update(&chunk[..n]);
            temporary.file.write_all(&chunk[..n])?;
        }
        let after = source.metadata()?;
        let stable = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if stable(&before) != stable(&after) || copied != size || hash.finalize() != digest {
            return Err(invalid("source changed or digest differs during admission"));
        }
        temporary
            .file
            .set_permissions(Permissions::from_mode(0o444))?;
        temporary.file.sync_all()?;
        verify_file(&mut temporary.file, size, digest, deadline, cancel)?;
        self.verify_layout()?;
        let name = digest.to_hex();
        match rustix::fs::linkat(
            &self.staging,
            temporary.name.as_str(),
            &self.objects,
            name.as_str(),
            AtFlags::empty(),
        ) {
            Ok(()) => (),
            Err(Errno::EXIST) => (),
            Err(error) => return Err(error.into()),
        }
        let mut installed =
            tos_fd_open::open_regular_at(&self.objects, Path::new(&name)).map_err(invalid)?;
        verify_file(&mut installed, size, digest, deadline, cancel)?;
        active(deadline, cancel)
    }
    /// Flush the complete object namespace once per batch, before validation
    /// or pointer publication. Individual newly installed objects are fsynced.
    pub(crate) fn sync_objects(&self, deadline: Instant, cancel: &AtomicBool) -> io::Result<()> {
        active(deadline, cancel)?;
        self.verify_layout()?;
        self.objects.sync_all()?;
        active(deadline, cancel)
    }
    pub(crate) fn copy_object(
        &self,
        digest: Digest256,
        size: u64,
        sink: &mut dyn Write,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        self.verify_layout()?;
        let mut file = tos_fd_open::open_regular_at(&self.objects, Path::new(&digest.to_hex()))
            .map_err(invalid)?;
        stream_verified(&mut file, size, digest, deadline, cancel, sink)?;
        self.verify_layout()
    }
    /// Retirement v1 binds the retired digest but omits its length. The
    /// selected held regular object supplies a bounded length for a subsequent
    /// complete digest verification; this is not a new manifest fact.
    pub(crate) fn object_size(
        &self,
        digest: Digest256,
        cap: u64,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<u64> {
        active(deadline, cancel)?;
        self.verify_layout()?;
        let file = tos_fd_open::open_regular_at(&self.objects, Path::new(&digest.to_hex()))
            .map_err(invalid)?;
        let size = file.metadata()?.len();
        if size > cap {
            return Err(invalid("retired object exceeds read bound"));
        }
        self.verify_layout()?;
        active(deadline, cancel)?;
        Ok(size)
    }
    pub(crate) fn verify_object(
        &self,
        digest: Digest256,
        size: u64,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        self.verify_layout()?;
        let mut file = tos_fd_open::open_regular_at(&self.objects, Path::new(&digest.to_hex()))
            .map_err(invalid)?;
        verify_file(&mut file, size, digest, deadline, cancel)?;
        self.verify_layout()
    }
    pub(crate) fn read_object(
        &self,
        digest: Digest256,
        size: u64,
        cap: usize,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Vec<u8>> {
        if size > cap as u64 {
            return Err(invalid("candidate object exceeds selected read bound"));
        }
        active(deadline, cancel)?;
        self.verify_layout()?;
        let mut file = tos_fd_open::open_regular_at(&self.objects, Path::new(&digest.to_hex()))
            .map_err(invalid)?;
        let mut bytes = Vec::with_capacity(size as usize);
        stream_verified(&mut file, size, digest, deadline, cancel, &mut bytes)?;
        self.verify_layout()?;
        Ok(bytes)
    }
}

fn stream_verified(
    file: &mut File,
    size: u64,
    digest: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
    sink: &mut (impl Write + ?Sized),
) -> io::Result<()> {
    let before = file.metadata()?;
    if !before.is_file() || before.len() != size {
        return Err(invalid("corpus object size/type differs"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut count = 0u64;
    let mut hash = Digest256Hasher::new();
    let mut bytes = [0; 65536];
    loop {
        active(deadline, cancel)?;
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .filter(|n| *n <= size)
            .ok_or_else(|| invalid("corpus object grew"))?;
        hash.update(&bytes[..n]);
        sink.write_all(&bytes[..n])?;
    }
    let after = file.metadata()?;
    if count != size
        || hash.finalize() != digest
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(invalid("corpus object fixity differs"));
    }
    active(deadline, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tos_foundation::{JsonLimits, SourceRevision};
    use tos_source_store::Selector;

    // Physical transaction control only. These fixtures intentionally do not
    // impersonate the complete source validator or its semantic admission.
    fn manifest(
        validator: &str,
        digest: Digest256,
        base: Option<Digest256>,
        json: JsonLimits,
    ) -> (Digest256, Vec<u8>) {
        let mut value = serde_json::json!({"schema_version":"tos_corpus_snapshot_v1",
            "base_revision":base.map(|r|r.to_hex()),"validator_sha256":Digest256::of_bytes(validator.as_bytes()).to_hex(),
            "files":[{"path":"ToS/control.txt","sha256":digest.to_hex(),"size_bytes":3,"mode":420}],
            "identities":{},"dependencies":{},"retirements":[]});
        let revision = Digest256::of_bytes(
            &crate::source_admission_candidate::canonical(&value, json).unwrap(),
        );
        value["revision"] = revision.to_hex().into();
        (
            revision,
            crate::source_admission_candidate::canonical(&value, json).unwrap(),
        )
    }

    #[test]
    fn immutable_objects_and_cas_preserve_selected_reader_on_rejection() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("new-parent/store");
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let store = AdmissionStore::create(&path, deadline, &cancel).unwrap();
        let json = JsonLimits::new(16384, 32, 4096, 4300).unwrap();
        let limits = ReadLimits {
            max_manifest_bytes: 16384,
            max_manifest_entries: 128,
            max_selected_object_bytes: 1024,
            json,
        };
        let digest = Digest256::of_bytes(b"abc");
        let input_path = temporary.path().join("input");
        std::fs::write(&input_path, b"abc").unwrap();
        let mut input = File::open(&input_path).unwrap();
        store
            .ingest(&mut input, 3, digest, deadline, &cancel)
            .unwrap();
        store.sync_objects(deadline, &cancel).unwrap();
        let (first, raw) = manifest("one", digest, None, json);
        store
            .publish(None, first, &raw, limits, deadline, &cancel, &|_| Ok(()))
            .unwrap();
        let reader = store.reader(limits).unwrap();
        let selected = reader.load_exact(SourceRevision(first)).unwrap();
        let member = tos_foundation::RelativePath::parse("ToS/control.txt").unwrap();
        let descriptor = reader.resolve(&selected, Selector::Path(&member)).unwrap();
        let mut copied = Cursor::new(Vec::new());
        reader
            .read_selected(&selected, &descriptor, 3, &mut copied)
            .unwrap();
        assert_eq!(copied.into_inner(), b"abc");

        let (stale, raw) = manifest("two", digest, None, json);
        assert!(
            store
                .publish(None, stale, &raw, limits, deadline, &cancel, &|_| Ok(()))
                .is_err()
        );
        assert_eq!(
            reader.select_current().unwrap(),
            Some(SourceRevision(first))
        );
        assert!(
            std::fs::read_dir(path.join("staging"))
                .unwrap()
                .next()
                .is_none()
        );

        std::fs::write(&input_path, b"bad").unwrap();
        let mut input = File::open(&input_path).unwrap();
        assert!(
            store
                .ingest(&mut input, 3, digest, deadline, &cancel)
                .is_err()
        );
        assert_eq!(
            store.read_object(digest, 3, 3, deadline, &cancel).unwrap(),
            b"abc"
        );

        // Closing just one clone does not release an OFD lock; dropping the
        // owned lease must explicitly unlock even while a clone remains live.
        let lock = store.lock(deadline, &cancel).unwrap();
        let inherited = lock.0.try_clone().unwrap();
        drop(lock);
        let next = store.lock(deadline, &cancel).unwrap();
        drop(inherited);
        drop(next);
        store
            .check_current(Some(first), limits, deadline, &cancel)
            .unwrap();
    }
}
fn verify_file(
    file: &mut File,
    size: u64,
    digest: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    stream_verified(file, size, digest, deadline, cancel, &mut io::sink())
}
