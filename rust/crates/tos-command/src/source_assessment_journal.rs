//! Protected current assessment history and compatible sorted subject flocks.
//! This reader never creates judgments or imports admission_at_commit as a grant.

use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, inode, protected_configuration_parents, raw};
use rustix::fs::{FlockOperation, Mode, OFlags};
use rustix::io::Errno;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const BATCH_SCHEMA: &str = "ToS/contracts/knowledge-assessment-batch.schema.json";
const MAX_EVENTS: usize = 1024;
const MAX_BATCH_BYTES: usize = 1_048_576;
const MAX_HISTORY_BYTES: usize = 33_554_432;

fn protected(file: &File, uid: u32, directory: bool) -> SourceCommandResult<()> {
    let m = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("assessment owner metadata"))?;
    if ![0, uid].contains(&m.uid())
        || m.mode() & 0o022 != 0
        || m.is_dir() != directory
        || !directory && !m.is_file()
    {
        return Err(SourceCommandError::Denied("assessment owner path boundary"));
    }
    Ok(())
}

fn protected_private(file: &File, uid: u32, directory: bool) -> SourceCommandResult<()> {
    protected(file, uid, directory)?;
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("assessment private metadata"))?;
    let expected_mode = if directory { 0o700 } else { 0o600 };
    if metadata.uid() != uid || metadata.mode() & 0o7777 != expected_mode {
        return Err(SourceCommandError::Denied(
            "assessment private owner mode or account",
        ));
    }
    Ok(())
}

/// Selected only from the actual protected assessment configuration. No public
/// constructor accepts a journal, ready flag, submission list or head claim.
pub(crate) struct ProtectedAssessmentJournal {
    configuration_path: PathBuf,
    configuration_raw: Vec<u8>,
    configuration: JsonValue,
    configuration_file: File,
    configuration_identity: (u64, u64),
    configuration_parent_path: PathBuf,
    configuration_parent: File,
    configuration_parent_identity: (u64, u64),
    directory_path: PathBuf,
    directory: File,
    identity: (u64, u64),
    directory_parent_path: PathBuf,
    directory_parent: File,
    directory_parent_identity: (u64, u64),
    uid: u32,
    private_root: Option<PathBuf>,
    private_root_file: Option<File>,
    private_root_identity: Option<(u64, u64)>,
}
impl ProtectedAssessmentJournal {
    pub(crate) fn select(
        configuration_path: &Path,
        source_root: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::select_public(configuration_path, source_root, false, deadline, cancelled)
    }

    /// Direct public Journal owner selection; the Sign selector above remains
    /// restricted to its source-bound v2/v3 profiles.
    pub(crate) fn select_public_command(
        configuration_path: &Path,
        source_root: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::select_public(configuration_path, source_root, true, deadline, cancelled)
    }

