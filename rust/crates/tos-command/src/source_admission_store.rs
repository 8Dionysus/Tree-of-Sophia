//! Physical custody for the maintained immutable corpus store.
//! Installing bytes is not admission. Only the complete native validator's
//! caller may perform the accepted-pointer compare-and-swap.
use super::source_admission::{active, invalid};
use rustix::fs::{AtFlags, FlockOperation, Mode, OFlags, RenameFlags, openat};
use rustix::io::Errno;
use std::{
    cell::RefCell,
    fs::{File, Permissions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_segment_store::AuthenticatedTreeIoLedgerV1;
use tos_source_store::{CorpusReader, ReadLimits};

/// A streamed pointer rename succeeded, but a required post-rename durability
/// or custody fence refused. The immutable revision identity and its original
/// allocation reservation remain available to the caller for exact recovery.
pub(crate) struct StreamedPublicationCommittedRefusal {
    pub(crate) revision: Digest256,
    pub(crate) manifest_sha256: Digest256,
    pub(crate) rootset_sha256: Option<Digest256>,
    pub(crate) custody: Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    cause: io::Error,
}

impl std::fmt::Debug for StreamedPublicationCommittedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamedPublicationCommittedRefusal")
            .field("revision", &self.revision.to_hex())
            .field("manifest_sha256", &self.manifest_sha256.to_hex())
            .field(
                "rootset_sha256",
                &self.rootset_sha256.map(Digest256::to_hex),
            )
            .field("custody_retained", &self.custody.is_some())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for StreamedPublicationCommittedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("streamed revision committed before a post-rename refusal")
    }
}

impl std::error::Error for StreamedPublicationCommittedRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Actual caller-owned strict decoder main and SAME invocation resources.
/// The selected native validator/index remains a separate publication prerequisite.
pub(crate) struct StreamedPublicationRead {
    pub limits: tos_source_store::StreamedCutReadLimitsV1,
    pub index_file: File,
    pub io_budget: tos_source_store::PinnedSqliteIoBudget,
    pub space_budget: tos_source_store::PinnedSqliteSpaceBudget,
    pub max_index_allocated_bytes: u64,
    pub max_manifest_allocated_bytes: u64,
    pub cancelled: Arc<AtomicBool>,
}

