//! Descriptor-bound maintained creation publication. This coordinates byte
//! mechanics in an independently selected owner filesystem, not source admission.
//! The corpus lock name and rename-no-replace protocol interoperate with Python.

use crate::source_claims::SerializedClaimCreation;
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation::{CreationPackage, SerializedCreation};
use rustix::fs::{AtFlags, FlockOperation, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, Metadata, Permissions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CORPUS_LOCK: &str = ".historical-create.writer.lock";
pub(crate) const MAX_FILES: usize = 4096;
pub(crate) const MAX_BYTES: usize = 33_554_432;

pub(crate) fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(SourceCommandError::Denied(
            "creation filesystem cancelled or expired",
        ))
    } else {
        Ok(())
    }
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
pub(crate) fn inode(m: &Metadata) -> (u64, u64) {
    (m.dev(), m.ino())
}
pub(crate) fn owned(file: &File, uid: u32, directory: bool) -> SourceCommandResult<Metadata> {
    let m = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("creation fd metadata"))?;
    if m.uid() != uid
        || m.mode() & 0o022 != 0
        || m.is_dir() != directory
        || (!directory && !m.is_file())
    {
        return Err(SourceCommandError::Denied(
            "creation path ownership/type/write boundary",
        ));
    }
    Ok(m)
}
fn child(parent: &File, leaf: &str) -> SourceCommandResult<File> {
    tos_fd_open::open_directory_at(parent, Path::new(leaf))
        .map_err(|_| SourceCommandError::Denied("creation directory absent or unsafe"))
}
pub(crate) fn protected_configuration_parents(path: &Path, uid: u32) -> SourceCommandResult<()> {
    use std::path::Component;
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return Err(SourceCommandError::Denied(
            "configuration path is not absolute",
        ));
    }
    let parts = components.collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|p| !matches!(p, Component::Normal(_))) {
        return Err(SourceCommandError::Denied(
            "configuration path is not normalized",
        ));
    }
    let mut parent = tos_fd_open::open_absolute_directory(Path::new("/"))
        .map_err(|_| SourceCommandError::Denied("configuration root descriptor"))?;
    for part in std::iter::once(None).chain(parts[..parts.len() - 1].iter().map(Some)) {
        if let Some(Component::Normal(name)) = part {
            parent = tos_fd_open::open_directory_at(&parent, Path::new(name))
                .map_err(|_| SourceCommandError::Denied("configuration ancestor unsafe"))?;
        }
        let metadata = parent
            .metadata()
            .map_err(|_| SourceCommandError::Denied("configuration ancestor metadata"))?;
        let root_sticky = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
        if !metadata.is_dir()
            || ![0, uid].contains(&metadata.uid())
            || metadata.mode() & 0o022 != 0 && !root_sticky
        {
            return Err(SourceCommandError::Denied(
                "configuration ancestor ownership/write boundary",
            ));
        }
    }
    Ok(())
}
fn walk(root: &File, path: &str, uid: u32) -> SourceCommandResult<File> {
    let relative = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("creation directory relative path"))?;
    let mut fd = tos_fd_open::reopen_directory(root)
        .map_err(|_| SourceCommandError::Denied("creation root descriptor"))?;
    for part in relative.as_str().split('/') {
        fd = child(&fd, part)?;
        owned(&fd, uid, true)?;
    }
    Ok(fd)
}
pub(crate) fn raw(
    file: &mut File,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let before = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("creation read metadata"))?;
    if !before.is_file() || before.len() > cap as u64 {
        return Err(SourceCommandError::Invalid(
            "creation read byte/type budget",
        ));
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 65536];
    loop {
        active(deadline, cancelled)?;
        let n = file
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Invalid("creation descriptor read"))?;
        if n == 0 {
            break;
        }
        if bytes.len().checked_add(n).is_none_or(|size| size > cap) {
            return Err(SourceCommandError::Invalid(
                "creation read exceeds byte budget",
            ));
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    let after = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("creation readback metadata"))?;
    if bytes.len() as u64 != before.len() || stamp(&before) != stamp(&after) {
        return Err(SourceCommandError::Conflict(
            "creation input changed during read",
        ));
    }
    Ok(bytes)
}

/// Normal Unix ownership and exact protected configuration are selected here.
/// Construction creates no directory, writes no record and issues no admission.
pub struct CreationFilesystem {
    root_path: PathBuf,
    root: File,
    root_identity: (u64, u64),
    configuration_path: PathBuf,
    configuration_raw: Vec<u8>,
    uid: u32,
}

/// The existing owner corpus mutex, held across a managed durable creation.
/// This observes current protected configuration; it grants no source admission
/// and says nothing about the managed cohort's index completeness.
pub(crate) struct CreationOwnerFence<'a> {
    filesystem: &'a CreationFilesystem,
    package: CreationPackage<'a>,
    witness: File,
    lock: File,
}
impl CreationOwnerFence<'_> {
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.filesystem
            .current_package(self.package, deadline, cancelled)?;
        let current = tos_fd_open::open_regular_at(&self.witness, Path::new(CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("creation corpus lock path changed"))?;
        if inode(&owned(&self.lock, self.filesystem.uid, false)?)
            != inode(&owned(&current, self.filesystem.uid, false)?)
        {
            return Err(SourceCommandError::Conflict(
                "creation locked inode detached",
            ));
        }
        Ok(())
    }
}

