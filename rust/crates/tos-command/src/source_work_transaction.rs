//! The selected-metadata transport used by the Work→Expression owner.
//!
//! This is private byte movement under the existing corpus writer lock.  A
//! publication control is a cooperating-reader epoch, not source admission or
//! protection from an independently mutating same-UID writer.

use super::{CreationFilesystem, active, inode, owned, raw, stamp, walk};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath};

const HOME: &str = "ToS/source-witnesses";
const CONTROL: &str = ".metadata-publication.json";
const STATE_SCHEMA: &str = "tos_source_metadata_publication_v1";
const MAX_STATE: usize = 8192;
const MAX_GENERATION: u64 = 9_007_199_254_740_991;
const MAX_FILES: usize = 64;
const MAX_DIRECTORIES: usize = 64;
const MAX_SIDE: usize = 8 * 1024 * 1024;
const MAX_AUTHORIZATION: usize = 64 * 1024;
const MAX_MANIFEST: usize = 512 * 1024;
const TRANSACTIONS: &str = ".metadata-transactions";
const MANIFEST_SCHEMA: &str = "tos_selected_metadata_transaction_v1";
const PROFILED_MANIFEST_SCHEMA: &str = "tos_selected_metadata_transaction_v2";
const CANONICAL_FORM_MANIFEST_SCHEMA: &str = "tos_selected_metadata_transaction_v3";
const COMPLETION_SCHEMA: &str = "tos_selected_metadata_completion_v1";

#[derive(Clone)]
pub(crate) struct PublicationSnapshot {
    state: Option<JsonValue>,
    pub(crate) token: Option<String>,
    pub(crate) generation: u64,
}

fn digest(value: &JsonValue) -> SourceCommandResult<String> {
    Ok(Digest256::of_bytes(&cmd::canonical(value)?).to_prefixed())
}
fn hash(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
pub(crate) fn is_hash(value: &str) -> bool {
    hash(value)
}
pub(crate) fn state(value: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(
        value,
        &[
            "schema_version",
            "generation",
            "transition_id",
            "phase",
            "transaction_id",
            "manifest_sha256",
            "outcome",
            "recovery_authorization",
            "token",
        ],
    )?;
    let generation = cmd::integer(value, "generation")?;
    let transition = cmd::text(value, "transition_id")?;
    let phase = cmd::text(value, "phase")?;
    let token = cmd::text(value, "token")?;
    if cmd::text(value, "schema_version")? != STATE_SCHEMA
        || !(1..=MAX_GENERATION).contains(&generation)
        || transition.len() != 32
        || !transition
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || !matches!(phase, "pending" | "ready")
        || !hash(cmd::text(value, "transaction_id")?)
        || !hash(cmd::text(value, "manifest_sha256")?)
        || !hash(token)
    {
        return Err(SourceCommandError::Invalid(
            "selected metadata publication control",
        ));
    }
    let outcome = cmd::field(value, "outcome")?;
    let renewal = cmd::field(value, "recovery_authorization")?;
    if phase == "pending" {
        if outcome != &JsonValue::Null || renewal != &JsonValue::Null {
            return Err(SourceCommandError::Invalid(
                "pending publication terminal fields",
            ));
        }
    } else if !matches!(outcome.as_str(), Some("committed" | "rolled-back")) {
        return Err(SourceCommandError::Invalid("terminal publication outcome"));
    }
    if renewal != &JsonValue::Null
        && (renewal.as_object().is_none() || cmd::canonical(renewal)?.len() > 4096)
    {
        return Err(SourceCommandError::Invalid(
            "publication recovery authorization",
        ));
    }
    let without_token = JsonValue::Object(
        value
            .as_object()
            .ok_or(SourceCommandError::Invalid("publication object"))?
            .iter()
            .filter(|(key, _)| key.as_str() != Some("token"))
            .cloned()
            .collect(),
    );
    if digest(&without_token)? != token {
        return Err(SourceCommandError::Invalid("publication token digest"));
    }
    Ok(())
}

fn read_state_at(
    root: &File,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<JsonValue>> {
    active(deadline, cancelled)?;
    let witness = walk(root, HOME, uid)?;
    let mut descriptor: File = match rustix::fs::openat(
        &witness,
        CONTROL,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd.into(),
        Err(Errno::NOENT) => return Ok(None),
        Err(_) => return Err(SourceCommandError::Denied("publication control unsafe")),
    };
    let initial = owned(&descriptor, uid, false)?;
    if initial.mode() & 0o7000 != 0 || !matches!(initial.mode() & 0o777, 0o600 | 0o644) {
        return Err(SourceCommandError::Denied("publication control file mode"));
    }
    if initial.len() > MAX_STATE as u64 {
        return Err(SourceCommandError::Invalid(
            "publication control byte budget",
        ));
    }
    let bytes = raw(&mut descriptor, MAX_STATE, deadline, cancelled)?;
    let again: File = rustix::fs::openat(
        &witness,
        CONTROL,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Conflict("publication control changed during read"))?;
    let current = owned(&again, uid, false)?;
    if stamp(&initial) != stamp(&current) || inode(&initial) != inode(&current) {
        return Err(SourceCommandError::Conflict(
            "publication control replaced during read",
        ));
    }
    let value = cmd::parse(&bytes)?;
    state(&value)?;
    Ok(Some(value))
}

fn read_state(
    fs: &CreationFilesystem,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<JsonValue>> {
    read_state_at(&fs.root, fs.uid, deadline, cancelled)
}

impl PublicationSnapshot {
    pub(crate) fn member_binding(&self) -> SourceCommandResult<Option<(Digest256, u64)>> {
        self.state
            .as_ref()
            .map(|state| {
                let raw = encoded(state)?;
                Ok((Digest256::of_bytes(&raw), raw.len() as u64))
            })
            .transpose()
    }
    /// Select once before the first live Work/catalog/dependency read.  A
    /// selected pending head is for explicit owner recovery only.
    pub(crate) fn select(
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        Self::select_at(&fs.root, fs.uid, deadline, cancelled)
    }

    /// Same physical selected publication control for the actual owner-local
    /// Text consumer. Its independently protected context supplies the root;
    /// the control alone never grants private reading or publication.
    pub(crate) fn select_at(
        root: &File,
        uid: u32,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let state = read_state_at(root, uid, deadline, cancelled)?;
        if state
            .as_ref()
            .is_some_and(|v| cmd::text(v, "phase").ok() != Some("ready"))
        {
            return Err(SourceCommandError::Conflict(
                "selected metadata publication pending",
            ));
        }
        let token = state
            .as_ref()
            .map(|v| cmd::text(v, "token").map(str::to_owned))
            .transpose()?;
        let generation = state
            .as_ref()
            .map(|v| cmd::integer(v, "generation"))
            .transpose()?
            .unwrap_or(0);
        Ok(Self {
            state,
            token,
            generation,
        })
    }

    pub(crate) fn verify_current(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify_at(&fs.root, fs.uid, deadline, cancelled)
    }

    pub(crate) fn verify_at(
        &self,
        root: &File,
        uid: u32,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let now = read_state_at(root, uid, deadline, cancelled)?;
        if now
            .as_ref()
            .is_some_and(|v| cmd::text(v, "phase").ok() != Some("ready"))
            || !match (&self.state, &now) {
                (None, None) => true,
                (Some(a), Some(b)) => cmd::same(a, b)?,
                _ => false,
            }
        {
            return Err(SourceCommandError::Conflict(
                "selected metadata publication changed",
            ));
        }
        Ok(())
    }
}

/// A Work owner assembles these complete before/after bytes only after its
/// typed parent, Claim, forms, source, schema and delegation checks.  No
/// externally supplied plan or serialized receipt is accepted as authority.
#[derive(Clone)]
pub(crate) struct SelectedFile {
    pub(crate) path: RelativePath,
    pub(crate) before: Option<Vec<u8>>,
    pub(crate) after: Option<Vec<u8>>,
}
#[derive(Clone)]
pub(crate) struct WorkPlan {
    pub(crate) transaction_id: String,
    pub(crate) authorization: JsonValue,
    pub(crate) item_path_profile: Option<RelativePath>,
    pub(crate) files: Vec<SelectedFile>,
    pub(crate) new_directories: Vec<RelativePath>,
}

#[derive(Clone)]
struct FrozenPlan {
    summary: JsonValue,
    files: Vec<SelectedFile>,
    directories: Vec<RelativePath>,
    blobs: BTreeMap<String, Vec<u8>>,
}
fn path(value: &str, directory: bool) -> SourceCommandResult<RelativePath> {
    profiled_path(value, directory, &BTreeSet::new(), None)
}
fn profiled_path(
    value: &str,
    directory: bool,
    companions: &BTreeSet<String>,
    canonical_form: Option<&CanonicalFormsPathProfile>,
) -> SourceCommandResult<RelativePath> {
    if value.len() > 1024 || value.contains('\\') || value.contains('\0') {
        return Err(SourceCommandError::Denied("selected metadata path grammar"));
    }
    let parsed = RelativePath::parse(value)
        .map_err(|_| SourceCommandError::Denied("selected metadata path grammar"))?;
    if !directory && canonical_form.is_some_and(|profile| value == profile.target.as_str()) {
        return Ok(parsed);
    }
    let parts: Vec<_> = value.split('/').collect();
    if !(3..=24).contains(&parts.len())
        || parts[..2] != ["ToS", "source-witnesses"]
        || parts.iter().any(|part| {
            part.starts_with('.')
                || matches!(
                    *part,
                    "payload" | "private" | "local-content" | "owner-local" | "catalog"
                )
        })
        || !directory
            && (parts.len() < 4
                || !(value.ends_with(".json")
                    || value.ends_with(".jsonl")
                    || companions.contains(value)))
    {
        return Err(SourceCommandError::Denied(
            "selected metadata public path boundary",
        ));
    }
    Ok(parsed)
}
/// A concrete canonical-form mover profile. It permits one adjacent form set,
/// never the canonical node itself. The family guard owns current source/schema,
/// delegated form IDs and protected configuration; serialized profile is not a grant.
struct CanonicalFormsPathProfile {
    source: RelativePath,
    target: RelativePath,
}
impl CanonicalFormsPathProfile {
    fn from_authorization(value: &JsonValue) -> SourceCommandResult<Option<Self>> {
        if value
            .object_get("schema_version")
            .and_then(JsonValue::as_str)
            != Some("tos_canonical_forms_authorization_v1")
        {
            return Ok(None);
        }
        let source = cmd::text(value, "source_path")?;
        let parts: Vec<_> = source.split('/').collect();
        if source.len() > 1024
            || !(5..=24).contains(&parts.len())
            || parts[..2] != ["ToS", "canon"]
            || parts.last() != Some(&"node.json")
            || parts.iter().any(|p| {
                p.is_empty()
                    || p.starts_with('.')
                    || matches!(
                        *p,
                        "payload" | "private" | "local-content" | "owner-local" | "catalog"
                    )
            })
        {
            return Err(SourceCommandError::Denied(
                "canonical form exact source path",
            ));
        }
        let source = RelativePath::parse(source)
            .map_err(|_| SourceCommandError::Denied("canonical form source path"))?;
        let parent = selected_parent(source.as_str())?;
        let target = RelativePath::parse(&format!("{parent}/node.human-forms.json"))
            .map_err(|_| SourceCommandError::Denied("canonical form target path"))?;
        Ok(Some(Self { source, target }))
    }
    fn encoded(&self) -> JsonValue {
        cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_canonical_form_metadata_paths_v1"),
            ),
            ("source_path", cmd::string(self.source.as_str())),
            ("target_path", cmd::string(self.target.as_str())),
        ])
    }
}
fn canonical_forms_profile(
    summary: &JsonValue,
) -> SourceCommandResult<Option<CanonicalFormsPathProfile>> {
    let profile =
        CanonicalFormsPathProfile::from_authorization(cmd::field(summary, "authorization")?)?;
    if let Some(profile) = &profile {
        if !cmd::same(cmd::field(summary, "path_profile")?, &profile.encoded())? {
            return Err(SourceCommandError::Conflict(
                "retained canonical form path profile differs",
            ));
        }
    }
    Ok(profile)
}
/// The maintained Item profile grants exactly two companions in one authorized
/// Item home. It never widens the ordinary metadata suffix contract.
fn item_companions(
    authorization: &JsonValue,
    item: Option<&RelativePath>,
) -> SourceCommandResult<BTreeSet<String>> {
    let Some(item) = item else {
        return Ok(BTreeSet::new());
    };
    path(item.as_str(), false)?;
    let parts: Vec<_> = item.as_str().split('/').collect();
    if parts.len() < 5
        || parts[parts.len() - 1] != "item.json"
        || parts[parts.len() - 3] != "items"
        || cmd::text(authorization, "schema_version")? != "tos_item_adoption_authorization_v1"
        || cmd::text(cmd::field(authorization, "scope")?, "item_source_path")? != item.as_str()
    {
        return Err(SourceCommandError::Denied(
            "Item metadata profile differs from adoption scope",
        ));
    }
    let home = item
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Item home"))?
        .0;
    Ok([
        format!("{home}/fixity.sha256"),
        format!("{home}/forensic-report.md"),
    ]
    .into_iter()
    .collect())
}
fn selected_profile(summary: &JsonValue) -> SourceCommandResult<Option<RelativePath>> {
    let Some(profile) = summary.object_get("path_profile") else {
        return Ok(None);
    };
    if cmd::text(profile, "schema_version")? == "tos_canonical_form_metadata_paths_v1" {
        canonical_forms_profile(summary)?.ok_or(SourceCommandError::Denied(
            "canonical form profile lacks family authorization",
        ))?;
        return Ok(None);
    }
    cmd::exact_keys(profile, &["schema_version", "item_source_path"])?;
    if cmd::text(profile, "schema_version")? != "tos_item_metadata_paths_v1" {
        return Err(SourceCommandError::Invalid("Item metadata path profile"));
    }
    let item = path(cmd::text(profile, "item_source_path")?, false)?;
    item_companions(cmd::field(summary, "authorization")?, Some(&item))?;
    Ok(Some(item))
}
fn binding(bytes: Option<&[u8]>) -> JsonValue {
    match bytes {
        None => JsonValue::Null,
        Some(raw) => cmd::object(vec![
            (
                "sha256",
                cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
            ),
            ("bytes", cmd::number(raw.len() as u64)),
        ]),
    }
}
/// Validate a fixed owner's in-memory transport plan before retaining any
/// auxiliary archive bytes. This is the existing byte/path law, not admission.
pub(crate) fn validate_plan(plan: &WorkPlan) -> SourceCommandResult<()> {
    freeze(plan.clone()).map(|_| ())
}
fn freeze(plan: WorkPlan) -> SourceCommandResult<FrozenPlan> {
    if !hash(&plan.transaction_id)
        || plan.files.is_empty()
        || plan.files.len() > MAX_FILES
        || plan.new_directories.len() > MAX_DIRECTORIES
        || plan.authorization.as_object().is_none()
        || cmd::canonical(&plan.authorization)?.len() > MAX_AUTHORIZATION
    {
        return Err(SourceCommandError::Invalid(
            "selected metadata plan budget/authorization",
        ));
    }
    let companions = item_companions(&plan.authorization, plan.item_path_profile.as_ref())?;
    let canonical_form = CanonicalFormsPathProfile::from_authorization(&plan.authorization)?;
    if let Some(profile) = &canonical_form {
        if plan.item_path_profile.is_some()
            || !plan.new_directories.is_empty()
            || plan.files.len() != 1
            || plan.files[0].path != profile.target
        {
            return Err(SourceCommandError::Denied(
                "canonical form mover selects only adjacent form set",
            ));
        }
    }
    let mut files = plan.files;
    files.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    let mut dirs = plan.new_directories;
    dirs.sort_by(|a, b| {
        let a = a.as_str();
        let b = b.as_str();
        (a.split('/').count(), a).cmp(&(b.split('/').count(), b))
    });
    let mut seen = BTreeSet::new();
    let mut before_bytes = 0usize;
    let mut after_bytes = 0usize;
    let mut changed = false;
    let mut blobs = BTreeMap::new();
    let mut summaries = Vec::with_capacity(files.len());
    for item in &files {
        let name = item.path.as_str();
        profiled_path(name, false, &companions, canonical_form.as_ref())?;
        if !seen.insert(name.to_owned()) || item.before.is_none() && item.after.is_none() {
            return Err(SourceCommandError::Invalid(
                "duplicate/empty selected metadata member",
            ));
        }
        if item.before != item.after {
            changed = true;
        }
        for (raw, total) in [
            (&item.before, &mut before_bytes),
            (&item.after, &mut after_bytes),
        ] {
            if let Some(raw) = raw {
                *total = total
                    .checked_add(raw.len())
                    .ok_or(SourceCommandError::Invalid("selected side overflow"))?;
                if raw.len() > MAX_SIDE || *total > MAX_SIDE {
                    return Err(SourceCommandError::Invalid("selected side byte budget"));
                }
                blobs
                    .entry(Digest256::of_bytes(raw).to_prefixed())
                    .or_insert_with(|| raw.clone());
            }
        }
        summaries.push(cmd::object(vec![
            ("path", cmd::string(name)),
            ("before", binding(item.before.as_deref())),
            ("after", binding(item.after.as_deref())),
        ]));
    }
    if !changed {
        return Err(SourceCommandError::Invalid(
            "selected metadata plan has no change",
        ));
    }
    if files.iter().enumerate().any(|(i, left)| {
        files.iter().skip(i + 1).any(|right| {
            right
                .path
                .as_str()
                .starts_with(&format!("{}/", left.path.as_str()))
        })
    }) {
        return Err(SourceCommandError::Invalid(
            "selected file ancestor collision",
        ));
    }
    let mut new_seen = BTreeSet::new();
    for dir in &dirs {
        let name = dir.as_str();
        path(name, true)?;
        if !new_seen.insert(name.to_owned())
            || !files.iter().any(|file| {
                file.before.is_none() && file.path.as_str().starts_with(&format!("{name}/"))
            })
        {
            return Err(SourceCommandError::Invalid(
                "new selected directory closure",
            ));
        }
    }
    for file in &files {
        if dirs.iter().any(|dir| {
            dir.as_str()
                .starts_with(&format!("{}/", file.path.as_str()))
        }) {
            return Err(SourceCommandError::Invalid(
                "selected file/dir ancestor collision",
            ));
        }
    }
    for (i, left) in dirs.iter().enumerate() {
        if dirs.iter().skip(i + 1).any(|right| right == left) {
            return Err(SourceCommandError::Invalid(
                "duplicate selected new directory",
            ));
        }
    }
    let mut summary_members = vec![
        ("authorization", plan.authorization),
        (
            "new_directories",
            JsonValue::Array(dirs.iter().map(|d| cmd::string(d.as_str())).collect()),
        ),
        ("files", JsonValue::Array(summaries)),
    ];
    if let Some(item) = &plan.item_path_profile {
        summary_members.push((
            "path_profile",
            cmd::object(vec![
                ("schema_version", cmd::string("tos_item_metadata_paths_v1")),
                ("item_source_path", cmd::string(item.as_str())),
            ]),
        ));
    }
    if let Some(profile) = canonical_form {
        summary_members.push(("path_profile", profile.encoded()));
    }
    let summary = cmd::object(summary_members);
    Ok(FrozenPlan {
        summary,
        files,
        directories: dirs,
        blobs,
    })
}