    fn select_public(
        configuration_path: &Path,
        source_root: &Path,
        allow_inline: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        let uid = rustix::process::getuid().as_raw();
        if uid != rustix::process::geteuid().as_raw() {
            return Err(SourceCommandError::Denied(
                "assessment refuses setuid context",
            ));
        }
        protected_configuration_parents(configuration_path, uid)?;
        let mut file = tos_fd_open::open_absolute_regular(configuration_path, 8_388_608)
            .map_err(|_| SourceCommandError::Denied("assessment protected configuration open"))?;
        protected(&file, uid, false)?;
        let configuration_raw = raw(&mut file, 8_388_608, deadline, cancelled)?;
        let configuration = cmd::parse(&configuration_raw)?;
        let (source_bound, native) = match cmd::text(&configuration, "schema_version")? {
            "tos_local_assessment_owner_v1" if allow_inline => (false, false),
            "tos_local_assessment_owner_v2" => (true, false),
            "tos_local_assessment_owner_v3" => (true, true),
            _ => {
                return Err(SourceCommandError::Denied(
                    "Sign assessment owner v2/v3 required",
                ));
            }
        };
        let mut keys = vec![
            "schema_version",
            "uid",
            "principal_id",
            "execution_profile",
            "policy",
            "authorities",
            "competencies",
            "records",
            "subjects",
            "journal_directory",
        ];
        if source_bound {
            keys.extend(["source_root", "source_records"]);
        }
        if native {
            keys.push("native_text_units");
        }
        cmd::exact_keys(&configuration, &keys)?;
        if cmd::integer(&configuration, "uid")? != u64::from(uid)
            || tos_foundation::python_strip_unicode16_v1(
                cmd::text(&configuration, "principal_id")?,
                8_388_608,
            )
            .map_err(|_| SourceCommandError::Invalid("assessment principal Unicode budget"))?
            .is_empty()
            || source_bound && Path::new(cmd::text(&configuration, "source_root")?) != source_root
        {
            return Err(SourceCommandError::Denied(
                "assessment current account/source root differs",
            ));
        }
        let directory_path = PathBuf::from(cmd::text(&configuration, "journal_directory")?);
        protected_configuration_parents(&directory_path, uid)?;
        let directory = tos_fd_open::open_absolute_directory(&directory_path)
            .map_err(|_| SourceCommandError::Denied("assessment protected journal directory"))?;
        if directory_path == Path::new("/") {
            return Err(SourceCommandError::Denied(
                "assessment dedicated owner directory required",
            ));
        }
        protected(&directory, uid, true)?;
        let identity = inode(
            &directory
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment directory identity"))?,
        );
        let configuration_identity = inode(
            &file
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment configuration identity"))?,
        );
        let configuration_parent_path = configuration_path
            .parent()
            .ok_or(SourceCommandError::Invalid(
                "assessment configuration parent",
            ))?
            .to_path_buf();
        let configuration_parent = tos_fd_open::open_absolute_directory(&configuration_parent_path)
            .map_err(|_| SourceCommandError::Denied("assessment configuration parent"))?;
        protected(&configuration_parent, uid, true)?;
        let configuration_parent_identity =
            inode(&configuration_parent.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment configuration parent identity")
            })?);
        let directory_parent_path = directory_path
            .parent()
            .ok_or(SourceCommandError::Invalid("assessment journal parent"))?
            .to_path_buf();
        let directory_parent = tos_fd_open::open_absolute_directory(&directory_parent_path)
            .map_err(|_| SourceCommandError::Denied("assessment journal parent"))?;
        protected(&directory_parent, uid, true)?;
        let directory_parent_identity = inode(
            &directory_parent
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment journal parent identity"))?,
        );
        let selected = Self {
            configuration_path: configuration_path.to_owned(),
            configuration_raw,
            configuration,
            configuration_file: file,
            configuration_identity,
            configuration_parent_path,
            configuration_parent,
            configuration_parent_identity,
            directory_path,
            directory,
            identity,
            directory_parent_path,
            directory_parent,
            directory_parent_identity,
            uid,
            private_root: None,
            private_root_file: None,
            private_root_identity: None,
        };
        selected.verify_current(deadline, cancelled)?;
        Ok(selected)
    }

    /// Select the separate confidential v4/v5/v6 assessment owner profile.
    /// The Sign v2/v3 selector above deliberately remains unchanged.
    pub(crate) fn select_owner_local(
        configuration_path: &Path,
        source_context_ref: &Path,
        private_root: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        let uid = rustix::process::getuid().as_raw();
        if uid != rustix::process::geteuid().as_raw() {
            return Err(SourceCommandError::Denied(
                "assessment private route refuses setuid context",
            ));
        }
        protected_configuration_parents(configuration_path, uid)?;
        let mut file = tos_fd_open::open_absolute_regular(configuration_path, 8_388_608)
            .map_err(|_| SourceCommandError::Denied("assessment private configuration open"))?;
        protected_private(&file, uid, false)?;
        let configuration_raw = raw(&mut file, 8_388_608, deadline, cancelled)?;
        let configuration = cmd::parse(&configuration_raw)?;
        let version = cmd::text(&configuration, "schema_version")?;
        if !matches!(
            version,
            "tos_local_assessment_owner_v4"
                | "tos_local_assessment_owner_v5"
                | "tos_local_assessment_owner_v6"
        ) {
            return Err(SourceCommandError::Denied(
                "private assessment owner v4/v5/v6 required",
            ));
        }
        let mut keys = vec![
            "schema_version",
            "uid",
            "principal_id",
            "execution_profile",
            "policy",
            "authorities",
            "competencies",
            "records",
            "subjects",
            "journal_directory",
            "source_context_ref",
            "source_records",
            "owner_local_source_records",
            "native_text_units",
        ];
        if configuration
            .object_get("owner_local_source_claims")
            .is_some()
        {
            keys.push("owner_local_source_claims");
        }
        if matches!(
            version,
            "tos_local_assessment_owner_v5" | "tos_local_assessment_owner_v6"
        ) {
            keys.extend(["native_text_layers", "quality_dependencies"]);
        }
        cmd::exact_keys(&configuration, &keys)?;
        if cmd::integer(&configuration, "uid")? != u64::from(uid)
            || tos_foundation::python_strip_unicode16_v1(
                cmd::text(&configuration, "principal_id")?,
                8_388_608,
            )
            .map_err(|_| SourceCommandError::Invalid("assessment principal Unicode budget"))?
            .is_empty()
            || Path::new(cmd::text(&configuration, "source_context_ref")?) != source_context_ref
        {
            return Err(SourceCommandError::Denied(
                "assessment private account or source context differs",
            ));
        }
        let private_root = crate::source_text_owner::normalized_absolute(
            private_root
                .to_str()
                .ok_or(SourceCommandError::Invalid("assessment private root UTF-8"))?,
        )?;
        let directory_path = crate::source_text_owner::normalized_absolute(cmd::text(
            &configuration,
            "journal_directory",
        )?)?;
        if directory_path == private_root || !directory_path.starts_with(&private_root) {
            return Err(SourceCommandError::Denied(
                "assessment journal leaves private source root",
            ));
        }
        protected_configuration_parents(&directory_path, uid)?;
        let private_root_handle = tos_fd_open::open_absolute_directory(&private_root)
            .map_err(|_| SourceCommandError::Denied("assessment private root"))?;
        protected_private(&private_root_handle, uid, true)?;
        let private_root_identity = inode(
            &private_root_handle
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment private root identity"))?,
        );
        let directory = tos_fd_open::open_absolute_directory(&directory_path)
            .map_err(|_| SourceCommandError::Denied("assessment private journal directory"))?;
        protected_private(&directory, uid, true)?;
        let identity =
            inode(&directory.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment private directory identity")
            })?);
        let configuration_identity = inode(&file.metadata().map_err(|_| {
            SourceCommandError::Invalid("assessment private configuration identity")
        })?);
        let configuration_parent_path = configuration_path
            .parent()
            .ok_or(SourceCommandError::Invalid(
                "assessment private config parent",
            ))?
            .to_path_buf();
        let configuration_parent = tos_fd_open::open_absolute_directory(&configuration_parent_path)
            .map_err(|_| SourceCommandError::Denied("assessment private config parent"))?;
        protected_private(&configuration_parent, uid, true)?;
        let configuration_parent_identity =
            inode(&configuration_parent.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment private config parent identity")
            })?);
        let directory_parent_path = directory_path
            .parent()
            .ok_or(SourceCommandError::Invalid(
                "assessment private journal parent",
            ))?
            .to_path_buf();
        let directory_parent = tos_fd_open::open_absolute_directory(&directory_parent_path)
            .map_err(|_| SourceCommandError::Denied("assessment private journal parent"))?;
        protected_private(&directory_parent, uid, true)?;
        let directory_parent_identity = inode(&directory_parent.metadata().map_err(|_| {
            SourceCommandError::Invalid("assessment private journal parent identity")
        })?);
        let selected = Self {
            configuration_path: configuration_path.to_owned(),
            configuration_raw,
            configuration,
            configuration_file: file,
            configuration_identity,
            configuration_parent_path,
            configuration_parent,
            configuration_parent_identity,
            directory_path,
            directory,
            identity,
            directory_parent_path,
            directory_parent,
            directory_parent_identity,
            uid,
            private_root: Some(private_root),
            private_root_file: Some(private_root_handle),
            private_root_identity: Some(private_root_identity),
        };
        selected.verify_current(deadline, cancelled)?;
        Ok(selected)
    }
    pub(crate) fn configuration(&self) -> &JsonValue {
        &self.configuration
    }
    pub(crate) fn configuration_raw(&self) -> &[u8] {
        &self.configuration_raw
    }
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        if rustix::process::getuid().as_raw() != self.uid
            || rustix::process::geteuid().as_raw() != self.uid
        {
            return Err(SourceCommandError::Denied("assessment account changed"));
        }
        protected_configuration_parents(&self.configuration_path, self.uid)?;
        let mut file = tos_fd_open::open_absolute_regular(&self.configuration_path, 8_388_608)
            .map_err(|_| SourceCommandError::Conflict("assessment configuration unavailable"))?;
        if self.private_root.is_some() {
            protected_private(&file, self.uid, false)?;
            protected_private(&self.configuration_file, self.uid, false)?;
        } else {
            protected(&file, self.uid, false)?;
            protected(&self.configuration_file, self.uid, false)?;
        }
        if raw(&mut file, 8_388_608, deadline, cancelled)? != self.configuration_raw {
            return Err(SourceCommandError::Conflict(
                "assessment protected configuration changed",
            ));
        }
        if inode(
            &file
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment configuration identity"))?,
        ) != self.configuration_identity
            || inode(
                &self.configuration_file.metadata().map_err(|_| {
                    SourceCommandError::Invalid("assessment retained config identity")
                })?,
            ) != self.configuration_identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment configuration identity changed",
            ));
        }
        let retained_config_parent = self
            .configuration_parent
            .metadata()
            .map_err(|_| SourceCommandError::Invalid("assessment retained config parent"))?;
        let current_config_parent = tos_fd_open::open_absolute_directory(
            &self.configuration_parent_path,
        )
        .map_err(|_| SourceCommandError::Conflict("assessment configuration parent changed"))?;
        if inode(&retained_config_parent) != self.configuration_parent_identity
            || inode(
                &current_config_parent.metadata().map_err(|_| {
                    SourceCommandError::Invalid("assessment config parent identity")
                })?,
            ) != self.configuration_parent_identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment configuration parent identity changed",
            ));
        }
        let configuration_name = self
            .configuration_path
            .file_name()
            .ok_or(SourceCommandError::Invalid("assessment config filename"))?;
        let named_config =
            tos_fd_open::open_regular_at(&self.configuration_parent, Path::new(configuration_name))
                .map_err(|_| {
                    SourceCommandError::Conflict("assessment configuration path changed")
                })?;
        if inode(
            &named_config
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment named config identity"))?,
        ) != self.configuration_identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment named configuration identity changed",
            ));
        }
        if let Some(root) = &self.private_root {
            protected_configuration_parents(root, self.uid)?;
            let private = tos_fd_open::open_absolute_directory(root)
                .map_err(|_| SourceCommandError::Conflict("assessment private root changed"))?;
            protected_private(&private, self.uid, true)?;
            if inode(
                &private
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("assessment private root identity"))?,
            ) != self
                .private_root_identity
                .ok_or(SourceCommandError::Invalid(
                    "assessment selected private root identity",
                ))?
            {
                return Err(SourceCommandError::Conflict(
                    "assessment private root identity changed",
                ));
            }
            let retained = self
                .private_root_file
                .as_ref()
                .ok_or(SourceCommandError::Invalid(
                    "assessment retained private root",
                ))?;
            protected_private(retained, self.uid, true)?;
            if inode(
                &retained.metadata().map_err(|_| {
                    SourceCommandError::Invalid("assessment retained root identity")
                })?,
            ) != self
                .private_root_identity
                .ok_or(SourceCommandError::Invalid(
                    "assessment selected private root identity",
                ))?
            {
                return Err(SourceCommandError::Conflict(
                    "assessment retained private root changed",
                ));
            }
        }
        protected_configuration_parents(&self.directory_path, self.uid)?;
        let directory = tos_fd_open::open_absolute_directory(&self.directory_path)
            .map_err(|_| SourceCommandError::Conflict("assessment journal replaced"))?;
        if self.private_root.is_some() {
            protected_private(&directory, self.uid, true)?;
        } else {
            protected(&directory, self.uid, true)?;
        }
        if inode(
            &directory
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment directory identity"))?,
        ) != self.identity
            || inode(&self.directory.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment retained directory identity")
            })?) != self.identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment journal identity changed",
            ));
        }
        let retained_directory_parent = self
            .directory_parent
            .metadata()
            .map_err(|_| SourceCommandError::Invalid("assessment retained journal parent"))?;
        let current_directory_parent =
            tos_fd_open::open_absolute_directory(&self.directory_parent_path)
                .map_err(|_| SourceCommandError::Conflict("assessment journal parent changed"))?;
        let private = self.private_root.is_some();
        if private {
            protected_private(&self.directory_parent, self.uid, true)?;
            protected_private(&current_directory_parent, self.uid, true)?;
        } else {
            protected(&self.directory_parent, self.uid, true)?;
            protected(&current_directory_parent, self.uid, true)?;
        }
        if inode(&retained_directory_parent) != self.directory_parent_identity
            || inode(
                &current_directory_parent.metadata().map_err(|_| {
                    SourceCommandError::Invalid("assessment journal parent identity")
                })?,
            ) != self.directory_parent_identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment journal parent identity changed",
            ));
        }
        let directory_name = self
            .directory_path
            .file_name()
            .ok_or(SourceCommandError::Invalid("assessment journal filename"))?;
        let named_directory =
            tos_fd_open::open_directory_at(&self.directory_parent, Path::new(directory_name))
                .map_err(|_| SourceCommandError::Conflict("assessment journal path changed"))?;
        if inode(
            &named_directory
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment named journal identity"))?,
        ) != self.identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment named journal identity changed",
            ));
        }
        Ok(())
    }

    /// Acquisition owns one fixed set; there is no nested expansion API. Sorting
    /// the SHA-256 homes exactly matches AssessmentJournal.locked_subjects.
    pub(crate) fn lock_subjects<'a>(
        &'a self,
        subject_ids: &[String],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentJournalFence<'a>> {
        self.verify_current(deadline, cancelled)?;
        if subject_ids.is_empty() || subject_ids.len() > 65 {
            return Err(SourceCommandError::Invalid("assessment lock scope count"));
        }
        let mut homes = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for id in subject_ids {
            if id.len() > 1_048_576
                || !ids.insert(id)
                || tos_foundation::python_strip_unicode16_v1(id, 1_048_576)
                    .map_err(|_| SourceCommandError::Invalid("assessment subject Unicode budget"))?
                    .is_empty()
            {
                return Err(SourceCommandError::Invalid(
                    "assessment distinct subject scope",
                ));
            }
            if homes
                .insert(Digest256::of_bytes(id.as_bytes()).to_hex(), id.clone())
                .is_some()
            {
                return Err(SourceCommandError::Conflict(
                    "assessment subject home collision",
                ));
            }
        }
        let lock_deadline = deadline.min(Instant::now() + Duration::from_secs(5));
        let mut held = BTreeMap::new();
        for (name, subject) in homes {
            active(lock_deadline, cancelled)?;
            match rustix::fs::mkdirat(&self.directory, name.as_str(), Mode::from_raw_mode(0o700)) {
                Ok(()) => self
                    .directory
                    .sync_all()
                    .map_err(|_| SourceCommandError::Invalid("assessment home directory fsync"))?,
                Err(Errno::EXIST) => (),
                Err(_) => return Err(SourceCommandError::Denied("assessment home mkdir")),
            }
            let directory = tos_fd_open::open_directory_at(&self.directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("assessment subject home unsafe"))?;
            if self.private_root.is_some() {
                protected_private(&directory, self.uid, true)?;
            } else {
                protected(&directory, self.uid, true)?;
            }
            let lock: File = rustix::fs::openat(
                &directory,
                ".writer.lock",
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map(File::from)
            .map_err(|_| SourceCommandError::Denied("assessment subject writer lock"))?;
            if self.private_root.is_some() {
                protected_private(&lock, self.uid, false)?;
            } else {
                protected(&lock, self.uid, false)?;
            }
            loop {
                active(lock_deadline, cancelled)?;
                match rustix::fs::flock(&lock, FlockOperation::NonBlockingLockExclusive) {
                    Ok(()) => break,
                    Err(Errno::AGAIN) => std::thread::sleep(Duration::from_millis(5)),
                    Err(_) => {
                        return Err(SourceCommandError::Denied("assessment flock unavailable"));
                    }
                }
            }
            if self.private_root.is_some() {
                protected_private(&lock, self.uid, false)?;
            } else {
                protected(&lock, self.uid, false)?;
            }
            let current = tos_fd_open::open_regular_at(&directory, Path::new(".writer.lock"))
                .map_err(|_| SourceCommandError::Conflict("assessment lock path changed"))?;
            if inode(
                &lock
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("assessment lock identity"))?,
            ) != inode(
                &current
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("assessment lock identity"))?,
            ) {
                return Err(SourceCommandError::Conflict(
                    "assessment locked inode replaced",
                ));
            }
            let identity = inode(
                &directory
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("assessment home identity"))?,
            );
            held.insert(
                subject,
                HeldSubject {
                    name,
                    directory,
                    identity,
                    _lock: lock,
                },
            );
        }
        self.verify_current(deadline, cancelled)?;
        Ok(AssessmentJournalFence { owner: self, held })
    }

    /// Select a fixed public-v2 read scope without creating homes or lock files.
    pub(crate) fn read_only_subjects<'a>(
        &'a self,
        subject_ids: &[String],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentJournalReadBatch<'a>> {
        if self.private_root.is_some()
            || cmd::text(&self.configuration, "schema_version")? != "tos_local_assessment_owner_v2"
        {
            return Err(SourceCommandError::Denied(
                "assessment read batch requires public owner v2",
            ));
        }
        self.verify_current(deadline, cancelled)?;
        if subject_ids.is_empty() || subject_ids.len() > 256 {
            return Err(SourceCommandError::Invalid(
                "assessment read batch scope count",
            ));
        }
        let configured_subjects = cmd::field(&self.configuration, "subjects")?;
        let mut names = BTreeMap::<String, String>::new();
        let mut ids = BTreeSet::<String>::new();
        for id in subject_ids {
            if id.len() > 1_048_576
                || !ids.insert(id.clone())
                || tos_foundation::python_strip_unicode16_v1(id, 1_048_576)
                    .map_err(|_| SourceCommandError::Invalid("assessment subject Unicode budget"))?
                    .is_empty()
            {
                return Err(SourceCommandError::Invalid(
                    "assessment distinct subject scope",
                ));
            }
            if configured_subjects.object_get(id).is_none() {
                return Err(SourceCommandError::Denied(
                    "assessment subject outside configured owner scope",
                ));
            }
            if names
                .insert(Digest256::of_bytes(id.as_bytes()).to_hex(), id.clone())
                .is_some()
            {
                return Err(SourceCommandError::Conflict(
                    "assessment subject home collision",
                ));
            }
        }
        let mut subjects = BTreeMap::new();
        for (name, subject) in names {
            active(deadline, cancelled)?;
            let (directory, identity) =
                match tos_fd_open::open_directory_at(&self.directory, Path::new(&name)) {
                    Ok(directory) => {
                        protected(&directory, self.uid, true)?;
                        let identity = inode(&directory.metadata().map_err(|_| {
                            SourceCommandError::Invalid("assessment home identity")
                        })?);
                        (Some(directory), Some(identity))
                    }
                    Err(error)
                        if error.source.as_ref().is_some_and(|source| {
                            source.kind() == std::io::ErrorKind::NotFound
                        }) =>
                    {
                        (None, None)
                    }
                    Err(_) => {
                        return Err(SourceCommandError::Denied("assessment subject home unsafe"));
                    }
                };
            subjects.insert(
                subject,
                ReadOnlySubjectHome {
                    name,
                    directory,
                    identity,
                },
            );
        }
        let selected = AssessmentJournalReadBatch {
            owner: self,
            subjects,
        };
        selected.verify_current(deadline, cancelled)?;
        Ok(selected)
    }
}
struct HeldSubject {
    name: String,
    directory: File,
    identity: (u64, u64),
    _lock: File,
}
pub(crate) struct AssessmentJournalFence<'a> {
    owner: &'a ProtectedAssessmentJournal,
    held: BTreeMap<String, HeldSubject>,
}
pub(crate) struct AssessmentReadHome<'a> {
    name: &'a str,
    directory: Option<&'a File>,
    identity: Option<(u64, u64)>,
}
struct ReadOnlySubjectHome {
    name: String,
    directory: Option<File>,
    identity: Option<(u64, u64)>,
}
pub(crate) struct AssessmentJournalReadBatch<'a> {
    owner: &'a ProtectedAssessmentJournal,
    subjects: BTreeMap<String, ReadOnlySubjectHome>,
}
pub(crate) trait AssessmentJournalReadView {
    fn owner(&self) -> &ProtectedAssessmentJournal;
    fn home(&self, subject: &str) -> SourceCommandResult<AssessmentReadHome<'_>>;
    fn home_named(&self, name: &str) -> SourceCommandResult<AssessmentReadHome<'_>>;
    fn verify_current(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()>;
}
impl AssessmentJournalReadBatch<'_> {
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.owner.verify_current(deadline, cancelled)?;
        for home in self.subjects.values() {
            active(deadline, cancelled)?;
            match (&home.directory, home.identity) {
                (Some(retained), Some(identity)) => {
                    protected(retained, self.owner.uid, true)?;
                    if inode(&retained.metadata().map_err(|_| {
                        SourceCommandError::Invalid("assessment retained home identity")
                    })?) != identity
                    {
                        return Err(SourceCommandError::Conflict(
                            "assessment retained home replaced",
                        ));
                    }
                    let current = tos_fd_open::open_directory_at(
                        &self.owner.directory,
                        Path::new(&home.name),
                    )
                    .map_err(|_| {
                        SourceCommandError::Conflict("assessment selected home disappeared")
                    })?;
                    protected(&current, self.owner.uid, true)?;
                    if inode(&current.metadata().map_err(|_| {
                        SourceCommandError::Invalid("assessment current home identity")
                    })?) != identity
                    {
                        return Err(SourceCommandError::Conflict(
                            "assessment selected home identity changed",
                        ));
                    }
                }
                (None, None) => match tos_fd_open::open_directory_at(
                    &self.owner.directory,
                    Path::new(&home.name),
                ) {
                    Err(error)
                        if error.source.as_ref().is_some_and(|source| {
                            source.kind() == std::io::ErrorKind::NotFound
                        }) => {}
                    _ => {
                        return Err(SourceCommandError::Conflict(
                            "assessment absent home appeared",
                        ));
                    }
                },
                _ => {
                    return Err(SourceCommandError::Invalid(
                        "assessment selected home identity state",
                    ));
                }
            }
        }
        self.owner.verify_current(deadline, cancelled)
    }

    pub(crate) fn head(
        &self,
        subject: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        let (head, member) = read_head(self, subject, deadline, cancelled)?;
        if let Some(member) = member.as_ref() {
            verify_member_current(self, member, deadline, cancelled)?;
        } else {
            verify_head_absent(self, subject, deadline, cancelled)?;
        }
        self.verify_current(deadline, cancelled)?;
        Ok(head)
    }

    pub(crate) fn read(
        &self,
        subject: &str,
        ctx: &CommandContext,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentHistory> {
        read_assessment_history(self, subject, ctx, worker, deadline, cancelled)
    }
}
impl AssessmentJournalReadView for AssessmentJournalFence<'_> {
    fn owner(&self) -> &ProtectedAssessmentJournal {
        self.owner
    }

    fn home(&self, subject: &str) -> SourceCommandResult<AssessmentReadHome<'_>> {
        let held = self.held.get(subject).ok_or(SourceCommandError::Denied(
            "assessment subject outside held scope",
        ))?;
        Ok(AssessmentReadHome {
            name: &held.name,
            directory: Some(&held.directory),
            identity: Some(held.identity),
        })
    }

    fn home_named(&self, name: &str) -> SourceCommandResult<AssessmentReadHome<'_>> {
        let held =
            self.held
                .values()
                .find(|held| held.name == name)
                .ok_or(SourceCommandError::Denied(
                    "assessment journal member outside held homes",
                ))?;
        Ok(AssessmentReadHome {
            name: &held.name,
            directory: Some(&held.directory),
            identity: Some(held.identity),
        })
    }

    fn verify_current(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        AssessmentJournalFence::verify_current(self, deadline, cancelled)
    }
}
impl AssessmentJournalReadView for AssessmentJournalReadBatch<'_> {
    fn owner(&self) -> &ProtectedAssessmentJournal {
        self.owner
    }

    fn home(&self, subject: &str) -> SourceCommandResult<AssessmentReadHome<'_>> {
        let home = self
            .subjects
            .get(subject)
            .ok_or(SourceCommandError::Denied(
                "assessment subject outside read batch scope",
            ))?;
        Ok(AssessmentReadHome {
            name: &home.name,
            directory: home.directory.as_ref(),
            identity: home.identity,
        })
    }

    fn home_named(&self, name: &str) -> SourceCommandResult<AssessmentReadHome<'_>> {
        let home = self
            .subjects
            .values()
            .find(|home| home.name == name)
            .ok_or(SourceCommandError::Denied(
                "assessment journal member outside read batch scope",
            ))?;
        Ok(AssessmentReadHome {
            name: &home.name,
            directory: home.directory.as_ref(),
            identity: home.identity,
        })
    }

    fn verify_current(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        AssessmentJournalReadBatch::verify_current(self, deadline, cancelled)
    }
}
pub(crate) struct AssessmentHistory {
    subject_id: String,
    pub(crate) head: Option<String>,
    pub(crate) batches: Vec<JsonValue>,
    pub(crate) submissions: Vec<tos_validation::assessment::AssessmentSubmissionInput>,
    members: Vec<HeldJournalMember>,
}
struct HeldJournalMember {
    home_name: String,
    name: String,
    file: File,
    identity: (u64, u64),
    digest: Digest256,
    size: usize,
}
impl AssessmentHistory {
    /// Raw authenticated journal bytes retained by this selected history.
    /// Callers charge this once against the operation-wide read budget before
    /// evaluating current admission or attempting a compare-and-swap write.
    pub(crate) fn input_bytes(&self) -> SourceCommandResult<usize> {
        self.members.iter().try_fold(0usize, |total, member| {
            total
                .checked_add(member.size)
                .ok_or(SourceCommandError::Invalid(
                    "assessment history input byte total overflow",
                ))
        })
    }

