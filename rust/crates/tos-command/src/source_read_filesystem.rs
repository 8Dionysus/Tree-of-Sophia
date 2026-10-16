//! Read-only live source custody. No writer configuration or source admission.
use super::{CORPUS_LOCK, active, inode, owned, raw, stamp, walk, work_transaction};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use std::collections::BTreeMap;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, RelativePath};

struct Observation {
    reference: String,
    offset: Option<u64>,
    file: File,
    stamp: (u64, u64, u64, i64, i64, i64, i64),
    digest: Digest256,
    bytes: usize,
}
/// The actual cooperating source mutex remains held until this owner is dropped.
/// Currentness rereads authenticate noncooperating path/byte changes as well.
pub(crate) struct SourceReadFilesystem {
    path: PathBuf,
    root: File,
    root_identity: (u64, u64),
    witness: File,
    lock: File,
    uid: u32,
    publication: work_transaction::PublicationSnapshot,
    observed: BTreeMap<String, Observation>,
    directories: BTreeMap<String, ((u64, u64), Vec<(String, bool)>)>,
    bytes_read: usize,
    max_bytes: usize,
    max_files: usize,
}
impl SourceReadFilesystem {
    pub(crate) fn open(
        path: &Path,
        max_files: usize,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        if max_files == 0 || max_files > 4096 || max_bytes == 0 || max_bytes > 67_108_864 {
            return Err(SourceCommandError::Invalid("source read custody limits"));
        }
        let uid = rustix::process::getuid().as_raw();
        let root = tos_fd_open::open_absolute_directory(path)
            .map_err(|_| SourceCommandError::Denied("source read root descriptor"))?;
        let root_identity = inode(&owned(&root, uid, true)?);
        let witness = walk(&root, "ToS/source-witnesses", uid)?;
        let lock = tos_fd_open::open_regular_at(&witness, Path::new(CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("source read mutex unavailable"))?;
        owned(&lock, uid, false)?;
        loop {
            active(deadline, cancelled)?;
            match rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(rustix::io::Errno::AGAIN) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => return Err(SourceCommandError::Denied("source read mutex unsupported")),
            }
        }
        let publication =
            work_transaction::PublicationSnapshot::select_at(&root, uid, deadline, cancelled)?;
        Ok(Self {
            path: path.to_owned(),
            root,
            root_identity,
            witness,
            lock,
            uid,
            publication,
            observed: BTreeMap::new(),
            directories: BTreeMap::new(),
            bytes_read: 0,
            max_bytes,
            max_files,
        })
    }
    pub(crate) fn source_root(&self) -> &Path {
        &self.path
    }
    pub(crate) fn effective_uid(&self) -> u32 {
        self.uid
    }
    pub(crate) fn publication(&self) -> (Option<&str>, u64) {
        (
            self.publication.token.as_deref(),
            self.publication.generation,
        )
    }
    pub(crate) fn bytes_read(&self) -> usize {
        self.bytes_read
    }
    fn charge(&mut self, n: usize) -> SourceCommandResult<()> {
        self.bytes_read = self
            .bytes_read
            .checked_add(n)
            .filter(|n| *n <= self.max_bytes)
            .ok_or(SourceCommandError::Unsupported(
                "source read aggregate byte budget",
            ))?;
        Ok(())
    }
    pub(crate) fn list_directory(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        active(deadline, cancelled)?;
        let directory = walk(&self.root, reference, self.uid)?;
        let identity = inode(&owned(&directory, self.uid, true)?);
        let names = Self::directory_names(&directory, self.max_files, deadline, cancelled)?;
        if let Some((old_identity, old_names)) = self.directories.get(reference) {
            if old_identity != &identity || old_names != &names {
                return Err(SourceCommandError::Conflict(
                    "source read directory changed",
                ));
            }
        } else {
            if self.directories.len() >= self.max_files {
                return Err(SourceCommandError::Unsupported(
                    "source read directory budget",
                ));
            }
            self.directories
                .insert(reference.to_owned(), (identity, names.clone()));
        }
        Ok(names)
    }
    fn directory_names(
        directory: &File,
        max: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        use std::os::fd::AsRawFd;
        let mut result = Vec::new();
        for entry in std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Denied("source read directory inventory"))?
        {
            active(deadline, cancelled)?;
            if result.len() >= max {
                return Err(SourceCommandError::Unsupported(
                    "source read directory entry budget",
                ));
            }
            let entry =
                entry.map_err(|_| SourceCommandError::Denied("source read directory entry"))?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Denied("source read directory name UTF8"))?;
            let kind = entry
                .file_type()
                .map_err(|_| SourceCommandError::Denied("source read directory entry type"))?;
            if !kind.is_file() && !kind.is_dir() {
                return Err(SourceCommandError::Denied("source read special member"));
            }
            result.push((name, kind.is_dir()));
        }
        result.sort();
        Ok(result)
    }
    fn selected_file(&self, reference: &str) -> SourceCommandResult<File> {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Denied("source read relative path"))?;
        let (parent, leaf) = path
            .as_str()
            .rsplit_once('/')
            .map_or((None, path.as_str()), |(p, l)| (Some(p), l));
        let directory = match parent {
            Some(p) => walk(&self.root, p, self.uid)?,
            None => tos_fd_open::reopen_directory(&self.root)
                .map_err(|_| SourceCommandError::Denied("source read root reopen"))?,
        };
        let file = tos_fd_open::open_regular_at(&directory, Path::new(leaf))
            .map_err(|_| SourceCommandError::Conflict("source read selected member unavailable"))?;
        owned(&file, self.uid, false)?;
        Ok(file)
    }
    pub(crate) fn read(
        &mut self,
        reference: &str,
        cap: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        active(deadline, cancelled)?;
        if cap == 0 || cap > 8_388_608 {
            return Err(SourceCommandError::Invalid("source read member budget"));
        }
        if !self.observed.contains_key(reference) && self.observed.len() >= self.max_files {
            return Err(SourceCommandError::Unsupported("source read file budget"));
        }
        let mut file = self.selected_file(reference)?;
        let before = stamp(&owned(&file, self.uid, false)?);
        let bytes = raw(&mut file, cap, deadline, cancelled)?;
        self.charge(bytes.len())?;
        let digest = Digest256::of_bytes(&bytes);
        if let Some(old) = self.observed.get(reference) {
            if old.stamp != before || old.digest != digest || old.bytes != bytes.len() {
                return Err(SourceCommandError::Conflict(
                    "source read selected bytes changed",
                ));
            }
        } else {
            self.observed.insert(
                reference.to_owned(),
                Observation {
                    reference: reference.to_owned(),
                    offset: None,
                    file,
                    stamp: before,
                    digest,
                    bytes: bytes.len(),
                },
            );
        }
        Ok(bytes)
    }
    pub(crate) fn read_range(
        &mut self,
        reference: &str,
        offset: u64,
        length: usize,
        expected_file_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        active(deadline, cancelled)?;
        if length == 0
            || length > 1_048_576
            || offset
                .checked_add(length as u64)
                .is_none_or(|end| end > expected_file_bytes)
        {
            return Err(SourceCommandError::Invalid("source slot range budget"));
        }
        let key = format!("{reference}\0{offset}:{length}");
        if !self.observed.contains_key(&key) && self.observed.len() >= self.max_files {
            return Err(SourceCommandError::Unsupported(
                "source read file/range budget",
            ));
        }
        let file = self.selected_file(reference)?;
        let before = stamp(&owned(&file, self.uid, false)?);
        if before.2 != expected_file_bytes {
            return Err(SourceCommandError::Conflict(
                "source slot file size changed",
            ));
        }
        let bytes = Self::range_bytes(&file, offset, length, deadline, cancelled)?;
        if stamp(&owned(&file, self.uid, false)?) != before {
            return Err(SourceCommandError::Conflict("source slot range changed"));
        }
        self.charge(bytes.len())?;
        let digest = Digest256::of_bytes(&bytes);
        if let Some(old) = self.observed.get(&key) {
            if old.stamp != before || old.digest != digest {
                return Err(SourceCommandError::Conflict(
                    "source slot selection changed",
                ));
            }
        } else {
            self.observed.insert(
                key,
                Observation {
                    reference: reference.to_owned(),
                    offset: Some(offset),
                    file,
                    stamp: before,
                    digest,
                    bytes: length,
                },
            );
        }
        Ok(bytes)
    }
    fn range_bytes(
        file: &File,
        offset: u64,
        length: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        let mut bytes = vec![0u8; length];
        let mut n = 0;
        while n < length {
            active(deadline, cancelled)?;
            let read = file
                .read_at(&mut bytes[n..length.min(n + 65536)], offset + n as u64)
                .map_err(|_| SourceCommandError::Conflict("source slot range IO"))?;
            if read == 0 {
                return Err(SourceCommandError::Conflict("source slot range short"));
            }
            n += read;
        }
        Ok(bytes)
    }
    pub(crate) fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        let current_root = tos_fd_open::open_absolute_directory(&self.path)
            .map_err(|_| SourceCommandError::Conflict("source read root detached"))?;
        if inode(&owned(&current_root, self.uid, true)?) != self.root_identity {
            return Err(SourceCommandError::Conflict("source read root replaced"));
        }
        let current_lock = tos_fd_open::open_regular_at(&self.witness, Path::new(CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("source read mutex detached"))?;
        if inode(&owned(&current_lock, self.uid, false)?)
            != inode(&owned(&self.lock, self.uid, false)?)
        {
            return Err(SourceCommandError::Conflict("source read mutex replaced"));
        }
        self.publication
            .verify_at(&self.root, self.uid, deadline, cancelled)?;
        for (name, (expected, names)) in &self.directories {
            active(deadline, cancelled)?;
            let directory = walk(&self.root, name, self.uid)?;
            if inode(&owned(&directory, self.uid, true)?) != *expected
                || Self::directory_names(&directory, self.max_files, deadline, cancelled)? != *names
            {
                return Err(SourceCommandError::Conflict(
                    "source read directory membership changed",
                ));
            }
        }
        let names = self.observed.keys().cloned().collect::<Vec<_>>();
        for name in names {
            active(deadline, cancelled)?;
            let old = &self.observed[&name];
            if stamp(&owned(&old.file, self.uid, false)?) != old.stamp {
                return Err(SourceCommandError::Conflict(
                    "source read held member changed",
                ));
            }
            let mut current = self.selected_file(&old.reference)?;
            if stamp(&owned(&current, self.uid, false)?) != old.stamp {
                return Err(SourceCommandError::Conflict(
                    "source read named member replaced",
                ));
            }
            let bytes = match old.offset {
                Some(offset) => {
                    Self::range_bytes(&current, offset, old.bytes, deadline, cancelled)?
                }
                None => raw(&mut current, old.bytes.max(1), deadline, cancelled)?,
            };
            let expected = old.digest;
            if stamp(&owned(&current, self.uid, false)?) != old.stamp {
                return Err(SourceCommandError::Conflict(
                    "source read member changed during final read",
                ));
            }
            self.charge(bytes.len())?;
            if Digest256::of_bytes(&bytes) != expected {
                return Err(SourceCommandError::Conflict(
                    "source read member digest changed",
                ));
            }
        }
        Ok(())
    }
}

