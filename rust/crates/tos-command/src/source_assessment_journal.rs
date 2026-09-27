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

/// Selected only from the actual protected assessment configuration. No public
/// constructor accepts a journal, ready flag, submission list or head claim.
pub(crate) struct ProtectedAssessmentJournal {
    configuration_path: PathBuf,
    configuration_raw: Vec<u8>,
    configuration: JsonValue,
    directory_path: PathBuf,
    directory: File,
    identity: (u64, u64),
    uid: u32,
}
impl ProtectedAssessmentJournal {
    pub(crate) fn select(
        configuration_path: &Path,
        source_root: &Path,
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
        let native = match cmd::text(&configuration, "schema_version")? {
            "tos_local_assessment_owner_v2" => false,
            "tos_local_assessment_owner_v3" => true,
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
            "source_root",
            "source_records",
        ];
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
            || Path::new(cmd::text(&configuration, "source_root")?) != source_root
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
        Ok(Self {
            configuration_path: configuration_path.to_owned(),
            configuration_raw,
            configuration,
            directory_path,
            directory,
            identity,
            uid,
        })
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
        protected(&file, self.uid, false)?;
        if raw(&mut file, 8_388_608, deadline, cancelled)? != self.configuration_raw {
            return Err(SourceCommandError::Conflict(
                "assessment protected configuration changed",
            ));
        }
        protected_configuration_parents(&self.directory_path, self.uid)?;
        let directory = tos_fd_open::open_absolute_directory(&self.directory_path)
            .map_err(|_| SourceCommandError::Conflict("assessment journal replaced"))?;
        protected(&directory, self.uid, true)?;
        if inode(
            &directory
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment directory identity"))?,
        ) != self.identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment journal identity changed",
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
            protected(&directory, self.uid, true)?;
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
            protected(&lock, self.uid, false)?;
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
            protected(&lock, self.uid, false)?;
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
pub(crate) struct AssessmentHistory {
    pub(crate) head: Option<String>,
    pub(crate) batches: Vec<JsonValue>,
    pub(crate) submissions: Vec<tos_validation::assessment::AssessmentSubmissionInput>,
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
            protected(&directory, self.owner.uid, true)?;
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
            protected(&current, self.owner.uid, false)?;
            protected(&held._lock, self.owner.uid, false)?;
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
        self.verify_current(deadline, cancelled)?;
        let held = self.held.get(subject).ok_or(SourceCommandError::Denied(
            "assessment subject outside held scope",
        ))?;
        let current = tos_fd_open::open_directory_at(&self.owner.directory, Path::new(&held.name))
            .map_err(|_| SourceCommandError::Conflict("assessment locked home changed"))?;
        protected(&current, self.owner.uid, true)?;
        if inode(
            &current
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("assessment home identity"))?,
        ) != held.identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment locked home replaced",
            ));
        }
        let head_file = match tos_fd_open::open_regular_at(&held.directory, Path::new("head")) {
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
        let head = if let Some(mut file) = head_file {
            protected(&file, self.owner.uid, false)?;
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
            Some(text.to_owned())
        } else {
            None
        };
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
        let head = self.head(subject, deadline, cancelled)?;
        let held = self.held.get(subject).ok_or(SourceCommandError::Denied(
            "assessment subject outside held scope",
        ))?;
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
            let mut file = tos_fd_open::open_regular_at(
                &held.directory,
                Path::new(&format!("{revision}.json")),
            )
            .map_err(|_| SourceCommandError::Invalid("assessment immutable batch absent/unsafe"))?;
            protected(&file, self.owner.uid, false)?;
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
        self.verify_current(deadline, cancelled)?;
        Ok(AssessmentHistory {
            head,
            batches: chain,
            submissions,
        })
    }
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