/// Bounded execution authority is an actually newly created private directory,
/// not a caller-supplied Boolean or an arbitrary existing canonical root. Its
/// opaque identity is retained through fixture seeding and publication.
pub struct IsolatedCreationRoot {
    path: PathBuf,
    directory: File,
    identity: (u64, u64),
}
impl IsolatedCreationRoot {
    pub fn create(
        parent: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let uid = rustix::process::geteuid().as_raw();
        let parent_fd = tos_fd_open::open_absolute_directory(parent)
            .map_err(|_| SourceCommandError::Denied("isolated creation parent unsafe"))?;
        owned(&parent_fd, uid, true)?;
        for _ in 0..32 {
            active(deadline, cancelled)?;
            let mut entropy = [0u8; 24];
            File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut entropy))
                .map_err(|_| SourceCommandError::Invalid("isolated root entropy"))?;
            let name = format!(
                "tos-isolated-create-{}",
                Digest256::of_bytes(&entropy).to_hex()
            );
            match rustix::fs::mkdirat(&parent_fd, name.as_str(), Mode::from_raw_mode(0o700)) {
                Ok(()) => {
                    let directory = child(&parent_fd, &name)?;
                    let identity = inode(&owned(&directory, uid, true)?);
                    parent_fd
                        .sync_all()
                        .map_err(|_| SourceCommandError::Invalid("isolated root parent fsync"))?;
                    return Ok(Self {
                        path: parent.join(name),
                        directory,
                        identity,
                    });
                }
                Err(Errno::EXIST) => continue,
                Err(_) => return Err(SourceCommandError::Invalid("isolated root mkdir")),
            }
        }
        Err(SourceCommandError::Conflict(
            "isolated root name collision budget",
        ))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<File> {
        active(deadline, cancelled)?;
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied("isolated root account changed"));
        }
        let current = tos_fd_open::open_absolute_directory(&self.path)
            .map_err(|_| SourceCommandError::Conflict("isolated root path replaced"))?;
        if inode(&owned(&current, uid, true)?) != self.identity
            || inode(&owned(&self.directory, uid, true)?) != self.identity
        {
            return Err(SourceCommandError::Conflict(
                "isolated root identity changed",
            ));
        }
        Ok(current)
    }
}
impl CreationFilesystem {
    pub fn select_isolated(
        isolated: &IsolatedCreationRoot,
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        let root = isolated.path();
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied(
                "creation refuses changed real/effective account",
            ));
        }
        let fd = tos_fd_open::open_absolute_directory(root)
            .map_err(|_| SourceCommandError::Denied("creation selected absolute root"))?;
        let m = owned(&fd, uid, true)?;
        if inode(&m) != isolated.identity
            || inode(&owned(&isolated.directory, uid, true)?) != isolated.identity
        {
            return Err(SourceCommandError::Conflict(
                "isolated creation root replaced",
            ));
        }
        protected_configuration_parents(configuration_path, uid)?;
        let mut config = tos_fd_open::open_absolute_regular(configuration_path, 1_048_576)
            .map_err(|_| SourceCommandError::Denied("creation protected configuration path"))?;
        let config_m = owned(&config, uid, false)?;
        if config_m.mode() & 0o077 != 0 || configuration_path.starts_with(root.join("ToS")) {
            return Err(SourceCommandError::Denied(
                "creation configuration is not privately protected",
            ));
        }
        let configuration_raw = raw(&mut config, 1_048_576, deadline, cancelled)?;
        let value = cmd::parse(&configuration_raw)?;
        if Path::new(cmd::text(&value, "source_root")?) != root
            || cmd::integer(&value, "uid")? != u64::from(uid)
        {
            return Err(SourceCommandError::Denied(
                "creation configuration selects another root/account",
            ));
        }
        Ok(Self {
            root_path: root.to_path_buf(),
            root: fd,
            root_identity: inode(&m),
            configuration_path: configuration_path.to_path_buf(),
            configuration_raw,
            uid,
        })
    }
    pub(crate) fn current(
        &self,
        package: &SerializedCreation,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.current_package(CreationPackage::V1(package), deadline, cancelled)
    }
    fn current_package(
        &self,
        package: CreationPackage<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.current_context(package.prepared().context(), deadline, cancelled)
    }
    fn current_context(
        &self,
        context: &cmd::CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        if rustix::process::geteuid().as_raw() != self.uid
            || rustix::process::getuid().as_raw() != self.uid
        {
            return Err(SourceCommandError::Denied("creation account changed"));
        }
        let root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|_| SourceCommandError::Conflict("creation root replaced or unsafe"))?;
        if inode(&owned(&root, self.uid, true)?) != self.root_identity {
            return Err(SourceCommandError::Conflict(
                "creation selected root identity changed",
            ));
        }
        protected_configuration_parents(&self.configuration_path, self.uid)?;
        let mut fd = tos_fd_open::open_absolute_regular(&self.configuration_path, 1_048_576)
            .map_err(|_| {
                SourceCommandError::Denied("creation current configuration unavailable")
            })?;
        if owned(&fd, self.uid, false)?.mode() & 0o077 != 0 {
            return Err(SourceCommandError::Denied(
                "creation current configuration protection changed",
            ));
        }
        let bytes = raw(&mut fd, 1_048_576, deadline, cancelled)?;
        if bytes != self.configuration_raw || bytes != context.configuration_raw {
            return Err(SourceCommandError::Conflict(
                "creation delegation changed before publication",
            ));
        }
        let config = cmd::parse(&bytes)?;
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        if context.effective_uid != u64::from(self.uid) {
            return Err(SourceCommandError::Denied(
                "creation prepared account differs",
            ));
        }
        Ok(())
    }

    /// Acquire before STO/PG commit locks and retain until the actual outcome.
    /// The consumer rechecks this fence at its final atomic write edge.
    pub(crate) fn hold_creation_owner<'a>(
        &'a self,
        package: CreationPackage<'a>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationOwnerFence<'a>> {
        self.current_package(package, deadline, cancelled)?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let lock = self.lock(&witness, deadline, cancelled)?;
        let fence = CreationOwnerFence {
            filesystem: self,
            package,
            witness,
            lock,
        };
        fence.verify_current(deadline, cancelled)?;
        Ok(fence)
    }

    /// Real maintained corpus mutex; separate open descriptions also conflict
    /// inside one process. No lock acquisition blocks beyond cancellation/time.
    fn lock(
        &self,
        witness: &File,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<File> {
        let fd: File = rustix::fs::openat(
            witness,
            CORPUS_LOCK,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|_| SourceCommandError::Denied("creation corpus lock open"))?;
        owned(&fd, self.uid, false)?;
        loop {
            active(deadline, cancelled)?;
            match rustix::fs::flock(&fd, FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(Errno::AGAIN) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => {
                    return Err(SourceCommandError::Denied(
                        "creation corpus lock unsupported",
                    ));
                }
            }
        }
        owned(&fd, self.uid, false)?;
        // Verify pathname still identifies the locked inode, not a replacement.
        let current = tos_fd_open::open_regular_at(witness, Path::new(CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("creation corpus lock path changed"))?;
        if inode(
            &fd.metadata()
                .map_err(|_| SourceCommandError::Invalid("creation lock identity"))?,
        ) != inode(
            &current
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("creation current lock identity"))?,
        ) {
            return Err(SourceCommandError::Conflict(
                "creation locked inode detached",
            ));
        }
        Ok(fd)
    }

    /// The Claim owner uses the same held corpus lock, secure directory
    /// staging and NOREPLACE publication as the source creation families.
    /// The package is privately built from a complete selected Claim cut.
    pub(crate) fn publish_claim_isolated(
        &self,
        package: &SerializedClaimCreation,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        if components != package.components() || software.selection() != components.capture() {
            return Err(SourceCommandError::Conflict(
                "Claim selected producer differs",
            ));
        }
        self.current_context(package.context(), deadline, cancelled)?;
        package
            .context()
            .check_from_selected_captures(cut, software, components, deadline, cancelled)?;
        let tos = walk(&self.root, "ToS", self.uid)?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current_context(package.context(), deadline, cancelled)?;
        self.reselect_context(
            package.context(),
            package.home(),
            package.files(),
            Some(package.operational_sidecars()),
            cut,
            None,
            false,
            deadline,
            cancelled,
        )?;
        self.reselect_components(software, components, deadline, cancelled)?;
        let (parent_path, target_name) = package
            .home()
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim target parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        let parent_identity = inode(&owned(&parent, self.uid, true)?);
        let mut stage = PendingCreation::create(&tos, self.uid, deadline, cancelled)?;
        let preparation = (|| {
            for (name, bytes) in package.files() {
                stage.write(name, bytes, deadline, cancelled)?;
            }
            stage
                .directory
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("Claim staging directory fsync"))?;
            self.current_context(package.context(), deadline, cancelled)?;
            self.reselect_context(
                package.context(),
                package.home(),
                package.files(),
                Some(package.operational_sidecars()),
                cut,
                Some(&stage.name),
                false,
                deadline,
                cancelled,
            )?;
            self.reselect_components(software, components, deadline, cancelled)?;
            let current_parent = walk(&self.root, parent_path, self.uid)?;
            if inode(&owned(&current_parent, self.uid, true)?) != parent_identity {
                return Err(SourceCommandError::Conflict("Claim target parent changed"));
            }
            rustix::fs::renameat_with(
                &tos,
                stage.name.as_str(),
                &parent,
                target_name,
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                if error == Errno::EXIST {
                    SourceCommandError::Conflict("Claim creation target occupied")
                } else {
                    SourceCommandError::Invalid("Claim creation atomic publication")
                }
            })?;
            stage.published = true;
            Ok(())
        })();
        if let Err(error) = preparation {
            stage.rollback()?;
            return Err(error);
        }
        let durable = parent.sync_all().is_ok() && tos.sync_all().is_ok();
        Ok(CreationPublication {
            home: package.home().clone(),
            receipt_sha256: Digest256::of_bytes(&package.files()["source-create-receipt.json"]),
            durability: if durable {
                CreationDurability::DirectoriesSynced
            } else {
                CreationDurability::PublishedSyncIncomplete
            },
            replayed: false,
        })
    }

    /// Cold exact replay: the caller must first reconstruct this package from
    /// retained bytes and the original selected cut. The current filesystem is
    /// independently reselected under the same corpus lock before success.
    pub(crate) fn replay_claim_isolated(
        &self,
        package: &SerializedClaimCreation,
        original_cut: &CorpusCutReader,
        current_context: &cmd::CommandContext,
        current_cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        if !package.command().replayed
            || components != package.components()
            || software.selection() != components.capture()
        {
            return Err(SourceCommandError::Conflict(
                "Claim retained replay basis differs",
            ));
        }
        self.current_context(package.context(), deadline, cancelled)?;
        if current_context.base_revision != current_cut.current().revision() {
            return Err(SourceCommandError::Conflict("Claim current cut differs"));
        }
        package.context().check_from_selected_captures(
            original_cut,
            software,
            components,
            deadline,
            cancelled,
        )?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current_context(package.context(), deadline, cancelled)?;
        self.reselect_context(
            current_context,
            package.home(),
            &BTreeMap::new(),
            Some(package.operational_sidecars()),
            current_cut,
            None,
            false,
            deadline,
            cancelled,
        )?;
        self.reselect_components(software, components, deadline, cancelled)?;
        let directory = walk(&self.root, package.home().as_str(), self.uid)?;
        let parent_path = package
            .home()
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim replay parent"))?
            .0;
        let parent = walk(&self.root, parent_path, self.uid)?;
        let tos = walk(&self.root, "ToS", self.uid)?;
        let durable =
            directory.sync_all().is_ok() && parent.sync_all().is_ok() && tos.sync_all().is_ok();
        Ok(CreationPublication {
            home: package.home().clone(),
            receipt_sha256: Digest256::of_bytes(&package.files()["source-create-receipt.json"]),
            durability: if durable {
                CreationDurability::DirectoriesSynced
            } else {
                CreationDurability::PublishedSyncIncomplete
            },
            replayed: true,
        })
    }

    /// A cold bounded read of the current Claim package. The Claim owner
    /// validates original/revised closure before it becomes a replay.
    pub(crate) fn read_claim_retained(
        &self,
        context: &cmd::CommandContext,
        home: &RelativePath,
        allowed_names: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<BTreeMap<String, Vec<u8>>>> {
        self.current_context(context, deadline, cancelled)?;
        let (parent_path, name) = home
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim retained parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        match rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(Errno::NOENT) => return Ok(None),
            Err(_) => return Err(SourceCommandError::Denied("Claim retained target unsafe")),
            Ok(_) => {}
        }
        let directory = walk(&self.root, home.as_str(), self.uid)?;
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Invalid("Claim retained directory listing"))?;
        let mut names = BTreeSet::new();
        for entry in entries {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| SourceCommandError::Invalid("Claim retained entry"))?
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Invalid("Claim retained name"))?;
            if !allowed_names.contains(&name)
                || !names.insert(name)
                || names.len() > allowed_names.len()
            {
                return Err(SourceCommandError::Invalid("Claim retained entry budget"));
            }
        }
        let required = BTreeSet::from([
            "source-claims.jsonl".to_owned(),
            "source-create-request.json".to_owned(),
            "source-create-environment.json".to_owned(),
            "source-create-provenance.jsonl".to_owned(),
            "source-create-receipt.json".to_owned(),
        ]);
        if !required.is_subset(&names) {
            return Err(SourceCommandError::Conflict(
                "Claim retained package incomplete",
            ));
        }
        // Maintained unrevised creation reads only its named adjacent members
        // with a per-file limit. Once HISTORY exists, its revision owner uses
        // the stricter flat-package 64-file/8 MiB aggregate contract.
        let revised = names.contains("claim-revision-history.json");
        if revised && names.len() > 64 {
            return Err(SourceCommandError::Invalid(
                "Claim revised package file budget",
            ));
        }
        let mut files = BTreeMap::new();
        let mut total = 0usize;
        for name in names {
            let mut file = tos_fd_open::open_regular_at(&directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("Claim retained member unsafe"))?;
            let expected_mode = if name.ends_with(".writer.lock") {
                0o600
            } else {
                0o644
            };
            if owned(&file, self.uid, false)?.mode() & 0o777 != expected_mode {
                return Err(SourceCommandError::Conflict(
                    "Claim retained member mode differs",
                ));
            }
            let bytes = raw(&mut file, 8_388_608, deadline, cancelled)?;
            total = total
                .checked_add(bytes.len())
                .ok_or(SourceCommandError::Invalid("Claim retained byte overflow"))?;
            if revised && total > 8_388_608 {
                return Err(SourceCommandError::Invalid(
                    "Claim revised package byte budget",
                ));
            }
            files.insert(name, bytes);
        }
        Ok(Some(files))
    }

    /// Publish only a privately constructed, genuinely serialized handler
    /// package. Canonical source admission is deliberately a separate gate.
    /// This reusable filesystem mechanism is executed only with independently
    /// selected isolated-owner authority until that gate exists.
    pub fn publish_isolated(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        self.publish(
            package, cut, software, components, deadline, cancelled, None,
        )
    }

    pub fn publish_sign_isolated(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        local_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        assessment_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        limits: tos_validation::assessment::AssessmentLimits,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        crate::source_sign::require_assessment_profile(assessment_worker)?;
        self.current(package, limits.deadline, cancelled)?;
        let mut read = crate::source_sign::SignPromotionRead::select(
            &self.configuration_path,
            package.prepared.context(),
            cut,
            limits.deadline,
            cancelled,
        )?;
        read.prepare_sources(package.prepared.context(), local_worker, limits, cancelled)?;
        crate::source_sign::finish_worker(local_worker, limits.deadline, cancelled)?;
        self.publish(
            package,
            cut,
            software,
            components,
            limits.deadline,
            cancelled,
            Some((&mut read, local_worker, assessment_worker, limits)),
        )
    }

    fn publish(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        sign: Option<(
            &mut crate::source_sign::SignPromotionRead<'_>,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            tos_validation::assessment::AssessmentLimits,
        )>,
    ) -> SourceCommandResult<CreationPublication> {
        if components != package.prepared.components() {
            return Err(SourceCommandError::Conflict(
                "creation sealed software subset differs",
            ));
        }
        if (package.prepared.family() == crate::source_creation::CreationFamily::Sign)
            != sign.is_some()
        {
            return Err(SourceCommandError::Unsupported(
                "Sign publication requires held current assessment journal fences",
            ));
        }
        self.current(package, deadline, cancelled)?;
        package
            .prepared
            .context()
            .check_from_selected_captures(cut, software, components, deadline, cancelled)?;
        let tos = walk(&self.root, "ToS", self.uid)?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current(package, deadline, cancelled)?;
        self.reselect(package, cut, None, false, deadline, cancelled)?;
        self.reselect_components(software, components, deadline, cancelled)?;
        let (parent_path, target_name) = package
            .prepared
            .home()
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("creation target parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        let parent_identity = inode(&owned(&parent, self.uid, true)?);
        let mut stage = PendingCreation::create(&tos, self.uid, deadline, cancelled)?;
        let preparation = (|| {
            for (name, bytes) in package.prepared.files() {
                stage.write(name, bytes, deadline, cancelled)?;
            }
            stage
                .directory
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("creation staging directory fsync"))?;
            let mut publish = |guard: Option<&mut dyn FnMut() -> SourceCommandResult<()>>| {
                self.current(package, deadline, cancelled)?;
                self.reselect(package, cut, Some(&stage.name), false, deadline, cancelled)?;
                self.reselect_components(software, components, deadline, cancelled)?;
                if let Some(guard) = guard {
                    guard()?;
                }
                let current_parent = walk(&self.root, parent_path, self.uid)?;
                if inode(&owned(&current_parent, self.uid, true)?) != parent_identity {
                    return Err(SourceCommandError::Conflict(
                        "creation target parent changed",
                    ));
                }
                // NOREPLACE treats files, directories and symlinks as occupied.
                rustix::fs::renameat_with(
                    &tos,
                    stage.name.as_str(),
                    &parent,
                    target_name,
                    RenameFlags::NOREPLACE,
                )
                .map_err(|error| {
                    if error == Errno::EXIST {
                        SourceCommandError::Conflict("creation target is already occupied")
                    } else {
                        SourceCommandError::Invalid("creation atomic no-replace publication")
                    }
                })?;
                stage.published = true;
                Ok(())
            };
            if let Some((read, local, assessment, limits)) = sign {
                read.with_current_basis(
                    package.prepared.context(),
                    local,
                    assessment,
                    limits,
                    cancelled,
                    |basis, guard| {
                        let request = cmd::parse(&package.prepared.context().request_raw)?;
                        if !cmd::same(
                            cmd::field(cmd::field(&request, "record")?, "promotion_basis")?,
                            basis,
                        )? {
                            return Err(SourceCommandError::Conflict(
                                "Sign promotion basis changed before publication",
                            ));
                        }
                        publish(Some(guard))
                    },
                )
            } else {
                publish(None)
            }
        })();
        if let Err(error) = preparation {
            stage.rollback()?;
            return Err(error);
        }
        // After rename, a failed directory sync is an observed publication with
        // uncertain crash durability. Never misreport it as rolled back.
        let parent_synced = parent.sync_all().is_ok();
        let staging_parent_synced = tos.sync_all().is_ok();
        let durable = parent_synced && staging_parent_synced;
        Ok(CreationPublication {
            home: package.prepared.home().clone(),
            receipt_sha256: Digest256::of_bytes(
                package
                    .prepared
                    .files()
                    .get("source-create-receipt.json")
                    .ok_or(SourceCommandError::Invalid("creation receipt absent"))?,
            ),
            durability: if durable {
                CreationDurability::DirectoriesSynced
            } else {
                CreationDurability::PublishedSyncIncomplete
            },
            replayed: false,
        })
    }

    /// Exact repeat of the original package. A later source/Claim successor
    /// needs the separate maintained history reconstruction route; it is never
    /// treated as the original package by a current-version fallback here.
    pub fn replay_isolated(
        &self,
        package: &SerializedCreation,
        original_base: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        self.replay(
            package,
            original_base,
            software,
            components,
            deadline,
            cancelled,
            None,
        )
    }

    pub fn replay_sign_isolated(
        &self,
        package: &SerializedCreation,
        original_base: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        local_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        assessment_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        limits: tos_validation::assessment::AssessmentLimits,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        crate::source_sign::require_assessment_profile(assessment_worker)?;
        self.current(package, limits.deadline, cancelled)?;
        let mut read = crate::source_sign::SignPromotionRead::select(
            &self.configuration_path,
            package.prepared.context(),
            original_base,
            limits.deadline,
            cancelled,
        )?;
        read.prepare_sources(package.prepared.context(), local_worker, limits, cancelled)?;
        crate::source_sign::finish_worker(local_worker, limits.deadline, cancelled)?;
        self.replay(
            package,
            original_base,
            software,
            components,
            limits.deadline,
            cancelled,
            Some((&mut read, local_worker, assessment_worker, limits)),
        )
    }

    fn replay(
        &self,
        package: &SerializedCreation,
        original_base: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        sign: Option<(
            &mut crate::source_sign::SignPromotionRead<'_>,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            tos_validation::assessment::AssessmentLimits,
        )>,
    ) -> SourceCommandResult<CreationPublication> {
        if components != package.prepared.components() {
            return Err(SourceCommandError::Conflict(
                "creation sealed software subset differs",
            ));
        }
        if (package.prepared.family() == crate::source_creation::CreationFamily::Sign)
            != sign.is_some()
        {
            return Err(SourceCommandError::Unsupported(
                "Sign replay requires current assessment journal fences",
            ));
        }
        if components != package.prepared.components() {
            return Err(SourceCommandError::Conflict(
                "creation sealed software subset differs",
            ));
        }
        self.current(package, deadline, cancelled)?;
        package.prepared.context().check_from_selected_captures(
            original_base,
            software,
            components,
            deadline,
            cancelled,
        )?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current(package, deadline, cancelled)?;
        let replay = |guard: Option<&mut dyn FnMut() -> SourceCommandResult<()>>| {
            self.reselect(package, original_base, None, true, deadline, cancelled)?;
            self.reselect_components(software, components, deadline, cancelled)?;
            if let Some(guard) = guard {
                guard()?;
            }
            let directory = walk(&self.root, package.prepared.home().as_str(), self.uid)?;
            let parent_path = package
                .prepared
                .home()
                .as_str()
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("creation replay parent"))?
                .0;
            let parent = walk(&self.root, parent_path, self.uid)?;
            // Repeating fsync can resolve a prior post-rename durability ambiguity.
            let tos = walk(&self.root, "ToS", self.uid)?;
            let directory_synced = directory.sync_all().is_ok();
            let parent_synced = parent.sync_all().is_ok();
            let staging_parent_synced = tos.sync_all().is_ok();
            let durable = directory_synced && parent_synced && staging_parent_synced;
            Ok(CreationPublication {
                home: package.prepared.home().clone(),
                receipt_sha256: Digest256::of_bytes(
                    package
                        .prepared
                        .files()
                        .get("source-create-receipt.json")
                        .ok_or(SourceCommandError::Invalid(
                            "creation replay receipt absent",
                        ))?,
                ),
                durability: if durable {
                    CreationDurability::DirectoriesSynced
                } else {
                    CreationDurability::PublishedSyncIncomplete
                },
                replayed: true,
            })
        };
        if let Some((read, local, assessment, limits)) = sign {
            read.with_current_basis(
                package.prepared.context(),
                local,
                assessment,
                limits,
                cancelled,
                |basis, guard| {
                    let request = cmd::parse(&package.prepared.context().request_raw)?;
                    if !cmd::same(
                        cmd::field(cmd::field(&request, "record")?, "promotion_basis")?,
                        basis,
                    )? {
                        return Err(SourceCommandError::Conflict(
                            "Sign current promotion basis differs on replay",
                        ));
                    }
                    replay(Some(guard))
                },
            )
        } else {
            replay(None)
        }
    }

    fn reselect_components(
        &self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if components.capture() != software.selection() {
            return Err(SourceCommandError::Conflict(
                "creation current component capture changed",
            ));
        }
        let mut remaining = 16_777_216u64;
        for member in components.members() {
            active(deadline, cancelled)?;
            if member.path.as_str().starts_with("ToS/") || member.size_bytes > remaining {
                return Err(SourceCommandError::Invalid(
                    "creation selected software namespace/aggregate budget",
                ));
            }
            remaining -= member.size_bytes;
            let captured = software
                .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
                .map_err(|_| {
                    SourceCommandError::Conflict("creation selected software custody changed")
                })?;
            let (parent_path, leaf) = member
                .path
                .as_str()
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("creation component parent"))?;
            let parent = walk(&self.root, parent_path, self.uid)?;
            let mut file =
                tos_fd_open::open_regular_at(&parent, Path::new(leaf)).map_err(|_| {
                    SourceCommandError::Conflict("creation current component unavailable")
                })?;
            owned(&file, self.uid, false)?;
            if raw(&mut file, 8_388_608, deadline, cancelled)? != captured {
                return Err(SourceCommandError::Conflict(
                    "creation current producer component differs from selected capture",
                ));
            }
        }
        Ok(())
    }

    fn reselect(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        staging: Option<&str>,
        published: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.reselect_context(
            package.prepared.context(),
            package.prepared.home(),
            package.prepared.files(),
            None,
            cut,
            staging,
            published,
            deadline,
            cancelled,
        )
    }
    fn reselect_context(
        &self,
        context: &cmd::CommandContext,
        home: &RelativePath,
        package_files: &BTreeMap<String, Vec<u8>>,
        operational_sidecars: Option<&BTreeSet<String>>,
        cut: &CorpusCutReader,
        staging: Option<&str>,
        published: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if cut.current().revision() != context.base_revision {
            return Err(SourceCommandError::Conflict("creation base cut changed"));
        }
        let mut observed = BTreeMap::new();
        let mut total = 0usize;
        let mut directories = 0usize;
        let tos = walk(&self.root, "ToS", self.uid)?;
        scan(
            &tos,
            "ToS",
            self.uid,
            staging,
            operational_sidecars,
            &mut observed,
            &mut total,
            &mut directories,
            deadline,
            cancelled,
        )?;
        let mut selected: BTreeMap<_, _> = cut
            .current()
            .members()
            .map(|m| (m.path.as_str().to_owned(), (m.sha256, m.size_bytes, m.mode)))
            .collect();
        if published {
            for (name, bytes) in package_files {
                let path = format!("{}/{name}", home.as_str());
                if selected
                    .insert(
                        path,
                        (Digest256::of_bytes(bytes), bytes.len() as u64, 0o644),
                    )
                    .is_some()
                {
                    return Err(SourceCommandError::Conflict(
                        "creation replay original base already contains target",
                    ));
                }
            }
        }
        if observed != selected {
            return Err(SourceCommandError::Conflict(
                "creation current authored membership/bytes/modes differ from selected base",
            ));
        }
        // Software inputs are selected separately and also reselected from the
        // actual owner tree. A restored capture alone cannot substitute them.
        for input in &context.files {
            active(deadline, cancelled)?;
            if input.path.as_str().starts_with("ToS/") {
                continue;
            }
            let (parent_path, name) = input
                .path
                .as_str()
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("creation software parent"))?;
            let parent = walk(&self.root, parent_path, self.uid)?;
            let mut fd = tos_fd_open::open_regular_at(&parent, Path::new(name)).map_err(|_| {
                SourceCommandError::Conflict("creation current software input unavailable")
            })?;
            owned(&fd, self.uid, false)?;
            if raw(&mut fd, 8_388_608, deadline, cancelled)? != input.raw {
                return Err(SourceCommandError::Conflict(
                    "creation current software input changed",
                ));
            }
        }
        Ok(())
    }
}