impl crate::source_revisions::ReadonlyRecordFiles for SourceReadFilesystem {
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        Self::read(self, path, max_bytes, deadline, cancelled)
    }
    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>> {
        Self::list_directory(self, path, deadline, cancelled)
    }
}
impl crate::source_read_layers::SourceLayerRead for SourceReadFilesystem {
    fn source_root(&self) -> &Path {
        &self.path
    }
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        Self::read(self, path, max_bytes, deadline, cancelled)
    }
    fn read_range(
        &mut self,
        path: &str,
        offset: u64,
        length: usize,
        expected_file_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        Self::read_range(
            self,
            path,
            offset,
            length,
            expected_file_bytes,
            deadline,
            cancelled,
        )
    }
    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        Self::verify_current(self, deadline, cancelled)
    }
}
impl crate::source_native_text_read::SignNativeRead for SourceReadFilesystem {
    fn read(
        &mut self,
        path: &str,
        kind: crate::source_native_text_read::NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if matches!(
            kind,
            crate::source_native_text_read::NativeReadKind::Support
        ) && !(path.starts_with("ToS/")
            || ["LICENSE", "NOTICE", "README", "README.md"].contains(&path))
        {
            return Err(SourceCommandError::Denied("native public support path"));
        }
        Self::read(self, path, max_bytes, deadline, cancelled)
    }
    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        Self::verify_current(self, deadline, cancelled)
    }
    fn owner_local(&self, path: &str) -> SourceCommandResult<bool> {
        Ok(path.split('/').any(|p| p == "owner-local"))
    }
}
impl crate::source_native_text_read::NativeUnitRead for SourceReadFilesystem {
    fn source_root(&self) -> &Path {
        &self.path
    }
}
