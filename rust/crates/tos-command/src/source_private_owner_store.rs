//! Flat private metadata transport for the maintained owner-local writers.
//! Context routes bytes; typed semantic engines own grants, archives and truth.
use super::*;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(crate) type PrivatePackage = BTreeMap<String, Vec<u8>>;
fn package_digest(files: &PrivatePackage) -> SourceCommandResult<Digest256> {
    let bindings = files
        .iter()
        .map(|(name, raw)| (name, Digest256::of_bytes(raw).to_hex()))
        .collect::<BTreeMap<_, _>>();
    Ok(Digest256::of_bytes(
        &serde_json::to_vec(&bindings).map_err(|_| bad_plan())?,
    ))
}
pub(crate) struct PrivateIdentityInputs {
    pub(crate) files: PrivatePackage,
    pub(crate) visited_entries: usize,
}
pub(crate) trait PrivateArchiveReader {
    fn read_archive(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PrivatePackage>;
}
impl PrivateArchiveReader for PrivateOwnerStore<'_> {
    fn read_archive(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PrivatePackage> {
        PrivateOwnerStore::read_archive(self, reference, deadline, cancelled)
    }
}
const MEMBER_BYTES: usize = 8_388_608;
const PACKAGE_BYTES: usize = 8_388_608;
const ARCHIVE_BYTES: usize = 16_777_216;
const PACKAGE_FILES: usize = 64;

fn private_leaf(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}
fn verify_locks(
    context: &OwnerTextContext,
    locks: &PrivateTextLocks,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    for (path, name, held) in [
        (
            context.public_root().join("ToS/source-witnesses"),
            ".historical-create.writer.lock",
            &locks._historical,
        ),
        (
            context.private_root().to_path_buf(),
            ".native-create.writer.lock",
            &locks._private,
        ),
    ] {
        let parent = tos_fd_open::open_absolute_directory(&path).map_err(|_| bad_plan())?;
        let current =
            tos_fd_open::open_regular_at(&parent, Path::new(name)).map_err(|_| bad_plan())?;
        let expected = stamp(&regular(held, context.account_uid())?);
        let observed = stamp(&regular(&current, context.account_uid())?);
        if (expected.0, expected.1) != (observed.0, observed.1) {
            return Err(SourceCommandError::Conflict(
                "private metadata held lock detached",
            ));
        }
    }
    Ok(())
}
fn bounded(files: &PrivatePackage, archive: bool) -> SourceCommandResult<()> {
    if files.is_empty()
        || files.len() > PACKAGE_FILES + usize::from(archive)
        || files.keys().any(|name| !private_leaf(name))
        || files.values().any(|raw| raw.len() > MEMBER_BYTES)
        || files
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|n| {
                n > if archive {
                    ARCHIVE_BYTES
                } else {
                    PACKAGE_BYTES
                }
            })
    {
        return Err(SourceCommandError::Invalid(
            "private metadata flat package budget",
        ));
    }
    Ok(())
}
fn read_flat(
    directory_fd: &File,
    uid: u32,
    archive: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PrivatePackage> {
    let before = stamp(&directory(directory_fd, uid, true)?);
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}", directory_fd.as_raw_fd()))
        .map_err(|_| bad_plan())?
    {
        active(deadline, cancelled)?;
        let name = entry
            .map_err(|_| bad_plan())?
            .file_name()
            .into_string()
            .map_err(|_| bad_plan())?;
        if names.len() >= PACKAGE_FILES + usize::from(archive)
            || !private_leaf(&name)
            || !names.insert(name)
        {
            return Err(SourceCommandError::Invalid(
                "private metadata package members",
            ));
        }
    }
    if names.is_empty() {
        return Err(SourceCommandError::Invalid(
            "private metadata empty package",
        ));
    }
    let mut inspected = BTreeMap::new();
    let mut total = 0usize;
    for name in &names {
        active(deadline, cancelled)?;
        let file =
            tos_fd_open::open_regular_at(directory_fd, Path::new(name)).map_err(|_| bad_plan())?;
        let metadata = regular(&file, uid)?;
        let len = usize::try_from(metadata.len()).map_err(|_| bad_plan())?;
        if len > MEMBER_BYTES {
            return Err(SourceCommandError::Invalid(
                "private metadata member budget",
            ));
        }
        total = total.checked_add(len).ok_or(bad_plan())?;
        if total
            > if archive {
                ARCHIVE_BYTES
            } else {
                PACKAGE_BYTES
            }
        {
            return Err(SourceCommandError::Invalid(
                "private metadata package byte budget",
            ));
        }
        inspected.insert(name.clone(), stamp(&metadata));
    }
    let mut files = BTreeMap::new();
    for name in names {
        let file =
            tos_fd_open::open_regular_at(directory_fd, Path::new(&name)).map_err(|_| bad_plan())?;
        if inspected.get(&name) != Some(&stamp(&regular(&file, uid)?)) {
            return Err(SourceCommandError::Conflict(
                "private metadata member changed before read",
            ));
        }
        let raw = read_at(directory_fd, &name, uid, MEMBER_BYTES, deadline, cancelled)?
            .ok_or(bad_plan())?;
        let current =
            tos_fd_open::open_regular_at(directory_fd, Path::new(&name)).map_err(|_| bad_plan())?;
        if inspected.get(&name) != Some(&stamp(&regular(&current, uid)?)) {
            return Err(SourceCommandError::Conflict(
                "private metadata member changed during read",
            ));
        }
        files.insert(name, raw);
    }
    if before != stamp(&directory(directory_fd, uid, true)?) {
        return Err(SourceCommandError::Conflict(
            "private metadata package changed during read",
        ));
    }
    bounded(&files, archive)?;
    Ok(files)
}

