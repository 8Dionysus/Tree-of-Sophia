//! Protected current assessment history and compatible sorted subject flocks.
//! This reader never creates judgments or imports admission_at_commit as a grant.

use crate::durable_private_packages::{
    PrivateOwnerPackageCapabilityV1, PrivateOwnerPackageFamilyV1, PrivateOwnerPackageHistoryV1,
    PrivateOwnerPackageReplayV1, PrivateOwnerPackageSnapshotV1, PrivateOwnerPackageWriteV1,
    PrivateOwnerScopeV1, capability_seal,
};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{
    ReadLength, active, inode, protected_configuration_parents, raw, raw_with_length_limit,
};
use crate::{DurableError, DurablePgCoordinator, DurableResult};
use rustix::fs::{FlockOperation, Mode, OFlags};
use rustix::io::Errno;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue, SourceRevision};
use tos_validation::FormatProfile;
use tos_validation::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutSchemaExecutor, CutWorkerSchemaExecutor,
};

const BATCH_SCHEMA: &str = "ToS/contracts/knowledge-assessment-batch.schema.json";
use crate::source_assessment_capacity::{
    CapacityRefusal, HistoryCapacity, Limits, MAX_BATCH_BYTES, MAX_EVENTS, MAX_HISTORY_BYTES,
    StorageEnvelope,
};