struct SnapshotStage<'a> {
    parent: &'a File,
    name: String,
    directory: File,
    published: bool,
    allocation: Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>,
    v2_rootset: bool,
}
impl Drop for SnapshotStage<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if let Ok(current) = tos_fd_open::open_directory_at(self.parent, Path::new(&self.name)) {
            if matches!((identity(&current), identity(&self.directory)), (Ok(a), Ok(b)) if a == b) {
                if rustix::fs::unlinkat(&self.directory, "snapshot.json", AtFlags::empty()).is_ok()
                {
                    // All locally owned snapshot FDs were declared after this
                    // guard and closed before it. Verified unlink ends this
                    // unpublished backing, not the reservation's lifetime.
                    if let Some(allocation) = &self.allocation {
                        let _ = allocation.update_actual_allocated(0);
                    }
                }
                if self.v2_rootset {
                    let _ =
                        rustix::fs::unlinkat(&self.directory, "rootset-v2.json", AtFlags::empty());
                }
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
fn directory_new(parent: &File, name: &str) -> io::Result<File> {
    rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700))?;
    parent.sync_all()?;
    owned_directory(tos_fd_open::open_directory_at(parent, Path::new(name)).map_err(invalid)?)
}
fn optional_directory(parent: &File, name: &str) -> io::Result<Option<File>> {
    match openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => owned_directory(File::from(fd)).map(Some),
        Err(Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
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

struct V2ObjectPrecharge {
    accountant: Arc<super::source_admission_segment_v2::NativeV2TreeIo>,
    reserved: u64,
    linked: bool,
}
impl Drop for V2ObjectPrecharge {
    fn drop(&mut self) {
        if !self.linked {
            let _ = self.accountant.release_file_allocation(self.reserved);
        }
    }
}

struct V2NamespacePrecharge {
    accountant: Arc<super::source_admission_segment_v2::NativeV2TreeIo>,
    reserved: u64,
    created: bool,
}
impl Drop for V2NamespacePrecharge {
    fn drop(&mut self) {
        if !self.created {
            let _ = self.accountant.release_file_allocation(self.reserved);
        }
    }
}

fn allocated_bytes(file: &File, reason: &'static str) -> io::Result<u64> {
    file.metadata()?
        .blocks()
        .checked_mul(512)
        .ok_or_else(|| invalid(reason))
}

fn partial_v2_namespace_allocation(
    parent: &File,
    name: &str,
    parent_before: u64,
) -> io::Result<Option<u64>> {
    let root = match tos_fd_open::open_directory_at(parent, Path::new(name)) {
        Ok(root) => owned_directory(root)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut allocated = allocated_bytes(&root, "V2 store directory allocation overflow")?;
    for child in ["objects", "revisions", "staging"] {
        match tos_fd_open::open_directory_at(&root, Path::new(child)) {
            Ok(directory) => {
                let directory = owned_directory(directory)?;
                allocated = allocated
                    .checked_add(allocated_bytes(
                        &directory,
                        "V2 store child-directory allocation overflow",
                    )?)
                    .ok_or_else(|| invalid("V2 store namespace allocation overflow"))?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    let parent_after = allocated_bytes(parent, "V2 store parent allocation overflow")?;
    allocated
        .checked_add(parent_after.saturating_sub(parent_before))
        .map(Some)
        .ok_or_else(|| invalid("V2 store namespace allocation overflow"))
}

pub(crate) struct AdmissionLock(File);
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
    // Created only by an explicit native V2 selection and retained as the
    // exact physical root for all authenticated current/history trees.
    v2_segments: RefCell<Option<File>>,
    // Never released at rename: the invocation and its returned custody retain
    // surviving named allocation until an explicit terminal/baseline handoff.
    streamed_manifest_custody: RefCell<Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>>,
    // V2 physical bytes remain attached to this exact private store when a
    // later source fence refuses or publication needs forward recovery.
    v2_store_custody: RefCell<Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>>>,
    // Attached by the protected native V2 invocation before immutable object
    // ingestion; object files and later trees share one source allocation cap.
    v2_allocation_accountant:
        RefCell<Option<Arc<super::source_admission_segment_v2::NativeV2TreeIo>>>,
}
impl AdmissionStore {
    pub(crate) fn streamed_manifest_custody(
        &self,
    ) -> io::Result<Arc<tos_source_store::PinnedSqliteSpaceReservation>> {
        self.streamed_manifest_custody
            .borrow()
            .clone()
            .ok_or_else(|| invalid("streamed publication custody absent"))
    }
    pub(crate) fn retain_v2_store_custody(
        &self,
        custody: Arc<tos_source_store::PinnedSqliteSpaceReservation>,
    ) {
        *self.v2_store_custody.borrow_mut() = Some(custody);
    }
    pub(crate) fn attach_v2_allocation_accountant(
        &self,
        accountant: Arc<super::source_admission_segment_v2::NativeV2TreeIo>,
    ) -> io::Result<()> {
        self.verify_layout()?;
        if self.v2_allocation_accountant.borrow().is_some() {
            return Err(invalid("V2 allocation accountant already attached"));
        }
        let unit = accountant.selected_allocation_unit_bytes();
        for directory in [&self.root, &self.objects, &self.revisions, &self.staging] {
            if directory.metadata()?.blksize() > unit {
                return Err(invalid(
                    "V2 selected allocation quantum is below held store geometry",
                ));
            }
        }
        self.retain_v2_store_custody(accountant.custody_reservation());
        *self.v2_allocation_accountant.borrow_mut() = Some(accountant);
        Ok(())
    }
    pub(crate) fn has_v2_allocation_accountant(
        &self,
        accountant: &Arc<super::source_admission_segment_v2::NativeV2TreeIo>,
    ) -> bool {
        self.v2_allocation_accountant
            .borrow()
            .as_ref()
            .is_some_and(|attached| Arc::ptr_eq(attached, accountant))
    }
    pub(crate) fn v2_store_custody(
        &self,
    ) -> Option<Arc<tos_source_store::PinnedSqliteSpaceReservation>> {
        self.v2_store_custody.borrow().clone()
    }
    pub(crate) fn has_v2_segments(&self) -> io::Result<bool> {
        self.verify_layout()?;
        Ok(self.v2_segments.borrow().is_some())
    }
    pub(crate) fn lock_for_v2_publication(
        &self,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<AdmissionLock> {
        self.lock(deadline, cancel)
    }
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
        let v2_segments = optional_directory(&root, "segments-v2")?;
        let store = Self {
            path: path.to_owned(),
            root,
            objects,
            revisions,
            staging,
            v2_segments: RefCell::new(v2_segments),
            streamed_manifest_custody: RefCell::new(None),
            v2_store_custody: RefCell::new(None),
            v2_allocation_accountant: RefCell::new(None),
        };
        store.verify_layout()?;
        active(deadline, cancel)?;
        Ok(store)
    }
    /// Borrowed physical namespaces for the existing restore lane's independent
    /// retained-closure backup. This supplies no mutation/admission witness;
    /// the caller must preserve exact bindings and recheck layout on completion.
    pub(crate) fn backup_namespaces(&self) -> io::Result<(&File, &File, &File)> {
        self.verify_layout()?;
        Ok((&self.root, &self.objects, &self.revisions))
    }
    pub(crate) fn reader(&self, limits: ReadLimits) -> io::Result<CorpusReader> {
        self.verify_layout()?;
        let reader = CorpusReader::open_existing(&self.path, limits).map_err(invalid)?;
        self.verify_layout()?;
        Ok(reader)
    }

    pub(crate) fn current_selection(
        &self,
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        io_budget: Option<&tos_source_store::PinnedSqliteIoBudget>,
    ) -> io::Result<Option<tos_source_store::CorpusCurrentSelection>> {
        let reader = self.reader(limits)?;
        let selection = match io_budget {
            Some(io) => reader.select_current_selection_budgeted(io, deadline, cancel),
            None => reader.select_current_selection(),
        }
        .map_err(invalid)?;
        active(deadline, cancel)?;
        self.verify_layout()?;
        Ok(selection)
    }

    /// Read the exact immutable rootset selected by a V2 pointer. The held
    /// revision directory, digest fence and EOF check bind these bytes to the
    /// one current selector without interpreting their source meaning.
    pub(crate) fn read_v2_rootset(
        &self,
        revision: Digest256,
        expected_sha256: Digest256,
        max_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
        io_budget: &tos_source_store::PinnedSqliteIoBudget,
    ) -> io::Result<Vec<u8>> {
        active(deadline, cancel)?;
        if max_bytes == 0 || max_bytes == usize::MAX {
            return Err(invalid("V2 rootset read profile is invalid"));
        }
        self.verify_layout()?;
        let directory =
            tos_fd_open::open_directory_at(&self.revisions, Path::new(&revision.to_hex()))
                .map_err(invalid)?;
        let mut file = tos_fd_open::open_regular_at(&directory, Path::new("rootset-v2.json"))
            .map_err(invalid)?;
        let before = file.metadata()?;
        if !before.is_file()
            || before.uid() != rustix::process::geteuid().as_raw()
            || before.mode() & 0o222 != 0
            || before.len() == 0
            || before.len() > max_bytes as u64
        {
            return Err(invalid("selected V2 rootset file custody differs"));
        }
        let stamp = |metadata: &std::fs::Metadata| {
            (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mode(),
                metadata.uid(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        };
        let count = usize::try_from(before.len())
            .map_err(|_| invalid("selected V2 rootset exceeds address space"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(count)
            .map_err(|_| invalid("selected V2 rootset allocation failed"))?;
        let mut block = [0u8; 8192];
        while bytes.len() < count {
            active(deadline, cancel)?;
            let wanted = (count - bytes.len()).min(block.len());
            io_budget.charge_read(wanted as u64).map_err(invalid)?;
            match file.read(&mut block[..wanted]) {
                Ok(0) => return Err(invalid("selected V2 rootset ended before its size")),
                Ok(read) => {
                    io_budget
                        .record_read_returned(read as u64)
                        .map_err(invalid)?;
                    bytes.extend_from_slice(&block[..read]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        active(deadline, cancel)?;
        io_budget.charge_read(1).map_err(invalid)?;
        let mut tail = [0u8; 1];
        let tail_read = file.read(&mut tail)?;
        io_budget
            .record_read_returned(tail_read as u64)
            .map_err(invalid)?;
        let after = file.metadata()?;
        let selected = tos_fd_open::open_regular_at(&directory, Path::new("rootset-v2.json"))
            .map_err(invalid)?;
        if tail_read != 0
            || stamp(&after) != stamp(&before)
            || stamp(&selected.metadata()?) != stamp(&before)
            || Digest256::of_bytes(&bytes) != expected_sha256
        {
            return Err(invalid("selected V2 rootset digest or EOF differs"));
        }
        active(deadline, cancel)?;
        self.verify_layout()?;
        Ok(bytes)
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
        self.publish_manifest(
            expected_base,
            revision,
            manifest.len() as u64,
            Digest256::of_bytes(manifest),
            limits,
            None,
            None,
            None,
            deadline,
            cancel,
            charge_read,
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
            |file, _, _| {
                for block in manifest.chunks(65536) {
                    active(deadline, cancel)?;
                    file.write_all(block)?;
                }
                Ok(())
            },
        )
    }

    /// Additive physical publication route for a fully validated native candidate.
    /// This consumes a held canonical manifest FD and a distinct private index FD;
    /// neither FD, the decoded index, nor this mechanical receipt grants source
    /// admission. The owning native caller must finish the same complete source
    /// validation/index/history construction before invoking this method.
    ///
    /// The resident API above keeps its original limits and parser. This route
    /// uses the existing streamed V1 decoder and the SAME staging/pointer CAS.
    pub(crate) fn publish_streamed(
        &self,
        expected_base: Option<Digest256>,
        revision: Digest256,
        manifest: File,
        manifest_bytes: u64,
        manifest_sha256: Digest256,
        limits: ReadLimits,
        streamed: StreamedPublicationRead,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        self.publish_streamed_inner(
            expected_base,
            revision,
            manifest,
            manifest_bytes,
            manifest_sha256,
            limits,
            streamed,
            None,
            None,
            deadline,
            cancel,
        )
    }

    pub(crate) fn publish_streamed_v2_initial(
        &self,
        expected_base: Option<Digest256>,
        revision: Digest256,
        manifest: File,
        manifest_bytes: u64,
        manifest_sha256: Digest256,
        limits: ReadLimits,
        streamed: StreamedPublicationRead,
        rootset: super::source_admission_segment_v2::BuiltInitialRootSetV2,
        lock: &AdmissionLock,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        if expected_base.is_some()
            || rootset.roots.current.revision.0 != revision
            || rootset.roots.current.base_revision
                != expected_base.map(tos_foundation::SourceRevision)
            || Digest256::of_bytes(&rootset.bytes) != rootset.sha256
        {
            return Err(invalid("initial V2 publication rootset binding differs"));
        }
        self.publish_streamed_inner(
            expected_base,
            revision,
            manifest,
            manifest_bytes,
            manifest_sha256,
            limits,
            streamed,
            Some(rootset),
            Some(lock),
            deadline,
            cancel,
        )
    }

    fn publish_streamed_inner(
        &self,
        expected_base: Option<Digest256>,
        revision: Digest256,
        mut manifest: File,
        manifest_bytes: u64,
        manifest_sha256: Digest256,
        limits: ReadLimits,
        streamed: StreamedPublicationRead,
        rootset: Option<super::source_admission_segment_v2::BuiltInitialRootSetV2>,
        prelocked: Option<&AdmissionLock>,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        if !std::ptr::eq(cancel, streamed.cancelled.as_ref()) {
            return Err(invalid("streamed publication cancellation differs"));
        }
        let io = streamed.io_budget.clone();
        let charge_read = &|n| io.charge_read(n).map_err(invalid);
        let charge_write = &|n| io.charge_write(n).map_err(invalid);
        let record_read = &|n| io.record_read_returned(n).map_err(invalid);
        let record_write = &|n| io.record_write_returned(n).map_err(invalid);
        let limits = limits.validate().map_err(invalid)?;
        if manifest_bytes == 0 || manifest_bytes > limits.max_manifest_bytes as u64 {
            return Err(invalid("corpus manifest byte bound exceeded"));
        }
        if streamed.max_manifest_allocated_bytes < manifest_bytes
            || streamed.max_manifest_allocated_bytes == u64::MAX
            || (rootset.is_none() && self.streamed_manifest_custody.borrow().is_some())
            || (rootset.is_some() && prelocked.is_none())
        {
            return Err(invalid(
                "streamed persistent manifest custody/profile differs",
            ));
        }
        if let Some(rootset) = &rootset {
            let custody = rootset.tree_io.custody_reservation();
            self.retain_v2_store_custody(Arc::clone(&custody));
        } else {
            let custody = Arc::new(
                streamed
                    .space_budget
                    .reserve(streamed.max_manifest_allocated_bytes)
                    .map_err(invalid)?,
            );
            *self.streamed_manifest_custody.borrow_mut() = Some(custody.clone());
        }
        active(deadline, cancel)?;
        let before = manifest.metadata()?;
        if !before.is_file()
            || before.uid() != rustix::process::geteuid().as_raw()
            || before.mode() & 0o022 != 0
            || before.len() != manifest_bytes
            || identity(&manifest)? == identity(&streamed.index_file)?
        {
            return Err(invalid("streamed candidate manifest FD custody differs"));
        }
        let stamp = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mode(),
                m.uid(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        verify_file_recorded(
            &mut manifest,
            manifest_bytes,
            manifest_sha256,
            deadline,
            cancel,
            charge_read,
            record_read,
        )?;
        if stamp(&manifest.metadata()?) != stamp(&before) {
            return Err(invalid(
                "streamed candidate manifest changed during verification",
            ));
        }
        self.publish_manifest(
            expected_base,
            revision,
            manifest_bytes,
            manifest_sha256,
            limits,
            Some(streamed),
            rootset.as_ref(),
            prelocked,
            deadline,
            cancel,
            charge_read,
            charge_write,
            record_read,
            record_write,
            |file, stage_write_charge, staged_write_returned| {
                manifest.seek(SeekFrom::Start(0))?;
                let mut remaining = manifest_bytes;
                let mut block = [0u8; 65536];
                while remaining != 0 {
                    active(deadline, cancel)?;
                    let count = remaining.min(block.len() as u64) as usize;
                    let mut filled = 0;
                    while filled < count {
                        active(deadline, cancel)?;
                        charge_read((count - filled) as u64)?;
                        match manifest.read(&mut block[filled..count]) {
                            Ok(0) => return Err(invalid("candidate manifest ended during copy")),
                            Ok(n) => {
                                record_read(n as u64)?;
                                filled += n;
                            }
                            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                            Err(e) => return Err(e),
                        }
                        active(deadline, cancel)?;
                    }
                    write_recorded(
                        file,
                        &block[..count],
                        deadline,
                        cancel,
                        stage_write_charge,
                        staged_write_returned,
                    )?;
                    remaining -= count as u64;
                }
                active(deadline, cancel)?;
                charge_read(1)?;
                let tail = manifest.read(&mut block[..1])?;
                record_read(tail as u64)?;
                if tail != 0 || stamp(&manifest.metadata()?) != stamp(&before) {
                    return Err(invalid("streamed candidate manifest changed during copy"));
                }
                active(deadline, cancel)
            },
        )?;
        Ok(())
    }

    fn publish_manifest(
        &self,
        expected_base: Option<Digest256>,
        revision: Digest256,
        manifest_bytes: u64,
        manifest_sha256: Digest256,
        limits: ReadLimits,
        streamed: Option<StreamedPublicationRead>,
        v2_rootset: Option<&super::source_admission_segment_v2::BuiltInitialRootSetV2>,
        prelocked: Option<&AdmissionLock>,
        deadline: Instant,
        cancel: &AtomicBool,
        charge_read: &dyn Fn(u64) -> io::Result<()>,
        charge_write: &dyn Fn(u64) -> io::Result<()>,
        record_read: &dyn Fn(u64) -> io::Result<()>,
        record_write: &dyn Fn(u64) -> io::Result<()>,
        emit: impl FnOnce(
            &mut File,
            &dyn Fn(u64) -> io::Result<()>,
            &dyn Fn(u64) -> io::Result<()>,
        ) -> io::Result<()>,
    ) -> io::Result<()> {
        let physical = streamed.is_some();
        let pointer_io = v2_rootset
            .map(|rootset| rootset.tree_io.io_budget().clone())
            .or_else(|| streamed.as_ref().map(|r| r.io_budget.clone()));
        let persistent_charge_read = |n| {
            if let Some(rootset) = v2_rootset {
                if rootset.tree_io.charge_read(n) {
                    Ok(())
                } else {
                    Err(invalid("V2 persistent read allowance refused"))
                }
            } else if physical {
                charge_read(n)
            } else {
                Ok(())
            }
        };
        let persistent_record_read = |n| {
            if let Some(rootset) = v2_rootset {
                if rootset.tree_io.record_read_returned(n) {
                    Ok(())
                } else {
                    Err(invalid("V2 persistent read return refused"))
                }
            } else if physical {
                record_read(n)
            } else {
                Ok(())
            }
        };
        let persistent_charge_write = |n| {
            if let Some(rootset) = v2_rootset {
                if rootset.tree_io.charge_write(n) {
                    Ok(())
                } else {
                    Err(invalid("V2 persistent write allowance refused"))
                }
            } else if physical {
                charge_write(n)
            } else {
                Ok(())
            }
        };
        let persistent_record_write = |n| {
            if let Some(rootset) = v2_rootset {
                if rootset.tree_io.record_write_returned(n) {
                    Ok(())
                } else {
                    Err(invalid("V2 persistent write return refused"))
                }
            } else if physical {
                record_write(n)
            } else {
                Ok(())
            }
        };
        let verify = |file: &mut File, size, digest| {
            if v2_rootset.is_some() {
                verify_file_recorded(
                    file,
                    size,
                    digest,
                    deadline,
                    cancel,
                    &persistent_charge_read,
                    &persistent_record_read,
                )
            } else if physical {
                verify_file_recorded(
                    file,
                    size,
                    digest,
                    deadline,
                    cancel,
                    charge_read,
                    record_read,
                )
            } else {
                verify_file(file, size, digest, deadline, cancel)
            }
        };
        let limits = limits.validate().map_err(invalid)?;
        active(deadline, cancel)?;
        if manifest_bytes == 0 || manifest_bytes > limits.max_manifest_bytes as u64 {
            return Err(invalid("corpus manifest byte bound exceeded"));
        }
        self.verify_layout()?;
        if let Some(rootset) = v2_rootset {
            if prelocked.is_none()
                || expected_base.is_some()
                || rootset.roots.current.revision.0 != revision
                || rootset.roots.current.base_revision
                    != expected_base.map(tos_foundation::SourceRevision)
                || rootset.roots.current.manifest_sha256 != manifest_sha256
                || rootset.bytes.len() > 65_536
                || rootset.roots.encode()?.as_slice() != rootset.bytes
                || Digest256::of_bytes(&rootset.bytes) != rootset.sha256
                || rootset.segment_store.custody_domain()
                    != super::source_admission_segment_v2::SOURCE_ADMISSION_V2_DOMAIN
            {
                return Err(invalid("V2 rootset or admission binding differs"));
            }
            rootset.roots.validate_store_binding(
                rootset.segment_store.store_id(),
                rootset.segment_store.domain_digest(),
            )?;
            let held = self.v2_segments.borrow();
            let held = held
                .as_ref()
                .ok_or_else(|| invalid("V2 segment namespace is not selected"))?;
            if identity(held)?
                != rootset
                    .segment_store
                    .physical_root_identity()
                    .map_err(invalid)?
            {
                return Err(invalid("V2 segment store physical root differs"));
            }
            let selected_lock =
                tos_fd_open::open_regular_at(&self.root, Path::new(".admission.lock"))
                    .map_err(invalid)?;
            let lock = prelocked.ok_or_else(|| invalid("V2 writer lock is absent"))?;
            if identity(&selected_lock)? != identity(&lock.0)? {
                return Err(invalid("V2 writer lock replaced before staging"));
            }
        }
        let reader = self.reader(limits)?;
        let selected_before_write = match &pointer_io {
            Some(io) => reader.select_current_selection_budgeted(io, deadline, cancel),
            None => reader.select_current_selection(),
        }
        .map_err(invalid)?;
        if v2_rootset.is_some() && selected_before_write.is_some() {
            return Err(invalid(
                "initial V2 writer requires an unselected empty store",
            ));
        }
        if v2_rootset.is_none()
            && selected_before_write.is_some_and(|selection| {
                selection.format == tos_source_store::CorpusPointerFormat::V2
            })
        {
            return Err(invalid("V1 writer cannot advance a V2-selected corpus"));
        }
        let v2_stage_reservation = if let Some(rootset) = v2_rootset {
            let file_bytes = manifest_bytes
                .checked_add(rootset.bytes.len() as u64)
                .ok_or_else(|| invalid("V2 staged-file allocation bound overflow"))?;
            Some((
                rootset.tree_io.reserve_file_allocation(file_bytes)?,
                rootset.tree_io.reserve_file_allocation(0)?,
            ))
        } else {
            None
        };
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
            allocation: if physical && v2_rootset.is_none() {
                self.streamed_manifest_custody.borrow().clone()
            } else {
                None
            },
            v2_rootset: v2_rootset.is_some(),
        };
        let mut file = File::from(rustix::fs::openat(
            &stage.directory,
            "snapshot.json",
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        let allocation_observer = file.try_clone()?;
        let persistent = self.streamed_manifest_custody.borrow().clone();
        let staged_write_returned = |n| {
            persistent_record_write(n)?;
            if physical && v2_rootset.is_none() {
                let allocated = allocation_observer
                    .metadata()?
                    .blocks()
                    .checked_mul(512)
                    .ok_or_else(|| invalid("staged manifest allocated bytes overflow"))?;
                persistent
                    .as_ref()
                    .ok_or_else(|| invalid("staged manifest custody absent"))?
                    .update_actual_allocated(allocated)
                    .map_err(invalid)?;
            }
            active(deadline, cancel)
        };
        emit(&mut file, &persistent_charge_write, &staged_write_returned)?;
        file.set_permissions(Permissions::from_mode(0o444))?;
        file.sync_all()?;
        if physical && v2_rootset.is_none() {
            let allocated = file
                .metadata()?
                .blocks()
                .checked_mul(512)
                .ok_or_else(|| invalid("staged manifest allocated bytes overflow"))?;
            persistent
                .as_ref()
                .ok_or_else(|| invalid("staged manifest custody absent"))?
                .update_actual_allocated(allocated)
                .map_err(invalid)?;
        }
        verify(&mut file, manifest_bytes, manifest_sha256)?;
        if let Some(rootset) = v2_rootset {
            let mut rootset_file = File::from(rustix::fs::openat(
                &stage.directory,
                "rootset-v2.json",
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )?);
            write_recorded(
                &mut rootset_file,
                &rootset.bytes,
                deadline,
                cancel,
                &persistent_charge_write,
                &persistent_record_write,
            )?;
            rootset_file.set_permissions(Permissions::from_mode(0o444))?;
            rootset_file.sync_all()?;
            verify(
                &mut rootset_file,
                rootset.bytes.len() as u64,
                rootset.sha256,
            )?;
        }
        stage.directory.sync_all()?;
        self.verify_layout()?;
        let revisions_before = self
            .revisions
            .metadata()?
            .blocks()
            .checked_mul(512)
            .ok_or_else(|| invalid("V2 revision-directory allocation overflow"))?;
        let existed = match rustix::fs::renameat_with(
            &self.staging,
            stage.name.as_str(),
            &self.revisions,
            name.as_str(),
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {
                stage.published = true;
                false
            }
            Err(Errno::EXIST) => true,
            Err(error) => return Err(error.into()),
        };
        if v2_rootset.is_some() && existed {
            return Err(invalid("initial V2 immutable revision already exists"));
        }
        if let (Some(rootset), Some((file_reservation, directory_reservation))) =
            (v2_rootset, v2_stage_reservation)
        {
            let snapshot =
                tos_fd_open::open_regular_at(&stage.directory, Path::new("snapshot.json"))
                    .map_err(invalid)?;
            let rootset_file =
                tos_fd_open::open_regular_at(&stage.directory, Path::new("rootset-v2.json"))
                    .map_err(invalid)?;
            let file_bytes = snapshot
                .metadata()?
                .blocks()
                .checked_add(rootset_file.metadata()?.blocks())
                .and_then(|blocks| blocks.checked_mul(512))
                .ok_or_else(|| invalid("V2 revision-file allocation overflow"))?;
            let revisions_after = self
                .revisions
                .metadata()?
                .blocks()
                .checked_mul(512)
                .ok_or_else(|| invalid("V2 revision-directory allocation overflow"))?;
            let directory_bytes = stage
                .directory
                .metadata()?
                .blocks()
                .checked_mul(512)
                .and_then(|bytes| {
                    bytes.checked_add(revisions_after.saturating_sub(revisions_before))
                })
                .ok_or_else(|| invalid("V2 revision-directory allocation overflow"))?;
            rootset
                .tree_io
                .reconcile_file_allocation(file_reservation, file_bytes)?;
            rootset
                .tree_io
                .reconcile_file_allocation(directory_reservation, directory_bytes)?;
        }
        if !existed {
            self.revisions.sync_all()?;
            self.staging.sync_all()?;
        }
        let directory =
            tos_fd_open::open_directory_at(&self.revisions, Path::new(&name)).map_err(invalid)?;
        let mut installed = tos_fd_open::open_regular_at(&directory, Path::new("snapshot.json"))
            .map_err(invalid)?;
        verify(&mut installed, manifest_bytes, manifest_sha256)?;
        if let Some(rootset) = v2_rootset {
            let mut installed_rootset =
                tos_fd_open::open_regular_at(&directory, Path::new("rootset-v2.json"))
                    .map_err(invalid)?;
            verify(
                &mut installed_rootset,
                rootset.bytes.len() as u64,
                rootset.sha256,
            )?;
        }
        if let Some(streamed) = streamed {
            let cut = reader
                .open_source_cut_streamed_budgeted(
                    tos_foundation::SourceRevision(revision),
                    streamed.limits,
                    streamed.index_file,
                    streamed.io_budget,
                    streamed.space_budget,
                    streamed.max_index_allocated_bytes,
                    deadline,
                    streamed.cancelled,
                )
                .map_err(invalid)?;
            let selected = cut
                .revision(tos_foundation::SourceRevision(revision))
                .map_err(invalid)?
                .ok_or_else(|| invalid("streamed revision absent"))?;
            if selected.base_revision.map(|r| r.0) != expected_base {
                return Err(invalid("corpus manifest base differs from transaction"));
            }
            if existed {
                let mut after = None;
                while let Some(member) = cut
                    .member_after(tos_foundation::SourceRevision(revision), after.as_ref())
                    .map_err(invalid)?
                {
                    active(deadline, cancel)?;
                    self.verify_object_accounted(
                        member.sha256,
                        member.size_bytes,
                        deadline,
                        cancel,
                        charge_read,
                        &|_| Ok(()),
                        record_read,
                        &|_| Ok(()),
                    )?;
                    after = Some(member.path);
                }
                for ordinal in 0..selected.retirement_count {
                    active(deadline, cancel)?;
                    let event = cut
                        .retirement_at(tos_foundation::SourceRevision(revision), ordinal)
                        .map_err(invalid)?
                        .ok_or_else(|| invalid("streamed retirement absent"))?;
                    let mut old = tos_fd_open::open_regular_at(
                        &self.objects,
                        Path::new(&event.sha256.to_hex()),
                    )
                    .map_err(invalid)?;
                    let size = old.metadata()?.len();
                    if size > limits.max_selected_object_bytes {
                        return Err(invalid("retired object exceeds read bound"));
                    }
                    verify_file_recorded(
                        &mut old,
                        size,
                        event.sha256,
                        deadline,
                        cancel,
                        charge_read,
                        record_read,
                    )?;
                    self.verify_object_accounted(
                        event.event_sha256,
                        event.event_size_bytes,
                        deadline,
                        cancel,
                        charge_read,
                        &|_| Ok(()),
                        record_read,
                        &|_| Ok(()),
                    )?;
                }
            }
        } else {
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
                    self.verify_object(
                        event.event_sha256,
                        event.event_size_bytes,
                        deadline,
                        cancel,
                    )?;
                }
            }
        }
        active(deadline, cancel)?;
        let owned_lock = if prelocked.is_none() {
            Some(self.lock(deadline, cancel)?)
        } else {
            None
        };
        let lock = prelocked
            .or(owned_lock.as_ref())
            .ok_or_else(|| invalid("corpus publication lock is absent"))?;
        let selection = match &pointer_io {
            Some(io) => reader.select_current_selection_budgeted(io, deadline, cancel),
            None => reader.select_current_selection(),
        }
        .map_err(invalid)?;
        if v2_rootset.is_some() && selection.is_some() {
            return Err(invalid("initial V2 writer lost its empty-selector CAS"));
        }
        if v2_rootset.is_none()
            && selection.is_some_and(|selected| {
                selected.format == tos_source_store::CorpusPointerFormat::V2
            })
        {
            return Err(invalid(
                "V1 publication cannot advance a V2-selected corpus rootset",
            ));
        }
        let current = selection.map(|selected| selected.revision.0);
        if current != expected_base && current != Some(revision) {
            return Err(invalid(
                "accepted base changed; re-admit against current revision",
            ));
        }
        if current == Some(revision) && v2_rootset.is_none() {
            self.verify_layout()?;
            return active(deadline, cancel);
        }
        let value = if let Some(rootset) = v2_rootset {
            serde_json::json!({
                "schema_version":"tos_corpus_pointer_v2",
                "current":revision.to_hex(),
                "previous":current.map(|d|d.to_hex()),
                "rootset_sha256":rootset.sha256.to_hex()
            })
        } else {
            serde_json::json!({"schema_version":"tos_corpus_pointer_v1",
                "current":revision.to_hex(), "previous":current.map(|d|d.to_hex())})
        };
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
        let pointer_allocation = if let Some(rootset) = v2_rootset {
            Some((
                rootset
                    .tree_io
                    .reserve_file_allocation(pointer.len() as u64)?,
                rootset.tree_io.reserve_file_allocation(0)?,
            ))
        } else {
            None
        };
        let root_before = if pointer_allocation.is_some() {
            Some(
                self.root
                    .metadata()?
                    .blocks()
                    .checked_mul(512)
                    .ok_or_else(|| invalid("V2 pointer directory allocation overflow"))?,
            )
        } else {
            None
        };
        let mut pending = Temporary::create(&self.root)?;
        if physical {
            write_recorded(
                &mut pending.file,
                &pointer,
                deadline,
                cancel,
                &persistent_charge_write,
                &persistent_record_write,
            )?;
        } else {
            charge_write(pointer.len() as u64)?;
            pending.file.write_all(&pointer)?;
        }
        pending.file.sync_all()?;
        verify(
            &mut pending.file,
            pointer.len() as u64,
            Digest256::of_bytes(&pointer),
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
        let post_rename = (|| {
            if let (
                Some(rootset),
                Some((pointer_file_reservation, directory_reservation)),
                Some(before),
            ) = (v2_rootset, pointer_allocation, root_before)
            {
                let pointer_bytes = pending
                    .file
                    .metadata()?
                    .blocks()
                    .checked_mul(512)
                    .ok_or_else(|| invalid("V2 pointer allocation overflow"))?;
                let directory_after = self
                    .root
                    .metadata()?
                    .blocks()
                    .checked_mul(512)
                    .ok_or_else(|| invalid("V2 pointer directory allocation overflow"))?;
                rootset
                    .tree_io
                    .reconcile_file_allocation(pointer_file_reservation, pointer_bytes)?;
                rootset.tree_io.reconcile_file_allocation(
                    directory_reservation,
                    directory_after.saturating_sub(before),
                )?;
            }
            self.root.sync_all()?;
            self.verify_layout()?;
            let selected = match &pointer_io {
                Some(io) => reader.select_current_selection_budgeted(io, deadline, cancel),
                None => reader.select_current_selection(),
            }
            .map_err(invalid)?;
            let matches = selected.is_some_and(|selected| {
                selected.revision.0 == revision
                    && match v2_rootset {
                        Some(rootset) => {
                            selected.format == tos_source_store::CorpusPointerFormat::V2
                                && selected.rootset_sha256 == Some(rootset.sha256)
                                && selected.previous
                                    == expected_base.map(tos_foundation::SourceRevision)
                        }
                        None => selected.format == tos_source_store::CorpusPointerFormat::V1,
                    }
            });
            if !matches {
                return Err(invalid(
                    "accepted corpus pointer changed during publication",
                ));
            }
            active(deadline, cancel)
        })();
        match post_rename {
            Ok(()) => Ok(()),
            Err(error) if physical => Err(io::Error::new(
                error.kind(),
                StreamedPublicationCommittedRefusal {
                    revision,
                    manifest_sha256,
                    rootset_sha256: v2_rootset.map(|rootset| rootset.sha256),
                    custody: v2_rootset
                        .map(|rootset| rootset.tree_io.custody_reservation())
                        .or_else(|| self.streamed_manifest_custody.borrow().clone()),
                    cause: error,
                },
            )),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn check_current(
        &self,
        expected: Option<Digest256>,
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<()> {
        self.check_current_with_budget(expected, limits, deadline, cancel, None)
    }
    pub(crate) fn check_current_budgeted(
        &self,
        expected: Option<Digest256>,
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        io: &tos_source_store::PinnedSqliteIoBudget,
    ) -> io::Result<()> {
        self.check_current_with_budget(expected, limits, deadline, cancel, Some(io))
    }
    fn check_current_with_budget(
        &self,
        expected: Option<Digest256>,
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        io: Option<&tos_source_store::PinnedSqliteIoBudget>,
    ) -> io::Result<()> {
        let reader = self.reader(limits)?;
        let _lock = self.lock(deadline, cancel)?;
        if match io {
            Some(io) => reader.select_current_budgeted(io, deadline, cancel),
            None => reader.select_current(),
        }
        .map_err(invalid)?
        .map(|r| r.0)
            != expected
        {
            return Err(invalid(
                "accepted base changed; re-admit against current revision",
            ));
        }
        self.verify_layout()?;
        active(deadline, cancel)
    }

    /// Open the selected store or create its four base namespaces under one
    /// native V2 allocation reservation. Existing directories are retained
    /// baseline; a fresh namespace is charged before its first mkdir.
    pub(crate) fn create_or_open_v2(
        path: &Path,
        accountant: Arc<super::source_admission_segment_v2::NativeV2TreeIo>,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        active(deadline, cancel)?;
        if !path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(invalid(
                "V2 corpus store path must be absolute and normalized",
            ));
        }
        let parent_path = path
            .parent()
            .ok_or_else(|| invalid("V2 corpus store parent absent"))?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| invalid("V2 corpus store name absent"))?;
        let parent = tos_fd_open::open_absolute_directory(parent_path).map_err(invalid)?;
        match tos_fd_open::open_directory_at(&parent, Path::new(name)) {
            Ok(existing) => {
                drop(existing);
                let store = Self::open_existing(path, deadline, cancel)?;
                store.attach_v2_allocation_accountant(accountant)?;
                return Ok(store);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }

        let unit = accountant.selected_allocation_unit_bytes();
        if unit == 0 || unit == u64::MAX || parent.metadata()?.blksize() > unit {
            return Err(invalid(
                "V2 store allocation quantum is below held geometry",
            ));
        }
        // Four private directories and growth in their two containing
        // directory entries fit within this finite envelope. This is an
        // invocation reservation, not filesystem free-space evidence.
        let precharge_input = unit
            .checked_mul(7)
            .ok_or_else(|| invalid("V2 namespace allocation profile overflow"))?;
        let parent_before = allocated_bytes(&parent, "V2 store parent allocation overflow")?;
        let reserved = accountant.reserve_file_allocation(precharge_input)?;
        let mut precharge = V2NamespacePrecharge {
            accountant: Arc::clone(&accountant),
            reserved,
            created: false,
        };
        let mut accounting_attempted = false;
        let result = (|| {
            rustix::fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700))?;
            precharge.created = true;
            parent.sync_all()?;
            let root = owned_directory(
                tos_fd_open::open_directory_at(&parent, Path::new(name)).map_err(invalid)?,
            )?;
            let objects = directory_new(&root, "objects")?;
            let revisions = directory_new(&root, "revisions")?;
            let staging = directory_new(&root, "staging")?;
            root.sync_all()?;
            let store = Self {
                path: path.to_owned(),
                root,
                objects,
                revisions,
                staging,
                v2_segments: RefCell::new(None),
                streamed_manifest_custody: RefCell::new(None),
                v2_store_custody: RefCell::new(None),
                v2_allocation_accountant: RefCell::new(None),
            };
            store.verify_layout()?;
            store.attach_v2_allocation_accountant(Arc::clone(&accountant))?;
            active(deadline, cancel)?;

            let parent_after = allocated_bytes(&parent, "V2 store parent allocation overflow")?;
            let allocated_revisions = allocated_bytes(
                &store.revisions,
                "V2 revisions directory allocation overflow",
            )?;
            let allocated_staging =
                allocated_bytes(&store.staging, "V2 staging directory allocation overflow")?;
            let actual = allocated_bytes(&store.root, "V2 store directory allocation overflow")?
                .checked_add(allocated_bytes(
                    &store.objects,
                    "V2 objects directory allocation overflow",
                )?)
                .and_then(|bytes| bytes.checked_add(allocated_revisions))
                .and_then(|bytes| bytes.checked_add(allocated_staging))
                .and_then(|bytes| bytes.checked_add(parent_after.saturating_sub(parent_before)))
                .ok_or_else(|| invalid("V2 namespace allocation overflow"))?;
            accounting_attempted = true;
            accountant.reconcile_file_allocation(reserved, actual)?;
            Ok(store)
        })();
        if result.is_err() && precharge.created && !accounting_attempted {
            match partial_v2_namespace_allocation(&parent, name, parent_before) {
                Ok(Some(actual)) => {
                    let _ = accountant.reconcile_file_allocation(reserved, actual);
                }
                Ok(None) | Err(_) => {
                    // Keep the complete precharge attached if the path cannot
                    // be safely measured after a partial namespace write.
                    let _ = accountant.reconcile_file_allocation(reserved, reserved);
                }
            }
        }
        result
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
        let v2_segments = optional_directory(&root, "segments-v2")?;
        let store = Self {
            path: path.to_owned(),
            root,
            objects,
            revisions,
            staging,
            v2_segments: RefCell::new(v2_segments),
            streamed_manifest_custody: RefCell::new(None),
            v2_store_custody: RefCell::new(None),
            v2_allocation_accountant: RefCell::new(None),
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
        match self.v2_segments.borrow().as_ref() {
            Some(held) => {
                let selected = owned_directory(
                    tos_fd_open::open_directory_at(&root, Path::new("segments-v2"))
                        .map_err(invalid)?,
                )?;
                if identity(&selected)? != identity(held)? {
                    return Err(invalid("corpus V2 segment namespace replaced"));
                }
            }
            None if optional_directory(&root, "segments-v2")?.is_some() => {
                return Err(invalid("unselected corpus V2 segment namespace exists"));
            }
            None => (),
        }
        Ok(())
    }

    /// Select the private authenticated V2 tree store below this exact held
    /// AdmissionStore root. The descriptor-root API keeps SegmentStore from
    /// resolving the caller's store path a second time.
    pub(crate) fn segment_store_v2(
        &self,
        domain: &[u8],
        limits: tos_segment_store::SegmentLimits,
    ) -> io::Result<tos_segment_store::SegmentStore> {
        self.segment_store_v2_inner(domain, limits, None, None, None)
    }

    pub(crate) fn segment_store_v2_with_io(
        &self,
        domain: &[u8],
        limits: tos_segment_store::SegmentLimits,
        io: Arc<dyn tos_segment_store::AuthenticatedTreeIoLedgerV1>,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<tos_segment_store::SegmentStore> {
        active(deadline, cancel)?;
        self.segment_store_v2_inner(domain, limits, Some(io), Some(deadline), Some(cancel))
    }

    fn segment_store_v2_inner(
        &self,
        domain: &[u8],
        limits: tos_segment_store::SegmentLimits,
        io: Option<Arc<dyn tos_segment_store::AuthenticatedTreeIoLedgerV1>>,
        deadline: Option<Instant>,
        cancel: Option<&AtomicBool>,
    ) -> io::Result<tos_segment_store::SegmentStore> {
        self.verify_layout()?;
        let existing = self
            .v2_segments
            .borrow()
            .as_ref()
            .map(File::try_clone)
            .transpose()?;
        let store = if let Some(root) = existing {
            match (io, deadline, cancel) {
                (Some(io), Some(deadline), Some(cancel)) => {
                    tos_segment_store::SegmentStore::open_existing_at_with_io(
                        &root, limits, io, deadline, cancel,
                    )
                    .map_err(invalid)?
                }
                (None, None, None) => {
                    tos_segment_store::SegmentStore::open_existing_at(&root, limits)
                        .map_err(invalid)?
                }
                _ => return Err(invalid("V2 segment IO profile association differs")),
            }
        } else {
            let parent_before = self
                .root
                .metadata()?
                .blocks()
                .checked_mul(512)
                .ok_or_else(|| invalid("V2 namespace allocation overflow"))?;
            let outer_reservation = if let Some(ledger) = io.as_deref() {
                let unit = ledger.allocation_unit_bytes();
                let upper = unit
                    .checked_mul(3)
                    .filter(|_| unit != 0 && unit != u64::MAX)
                    .ok_or_else(|| invalid("V2 namespace allocation profile differs"))?;
                if !ledger.reserve_allocated_bytes(upper) {
                    return Err(invalid("V2 namespace allocation precharge refused"));
                }
                Some((ledger, upper))
            } else {
                None
            };
            let root = directory(&self.root, "segments-v2")?;
            if let Some((ledger, upper)) = outer_reservation {
                let root_bytes = root
                    .metadata()?
                    .blocks()
                    .checked_mul(512)
                    .ok_or_else(|| invalid("V2 namespace allocation overflow"))?;
                let parent_after = self
                    .root
                    .metadata()?
                    .blocks()
                    .checked_mul(512)
                    .ok_or_else(|| invalid("V2 namespace allocation overflow"))?;
                let actual = root_bytes
                    .checked_add(parent_after.saturating_sub(parent_before))
                    .ok_or_else(|| invalid("V2 namespace allocation overflow"))?;
                if !ledger.reconcile_allocated_bytes(upper, actual) {
                    return Err(invalid("V2 namespace allocation reconciliation refused"));
                }
            }
            *self.v2_segments.borrow_mut() = Some(root.try_clone()?);
            let store = match (io, deadline, cancel) {
                (Some(io), Some(deadline), Some(cancel)) => {
                    tos_segment_store::SegmentStore::initialize_empty_at_with_io(
                        &root, domain, limits, io, deadline, cancel,
                    )
                    .map_err(invalid)?
                }
                (None, None, None) => {
                    tos_segment_store::SegmentStore::initialize_empty_at(&root, domain, limits)
                        .map_err(invalid)?
                }
                _ => return Err(invalid("V2 segment IO profile association differs")),
            };
            store
        };
        self.verify_layout()?;
        Ok(store)
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

    /// Hold the existing admission lock for a read-only backup walk. This
    /// path never creates or repairs the lock: a backup must not mutate the
    /// selected store merely by opening it. Publishers take the exclusive
    /// variant above, so this shared lease keeps the selected current/history
    /// descriptor and its retained physical closure stable for the walk.
    pub(crate) fn lock_for_backup(
        &self,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<AdmissionLock> {
        active(deadline, cancel)?;
        self.verify_layout()?;
        let file = File::from(rustix::fs::openat(
            &self.root,
            ".admission.lock",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o077 != 0
        {
            return Err(invalid("corpus admission lock ownership or mode differs"));
        }
        loop {
            active(deadline, cancel)?;
            match rustix::fs::flock(&file, FlockOperation::NonBlockingLockShared) {
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
        self.ingest_accounted(
            source,
            size,
            digest,
            deadline,
            cancel,
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
        )
    }
    pub(crate) fn ingest_accounted(
        &self,
        source: &mut File,
        size: u64,
        digest: Digest256,
        deadline: Instant,
        cancel: &AtomicBool,
        charge_read: &dyn Fn(u64) -> io::Result<()>,
        charge_write: &dyn Fn(u64) -> io::Result<()>,
        record_read: &dyn Fn(u64) -> io::Result<()>,
        record_write: &dyn Fn(u64) -> io::Result<()>,
    ) -> io::Result<()> {
        active(deadline, cancel)?;
        let before = source.metadata()?;
        if !before.is_file() || before.len() != size {
            return Err(invalid("source update size/type differs"));
        }
        let name = digest.to_hex();
        match tos_fd_open::open_regular_at(&self.objects, Path::new(&name)) {
            Ok(mut existing) => {
                verify_file_recorded(
                    source,
                    size,
                    digest,
                    deadline,
                    cancel,
                    charge_read,
                    record_read,
                )?;
                verify_file_recorded(
                    &mut existing,
                    size,
                    digest,
                    deadline,
                    cancel,
                    charge_read,
                    record_read,
                )?;
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
        let mut object_precharge = self
            .v2_allocation_accountant
            .borrow()
            .clone()
            .map(|accountant| {
                let input_bytes = size
                    .checked_add(accountant.selected_allocation_unit_bytes())
                    .ok_or_else(|| invalid("V2 object allocation profile overflow"))?;
                let reserved = accountant.reserve_file_allocation(input_bytes)?;
                Ok::<_, io::Error>(V2ObjectPrecharge {
                    accountant,
                    reserved,
                    linked: false,
                })
            })
            .transpose()?;
        let directory_allocated = |directory: &File| -> io::Result<u64> {
            directory
                .metadata()?
                .blocks()
                .checked_mul(512)
                .ok_or_else(|| invalid("V2 object namespace allocation overflow"))
        };
        let objects_before = if object_precharge.is_some() {
            directory_allocated(&self.objects)?
        } else {
            0
        };
        let staging_before = if object_precharge.is_some() {
            directory_allocated(&self.staging)?
        } else {
            0
        };
        let mut temporary = Temporary::create(&self.staging)?;
        source.seek(SeekFrom::Start(0))?;
        let mut copied = 0u64;
        let mut hash = Digest256Hasher::new();
        let mut chunk = [0; 65536];
        loop {
            active(deadline, cancel)?;
            charge_read(chunk.len() as u64)?;
            let n = source.read(&mut chunk)?;
            record_read(n as u64)?;
            if n == 0 {
                break;
            }
            copied = copied
                .checked_add(n as u64)
                .filter(|n| *n <= size)
                .ok_or_else(|| invalid("source grew during admission"))?;
            hash.update(&chunk[..n]);
            write_recorded(
                &mut temporary.file,
                &chunk[..n],
                deadline,
                cancel,
                charge_write,
                record_write,
            )?;
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
        verify_file_recorded(
            &mut temporary.file,
            size,
            digest,
            deadline,
            cancel,
            charge_read,
            record_read,
        )?;
        self.verify_layout()?;
        let name = digest.to_hex();
        let installed_new = match rustix::fs::linkat(
            &self.staging,
            temporary.name.as_str(),
            &self.objects,
            name.as_str(),
            AtFlags::empty(),
        ) {
            Ok(()) => true,
            Err(Errno::EXIST) => false,
            Err(error) => return Err(error.into()),
        };
        if installed_new {
            if let Some(precharge) = object_precharge.as_mut() {
                precharge.linked = true;
            }
        }
        let mut installed =
            tos_fd_open::open_regular_at(&self.objects, Path::new(&name)).map_err(invalid)?;
        if installed_new {
            if let Some(precharge) = object_precharge.as_ref() {
                let file_allocated = directory_allocated(&installed)?;
                drop(temporary);
                let objects_after = directory_allocated(&self.objects)?;
                let staging_after = directory_allocated(&self.staging)?;
                let namespace_growth = objects_after
                    .saturating_sub(objects_before)
                    .checked_add(staging_after.saturating_sub(staging_before))
                    .ok_or_else(|| invalid("V2 object namespace growth overflow"))?;
                let actual = file_allocated
                    .checked_add(namespace_growth)
                    .ok_or_else(|| invalid("V2 object allocation overflow"))?;
                precharge
                    .accountant
                    .reconcile_file_allocation(precharge.reserved, actual)?;
            }
        }
        verify_file_recorded(
            &mut installed,
            size,
            digest,
            deadline,
            cancel,
            charge_read,
            record_read,
        )?;
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
        self.copy_object_accounted(
            digest,
            size,
            sink,
            deadline,
            cancel,
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
        )
    }
    pub(crate) fn copy_object_accounted(
        &self,
        digest: Digest256,
        size: u64,
        sink: &mut dyn Write,
        deadline: Instant,
        cancel: &AtomicBool,
        charge_read: &dyn Fn(u64) -> io::Result<()>,
        charge_write: &dyn Fn(u64) -> io::Result<()>,
        record_read: &dyn Fn(u64) -> io::Result<()>,
        record_write: &dyn Fn(u64) -> io::Result<()>,
    ) -> io::Result<()> {
        self.verify_layout()?;
        let mut file = tos_fd_open::open_regular_at(&self.objects, Path::new(&digest.to_hex()))
            .map_err(invalid)?;
        stream_verified_recorded(
            &mut file,
            size,
            digest,
            deadline,
            cancel,
            sink,
            charge_read,
            charge_write,
            record_read,
            record_write,
        )?;
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
        self.verify_object_accounted(
            digest,
            size,
            deadline,
            cancel,
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
        )
    }
    pub(crate) fn verify_object_accounted(
        &self,
        digest: Digest256,
        size: u64,
        deadline: Instant,
        cancel: &AtomicBool,
        charge_read: &dyn Fn(u64) -> io::Result<()>,
        _charge_write: &dyn Fn(u64) -> io::Result<()>,
        record_read: &dyn Fn(u64) -> io::Result<()>,
        _record_write: &dyn Fn(u64) -> io::Result<()>,
    ) -> io::Result<()> {
        self.verify_layout()?;
        let mut file = tos_fd_open::open_regular_at(&self.objects, Path::new(&digest.to_hex()))
            .map_err(invalid)?;
        verify_file_recorded(
            &mut file,
            size,
            digest,
            deadline,
            cancel,
            charge_read,
            record_read,
        )?;
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
        self.read_object_accounted(
            digest,
            size,
            cap,
            deadline,
            cancel,
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
            &|_| Ok(()),
        )
    }
    pub(crate) fn read_object_accounted(
        &self,
        digest: Digest256,
        size: u64,
        cap: usize,
        deadline: Instant,
        cancel: &AtomicBool,
        charge_read: &dyn Fn(u64) -> io::Result<()>,
        _charge_write: &dyn Fn(u64) -> io::Result<()>,
        record_read: &dyn Fn(u64) -> io::Result<()>,
        _record_write: &dyn Fn(u64) -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        if size > cap as u64 {
            return Err(invalid("candidate object exceeds selected read bound"));
        }
        active(deadline, cancel)?;
        self.verify_layout()?;
        let mut file = tos_fd_open::open_regular_at(&self.objects, Path::new(&digest.to_hex()))
            .map_err(invalid)?;
        let mut bytes = Vec::with_capacity(size as usize);
        stream_verified_recorded(
            &mut file,
            size,
            digest,
            deadline,
            cancel,
            &mut bytes,
            charge_read,
            &|_| Ok(()),
            record_read,
            &|_| Ok(()),
        )?;
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
    stream_verified_accounted(
        file,
        size,
        digest,
        deadline,
        cancel,
        sink,
        &|_| Ok(()),
        &|_| Ok(()),
    )
}
fn stream_verified_accounted(
    file: &mut File,
    size: u64,
    digest: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
    sink: &mut (impl Write + ?Sized),
    charge_read: &dyn Fn(u64) -> io::Result<()>,
    charge_write: &dyn Fn(u64) -> io::Result<()>,
) -> io::Result<()> {
    stream_verified_recorded(
        file,
        size,
        digest,
        deadline,
        cancel,
        sink,
        charge_read,
        charge_write,
        &|_| Ok(()),
        &|_| Ok(()),
    )
}
fn stream_verified_recorded(
    file: &mut File,
    size: u64,
    digest: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
    sink: &mut (impl Write + ?Sized),
    charge_read: &dyn Fn(u64) -> io::Result<()>,
    charge_write: &dyn Fn(u64) -> io::Result<()>,
    record_read: &dyn Fn(u64) -> io::Result<()>,
    record_write: &dyn Fn(u64) -> io::Result<()>,
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
        charge_read(bytes.len() as u64)?;
        let n = file.read(&mut bytes)?;
        record_read(n as u64)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .filter(|n| *n <= size)
            .ok_or_else(|| invalid("corpus object grew"))?;
        hash.update(&bytes[..n]);
        write_recorded(
            sink,
            &bytes[..n],
            deadline,
            cancel,
            charge_write,
            record_write,
        )?;
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

fn verify_file_accounted(
    file: &mut File,
    size: u64,
    digest: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
    charge_read: &dyn Fn(u64) -> io::Result<()>,
) -> io::Result<()> {
    stream_verified_accounted(
        file,
        size,
        digest,
        deadline,
        cancel,
        &mut io::sink(),
        charge_read,
        &|_| Ok(()),
    )
}

fn write_accounted(
    sink: &mut (impl Write + ?Sized),
    mut raw: &[u8],
    deadline: Instant,
    cancel: &AtomicBool,
    charge: &dyn Fn(u64) -> io::Result<()>,
) -> io::Result<()> {
    write_recorded(sink, raw, deadline, cancel, charge, &|_| Ok(()))
}

fn verify_file_recorded(
    file: &mut File,
    size: u64,
    digest: Digest256,
    deadline: Instant,
    cancel: &AtomicBool,
    charge: &dyn Fn(u64) -> io::Result<()>,
    record: &dyn Fn(u64) -> io::Result<()>,
) -> io::Result<()> {
    stream_verified_recorded(
        file,
        size,
        digest,
        deadline,
        cancel,
        &mut io::sink(),
        charge,
        &|_| Ok(()),
        record,
        &|_| Ok(()),
    )
}
fn write_recorded(
    sink: &mut (impl Write + ?Sized),
    mut raw: &[u8],
    deadline: Instant,
    cancel: &AtomicBool,
    charge: &dyn Fn(u64) -> io::Result<()>,
    record: &dyn Fn(u64) -> io::Result<()>,
) -> io::Result<()> {
    while !raw.is_empty() {
        active(deadline, cancel)?;
        charge(raw.len() as u64)?;
        match sink.write(raw) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "admission sink made no progress",
                ));
            }
            Ok(n) => {
                record(n as u64)?;
                raw = &raw[n..];
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        active(deadline, cancel)?;
    }
    Ok(())
}
