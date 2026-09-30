//! Flat retained public historical Claim packages, under an exact typed owner.
use super::*;
use std::collections::BTreeSet;

pub(crate) type LegacyPackage = BTreeMap<String, Vec<u8>>;
fn package_digest(files: &LegacyPackage) -> SourceCommandResult<Digest256> {
    let bindings = files
        .iter()
        .map(|(name, raw)| (name, Digest256::of_bytes(raw).to_hex()))
        .collect::<BTreeMap<_, _>>();
    Ok(Digest256::of_bytes(
        &serde_json::to_vec(&bindings).map_err(|_| invalid())?,
    ))
}
/// Retained metadata writers use mode0600 temporary members whose portable
/// authored cut declares0644. This existing writer exception is explicit here.
pub(crate) fn verify_owner_metadata_current_cut(
    root_path: &Path,
    uid: u32,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    verify_metadata_cut(root_path, uid, cut, None, deadline, cancelled)
}
fn verify_metadata_cut(
    root_path: &Path,
    uid: u32,
    cut: &CorpusCutReader,
    auxiliary: Option<&BTreeSet<String>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let root = tos_fd_open::open_absolute_directory(root_path).map_err(|_| invalid())?;
    owned(&root, uid, true)?;
    let tos = walk(&root, "ToS", uid)?;
    let mut observed = BTreeMap::new();
    let mut total = 0;
    let mut directories = 0;
    scan(
        &tos,
        "ToS",
        uid,
        None,
        None,
        auxiliary,
        &mut observed,
        &mut total,
        &mut directories,
        deadline,
        cancelled,
    )?;
    let selected = cut
        .current()
        .members()
        .map(|member| {
            (
                member.path.as_str(),
                (member.sha256, member.size_bytes, member.mode),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if auxiliary.is_some_and(|paths| {
        selected.keys().any(|member| {
            paths.iter().any(|path| {
                *member == path
                    || member
                        .strip_prefix(path.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
            })
        })
    }) {
        return Err(SourceCommandError::Denied(
            "historical auxiliary cannot hide selected authored member",
        ));
    }
    if observed.len() != selected.len()
        || observed.iter().any(|(path, actual)| {
            selected.get(path.as_str()).is_none_or(|expected| {
                actual.0 != expected.0
                    || actual.1 != expected.1
                    || !member_mode_matches(actual.2, expected.2, true)
            })
        })
    {
        return Err(SourceCommandError::Conflict(
            "owner metadata current authored cut differs",
        ));
    }
    Ok(())
}
pub(crate) trait LegacyArchiveReader {
    fn read_archive(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<LegacyPackage>;
}
impl LegacyArchiveReader for LegacyOwnerStore<'_> {
    fn read_archive(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<LegacyPackage> {
        LegacyOwnerStore::read_archive(self, reference, deadline, cancelled)
    }
}
fn invalid() -> SourceCommandError {
    SourceCommandError::Invalid("historical Claim flat transport")
}
fn leaf(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}
fn bounded(files: &LegacyPackage, archive: bool) -> SourceCommandResult<()> {
    if files.is_empty()
        || files.len() > 64 + usize::from(archive)
        || files.keys().any(|name| !leaf(name))
        || files.values().any(|raw| raw.len() > 8_388_608)
        || files
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|n| n > if archive { 16_777_216 } else { 8_388_608 })
    {
        return Err(invalid());
    }
    Ok(())
}
fn read_flat(
    fd: &File,
    uid: u32,
    archive: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<LegacyPackage> {
    let before = stamp(&owned(fd, uid, true)?);
    let mut names = BTreeSet::new();
    let mut total = 0usize;
    let mut selected = BTreeMap::new();
    for entry in
        std::fs::read_dir(format!("/proc/self/fd/{}", fd.as_raw_fd())).map_err(|_| invalid())?
    {
        active(deadline, cancelled)?;
        let name = entry
            .map_err(|_| invalid())?
            .file_name()
            .into_string()
            .map_err(|_| invalid())?;
        if !leaf(&name) || !names.insert(name.clone()) || names.len() > 64 + usize::from(archive) {
            return Err(invalid());
        }
        let member = tos_fd_open::open_regular_at(fd, Path::new(&name)).map_err(|_| invalid())?;
        let meta = owned(&member, uid, false)?;
        if meta.mode() & 0o7777 != 0o600 && meta.mode() & 0o7777 != 0o644 {
            return Err(invalid());
        }
        let size = usize::try_from(meta.len()).map_err(|_| invalid())?;
        total = total.checked_add(size).ok_or(invalid())?;
        if size > 8_388_608 || total > if archive { 16_777_216 } else { 8_388_608 } {
            return Err(invalid());
        }
        selected.insert(name, stamp(&meta));
    }
    let mut files = BTreeMap::new();
    for (name, identity) in selected {
        let mut member =
            tos_fd_open::open_regular_at(fd, Path::new(&name)).map_err(|_| invalid())?;
        if stamp(&owned(&member, uid, false)?) != identity {
            return Err(SourceCommandError::Conflict(
                "historical Claim member changed",
            ));
        }
        let bytes = raw(&mut member, 8_388_608, deadline, cancelled)?;
        let current = tos_fd_open::open_regular_at(fd, Path::new(&name)).map_err(|_| invalid())?;
        if stamp(&owned(&current, uid, false)?) != identity {
            return Err(SourceCommandError::Conflict(
                "historical Claim member path changed",
            ));
        }
        files.insert(name, bytes);
    }
    if stamp(&owned(fd, uid, true)?) != before {
        return Err(SourceCommandError::Conflict(
            "historical Claim package changed",
        ));
    }
    bounded(&files, archive)?;
    Ok(files)
}

pub(crate) struct LegacyOwnerStore<'a> {
    filesystem: &'a CreationFilesystem,
    context: &'a cmd::CommandContext,
    home: String,
    owned_auxiliary: RefCell<BTreeSet<String>>,
    observed_archives: RefCell<BTreeMap<String, Digest256>>,
}
impl<'a> LegacyOwnerStore<'a> {
    pub(crate) fn verify_current_cut(
        &self,
        cut: &CorpusCutReader,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.current(deadline, cancelled)?;
        let auxiliary = self
            .owned_auxiliary
            .borrow()
            .iter()
            .filter(|path| {
                !cut.current().members().any(|member| {
                    member.path.as_str() == path.as_str()
                        || member
                            .path
                            .as_str()
                            .strip_prefix(path.as_str())
                            .is_some_and(|suffix| suffix.starts_with('/'))
                })
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        verify_metadata_cut(
            &self.filesystem.root_path,
            self.filesystem.uid,
            cut,
            Some(&auxiliary),
            deadline,
            cancelled,
        )
    }
    pub(crate) fn select(
        filesystem: &'a CreationFilesystem,
        context: &'a cmd::CommandContext,
    ) -> SourceCommandResult<Self> {
        let config = cmd::parse(&context.configuration_raw)?;
        if !matches!(
            cmd::text(&config, "schema_version")?,
            "tos_local_historical_claim_revision_owner_v1"
                | "tos_local_historical_claim_form_owner_v1"
        ) {
            return Err(SourceCommandError::Denied(
                "historical Claim exact owner family",
            ));
        }
        let source = cmd::text(&config, "source_path")?;
        let home = source
            .strip_suffix("/historical-claims.jsonl")
            .ok_or(invalid())?;
        if !home.starts_with("ToS/source-witnesses/")
            || home
                .split('/')
                .any(|part| !leaf(part) || part.starts_with('.'))
        {
            return Err(invalid());
        }
        Ok(Self {
            filesystem,
            context,
            home: home.to_owned(),
            owned_auxiliary: RefCell::new(BTreeSet::new()),
            observed_archives: RefCell::new(BTreeMap::new()),
        })
    }
    fn current(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        self.filesystem
            .current_context(self.context, deadline, cancelled)
    }
    fn at(
        &self,
        reference: &str,
        archive: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<LegacyPackage> {
        self.current(deadline, cancelled)?;
        let fd = walk(&self.filesystem.root, reference, self.filesystem.uid)?;
        if archive && owned(&fd, self.filesystem.uid, true)?.mode() & 0o7777 != 0o700 {
            return Err(SourceCommandError::Denied(
                "historical Claim archive directory mode",
            ));
        }
        let identity = stamp(&owned(&fd, self.filesystem.uid, true)?);
        let result = read_flat(&fd, self.filesystem.uid, archive, deadline, cancelled)?;
        let current = walk(&self.filesystem.root, reference, self.filesystem.uid)?;
        if stamp(&owned(&current, self.filesystem.uid, true)?) != identity {
            return Err(SourceCommandError::Conflict(
                "historical Claim package path changed",
            ));
        }
        Ok(result)
    }
    pub(crate) fn read_package(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<LegacyPackage> {
        self.at(&self.home, false, deadline, cancelled)
    }
    fn archive_ref(reference: &str) -> SourceCommandResult<()> {
        let suffix = reference
            .strip_prefix("ToS/source-witnesses/.record-revisions/")
            .ok_or(invalid())?;
        if !leaf(suffix) || suffix.starts_with('.') {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn read_archive(
        &self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<LegacyPackage> {
        Self::archive_ref(reference)?;
        let files = self.at(reference, true, deadline, cancelled)?;
        let digest = package_digest(&files)?;
        let mut observed = self.observed_archives.borrow_mut();
        if observed.get(reference).is_some_and(|old| *old != digest) {
            return Err(SourceCommandError::Conflict(
                "historical retained archive changed",
            ));
        }
        if observed.len() >= 128 && !observed.contains_key(reference) {
            return Err(invalid());
        }
        observed.insert(reference.to_owned(), digest);
        Ok(files)
    }
    pub(crate) fn verify_observed_archives(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let observed = self.observed_archives.borrow().clone();
        for (reference, digest) in observed {
            let files = self.at(&reference, true, deadline, cancelled)?;
            if package_digest(&files)? != digest {
                return Err(SourceCommandError::Conflict(
                    "historical retained archive freshness differs",
                ));
            }
        }
        Ok(())
    }
    fn stage(
        &self,
        parent: &File,
        name: &str,
        files: &LegacyPackage,
        archive: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<File> {
        bounded(files, archive)?;
        match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
            Ok(()) | Err(Errno::EXIST) => (),
            Err(_) => return Err(invalid()),
        }
        let stage = child(parent, name)?;
        if owned(&stage, self.filesystem.uid, true)?.mode() & 0o7777 != 0o700 {
            return Err(SourceCommandError::Denied(
                "historical Claim stage directory mode",
            ));
        }
        let mut present = BTreeSet::new();
        for entry in std::fs::read_dir(format!("/proc/self/fd/{}", stage.as_raw_fd()))
            .map_err(|_| invalid())?
        {
            let name = entry
                .map_err(|_| invalid())?
                .file_name()
                .into_string()
                .map_err(|_| invalid())?;
            if !files.contains_key(&name) || !present.insert(name.clone()) {
                return Err(invalid());
            }
            let mut member =
                tos_fd_open::open_regular_at(&stage, Path::new(&name)).map_err(|_| invalid())?;
            owned(&member, self.filesystem.uid, false)?;
            if raw(&mut member, 8_388_608, deadline, cancelled)? != files[&name] {
                return Err(SourceCommandError::Conflict(
                    "historical Claim retained stage differs",
                ));
            }
        }
        for (name, bytes) in files {
            active(deadline, cancelled)?;
            if present.contains(name) {
                continue;
            }
            let mut member: File = rustix::fs::openat(
                &stage,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map(File::from)
            .map_err(|_| invalid())?;
            member.write_all(bytes).map_err(|_| invalid())?;
            member.sync_all().map_err(|_| invalid())?;
        }
        stage.sync_all().map_err(|_| invalid())?;
        if read_flat(&stage, self.filesystem.uid, archive, deadline, cancelled)? != *files {
            return Err(invalid());
        }
        Ok(stage)
    }
    pub(crate) fn publish_successor(
        &self,
        request: &JsonValue,
        before: &LegacyPackage,
        after: &LegacyPackage,
        archive: Option<(&str, &LegacyPackage)>,
        mut stage_guard: impl FnMut() -> SourceCommandResult<()>,
        mut final_guard: impl FnMut() -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        bounded(before, false)?;
        bounded(after, false)?;
        self.current(deadline, cancelled)?;
        let witness = walk(
            &self.filesystem.root,
            "ToS/source-witnesses",
            self.filesystem.uid,
        )?;
        let lock = self.filesystem.lock(&witness, deadline, cancelled)?;
        if self.read_package(deadline, cancelled)? != *before {
            return Err(SourceCommandError::Conflict(
                "historical Claim current bytes differ",
            ));
        }
        let request_bytes = cmd::canonical(request)?;
        let bindings = after
            .iter()
            .map(|(name, raw)| (name.clone(), Digest256::of_bytes(raw).to_hex()))
            .collect::<BTreeMap<_, _>>();
        let stage_selection = serde_json::json!({"source_home":self.home,"request_sha256":Digest256::of_bytes(&request_bytes).to_hex(),"files":bindings});
        let request_digest =
            Digest256::of_bytes(&serde_json::to_vec(&stage_selection).map_err(|_| invalid())?)
                .to_hex();
        let stage_name = format!(".historical-claim-{request_digest}.pending");
        let stage = self.stage(&witness, &stage_name, after, false, deadline, cancelled)?;
        self.owned_auxiliary
            .borrow_mut()
            .insert(format!("ToS/source-witnesses/{stage_name}"));
        if let Some((reference, files)) = archive {
            Self::archive_ref(reference)?;
            if let Ok(archives) = child(&witness, ".record-revisions") {
                let name = reference.rsplit('/').next().ok_or(invalid())?;
                let pending = format!(".archive-{request_digest}.pending");
                match rustix::fs::statat(&archives, pending.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(_) => {
                        self.stage(&archives, &pending, files, true, deadline, cancelled)?;
                        self.owned_auxiliary
                            .borrow_mut()
                            .insert(format!("ToS/source-witnesses/.record-revisions/{pending}"));
                    }
                    Err(Errno::NOENT) => (),
                    Err(_) => return Err(invalid()),
                }
                match rustix::fs::statat(&archives, name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(_) => {
                        if self.read_archive(reference, deadline, cancelled)? != *files {
                            return Err(SourceCommandError::Conflict(
                                "historical Claim retained archive differs",
                            ));
                        }
                        self.owned_auxiliary
                            .borrow_mut()
                            .insert(reference.to_owned());
                    }
                    Err(Errno::NOENT) => (),
                    Err(_) => return Err(invalid()),
                }
            }
        }
        stage_guard()?;
        if let Some((reference, files)) = archive {
            Self::archive_ref(reference)?;
            bounded(files, true)?;
            match rustix::fs::mkdirat(&witness, ".record-revisions", Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(Errno::EXIST) => (),
                Err(_) => return Err(invalid()),
            }
            let archives = child(&witness, ".record-revisions")?;
            owned(&archives, self.filesystem.uid, true)?;
            let name = reference.rsplit('/').next().ok_or(invalid())?;
            match rustix::fs::statat(&archives, name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(_) => {
                    if self.read_archive(reference, deadline, cancelled)? != *files {
                        return Err(SourceCommandError::Conflict(
                            "historical Claim archive differs",
                        ));
                    }
                }
                Err(Errno::NOENT) => {
                    let pending = format!(".archive-{request_digest}.pending");
                    self.stage(&archives, &pending, files, true, deadline, cancelled)?;
                    rustix::fs::renameat_with(
                        &archives,
                        pending.as_str(),
                        &archives,
                        name,
                        RenameFlags::NOREPLACE,
                    )
                    .map_err(|_| invalid())?;
                    archives.sync_all().map_err(|_| invalid())?;
                    self.owned_auxiliary
                        .borrow_mut()
                        .insert(reference.to_owned());
                }
                Err(_) => return Err(invalid()),
            }
        }
        self.current(deadline, cancelled)?;
        if self.read_package(deadline, cancelled)? != *before {
            return Err(SourceCommandError::Conflict(
                "historical Claim changed before exchange",
            ));
        }
        final_guard()?;
        let lock_current = tos_fd_open::open_regular_at(&witness, Path::new(CORPUS_LOCK))
            .map_err(|_| invalid())?;
        if inode(&owned(&lock, self.filesystem.uid, false)?)
            != inode(&owned(&lock_current, self.filesystem.uid, false)?)
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim lock detached",
            ));
        }
        if let Some((reference, files)) = archive {
            if self.read_archive(reference, deadline, cancelled)? != *files {
                return Err(SourceCommandError::Conflict(
                    "historical Claim archive path changed before publication",
                ));
            }
        }
        let stage_current = child(&witness, &stage_name)?;
        if inode(&owned(&stage_current, self.filesystem.uid, true)?)
            != inode(&owned(&stage, self.filesystem.uid, true)?)
            || read_flat(
                &stage_current,
                self.filesystem.uid,
                false,
                deadline,
                cancelled,
            )? != *after
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim stage path changed",
            ));
        }
        let parent_ref = self.home.rsplit_once('/').ok_or(invalid())?.0;
        let name = self.home.rsplit('/').next().ok_or(invalid())?;
        let parent = walk(&self.filesystem.root, parent_ref, self.filesystem.uid)?;
        let parent_identity = inode(&owned(&parent, self.filesystem.uid, true)?);
        let predecessor = child(&parent, name)?;
        let predecessor_identity = inode(&owned(&predecessor, self.filesystem.uid, true)?);
        if read_flat(
            &predecessor,
            self.filesystem.uid,
            false,
            deadline,
            cancelled,
        )? != *before
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim predecessor before exchange",
            ));
        }
        let parent_current = walk(&self.filesystem.root, parent_ref, self.filesystem.uid)?;
        let target_current = child(&parent_current, name)?;
        if inode(&owned(&parent_current, self.filesystem.uid, true)?) != parent_identity
            || inode(&owned(&target_current, self.filesystem.uid, true)?) != predecessor_identity
        {
            return Err(SourceCommandError::Conflict(
                "historical Claim target pathname before exchange",
            ));
        }
        rustix::fs::renameat_with(
            &witness,
            stage_name.as_str(),
            &parent,
            name,
            RenameFlags::EXCHANGE,
        )
        .map_err(|_| invalid())?;
        parent.sync_all().map_err(|_| invalid())?;
        witness.sync_all().map_err(|_| invalid())?;
        if self.read_package(deadline, cancelled)? != *after {
            return Err(SourceCommandError::Conflict(
                "historical Claim published readback differs",
            ));
        }
        if read_flat(&stage, self.filesystem.uid, false, deadline, cancelled)? != *after {
            return Err(invalid());
        }
        let old = child(&witness, &stage_name)?;
        if read_flat(&old, self.filesystem.uid, false, deadline, cancelled)? != *before {
            return Err(SourceCommandError::Conflict(
                "historical Claim exchanged predecessor differs",
            ));
        }
        for member in before.keys() {
            rustix::fs::unlinkat(&old, member.as_str(), AtFlags::empty()).map_err(|_| invalid())?;
        }
        old.sync_all().map_err(|_| invalid())?;
        rustix::fs::unlinkat(&witness, stage_name.as_str(), AtFlags::REMOVEDIR)
            .map_err(|_| invalid())?;
        witness.sync_all().map_err(|_| invalid())?;
        Ok(())
    }
}