/// Lower-only delegated IO limits. These do not certify returned-owned capacity.
pub(crate) struct C01OwnerReadLimits {
    pub(crate) config_bytes: usize,
    pub(crate) batch_bytes: usize,
    pub(crate) history_bytes: usize,
    pub(crate) batches: usize,
    pub(crate) events: usize,
}
impl C01OwnerReadLimits {
    fn validate(&self) -> SourceCommandResult<()> {
        if self.config_bytes == 0
            || self.config_bytes > 8_388_608
            || self.batch_bytes == 0
            || self.batch_bytes > MAX_BATCH_BYTES
            || self.history_bytes == 0
            || self.history_bytes > MAX_HISTORY_BYTES
            || self.batches == 0
            || self.batches > MAX_EVENTS
            || self.events == 0
            || self.events > MAX_EVENTS
        {
            return Err(SourceCommandError::Invalid("assessment lower read limits"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum AssessmentReadPolicy<'a> {
    Legacy,
    Limited(&'a C01OwnerReadLimits),
}
impl AssessmentReadPolicy<'_> {
    fn validate(self) -> SourceCommandResult<()> {
        match self {
            Self::Legacy => Ok(()),
            Self::Limited(read) => read.validate(),
        }
    }
    fn config_cap(self) -> usize {
        match self {
            Self::Legacy => 8_388_608,
            Self::Limited(read) => read.config_bytes,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ExactJournalHead<'a> {
    None,
    Some(&'a str),
}
impl ExactJournalHead<'_> {
    fn validate(self) -> SourceCommandResult<()> {
        if let Self::Some(value) = self {
            if !digest(value) {
                return Err(SourceCommandError::Invalid(
                    "assessment expected head digest",
                ));
            }
        }
        Ok(())
    }
    fn matches(self, head: Option<&str>) -> bool {
        match self {
            Self::None => head.is_none(),
            Self::Some(value) => head == Some(value),
        }
    }
}
#[derive(Clone, Copy)]
enum AssessmentHeadSelection<'a> {
    LegacyUnpinned,
    Exact(ExactJournalHead<'a>),
}

fn capacity_refusal(error: CapacityRefusal) -> SourceCommandError {
    SourceCommandError::Invalid(match error {
        CapacityRefusal::BatchBytes => "assessment batch byte budget",
        CapacityRefusal::HistoryBytes => "assessment complete history byte budget",
        CapacityRefusal::Batches => "assessment complete history budget",
        CapacityRefusal::Events => "assessment complete history event budget",
    })
}

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

/// Protected v5/v6 Assessment configuration selected for a PostgreSQL
/// history route. Unlike `ProtectedAssessmentJournal`, this selection does
/// not require or inspect the legacy filesystem journal directory. The
/// directory value is only a stable logical namespace input for PG targets;
/// history bytes and current heads come exclusively from the package broker.
pub(crate) struct ProtectedAssessmentPgGrant {
    configuration_path: PathBuf,
    configuration_raw: Vec<u8>,
    configuration: JsonValue,
    configuration_file: File,
    configuration_identity: (u64, u64),
    configuration_parent_path: PathBuf,
    configuration_parent: File,
    configuration_parent_identity: (u64, u64),
    uid: u32,
}

impl ProtectedAssessmentPgGrant {
    /// Read the selected context path from a protected v5/v6 grant so its
    /// context can be constructed before the final grant selection. This is
    /// only a locator: `select` must reread and retain the exact grant before
    /// any operation is authorized.
    pub(crate) fn source_context_ref(
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PathBuf> {
        active(deadline, cancelled)?;
        let uid = rustix::process::getuid().as_raw();
        if uid != rustix::process::geteuid().as_raw() {
            return Err(SourceCommandError::Denied(
                "assessment private PG route refuses setuid context",
            ));
        }
        let configuration_path =
            crate::source_text_owner::normalized_absolute(configuration_path.to_str().ok_or(
                SourceCommandError::Invalid("assessment PG config path UTF-8"),
            )?)?;
        protected_configuration_parents(&configuration_path, uid)?;
        let mut file = tos_fd_open::open_absolute_regular(&configuration_path, 8_388_608)
            .map_err(|_| SourceCommandError::Denied("assessment PG config open"))?;
        protected_private(&file, uid, false)?;
        let raw = raw(&mut file, 8_388_608, deadline, cancelled)?;
        let configuration = cmd::parse(&raw)?;
        if !matches!(
            cmd::text(&configuration, "schema_version")?,
            "tos_local_assessment_owner_v5" | "tos_local_assessment_owner_v6"
        ) {
            return Err(SourceCommandError::Denied(
                "private assessment PG owner v5/v6 required",
            ));
        }
        crate::source_text_owner::normalized_absolute(cmd::text(
            &configuration,
            "source_context_ref",
        )?)
    }

    pub(crate) fn select(
        configuration_path: &Path,
        source_context_ref: &Path,
        context: &crate::source_text_owner::OwnerTextContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        let uid = rustix::process::getuid().as_raw();
        if uid != rustix::process::geteuid().as_raw() || context.account_uid() != uid {
            return Err(SourceCommandError::Denied(
                "assessment private PG route refuses setuid context",
            ));
        }
        let configuration_path =
            crate::source_text_owner::normalized_absolute(configuration_path.to_str().ok_or(
                SourceCommandError::Invalid("assessment PG config path UTF-8"),
            )?)?;
        protected_configuration_parents(&configuration_path, uid)?;
        let mut configuration_file =
            tos_fd_open::open_absolute_regular(&configuration_path, 8_388_608)
                .map_err(|_| SourceCommandError::Denied("assessment PG config open"))?;
        protected_private(&configuration_file, uid, false)?;
        let configuration_raw = raw(&mut configuration_file, 8_388_608, deadline, cancelled)?;
        let configuration = cmd::parse(&configuration_raw)?;
        let version = cmd::text(&configuration, "schema_version")?;
        if !matches!(
            version,
            "tos_local_assessment_owner_v5" | "tos_local_assessment_owner_v6"
        ) {
            return Err(SourceCommandError::Denied(
                "private assessment PG owner v5/v6 required",
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
            "native_text_layers",
            "quality_dependencies",
        ];
        if configuration
            .object_get("owner_local_source_claims")
            .is_some()
        {
            keys.push("owner_local_source_claims");
        }
        cmd::exact_keys(&configuration, &keys)?;
        let expected_context =
            crate::source_text_owner::normalized_absolute(source_context_ref.to_str().ok_or(
                SourceCommandError::Invalid("assessment PG context path UTF-8"),
            )?)?;
        let selected_private_root =
            crate::source_text_owner::normalized_absolute(context.private_root().to_str().ok_or(
                SourceCommandError::Invalid("assessment PG private root UTF-8"),
            )?)?;
        if cmd::integer(&configuration, "uid")? != u64::from(uid)
            || tos_foundation::python_strip_unicode16_v1(
                cmd::text(&configuration, "principal_id")?,
                8_388_608,
            )
            .map_err(|_| SourceCommandError::Invalid("assessment PG principal Unicode budget"))?
            .is_empty()
            || crate::source_text_owner::normalized_absolute(cmd::text(
                &configuration,
                "source_context_ref",
            )?)? != expected_context
            || crate::source_text_owner::normalized_absolute(
                context
                    .configuration_path()
                    .to_str()
                    .ok_or(SourceCommandError::Invalid(
                        "assessment PG selected context UTF-8",
                    ))?,
            )? != expected_context
            || !crate::source_text_owner::normalized_absolute(cmd::text(
                &configuration,
                "journal_directory",
            )?)?
            .starts_with(&selected_private_root)
        {
            return Err(SourceCommandError::Denied(
                "assessment PG owner account, context or journal namespace differs",
            ));
        }
        let configuration_parent_path = configuration_path
            .parent()
            .ok_or(SourceCommandError::Invalid("assessment PG config parent"))?
            .to_path_buf();
        let configuration_parent = tos_fd_open::open_absolute_directory(&configuration_parent_path)
            .map_err(|_| SourceCommandError::Denied("assessment PG config parent"))?;
        protected_private(&configuration_parent, uid, true)?;
        let configuration_identity = inode(
            &configuration_file
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment PG config identity"))?,
        );
        let configuration_parent_identity =
            inode(&configuration_parent.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment PG config parent identity")
            })?);
        let selected = Self {
            configuration_path,
            configuration_raw,
            configuration,
            configuration_file,
            configuration_identity,
            configuration_parent_path,
            configuration_parent,
            configuration_parent_identity,
            uid,
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

    /// Map the protected local journal-directory selection to a stable PG
    /// package namespace. The path is never opened here; PG history remains
    /// authoritative after restore.
    pub(crate) fn target_ref(
        &self,
        context: &crate::source_text_owner::OwnerTextContext,
        subject: &str,
    ) -> SourceCommandResult<String> {
        if context.account_uid() != self.uid
            || subject.is_empty()
            || subject.len() > 1_048_576
            || tos_foundation::python_strip_unicode16_v1(subject, 1_048_576)
                .map_err(|_| SourceCommandError::Invalid("assessment PG subject Unicode budget"))?
                .is_empty()
            || !cmd::field(&self.configuration, "subjects")?
                .object_get(subject)
                .is_some()
        {
            return Err(SourceCommandError::Denied(
                "assessment PG subject is outside protected owner scope",
            ));
        }
        let journal = crate::source_text_owner::normalized_absolute(cmd::text(
            &self.configuration,
            "journal_directory",
        )?)?;
        let private_root =
            crate::source_text_owner::normalized_absolute(context.private_root().to_str().ok_or(
                SourceCommandError::Invalid("assessment PG private root UTF-8"),
            )?)?;
        let relative = journal
            .strip_prefix(private_root)
            .map_err(|_| SourceCommandError::Denied("assessment PG journal namespace"))?;
        let relative = relative
            .to_str()
            .ok_or(SourceCommandError::Invalid(
                "assessment PG journal path UTF-8",
            ))?
            .trim_start_matches('/');
        let home = Digest256::of_bytes(subject.as_bytes()).to_hex();
        let base = format!(
            "{}assessment-journal/{}",
            context.private_prefix(),
            Digest256::of_bytes(relative.as_bytes()).to_hex()
        );
        let target_ref = format!("{base}/{home}");
        if !target_ref.starts_with(context.private_prefix())
            || tos_foundation::RelativePath::parse(&target_ref).is_err()
        {
            return Err(SourceCommandError::Invalid(
                "assessment PG target reference",
            ));
        }
        Ok(target_ref)
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
            return Err(SourceCommandError::Denied("assessment PG account changed"));
        }
        protected_configuration_parents(&self.configuration_path, self.uid)?;
        let mut current =
            tos_fd_open::open_absolute_regular(&self.configuration_path, 8_388_608)
                .map_err(|_| SourceCommandError::Conflict("assessment PG grant unavailable"))?;
        protected_private(&current, self.uid, false)?;
        protected_private(&self.configuration_file, self.uid, false)?;
        if raw(&mut current, 8_388_608, deadline, cancelled)? != self.configuration_raw
            || inode(
                &current.metadata().map_err(|_| {
                    SourceCommandError::Invalid("assessment PG current grant identity")
                })?,
            ) != self.configuration_identity
            || inode(&self.configuration_file.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment PG retained grant identity")
            })?) != self.configuration_identity
        {
            return Err(SourceCommandError::Conflict("assessment PG grant changed"));
        }
        let retained_parent = self
            .configuration_parent
            .metadata()
            .map_err(|_| SourceCommandError::Invalid("assessment PG retained parent"))?;
        let current_parent = tos_fd_open::open_absolute_directory(&self.configuration_parent_path)
            .map_err(|_| SourceCommandError::Conflict("assessment PG grant parent changed"))?;
        protected_private(&current_parent, self.uid, true)?;
        if inode(&retained_parent) != self.configuration_parent_identity
            || inode(&current_parent.metadata().map_err(|_| {
                SourceCommandError::Invalid("assessment PG current parent identity")
            })?) != self.configuration_parent_identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment PG grant parent identity changed",
            ));
        }
        Ok(())
    }
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
        Self::select_public_with_policy(
            configuration_path,
            source_root,
            allow_inline,
            AssessmentReadPolicy::Legacy,
            deadline,
            cancelled,
        )
    }

    pub(crate) fn select_public_command_with_read_limits(
        configuration_path: &Path,
        source_root: &Path,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        read.validate()?;
        Self::select_public_with_policy(
            configuration_path,
            source_root,
            true,
            AssessmentReadPolicy::Limited(read),
            deadline,
            cancelled,
        )
    }

    fn select_public_with_policy(
        configuration_path: &Path,
        source_root: &Path,
        allow_inline: bool,
        policy: AssessmentReadPolicy<'_>,
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
        let mut file = tos_fd_open::open_absolute_regular(
            configuration_path,
            u64::try_from(policy.config_cap())
                .map_err(|_| SourceCommandError::Invalid("assessment lower read limits"))?,
        )
        .map_err(|_| SourceCommandError::Denied("assessment protected configuration open"))?;
        protected(&file, uid, false)?;
        let configuration_raw = match policy {
            AssessmentReadPolicy::Legacy => raw(&mut file, 8_388_608, deadline, cancelled)?,
            AssessmentReadPolicy::Limited(read) => raw_with_length_limit(
                &mut file,
                ReadLength::AtMost(read.config_bytes),
                deadline,
                cancelled,
            )?,
        };
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
        selected.verify_current_with_policy(policy, deadline, cancelled)?;
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
        self.verify_current_with_policy(AssessmentReadPolicy::Legacy, deadline, cancelled)
    }

    pub(crate) fn verify_current_with_read_limits(
        &self,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        read.validate()?;
        self.verify_current_with_policy(AssessmentReadPolicy::Limited(read), deadline, cancelled)
    }

    fn verify_current_with_policy(
        &self,
        policy: AssessmentReadPolicy<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if let AssessmentReadPolicy::Limited(read) = policy {
            read.validate()?;
            if self.configuration_raw.len() > read.config_bytes {
                return Err(SourceCommandError::Invalid(
                    "assessment retained configuration read limit",
                ));
            }
        }
        active(deadline, cancelled)?;
        if rustix::process::getuid().as_raw() != self.uid
            || rustix::process::geteuid().as_raw() != self.uid
        {
            return Err(SourceCommandError::Denied("assessment account changed"));
        }
        protected_configuration_parents(&self.configuration_path, self.uid)?;
        let mut file = tos_fd_open::open_absolute_regular(
            &self.configuration_path,
            u64::try_from(policy.config_cap())
                .map_err(|_| SourceCommandError::Invalid("assessment lower read limits"))?,
        )
        .map_err(|_| SourceCommandError::Conflict("assessment configuration unavailable"))?;
        if self.private_root.is_some() {
            protected_private(&file, self.uid, false)?;
            protected_private(&self.configuration_file, self.uid, false)?;
        } else {
            protected(&file, self.uid, false)?;
            protected(&self.configuration_file, self.uid, false)?;
        }
        if match policy {
            AssessmentReadPolicy::Legacy => raw(&mut file, 8_388_608, deadline, cancelled)?,
            AssessmentReadPolicy::Limited(_) => raw_with_length_limit(
                &mut file,
                ReadLength::Exact(self.configuration_raw.len()),
                deadline,
                cancelled,
            )?,
        } != self.configuration_raw
        {
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
        self.lock_subjects_with_policy(
            subject_ids,
            AssessmentReadPolicy::Legacy,
            deadline,
            cancelled,
        )
    }

    pub(crate) fn lock_subjects_with_read_limits<'a>(
        &'a self,
        subject_ids: &[String],
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentJournalFence<'a>> {
        read.validate()?;
        self.lock_subjects_with_policy(
            subject_ids,
            AssessmentReadPolicy::Limited(read),
            deadline,
            cancelled,
        )
    }

    fn lock_subjects_with_policy<'a>(
        &'a self,
        subject_ids: &[String],
        policy: AssessmentReadPolicy<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentJournalFence<'a>> {
        self.verify_current_with_policy(policy, deadline, cancelled)?;
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
        self.verify_current_with_policy(policy, deadline, cancelled)?;
        Ok(AssessmentJournalFence { owner: self, held })
    }

    /// Select a fixed public-v2 read scope without creating homes or lock files.
    pub(crate) fn read_only_subjects<'a>(
        &'a self,
        subject_ids: &[String],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentJournalReadBatch<'a>> {
        self.read_only_subjects_with_policy(
            subject_ids,
            AssessmentReadPolicy::Legacy,
            deadline,
            cancelled,
        )
    }