fn owned_object(fields: impl IntoIterator<Item = (String, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(name, value)| (JsonString::from_utf8(&name), value))
            .collect(),
    )
}
fn binding_of(bytes: Option<&[u8]>) -> Option<(String, usize)> {
    bytes.map(|raw| (Digest256::of_bytes(raw).to_prefixed(), raw.len()))
}
fn selected_parent(path: &str) -> SourceCommandResult<&str> {
    path.rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(SourceCommandError::Invalid("selected metadata parent path"))
}
fn parent_refs(plan: &FrozenPlan) -> SourceCommandResult<Vec<String>> {
    let mut result = BTreeSet::new();
    result.insert(HOME.to_owned());
    let canonical = canonical_forms_profile(&plan.summary)?.is_some();
    let boundary = if canonical { "ToS/canon" } else { HOME };
    for path in plan
        .files
        .iter()
        .map(|f| f.path.as_str())
        .chain(plan.directories.iter().map(RelativePath::as_str))
    {
        let mut parent = selected_parent(path)?;
        loop {
            if parent != boundary && !parent.starts_with(&format!("{boundary}/")) {
                return Err(SourceCommandError::Denied(
                    "selected metadata parent outside source home",
                ));
            }
            result.insert(parent.to_owned());
            if parent == boundary {
                break;
            }
            parent = selected_parent(parent)?;
        }
    }
    let mut result: Vec<_> = result.into_iter().collect();
    result.sort_by(|a, b| {
        (a.split('/').count(), a.as_str()).cmp(&(b.split('/').count(), b.as_str()))
    });
    Ok(result)
}
fn dir_binding(file: &File, uid: u32) -> SourceCommandResult<JsonValue> {
    let m = owned(file, uid, true)?;
    Ok(cmd::object(vec![
        ("device", cmd::number(m.dev())),
        ("inode", cmd::number(m.ino())),
        ("mode", cmd::number(u64::from(m.mode()))),
        ("uid", cmd::number(u64::from(m.uid()))),
    ]))
}
pub(crate) fn read_existing_parent(
    fs: &CreationFilesystem,
    path: &str,
) -> SourceCommandResult<Option<File>> {
    let mut current = tos_fd_open::reopen_directory(&fs.root)
        .map_err(|_| SourceCommandError::Denied("selected metadata root descriptor"))?;
    for part in path.split('/') {
        let next = match rustix::fs::openat(
            &current,
            part,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => File::from(fd),
            Err(Errno::NOENT) => return Ok(None),
            Err(_) => {
                return Err(SourceCommandError::Denied(
                    "selected metadata parent unsafe",
                ));
            }
        };
        owned(&next, fs.uid, true)?;
        current = next;
    }
    Ok(Some(current))
}
fn capture_parents(fs: &CreationFilesystem, plan: &FrozenPlan) -> SourceCommandResult<JsonValue> {
    let new: BTreeSet<_> = plan.directories.iter().map(RelativePath::as_str).collect();
    let mut result = Vec::new();
    for reference in parent_refs(plan)? {
        let observed = read_existing_parent(fs, &reference)?;
        if observed.is_none() && !new.contains(reference.as_str()) {
            return Err(SourceCommandError::Conflict(
                "undeclared selected metadata parent absent",
            ));
        }
        if observed.is_some() && new.contains(reference.as_str()) {
            return Err(SourceCommandError::Conflict(
                "new selected metadata directory occupied",
            ));
        }
        let binding = observed
            .as_ref()
            .map(|fd| dir_binding(fd, fs.uid))
            .transpose()?
            .unwrap_or(JsonValue::Null);
        result.push((reference, binding));
    }
    for reference in &plan.directories {
        if read_existing_parent(fs, reference.as_str())?.is_some() {
            return Err(SourceCommandError::Conflict(
                "new selected metadata directory occupied",
            ));
        }
    }
    Ok(owned_object(result))
}
fn manifest(
    id: &str,
    snapshot: &PublicationSnapshot,
    plan: &FrozenPlan,
    parents: JsonValue,
) -> JsonValue {
    cmd::object(vec![
        (
            "schema_version",
            cmd::string(
                if plan
                    .summary
                    .object_get("path_profile")
                    .and_then(|p| p.object_get("schema_version"))
                    .and_then(JsonValue::as_str)
                    == Some("tos_canonical_form_metadata_paths_v1")
                {
                    CANONICAL_FORM_MANIFEST_SCHEMA
                } else if plan.summary.object_get("path_profile").is_some() {
                    PROFILED_MANIFEST_SCHEMA
                } else {
                    MANIFEST_SCHEMA
                },
            ),
        ),
        ("transaction_id", cmd::string(id)),
        (
            "base_publication",
            cmd::object(vec![
                (
                    "token",
                    snapshot
                        .token
                        .as_ref()
                        .map(|s| cmd::string(s))
                        .unwrap_or(JsonValue::Null),
                ),
                ("generation", cmd::number(snapshot.generation)),
            ]),
        ),
        ("plan", plan.summary.clone()),
        ("parents", parents),
    ])
}
pub(crate) fn read_at(
    parent: &File,
    name: &str,
    uid: u32,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    Ok(read_at_mode(parent, name, uid, limit, deadline, cancelled)?.map(|(raw, _)| raw))
}
pub(crate) fn read_at_mode(
    parent: &File,
    name: &str,
    uid: u32,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<(Vec<u8>, u32)>> {
    let mut file: File = match rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd.into(),
        Err(Errno::NOENT) => return Ok(None),
        Err(_) => return Err(SourceCommandError::Denied("selected metadata file unsafe")),
    };
    let before = owned(&file, uid, false)?;
    if before.mode() & 0o7000 != 0 {
        return Err(SourceCommandError::Denied(
            "selected metadata special file mode",
        ));
    }
    if before.len() > limit as u64 {
        return Err(SourceCommandError::Invalid(
            "selected metadata file byte budget",
        ));
    }
    let bytes = raw(&mut file, limit, deadline, cancelled)?;
    let again: File = rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Conflict("selected metadata file replaced"))?;
    let current = owned(&again, uid, false)?;
    if stamp(&before) != stamp(&current) {
        return Err(SourceCommandError::Conflict(
            "selected metadata file changed",
        ));
    }
    Ok(Some((bytes, before.mode() & 0o7777)))
}
fn entropy_hex(bytes: usize) -> SourceCommandResult<String> {
    let mut random = vec![0u8; bytes];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut random))
        .map_err(|_| SourceCommandError::Invalid("selected metadata entropy"))?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}
