//! Protected CLI selection for existing initial source creation families.
//! Family validation remains with the authored creation engine.
use super::*;

impl CreationFilesystem {
    pub(crate) fn select_creation_owner(
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(Self, Vec<u8>)> {
        let selected =
            Self::select_protected_native_owner(configuration_path, deadline, cancelled)?;
        let config = cmd::parse(&selected.1)?;
        crate::source_creation::CreationFamily::parse(cmd::text(&config, "schema_version")?)?;
        Ok(selected)
    }
    pub(crate) fn creation_target_exists(
        &self,
        context: &cmd::CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<bool> {
        self.current_context(context, deadline, cancelled)?;
        let config = cmd::parse(&context.configuration_raw)?;
        let source = cmd::text(&config, "source_path")?;
        let home = source
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("creation source home"))?
            .0;
        let (parent_path, name) = home
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("creation source parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        match rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(Errno::NOENT) => Ok(false),
            Err(_) => Err(SourceCommandError::Denied("creation source target unsafe")),
        }
    }

    /// Read only the complete expected initial package, retaining descriptor
    /// identity across metadata preflight and body reads. Later revisions
    /// belong to their history reader and are refused by this initial route.
    pub(crate) fn read_creation_retained(
        &self,
        prepared: &crate::source_creation::PreparedCreation,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<BTreeMap<String, Vec<u8>>>> {
        self.current_context(prepared.context(), deadline, cancelled)?;
        let (parent_path, name) = prepared
            .home()
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("creation retained parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        match rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(Errno::NOENT) => return Ok(None),
            Err(_) => {
                return Err(SourceCommandError::Denied(
                    "creation retained target unsafe",
                ));
            }
            Ok(_) => {}
        }
        let directory = walk(&self.root, prepared.home().as_str(), self.uid)?;
        let before = stamp(&owned(&directory, self.uid, true)?);
        let mut expected: BTreeSet<String> = prepared.files().keys().cloned().collect();
        expected.insert("source-create-receipt.json".into());
        if prepared.family() != crate::source_creation::CreationFamily::HistoricalV1 {
            expected.extend(
                [
                    "source-create-request.json",
                    "source-create-environment.json",
                    "source-create-provenance.jsonl",
                ]
                .into_iter()
                .map(str::to_owned),
            );
        }
        let mut names = BTreeSet::new();
        for entry in std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Invalid("creation retained listing"))?
        {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| SourceCommandError::Invalid("creation retained entry"))?
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Invalid("creation retained name"))?;
            if names.len() >= expected.len() || !expected.contains(&name) || !names.insert(name) {
                return Err(SourceCommandError::Conflict(
                    "creation retained package members differ",
                ));
            }
        }
        if names != expected {
            return Err(SourceCommandError::Conflict(
                "creation retained package incomplete",
            ));
        }
        let mut metadata = BTreeMap::new();
        let mut total = 0u64;
        for name in &names {
            active(deadline, cancelled)?;
            let file = tos_fd_open::open_regular_at(&directory, Path::new(name))
                .map_err(|_| SourceCommandError::Denied("creation retained member unsafe"))?;
            let observed = owned(&file, self.uid, false)?;
            if observed.mode() & 0o7777 != 0o644 || observed.len() > 8_388_608 {
                return Err(SourceCommandError::Invalid(
                    "creation retained member mode or byte budget",
                ));
            }
            total = total
                .checked_add(observed.len())
                .ok_or(SourceCommandError::Invalid(
                    "creation retained byte overflow",
                ))?;
            if total > 33_554_432 {
                return Err(SourceCommandError::Invalid(
                    "creation retained total byte budget",
                ));
            }
            metadata.insert(name.clone(), stamp(&observed));
        }
        let mut files = BTreeMap::new();
        for name in names {
            let mut file = tos_fd_open::open_regular_at(&directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("creation retained member unsafe"))?;
            if metadata.get(&name) != Some(&stamp(&owned(&file, self.uid, false)?)) {
                return Err(SourceCommandError::Conflict(
                    "creation retained member changed",
                ));
            }
            let bytes = raw(&mut file, 8_388_608, deadline, cancelled)?;
            if metadata.get(&name) != Some(&stamp(&owned(&file, self.uid, false)?)) {
                return Err(SourceCommandError::Conflict(
                    "creation retained member changed during read",
                ));
            }
            files.insert(name, bytes);
        }
        if before != stamp(&owned(&directory, self.uid, true)?) {
            return Err(SourceCommandError::Conflict(
                "creation retained directory changed",
            ));
        }
        self.current_context(prepared.context(), deadline, cancelled)?;
        Ok(Some(files))
    }
}