    /// Keep each authenticated journal member descriptor alive through the
    /// source-currentness fence and compare its retained inode with the exact
    /// path still named under the held subject home.
    pub(crate) fn verify_current<V: AssessmentJournalReadView>(
        &self,
        fence: &V,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify_members(fence, true, deadline, cancelled)
    }

    pub(crate) fn verify_batches_current(
        &self,
        fence: &AssessmentJournalFence<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify_members(fence, false, deadline, cancelled)
    }

    fn verify_members<V: AssessmentJournalReadView>(
        &self,
        fence: &V,
        include_head: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        fence.verify_current(deadline, cancelled)?;
        for member in &self.members {
            if !include_head && member.name == "head" {
                continue;
            }
            active(deadline, cancelled)?;
            verify_member_current(fence, member, deadline, cancelled)?;
        }
        if include_head && self.head.is_none() {
            verify_head_absent(fence, &self.subject_id, deadline, cancelled)?;
        }
        fence.verify_current(deadline, cancelled)?;
        Ok(())
    }
}

fn verify_head_absent<V: AssessmentJournalReadView>(
    view: &V,
    subject: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let home = view.home(subject)?;
    let Some(directory) = home.directory else {
        return match tos_fd_open::open_directory_at(&view.owner().directory, Path::new(home.name)) {
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) =>
            {
                Ok(())
            }
            _ => Err(SourceCommandError::Conflict(
                "assessment selected home appeared",
            )),
        };
    };
    match tos_fd_open::open_regular_at(directory, Path::new("head")) {
        Err(error)
            if error
                .source
                .as_ref()
                .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(())
        }
        _ => Err(SourceCommandError::Conflict(
            "assessment head appeared after empty history read",
        )),
    }
}