pub(crate) fn atomic_write(
    parent: &File,
    name: &str,
    bytes: &[u8],
    no_replace: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let temporary = format!(".metadata-{}.pending", entropy_hex(16)?);
    let mut fd: File = rustix::fs::openat(
        parent,
        temporary.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Conflict("selected metadata temporary occupied"))?;
    let attempt = (|| {
        for block in bytes.chunks(65536) {
            active(deadline, cancelled)?;
            fd.write_all(block)
                .map_err(|_| SourceCommandError::Invalid("selected metadata write"))?;
        }
        fd.sync_all()
            .map_err(|_| SourceCommandError::Invalid("selected metadata file fsync"))?;
        let flags = if no_replace {
            RenameFlags::NOREPLACE
        } else {
            RenameFlags::empty()
        };
        rustix::fs::renameat_with(parent, temporary.as_str(), parent, name, flags).map_err(
            |err| {
                if err == Errno::EXIST {
                    SourceCommandError::Conflict("absent selected metadata file occupied")
                } else {
                    SourceCommandError::Invalid("selected metadata atomic rename")
                }
            },
        )?;
        parent
            .sync_all()
            .map_err(|_| SourceCommandError::Invalid("selected metadata parent fsync"))
    })();
    if attempt.is_err() {
        let _ = rustix::fs::unlinkat(parent, temporary.as_str(), AtFlags::empty());
    }
    attempt
}
fn immutable(
    parent: &File,
    name: &str,
    bytes: &[u8],
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    match read_at(
        parent,
        name,
        uid,
        bytes.len().max(8192),
        deadline,
        cancelled,
    )? {
        Some(existing) if existing == bytes => Ok(()),
        Some(_) => Err(SourceCommandError::Conflict(
            "retained transaction member differs",
        )),
        None => atomic_write(parent, name, bytes, true, deadline, cancelled),
    }
}

/// Preserve the exact selected Work predecessor before any publication-control
/// pending state. This is the existing v2 record archive format; it is not a
/// record.revise grant and it never makes the archived content current.
pub(crate) struct WorkArchive {
    pub(crate) path: String,
    members: BTreeMap<String, (Digest256, usize)>,
}
impl WorkArchive {
    pub(crate) fn member_paths(&self) -> impl Iterator<Item = String> + '_ {
        self.members
            .keys()
            .map(|name| format!("{}/{name}", self.path))
    }

    pub(crate) fn verify_current(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let directory = walk(&fs.root, &self.path, fs.uid)?;
        let before = owned(&directory, fs.uid, true)?;
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Invalid("Work archive enumeration"))?;
        let mut names = BTreeSet::new();
        for entry in entries {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| SourceCommandError::Invalid("Work archive entry"))?
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Denied("Work archive non-UTF8 entry"))?;
            if names.len() >= self.members.len() || !names.insert(name) {
                return Err(SourceCommandError::Conflict("Work archive extra member"));
            }
        }
        if names != self.members.keys().cloned().collect() {
            return Err(SourceCommandError::Conflict(
                "Work archive member set changed",
            ));
        }
        for (name, (sha, size)) in &self.members {
            let (raw, mode) = read_at_mode(&directory, name, fs.uid, *size, deadline, cancelled)?
                .ok_or(SourceCommandError::Conflict(
                "Work archive member disappeared",
            ))?;
            if !matches!(mode, 0o600 | 0o644)
                || raw.len() != *size
                || Digest256::of_bytes(&raw) != *sha
            {
                return Err(SourceCommandError::Conflict(
                    "Work archive member mode/bytes changed",
                ));
            }
        }
        let current = walk(&fs.root, &self.path, fs.uid)?;
        if inode(&before) != inode(&owned(&current, fs.uid, true)?)
            || stamp(&before) != stamp(&owned(&directory, fs.uid, true)?)
        {
            return Err(SourceCommandError::Conflict(
                "Work archive directory changed",
            ));
        }
        Ok(())
    }
}