pub(crate) struct PrivateOwnerStore<'a> {
    context: &'a OwnerTextContext,
    source_ref: String,
    target: PathBuf,
    uid: u32,
    observed_archives: RefCell<BTreeMap<String, Digest256>>,
}
impl<'a> PrivateOwnerStore<'a> {
    /// Current identities beneath the explicitly selected private context.
    /// Hidden archives and staging directories are never competing owners.
    pub(crate) fn read_identity_inputs(
        &self,
        basenames: &BTreeSet<String>,
        exclude_package: Option<&str>,
        include_provenance: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PrivateIdentityInputs> {
        self.current_target(deadline, cancelled)?;
        let excluded = exclude_package
            .map(|reference| self.context.private_new_package_target(reference))
            .transpose()?;
        let mut pending = vec![self.context.private_identity_home()];
        let mut result = BTreeMap::new();
        let mut visited = 0usize;
        let mut remaining = 67_108_864usize;
        let mut observations = Vec::new();
        while let Some(path) = pending.pop() {
            active(deadline, cancelled)?;
            if excluded.as_ref().is_some_and(|target| &path == target) {
                continue;
            }
            let fd = tos_fd_open::open_absolute_directory(&path).map_err(|_| bad_plan())?;
            let before = stamp(&directory(&fd, self.uid, true)?);
            let mut entries = Vec::new();
            for entry in std::fs::read_dir(format!("/proc/self/fd/{}", fd.as_raw_fd()))
                .map_err(|_| bad_plan())?
            {
                visited += 1;
                if visited > 32_768 {
                    return Err(SourceCommandError::Invalid("private identity entry budget"));
                }
                let entry = entry.map_err(|_| bad_plan())?;
                let name = entry.file_name().into_string().map_err(|_| bad_plan())?;
                if name.starts_with('.')
                    || matches!(name.as_str(), "payload" | "local-content" | "catalog")
                {
                    continue;
                }
                entries.push((name, entry.file_type().map_err(|_| bad_plan())?));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, kind) in entries {
                active(deadline, cancelled)?;
                if kind.is_symlink() {
                    return Err(SourceCommandError::Denied("private identity alias"));
                }
                let member = path.join(&name);
                if kind.is_dir() {
                    pending.push(member);
                    continue;
                }
                if !(basenames.contains(&name)
                    || name.ends_with(".human-forms.json")
                    || name.starts_with("semantic-annotation") && name.ends_with(".json")
                    || include_provenance
                        && name.contains("provenance")
                        && name.ends_with(".jsonl"))
                {
                    continue;
                }
                if !kind.is_file() || result.len() >= 2048 {
                    return Err(SourceCommandError::Invalid("private identity file budget"));
                }
                let logical = member
                    .strip_prefix(self.context.private_root())
                    .map_err(|_| bad_plan())?
                    .to_str()
                    .ok_or(bad_plan())?
                    .to_owned();
                let raw =
                    self.context
                        .read(&logical, remaining.min(33_554_432), deadline, cancelled)?;
                remaining = remaining.checked_sub(raw.len()).ok_or(bad_plan())?;
                result.insert(logical, raw);
            }
            if stamp(&directory(&fd, self.uid, true)?) != before {
                return Err(SourceCommandError::Conflict(
                    "private identity directory changed",
                ));
            }
            observations.push((path, before));
        }
        for (path, before) in observations {
            let fd = tos_fd_open::open_absolute_directory(&path).map_err(|_| bad_plan())?;
            if stamp(&directory(&fd, self.uid, true)?) != before {
                return Err(SourceCommandError::Conflict(
                    "private identity namespace changed",
                ));
            }
        }
        self.current_target(deadline, cancelled)?;
        Ok(PrivateIdentityInputs {
            files: result,
            visited_entries: visited,
        })
    }
    /// Claim inventory uses the maintained combined public/private census.
    /// Public bytes remain selected cut evidence; they do not grant private writes.
    pub(crate) fn read_claim_identity_inputs(
        &self,
        basenames: &BTreeSet<String>,
        exclude_package: Option<&str>,
        include_provenance: bool,
        cut: &tos_source_store::CorpusCutReader,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PrivateIdentityInputs> {
        let mut inputs = self.read_identity_inputs(
            basenames,
            exclude_package,
            include_provenance,
            deadline,
            cancelled,
        )?;
        let mut selected_files = inputs.files.len();
        let private_bytes = inputs
            .files
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()).ok_or(bad_plan()))?;
        let mut remaining = 67_108_864usize
            .checked_sub(private_bytes)
            .ok_or(bad_plan())?;
        let root_path = self.context.public_root().to_path_buf();
        let root = tos_fd_open::open_absolute_directory(&root_path).map_err(|_| bad_plan())?;
        let root_identity = stamp(&directory(&root, self.uid, false)?);
        let mut pending = vec![root_path.join("ToS/source-witnesses")];
        let mut observations = Vec::new();
        while let Some(path) = pending.pop() {
            active(deadline, cancelled)?;
            let fd = tos_fd_open::open_absolute_directory(&path).map_err(|_| bad_plan())?;
            let before = stamp(&directory(&fd, self.uid, false)?);
            for entry in std::fs::read_dir(format!("/proc/self/fd/{}", fd.as_raw_fd()))
                .map_err(|_| bad_plan())?
            {
                active(deadline, cancelled)?;
                inputs.visited_entries = inputs.visited_entries.checked_add(1).ok_or(bad_plan())?;
                if inputs.visited_entries > 32_768 {
                    return Err(SourceCommandError::Invalid("Claim identity entry budget"));
                }
                let entry = entry.map_err(|_| bad_plan())?;
                let name = entry.file_name().into_string().map_err(|_| bad_plan())?;
                if name.starts_with('.')
                    || matches!(name.as_str(), "payload" | "local-content" | "catalog")
                {
                    continue;
                }
                let kind = entry.file_type().map_err(|_| bad_plan())?;
                if kind.is_symlink() {
                    return Err(SourceCommandError::Denied("Claim public identity alias"));
                }
                let member = path.join(&name);
                if kind.is_dir() {
                    pending.push(member);
                    continue;
                }
                if !(basenames.contains(&name)
                    || name.ends_with(".human-forms.json")
                    || name.starts_with("semantic-annotation") && name.ends_with(".json")
                    || include_provenance
                        && name.contains("provenance")
                        && name.ends_with(".jsonl"))
                {
                    continue;
                }
                selected_files = selected_files.checked_add(1).ok_or(bad_plan())?;
                if !kind.is_file() || selected_files > 2048 {
                    return Err(SourceCommandError::Invalid("Claim identity file budget"));
                }
                let reference = member
                    .strip_prefix(&root_path)
                    .map_err(|_| bad_plan())?
                    .to_str()
                    .ok_or(bad_plan())?;
                let selected_path =
                    tos_foundation::RelativePath::parse(reference).map_err(|_| bad_plan())?;
                let selected =
                    cut.current()
                        .member(&selected_path)
                        .ok_or(SourceCommandError::Denied(
                            "Claim public identity outside selected cut",
                        ))?;
                let raw =
                    self.context
                        .read(reference, remaining.min(33_554_432), deadline, cancelled)?;
                if raw.len() as u64 != selected.size_bytes
                    || Digest256::of_bytes(&raw) != selected.sha256
                {
                    return Err(SourceCommandError::Conflict(
                        "Claim public identity differs from cut",
                    ));
                }
                remaining = remaining.checked_sub(raw.len()).ok_or(bad_plan())?;
            }
            if stamp(&directory(&fd, self.uid, false)?) != before {
                return Err(SourceCommandError::Conflict(
                    "Claim public identity directory changed",
                ));
            }
            observations.push((path, before));
        }
        for (path, before) in observations {
            let fd = tos_fd_open::open_absolute_directory(&path).map_err(|_| bad_plan())?;
            if stamp(&directory(&fd, self.uid, false)?) != before {
                return Err(SourceCommandError::Conflict(
                    "Claim public identity namespace changed",
                ));
            }
        }
        let current_root =
            tos_fd_open::open_absolute_directory(&root_path).map_err(|_| bad_plan())?;
        if stamp(&directory(&current_root, self.uid, false)?) != root_identity {
            return Err(SourceCommandError::Conflict(
                "Claim public owner root changed",
            ));
        }
        self.current_target(deadline, cancelled)?;
        Ok(inputs)
    }