fn scan(
    directory: &File,
    prefix: &str,
    uid: u32,
    staging: Option<&str>,
    operational_sidecars: Option<&BTreeSet<String>>,
    files: &mut BTreeMap<String, (Digest256, u64, u32)>,
    total: &mut usize,
    directories: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    *directories = directories
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid(
            "creation directory count overflow",
        ))?;
    if *directories > 8192 {
        return Err(SourceCommandError::Invalid(
            "creation directory traversal budget",
        ));
    }
    let before = owned(directory, uid, true)?;
    // This kernel-owned path addresses the pinned FD, never a request path.
    let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| SourceCommandError::Invalid("creation current directory enumeration"))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        active(deadline, cancelled)?;
        let name = entry
            .map_err(|_| SourceCommandError::Invalid("creation directory entry"))?
            .file_name()
            .into_string()
            .map_err(|_| SourceCommandError::Invalid("creation non-UTF8 source path"))?;
        if names.len() >= MAX_FILES || !names.insert(name) {
            return Err(SourceCommandError::Invalid(
                "creation directory entry budget",
            ));
        }
    }
    for name in names {
        let path = format!("{prefix}/{name}");
        if path == format!("ToS/source-witnesses/{CORPUS_LOCK}")
            || (prefix == "ToS" && staging == Some(name.as_str()))
        {
            continue;
        }
        if path.ends_with(".writer.lock") && operational_sidecars.is_some() {
            if !operational_sidecars.is_some_and(|sidecars| sidecars.contains(&path)) {
                return Err(SourceCommandError::Conflict(
                    "Claim current operational lock path is not delegated",
                ));
            }
            let mut sidecar = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("Claim operational lock unsafe"))?;
            let before = owned(&sidecar, uid, false)?;
            if before.mode() & 0o777 != 0o600
                || !raw(&mut sidecar, 1, deadline, cancelled)?.is_empty()
            {
                return Err(SourceCommandError::Conflict(
                    "Claim operational lock mode or contents changed",
                ));
            }
            let after = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Conflict("Claim operational lock detached"))?;
            if stamp(&before)
                != stamp(
                    &after
                        .metadata()
                        .map_err(|_| SourceCommandError::Invalid("Claim lock metadata"))?,
                )
            {
                return Err(SourceCommandError::Conflict(
                    "Claim operational lock replaced",
                ));
            }
            continue;
        }
        // Directory traversal and file membership are different: weak output
        // parents can contain authored Markdown. Use the existing cut owner's
        // shared component exclusions without reading its excluded payloads.
        if !tos_source_store::has_authored_source_descendants_v1(&path) {
            continue;
        }
        // Secure open rejects symlinks, devices/FIFOs and replaced components.
        if let Ok(child) = tos_fd_open::open_directory_at(directory, Path::new(&name)) {
            if path.split('/').count() > 64 {
                return Err(SourceCommandError::Invalid("creation source depth budget"));
            }
            scan(
                &child,
                &path,
                uid,
                staging,
                operational_sidecars,
                files,
                total,
                directories,
                deadline,
                cancelled,
            )?;
            let now =
                tos_fd_open::open_directory_at(directory, Path::new(&name)).map_err(|_| {
                    SourceCommandError::Conflict("creation traversed directory detached")
                })?;
            if inode(
                &now.metadata()
                    .map_err(|_| SourceCommandError::Invalid("creation child metadata"))?,
            ) != inode(
                &child
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("creation child identity"))?,
            ) {
                return Err(SourceCommandError::Conflict(
                    "creation traversed directory replaced",
                ));
            }
        } else {
            if !tos_source_store::is_authored_source_path_v1(&path) {
                continue;
            }
            let mut file = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("creation current member unsafe"))?;
            let metadata = owned(&file, uid, false)?;
            let bytes = raw(&mut file, 8_388_608, deadline, cancelled)?;
            *total = total
                .checked_add(bytes.len())
                .ok_or(SourceCommandError::Invalid("creation total byte overflow"))?;
            if *total > MAX_BYTES || files.len() >= MAX_FILES {
                return Err(SourceCommandError::Invalid(
                    "creation current source budget",
                ));
            }
            let current = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Conflict("creation current member detached"))?;
            if stamp(
                &current
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("creation member current metadata"))?,
            ) != stamp(&metadata)
            {
                return Err(SourceCommandError::Conflict(
                    "creation current member replaced",
                ));
            }
            files.insert(
                path,
                (
                    Digest256::of_bytes(&bytes),
                    bytes.len() as u64,
                    metadata.mode() & 0o777,
                ),
            );
        }
    }
    let after = owned(directory, uid, true)?;
    if stamp(&before) != stamp(&after) {
        return Err(SourceCommandError::Conflict(
            "creation source directory changed during enumeration",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationDurability {
    DirectoriesSynced,
    PublishedSyncIncomplete,
}
pub struct CreationPublication {
    pub home: RelativePath,
    pub receipt_sha256: Digest256,
    pub durability: CreationDurability,
    pub replayed: bool,
}

/// A complete executable initial-package path in an actually isolated owner
/// corpus. PreparedCommand remains an unauthorised canonical-write proposal.
pub fn execute_isolated_creation_from_captures(
    filesystem: &CreationFilesystem,
    context: &cmd::CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    SerializedCreation,
    CreationPublication,
    tos_foundation::JsonValue,
)> {
    let prepared = crate::source_creation::prepare_source_creation_from_captures(
        context, cut, software, components, worker, deadline, cancelled,
    )?;
    let serialized = prepared.serialize(software, components, worker, deadline, cancelled)?;
    finish_creation_worker(worker, deadline, cancelled)?;
    active(deadline, cancelled)?;
    let publication =
        filesystem.publish_isolated(&serialized, cut, software, components, deadline, cancelled)?;
    let response = serialized.published_result(publication.replayed)?;
    Ok((serialized, publication, response))
}

pub(crate) fn finish_creation_worker(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    worker
        .finish(deadline, cancelled)
        .map_err(|error| match error {
            tos_validation::item_rules::ItemRefusal::Deadline => {
                SourceCommandError::Denied("creation schema operation deadline")
            }
            tos_validation::item_rules::ItemRefusal::Budget
            | tos_validation::item_rules::ItemRefusal::BudgetCheck { .. } => {
                SourceCommandError::Invalid("creation schema operation budget")
            }
            tos_validation::item_rules::ItemRefusal::Source(_)
                if cancelled.load(Ordering::Relaxed) =>
            {
                SourceCommandError::Denied("creation schema operation cancelled")
            }
            _ => SourceCommandError::Unsupported("creation schema operation incomplete"),
        })
}

pub(crate) struct PendingCreation<'a> {
    parent: &'a File,
    pub(crate) directory: File,
    identity: (u64, u64),
    pub(crate) name: String,
    names: BTreeSet<String>,
    pub(crate) published: bool,
}
impl<'a> PendingCreation<'a> {
    pub(crate) fn create(
        parent: &'a File,
        uid: u32,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        for _ in 0..32 {
            active(deadline, cancelled)?;
            let mut entropy = [0u8; 24];
            File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut entropy))
                .map_err(|_| SourceCommandError::Invalid("creation staging entropy unavailable"))?;
            let name = format!(
                ".source-create-{}.pending",
                Digest256::of_bytes(&entropy).to_hex()
            );
            match rustix::fs::mkdirat(parent, name.as_str(), Mode::from_raw_mode(0o700)) {
                Ok(()) => {
                    let directory = child(parent, &name)?;
                    let identity = inode(&owned(&directory, uid, true)?);
                    return Ok(Self {
                        parent,
                        directory,
                        identity,
                        name,
                        names: BTreeSet::new(),
                        published: false,
                    });
                }
                Err(Errno::EXIST) => continue,
                Err(_) => return Err(SourceCommandError::Invalid("creation staging mkdir")),
            }
        }
        Err(SourceCommandError::Conflict(
            "creation staging name collisions",
        ))
    }
    pub(crate) fn write(
        &mut self,
        name: &str,
        bytes: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("creation package file name"))?;
        if path.as_str().contains('/') || self.names.len() >= 40 || bytes.len() > 8_388_608 {
            return Err(SourceCommandError::Invalid(
                "creation package leaf/count/byte budget",
            ));
        }
        active(deadline, cancelled)?;
        let mut file: File = rustix::fs::openat(
            &self.directory,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|_| SourceCommandError::Conflict("creation staging file occupied or unsafe"))?;
        self.names.insert(name.to_owned());
        for block in bytes.chunks(65536) {
            active(deadline, cancelled)?;
            file.write_all(block)
                .map_err(|_| SourceCommandError::Invalid("creation staging write"))?;
        }
        file.set_permissions(Permissions::from_mode(0o644))
            .map_err(|_| SourceCommandError::Invalid("creation package file permissions"))?;
        file.sync_all()
            .map_err(|_| SourceCommandError::Invalid("creation package file fsync"))?;
        let mut check = tos_fd_open::open_regular_at(&self.directory, Path::new(name))
            .map_err(|_| SourceCommandError::Conflict("creation staging readback path"))?;
        if raw(&mut check, 8_388_608, deadline, cancelled)? != bytes {
            return Err(SourceCommandError::Conflict(
                "creation staging readback differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn rollback(&mut self) -> SourceCommandResult<()> {
        if self.published {
            return Ok(());
        }
        let current = child(self.parent, &self.name)?;
        if inode(
            &current
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("creation rollback metadata"))?,
        ) != self.identity
        {
            return Err(SourceCommandError::Conflict(
                "creation rollback directory replaced",
            ));
        }
        for name in &self.names {
            rustix::fs::unlinkat(&self.directory, name.as_str(), AtFlags::empty())
                .map_err(|_| SourceCommandError::Invalid("creation owned pending file cleanup"))?;
        }
        rustix::fs::unlinkat(self.parent, self.name.as_str(), AtFlags::REMOVEDIR).map_err(
            |_| SourceCommandError::Conflict("creation pending directory not empty or replaced"),
        )?;
        self.published = true;
        self.parent
            .sync_all()
            .map_err(|_| SourceCommandError::Invalid("creation pending cleanup fsync"))?;
        Ok(())
    }
}
impl Drop for PendingCreation<'_> {
    fn drop(&mut self) {
        if !self.published {
            let _ = self.rollback();
        }
    }
}