pub(crate) fn work_archive(
    fs: &CreationFilesystem,
    work_path: &str,
    work: &JsonValue,
    before: &BTreeMap<String, Vec<u8>>,
    expected_revision: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    create: bool,
) -> SourceCommandResult<WorkArchive> {
    compound_archive(
        fs,
        work_path,
        &cmd::reference(work, "record_id", "record_version")?,
        before,
        expected_revision,
        deadline,
        cancelled,
        create,
        "work.json",
        true,
    )
}
/// Expression-owned compounds retain the exact three-file parent revision.
/// Responsibility and Edition owners still authenticate their own selected path.
pub(crate) fn expression_archive(
    fs: &CreationFilesystem,
    expression_path: &str,
    expression: &JsonValue,
    before: &BTreeMap<String, Vec<u8>>,
    expected_revision: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    create: bool,
) -> SourceCommandResult<WorkArchive> {
    if cmd::text(expression, "record_type")? != "expression"
        || !expression_path.ends_with("/expression.json")
    {
        return Err(SourceCommandError::Denied(
            "Expression archive selected parent profile",
        ));
    }
    compound_archive(
        fs,
        expression_path,
        &cmd::reference(expression, "record_id", "record_version")?,
        before,
        expected_revision,
        deadline,
        cancelled,
        create,
        "expression.json",
        true,
    )
}
pub(crate) fn item_archive(
    fs: &CreationFilesystem,
    edition_path: &str,
    edition: &JsonValue,
    before: &BTreeMap<String, Vec<u8>>,
    expected_revision: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    create: bool,
) -> SourceCommandResult<WorkArchive> {
    if cmd::text(edition, "record_type")? != "edition" || !edition_path.ends_with("/edition.json") {
        return Err(SourceCommandError::Denied(
            "Item archive selected Edition profile",
        ));
    }
    compound_archive(
        fs,
        edition_path,
        &cmd::reference(edition, "record_id", "record_version")?,
        before,
        expected_revision,
        deadline,
        cancelled,
        create,
        "edition.json",
        true,
    )
}
/// Record revision archives use the exact validated owner family protocol.
/// Existing compound owners continue to select the version-two protocol.
pub(crate) fn record_revision_archive(
    fs: &CreationFilesystem,
    ctx: &crate::source_command::CommandContext,
    record: &JsonValue,
    before: &BTreeMap<String, Vec<u8>>,
    expected_revision: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    create: bool,
) -> SourceCommandResult<WorkArchive> {
    let (config, family) = crate::source_revisions::configuration(ctx)?;
    let source_path = cmd::text(&config, "source_path")?;
    let subject = crate::source_forms::metadata_subject(record)?;
    if cmd::text(&subject, "id")? != cmd::text(&config, "record_id")? {
        return Err(SourceCommandError::Denied(
            "record revision archive selected identity",
        ));
    }
    let names = crate::source_revisions::names(source_path)?;
    let selected_record = cmd::parse(before.get(&names[0]).ok_or(
        SourceCommandError::Conflict("record revision archive source absent"),
    )?)?;
    if !cmd::same(&selected_record, record)?
        || crate::source_revisions::package(ctx, source_path, family.selected(), None)? != *before
    {
        return Err(SourceCommandError::Denied(
            "record revision archive selected package",
        ));
    }
    compound_archive(
        fs,
        source_path,
        &subject,
        before,
        expected_revision,
        deadline,
        cancelled,
        create,
        &names[0],
        family.selected(),
    )
}
/// Archive storage for the exact Collection predecessor; this supplies no
/// record.revise grant and cannot select a different record profile.
pub(crate) fn collection_archive(
    fs: &CreationFilesystem,
    collection_path: &str,
    collection: &JsonValue,
    before: &BTreeMap<String, Vec<u8>>,
    expected_revision: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    create: bool,
) -> SourceCommandResult<WorkArchive> {
    if cmd::text(collection, "record_type")? != "collection"
        || !collection_path.starts_with("ToS/source-witnesses/collections/")
        || !collection_path.ends_with("/collection.json")
    {
        return Err(SourceCommandError::Denied(
            "Collection archive selected profile",
        ));
    }
    compound_archive(
        fs,
        collection_path,
        &cmd::reference(collection, "record_id", "record_version")?,
        before,
        expected_revision,
        deadline,
        cancelled,
        create,
        "collection.json",
        true,
    )
}
fn compound_archive(
    fs: &CreationFilesystem,
    work_path: &str,
    subject: &JsonValue,
    before: &BTreeMap<String, Vec<u8>>,
    expected_revision: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    create: bool,
    basename: &str,
    selected_protocol: bool,
) -> SourceCommandResult<WorkArchive> {
    let revision = crate::source_revisions::revision(before)?;
    if revision != expected_revision
        || before.is_empty()
        || before.len() > if selected_protocol { 3 } else { 64 }
        || before.get(basename).is_none()
        || before.values().any(|raw| raw.len() > 2_097_152)
        || before
            .values()
            .try_fold(0usize, |n, raw| n.checked_add(raw.len()))
            .is_none_or(|n| n > MAX_SIDE)
    {
        return Err(SourceCommandError::Conflict(
            "selected Work archive predecessor differs or exceeds its budget",
        ));
    }
    let id = cmd::text(subject, "id")?;
    let config = cmd::object(vec![("record_id", cmd::string(id))]);
    let location = crate::source_revisions::archive_path(&config, &revision)?;
    let leaf = location
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Work archive path"))?
        .1;
    let mut expected = BTreeMap::new();
    for raw in before.values() {
        expected.insert(
            format!("{}.blob", Digest256::of_bytes(raw).to_hex()),
            raw.as_slice(),
        );
    }
    let mut manifest = cmd::object(vec![
        (
            "schema_version",
            cmd::string(if selected_protocol {
                "tos_source_package_archive_v2"
            } else {
                "tos_source_package_archive_v1"
            }),
        ),
        ("source_path", cmd::string(work_path)),
        ("source", subject.clone()),
        ("revision", cmd::string(&revision)),
        ("files", crate::source_revisions::file_refs(before, true)),
    ]);
    if selected_protocol {
        cmd::set(
            &mut manifest,
            "publication_protocol",
            cmd::string("tos_selected_source_metadata_v1"),
        )?;
    }
    let manifest_raw = cmd::published(&manifest)?;
    expected.insert("manifest.json".to_owned(), manifest_raw.as_slice());
    let mut observation = WorkArchive {
        path: location.clone(),
        members: expected
            .iter()
            .map(|(name, raw)| (name.clone(), (Digest256::of_bytes(raw), raw.len())))
            .collect(),
    };
    if !create {
        // Retained writers share the exact archive object contract, while their
        // published JSON member order can differ. Authenticate that object,
        // then retain the actual bytes for every subsequent currentness check.
        let directory = walk(&fs.root, &location, fs.uid)?;
        let (raw, mode) = read_at_mode(
            &directory,
            "manifest.json",
            fs.uid,
            8192,
            deadline,
            cancelled,
        )?
        .ok_or(SourceCommandError::Conflict(
            "Work archive manifest disappeared",
        ))?;
        if !matches!(mode, 0o600 | 0o644)
            || cmd::canonical(&cmd::parse(&raw)?)? != cmd::canonical(&manifest)?
        {
            return Err(SourceCommandError::Conflict(
                "Work archive manifest differs",
            ));
        }
        observation.members.insert(
            "manifest.json".to_owned(),
            (Digest256::of_bytes(&raw), raw.len()),
        );
        observation.verify_current(fs, deadline, cancelled)?;
        return Ok(observation);
    }
    let witness = walk(&fs.root, HOME, fs.uid)?;
    match rustix::fs::mkdirat(&witness, ".record-revisions", Mode::from_raw_mode(0o700)) {
        Ok(()) => witness
            .sync_all()
            .map_err(|_| SourceCommandError::Invalid("Work archive home fsync"))?,
        Err(Errno::EXIST) => (),
        Err(_) => return Err(SourceCommandError::Denied("Work archive home unsafe")),
    }
    let home = walk(&fs.root, "ToS/source-witnesses/.record-revisions", fs.uid)?;
    let verify = |directory: &File| -> SourceCommandResult<()> {
        owned(directory, fs.uid, true)?;
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Invalid("Work archive member enumeration"))?;
        let mut names = BTreeSet::new();
        for entry in entries {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| SourceCommandError::Invalid("Work archive entry"))?
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Denied("Work archive non-UTF8 entry"))?;
            if names.len() >= expected.len() || !names.insert(name) {
                return Err(SourceCommandError::Conflict(
                    "Work archive has an unbound or duplicate member",
                ));
            }
        }
        if names != expected.keys().cloned().collect() {
            return Err(SourceCommandError::Conflict(
                "Work archive member set differs",
            ));
        }
        for (name, raw) in &expected {
            if read_at(directory, name, fs.uid, raw.len(), deadline, cancelled)?.as_deref()
                != Some(*raw)
            {
                return Err(SourceCommandError::Conflict(
                    "Work archive byte binding differs",
                ));
            }
        }
        Ok(())
    };
    match rustix::fs::openat(
        &home,
        leaf,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(existing) => {
            verify(&File::from(existing))?;
            return Ok(observation);
        }
        Err(Errno::NOENT) => (),
        Err(_) => return Err(SourceCommandError::Denied("Work archive target unsafe")),
    }
    let staging_name = format!(".source-archive-{}", entropy_hex(16)?);
    rustix::fs::mkdirat(&home, staging_name.as_str(), Mode::from_raw_mode(0o700))
        .map_err(|_| SourceCommandError::Conflict("Work archive staging occupied"))?;
    home.sync_all()
        .map_err(|_| SourceCommandError::Invalid("Work archive staging parent fsync"))?;
    let staged: File = rustix::fs::openat(
        &home,
        staging_name.as_str(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Denied("Work archive staging unsafe"))?;
    owned(&staged, fs.uid, true)?;
    for (name, raw) in &expected {
        atomic_write(&staged, name, raw, true, deadline, cancelled)?;
    }
    staged
        .sync_all()
        .map_err(|_| SourceCommandError::Invalid("Work archive staging fsync"))?;
    rustix::fs::renameat_with(
        &home,
        staging_name.as_str(),
        &home,
        leaf,
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| SourceCommandError::Conflict("Work archive target occupied"))?;
    home.sync_all()
        .map_err(|_| SourceCommandError::Invalid("Work archive parent fsync"))?;
    let directory: File = rustix::fs::openat(
        &home,
        leaf,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| SourceCommandError::Conflict("Work archive detached after publication"))?;
    verify(&directory)?;
    Ok(observation)
}

struct Parents<'a> {
    fs: &'a CreationFilesystem,
    expected: JsonValue,
    new: BTreeSet<String>,
    opened: BTreeMap<String, File>,
}