    pub(crate) fn read_only_subjects_with_read_limits<'a>(
        &'a self,
        subject_ids: &[String],
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentJournalReadBatch<'a>> {
        read.validate()?;
        self.read_only_subjects_with_policy(
            subject_ids,
            AssessmentReadPolicy::Limited(read),
            deadline,
            cancelled,
        )
    }

    fn read_only_subjects_with_policy<'a>(
        &'a self,
        subject_ids: &[String],
        policy: AssessmentReadPolicy<'_>,
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
        self.verify_current_with_policy(policy, deadline, cancelled)?;
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
        selected.verify_current_with_policy(policy, deadline, cancelled)?;
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
    fn verify_current_with_read_limits(
        &self,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()>;
}
impl AssessmentJournalReadBatch<'_> {
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify_current_with_policy(AssessmentReadPolicy::Legacy, deadline, cancelled)
    }

    pub(crate) fn verify_current_with_read_limits(
        &self,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        read.validate()?;
        self.verify_current_with_policy(AssessmentReadPolicy::Limited(read), deadline, cancelled)
    }

    fn verify_current_with_policy(
        &self,
        policy: AssessmentReadPolicy<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.owner
            .verify_current_with_policy(policy, deadline, cancelled)?;
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
        self.owner
            .verify_current_with_policy(policy, deadline, cancelled)
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
    pub(crate) fn read_with_read_limits(
        &self,
        subject: &str,
        ctx: &CommandContext,
        worker: &mut CutWorkerSchemaExecutor,
        expected: ExactJournalHead<'_>,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentHistory> {
        read_assessment_history_with_policy(
            self,
            subject,
            ctx,
            worker,
            AssessmentReadPolicy::Limited(read),
            AssessmentHeadSelection::Exact(expected),
            deadline,
            cancelled,
        )
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
    fn verify_current_with_read_limits(
        &self,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        AssessmentJournalFence::verify_current_with_read_limits(self, read, deadline, cancelled)
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
    fn verify_current_with_read_limits(
        &self,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        AssessmentJournalReadBatch::verify_current_with_read_limits(self, read, deadline, cancelled)
    }
}
pub(crate) struct AssessmentHistory {
    subject_id: String,
    pub(crate) head: Option<String>,
    pub(crate) batches: Vec<JsonValue>,
    pub(crate) submissions: Vec<tos_validation::assessment::AssessmentSubmissionInput>,
    members: Vec<HeldJournalMember>,
    input_bytes: usize,
    capacity: HistoryCapacity,
    storage_envelope: StorageEnvelope,
}

/// Assessment's PG package writer. The only package members are the current
/// head and the newest immutable batch; older batches remain in broker-owned
/// immutable package history and are reconstructed by `from_private_package_history_selected_schema`.
pub(crate) struct AssessmentPrivatePackageCapabilityV1 {
    write: PrivateOwnerPackageWriteV1,
    package_digest: Digest256,
    expected_history_heads: Option<BTreeMap<String, Option<Vec<u8>>>>,
}

impl AssessmentPrivatePackageCapabilityV1 {
    /// Bind one already-evaluated immutable assessment batch to the current
    /// package snapshot. Semantic-history replay must be resolved by the owner
    /// before this new-append capability is built; the broker receipt handles
    /// only recovery of the same committed append.
    pub(crate) fn from_batch(
        mut write: PrivateOwnerPackageWriteV1,
        batch: &JsonValue,
    ) -> DurableResult<Self> {
        let batch_bytes = cmd::canonical(batch).map_err(DurableError::Source)?;
        let head = cmd::record_digest(batch)
            .map_err(DurableError::Source)?
            .to_hex();
        write.members = BTreeMap::from([
            ("batch.json".to_owned(), batch_bytes),
            ("head".to_owned(), head.into_bytes()),
        ]);
        Self::new(write)
    }

    pub(crate) fn new(write: PrivateOwnerPackageWriteV1) -> DurableResult<Self> {
        if write.family != PrivateOwnerPackageFamilyV1::Assessment
            || !write.archives.is_empty()
            || write.members.len() != 2
            || !write.members.contains_key("head")
            || !write.members.contains_key("batch.json")
        {
            return Err(DurableError::Invalid(
                "assessment private package write shape",
            ));
        }
        if let Some(current) = &write.expected_current {
            if current.scope != write.scope
                || current.target_ref != write.target_ref
                || current.family != PrivateOwnerPackageFamilyV1::Assessment
                || current.private_audit_generation
                    != write.source_binding.private_owner_audit_generation
            {
                return Err(DurableError::Conflict(
                    "assessment private package current snapshot differs",
                ));
            }
        }
        let head = write.members.get("head").unwrap();
        let batch_bytes = write.members.get("batch.json").unwrap();
        if head.len() > 65 || batch_bytes.len() > MAX_BATCH_BYTES {
            return Err(DurableError::Invalid(
                "assessment private package member budget",
            ));
        }
        let batch = cmd::parse(batch_bytes).map_err(DurableError::Source)?;
        let revision = cmd::record_digest(&batch)
            .map_err(DurableError::Source)?
            .to_hex();
        if parse_head_bytes(head).map_err(DurableError::Source)? != revision {
            return Err(DurableError::Invalid(
                "assessment private package head differs from batch",
            ));
        }
        let expected_previous = write
            .expected_current
            .as_ref()
            .map(|snapshot| snapshot.members.get("head"))
            .flatten()
            .map(|bytes| parse_head_bytes(bytes).map_err(DurableError::Source))
            .transpose()?
            .map(|revision| cmd::string(&revision))
            .unwrap_or(JsonValue::Null);
        if !cmd::same(
            cmd::field(&batch, "previous_revision").map_err(DurableError::Source)?,
            &expected_previous,
        )
        .map_err(DurableError::Source)?
        {
            return Err(DurableError::Conflict(
                "assessment private batch predecessor differs",
            ));
        }
        let request = cmd::field(&batch, "request").map_err(DurableError::Source)?;
        if cmd::text(request, "command_id").map_err(DurableError::Source)? != write.command_id
            || cmd::record_digest(request).map_err(DurableError::Source)? != write.request_digest
        {
            return Err(DurableError::Conflict(
                "assessment private package command binding differs",
            ));
        }
        let package_digest = crate::durable_private_packages::package_digest(&write.members)?;
        Ok(Self {
            write,
            package_digest,
            expected_history_heads: None,
        })
    }

    pub(crate) fn with_expected_history_heads(
        mut self,
        expected_heads: BTreeMap<String, Option<String>>,
    ) -> DurableResult<Self> {
        if expected_heads.is_empty()
            || expected_heads.len() > crate::durable_private_packages::MAX_PACKAGE_HISTORY_TARGETS
            || !expected_heads.contains_key(&self.write.target_ref)
        {
            return Err(DurableError::Invalid(
                "assessment private expected history head closure",
            ));
        }
        let expected_heads = expected_heads
            .into_iter()
            .map(|(target, head)| (target, head.map(String::into_bytes)))
            .collect::<BTreeMap<_, _>>();
        let target_head =
            expected_heads
                .get(&self.write.target_ref)
                .ok_or(DurableError::Invalid(
                    "assessment private target head absent from closure",
                ))?;
        let current_head = self
            .write
            .expected_current
            .as_ref()
            .and_then(|current| current.members.get("head"));
        if current_head != target_head.as_ref() {
            return Err(DurableError::Conflict(
                "assessment expected target head differs from append snapshot",
            ));
        }
        self.expected_history_heads = Some(expected_heads);
        Ok(self)
    }
}

/// Commit an Assessment append while rechecking its protected owner inputs
/// from inside the broker's exclusive scope transaction. The callback runs
/// for both a fresh append and a matching broker receipt replay.
pub(crate) fn commit_private_assessment_batch_with_expected_heads_and_currentness<F>(
    coordinator: &mut DurablePgCoordinator,
    write: PrivateOwnerPackageWriteV1,
    batch: &JsonValue,
    expected_heads: BTreeMap<String, Option<String>>,
    revalidate_currentness: F,
) -> DurableResult<PrivateOwnerPackageReplayV1>
where
    F: FnMut() -> SourceCommandResult<()>,
{
    let inner = AssessmentPrivatePackageCapabilityV1::from_batch(write, batch)?
        .with_expected_history_heads(expected_heads)?;
    let mut capability = AssessmentPrivatePackageCurrentCapabilityV1 {
        inner,
        revalidate_currentness,
    };
    coordinator.commit_private_owner_package(&mut capability)
}

impl capability_seal::Sealed for AssessmentPrivatePackageCapabilityV1 {}

impl PrivateOwnerPackageCapabilityV1 for AssessmentPrivatePackageCapabilityV1 {
    fn write(&self) -> &PrivateOwnerPackageWriteV1 {
        &self.write
    }

    fn expected_history_heads(&self) -> Option<&BTreeMap<String, Option<Vec<u8>>>> {
        self.expected_history_heads.as_ref()
    }

    fn revalidate(
        &mut self,
        current: Option<&PrivateOwnerPackageSnapshotV1>,
        replay: Option<&PrivateOwnerPackageReplayV1>,
    ) -> DurableResult<()> {
        if replay.is_none() && current != self.write.expected_current.as_ref() {
            return Err(DurableError::Conflict(
                "assessment private package current snapshot changed",
            ));
        }
        if current.is_some_and(|snapshot| {
            snapshot.scope != self.write.scope
                || snapshot.target_ref != self.write.target_ref
                || snapshot.family != PrivateOwnerPackageFamilyV1::Assessment
        }) {
            return Err(DurableError::Conflict(
                "assessment private package identity changed",
            ));
        }
        if let Some(replay) = replay {
            if current.map(|snapshot| snapshot.package_digest) != Some(replay.package_digest)
                || replay.package_digest != self.package_digest
                || replay.response_bytes != self.write.response_bytes
            {
                return Err(DurableError::Conflict(
                    "assessment private package replay differs",
                ));
            }
        }
        Ok(())
    }
}

struct AssessmentPrivatePackageCurrentCapabilityV1<F> {
    inner: AssessmentPrivatePackageCapabilityV1,
    revalidate_currentness: F,
}

impl<F> capability_seal::Sealed for AssessmentPrivatePackageCurrentCapabilityV1<F> {}

impl<F> PrivateOwnerPackageCapabilityV1 for AssessmentPrivatePackageCurrentCapabilityV1<F>
where
    F: FnMut() -> SourceCommandResult<()>,
{
    fn write(&self) -> &PrivateOwnerPackageWriteV1 {
        &self.inner.write
    }

    fn expected_history_heads(&self) -> Option<&BTreeMap<String, Option<Vec<u8>>>> {
        self.inner.expected_history_heads.as_ref()
    }

    fn revalidate(
        &mut self,
        current: Option<&PrivateOwnerPackageSnapshotV1>,
        replay: Option<&PrivateOwnerPackageReplayV1>,
    ) -> DurableResult<()> {
        self.inner.revalidate(current, replay)?;
        (self.revalidate_currentness)().map_err(DurableError::Source)
    }
}

/// Selected-input guard for an Assessment operation whose exact private
/// Profile/Claim source packages must be read from the same held broker
/// snapshot as its assessment histories. Source package bytes are returned
/// only for caller-authorized target refs; this helper does not enumerate the
/// owner namespace or widen the grant.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_private_assessment_and_source_histories_guard_selected_schema<
    I: Copy + Eq,
    T,
>(
    coordinator: &mut DurablePgCoordinator,
    scope: &PrivateOwnerScopeV1,
    subject_targets: &[(String, String)],
    source_target_refs: &[String],
    expected_heads: &BTreeMap<String, Option<String>>,
    batch_schema: &[u8],
    input_identity: &I,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    max_total_rows: usize,
    max_total_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    callback: impl FnOnce(
        &BTreeMap<String, AssessmentHistory>,
        &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
        &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
        &mut CandidateCutWorkerSchemaExecutor<I>,
        Instant,
        &AtomicBool,
    ) -> DurableResult<T>,
) -> DurableResult<T> {
    with_private_assessment_and_source_histories_and_read_session_guard_selected_schema(
        coordinator,
        scope,
        subject_targets,
        source_target_refs,
        expected_heads,
        batch_schema,
        input_identity,
        worker,
        max_total_rows,
        max_total_bytes,
        deadline,
        cancelled,
        |histories, packages, sources, _reader, worker, deadline, cancelled| {
            callback(histories, packages, sources, worker, deadline, cancelled)
        },
    )
    .map_err(crate::durable_private_packages::PrivateOwnerReadFailure::legacy_error)
}

/// Finalize the actual worker after the WHOLE worker-using history body,
/// including restoration/head/source-collection errors before user callback.
/// Ordinary Result exits only: a panic is not a successful/fenced result.
fn with_private_history_worker_exit<W: CutSchemaExecutor, T>(
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
    body: impl FnOnce(&mut W) -> DurableResult<T>,
) -> DurableResult<T> {
    let result = body(worker);
    let fenced = crate::source_sign::finish_worker(worker, deadline, cancelled)
        .map_err(DurableError::Source);
    match (result, fenced) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// The actual selected-schema Assessment history guard with a lexical reader
/// for only its selected private targets, on the SAME owner RR transaction.
/// It adds no writer grant, source admission, or fabricated history snapshot.
pub(crate) fn with_private_assessment_and_source_histories_and_read_session_guard_selected_schema<
    I: Copy + Eq,
    T,
>(
    coordinator: &mut DurablePgCoordinator,
    scope: &PrivateOwnerScopeV1,
    subject_targets: &[(String, String)],
    source_target_refs: &[String],
    expected_heads: &BTreeMap<String, Option<String>>,
    batch_schema: &[u8],
    input_identity: &I,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    max_total_rows: usize,
    max_total_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
    callback: impl FnOnce(
        &BTreeMap<String, AssessmentHistory>,
        &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
        &BTreeMap<String, PrivateOwnerPackageHistoryV1>,
        &mut crate::durable_private_packages::PrivateOwnerSelectedHistoryRead<'_, '_, '_>,
        &mut CandidateCutWorkerSchemaExecutor<I>,
        Instant,
        &AtomicBool,
    ) -> DurableResult<T>,
) -> crate::durable_private_packages::PrivateOwnerReadResult<T> {
    if subject_targets.is_empty()
        || subject_targets.len() > crate::durable_private_packages::MAX_PACKAGE_HISTORY_TARGETS
        || expected_heads.len() != subject_targets.len()
        || max_total_rows == 0
        || max_total_rows > crate::durable_private_packages::MAX_PACKAGE_HISTORY_ROWS
        || max_total_bytes == 0
        || max_total_bytes > crate::durable_private_packages::MAX_PACKAGE_HISTORY_BYTES
    {
        return Err(
            DurableError::Invalid("assessment private history closure count or budget").into(),
        );
    }
    let mut subject_ids = BTreeSet::new();
    let mut subject_homes = BTreeSet::new();
    let mut target_refs = Vec::with_capacity(subject_targets.len() + source_target_refs.len());
    for (subject_id, target_ref) in subject_targets {
        if subject_id.len() > 1_048_576
            || tos_foundation::python_strip_unicode16_v1(subject_id, 1_048_576)
                .map_err(|_| DurableError::Invalid("assessment subject Unicode budget"))?
                .is_empty()
            || !subject_ids.insert(subject_id.as_str())
            || !subject_homes.insert(Digest256::of_bytes(subject_id.as_bytes()))
            || !expected_heads.contains_key(subject_id)
        {
            return Err(
                DurableError::Invalid("assessment distinct private history subjects").into(),
            );
        }
        target_refs.push(target_ref.clone());
    }
    if expected_heads
        .keys()
        .any(|subject| !subject_ids.contains(subject.as_str()))
    {
        return Err(DurableError::Invalid("assessment private history expected-head set").into());
    }
    let mut all_targets = target_refs.iter().cloned().collect::<BTreeSet<_>>();
    for target_ref in source_target_refs {
        if !all_targets.insert(target_ref.clone())
            || all_targets.len() > crate::durable_private_packages::MAX_PACKAGE_HISTORY_TARGETS
        {
            return Err(DurableError::Invalid("assessment private source target closure").into());
        }
        target_refs.push(target_ref.clone());
    }
    coordinator.with_private_owner_package_histories_and_read_session_guard(
        scope,
        &target_refs,
        max_total_rows,
        max_total_bytes,
        deadline,
        cancelled,
        |package_histories, selected_reader, held_deadline, held_cancelled| {
            with_private_history_worker_exit(worker, held_deadline, held_cancelled, |worker| {
                let mut histories = BTreeMap::new();
                let mut packages_by_subject = BTreeMap::new();
                for (subject_id, target_ref) in subject_targets {
                    active(held_deadline, held_cancelled).map_err(DurableError::Source)?;
                    let package =
                        package_histories
                            .get(target_ref)
                            .ok_or(DurableError::Corrupt(
                                "assessment private history target absent",
                            ))?;
                    let history = AssessmentHistory::from_private_package_history_selected_schema(
                        package,
                        scope,
                        target_ref,
                        subject_id,
                        batch_schema,
                        input_identity,
                        worker,
                        held_deadline,
                        held_cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    if history.head.as_deref()
                        != expected_heads
                            .get(subject_id)
                            .and_then(|expected| expected.as_deref())
                    {
                        return Err(DurableError::Source(SourceCommandError::Conflict(
                            "assessment private history head changed",
                        )));
                    }
                    histories.insert(subject_id.clone(), history);
                    packages_by_subject.insert(subject_id.clone(), package.clone());
                }
                let source_packages = source_target_refs
                    .iter()
                    .map(|target_ref| {
                        let package =
                            package_histories
                                .get(target_ref)
                                .ok_or(DurableError::Corrupt(
                                    "assessment private source package absent",
                                ))?;
                        Ok((target_ref.clone(), package.clone()))
                    })
                    .collect::<DurableResult<BTreeMap<_, _>>>()?;
                callback(
                    &histories,
                    &packages_by_subject,
                    &source_packages,
                    selected_reader,
                    worker,
                    held_deadline,
                    held_cancelled,
                )
            })
        },
    )
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
    /// Check that a stored admission references only exact immutable
    /// assessment records in this selected history. This supports idempotent
    /// ready-result recovery where the owner must validate the persisted
    /// decision against a guarded head without repeating semantic evaluation.
    pub(crate) fn contains_exact_assessment_refs(
        &self,
        references: &[JsonValue],
    ) -> SourceCommandResult<bool> {
        let mut history_refs = BTreeMap::<String, JsonValue>::new();
        for submission in &self.submissions {
            let assessment = cmd::parse(&submission.assessment)?;
            let id = cmd::text(&assessment, "assessment_id")?.to_owned();
            let reference = cmd::object(vec![
                ("id", cmd::string(&id)),
                ("version", cmd::number(1)),
                (
                    "digest",
                    cmd::string(&cmd::record_digest(&assessment)?.to_prefixed()),
                ),
            ]);
            if let Some(previous) = history_refs.get(&id) {
                if !cmd::same(previous, &reference)? {
                    return Err(SourceCommandError::Conflict(
                        "assessment history identity collision",
                    ));
                }
            } else {
                history_refs.insert(id, reference);
            }
        }
        for reference in references {
            cmd::exact_keys(reference, &["id", "version", "digest"])?;
            if cmd::integer(reference, "version")? != 1
                || Digest256::from_prefixed(cmd::text(reference, "digest")?).is_err()
            {
                return Err(SourceCommandError::Invalid(
                    "assessment history reference shape",
                ));
            }
            let id = cmd::text(reference, "id")?;
            if history_refs
                .get(id)
                .is_none_or(|current| current != reference)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Selected-input counterpart that authenticates the exact schema bytes
    /// without requiring the legacy `CommandContext` revision field.
    pub(crate) fn from_private_package_history_selected_schema<I: Copy + Eq>(
        package_history: &PrivateOwnerPackageHistoryV1,
        scope: &PrivateOwnerScopeV1,
        target_ref: &str,
        subject: &str,
        batch_schema: &[u8],
        input_identity: &I,
        worker: &mut CandidateCutWorkerSchemaExecutor<I>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        if worker.input_identity() != input_identity
            || worker.profile() != FormatProfile::AssertedSourceCandidateV1
        {
            return Err(SourceCommandError::Conflict(
                "assessment selected schema worker identity or profile differs",
            ));
        }
        Self::from_private_package_history_with_worker(
            package_history,
            scope,
            target_ref,
            subject,
            batch_schema,
            worker,
            None,
            deadline,
            cancelled,
        )
    }

    fn from_private_package_history_with_worker<W: CutSchemaExecutor>(
        package_history: &PrivateOwnerPackageHistoryV1,
        scope: &PrivateOwnerScopeV1,
        target_ref: &str,
        subject: &str,
        batch_schema: &[u8],
        worker: &mut W,
        expected_source_revision: Option<SourceRevision>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        if package_history.scope != *scope || package_history.target_ref != target_ref {
            return Err(SourceCommandError::Conflict(
                "assessment private package scope or target changed",
            ));
        }
        if expected_source_revision
            .is_some_and(|revision| worker.selected_source_revision() != Some(revision))
        {
            return Err(SourceCommandError::Conflict(
                "assessment journal schema/source cut differs",
            ));
        }
        if worker.selected_contract_digest(BATCH_SCHEMA) != Some(Digest256::of_bytes(batch_schema))
        {
            return Err(SourceCommandError::Conflict(
                "assessment journal schema/source cut differs",
            ));
        }

        let Some(current) = package_history.current.as_ref() else {
            if package_history.family.is_some() || !package_history.history.is_empty() {
                return Err(SourceCommandError::Invalid(
                    "assessment private history has no current package",
                ));
            }
            return Ok(Self {
                subject_id: subject.to_owned(),
                head: None,
                batches: Vec::new(),
                submissions: Vec::new(),
                members: Vec::new(),
                input_bytes: 0,
                capacity: HistoryCapacity::EMPTY,
                storage_envelope: StorageEnvelope::PgPackage,
            });
        };
        if package_history.family != Some(PrivateOwnerPackageFamilyV1::Assessment)
            || current.scope != *scope
            || current.target_ref != target_ref
            || current.family != PrivateOwnerPackageFamilyV1::Assessment
            || package_history.history.is_empty()
        {
            return Err(SourceCommandError::Conflict(
                "assessment private package family or history changed",
            ));
        }

        let mut batches = Vec::with_capacity(package_history.history.len());
        let mut head = None;
        let mut package_digest = None;
        let mut total_bytes = 0usize;
        for (index, entry) in package_history.history.iter().enumerate() {
            active(deadline, cancelled)?;
            if entry.revision != (index + 1) as u64
                || entry.family != PrivateOwnerPackageFamilyV1::Assessment
                || entry.previous_digest != package_digest
                || entry.members.len() != 2
                || !entry.members.contains_key("head")
                || !entry.members.contains_key("batch.json")
            {
                return Err(SourceCommandError::Invalid(
                    "assessment private package history shape",
                ));
            }
            let head_bytes = entry.members.get("head").unwrap();
            let batch_bytes = entry.members.get("batch.json").unwrap();
            if head_bytes.len() > 65 || batch_bytes.len() > MAX_BATCH_BYTES {
                return Err(SourceCommandError::Invalid(
                    "assessment private package member budget",
                ));
            }
            total_bytes = total_bytes
                .checked_add(head_bytes.len())
                .and_then(|total| total.checked_add(batch_bytes.len()))
                .filter(|total| *total <= MAX_HISTORY_BYTES)
                .ok_or(SourceCommandError::Invalid(
                    "assessment complete history byte budget",
                ))?;
            let batch = cmd::parse(batch_bytes)?;
            let revision = cmd::record_digest(&batch)?.to_hex();
            let stored_head = parse_head_bytes(head_bytes)?;
            if stored_head != revision {
                return Err(SourceCommandError::Invalid(
                    "assessment private package head differs from batch",
                ));
            }
            if head.as_deref().is_some_and(|previous| previous == revision) {
                return Err(SourceCommandError::Invalid(
                    "assessment duplicate private batch revision",
                ));
            }
            head = Some(revision);
            package_digest = Some(entry.package_digest);
            batches.push(batch);
        }
        let latest = package_history.history.last().unwrap();
        if current.revision != latest.revision
            || current.package_digest != latest.package_digest
            || current.members != latest.members
            || current.private_audit_generation != package_history.private_audit_generation
            || package_history.history.len() > MAX_EVENTS
        {
            return Err(SourceCommandError::Conflict(
                "assessment current package differs from immutable history",
            ));
        }
        let current_head = current
            .members
            .get("head")
            .ok_or(SourceCommandError::Invalid(
                "assessment current package head absent",
            ))?;
        if Some(parse_head_bytes(current_head)?) != head {
            return Err(SourceCommandError::Conflict(
                "assessment current package head differs from history",
            ));
        }
        let (submissions, canonical_bytes) = validate_assessment_history_batches(
            subject,
            head.as_deref(),
            &batches,
            total_bytes,
            batch_schema,
            worker,
            expected_source_revision,
            deadline,
            cancelled,
        )?;
        let capacity = HistoryCapacity::observed(
            total_bytes,
            canonical_bytes,
            batches.len(),
            submissions.len(),
            Limits::V1,
        )
        .map_err(capacity_refusal)?;
        Ok(Self {
            subject_id: subject.to_owned(),
            head,
            batches,
            submissions,
            members: Vec::new(),
            input_bytes: total_bytes,
            capacity,
            storage_envelope: StorageEnvelope::PgPackage,
        })
    }

    /// A new payload uses the same capacity predicate as authenticated reader
    /// state. Replay, qualification and dedup stay in common prepare. This does
    /// not replace currentness, source fences, worker checks or operation costs.
    pub(crate) fn checked_successor_payload(
        &self,
        batch: &JsonValue,
    ) -> SourceCommandResult<Vec<u8>> {
        let payload = cmd::canonical(batch)?;
        // Check the capacity prefix before parsing events, preserving the
        // existing malformed-batch refusal order. Both calls use the same pure
        // O(1) predicate; zero is only a precheck, never a published event count.
        self.capacity
            .append(payload.len(), 0, self.storage_envelope, Limits::V1)
            .map_err(capacity_refusal)?;
        self.capacity
            .append(
                payload.len(),
                cmd::array(batch, "events")?.len(),
                self.storage_envelope,
                Limits::V1,
            )
            .map_err(capacity_refusal)?;
        Ok(payload)
    }

    /// Raw authenticated journal bytes retained by this selected history.
    /// Callers charge this once against the operation-wide read budget before
    /// evaluating current admission or attempting a compare-and-swap write.
    pub(crate) fn input_bytes(&self) -> SourceCommandResult<usize> {
        Ok(self.input_bytes)
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
        self.verify_members_with_policy(
            fence,
            include_head,
            AssessmentReadPolicy::Legacy,
            deadline,
            cancelled,
        )
    }

    pub(crate) fn verify_current_with_read_limits<V: AssessmentJournalReadView>(
        &self,
        view: &V,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        read.validate()?;
        self.verify_members_with_policy(
            view,
            true,
            AssessmentReadPolicy::Limited(read),
            deadline,
            cancelled,
        )
    }

    fn verify_members_with_policy<V: AssessmentJournalReadView>(
        &self,
        fence: &V,
        include_head: bool,
        policy: AssessmentReadPolicy<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        verify_view_current(fence, policy, deadline, cancelled)?;
        for member in &self.members {
            if !include_head && member.name == "head" {
                continue;
            }
            active(deadline, cancelled)?;
            verify_member_current_with_policy(fence, member, policy, deadline, cancelled)?;
        }
        if include_head && self.head.is_none() {
            verify_head_absent_with_policy(fence, &self.subject_id, policy, deadline, cancelled)?;
        }
        verify_view_current(fence, policy, deadline, cancelled)?;
        Ok(())
    }
}

fn verify_head_absent_with_policy<V: AssessmentJournalReadView>(
    view: &V,
    subject: &str,
    _policy: AssessmentReadPolicy<'_>,
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
    verify_member_current_with_policy(
        fence,
        member,
        AssessmentReadPolicy::Legacy,
        deadline,
        cancelled,
    )
}

fn verify_member_current_with_policy<V: AssessmentJournalReadView>(
    fence: &V,
    member: &HeldJournalMember,
    policy: AssessmentReadPolicy<'_>,
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
    let bytes = match policy {
        AssessmentReadPolicy::Legacy => {
            raw(&mut current, MAX_BATCH_BYTES.max(65), deadline, cancelled)?
        }
        AssessmentReadPolicy::Limited(read) => {
            let cap = if member.name == "head" {
                65
            } else {
                read.batch_bytes
            };
            if member.size > cap {
                return Err(SourceCommandError::Invalid(
                    "assessment retained member read limit",
                ));
            }
            raw_with_length_limit(
                &mut current,
                ReadLength::Exact(member.size),
                deadline,
                cancelled,
            )?
        }
    };
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
        self.verify_current_with_policy(AssessmentReadPolicy::Legacy, deadline, cancelled)
    }

    pub(crate) fn verify_current_with_read_limits(
        &self,
        read: &C01OwnerReadLimits,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        read.validate()?;
        self.verify_current_with_policy(AssessmentReadPolicy::Limited(read), deadline, cancelled)
    }

    fn verify_current_with_policy(
        &self,
        policy: AssessmentReadPolicy<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.owner
            .verify_current_with_policy(policy, deadline, cancelled)?;
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
        let payload = history.checked_successor_payload(batch)?;
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

fn parse_head_bytes(bytes: &[u8]) -> SourceCommandResult<String> {
    if bytes.len() > 65 || !bytes.is_ascii() {
        return Err(SourceCommandError::Invalid("assessment head ASCII"));
    }
    let text = tos_foundation::python_strip_unicode16_v1(std::str::from_utf8(bytes).unwrap(), 65)
        .map_err(|_| SourceCommandError::Invalid("assessment head whitespace"))?;
    if !digest(text) {
        return Err(SourceCommandError::Invalid("assessment head digest"));
    }
    Ok(text.to_owned())
}

fn validate_assessment_history_batches(
    subject: &str,
    head: Option<&str>,
    chain: &[JsonValue],
    total_bytes: usize,
    batch_schema: &[u8],
    worker: &mut impl CutSchemaExecutor,
    expected_source_revision: Option<SourceRevision>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    Vec<tos_validation::assessment::AssessmentSubmissionInput>,
    usize,
)> {
    if total_bytes > MAX_HISTORY_BYTES || chain.len() > MAX_EVENTS {
        return Err(SourceCommandError::Invalid(
            "assessment complete history budget",
        ));
    }
    if expected_source_revision
        .is_some_and(|revision| worker.selected_source_revision() != Some(revision))
        || worker.selected_contract_digest(BATCH_SCHEMA) != Some(Digest256::of_bytes(batch_schema))
    {
        return Err(SourceCommandError::Conflict(
            "assessment journal schema/source cut differs",
        ));
    }
    let mut known = BTreeMap::<String, Vec<u8>>::new();
    let mut submissions = Vec::new();
    let mut batch_bytes_total = 0usize;
    let mut event_count = 0usize;
    let mut revisions = BTreeSet::new();
    let mut previous_revision: Option<String> = None;
    for (index, batch) in chain.iter().enumerate() {
        active(deadline, cancelled)?;
        let canonical = cmd::canonical(batch)?;
        batch_bytes_total = batch_bytes_total
            .checked_add(canonical.len())
            .filter(|total| *total <= MAX_HISTORY_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "assessment complete history byte budget",
            ))?;
        if canonical.len() > MAX_BATCH_BYTES {
            return Err(SourceCommandError::Invalid("assessment batch byte budget"));
        }
        let revision = cmd::record_digest(batch)?.to_hex();
        if !revisions.insert(revision.clone())
            || !worker
                .check(
                    &format!("assessment-journal/{revision}.json"),
                    &canonical,
                    BATCH_SCHEMA,
                    deadline,
                    cancelled,
                )
                .map_err(|_| SourceCommandError::Invalid("assessment batch schema execution"))?
        {
            return Err(SourceCommandError::Invalid("assessment batch schema"));
        }
        let expected_previous = previous_revision
            .as_deref()
            .map_or(JsonValue::Null, cmd::string);
        if cmd::integer(batch, "sequence")? != (index + 1) as u64
            || !cmd::same(cmd::field(batch, "previous_revision")?, &expected_previous)?
            || (index == 0) != previous_revision.is_none()
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
        if cmd::text(batch, "schema_version")? != "tos_assessment_batch_v1"
            || cmd::text(batch, "subject_id")? != subject
            || cmd::text(cmd::field(request, "subject")?, "id")? != subject
            || cmd::record_digest(request)?.to_hex() != cmd::text(batch, "request_digest")?
            || !cmd::same(
                cmd::field(request, "expected_revision")?,
                cmd::field(batch, "previous_revision")?,
            )?
            || (index + 1 == chain.len() && head != Some(revision.as_str()))
        {
            return Err(SourceCommandError::Invalid(
                "assessment batch/request identity binding",
            ));
        }
        let events = cmd::array(batch, "events")?;
        if events.is_empty() {
            return Err(SourceCommandError::Invalid("assessment empty batch"));
        }
        event_count = event_count
            .checked_add(events.len())
            .filter(|count| *count <= MAX_EVENTS)
            .ok_or(SourceCommandError::Invalid(
                "assessment complete history event budget",
            ))?;
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
        for event in events {
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
        previous_revision = Some(revision);
    }
    if chain.is_empty() != head.is_none()
        || chain
            .last()
            .map(cmd::record_digest)
            .transpose()?
            .map(|value| value.to_hex())
            .as_deref()
            != head
    {
        return Err(SourceCommandError::Invalid(
            "assessment history final head binding",
        ));
    }
    Ok((submissions, batch_bytes_total))
}

fn verify_view_current<V: AssessmentJournalReadView>(
    view: &V,
    policy: AssessmentReadPolicy<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    match policy {
        AssessmentReadPolicy::Legacy => view.verify_current(deadline, cancelled),
        AssessmentReadPolicy::Limited(read) => {
            view.verify_current_with_read_limits(read, deadline, cancelled)
        }
    }
}

fn read_head<V: AssessmentJournalReadView>(
    view: &V,
    subject: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(Option<String>, Option<HeldJournalMember>)> {
    read_head_with_policy(
        view,
        subject,
        AssessmentReadPolicy::Legacy,
        deadline,
        cancelled,
    )
}

fn read_head_with_policy<V: AssessmentJournalReadView>(
    view: &V,
    subject: &str,
    policy: AssessmentReadPolicy<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(Option<String>, Option<HeldJournalMember>)> {
    verify_view_current(view, policy, deadline, cancelled)?;
    let home = view.home(subject)?;
    let owner = view.owner();
    let Some(directory) = home.directory else {
        verify_view_current(view, policy, deadline, cancelled)?;
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
        let raw = match policy {
            AssessmentReadPolicy::Legacy => raw(&mut file, 65, deadline, cancelled)?,
            AssessmentReadPolicy::Limited(_) => {
                raw_with_length_limit(&mut file, ReadLength::AtMost(65), deadline, cancelled)?
            }
        };
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
    verify_view_current(view, policy, deadline, cancelled)?;
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
    read_assessment_history_with_policy(
        view,
        subject,
        ctx,
        worker,
        AssessmentReadPolicy::Legacy,
        AssessmentHeadSelection::LegacyUnpinned,
        deadline,
        cancelled,
    )
}

fn read_assessment_history_with_policy<V: AssessmentJournalReadView>(
    view: &V,
    subject: &str,
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    policy: AssessmentReadPolicy<'_>,
    expected: AssessmentHeadSelection<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AssessmentHistory> {
    policy.validate()?;
    if let AssessmentHeadSelection::Exact(head) = expected {
        head.validate()?;
    }
    let (head, head_member) = read_head_with_policy(view, subject, policy, deadline, cancelled)?;
    if let AssessmentHeadSelection::Exact(expected) = expected {
        if !expected.matches(head.as_deref()) {
            return Err(SourceCommandError::Conflict(
                "assessment selected history head changed",
            ));
        }
    }
    let home = view.home(subject)?;
    let owner = view.owner();
    let mut members = head_member.into_iter().collect::<Vec<_>>();
    let mut cursor = head.clone();
    let mut seen = BTreeSet::new();
    let mut chain = Vec::new();
    let mut byte_count = 0usize;
    while let Some(revision) = cursor {
        active(deadline, cancelled)?;
        if !digest(&revision) || !seen.insert(revision.clone()) || chain.len() >= MAX_EVENTS {
            return Err(SourceCommandError::Invalid(
                "assessment cyclic/oversized history",
            ));
        }
        if let AssessmentReadPolicy::Limited(read) = policy {
            if chain.len() >= read.batches {
                return Err(SourceCommandError::Invalid(
                    "assessment complete history budget",
                ));
            }
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
        let bytes = match policy {
            AssessmentReadPolicy::Legacy => raw(&mut file, MAX_BATCH_BYTES, deadline, cancelled)?,
            AssessmentReadPolicy::Limited(read) => {
                let remaining = read
                    .history_bytes
                    .checked_sub(byte_count)
                    .filter(|remaining| *remaining != 0)
                    .ok_or(SourceCommandError::Invalid(
                        "assessment complete history byte budget",
                    ))?;
                raw_with_length_limit(
                    &mut file,
                    ReadLength::AtMost(read.batch_bytes.min(remaining)),
                    deadline,
                    cancelled,
                )?
            }
        };
        byte_count = byte_count
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_HISTORY_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "assessment complete history byte budget",
            ))?;
        let batch = cmd::parse(&bytes)?;
        if cmd::record_digest(&batch)?.to_hex() != revision {
            return Err(SourceCommandError::Invalid(
                "assessment batch path digest differs",
            ));
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
    let schema_path = tos_foundation::RelativePath::parse(BATCH_SCHEMA).unwrap();
    let batch_schema = ctx
        .file(&schema_path)?
        .ok_or(SourceCommandError::Unsupported(
            "selected assessment batch schema absent",
        ))?;
    let (submissions, canonical_bytes) = validate_assessment_history_batches(
        subject,
        head.as_deref(),
        &chain,
        byte_count,
        batch_schema,
        worker,
        Some(ctx.base_revision),
        deadline,
        cancelled,
    )?;
    if let AssessmentReadPolicy::Limited(read) = policy {
        if canonical_bytes > read.history_bytes {
            return Err(SourceCommandError::Invalid(
                "assessment complete history byte budget",
            ));
        }
        if submissions.len() > read.events {
            return Err(SourceCommandError::Invalid(
                "assessment complete history event budget",
            ));
        }
    }
    verify_view_current(view, policy, deadline, cancelled)?;
    let input_bytes = members.iter().try_fold(0usize, |total, member| {
        total
            .checked_add(member.size)
            .ok_or(SourceCommandError::Invalid(
                "assessment history input byte total overflow",
            ))
    })?;
    let capacity = HistoryCapacity::observed(
        byte_count,
        canonical_bytes,
        chain.len(),
        submissions.len(),
        Limits::V1,
    )
    .map_err(capacity_refusal)?;
    let history = AssessmentHistory {
        subject_id: subject.to_owned(),
        head,
        batches: chain,
        submissions,
        members,
        input_bytes,
        capacity,
        storage_envelope: StorageEnvelope::Local,
    };
    history.verify_members_with_policy(view, true, policy, deadline, cancelled)?;
    Ok(history)
}

/// Test-only introspection; no publication or authority bypass.
#[cfg(test)]
pub(crate) fn capacity_limits_for_test() -> (usize, usize, usize) {
    (MAX_EVENTS, MAX_BATCH_BYTES, MAX_HISTORY_BYTES)
}

use crate::temporal_research_crosswalk::c01_each;

// Opt-in C01 transfer boundaries. Ordinary selectors/readers remain untouched.
impl C01OwnerReadLimits {
    fn c01_profile(
        &self,
        a: &mut crate::temporal_research_crosswalk::C01Admission<'_>,
    ) -> SourceCommandResult<()> {
        a.visit(64)?;
        self.validate()?;
        if self.config_bytes > 32768
            || self.batch_bytes > 8192
            || self.history_bytes > 16384
            || self.batches > 2
            || self.events > 2
        {
            return Err(SourceCommandError::Invalid("C01 lower read profile"));
        }
        Ok(())
    }
}
impl ProtectedAssessmentJournal {
    pub(crate) fn select_c01_owner_with_return_limit(
        configuration_path: &Path,
        source_root: &Path,
        read: &C01OwnerReadLimits,
        adapter: &mut crate::temporal_research_crosswalk::C01AdapterPayer<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        use crate::temporal_research_crosswalk::{C01Counter, C01Row};
        let mut a = adapter.reserve(C01Row::Owner)?;
        read.c01_profile(&mut a)?;
        let owner = match Self::select_public_command_with_read_limits(
            configuration_path,
            source_root,
            read,
            deadline,
            cancelled,
        ) {
            Ok(owner) => owner,
            Err(error) => return a.error(error),
        };
        a.public_v2(&owner.configuration)?;
        {
            let mut c = C01Counter::new(&mut a)?;
            owner.c01_count(&mut c)?;
        }
        Ok(owner)
    }
    fn c01_count<'v>(
        &'v self,
        c: &mut crate::temporal_research_crosswalk::C01Counter<'v, '_, '_>,
    ) -> SourceCommandResult<()> {
        let Self {
            configuration_path,
            configuration_raw,
            configuration,
            configuration_file,
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
            private_root,
            private_root_file,
            private_root_identity,
        } = self;
        c.root::<SourceCommandResult<Self>>()?;
        c.path(configuration_path)?;
        c.bytes(configuration_raw)?;
        c.json(configuration)?;
        c.path(configuration_parent_path)?;
        c.path(directory_path)?;
        c.path(directory_parent_path)?;
        c.visit(64)?;
        if let Some(path) = private_root {
            c.path(path)?;
        }
        // Each remaining field is inline fd/identity/account/Option state.
        c.visit(11)?;
        let _ = (
            configuration_file,
            configuration_identity,
            configuration_parent,
            configuration_parent_identity,
            directory,
            identity,
            directory_parent,
            directory_parent_identity,
            uid,
            private_root_file,
            private_root_identity,
        );
        Ok(())
    }
    pub(crate) fn read_only_subjects_c01_with_return_limit<'a>(
        &'a self,
        subject_ids: &[String],
        read: &C01OwnerReadLimits,
        adapter: &mut crate::temporal_research_crosswalk::C01AdapterPayer<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentJournalReadBatch<'a>> {
        use crate::temporal_research_crosswalk::{C01Counter, C01Row};
        let mut a = adapter.reserve(C01Row::ReadBatch)?;
        read.c01_profile(&mut a)?;
        a.visit(64)?;
        if subject_ids.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "C01 one-subject read scope",
            ));
        }
        let batch = match self.read_only_subjects_with_read_limits(
            subject_ids,
            read,
            deadline,
            cancelled,
        ) {
            Ok(batch) => batch,
            Err(error) => return a.error(error),
        };
        {
            let mut c = C01Counter::new(&mut a)?;
            c.root::<SourceCommandResult<AssessmentJournalReadBatch<'_>>>()?;
            let AssessmentJournalReadBatch { owner, subjects } = &batch;
            c.visit(1)?;
            let _ = owner; // borrowed owner, no duplicate charge
            c.visit(1)?;
            c.btree::<String, ReadOnlySubjectHome>(subjects.len())?;
            c01_each!(c, subjects, (subject, home), {
                c.visit(1)?;
                c.string(subject)?;
                let ReadOnlySubjectHome {
                    name,
                    directory,
                    identity,
                } = home;
                c.string(name)?;
                c.visit(2)?;
                let _ = (directory, identity);
            });
            c.visit(1)?;
        }
        Ok(batch)
    }
}
impl AssessmentJournalReadBatch<'_> {
    pub(crate) fn read_c01_history_with_return_limit(
        &self,
        subject: &str,
        ctx: &CommandContext,
        worker: &mut CutWorkerSchemaExecutor,
        expected: ExactJournalHead<'_>,
        read: &C01OwnerReadLimits,
        adapter: &mut crate::temporal_research_crosswalk::C01AdapterPayer<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<AssessmentHistory> {
        use crate::temporal_research_crosswalk::{C01Counter, C01Row};
        let mut a = adapter.reserve(C01Row::History)?;
        read.c01_profile(&mut a)?;
        let history = match self
            .read_with_read_limits(subject, ctx, worker, expected, read, deadline, cancelled)
        {
            Ok(history) => history,
            Err(error) => return a.error(error),
        };
        {
            let mut c = C01Counter::new(&mut a)?;
            let AssessmentHistory {
                subject_id,
                head,
                batches,
                submissions,
                members,
                input_bytes,
                capacity,
                storage_envelope,
            } = &history;
            c.root::<SourceCommandResult<AssessmentHistory>>()?;
            c.string(subject_id)?;
            c.visit(64)?;
            if let Some(head) = head {
                c.string(head)?;
            }
            c.json_vec(batches)?;
            c.slots::<tos_validation::assessment::AssessmentSubmissionInput>(
                submissions.capacity(),
            )?;
            c01_each!(c, submissions, submission, {
                c.visit(1)?;
                let tos_validation::assessment::AssessmentSubmissionInput {
                    assessment,
                    principal_id,
                    execution_profile,
                    committed_scope,
                } = submission;
                c.bytes(assessment)?;
                c.string(principal_id)?;
                c.bytes(execution_profile)?;
                c.visit(64)?;
                if let Some(scope) = committed_scope {
                    c.bytes(scope)?;
                }
            });
            c.visit(1)?;
            c.visit(1)?;
            c.slots::<HeldJournalMember>(members.capacity())?;
            c01_each!(c, members, member, {
                c.visit(1)?;
                let HeldJournalMember {
                    home_name,
                    name,
                    file,
                    identity,
                    digest,
                    size,
                } = member;
                c.string(home_name)?;
                c.string(name)?;
                c.visit(4)?;
                let _ = (file, identity, digest, size);
            });
            c.visit(4)?;
            let _ = (input_bytes, capacity, storage_envelope);
        }
        Ok(history)
    }
}

#[cfg(test)]
mod private_history_worker_exit_tests {
    use super::*;
    use std::{cell::Cell, time::Duration};
    use tos_validation::item_rules::ItemRefusal;

    // Negative control-flow probe only. It cannot accept a schema or issue an
    // execution binding, source permission, native receipt or successful Claim.
    struct RefusingWorker {
        schema: Digest256,
        checks: usize,
        exits: usize,
        exit_fails: bool,
    }
    impl CutSchemaExecutor for RefusingWorker {
        fn selected_contract_digest(&self, contract: &str) -> Option<Digest256> {
            (contract == BATCH_SCHEMA).then_some(self.schema)
        }
        fn check(
            &mut self,
            _path: &str,
            _raw: &[u8],
            _contract: &str,
            _deadline: Instant,
            _cancelled: &AtomicBool,
        ) -> Result<bool, ItemRefusal> {
            self.checks += 1;
            Ok(false)
        }
        fn finish(
            &mut self,
            _deadline: Instant,
            _cancelled: &AtomicBool,
        ) -> Result<(), ItemRefusal> {
            self.exits += 1;
            if self.exit_fails {
                Err(ItemRefusal::Deadline)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn history_schema_refusal_attempts_exit_before_user_callback() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let schema = b"{}";
        for exit_fails in [false, true] {
            let mut worker = RefusingWorker {
                schema: Digest256::of_bytes(schema),
                checks: 0,
                exits: 0,
                exit_fails,
            };
            let callback_reached = Cell::new(false);
            let result =
                with_private_history_worker_exit(&mut worker, deadline, &cancelled, |worker| {
                    // Actual restoration validator used by
                    // from_private_package_history_selected_schema. A negative
                    // schema execution exits before head/collection/user callback.
                    validate_assessment_history_batches(
                        "tos.claim.test",
                        None,
                        &[cmd::object(vec![])],
                        2,
                        schema,
                        worker,
                        None,
                        deadline,
                        &cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    callback_reached.set(true);
                    Ok(())
                });
            assert!(matches!(
                result,
                Err(DurableError::Source(SourceCommandError::Invalid(
                    "assessment batch schema"
                )))
            ));
            assert_eq!(worker.checks, 1);
            assert_eq!(worker.exits, 1);
            assert!(!callback_reached.get());
        }
    }

    #[test]
    fn callback_refusal_attempts_exit_and_failed_exit_cannot_release_success() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut worker = RefusingWorker {
            schema: Digest256::of_bytes(b"{}"),
            checks: 0,
            exits: 0,
            exit_fails: true,
        };
        let original = SourceCommandError::Denied("original callback refusal");
        let result: DurableResult<()> =
            with_private_history_worker_exit(&mut worker, deadline, &cancelled, |_| {
                Err(DurableError::Source(original.clone()))
            });
        match result {
            Err(DurableError::Source(actual)) => assert_eq!(actual, original),
            _ => panic!("worker exit must not replace the original body error"),
        }
        assert_eq!(worker.exits, 1);
        let success =
            with_private_history_worker_exit(&mut worker, deadline, &cancelled, |_| Ok(7));
        assert!(matches!(
            success,
            Err(DurableError::Source(SourceCommandError::Denied(
                "Sign schema operation expired"
            )))
        ));
        assert_eq!(worker.exits, 2);
    }
}