fn verify_member_current<V: AssessmentJournalReadView>(
    fence: &V,
    member: &HeldJournalMember,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    let home = fence.home_named(&member.home_name)?;
    let directory = home.directory.ok_or(SourceCommandError::Denied(
        "assessment journal member has no selected home",
    ))?;
    let owner = fence.owner();
    let private = owner.private_root.is_some();
    let retained = member
        .file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("assessment retained member identity"))?;
    if inode(&retained) != member.identity {
        return Err(SourceCommandError::Conflict(
            "assessment retained member inode changed",
        ));
    }
    let mut current = tos_fd_open::open_regular_at(directory, Path::new(&member.name))
        .map_err(|_| SourceCommandError::Conflict("assessment journal member replaced"))?;
    if private {
        protected_private(&current, owner.uid, false)?;
    } else {
        protected(&current, owner.uid, false)?;
    }
    if inode(
        &current
            .metadata()
            .map_err(|_| SourceCommandError::Invalid("assessment current member identity"))?,
    ) != member.identity
    {
        return Err(SourceCommandError::Conflict(
            "assessment journal member path identity changed",
        ));
    }
    let bytes = raw(&mut current, MAX_BATCH_BYTES.max(65), deadline, cancelled)?;
    if bytes.len() != member.size || Digest256::of_bytes(&bytes) != member.digest {
        return Err(SourceCommandError::Conflict(
            "assessment journal member bytes changed",
        ));
    }
    Ok(())
}
impl AssessmentJournalFence<'_> {
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.owner.verify_current(deadline, cancelled)?;
        for held in self.held.values() {
            active(deadline, cancelled)?;
            let directory =
                tos_fd_open::open_directory_at(&self.owner.directory, Path::new(&held.name))
                    .map_err(|_| {
                        SourceCommandError::Conflict("assessment held home unavailable")
                    })?;
            if self.owner.private_root.is_some() {
                protected_private(&directory, self.owner.uid, true)?;
            } else {
                protected(&directory, self.owner.uid, true)?;
            }
            if inode(
                &directory
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("assessment held home identity"))?,
            ) != held.identity
            {
                return Err(SourceCommandError::Conflict(
                    "assessment held home replaced",
                ));
            }
            let current = tos_fd_open::open_regular_at(&directory, Path::new(".writer.lock"))
                .map_err(|_| SourceCommandError::Conflict("assessment held lock unavailable"))?;
            if self.owner.private_root.is_some() {
                protected_private(&current, self.owner.uid, false)?;
                protected_private(&held._lock, self.owner.uid, false)?;
            } else {
                protected(&current, self.owner.uid, false)?;
                protected(&held._lock, self.owner.uid, false)?;
            }
            if inode(
                &current
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("assessment held lock identity"))?,
            ) != inode(
                &held
                    ._lock
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("assessment held lock identity"))?,
            ) {
                return Err(SourceCommandError::Conflict(
                    "assessment held lock replaced",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn head(
        &self,
        subject: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        read_head(self, subject, deadline, cancelled).map(|(head, _)| head)
    }

    pub(crate) fn read(
        &self,
        subject: &str,
        ctx: &CommandContext,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentHistory> {
        read_assessment_history(self, subject, ctx, worker, deadline, cancelled)
    }

    /// Publish one fully evaluated v1 assessment batch through the common
    /// immutable-blob/atomic-head protocol. The owner adapter supplies a guard
    /// which rechecks its selected private/public source closure at both edges.
    pub(crate) fn publish(
        &self,
        subject: &str,
        history: &AssessmentHistory,
        expected_revision: Option<&str>,
        batch: &JsonValue,
        mut currentness: impl FnMut() -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<String> {
        active(deadline, cancelled)?;
        if history.head.as_deref() != expected_revision {
            return Err(SourceCommandError::Conflict(
                "assessment expected revision differs from held history",
            ));
        }
        let expected = expected_revision.map_or(JsonValue::Null, |value| cmd::string(value));
        if !cmd::same(cmd::field(batch, "previous_revision")?, &expected)? {
            return Err(SourceCommandError::Invalid(
                "assessment batch predecessor differs from expected revision",
            ));
        }
        let payload = cmd::canonical(batch)?;
        if payload.len() > MAX_BATCH_BYTES {
            return Err(SourceCommandError::Invalid("assessment batch byte budget"));
        }
        let revision = cmd::record_digest(batch)?.to_hex();
        let batch_name = format!("{revision}.json");
        history.verify_current(self, deadline, cancelled)?;
        self.owner.verify_current(deadline, cancelled)?;
        currentness()?;
        if self.head(subject, deadline, cancelled)?.as_deref() != expected_revision {
            return Err(SourceCommandError::Conflict(
                "assessment expected head changed before blob publication",
            ));
        }
        let published_batch =
            self.write_immutable(subject, &batch_name, &payload, deadline, cancelled)?;
        // An unreferenced immutable blob is harmless if any source or CAS
        // fence changes here; only `head` makes it visible history.
        history.verify_current(self, deadline, cancelled)?;
        verify_member_current(self, &published_batch, deadline, cancelled)?;
        self.owner.verify_current(deadline, cancelled)?;
        currentness()?;
        if self.head(subject, deadline, cancelled)?.as_deref() != expected_revision {
            return Err(SourceCommandError::Conflict(
                "assessment expected head changed before CAS publication",
            ));
        }
        let held = self.held.get(subject).ok_or(SourceCommandError::Denied(
            "assessment subject outside held scope",
        ))?;
        let head_bytes = format!("{revision}\n").into_bytes();
        crate::source_creation_store::work_transaction::atomic_write(
            &held.directory,
            "head",
            &head_bytes,
            false,
            deadline,
            cancelled,
        )?;
        self.owner.verify_current(deadline, cancelled)?;
        currentness()?;
        verify_member_current(self, &published_batch, deadline, cancelled)?;
        if self.head(subject, deadline, cancelled)?.as_deref() != Some(revision.as_str()) {
            return Err(SourceCommandError::Conflict(
                "assessment published head failed exact reread",
            ));
        }
        history.verify_batches_current(self, deadline, cancelled)?;
        verify_member_current(self, &published_batch, deadline, cancelled)?;
        Ok(revision)
    }

    fn write_immutable(
        &self,
        subject: &str,
        name: &str,
        payload: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<HeldJournalMember> {
        let held = self.held.get(subject).ok_or(SourceCommandError::Denied(
            "assessment subject outside held scope",
        ))?;
        match tos_fd_open::open_regular_at(&held.directory, Path::new(name)) {
            Ok(mut file) => {
                if self.owner.private_root.is_some() {
                    protected_private(&file, self.owner.uid, false)?;
                } else {
                    protected(&file, self.owner.uid, false)?;
                }
                let identity = inode(&file.metadata().map_err(|_| {
                    SourceCommandError::Invalid("assessment existing batch identity")
                })?);
                if raw(&mut file, MAX_BATCH_BYTES, deadline, cancelled)? != payload {
                    return Err(SourceCommandError::Conflict(
                        "assessment immutable batch name has different bytes",
                    ));
                }
                let current = tos_fd_open::open_regular_at(&held.directory, Path::new(name))
                    .map_err(|_| {
                        SourceCommandError::Conflict("assessment existing batch replaced")
                    })?;
                if inode(&current.metadata().map_err(|_| {
                    SourceCommandError::Invalid("assessment existing batch path identity")
                })?) != identity
                {
                    return Err(SourceCommandError::Conflict(
                        "assessment existing immutable batch path changed",
                    ));
                }
                return Ok(HeldJournalMember {
                    home_name: held.name.clone(),
                    name: name.to_owned(),
                    file,
                    identity,
                    digest: Digest256::of_bytes(payload),
                    size: payload.len(),
                });
            }
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) => {}
            Err(_) => {
                return Err(SourceCommandError::Denied(
                    "assessment immutable batch destination unsafe",
                ));
            }
        }
        crate::source_creation_store::work_transaction::atomic_write(
            &held.directory,
            name,
            payload,
            true,
            deadline,
            cancelled,
        )?;
        let mut installed = tos_fd_open::open_regular_at(&held.directory, Path::new(name))
            .map_err(|_| SourceCommandError::Conflict("assessment batch install missing"))?;
        if self.owner.private_root.is_some() {
            protected_private(&installed, self.owner.uid, false)?;
        } else {
            protected(&installed, self.owner.uid, false)?;
        }
        let identity = inode(
            &installed
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment installed batch identity"))?,
        );
        if raw(&mut installed, MAX_BATCH_BYTES, deadline, cancelled)? != payload {
            return Err(SourceCommandError::Conflict(
                "assessment installed immutable batch differs",
            ));
        }
        let current = tos_fd_open::open_regular_at(&held.directory, Path::new(name))
            .map_err(|_| SourceCommandError::Conflict("assessment installed batch path missing"))?;
        if inode(
            &current.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment installed batch path identity")
            })?,
        ) != identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment installed batch path identity changed",
            ));
        }
        Ok(HeldJournalMember {
            home_name: held.name.clone(),
            name: name.to_owned(),
            file: installed,
            identity,
            digest: Digest256::of_bytes(payload),
            size: payload.len(),
        })
    }
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn read_head<V: AssessmentJournalReadView>(
    view: &V,
    subject: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(Option<String>, Option<HeldJournalMember>)> {
    view.verify_current(deadline, cancelled)?;
    let home = view.home(subject)?;
    let owner = view.owner();
    let Some(directory) = home.directory else {
        view.verify_current(deadline, cancelled)?;
        return Ok((None, None));
    };
    let expected_identity = home.identity.ok_or(SourceCommandError::Invalid(
        "assessment selected home identity absent",
    ))?;
    let current = tos_fd_open::open_directory_at(&owner.directory, Path::new(home.name))
        .map_err(|_| SourceCommandError::Conflict("assessment selected home changed"))?;
    if owner.private_root.is_some() {
        protected_private(&current, owner.uid, true)?;
    } else {
        protected(&current, owner.uid, true)?;
    }
    if inode(
        &current
            .metadata()
            .map_err(|_| SourceCommandError::Invalid("assessment home identity"))?,
    ) != expected_identity
    {
        return Err(SourceCommandError::Conflict(
            "assessment selected home replaced",
        ));
    }
    let head_file = match tos_fd_open::open_regular_at(directory, Path::new("head")) {
        Ok(file) => Some(file),
        Err(error)
            if error
                .source
                .as_ref()
                .is_some_and(|source| source.kind() == std::io::ErrorKind::NotFound) =>
        {
            None
        }
        Err(_) => {
            return Err(SourceCommandError::Invalid(
                "assessment head pointer unsafe",
            ));
        }
    };
    let (head, member) = if let Some(mut file) = head_file {
        if owner.private_root.is_some() {
            protected_private(&file, owner.uid, false)?;
        } else {
            protected(&file, owner.uid, false)?;
        }
        let identity = inode(
            &file
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment head identity"))?,
        );
        let raw = raw(&mut file, 65, deadline, cancelled)?;
        if !raw.is_ascii() {
            return Err(SourceCommandError::Invalid("assessment head ASCII"));
        }
        let text =
            tos_foundation::python_strip_unicode16_v1(std::str::from_utf8(&raw).unwrap(), 65)
                .map_err(|_| SourceCommandError::Invalid("assessment head whitespace"))?;
        if !digest(text) {
            return Err(SourceCommandError::Invalid("assessment head digest"));
        }
        (
            Some(text.to_owned()),
            Some(HeldJournalMember {
                home_name: home.name.to_owned(),
                name: "head".to_owned(),
                file,
                identity,
                digest: Digest256::of_bytes(&raw),
                size: raw.len(),
            }),
        )
    } else {
        (None, None)
    };
    view.verify_current(deadline, cancelled)?;
    Ok((head, member))
}

fn read_assessment_history<V: AssessmentJournalReadView>(
    view: &V,
    subject: &str,
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AssessmentHistory> {
    let (head, head_member) = read_head(view, subject, deadline, cancelled)?;
    let home = view.home(subject)?;
    let owner = view.owner();
    let mut members = head_member.into_iter().collect::<Vec<_>>();
    let contract = ctx
        .file(&tos_foundation::RelativePath::parse(BATCH_SCHEMA).unwrap())?
        .ok_or(SourceCommandError::Unsupported(
            "selected assessment batch schema absent",
        ))?;
    if worker.source_revision() != ctx.base_revision
        || worker.contract_digest(BATCH_SCHEMA) != Some(Digest256::of_bytes(contract))
    {
        return Err(SourceCommandError::Conflict(
            "assessment journal schema/source cut differs",
        ));
    }
    let mut cursor = head.clone();
    let mut seen = BTreeSet::new();
    let mut chain = Vec::new();
    let mut count = 0usize;
    let mut byte_count = 0usize;
    while let Some(revision) = cursor {
        active(deadline, cancelled)?;
        if !digest(&revision) || !seen.insert(revision.clone()) || chain.len() >= MAX_EVENTS {
            return Err(SourceCommandError::Invalid(
                "assessment cyclic/oversized history",
            ));
        }
        let directory = home.directory.ok_or(SourceCommandError::Invalid(
            "assessment history home absent",
        ))?;
        let name = format!("{revision}.json");
        let mut file = tos_fd_open::open_regular_at(directory, Path::new(&name))
            .map_err(|_| SourceCommandError::Invalid("assessment immutable batch absent/unsafe"))?;
        if owner.private_root.is_some() {
            protected_private(&file, owner.uid, false)?;
        } else {
            protected(&file, owner.uid, false)?;
        }
        let identity = inode(
            &file
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment batch identity"))?,
        );
        let bytes = raw(&mut file, MAX_BATCH_BYTES, deadline, cancelled)?;
        byte_count = byte_count
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_HISTORY_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "assessment complete history byte budget",
            ))?;
        let batch = cmd::parse(&bytes)?;
        if !worker
            .check(
                &format!("assessment-journal/{revision}.json"),
                &cmd::canonical(&batch)?,
                BATCH_SCHEMA,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Invalid("assessment batch schema execution"))?
        {
            return Err(SourceCommandError::Invalid("assessment batch schema"));
        }
        let request = cmd::field(&batch, "request")?;
        if cmd::record_digest(&batch)?.to_hex() != revision
            || cmd::text(&batch, "schema_version")? != "tos_assessment_batch_v1"
            || cmd::text(&batch, "subject_id")? != subject
            || cmd::text(cmd::field(request, "subject")?, "id")? != subject
            || cmd::record_digest(request)?.to_hex() != cmd::text(&batch, "request_digest")?
            || !cmd::same(
                cmd::field(request, "expected_revision")?,
                cmd::field(&batch, "previous_revision")?,
            )?
        {
            return Err(SourceCommandError::Invalid(
                "assessment batch/request identity binding",
            ));
        }
        let events = cmd::array(&batch, "events")?;
        count = count
            .checked_add(events.len())
            .filter(|n| *n <= MAX_EVENTS)
            .ok_or(SourceCommandError::Invalid(
                "assessment complete history event budget",
            ))?;
        if events.is_empty() {
            return Err(SourceCommandError::Invalid("assessment empty batch"));
        }
        members.push(HeldJournalMember {
            home_name: home.name.to_owned(),
            name,
            file,
            identity,
            digest: Digest256::of_bytes(&bytes),
            size: bytes.len(),
        });
        cursor = match cmd::field(&batch, "previous_revision")? {
            JsonValue::Null => None,
            value => Some(
                value
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("assessment predecessor digest"))?
                    .to_owned(),
            ),
        };
        chain.push(batch);
    }
    chain.reverse();
    let mut known = BTreeMap::<String, Vec<u8>>::new();
    let mut submissions = Vec::new();
    for (index, batch) in chain.iter().enumerate() {
        active(deadline, cancelled)?;
        if cmd::integer(batch, "sequence")? != (index + 1) as u64
            || index > 0
                && tos_validation::retirement_rules::observed_instant_order(
                    cmd::text(&chain[index - 1], "recorded_at")?,
                    cmd::text(batch, "recorded_at")?,
                )
                .map_err(|_| SourceCommandError::Invalid("assessment recorded chronology"))?
                    == std::cmp::Ordering::Greater
        {
            return Err(SourceCommandError::Invalid(
                "assessment history sequence/chronology",
            ));
        }
        let request = cmd::field(batch, "request")?;
        let mut expected = Vec::<JsonValue>::new();
        let mut expected_bytes = BTreeSet::new();
        for event in cmd::array(request, "events")? {
            let id = cmd::text(cmd::field(event, "assessment")?, "assessment_id")?;
            let bytes = cmd::canonical(event)?;
            if let Some(original) = known.get(id) {
                if original != &bytes {
                    return Err(SourceCommandError::Invalid("assessment identity rewritten"));
                }
            } else if expected_bytes.insert(bytes) {
                expected.push(event.clone());
            }
        }
        if !cmd::same(&JsonValue::Array(expected), cmd::field(batch, "events")?)? {
            return Err(SourceCommandError::Invalid(
                "assessment batch does not contain exact new request events",
            ));
        }
        let scope = cmd::object(vec![
            ("assertion_layer", cmd::field(request, "layer")?.clone()),
            ("risk", cmd::field(request, "risk")?.clone()),
            ("languages", cmd::field(request, "languages")?.clone()),
            ("maker_id", cmd::field(request, "maker_id")?.clone()),
            ("requested_use", cmd::field(request, "use")?.clone()),
        ]);
        for event in cmd::array(batch, "events")? {
            let id = cmd::text(cmd::field(event, "assessment")?, "assessment_id")?.to_owned();
            if known.insert(id, cmd::canonical(event)?).is_some() {
                return Err(SourceCommandError::Invalid(
                    "assessment duplicate committed event",
                ));
            }
            submissions.push(tos_validation::assessment::AssessmentSubmissionInput {
                assessment: cmd::canonical(cmd::field(event, "assessment")?)?,
                principal_id: cmd::text(event, "principal_id")?.to_owned(),
                execution_profile: cmd::canonical(cmd::field(event, "execution_profile")?)?,
                committed_scope: Some(cmd::canonical(&scope)?),
            });
        }
    }
    view.verify_current(deadline, cancelled)?;
    let history = AssessmentHistory {
        subject_id: subject.to_owned(),
        head,
        batches: chain,
        submissions,
        members,
    };
    history.verify_current(view, deadline, cancelled)?;
    Ok(history)
}