pub(crate) struct WorkCorpusFence<'a> {
    fs: &'a CreationFilesystem,
    witness: File,
    lock: File,
}
pub(crate) struct WorkGuard<'a> {
    pub(crate) full_membership: bool,
    pub(crate) journal_members: &'a BTreeSet<String>,
    pub(crate) pending_state: Option<&'a JsonValue>,
    pub(crate) prior_completion_ready: bool,
}
impl<'a> WorkCorpusFence<'a> {
    /// Read-only observers require a previously retained actual writer mutex;
    /// unlike hold(), this path never creates a missing lock file.
    pub(crate) fn hold_existing(
        fs: &'a CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let witness = walk(&fs.root, HOME, fs.uid)?;
        let lock = tos_fd_open::open_regular_at(&witness, Path::new(super::CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("committed corpus lock absent"))?;
        owned(&lock, fs.uid, false)?;
        loop {
            active(deadline, cancelled)?;
            match rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(Errno::AGAIN) => std::thread::sleep(std::time::Duration::from_millis(5)),
                Err(_) => {
                    return Err(SourceCommandError::Denied(
                        "committed corpus lock unsupported",
                    ));
                }
            }
        }
        let result = Self { fs, witness, lock };
        result.verify(deadline, cancelled)?;
        Ok(result)
    }
    pub(crate) fn hold(
        fs: &'a CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let witness = walk(&fs.root, HOME, fs.uid)?;
        let lock = fs.lock(&witness, deadline, cancelled)?;
        let result = Self { fs, witness, lock };
        result.verify(deadline, cancelled)?;
        Ok(result)
    }
    pub(crate) fn verify(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        let current = tos_fd_open::open_regular_at(&self.witness, Path::new(super::CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("selected metadata corpus lock detached"))?;
        if inode(&owned(&current, self.fs.uid, false)?)
            != inode(&owned(&self.lock, self.fs.uid, false)?)
        {
            return Err(SourceCommandError::Conflict(
                "selected metadata corpus lock replaced",
            ));
        }
        Ok(())
    }
}

fn encoded(value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    let mut raw = cmd::canonical(value)?;
    raw.push(b'\n');
    Ok(raw)
}
fn journal_dir(
    fs: &CreationFilesystem,
    id: &str,
    create: bool,
) -> SourceCommandResult<Option<File>> {
    if !hash(id) {
        return Err(SourceCommandError::Invalid("transaction id"));
    }
    let witness = walk(&fs.root, HOME, fs.uid)?;
    if create {
        match rustix::fs::mkdirat(&witness, TRANSACTIONS, Mode::from_raw_mode(0o700)) {
            Ok(()) => witness
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("transaction home fsync"))?,
            Err(Errno::EXIST) => (),
            Err(_) => return Err(SourceCommandError::Invalid("transaction home mkdir")),
        }
    }
    let home = match rustix::fs::openat(
        &witness,
        TRANSACTIONS,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::NOENT) if !create => return Ok(None),
        Err(_) => return Err(SourceCommandError::Denied("transaction home unsafe")),
    };
    owned(&home, fs.uid, true)?;
    let leaf = &id[7..];
    if create {
        match rustix::fs::mkdirat(&home, leaf, Mode::from_raw_mode(0o700)) {
            Ok(()) => home
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("transaction directory fsync"))?,
            Err(Errno::EXIST) => (),
            Err(_) => return Err(SourceCommandError::Invalid("transaction directory mkdir")),
        }
    }
    let result = match rustix::fs::openat(
        &home,
        leaf,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => File::from(fd),
        Err(Errno::NOENT) if !create => return Ok(None),
        Err(_) => return Err(SourceCommandError::Denied("transaction directory unsafe")),
    };
    owned(&result, fs.uid, true)?;
    Ok(Some(result))
}
fn retain(
    fs: &CreationFilesystem,
    id: &str,
    manifest: &JsonValue,
    plan: &FrozenPlan,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let raw = encoded(manifest)?;
    if raw.len() > MAX_MANIFEST {
        return Err(SourceCommandError::Invalid(
            "transaction manifest byte budget",
        ));
    }
    let journal = journal_dir(fs, id, true)?.unwrap();
    let mut total = 0usize;
    for (digest, blob) in &plan.blobs {
        total = total
            .checked_add(blob.len())
            .ok_or(SourceCommandError::Invalid("transaction blob overflow"))?;
        if total > 2 * MAX_SIDE {
            return Err(SourceCommandError::Invalid("transaction blob budget"));
        }
        immutable(
            &journal,
            &format!("{}.blob", &digest[7..]),
            blob,
            fs.uid,
            deadline,
            cancelled,
        )?;
    }
    immutable(&journal, "manifest.json", &raw, fs.uid, deadline, cancelled)?;
    journal
        .sync_all()
        .map_err(|_| SourceCommandError::Invalid("transaction retention fsync"))?;
    Ok(Digest256::of_bytes(&raw).to_prefixed())
}
fn journal_members(id: &str, plan: &FrozenPlan) -> SourceCommandResult<BTreeSet<String>> {
    if !hash(id) {
        return Err(SourceCommandError::Invalid("transaction id"));
    }
    let home = format!("{HOME}/{TRANSACTIONS}/{}", &id[7..]);
    let mut members = BTreeSet::from([format!("{home}/manifest.json")]);
    for digest in plan.blobs.keys() {
        if !hash(digest) {
            return Err(SourceCommandError::Invalid("retained blob digest"));
        }
        members.insert(format!("{home}/{}.blob", &digest[7..]));
    }
    Ok(members)
}
fn verify_retained(
    fs: &CreationFilesystem,
    id: &str,
    manifest: &JsonValue,
    digest: &str,
    plan: &FrozenPlan,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let journal = journal_dir(fs, id, false)?
        .ok_or(SourceCommandError::Conflict("retained transaction missing"))?;
    let directory_before = owned(&journal, fs.uid, true)?;
    let expected = journal_members(id, plan)?;
    let prefix = format!("{HOME}/{TRANSACTIONS}/{}/", &id[7..]);
    let mut names = BTreeSet::new();
    let entries = std::fs::read_dir(format!("/proc/self/fd/{}", journal.as_raw_fd()))
        .map_err(|_| SourceCommandError::Invalid("retained transaction enumeration"))?;
    for entry in entries {
        active(deadline, cancelled)?;
        let name = entry
            .map_err(|_| SourceCommandError::Invalid("retained transaction entry"))?
            .file_name()
            .into_string()
            .map_err(|_| SourceCommandError::Denied("retained transaction entry name"))?;
        if names.len() >= expected.len() || !names.insert(format!("{prefix}{name}")) {
            return Err(SourceCommandError::Conflict(
                "retained transaction extra member",
            ));
        }
    }
    if names != expected {
        return Err(SourceCommandError::Conflict(
            "retained transaction member set changed",
        ));
    }
    let (raw, manifest_mode) = read_at_mode(
        &journal,
        "manifest.json",
        fs.uid,
        MAX_MANIFEST,
        deadline,
        cancelled,
    )?
    .ok_or(SourceCommandError::Conflict(
        "retained transaction manifest missing",
    ))?;
    if !matches!(manifest_mode, 0o600 | 0o644)
        || Digest256::of_bytes(&raw).to_prefixed() != digest
        || !cmd::same(&cmd::parse(&raw)?, manifest)?
    {
        return Err(SourceCommandError::Conflict(
            "retained transaction manifest changed",
        ));
    }
    for (sha, expected) in &plan.blobs {
        let (raw, mode) = read_at_mode(
            &journal,
            &format!("{}.blob", &sha[7..]),
            fs.uid,
            expected.len(),
            deadline,
            cancelled,
        )?
        .ok_or(SourceCommandError::Conflict(
            "retained transaction blob missing",
        ))?;
        if !matches!(mode, 0o600 | 0o644)
            || &raw != expected
            || Digest256::of_bytes(&raw).to_prefixed() != *sha
        {
            return Err(SourceCommandError::Conflict(
                "retained transaction blob changed",
            ));
        }
    }
    let directory_current = journal_dir(fs, id, false)?.ok_or(SourceCommandError::Conflict(
        "retained transaction detached",
    ))?;
    if inode(&directory_before) != inode(&owned(&directory_current, fs.uid, true)?)
        || stamp(&directory_before) != stamp(&owned(&journal, fs.uid, true)?)
    {
        return Err(SourceCommandError::Conflict(
            "retained transaction directory changed",
        ));
    }
    Ok(())
}
fn publication_state(
    manifest: &JsonValue,
    manifest_digest: &str,
    pending: bool,
    outcome: Option<&str>,
    recovery_authorization: Option<JsonValue>,
) -> SourceCommandResult<JsonValue> {
    let base = cmd::field(manifest, "base_publication")?;
    let generation = cmd::integer(base, "generation")?
        .checked_add(if pending { 1 } else { 2 })
        .ok_or(SourceCommandError::Invalid(
            "publication generation overflow",
        ))?;
    if generation > MAX_GENERATION {
        return Err(SourceCommandError::Invalid("publication generation budget"));
    }
    let mut state = cmd::object(vec![
        ("schema_version", cmd::string(STATE_SCHEMA)),
        ("generation", cmd::number(generation)),
        ("transition_id", cmd::string(&entropy_hex(16)?)),
        (
            "phase",
            cmd::string(if pending { "pending" } else { "ready" }),
        ),
        (
            "transaction_id",
            cmd::field(manifest, "transaction_id")?.clone(),
        ),
        ("manifest_sha256", cmd::string(manifest_digest)),
        (
            "outcome",
            outcome.map(cmd::string).unwrap_or(JsonValue::Null),
        ),
        (
            "recovery_authorization",
            recovery_authorization.unwrap_or(JsonValue::Null),
        ),
    ]);
    let token = digest(&state)?;
    cmd::set(&mut state, "token", cmd::string(&token))?;
    self::state(&state)?;
    Ok(state)
}
fn same_state(a: Option<&JsonValue>, b: Option<&JsonValue>) -> SourceCommandResult<bool> {
    Ok(match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => cmd::same(a, b)?,
        _ => false,
    })
}
fn publish_state(
    fs: &CreationFilesystem,
    value: &JsonValue,
    expected: Option<&JsonValue>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if !same_state(read_state(fs, deadline, cancelled)?.as_ref(), expected)? {
        return Err(SourceCommandError::Conflict(
            "publication control changed outside writer lock",
        ));
    }
    let witness = walk(&fs.root, HOME, fs.uid)?;
    atomic_write(
        &witness,
        CONTROL,
        &encoded(value)?,
        expected.is_none(),
        deadline,
        cancelled,
    )
}
fn record_completion(
    fs: &CreationFilesystem,
    terminal: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let id = cmd::text(terminal, "transaction_id")?;
    let journal = journal_dir(fs, id, false)?
        .ok_or(SourceCommandError::Conflict("terminal journal missing"))?;
    let bytes = encoded(&cmd::object(vec![
        ("schema_version", cmd::string(COMPLETION_SCHEMA)),
        ("publication", terminal.clone()),
    ]))?;
    immutable(
        &journal,
        "completion.json",
        &bytes,
        fs.uid,
        deadline,
        cancelled,
    )
}

struct Retained {
    manifest: JsonValue,
    digest: String,
    plan: FrozenPlan,
    raw_plan: WorkPlan,
}
/// Narrow read profile for the native record-revision admission bridge. The
/// legacy transaction observers keep their original wider profile; this
/// adapter binds only the maintained three-file record-revision plan before
/// any retained side blobs are expanded.
#[derive(Clone, Copy)]
pub(crate) struct RecordRevisionInspectionLimits {
    pub(crate) max_manifest_bytes: usize,
    pub(crate) max_files: usize,
    pub(crate) max_side_bytes: usize,
    pub(crate) max_total_side_bytes: usize,
    pub(crate) max_total_blob_bytes: usize,
}