    pub(crate) fn select(
        context: &'a OwnerTextContext,
        source_ref: &str,
    ) -> SourceCommandResult<Self> {
        let target = context.private_new_package_target(source_ref)?;
        Ok(Self {
            context,
            source_ref: source_ref.to_owned(),
            target,
            uid: context.account_uid(),
            observed_archives: RefCell::new(BTreeMap::new()),
        })
    }
    fn current_target(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        self.context.snapshot(deadline, cancelled)?;
        if self.context.private_new_package_target(&self.source_ref)? != self.target {
            return Err(SourceCommandError::Conflict(
                "private metadata target changed",
            ));
        }
        Ok(())
    }
    fn at(
        &self,
        target: &Path,
        archive: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<PrivatePackage>> {
        self.current_target(deadline, cancelled)?;
        let parent = tos_fd_open::open_absolute_directory(target.parent().ok_or(bad_plan())?)
            .map_err(|_| bad_plan())?;
        directory(&parent, self.uid, true)?;
        let name = target
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| private_leaf(n))
            .ok_or(bad_plan())?;
        match rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(Errno::NOENT) => return Ok(None),
            Err(_) => return Err(bad_plan()),
            Ok(_) => (),
        }
        let retained = child(&parent, name, self.uid)?;
        let identity = stamp(&directory(&retained, self.uid, true)?);
        let files = read_flat(&retained, self.uid, archive, deadline, cancelled)?;
        let reselected = child(&parent, name, self.uid)?;
        if stamp(&directory(&reselected, self.uid, true)?) != identity {
            return Err(SourceCommandError::Conflict(
                "private metadata pathname changed during read",
            ));
        }
        self.current_target(deadline, cancelled)?;
        Ok(Some(files))
    }
    pub(crate) fn read_package(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<PrivatePackage>> {
        self.at(&self.target, false, deadline, cancelled)
    }
    fn archive_target(&self, archive_ref: &str) -> SourceCommandResult<PathBuf> {
        // Existing archive names are computed by the semantic owner. Context
        // still refuses another store or a public route. No path discovery.
        let probe = format!("{archive_ref}/manifest.json");
        let target = self.context.private_new_package_target(&probe)?;
        if !target.starts_with(
            self.context
                .private_identity_home()
                .join(".record-revisions"),
        ) {
            return Err(SourceCommandError::Denied(
                "private metadata archive owner namespace",
            ));
        }
        Ok(target)
    }
    pub(crate) fn read_archive(
        &self,
        archive_ref: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PrivatePackage> {
        let files = self
            .at(
                &self.archive_target(archive_ref)?,
                true,
                deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "private metadata predecessor archive absent",
            ))?;
        let digest = package_digest(&files)?;
        let mut observed = self.observed_archives.borrow_mut();
        if observed.get(archive_ref).is_some_and(|old| *old != digest) {
            return Err(SourceCommandError::Conflict(
                "private retained archive changed",
            ));
        }
        if observed.len() >= 128 && !observed.contains_key(archive_ref) {
            return Err(bad_plan());
        }
        observed.insert(archive_ref.to_owned(), digest);
        Ok(files)
    }
    pub(crate) fn verify_observed_archives(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let observed = self.observed_archives.borrow().clone();
        for (reference, digest) in observed {
            let files = self
                .at(&self.archive_target(&reference)?, true, deadline, cancelled)?
                .ok_or(bad_plan())?;
            if package_digest(&files)? != digest {
                return Err(SourceCommandError::Conflict(
                    "private retained archive freshness differs",
                ));
            }
        }
        Ok(())
    }
    fn root(&self) -> SourceCommandResult<File> {
        let root = tos_fd_open::open_absolute_directory(self.context.private_root())
            .map_err(|_| bad_plan())?;
        directory(&root, self.uid, true)?;
        Ok(root)
    }
    fn stage(
        &self,
        request: &JsonValue,
        files: &PrivatePackage,
        archive: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(File, String, File)> {
        bounded(files, archive)?;
        let label = cmd::canonical(&cmd::object(vec![
            ("source", cmd::string(&self.source_ref)),
            ("request", request.clone()),
            ("archive", JsonValue::Bool(archive)),
            ("files", crate::source_revisions::file_refs(files, false)),
        ]))?;
        let name = format!(
            ".owner-source-{}.pending",
            Digest256::of_bytes(&label).to_hex()
        );
        let root = self.root()?;
        match rustix::fs::mkdirat(&root, name.as_str(), Mode::from_raw_mode(0o700)) {
            Ok(()) => root.sync_all().map_err(|_| bad_plan())?,
            Err(Errno::EXIST) => (),
            Err(_) => return Err(bad_plan()),
        }
        let stage = child(&root, &name, self.uid)?;
        let expected = files.keys().map(String::as_str).collect::<Vec<_>>();
        enumerate(&stage, &expected, true, deadline, cancelled)?;
        for (member, raw) in files {
            match read_at(&stage, member, self.uid, MEMBER_BYTES, deadline, cancelled)? {
                Some(observed) if observed == *raw => (),
                Some(_) => {
                    return Err(SourceCommandError::Conflict(
                        "private metadata pending bytes differ",
                    ));
                }
                None => write_new(&stage, member, raw, self.uid, deadline, cancelled)?,
            }
        }
        if read_flat(&stage, self.uid, archive, deadline, cancelled)? != *files {
            return Err(SourceCommandError::Conflict(
                "private metadata staged bytes differ",
            ));
        }
        stage.sync_all().map_err(|_| bad_plan())?;
        Ok((root, name, stage))
    }
    fn archive(
        &self,
        request: &JsonValue,
        reference: &str,
        files: &PrivatePackage,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        bounded(files, true)?;
        let identity_home = self.context.private_identity_home();
        let identity =
            tos_fd_open::open_absolute_directory(&identity_home).map_err(|_| bad_plan())?;
        directory(&identity, self.uid, true)?;
        match rustix::fs::mkdirat(&identity, ".record-revisions", Mode::from_raw_mode(0o700)) {
            Ok(()) => identity.sync_all().map_err(|_| bad_plan())?,
            Err(Errno::EXIST) => (),
            Err(_) => return Err(bad_plan()),
        }
        let target = self.archive_target(reference)?;
        let parent_path = target.parent().ok_or(bad_plan())?;
        let parent = tos_fd_open::open_absolute_directory(parent_path).map_err(|_| bad_plan())?;
        directory(&parent, self.uid, true)?;
        if let Some(existing) = self.at(&target, true, deadline, cancelled)? {
            if existing == *files {
                return Ok(());
            }
            return Err(SourceCommandError::Conflict(
                "private metadata existing archive differs",
            ));
        }
        let (root, stage_name, stage) = self.stage(request, files, true, deadline, cancelled)?;
        let target_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(bad_plan())?;
        self.current_target(deadline, cancelled)?;
        if read_flat(&stage, self.uid, true, deadline, cancelled)? != *files {
            return Err(bad_plan());
        }
        rustix::fs::renameat_with(
            &root,
            stage_name.as_str(),
            &parent,
            target_name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|_| {
            SourceCommandError::Conflict("private metadata archive publication occupied")
        })?;
        parent.sync_all().map_err(|_| bad_plan())?;
        root.sync_all().map_err(|_| bad_plan())?;
        if self.at(&target, true, deadline, cancelled)?.as_ref() != Some(files) {
            return Err(bad_plan());
        }
        Ok(())
    }
    pub(crate) fn publish_new(
        &self,
        request: &JsonValue,
        files: &PrivatePackage,
        mut stage_guard: impl FnMut() -> SourceCommandResult<()>,
        mut final_guard: impl FnMut() -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let locks = PrivateTextLocks::acquire(self.context, deadline, cancelled)?;
        stage_guard()?;
        self.current_target(deadline, cancelled)?;
        if self.read_package(deadline, cancelled)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "private metadata creation occupied",
            ));
        }
        let (root, name, stage) = self.stage(request, files, false, deadline, cancelled)?;
        let parent = tos_fd_open::open_absolute_directory(self.target.parent().ok_or(bad_plan())?)
            .map_err(|_| bad_plan())?;
        let identity = stamp(&directory(&parent, self.uid, true)?);
        final_guard()?;
        verify_locks(self.context, &locks, deadline, cancelled)?;
        self.current_target(deadline, cancelled)?;
        let current = tos_fd_open::open_absolute_directory(self.target.parent().ok_or(bad_plan())?)
            .map_err(|_| bad_plan())?;
        if stamp(&directory(&parent, self.uid, true)?) != identity
            || stamp(&directory(&current, self.uid, true)?) != identity
            || read_flat(&stage, self.uid, false, deadline, cancelled)? != *files
        {
            return Err(bad_plan());
        }
        rustix::fs::renameat_with(
            &root,
            name.as_str(),
            &parent,
            self.target
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or(bad_plan())?,
            RenameFlags::NOREPLACE,
        )
        .map_err(|_| {
            SourceCommandError::Conflict(
                "private metadata creation publication occupied or uncertain",
            )
        })?;
        parent.sync_all().map_err(|_| bad_plan())?;
        root.sync_all().map_err(|_| bad_plan())?;
        if self.read_package(deadline, cancelled)?.as_ref() != Some(files) {
            return Err(bad_plan());
        }
        Ok(())
    }
    pub(crate) fn publish_successor(
        &self,
        request: &JsonValue,
        before: &PrivatePackage,
        after: &PrivatePackage,
        archive: Option<(&str, &PrivatePackage)>,
        mut stage_guard: impl FnMut() -> SourceCommandResult<()>,
        mut final_guard: impl FnMut() -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        bounded(before, false)?;
        bounded(after, false)?;
        let locks = PrivateTextLocks::acquire(self.context, deadline, cancelled)?;
        stage_guard()?;
        self.current_target(deadline, cancelled)?;
        if self.read_package(deadline, cancelled)?.as_ref() != Some(before) {
            return Err(SourceCommandError::Conflict(
                "private metadata predecessor changed",
            ));
        }
        if let Some((reference, files)) = archive {
            verify_locks(self.context, &locks, deadline, cancelled)?;
            self.archive(request, reference, files, deadline, cancelled)?;
        }
        let (root, name, stage) = self.stage(request, after, false, deadline, cancelled)?;
        let parent_path = self.target.parent().ok_or(bad_plan())?;
        let parent = tos_fd_open::open_absolute_directory(parent_path).map_err(|_| bad_plan())?;
        let parent_identity = stamp(&directory(&parent, self.uid, true)?);
        let target_name = self
            .target
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(bad_plan())?;
        let predecessor = child(&parent, target_name, self.uid)?;
        let predecessor_identity = stamp(&directory(&predecessor, self.uid, true)?);
        final_guard()?;
        verify_locks(self.context, &locks, deadline, cancelled)?;
        self.current_target(deadline, cancelled)?;
        let current_parent =
            tos_fd_open::open_absolute_directory(parent_path).map_err(|_| bad_plan())?;
        if stamp(&directory(&parent, self.uid, true)?) != parent_identity
            || stamp(&directory(&current_parent, self.uid, true)?) != parent_identity
            || stamp(&directory(
                &child(&parent, target_name, self.uid)?,
                self.uid,
                true,
            )?) != predecessor_identity
            || read_flat(&predecessor, self.uid, false, deadline, cancelled)? != *before
            || read_flat(&stage, self.uid, false, deadline, cancelled)? != *after
        {
            return Err(bad_plan());
        }
        if let Some((reference, files)) = archive {
            if self.read_archive(reference, deadline, cancelled)? != *files {
                return Err(bad_plan());
            }
        }
        rustix::fs::renameat_with(
            &root,
            name.as_str(),
            &parent,
            target_name,
            RenameFlags::EXCHANGE,
        )
        .map_err(|_| SourceCommandError::Conflict("private metadata exchange uncertain"))?;
        parent.sync_all().map_err(|_| bad_plan())?;
        root.sync_all().map_err(|_| bad_plan())?;
        if self.read_package(deadline, cancelled)?.as_ref() != Some(after)
            || read_flat(&stage, self.uid, false, deadline, cancelled)? != *before
        {
            return Err(bad_plan());
        }
        // Only the exact held predecessor is removed after successor and, for
        // record/Claim revisions, archived evidence have been reread.
        for member in before.keys() {
            active(deadline, cancelled)?;
            rustix::fs::unlinkat(&stage, member.as_str(), AtFlags::empty())
                .map_err(|_| bad_plan())?;
        }
        stage.sync_all().map_err(|_| bad_plan())?;
        rustix::fs::unlinkat(&root, name.as_str(), AtFlags::REMOVEDIR).map_err(|_| bad_plan())?;
        root.sync_all().map_err(|_| bad_plan())?;
        Ok(())
    }
}
