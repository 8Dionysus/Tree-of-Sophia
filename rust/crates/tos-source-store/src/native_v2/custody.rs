//! Descriptor-rooted source custody shared by exact readers and command writers.
//! Opening and verifying this handle does not publish or admit a source.
use super::work::{active, invalid};
use crate::{CorpusReader, PinnedSqliteIoBudget, ReadLimits};
use rustix::fs::{Mode, OFlags, openat};
use rustix::io::Errno;
use std::{
    cell::RefCell,
    fs::File,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher};
const V2_LAYOUT_COMPONENT_METADATA_BYTES: u64 = 4096;
const V2_LAYOUT_PATH_BYTES: usize = 4096;
const V2_LAYOUT_PATH_COMPONENTS: usize = 128;
pub fn v2_component_guard_bytes(name: &[u8]) -> io::Result<u64> {
    u64::try_from(name.len())
        .map_err(|_| invalid("V2 layout component length exceeds range"))?
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(V2_LAYOUT_COMPONENT_METADATA_BYTES))
        .ok_or_else(|| invalid("V2 layout component guard overflow"))
}
pub fn charge_v2_component_guard(io: &crate::PinnedSqliteIoBudget, name: &str) -> io::Result<()> {
    io.charge_read_upper_bound(v2_component_guard_bytes(name.as_bytes())?)
        .map_err(invalid)
}
pub fn charge_v2_layout_guard(io: &crate::PinnedSqliteIoBudget, path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.as_os_str().as_encoded_bytes().len() > V2_LAYOUT_PATH_BYTES
        || path.components().count() > V2_LAYOUT_PATH_COMPONENTS
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(invalid("V2 layout path is not bounded and normalized"));
    }
    let relative_names = ["objects", "revisions", "staging", "segments-v2"]
        .into_iter()
        .try_fold(0u64, |total, name| {
            let component = v2_component_guard_bytes(name.as_bytes())?;
            total
                .checked_add(component)
                .ok_or_else(|| invalid("V2 layout guard overflow"))
        })?;
    // Reopening the root consumes its name and metadata, exactly like the
    // child namespaces. No 64 KiB file is read by this layout fence. The
    // existing metadata allowance covers owned_directory plus both identity
    // observations; keep every original openat2 and inode comparison below.
    let upper = v2_component_guard_bytes(path.as_os_str().as_encoded_bytes())?
        .checked_add(relative_names)
        .ok_or_else(|| invalid("V2 layout guard overflow"))?;
    io.charge_read_upper_bound(upper).map_err(invalid)
}
pub fn identity(file: &File) -> io::Result<(u64, u64)> {
    let m = file.metadata()?;
    Ok((m.dev(), m.ino()))
}
pub fn owned_directory(file: File) -> io::Result<File> {
    let m = file.metadata()?;
    if !m.is_dir() || m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o022 != 0 {
        return Err(invalid("corpus write directory ownership or mode differs"));
    }
    Ok(file)
}
pub fn optional_directory(parent: &File, name: &str) -> io::Result<Option<File>> {
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
pub struct NativeSourceCustody {
    pub path: PathBuf,
    pub root: File,
    pub objects: File,
    pub revisions: File,
    pub staging: File,
    // Created only by the writer; readers retain this exact existing namespace.
    pub v2_segments: RefCell<Option<File>>,
    // The selected invocation ledger also accounts every later layout fence.
    pub v2_layout_io: RefCell<Option<PinnedSqliteIoBudget>>,
}
impl NativeSourceCustody {
    pub fn retained_path_capacity(&self) -> usize {
        self.path.capacity()
    }
    /// Open an existing store without creating namespaces.
    pub fn open_existing(path: &Path, deadline: Instant, cancel: &AtomicBool) -> io::Result<Self> {
        active(deadline, cancel)?;
        if !path.is_absolute() {
            return Err(invalid("corpus store must be absolute"));
        }
        let root = owned_directory(tos_fd_open::open_absolute_directory(path).map_err(invalid)?)?;
        Self::open_existing_at_named(path, &root, deadline, cancel)
    }
    pub fn open_existing_with_io(
        path: &Path,
        io: crate::PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        active(deadline, cancel)?;
        charge_v2_layout_guard(&io, path)?;
        let root = owned_directory(tos_fd_open::open_absolute_directory(path).map_err(invalid)?)?;
        Self::open_existing_at_named_with_io(path, &root, io, deadline, cancel)
    }
    /// Hold namespaces below the supplied descriptor and fence name substitution.
    pub fn open_existing_at_named(
        path: &Path,
        held_root: &File,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        Self::open_existing_at_named_inner(path, held_root, None, deadline, cancel)
    }
    pub fn open_existing_at_named_with_io(
        path: &Path,
        held_root: &File,
        io: crate::PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        Self::open_existing_at_named_inner(path, held_root, Some(io), deadline, cancel)
    }
    fn open_existing_at_named_inner(
        path: &Path,
        held_root: &File,
        io: Option<crate::PinnedSqliteIoBudget>,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<Self> {
        active(deadline, cancel)?;
        if let Some(io) = io.as_ref() {
            charge_v2_layout_guard(io, path)?;
        } else if !path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(invalid("corpus store path must be absolute and normalized"));
        }
        // A protected root may be an O_PATH anchor. Open a readable
        // description of that exact inode; cloning would preserve O_PATH
        // and make the backup walk fail at getdents64. A fresh description
        // also gives each store its own directory cursor.
        if let Some(io) = io.as_ref() {
            io.charge_read_upper_bound(v2_component_guard_bytes(b".")?)
                .map_err(invalid)?;
        }
        let root = owned_directory(tos_fd_open::reopen_directory(held_root).map_err(invalid)?)?;
        let named = owned_directory(tos_fd_open::open_absolute_directory(path).map_err(invalid)?)?;
        if identity(&named)? != identity(&root)? {
            return Err(invalid("named corpus root differs from held root"));
        }
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

            v2_layout_io: RefCell::new(io),
        };
        store.verify_layout()?;
        active(deadline, cancel)?;
        Ok(store)
    }
    /// Borrow the physical namespaces; callers must recheck layout after use.
    pub fn backup_namespaces(&self) -> io::Result<(&File, &File, &File)> {
        self.verify_layout()?;
        Ok((&self.root, &self.objects, &self.revisions))
    }
    pub fn reader(&self, limits: ReadLimits) -> io::Result<CorpusReader> {
        self.verify_layout()?;
        let reader = CorpusReader::open_from_held_namespaces(
            &self.root,
            &self.objects,
            &self.revisions,
            limits,
        )
        .map_err(invalid)?;
        self.verify_layout()?;
        Ok(reader)
    }
    pub fn current_selection(
        &self,
        limits: ReadLimits,
        deadline: Instant,
        cancel: &AtomicBool,
        io_budget: Option<&crate::PinnedSqliteIoBudget>,
    ) -> io::Result<Option<crate::CorpusCurrentSelection>> {
        if let Some(io) = self.v2_layout_io.borrow().as_ref() {
            let selected =
                io_budget.ok_or_else(|| invalid("V2 selector lacks its original IO ledger"))?;
            if !io.shares_with(selected) {
                return Err(invalid("V2 selector and layout IO ledgers differ"));
            }
            charge_v2_component_guard(io, "current.json")?;
        }
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
    pub fn read_v2_rootset(
        &self,
        revision: Digest256,
        expected_sha256: Digest256,
        max_bytes: usize,
        deadline: Instant,
        cancel: &AtomicBool,
        io_budget: &crate::PinnedSqliteIoBudget,
    ) -> io::Result<Vec<u8>> {
        active(deadline, cancel)?;
        if max_bytes == 0 || max_bytes == usize::MAX {
            return Err(invalid("V2 rootset read profile is invalid"));
        }
        charge_v2_component_guard(io_budget, &revision.to_hex())?;
        charge_v2_component_guard(io_budget, "rootset-v2.json")?;
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
        let mut tail = [0u8; 1];
        let tail_read = loop {
            active(deadline, cancel)?;
            io_budget.charge_read(1).map_err(invalid)?;
            match file.read(&mut tail) {
                Ok(read) => {
                    io_budget
                        .record_read_returned(read as u64)
                        .map_err(invalid)?;
                    break read;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        };
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
    pub fn verify_layout(&self) -> io::Result<()> {
        if let Some(io) = self.v2_layout_io.borrow().as_ref() {
            charge_v2_layout_guard(io, &self.path)?;
        }
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
    /// Verify an exact typed source artifact under the held immutable revision
    /// directory. This is used only when recovering an already accepted
    /// publication from the authenticated V2 history tree; it creates no new
    /// publication receipt or persistent allocation custody.
    pub fn verify_v2_source_artifact(
        &self,
        revision: Digest256,
        artifact: &super::roots::SourceRevisionArtifactV2,
        max_bytes: u64,
        deadline: Instant,
        cancel: &AtomicBool,
        io_budget: &crate::PinnedSqliteIoBudget,
    ) -> io::Result<()> {
        active(deadline, cancel)?;
        let layout_io = self
            .v2_layout_io
            .borrow()
            .clone()
            .ok_or_else(|| invalid("V2 source artifact lacks its original IO ledger"))?;
        if !layout_io.shares_with(io_budget) || max_bytes == 0 || max_bytes == u64::MAX {
            return Err(invalid("V2 source artifact read profile differs"));
        }
        let size = artifact
            .bytes()
            .ok_or_else(|| invalid("legacy V2 source artifact has no typed byte count"))?;
        if size == 0 || size > max_bytes {
            return Err(invalid("V2 source artifact exceeds selected read bound"));
        }
        if matches!(
            artifact,
            super::roots::SourceRevisionArtifactV2::CompactCommitV2 { .. }
                | super::roots::SourceRevisionArtifactV2::CompactPackedV2 { .. }
        ) && size > super::roots::MAX_COMPACT_COMMIT_V2_BYTES as u64
        {
            return Err(invalid("compact source artifact exceeds its wire bound"));
        }
        let revision_name = revision.to_hex();
        let filename = artifact.filename();
        charge_v2_component_guard(io_budget, &revision_name)?;
        charge_v2_component_guard(io_budget, filename)?;
        self.verify_layout()?;
        let directory = tos_fd_open::open_directory_at(&self.revisions, Path::new(&revision_name))
            .map_err(invalid)?;
        let mut file =
            tos_fd_open::open_regular_at(&directory, Path::new(filename)).map_err(invalid)?;
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
        let before = file.metadata()?;
        if !before.is_file()
            || before.uid() != rustix::process::geteuid().as_raw()
            || before.mode() & 0o222 != 0
            || before.len() != size
        {
            return Err(invalid("V2 source artifact custody differs"));
        }
        let mut hash = Digest256Hasher::new();
        let mut remaining = size;
        let mut block = [0u8; 65_536];
        while remaining != 0 {
            active(deadline, cancel)?;
            let count = remaining.min(block.len() as u64) as usize;
            let mut filled = 0usize;
            while filled < count {
                active(deadline, cancel)?;
                io_budget
                    .charge_read((count - filled) as u64)
                    .map_err(invalid)?;
                match file.read(&mut block[filled..count]) {
                    Ok(0) => return Err(invalid("V2 source artifact ended before its size")),
                    Ok(read) => {
                        io_budget
                            .record_read_returned(read as u64)
                            .map_err(invalid)?;
                        hash.update(&block[filled..filled + read]);
                        filled += read;
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
            remaining -= count as u64;
        }
        active(deadline, cancel)?;
        io_budget.charge_read(1).map_err(invalid)?;
        let mut tail = [0u8; 1];
        let tail_read = file.read(&mut tail)?;
        io_budget
            .record_read_returned(tail_read as u64)
            .map_err(invalid)?;
        let after = file.metadata()?;
        let selected =
            tos_fd_open::open_regular_at(&directory, Path::new(filename)).map_err(invalid)?;
        if tail_read != 0
            || stamp(&after) != stamp(&before)
            || stamp(&selected.metadata()?) != stamp(&before)
            || hash.finalize() != artifact.sha256()
        {
            return Err(invalid("V2 source artifact digest, EOF or stamp differs"));
        }
        active(deadline, cancel)?;
        self.verify_layout()?;
        Ok(())
    }

    /// Open only the existing selected segment namespace; never initialize it.
    pub fn open_segment_store_v2(
        &self,
        limits: tos_segment_store::SegmentLimits,
        io: Arc<dyn tos_segment_store::AuthenticatedTreeIoLedgerV1>,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> io::Result<tos_segment_store::SegmentStore> {
        active(deadline, cancel)?;
        self.verify_layout()?;
        let root = self
            .v2_segments
            .borrow()
            .as_ref()
            .map(File::try_clone)
            .transpose()?
            .ok_or_else(|| invalid("selected V2 segment namespace absent"))?;
        let store = tos_segment_store::SegmentStore::open_existing_at_with_io(
            &root, limits, io, deadline, cancel,
        )
        .map_err(invalid)?;
        self.verify_layout()?;
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::fs::DirBuilderExt, time::Duration};
    #[test]
    fn held_path_anchor_opens_readable_independent_store_directory() {
        let make_store = |path: &Path| {
            std::fs::DirBuilder::new().mode(0o700).create(path).unwrap();
            for name in ["objects", "revisions", "staging"] {
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(path.join(name))
                    .unwrap();
            }
        };
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("store");
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        make_store(&path);
        let held = File::from(
            rustix::fs::open(
                &path,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        );
        let first =
            NativeSourceCustody::open_existing_at_named(&path, &held, deadline, &cancel).unwrap();
        let second =
            NativeSourceCustody::open_existing_at_named(&path, &held, deadline, &cancel).unwrap();
        let collect = |store: &NativeSourceCustody| {
            let (root, _, _) = store.backup_namespaces().unwrap();
            assert_eq!(identity(root).unwrap(), identity(&held).unwrap());
            let mut buffer = [std::mem::MaybeUninit::uninit(); 8192];
            let mut entries = rustix::fs::RawDir::new(root, &mut buffer);
            let mut names = std::collections::BTreeSet::new();
            while let Some(entry) = entries.next() {
                names.insert(entry.unwrap().file_name().to_str().unwrap().to_owned());
            }
            names
        };
        let names = collect(&first);
        assert!(
            names.contains("objects") && names.contains("revisions") && names.contains("staging")
        );
        assert_eq!(collect(&second), names);
        // A held inode must still match the requested named store.
        let other = temporary.path().join("other");
        make_store(&other);
        assert!(
            NativeSourceCustody::open_existing_at_named(&other, &held, deadline, &cancel).is_err()
        );
    }
}