fn load_retained(
    fs: &CreationFilesystem,
    id: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Retained>> {
    load_retained_with_limits(fs, id, None, deadline, cancelled)
}

fn load_retained_with_limits(
    fs: &CreationFilesystem,
    id: &str,
    limits: Option<RecordRevisionInspectionLimits>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Retained>> {
    if let Some(limits) = limits {
        if limits.max_manifest_bytes == 0
            || limits.max_manifest_bytes > MAX_MANIFEST
            || limits.max_files == 0
            || limits.max_files > 3
            || limits.max_side_bytes == 0
            || limits.max_side_bytes > MAX_SIDE
            || limits.max_total_side_bytes == 0
            || limits.max_total_side_bytes > MAX_SIDE
            || limits.max_total_blob_bytes == 0
            || limits.max_total_blob_bytes > 2 * MAX_SIDE
        {
            return Err(SourceCommandError::Invalid(
                "record revision retained inspection profile",
            ));
        }
    }
    let Some(journal) = journal_dir(fs, id, false)? else {
        return Ok(None);
    };
    let (raw, manifest_mode) = read_at_mode(
        &journal,
        "manifest.json",
        fs.uid,
        limits.map_or(MAX_MANIFEST, |selected| selected.max_manifest_bytes),
        deadline,
        cancelled,
    )?
    .ok_or(SourceCommandError::Conflict(
        "retained transaction manifest absent",
    ))?;
    if !matches!(manifest_mode, 0o600 | 0o644) {
        return Err(SourceCommandError::Denied(
            "retained transaction manifest mode",
        ));
    }
    let manifest = cmd::parse(&raw)?;
    cmd::exact_keys(
        &manifest,
        &[
            "schema_version",
            "transaction_id",
            "base_publication",
            "plan",
            "parents",
        ],
    )?;
    if !matches!(
        cmd::text(&manifest, "schema_version")?,
        MANIFEST_SCHEMA | PROFILED_MANIFEST_SCHEMA | CANONICAL_FORM_MANIFEST_SCHEMA
    ) || cmd::text(&manifest, "transaction_id")? != id
    {
        return Err(SourceCommandError::Conflict(
            "retained transaction manifest identity",
        ));
    }
    let base = cmd::field(&manifest, "base_publication")?;
    cmd::exact_keys(base, &["token", "generation"])?;
    let generation = cmd::integer(base, "generation")?;
    let base_token = cmd::field(base, "token")?;
    if generation > MAX_GENERATION - 2
        || (base_token == &JsonValue::Null) != (generation == 0)
        || base_token != &JsonValue::Null && !base_token.as_str().is_some_and(hash)
    {
        return Err(SourceCommandError::Conflict(
            "retained publication predecessor",
        ));
    }
    let summary = cmd::field(&manifest, "plan")?;
    if summary.object_get("path_profile").is_some() {
        cmd::exact_keys(
            summary,
            &["authorization", "files", "new_directories", "path_profile"],
        )?;
    } else {
        cmd::exact_keys(summary, &["authorization", "files", "new_directories"])?;
    }
    let item_path_profile = selected_profile(summary)?;
    let canonical_form = canonical_forms_profile(summary)?;
    if (cmd::text(&manifest, "schema_version")? == PROFILED_MANIFEST_SCHEMA)
        != item_path_profile.is_some()
        || (cmd::text(&manifest, "schema_version")? == CANONICAL_FORM_MANIFEST_SCHEMA)
            != canonical_form.is_some()
    {
        return Err(SourceCommandError::Conflict(
            "retained Item manifest/profile version differs",
        ));
    }
    let companions = item_companions(
        cmd::field(summary, "authorization")?,
        item_path_profile.as_ref(),
    )?;
    // A repeated blob reference is legal, but its selected side still pays
    // for every file binding. Preflight both complete sides before any raw
    // retained blob is cloned into the expanded plan.
    let entries = cmd::array(summary, "files")?;
    let max_files = limits.map_or(MAX_FILES, |selected| selected.max_files);
    if entries.is_empty() || entries.len() > max_files {
        return Err(SourceCommandError::Invalid(
            "retained selected file count budget",
        ));
    }
    let mut side_totals = [0usize; 2];
    let mut prior_path: Option<&str> = None;
    for entry in entries {
        cmd::exact_keys(entry, &["path", "before", "after"])?;
        let name = cmd::text(entry, "path")?;
        profiled_path(name, false, &companions, canonical_form.as_ref())?;
        if prior_path.is_some_and(|prior| prior >= name) {
            return Err(SourceCommandError::Conflict(
                "retained selected path order or duplicate",
            ));
        }
        prior_path = Some(name);
        for (index, side) in ["before", "after"].into_iter().enumerate() {
            let value = cmd::field(entry, side)?;
            if value == &JsonValue::Null {
                continue;
            }
            cmd::exact_keys(value, &["sha256", "bytes"])?;
            if !hash(cmd::text(value, "sha256")?) {
                return Err(SourceCommandError::Conflict(
                    "retained selected side digest",
                ));
            }
            let count = usize::try_from(cmd::integer(value, "bytes")?)
                .map_err(|_| SourceCommandError::Invalid("retained side size range"))?;
            side_totals[index] = side_totals[index]
                .checked_add(count)
                .ok_or(SourceCommandError::Invalid("retained side size overflow"))?;
            let max_side = limits.map_or(MAX_SIDE, |selected| selected.max_side_bytes);
            let max_total_side = limits.map_or(MAX_SIDE, |selected| selected.max_total_side_bytes);
            if count > max_side || side_totals[index] > max_total_side {
                return Err(SourceCommandError::Invalid(
                    "retained selected side byte budget",
                ));
            }
        }
    }
    let mut total = 0usize;
    let mut cache = BTreeMap::new();
    let mut files = Vec::new();
    let read_side = |value: &JsonValue,
                     total: &mut usize,
                     cache: &mut BTreeMap<String, Vec<u8>>|
     -> SourceCommandResult<Option<Vec<u8>>> {
        if value == &JsonValue::Null {
            return Ok(None);
        }
        cmd::exact_keys(value, &["sha256", "bytes"])?;
        let sha = cmd::text(value, "sha256")?;
        let count = usize::try_from(cmd::integer(value, "bytes")?)
            .map_err(|_| SourceCommandError::Invalid("retained side size range"))?;
        if !hash(sha) || count > limits.map_or(MAX_SIDE, |selected| selected.max_side_bytes) {
            return Err(SourceCommandError::Conflict(
                "retained selected side binding",
            ));
        }
        if let Some(cached) = cache.get(sha) {
            if cached.len() != count {
                return Err(SourceCommandError::Conflict("retained blob size alias"));
            }
            return Ok(Some(cached.clone()));
        }
        *total = total
            .checked_add(count)
            .ok_or(SourceCommandError::Invalid("retained blob overflow"))?;
        if *total > limits.map_or(2 * MAX_SIDE, |selected| selected.max_total_blob_bytes) {
            return Err(SourceCommandError::Invalid("retained blob total budget"));
        }
        let (bytes, mode) = read_at_mode(
            &journal,
            &format!("{}.blob", &sha[7..]),
            fs.uid,
            count,
            deadline,
            cancelled,
        )?
        .ok_or(SourceCommandError::Conflict("retained blob absent"))?;
        if !matches!(mode, 0o600 | 0o644)
            || bytes.len() != count
            || Digest256::of_bytes(&bytes).to_prefixed() != sha
        {
            return Err(SourceCommandError::Conflict("retained blob fixity"));
        }
        cache.insert(sha.to_owned(), bytes.clone());
        Ok(Some(bytes))
    };
    for entry in entries {
        cmd::exact_keys(entry, &["path", "before", "after"])?;
        let name = cmd::text(entry, "path")?;
        files.push(SelectedFile {
            path: profiled_path(name, false, &companions, canonical_form.as_ref())?,
            before: read_side(cmd::field(entry, "before")?, &mut total, &mut cache)?,
            after: read_side(cmd::field(entry, "after")?, &mut total, &mut cache)?,
        });
    }
    let directories = cmd::array(summary, "new_directories")?
        .iter()
        .map(|v| {
            path(
                v.as_str()
                    .ok_or(SourceCommandError::Invalid("retained new directory"))?,
                true,
            )
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let raw_plan = WorkPlan {
        transaction_id: id.to_owned(),
        authorization: cmd::field(summary, "authorization")?.clone(),
        item_path_profile,
        files,
        new_directories: directories,
    };
    let plan = freeze(raw_plan.clone())?;
    if !cmd::same(&plan.summary, summary)? {
        return Err(SourceCommandError::Conflict(
            "retained plan order/binding differs",
        ));
    }
    let parents = cmd::field(&manifest, "parents")?;
    let entries = parents
        .as_object()
        .ok_or(SourceCommandError::Invalid("retained parent map"))?;
    let expected = parent_refs(&plan)?;
    if entries.len() != expected.len()
        || expected
            .iter()
            .any(|refname| parents.object_get(refname).is_none())
    {
        return Err(SourceCommandError::Conflict(
            "retained parent closure differs",
        ));
    }
    for (name, value) in entries {
        let reference = name
            .as_str()
            .ok_or(SourceCommandError::Invalid("retained parent ref"))?;
        if value == &JsonValue::Null
            && !plan.directories.iter().any(|dir| dir.as_str() == reference)
        {
            return Err(SourceCommandError::Conflict(
                "undeclared absent retained parent",
            ));
        }
        if value != &JsonValue::Null {
            cmd::exact_keys(value, &["device", "inode", "mode", "uid"])?;
            let mode = cmd::integer(value, "mode")?;
            if mode & 0o170000 != 0o040000 || mode & 0o022 != 0 {
                return Err(SourceCommandError::Denied("retained parent mode unsafe"));
            }
            for key in ["device", "inode", "uid"] {
                cmd::integer(value, key)?;
            }
        }
    }
    let digest = Digest256::of_bytes(&raw).to_prefixed();
    if let Some((completion_raw, mode)) = read_at_mode(
        &journal,
        "completion.json",
        fs.uid,
        8192,
        deadline,
        cancelled,
    )? {
        if !matches!(mode, 0o600 | 0o644) {
            return Err(SourceCommandError::Denied("retained completion mode"));
        }
        let completion = cmd::parse(&completion_raw)?;
        cmd::exact_keys(&completion, &["schema_version", "publication"])?;
        if cmd::text(&completion, "schema_version")? != COMPLETION_SCHEMA {
            return Err(SourceCommandError::Conflict("retained completion schema"));
        }
        let terminal = cmd::field(&completion, "publication")?;
        state(terminal)?;
        if cmd::text(terminal, "phase")? != "ready"
            || cmd::text(terminal, "transaction_id")? != id
            || cmd::text(terminal, "manifest_sha256")? != digest
            || cmd::integer(terminal, "generation")? != generation + 2
        {
            return Err(SourceCommandError::Conflict(
                "retained completion/manifest binding",
            ));
        }
    }
    Ok(Some(Retained {
        manifest,
        digest,
        plan,
        raw_plan,
    }))
}

/// Read only the head-selected pending plan.  It grants no mutation, and no
/// orphan journal is implicitly adopted as a pending transaction.
pub(crate) struct PendingWork {
    pub(crate) state: JsonValue,
    pub(crate) base_publication: JsonValue,
    pub(crate) plan: WorkPlan,
}
pub(crate) fn read_pending(
    fs: &CreationFilesystem,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<PendingWork>> {
    let Some(current) = read_state(fs, deadline, cancelled)? else {
        return Ok(None);
    };
    if cmd::text(&current, "phase")? != "pending" {
        return Ok(None);
    }
    let id = cmd::text(&current, "transaction_id")?;
    let retained = load_retained(fs, id, deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("pending selected transaction manifest absent"),
    )?;
    if cmd::text(&current, "manifest_sha256")? != retained.digest
        || cmd::integer(&current, "generation")?
            != cmd::integer(
                cmd::field(&retained.manifest, "base_publication")?,
                "generation",
            )? + 1
    {
        return Err(SourceCommandError::Conflict(
            "pending publication/manifest binding",
        ));
    }
    if !same_state(
        Some(&current),
        read_state(fs, deadline, cancelled)?.as_ref(),
    )? {
        return Err(SourceCommandError::Conflict(
            "pending publication changed during read",
        ));
    }
    Ok(Some(PendingWork {
        state: current,
        base_publication: cmd::field(&retained.manifest, "base_publication")?.clone(),
        plan: retained.raw_plan,
    }))
}

/// Historical transport evidence for a previously linked native Expression.
/// This validates a selected ready head or immutable completion, not current
/// Work lineage, source bytes, permission to replay, or semantic admission.
pub(crate) fn retained_item_orphan(
    fs: &CreationFilesystem,
    id: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<(WorkPlan, JsonValue)>> {
    let Some(retained) = load_retained(fs, id, deadline, cancelled)? else {
        return Ok(None);
    };
    if cmd::text(&retained.raw_plan.authorization, "schema_version")?
        != "tos_item_adoption_authorization_v1"
    {
        return Err(SourceCommandError::Denied("retained Item authority family"));
    }
    let head = read_state(fs, deadline, cancelled)?;
    if head
        .as_ref()
        .is_some_and(|v| cmd::text(v, "transaction_id").ok() == Some(id))
    {
        return Err(SourceCommandError::Conflict(
            "Item transaction is head selected; orphan recovery refused",
        ));
    }
    let journal = journal_dir(fs, id, false)?
        .ok_or(SourceCommandError::Conflict("Item retained journal absent"))?;
    if read_at(
        &journal,
        "completion.json",
        fs.uid,
        MAX_STATE,
        deadline,
        cancelled,
    )?
    .is_some()
    {
        return Err(SourceCommandError::Conflict(
            "Item transaction already terminated",
        ));
    }
    Ok(Some((
        retained.raw_plan,
        cmd::field(&retained.manifest, "base_publication")?.clone(),
    )))
}
/// Exact auxiliary members for an already inspected committed plan.
/// Refreezing reuses the selected journal path and manifest byte law.
pub(crate) fn committed_member_paths(plan: &WorkPlan) -> SourceCommandResult<BTreeSet<String>> {
    let frozen = freeze(plan.clone())?;
    let mut members = journal_members(&plan.transaction_id, &frozen)?;
    members.insert(format!(
        "{HOME}/{TRANSACTIONS}/{}/completion.json",
        &plan.transaction_id[7..]
    ));
    Ok(members)
}

pub(crate) fn inspect_committed(
    fs: &CreationFilesystem,
    id: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, WorkPlan, JsonValue, JsonValue)> {
    inspect_terminal(fs, id, "committed", deadline, cancelled)
}

/// Inspect one committed selected-metadata revision under its smaller exact
/// file/side profile. This is still the Work owner’s journal/fixity verifier;
/// the caller supplies a narrower resource envelope, never an authority token.
pub(crate) fn inspect_committed_record_revision(
    fs: &CreationFilesystem,
    id: &str,
    limits: RecordRevisionInspectionLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, WorkPlan, JsonValue, JsonValue)> {
    inspect_terminal_with_limits(fs, id, "committed", Some(limits), deadline, cancelled)
}

pub(crate) fn inspect_rolled_back(
    fs: &CreationFilesystem,
    id: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, WorkPlan, JsonValue, JsonValue)> {
    inspect_terminal(fs, id, "rolled-back", deadline, cancelled)
}

fn inspect_terminal(
    fs: &CreationFilesystem,
    id: &str,
    outcome: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, WorkPlan, JsonValue, JsonValue)> {
    inspect_terminal_with_limits(fs, id, outcome, None, deadline, cancelled)
}

fn inspect_terminal_with_limits(
    fs: &CreationFilesystem,
    id: &str,
    outcome: &str,
    limits: Option<RecordRevisionInspectionLimits>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(String, WorkPlan, JsonValue, JsonValue)> {
    let head = read_state(fs, deadline, cancelled)?;
    let retained = load_retained_with_limits(fs, id, limits, deadline, cancelled)?.ok_or(
        SourceCommandError::Conflict("prior Work transaction absent"),
    )?;
    let journal = journal_dir(fs, id, false)?
        .ok_or(SourceCommandError::Conflict("prior Work journal absent"))?;
    let directory_before = owned(&journal, fs.uid, true)?;
    let completion = read_at_mode(
        &journal,
        "completion.json",
        fs.uid,
        8192,
        deadline,
        cancelled,
    )?
    .map(|(raw, mode)| -> SourceCommandResult<JsonValue> {
        if !matches!(mode, 0o600 | 0o644) {
            return Err(SourceCommandError::Denied("prior Work completion mode"));
        }
        let value = cmd::parse(&raw)?;
        cmd::exact_keys(&value, &["schema_version", "publication"])?;
        if cmd::text(&value, "schema_version")? != COMPLETION_SCHEMA {
            return Err(SourceCommandError::Conflict("prior Work completion schema"));
        }
        Ok(cmd::field(&value, "publication")?.clone())
    })
    .transpose()?;
    let mut expected_members = journal_members(id, &retained.plan)?;
    if completion.is_some() {
        expected_members.insert(format!(
            "{HOME}/{TRANSACTIONS}/{}/completion.json",
            &id[7..]
        ));
    }
    let mut observed_members = BTreeSet::new();
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}", journal.as_raw_fd()))
        .map_err(|_| SourceCommandError::Invalid("prior Work journal enumeration"))?
    {
        active(deadline, cancelled)?;
        let name = entry
            .map_err(|_| SourceCommandError::Invalid("prior Work journal entry"))?
            .file_name()
            .into_string()
            .map_err(|_| SourceCommandError::Denied("prior Work journal entry name"))?;
        observed_members.insert(format!("{HOME}/{TRANSACTIONS}/{}/{name}", &id[7..]));
        if observed_members.len() > expected_members.len() {
            return Err(SourceCommandError::Conflict(
                "prior Work extra journal member",
            ));
        }
    }
    if observed_members != expected_members {
        return Err(SourceCommandError::Conflict(
            "prior Work journal member set changed",
        ));
    }
    let directory_current = journal_dir(fs, id, false)?
        .ok_or(SourceCommandError::Conflict("prior Work journal detached"))?;
    if inode(&directory_before) != inode(&owned(&directory_current, fs.uid, true)?)
        || stamp(&directory_before) != stamp(&owned(&journal, fs.uid, true)?)
    {
        return Err(SourceCommandError::Conflict(
            "prior Work journal directory changed",
        ));
    }
    let selected = head
        .as_ref()
        .filter(|state| cmd::text(state, "transaction_id").ok() == Some(id));
    let terminal = selected
        .or(completion.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "prior Work transaction lacks terminal evidence",
        ))?;
    state(terminal)?;
    if cmd::text(terminal, "phase")? != "ready"
        || cmd::text(terminal, "outcome")? != outcome
        || cmd::text(terminal, "manifest_sha256")? != retained.digest
        || cmd::integer(terminal, "generation")?
            != cmd::integer(
                cmd::field(&retained.manifest, "base_publication")?,
                "generation",
            )? + 2
        || completion.as_ref().is_some_and(|receipt| {
            selected.is_some_and(|current| !cmd::same(receipt, current).unwrap_or(false))
        })
    {
        return Err(SourceCommandError::Conflict(
            "prior Work transaction completion differs",
        ));
    }
    if !same_state(head.as_ref(), read_state(fs, deadline, cancelled)?.as_ref())? {
        return Err(SourceCommandError::Conflict(
            "prior Work publication changed",
        ));
    }
    Ok((
        retained.digest,
        retained.raw_plan,
        cmd::field(&retained.manifest, "base_publication")?.clone(),
        terminal.clone(),
    ))
}

pub(crate) struct WorkTransportResult {
    pub(crate) transaction_id: String,
    pub(crate) manifest_sha256: String,
    pub(crate) publication: JsonValue,
    pub(crate) committed: bool,
}
pub(crate) fn still_pending(
    fs: &CreationFilesystem,
    pending: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if !same_state(Some(pending), read_state(fs, deadline, cancelled)?.as_ref())? {
        return Err(SourceCommandError::Conflict(
            "selected pending publication changed",
        ));
    }
    Ok(())
}
fn move_pending(
    fence: &WorkCorpusFence<'_>,
    retained: &Retained,
    pending: &JsonValue,
    rollback: bool,
    recovery_authorization: Option<JsonValue>,
    initial_full_membership: bool,
    guard: &mut impl FnMut(&JsonValue, WorkGuard<'_>) -> SourceCommandResult<()>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<WorkTransportResult> {
    let fs = fence.fs;
    let id = cmd::text(&retained.manifest, "transaction_id")?;
    let journal_members = journal_members(id, &retained.plan)?;
    let mut check = |full_membership: bool| -> SourceCommandResult<()> {
        fence.verify(deadline, cancelled)?;
        still_pending(fs, pending, deadline, cancelled)?;
        verify_retained(
            fs,
            id,
            &retained.manifest,
            &retained.digest,
            &retained.plan,
            deadline,
            cancelled,
        )?;
        guard(
            &retained.plan.summary,
            WorkGuard {
                full_membership,
                journal_members: &journal_members,
                pending_state: Some(pending),
                prior_completion_ready: true,
            },
        )
    };
    check(initial_full_membership)?;
    let mut parents = Parents::new(fs, &retained.manifest)?;
    parents.check_files(&retained.plan, None, deadline, cancelled)?;
    if !rollback {
        for reference in &retained.plan.directories {
            check(false)?;
            parents.check_files(&retained.plan, None, deadline, cancelled)?;
            let (parent_ref, leaf) = reference
                .as_str()
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("new selected directory parent"))?;
            let parent = parents
                .get(parent_ref)?
                .ok_or(SourceCommandError::Conflict(
                    "new selected directory ancestor absent",
                ))?;
            match rustix::fs::mkdirat(&parent, leaf, Mode::from_raw_mode(0o700)) {
                Ok(()) => parent
                    .sync_all()
                    .map_err(|_| SourceCommandError::Invalid("new selected directory fsync"))?,
                Err(Errno::EXIST) => (),
                Err(_) => return Err(SourceCommandError::Invalid("new selected directory mkdir")),
            }
            parents
                .get(reference.as_str())?
                .ok_or(SourceCommandError::Conflict(
                    "new selected directory unavailable",
                ))?;
        }
    }
    for file in &retained.plan.files {
        check(false)?;
        parents.check_files(&retained.plan, None, deadline, cancelled)?;
        let current = parents.selected(file, deadline, cancelled)?;
        let desired = if rollback { &file.before } else { &file.after };
        if current.as_deref() == desired.as_deref() {
            continue;
        }
        let (parent_ref, leaf) = file
            .path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("selected file parent"))?;
        let parent = parents
            .get(parent_ref)?
            .ok_or(SourceCommandError::Conflict("selected file parent absent"))?;
        if let Some(raw) = desired {
            let sha = Digest256::of_bytes(raw).to_prefixed();
            if retained.plan.blobs.get(&sha) != Some(raw) {
                return Err(SourceCommandError::Conflict(
                    "retained desired blob differs",
                ));
            }
            atomic_write(&parent, leaf, raw, current.is_none(), deadline, cancelled)?;
        } else {
            rustix::fs::unlinkat(&parent, leaf, AtFlags::empty())
                .map_err(|_| SourceCommandError::Conflict("selected file remove refused"))?;
            parent
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("selected file removal fsync"))?;
        }
        parents.verify()?;
    }
    if rollback {
        for reference in retained.plan.directories.iter().rev() {
            check(false)?;
            parents.check_files(&retained.plan, Some(false), deadline, cancelled)?;
            let (parent_ref, leaf) =
                reference
                    .as_str()
                    .rsplit_once('/')
                    .ok_or(SourceCommandError::Invalid(
                        "rollback selected directory parent",
                    ))?;
            let Some(parent) = parents.get(parent_ref)? else {
                continue;
            };
            match rustix::fs::unlinkat(&parent, leaf, AtFlags::REMOVEDIR) {
                Ok(()) => parent
                    .sync_all()
                    .map_err(|_| SourceCommandError::Invalid("rollback directory fsync"))?,
                Err(Errno::NOENT) => (),
                Err(_) => {
                    return Err(SourceCommandError::Conflict(
                        "rollback directory has unselected contents",
                    ));
                }
            }
            parents.opened.remove(reference.as_str());
        }
    }
    check(false)?;
    parents.check_files(&retained.plan, Some(!rollback), deadline, cancelled)?;
    parents.sync_selected(&retained.plan, deadline, cancelled)?;
    parents.check_files(&retained.plan, Some(!rollback), deadline, cancelled)?;
    check(true)?;
    parents.verify()?;
    let terminal = publication_state(
        &retained.manifest,
        &retained.digest,
        false,
        Some(if rollback { "rolled-back" } else { "committed" }),
        recovery_authorization,
    )?;
    publish_state(fs, &terminal, Some(pending), deadline, cancelled)?;
    // A failed completion write after ready is an observed terminal publication;
    // the exact head+manifest remains sufficient to complete on a later call.
    record_completion(fs, &terminal, deadline, cancelled)?;
    if !same_state(
        Some(&terminal),
        read_state(fs, deadline, cancelled)?.as_ref(),
    )? {
        return Err(SourceCommandError::Conflict("terminal publication changed"));
    }
    let completion = cmd::object(vec![
        ("schema_version", cmd::string(COMPLETION_SCHEMA)),
        ("publication", terminal.clone()),
    ]);
    let journal = journal_dir(fs, id, false)?
        .ok_or(SourceCommandError::Conflict("terminal journal detached"))?;
    if read_at(
        &journal,
        "completion.json",
        fs.uid,
        MAX_STATE,
        deadline,
        cancelled,
    )?
    .as_deref()
        != Some(encoded(&completion)?.as_slice())
    {
        return Err(SourceCommandError::Conflict("terminal completion changed"));
    }
    Ok(WorkTransportResult {
        transaction_id: id.to_owned(),
        manifest_sha256: retained.digest.clone(),
        publication: terminal,
        committed: !rollback,
    })
}

impl WorkCorpusFence<'_> {
    /// Caller owns the Work-specific complete authority/schema/lineage guard;
    /// this method never accepts a serialized plan from an external request.
    pub(crate) fn apply(
        &self,
        plan: WorkPlan,
        snapshot: &PublicationSnapshot,
        mut guard: impl FnMut(&JsonValue, WorkGuard<'_>) -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<WorkTransportResult> {
        self.apply_selected(
            plan, snapshot, None, false, false, guard, deadline, cancelled,
        )
    }
    /// Initial-only caller uses the SAME mover and issuer. Refuse a preexisting
    /// head/journal before decoding any foreign retained plan; this entry cannot
    /// recover, adopt or replay another owner's serialized transaction.
    pub(crate) fn apply_initial(
        &self,
        plan: WorkPlan,
        snapshot: &PublicationSnapshot,
        guard: impl FnMut(&JsonValue, WorkGuard<'_>) -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<WorkTransportResult> {
        if snapshot.token.is_some() || snapshot.generation != 0 {
            return Err(SourceCommandError::Conflict(
                "initial selected publication already exists",
            ));
        }
        self.apply_selected(
            plan, snapshot, None, false, true, guard, deadline, cancelled,
        )
    }
    pub(crate) fn apply_retained_item(
        &self,
        plan: WorkPlan,
        snapshot: &PublicationSnapshot,
        renewal: Option<JsonValue>,
        guard: impl FnMut(&JsonValue, WorkGuard<'_>) -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<WorkTransportResult> {
        if cmd::text(&plan.authorization, "schema_version")? != "tos_item_adoption_authorization_v1"
            || renewal
                .as_ref()
                .map(cmd::canonical)
                .transpose()?
                .is_some_and(|raw| raw.len() > 4096)
        {
            return Err(SourceCommandError::Denied(
                "retained Item selection differs",
            ));
        }
        self.apply_selected(
            plan, snapshot, renewal, true, false, guard, deadline, cancelled,
        )
    }
    fn apply_selected(
        &self,
        plan: WorkPlan,
        snapshot: &PublicationSnapshot,
        renewal: Option<JsonValue>,
        item_retained: bool,
        initial_only: bool,
        mut guard: impl FnMut(&JsonValue, WorkGuard<'_>) -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<WorkTransportResult> {
        self.verify(deadline, cancelled)?;
        snapshot.verify_current(self.fs, deadline, cancelled)?;
        let id = plan.transaction_id.clone();
        let frozen = freeze(plan)?;
        let no_journal = BTreeSet::new();
        guard(
            &frozen.summary,
            WorkGuard {
                full_membership: false,
                journal_members: &no_journal,
                pending_state: None,
                prior_completion_ready: false,
            },
        )?;
        let current = read_state(self.fs, deadline, cancelled)?;
        let existing = if initial_only {
            if current.is_some() || journal_dir(self.fs, &id, false)?.is_some() {
                return Err(SourceCommandError::Conflict(
                    "initial selected transaction already exists",
                ));
            }
            None
        } else {
            load_retained(self.fs, &id, deadline, cancelled)?
        };
        if item_retained && existing.is_none() {
            return Err(SourceCommandError::Conflict(
                "selected Item orphan vanished",
            ));
        }
        if let Some(existing) = &existing {
            if !cmd::same(&existing.plan.summary, &frozen.summary)?
                || existing
                    .plan
                    .files
                    .iter()
                    .zip(&frozen.files)
                    .any(|(a, b)| a.path != b.path || a.before != b.before || a.after != b.after)
            {
                return Err(SourceCommandError::Conflict(
                    "transaction id reused for different exact plan",
                ));
            }
            // A pending head is deliberately rejected by the publication
            // snapshot above. Only the Work owner's explicit selected
            // recovery path may resume or roll back it.
            if !item_retained {
                return Err(SourceCommandError::Conflict(
                    "retained transaction requires owner replay or selected recovery",
                ));
            }
            if !cmd::same(
                cmd::field(&existing.manifest, "base_publication")?,
                &cmd::object(vec![
                    (
                        "token",
                        snapshot
                            .token
                            .as_ref()
                            .map_or(JsonValue::Null, |v| cmd::string(v)),
                    ),
                    ("generation", cmd::number(snapshot.generation)),
                ]),
            )? || read_at(
                &journal_dir(self.fs, &id, false)?
                    .ok_or(SourceCommandError::Conflict("Item retained journal absent"))?,
                "completion.json",
                self.fs.uid,
                MAX_STATE,
                deadline,
                cancelled,
            )?
            .is_some()
            {
                return Err(SourceCommandError::Conflict(
                    "retained Item is not an orphan on this exact snapshot",
                ));
            }
        }
        if let Some(previous) = &current {
            if cmd::text(previous, "phase")? != "ready" {
                return Err(SourceCommandError::Conflict(
                    "another selected transaction pending",
                ));
            }
            let prior_id = cmd::text(previous, "transaction_id")?;
            let prior = load_retained(self.fs, prior_id, deadline, cancelled)?.ok_or(
                SourceCommandError::Conflict("previous ready manifest missing"),
            )?;
            if cmd::text(previous, "manifest_sha256")? != prior.digest
                || cmd::integer(previous, "generation")?
                    != cmd::integer(
                        cmd::field(&prior.manifest, "base_publication")?,
                        "generation",
                    )? + 2
            {
                return Err(SourceCommandError::Conflict(
                    "previous ready manifest differs",
                ));
            }
            record_completion(self.fs, previous, deadline, cancelled)?;
        }
        let manifest = if let Some(existing) = &existing {
            existing.manifest.clone()
        } else {
            manifest(&id, snapshot, &frozen, capture_parents(self.fs, &frozen)?)
        };
        let mut opened = Parents::new(self.fs, &manifest)?;
        opened.check_files(&frozen, Some(false), deadline, cancelled)?;
        let digest = retain(self.fs, &id, &manifest, &frozen, deadline, cancelled)?;
        self.verify(deadline, cancelled)?;
        snapshot.verify_current(self.fs, deadline, cancelled)?;
        verify_retained(
            self.fs, &id, &manifest, &digest, &frozen, deadline, cancelled,
        )?;
        let journal_members = journal_members(&id, &frozen)?;
        guard(
            &frozen.summary,
            WorkGuard {
                full_membership: true,
                journal_members: &journal_members,
                pending_state: None,
                prior_completion_ready: current.is_some(),
            },
        )?;
        opened.check_files(&frozen, Some(false), deadline, cancelled)?;
        let pending = publication_state(&manifest, &digest, true, None, None)?;
        publish_state(self.fs, &pending, current.as_ref(), deadline, cancelled)?;
        let retained = Retained {
            manifest,
            digest,
            plan: frozen.clone(),
            raw_plan: WorkPlan {
                transaction_id: id,
                authorization: cmd::field(&frozen.summary, "authorization")?.clone(),
                item_path_profile: selected_profile(&frozen.summary)?,
                files: frozen.files.clone(),
                new_directories: frozen.directories.clone(),
            },
        };
        move_pending(
            self, &retained, &pending, false, renewal, false, &mut guard, deadline, cancelled,
        )
    }

    /// Only the head-selected exact pending plan may be resumed or rolled back;
    /// the Work owner reconstructs its original request and current delegation
    /// from `read_pending` before invoking this method.
    pub(crate) fn recover(
        &self,
        selected: &PendingWork,
        rollback: bool,
        recovery_authorization: Option<JsonValue>,
        mut guard: impl FnMut(&JsonValue, WorkGuard<'_>) -> SourceCommandResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<WorkTransportResult> {
        self.verify(deadline, cancelled)?;
        let id = cmd::text(&selected.state, "transaction_id")?;
        let retained = load_retained(self.fs, id, deadline, cancelled)?.ok_or(
            SourceCommandError::Conflict("selected pending manifest absent"),
        )?;
        let requested = freeze(selected.plan.clone())?;
        if !cmd::same(&requested.summary, &retained.plan.summary)?
            || cmd::text(&selected.state, "manifest_sha256")? != retained.digest
            || !same_state(
                Some(&selected.state),
                read_state(self.fs, deadline, cancelled)?.as_ref(),
            )?
        {
            return Err(SourceCommandError::Conflict(
                "selected pending recovery plan changed",
            ));
        }
        if let Some(value) = &recovery_authorization {
            if cmd::canonical(value)?.len() > 4096 {
                return Err(SourceCommandError::Invalid(
                    "recovery authorization byte budget",
                ));
            }
        }
        move_pending(
            self,
            &retained,
            &selected.state,
            rollback,
            recovery_authorization,
            true,
            &mut guard,
            deadline,
            cancelled,
        )
    }
}
impl<'a> Parents<'a> {
    fn new(fs: &'a CreationFilesystem, manifest: &JsonValue) -> SourceCommandResult<Self> {
        let expected = cmd::field(manifest, "parents")?.clone();
        let new = cmd::array(cmd::field(manifest, "plan")?, "new_directories")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or(SourceCommandError::Invalid("new directory ref"))
            })
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        let parents = Self {
            fs,
            expected,
            new,
            opened: BTreeMap::new(),
        };
        parents.verify()?;
        Ok(parents)
    }
    fn expected(&self, reference: &str) -> SourceCommandResult<&JsonValue> {
        self.expected
            .object_get(reference)
            .ok_or(SourceCommandError::Invalid(
                "retained parent binding absent",
            ))
    }
    fn verify(&self) -> SourceCommandResult<()> {
        let entries = self
            .expected
            .as_object()
            .ok_or(SourceCommandError::Invalid("retained parent map"))?;
        for (name, expected) in entries {
            let reference = name
                .as_str()
                .ok_or(SourceCommandError::Invalid("parent ref text"))?;
            let current = read_existing_parent(self.fs, reference)?;
            match current {
                None if expected == &JsonValue::Null && !self.opened.contains_key(reference) => (),
                None => {
                    return Err(SourceCommandError::Conflict(
                        "selected metadata parent disappeared",
                    ));
                }
                Some(fd) => {
                    let observed = dir_binding(&fd, self.fs.uid)?;
                    if expected != &JsonValue::Null && !cmd::same(&observed, expected)? {
                        return Err(SourceCommandError::Conflict(
                            "selected metadata parent changed",
                        ));
                    }
                    if let Some(held) = self.opened.get(reference) {
                        if !cmd::same(&observed, &dir_binding(held, self.fs.uid)?)? {
                            return Err(SourceCommandError::Conflict(
                                "selected metadata parent detached",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn get(&mut self, reference: &str) -> SourceCommandResult<Option<File>> {
        if !self.opened.contains_key(reference) {
            let current = read_existing_parent(self.fs, reference)?;
            let Some(fd) = current else {
                if self.new.contains(reference) && self.expected(reference)? == &JsonValue::Null {
                    return Ok(None);
                }
                return Err(SourceCommandError::Conflict(
                    "selected metadata parent absent",
                ));
            };
            let expected = self.expected(reference)?;
            if expected != &JsonValue::Null
                && !cmd::same(&dir_binding(&fd, self.fs.uid)?, expected)?
            {
                return Err(SourceCommandError::Conflict(
                    "selected metadata parent binding",
                ));
            }
            self.opened.insert(reference.to_owned(), fd);
        }
        self.opened
            .get(reference)
            .unwrap()
            .try_clone()
            .map(Some)
            .map_err(|_| SourceCommandError::Invalid("selected metadata parent descriptor clone"))
    }
    fn selected(
        &mut self,
        file: &SelectedFile,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<Vec<u8>>> {
        let reference = selected_parent(file.path.as_str())?;
        let Some(parent) = self.get(reference)? else {
            return Ok(None);
        };
        let limit = file
            .before
            .as_ref()
            .map(Vec::len)
            .unwrap_or(0)
            .max(file.after.as_ref().map(Vec::len).unwrap_or(0));
        read_at(
            &parent,
            file.path.as_str().rsplit('/').next().unwrap(),
            self.fs.uid,
            limit,
            deadline,
            cancelled,
        )
    }
    fn check_files(
        &mut self,
        plan: &FrozenPlan,
        side: Option<bool>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify()?;
        for file in &plan.files {
            active(deadline, cancelled)?;
            let observed = self.selected(file, deadline, cancelled)?;
            let allowed = match side {
                Some(true) => observed.as_deref() == file.after.as_deref(),
                Some(false) => observed.as_deref() == file.before.as_deref(),
                None => {
                    observed.as_deref() == file.before.as_deref()
                        || observed.as_deref() == file.after.as_deref()
                }
            };
            if !allowed {
                return Err(SourceCommandError::Conflict(
                    "selected file has third or wrong side state",
                ));
            }
        }
        Ok(())
    }
    fn sync_selected(
        &mut self,
        plan: &FrozenPlan,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        for file in &plan.files {
            active(deadline, cancelled)?;
            let reference = selected_parent(file.path.as_str())?;
            let Some(parent) = self.get(reference)? else {
                continue;
            };
            let name = file.path.as_str().rsplit('/').next().unwrap();
            match rustix::fs::openat(
                &parent,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(fd) => {
                    let fd: File = fd.into();
                    owned(&fd, self.fs.uid, false)?;
                    fd.sync_all()
                        .map_err(|_| SourceCommandError::Invalid("selected file recovery fsync"))?;
                }
                Err(Errno::NOENT) => (),
                Err(_) => return Err(SourceCommandError::Denied("selected file recovery unsafe")),
            }
        }
        for fd in self.opened.values() {
            fd.sync_all()
                .map_err(|_| SourceCommandError::Invalid("selected parent recovery fsync"))?;
        }
        Ok(())
    }
}
