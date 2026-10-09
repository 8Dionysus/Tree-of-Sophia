//! Bounded v4 owner-local assessment source selection.
//!
//! This adapter reads only source records explicitly selected by the protected
//! assessment owner. It validates the existing semantic profile and adjacent
//! HumanForm contracts, returns the native assessment record envelopes, and
//! retains the exact read descriptors and bytes for a later owner-fenced
//! currentness check. It has no journal, canon, or publication write path.

use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_sign_native::{
    NativeInput, NativeReadKind, NativeReadScope, ResolvedSignNative, SignNativeRead,
};
use crate::source_text_owner::OwnerTextContext;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue, RelativePath, SourceRevision};
use tos_source_store::CorpusCutReader;
use tos_validation::assessment::{AssessmentRecordInput, MAX_ASSESSMENTS, MAX_RECORD_BYTES};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

const MAX_SELECTIONS: usize = 64;
const MAX_SELECTED_FILES: usize = 128;
const MAX_SELECTED_METADATA_BYTES: usize = 8_388_608;
const MAX_SELECTED_CONTENT_BYTES: usize = 8_388_608;
const MAX_SELECTED_FILE_BYTES: usize = 8_388_608;
const MAX_SELECTION_BYTES: usize = 1_048_576;
const MAX_FORMS_PER_SOURCE: usize = 32;
const MAX_FORM_SET_BYTES: usize = 2 * MAX_RECORD_BYTES;
const MAX_NATIVE_IDENTITY_FILES: usize = 1024;
const MAX_NATIVE_IDENTITY_BYTES: usize = 8_388_608;
const MAX_NATIVE_IDENTITY_SCAN_ENTRIES: usize = 65_536;
const MAX_NATIVE_IDENTITY_SCAN_DEPTH: usize = 64;
const PRIVATE_HOME: &str = "ToS/source-witnesses/owner-local/";
const PUBLIC_SOURCE_HOME: &str = "ToS/source-witnesses/";
const PUBLIC_SCHEMA_HOME: &str = "ToS/contracts/";
const PUBLIC_CLAIM_RELATIONS: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const PUBLIC_CLAIM_ENTITIES: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const OWNER_ASSESSMENT_FORM_CONTRACTS: &[&str] = &[
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/knowledge-assessment-policy.schema.json",
    "ToS/contracts/knowledge-assessment-authority.schema.json",
    "ToS/contracts/knowledge-assessment-competence.schema.json",
    "ToS/contracts/knowledge-assessment-batch.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
    uid: u32,
    mode: u32,
}

impl DirectoryIdentity {
    fn read(file: &File, private: bool, uid: u32) -> SourceCommandResult<Self> {
        let metadata = file
            .metadata()
            .map_err(|_| SourceCommandError::Denied("assessment source directory metadata"))?;
        if !metadata.is_dir()
            || if private {
                metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700
            } else {
                ![0, uid].contains(&metadata.uid()) || metadata.mode() & 0o022 != 0
            }
        {
            return Err(SourceCommandError::Denied(
                "assessment source directory owner or mode",
            ));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            uid: metadata.uid() as u32,
            mode: metadata.mode() & 0o7777,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    uid: u32,
    mode: u32,
    links: u64,
    length: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl FileIdentity {
    fn read(file: &File, private: bool, uid: u32) -> SourceCommandResult<Self> {
        let metadata = file
            .metadata()
            .map_err(|_| SourceCommandError::Denied("assessment source file metadata"))?;
        if !metadata.is_file()
            || if private {
                metadata.uid() != uid || metadata.mode() & 0o7777 != 0o600
            } else {
                ![0, uid].contains(&metadata.uid()) || metadata.mode() & 0o022 != 0
            }
        {
            return Err(SourceCommandError::Denied(
                "assessment source file owner or mode",
            ));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            uid: metadata.uid() as u32,
            mode: metadata.mode() & 0o7777,
            links: metadata.nlink(),
            length: metadata.len(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryListingIdentity {
    device: u64,
    inode: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl DirectoryListingIdentity {
    fn read(file: &File) -> SourceCommandResult<Self> {
        let metadata = file
            .metadata()
            .map_err(|_| SourceCommandError::Conflict("assessment source directory changed"))?;
        if !metadata.is_dir() {
            return Err(SourceCommandError::Conflict(
                "assessment source directory changed",
            ));
        }
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        })
    }
}

fn public_native_identity_member(reference: &str) -> bool {
    let basename = reference.rsplit('/').next().unwrap_or(reference);
    reference.starts_with(PUBLIC_SOURCE_HOME)
        && basename.starts_with("semantic-annotation")
        && basename.ends_with(".json")
        && !reference
            .split('/')
            .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
}

fn cut_native_identity_paths(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
) -> SourceCommandResult<BTreeSet<String>> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "assessment identity inventory source cut differs",
        ));
    }
    let mut paths = BTreeSet::new();
    let mut total_bytes = 0usize;
    for member in cut.current().members() {
        let reference = member.path.as_str();
        if reference == "ToS/source-witnesses/owner-local"
            || reference.starts_with("ToS/source-witnesses/owner-local/")
        {
            return Err(SourceCommandError::Denied(
                "reserved owner-local namespace cannot enter public native identity inventory",
            ));
        }
        if !public_native_identity_member(reference) {
            continue;
        }
        if paths.len() >= MAX_NATIVE_IDENTITY_FILES {
            return Err(SourceCommandError::Invalid(
                "assessment native identity packet-count budget",
            ));
        }
        let bytes = usize::try_from(member.size_bytes).map_err(|_| {
            SourceCommandError::Invalid("assessment native identity packet byte budget")
        })?;
        if bytes > 1_048_576
            || total_bytes
                .checked_add(bytes)
                .filter(|total| *total <= MAX_NATIVE_IDENTITY_BYTES)
                .is_none()
        {
            return Err(SourceCommandError::Invalid(
                "assessment native identity packet byte budget",
            ));
        }
        total_bytes += bytes;
        paths.insert(reference.to_owned());
    }
    Ok(paths)
}

fn scan_public_native_identity_directory(
    directory: &File,
    prefix: &str,
    uid: u32,
    depth: usize,
    entries_seen: &mut usize,
    paths: &mut BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    crate::source_creation_store::active(deadline, cancelled)?;
    let before = DirectoryListingIdentity::read(directory)?;
    DirectoryIdentity::read(directory, false, uid)?;
    let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| SourceCommandError::Denied("assessment identity directory enumeration"))?;
    for entry in entries {
        crate::source_creation_store::active(deadline, cancelled)?;
        *entries_seen = entries_seen
            .checked_add(1)
            .ok_or(SourceCommandError::Invalid(
                "assessment identity directory-entry budget",
            ))?;
        if *entries_seen > MAX_NATIVE_IDENTITY_SCAN_ENTRIES {
            return Err(SourceCommandError::Invalid(
                "assessment identity directory-entry budget",
            ));
        }
        let entry =
            entry.map_err(|_| SourceCommandError::Denied("assessment identity directory entry"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| SourceCommandError::Invalid("assessment identity path encoding"))?;
        let reference = format!("{prefix}/{name}");
        if prefix == "ToS/source-witnesses" && name == "owner-local" {
            return Err(SourceCommandError::Denied(
                "reserved owner-local namespace cannot enter public native identity inventory",
            ));
        }
        if matches!(name.as_str(), "payload" | "local-content" | "catalog") {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|_| SourceCommandError::Denied("assessment identity entry type"))?;
        if file_type.is_symlink() {
            if public_native_identity_member(&reference) {
                return Err(SourceCommandError::Conflict(
                    "assessment native identity member is a symbolic link",
                ));
            }
            continue;
        }
        if file_type.is_dir() {
            if depth >= MAX_NATIVE_IDENTITY_SCAN_DEPTH {
                return Err(SourceCommandError::Invalid(
                    "assessment identity directory-depth budget",
                ));
            }
            let child = tos_fd_open::open_directory_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("assessment identity child directory"))?;
            DirectoryIdentity::read(&child, false, uid)?;
            scan_public_native_identity_directory(
                &child,
                &reference,
                uid,
                depth + 1,
                entries_seen,
                paths,
                deadline,
                cancelled,
            )?;
        } else if file_type.is_file() && public_native_identity_member(&reference) {
            RelativePath::parse(&reference)
                .map_err(|_| SourceCommandError::Invalid("assessment native identity path"))?;
            if paths.insert(reference) && paths.len() > MAX_NATIVE_IDENTITY_FILES {
                return Err(SourceCommandError::Invalid(
                    "assessment native identity packet-count budget",
                ));
            }
        }
    }
    crate::source_creation_store::active(deadline, cancelled)?;
    if DirectoryListingIdentity::read(directory)? != before
        || DirectoryIdentity::read(directory, false, uid).is_err()
    {
        return Err(SourceCommandError::Conflict(
            "assessment native identity membership changed during enumeration",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootKind {
    Public,
    Private,
}

struct RetainedOwnerFile {
    reference: String,
    root: RootKind,
    kind: NativeReadKind,
    components: Vec<String>,
    directories: Vec<File>,
    directory_identities: Vec<DirectoryIdentity>,
    file: File,
    identity: FileIdentity,
    raw: Vec<u8>,
}

struct PinnedOwnerTransport {
    public_root: File,
    private_root: File,
    public_root_path: PathBuf,
    private_root_path: PathBuf,
    public_root_identity: DirectoryIdentity,
    private_root_identity: DirectoryIdentity,
    private_prefix: String,
    uid: u32,
    retained: BTreeMap<String, RetainedOwnerFile>,
    total_metadata_bytes: usize,
    total_content_bytes: usize,
}

impl PinnedOwnerTransport {
    fn select(
        owner: &OwnerTextContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(Self, String)> {
        let before = owner.snapshot(deadline, cancelled)?.to_prefixed();
        let uid = owner.account_uid();
        let public_root = owner.public_root_handle()?;
        let private_root = owner.private_root_handle()?;
        let public_root_identity = DirectoryIdentity::read(&public_root, false, uid)?;
        let private_root_identity = DirectoryIdentity::read(&private_root, true, uid)?;
        let private_prefix = owner
            .private_identity_home()
            .strip_prefix(owner.private_root())
            .ok()
            .and_then(Path::to_str)
            .filter(|value| value.starts_with(PRIVATE_HOME))
            .map(|value| format!("{}/", value.trim_end_matches('/')))
            .ok_or(SourceCommandError::Denied(
                "assessment selected private source namespace",
            ))?;
        let transport = Self {
            public_root,
            private_root,
            public_root_path: owner.public_root().to_path_buf(),
            private_root_path: owner.private_root().to_path_buf(),
            public_root_identity,
            private_root_identity,
            private_prefix,
            uid,
            retained: BTreeMap::new(),
            total_metadata_bytes: 0,
            total_content_bytes: 0,
        };
        transport.verify_roots(owner, deadline, cancelled)?;
        if owner.snapshot(deadline, cancelled)?.to_prefixed() != before {
            return Err(SourceCommandError::Conflict(
                "assessment owner context changed during root selection",
            ));
        }
        Ok((transport, before))
    }

    fn classify(&self, reference: &str) -> SourceCommandResult<(RootKind, Vec<String>)> {
        let parsed = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Denied("assessment source logical path"))?;
        let parts = parsed
            .as_str()
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let public_identity_packet = public_native_identity_member(reference);
        if parts.len() < 3
            || parts.iter().any(|part| {
                (part.starts_with('.') && !public_identity_packet)
                    || matches!(part.as_str(), "catalog" | "payload" | "local-content")
            })
        {
            return Err(SourceCommandError::Denied(
                "assessment source metadata path",
            ));
        }
        let root =
            if reference.starts_with(&self.private_prefix) {
                let private_parts = reference.strip_prefix(&self.private_prefix).ok_or(
                    SourceCommandError::Denied("assessment private source prefix"),
                )?;
                if private_parts.is_empty() {
                    return Err(SourceCommandError::Denied("assessment private source file"));
                }
                RootKind::Private
            } else {
                if reference.starts_with(PRIVATE_HOME)
                    || !(reference.starts_with(PUBLIC_SCHEMA_HOME)
                        || reference.starts_with(PUBLIC_SOURCE_HOME)
                        || matches!(reference, PUBLIC_CLAIM_RELATIONS | PUBLIC_CLAIM_ENTITIES))
                    || parts.iter().any(|part| part == "owner-local")
                {
                    return Err(SourceCommandError::Denied(
                        "assessment source leaves selected public or private root",
                    ));
                }
                RootKind::Public
            };
        Ok((root, parts))
    }

    fn classify_content(&self, reference: &str) -> SourceCommandResult<(RootKind, Vec<String>)> {
        let parsed = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Denied("assessment content logical path"))?;
        let parts = parsed
            .as_str()
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let prefix_components = self.private_prefix.trim_end_matches('/').split('/').count();
        if !reference.starts_with(&self.private_prefix)
            || parts.len() < prefix_components + 1
            || parts
                .iter()
                .any(|part| part.starts_with('.') || part == "catalog")
        {
            return Err(SourceCommandError::Denied(
                "assessment content leaves selected private root",
            ));
        }
        Ok((RootKind::Private, parts))
    }

    fn open_selected(
        &self,
        reference: &str,
        kind: NativeReadKind,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(
        RootKind,
        Vec<String>,
        Vec<File>,
        Vec<DirectoryIdentity>,
        File,
        FileIdentity,
    )> {
        let (root_kind, components) = if kind == NativeReadKind::Content {
            self.classify_content(reference)?
        } else {
            self.classify(reference)?
        };
        let private = root_kind == RootKind::Private;
        let root = if private {
            &self.private_root
        } else {
            &self.public_root
        };
        let mut parent = tos_fd_open::reopen_directory(root)
            .map_err(|_| SourceCommandError::Denied("assessment selected root descriptor"))?;
        let mut directories = Vec::with_capacity(components.len().saturating_sub(1));
        let mut directory_identities = Vec::with_capacity(components.len().saturating_sub(1));
        for component in &components[..components.len() - 1] {
            crate::source_creation_store::active(deadline, cancelled)?;
            parent = tos_fd_open::open_directory_at(&parent, Path::new(component))
                .map_err(|_| SourceCommandError::Denied("assessment source parent descriptor"))?;
            let identity = DirectoryIdentity::read(&parent, private, self.uid)?;
            directory_identities.push(identity);
            directories.push(parent);
            parent = tos_fd_open::reopen_directory(directories.last().ok_or(
                SourceCommandError::Invalid("assessment source parent chain"),
            )?)
            .map_err(|_| SourceCommandError::Denied("assessment source parent pin"))?;
        }
        crate::source_creation_store::active(deadline, cancelled)?;
        let leaf = components
            .last()
            .ok_or(SourceCommandError::Invalid("assessment source leaf"))?;
        let mut file = tos_fd_open::open_regular_at(&parent, Path::new(leaf))
            .map_err(|_| SourceCommandError::Denied("assessment source file descriptor"))?;
        let identity = FileIdentity::read(&file, private, self.uid)?;
        if identity.length > MAX_SELECTED_FILE_BYTES as u64 {
            return Err(SourceCommandError::Invalid(
                "assessment source file exceeds aggregate budget",
            ));
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| SourceCommandError::Denied("assessment source descriptor seek"))?;
        Ok((
            root_kind,
            components,
            directories,
            directory_identities,
            file,
            identity,
        ))
    }

    /// Read via the selected OwnerTextContext first, then bind those exact
    /// bytes to an openat2-relative descriptor chain retained by this route.
    fn read(
        &mut self,
        owner: &OwnerTextContext,
        reference: &str,
        kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        crate::source_creation_store::active(deadline, cancelled)?;
        if let Some(existing) = self.retained.get(reference) {
            if (existing.kind == NativeReadKind::Content) != (kind == NativeReadKind::Content) {
                return Err(SourceCommandError::Conflict(
                    "assessment source path selected under another read kind",
                ));
            }
            if existing.raw.len() > max_bytes {
                return Err(SourceCommandError::Invalid(
                    "assessment repeated source exceeds caller cap",
                ));
            }
            let expected = existing.raw.clone();
            let owner_bytes = owner.read(reference, expected.len(), deadline, cancelled)?;
            let (_, _, _, _, _current_file, current_identity) =
                self.open_selected(reference, kind, deadline, cancelled)?;
            if owner_bytes != expected || current_identity != existing.identity {
                return Err(SourceCommandError::Conflict(
                    "assessment selected source changed between reads",
                ));
            }
            return Ok(expected);
        }
        let (selected_bytes, selected_limit) = if kind == NativeReadKind::Content {
            (self.total_content_bytes, MAX_SELECTED_CONTENT_BYTES)
        } else {
            (self.total_metadata_bytes, MAX_SELECTED_METADATA_BYTES)
        };
        let remaining =
            selected_limit
                .checked_sub(selected_bytes)
                .ok_or(SourceCommandError::Invalid(
                    "assessment source aggregate budget",
                ))?;
        let bounded = max_bytes.min(remaining);
        if bounded == 0 {
            return Err(SourceCommandError::Invalid(
                "assessment source aggregate byte budget",
            ));
        }
        let (opened, owner_bytes) = {
            let opened = self.open_selected(reference, kind, deadline, cancelled)?;
            let owner_bytes = owner.read(reference, bounded, deadline, cancelled)?;
            (opened, owner_bytes)
        };
        let (root, components, directories, directory_identities, mut file, identity) = opened;
        if self.retained.len() >= MAX_SELECTED_FILES {
            return Err(SourceCommandError::Invalid(
                "assessment source file-count budget",
            ));
        }
        let cap = bounded.min(identity.length as usize);
        let before = FileIdentity::read(&file, root == RootKind::Private, self.uid)?;
        let raw = crate::source_creation_store::raw(&mut file, cap, deadline, cancelled)?;
        let after = FileIdentity::read(&file, root == RootKind::Private, self.uid)?;
        if raw != owner_bytes || before != after || raw.len() as u64 != identity.length {
            return Err(SourceCommandError::Conflict(
                "assessment owner source bytes or descriptor identity changed",
            ));
        }
        let parent = directories.last().unwrap_or(if root == RootKind::Private {
            &self.private_root
        } else {
            &self.public_root
        });
        let current = tos_fd_open::open_regular_at(
            parent,
            Path::new(
                components
                    .last()
                    .ok_or(SourceCommandError::Invalid("assessment source leaf"))?,
            ),
        )
        .map_err(|_| SourceCommandError::Conflict("assessment source path changed"))?;
        if FileIdentity::read(&current, root == RootKind::Private, self.uid)? != identity {
            return Err(SourceCommandError::Conflict(
                "assessment source path no longer names selected file",
            ));
        }
        let total = selected_bytes
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Invalid(
                "assessment source byte overflow",
            ))?;
        if kind == NativeReadKind::Content {
            self.total_content_bytes = total;
        } else {
            self.total_metadata_bytes = total;
        }
        self.retained.insert(
            reference.to_owned(),
            RetainedOwnerFile {
                reference: reference.to_owned(),
                root,
                kind,
                components,
                directories,
                directory_identities,
                file,
                identity,
                raw: raw.clone(),
            },
        );
        Ok(raw)
    }

    fn verify_roots(
        &self,
        owner: &OwnerTextContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let _ = owner.snapshot(deadline, cancelled)?;
        for (path, expected, private) in [
            (
                self.public_root_path.as_path(),
                self.public_root_identity,
                false,
            ),
            (
                self.private_root_path.as_path(),
                self.private_root_identity,
                true,
            ),
        ] {
            crate::source_creation_store::active(deadline, cancelled)?;
            let current = tos_fd_open::open_absolute_directory(path)
                .map_err(|_| SourceCommandError::Conflict("assessment source root changed"))?;
            if DirectoryIdentity::read(&current, private, self.uid)? != expected {
                return Err(SourceCommandError::Conflict(
                    "assessment source root no longer names selected directory",
                ));
            }
        }
        if DirectoryIdentity::read(&self.public_root, false, self.uid)? != self.public_root_identity
            || DirectoryIdentity::read(&self.private_root, true, self.uid)?
                != self.private_root_identity
        {
            return Err(SourceCommandError::Conflict(
                "assessment pinned source root identity changed",
            ));
        }
        Ok(())
    }

    fn verify_files(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        for retained in self.retained.values() {
            crate::source_creation_store::active(deadline, cancelled)?;
            let private = retained.root == RootKind::Private;
            let root = if private {
                &self.private_root
            } else {
                &self.public_root
            };
            let mut parent = tos_fd_open::reopen_directory(root)
                .map_err(|_| SourceCommandError::Conflict("assessment retained root changed"))?;
            for (index, component) in retained.components[..retained.components.len() - 1]
                .iter()
                .enumerate()
            {
                crate::source_creation_store::active(deadline, cancelled)?;
                let current = tos_fd_open::open_directory_at(&parent, Path::new(component))
                    .map_err(|_| {
                        SourceCommandError::Conflict("assessment source parent changed")
                    })?;
                let identity = DirectoryIdentity::read(&current, private, self.uid)?;
                let held_identity = DirectoryIdentity::read(
                    retained
                        .directories
                        .get(index)
                        .ok_or(SourceCommandError::Invalid(
                            "assessment retained parent chain",
                        ))?,
                    private,
                    self.uid,
                )?;
                if identity != retained.directory_identities[index] || identity != held_identity {
                    return Err(SourceCommandError::Conflict(
                        "assessment source parent identity changed",
                    ));
                }
                parent = current;
            }
            let leaf = retained
                .components
                .last()
                .ok_or(SourceCommandError::Invalid("assessment retained file leaf"))?;
            let current = tos_fd_open::open_regular_at(&parent, Path::new(leaf))
                .map_err(|_| SourceCommandError::Conflict("assessment source file changed"))?;
            if FileIdentity::read(&current, private, self.uid)? != retained.identity
                || FileIdentity::read(&retained.file, private, self.uid)? != retained.identity
            {
                return Err(SourceCommandError::Conflict(
                    "assessment source file identity changed",
                ));
            }
            let mut held = retained
                .file
                .try_clone()
                .map_err(|_| SourceCommandError::Conflict("assessment retained file descriptor"))?;
            held.seek(SeekFrom::Start(0))
                .map_err(|_| SourceCommandError::Conflict("assessment retained source seek"))?;
            let bytes = crate::source_creation_store::raw(
                &mut held,
                retained.raw.len(),
                deadline,
                cancelled,
            )?;
            if bytes != retained.raw
                || FileIdentity::read(&retained.file, private, self.uid)? != retained.identity
                || FileIdentity::read(&current, private, self.uid)? != retained.identity
            {
                return Err(SourceCommandError::Conflict(
                    "assessment retained source bytes changed",
                ));
            }
            let _ = &retained.reference;
        }
        Ok(())
    }

    fn enumerate_public_native_identity_paths(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<BTreeSet<String>> {
        crate::source_creation_store::active(deadline, cancelled)?;
        let tos = tos_fd_open::open_directory_at(&self.public_root, Path::new("ToS"))
            .map_err(|_| SourceCommandError::Conflict("assessment public source tree changed"))?;
        DirectoryIdentity::read(&tos, false, self.uid)?;
        let witnesses = tos_fd_open::open_directory_at(&tos, Path::new("source-witnesses"))
            .map_err(|_| SourceCommandError::Conflict("assessment public source tree changed"))?;
        DirectoryIdentity::read(&witnesses, false, self.uid)?;
        let mut entries_seen = 0usize;
        let mut paths = BTreeSet::new();
        scan_public_native_identity_directory(
            &witnesses,
            "ToS/source-witnesses",
            self.uid,
            0,
            &mut entries_seen,
            &mut paths,
            deadline,
            cancelled,
        )?;
        Ok(paths)
    }

    fn verify_public_native_identity_inventory(
        &self,
        ctx: &CommandContext,
        cut: &CorpusCutReader,
        expected: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let cut_paths = cut_native_identity_paths(ctx, cut)?;
        if &cut_paths != expected {
            return Err(SourceCommandError::Conflict(
                "assessment native identity inventory differs from selected cut",
            ));
        }
        let first = self.enumerate_public_native_identity_paths(deadline, cancelled)?;
        let second = self.enumerate_public_native_identity_paths(deadline, cancelled)?;
        if first != second || &second != expected {
            return Err(SourceCommandError::Conflict(
                "assessment native identity inventory membership changed",
            ));
        }
        for reference in expected {
            let retained = self
                .retained
                .get(reference)
                .ok_or(SourceCommandError::Conflict(
                    "assessment native identity member lacks retained source bytes",
                ))?;
            let logical = RelativePath::parse(reference)
                .map_err(|_| SourceCommandError::Conflict("assessment identity source path"))?;
            let member = cut
                .current()
                .member(&logical)
                .ok_or(SourceCommandError::Conflict(
                    "assessment native identity member is absent from selected cut",
                ))?;
            if retained.root != RootKind::Public
                || retained.kind == NativeReadKind::Content
                || retained.raw.len() as u64 != member.size_bytes
                || Digest256::of_bytes(&retained.raw) != member.sha256
            {
                return Err(SourceCommandError::Conflict(
                    "assessment native identity member differs from selected cut metadata",
                ));
            }
        }
        Ok(())
    }
}

struct PinnedOwnerReader<'a> {
    owner: &'a OwnerTextContext,
    ctx: &'a CommandContext,
    cut: &'a CorpusCutReader,
    transport: &'a mut PinnedOwnerTransport,
    allow_content: bool,
}

fn verify_public_selected_bytes(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    reference: &str,
    raw: &[u8],
) -> SourceCommandResult<()> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "assessment public source cut differs from command context",
        ));
    }
    let path = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Invalid("assessment public source path"))?;
    let member = cut
        .current()
        .member(&path)
        .ok_or(SourceCommandError::Conflict(
            "assessment public source is outside the selected cut",
        ))?;
    // `ctx.files` is the selected command-input closure, not the complete
    // corpus member set. The selected cut revision and member digest bind
    // these owner-read public bytes without requiring ToS records to be
    // duplicated as direct command inputs.
    if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(raw) != member.sha256 {
        return Err(SourceCommandError::Conflict(
            "assessment public source differs from selected cut",
        ));
    }
    Ok(())
}

impl SignNativeRead for PinnedOwnerReader<'_> {
    fn read(
        &mut self,
        reference: &str,
        kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if kind == NativeReadKind::Content && !self.allow_content {
            return Err(SourceCommandError::Denied(
                "v4 assessment source-profile metadata cannot disclose text content",
            ));
        }
        let raw = self
            .transport
            .read(self.owner, reference, kind, max_bytes, deadline, cancelled)?;
        if !reference.starts_with(&self.transport.private_prefix) {
            verify_public_selected_bytes(self.ctx, self.cut, reference, &raw)?;
        }
        Ok(raw)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if self.cut.current().revision() != self.ctx.base_revision {
            return Err(SourceCommandError::Conflict(
                "assessment native source cut differs from command context",
            ));
        }
        for (reference, retained) in &self.transport.retained {
            if retained.root == RootKind::Public {
                verify_public_selected_bytes(self.ctx, self.cut, reference, &retained.raw)?;
            }
        }
        let _ = self.owner.snapshot(deadline, cancelled)?;
        self.transport
            .verify_roots(self.owner, deadline, cancelled)?;
        self.transport.verify_files(deadline, cancelled)?;
        self.transport.verify_roots(self.owner, deadline, cancelled)
    }

    fn owner_local(&self, reference: &str) -> SourceCommandResult<bool> {
        Ok(reference.starts_with(&self.transport.private_prefix))
    }

    fn owner_context_snapshot(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<String>> {
        Ok(Some(
            self.owner.snapshot(deadline, cancelled)?.to_prefixed(),
        ))
    }
}

#[derive(Clone)]
struct SourceSelection {
    path: String,
    record_id: String,
    profile_type_id: String,
    origin_id: String,
    read_scope: String,
    source_access: JsonValue,
    source_binding: Option<JsonValue>,
    form_ids: Vec<String>,
}

#[derive(Clone)]
struct NativeTextSelection {
    binding: JsonValue,
    origin_id: String,
    read_scope: String,
}

/// Exact retained state from the v4 private source-profile route. The raw
/// source/form/native bytes and file descriptors remain private to this
/// adapter and can only be inspected by the journal owner through this module.
pub(crate) struct PrivateAssessmentSources {
    pub(crate) records: Vec<AssessmentRecordInput>,
    pub(crate) native_records: Vec<AssessmentRecordInput>,
    pub(crate) native_inputs: Vec<NativeInput>,
    pub(crate) native_snapshots: Vec<String>,
    pub(crate) native_summaries: Vec<JsonValue>,
    pub(crate) profile_snapshots: Vec<String>,
    pub(crate) owner_local_sources_snapshot: String,
    pub(crate) required_source_refs: BTreeMap<String, Vec<JsonValue>>,
    pub(crate) required_languages: BTreeMap<String, Vec<String>>,
    pub(crate) claim_snapshots: Vec<String>,
    pub(crate) form_sets: BTreeMap<String, JsonValue>,
    pub(crate) form_paths: BTreeMap<String, String>,
    pub(crate) source_paths: BTreeMap<String, String>,
    pub(crate) snapshot: String,
    pub(crate) selection_rows: JsonValue,
    pub(crate) retained_raw: BTreeMap<String, Vec<u8>>,
    context_snapshot: String,
    selected_revision: SourceRevision,
    source_dependencies: BTreeMap<String, String>,
    public_native_identity_paths: Option<BTreeSet<String>>,
    transport: PinnedOwnerTransport,
}

impl PrivateAssessmentSources {
    pub(crate) fn required_source_refs(&self) -> &BTreeMap<String, Vec<JsonValue>> {
        &self.required_source_refs
    }

    pub(crate) fn required_languages(&self) -> &BTreeMap<String, Vec<String>> {
        &self.required_languages
    }

    pub(crate) fn form_sets(&self) -> &BTreeMap<String, JsonValue> {
        &self.form_sets
    }

    pub(crate) fn form_paths(&self) -> &BTreeMap<String, String> {
        &self.form_paths
    }

    /// Total raw input bytes selected by this adapter. Every Claim helper and
    /// native resolver input is rebound to the same retained path map before
    /// this value is exposed, so shared paths are charged only once.
    pub(crate) fn input_bytes(&self) -> SourceCommandResult<usize> {
        self.retained_raw.values().try_fold(0usize, |total, raw| {
            total
                .checked_add(raw.len())
                .ok_or(SourceCommandError::Invalid(
                    "private assessment selected input byte total overflow",
                ))
        })
    }

    /// Recheck every exact source, form, and native metadata byte through its
    /// retained root and file descriptors after the journal has acquired its
    /// locks. Replacement, in-place edits, or parent-chain changes conflict.
    pub(crate) fn verify_current(
        &self,
        owner: &OwnerTextContext,
        ctx: &CommandContext,
        cut: &CorpusCutReader,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if ctx.base_revision != self.selected_revision
            || cut.current().revision() != self.selected_revision
        {
            return Err(SourceCommandError::Conflict(
                "assessment selected source cut changed after source selection",
            ));
        }
        if owner.snapshot(deadline, cancelled)?.to_prefixed() != self.context_snapshot {
            return Err(SourceCommandError::Conflict(
                "assessment owner context changed after source selection",
            ));
        }
        self.transport.verify_roots(owner, deadline, cancelled)?;
        self.transport.verify_files(deadline, cancelled)?;
        if let Some(paths) = &self.public_native_identity_paths {
            self.transport
                .verify_public_native_identity_inventory(ctx, cut, paths, deadline, cancelled)?;
        }
        self.transport.verify_roots(owner, deadline, cancelled)?;
        if owner.snapshot(deadline, cancelled)?.to_prefixed() != self.context_snapshot {
            return Err(SourceCommandError::Conflict(
                "assessment owner context changed during source verification",
            ));
        }
        let current = source_snapshot(
            &self.context_snapshot,
            &self.selection_rows,
            &self.transport.retained,
            &self.source_dependencies,
            &self.native_inputs,
            &self.native_snapshots,
            &self.claim_snapshots,
        )?;
        if current != self.snapshot {
            return Err(SourceCommandError::Conflict(
                "assessment private source snapshot changed",
            ));
        }
        let maintained = owner_local_sources_snapshot(
            owner,
            &self.context_snapshot,
            &self.profile_snapshots,
            &self.claim_snapshots,
            &self.transport.retained,
        )?;
        if maintained != self.owner_local_sources_snapshot {
            return Err(SourceCommandError::Conflict(
                "maintained owner-local source snapshot changed",
            ));
        }
        Ok(())
    }
}

fn owner_local_sources_snapshot(
    owner: &OwnerTextContext,
    context_snapshot: &str,
    profile_snapshots: &[String],
    claim_snapshots: &[String],
    retained: &BTreeMap<String, RetainedOwnerFile>,
) -> SourceCommandResult<String> {
    let readers = profile_snapshots
        .iter()
        .chain(claim_snapshots.iter())
        .map(|value| cmd::string(value))
        .collect();
    let mut sources = BTreeMap::<String, String>::new();
    for (reference, entry) in retained {
        let root = match entry.root {
            RootKind::Private => owner.private_root(),
            RootKind::Public => owner.public_root(),
        };
        let logical = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("assessment selected source path"))?;
        let path = root
            .join(logical.as_str())
            .into_os_string()
            .into_string()
            .map_err(|_| SourceCommandError::Invalid("assessment source snapshot path UTF-8"))?;
        let digest = Digest256::of_bytes(&entry.raw).to_hex();
        if sources
            .insert(path, digest.clone())
            .is_some_and(|previous| previous != digest)
        {
            return Err(SourceCommandError::Conflict(
                "assessment source path resolves to conflicting retained bytes",
            ));
        }
    }
    let source_rows = JsonValue::Object(
        sources
            .into_iter()
            .map(|(path, digest)| (JsonString::from_utf8(&path), cmd::string(&digest)))
            .collect(),
    );
    let basis = cmd::object(vec![
        ("context", cmd::string(context_snapshot)),
        ("readers", JsonValue::Array(readers)),
        ("sources", source_rows),
    ]);
    Ok(cmd::record_digest(&basis)?.to_prefixed())
}

fn owner_profile_reader_snapshot(
    context_snapshot: &str,
    source_access: &JsonValue,
    source_binding: Option<&JsonValue>,
    contracts: &BTreeMap<String, String>,
    source_path: &str,
    source_raw: &[u8],
    native_metadata: Option<&str>,
) -> SourceCommandResult<String> {
    let binding = source_binding
        .map(|value| {
            cmd::canonical(value).map(|raw| cmd::string(&Digest256::of_bytes(&raw).to_hex()))
        })
        .transpose()?
        .unwrap_or(JsonValue::Null);
    let contract_rows = JsonValue::Object(
        contracts
            .iter()
            .map(|(path, digest)| (JsonString::from_utf8(path), cmd::string(digest)))
            .collect(),
    );
    let source_rows = cmd::object(vec![(
        source_path,
        cmd::string(&Digest256::of_bytes(source_raw).to_hex()),
    )]);
    let native = cmd::object(vec![
        (
            "metadata",
            native_metadata.map(cmd::string).unwrap_or(JsonValue::Null),
        ),
        ("exact", JsonValue::Null),
    ]);
    let basis = cmd::object(vec![
        ("context", cmd::string(context_snapshot)),
        ("source_access", source_access.clone()),
        ("source_binding", binding),
        ("contracts", contract_rows),
        ("sources", source_rows),
        ("native", native),
    ]);
    Ok(cmd::record_digest(&basis)?.to_prefixed())
}

fn source_snapshot(
    context_snapshot: &str,
    selection_rows: &JsonValue,
    retained: &BTreeMap<String, RetainedOwnerFile>,
    source_dependencies: &BTreeMap<String, String>,
    native_inputs: &[NativeInput],
    native_snapshots: &[String],
    claim_snapshots: &[String],
) -> SourceCommandResult<String> {
    let source_files = JsonValue::Object(
        retained
            .iter()
            .map(|(reference, entry)| {
                (
                    JsonString::from_utf8(reference),
                    cmd::object(vec![
                        (
                            "sha256",
                            cmd::string(&Digest256::of_bytes(&entry.raw).to_prefixed()),
                        ),
                        ("bytes", cmd::number(entry.raw.len() as u64)),
                        (
                            "root",
                            cmd::string(if entry.root == RootKind::Private {
                                "owner-local"
                            } else {
                                "selected-public-root"
                            }),
                        ),
                        (
                            "kind",
                            cmd::string(if entry.kind == NativeReadKind::Content {
                                "content"
                            } else {
                                "metadata"
                            }),
                        ),
                    ]),
                )
            })
            .collect(),
    );
    let native = native_inputs
        .iter()
        .map(|input| {
            let kind = match input.kind {
                NativeReadKind::Metadata => "metadata",
                NativeReadKind::Schema => "schema",
                NativeReadKind::Support => "support",
                NativeReadKind::Content => "content",
            };
            JsonValue::Array(vec![
                cmd::string(&input.reference),
                cmd::string(kind),
                cmd::string(input.category),
                cmd::string(&input.raw_sha256.to_prefixed()),
                cmd::number(input.raw_size as u64),
            ])
        })
        .collect::<Vec<_>>();
    let dependencies = JsonValue::Object(
        source_dependencies
            .iter()
            .map(|(reference, digest)| (JsonString::from_utf8(reference), cmd::string(digest)))
            .collect(),
    );
    let basis = cmd::object(vec![
        ("context", cmd::string(context_snapshot)),
        ("selection_rows", selection_rows.clone()),
        ("source_files", source_files),
        ("source_dependencies", dependencies),
        ("native_inputs", JsonValue::Array(native)),
        (
            "native_snapshots",
            JsonValue::Array(
                native_snapshots
                    .iter()
                    .map(|value| cmd::string(value))
                    .collect(),
            ),
        ),
        (
            "claim_snapshots",
            JsonValue::Array(
                claim_snapshots
                    .iter()
                    .map(|value| cmd::string(value))
                    .collect(),
            ),
        ),
    ]);
    Ok(cmd::record_digest(&basis)?.to_prefixed())
}

fn selection_access(value: &JsonValue) -> SourceCommandResult<String> {
    cmd::exact_keys(value, &["read_scope", "access_allowed", "authority_ref"])?;
    let scope = cmd::text(value, "read_scope")?;
    if !matches!(scope, "metadata_only" | "exact_owner_local")
        || cmd::field(value, "access_allowed")? != &JsonValue::Bool(true)
        || !cmd::nonblank(cmd::text(value, "authority_ref")?)
    {
        return Err(SourceCommandError::Denied(
            "private assessment requires explicit source access",
        ));
    }
    Ok(scope.to_owned())
}

fn selected_private_path(
    transport: &PinnedOwnerTransport,
    reference: &str,
    form_set: bool,
) -> SourceCommandResult<()> {
    let (root, _) = transport.classify(reference)?;
    if root != RootKind::Private {
        return Err(SourceCommandError::Denied(
            "private assessment source must stay in the selected private root",
        ));
    }
    let prefix_components = transport
        .private_prefix
        .trim_end_matches('/')
        .split('/')
        .count();
    let path = RelativePath::parse(reference)
        .map_err(|_| SourceCommandError::Denied("private assessment source path"))?;
    let components = path.as_str().split('/').collect::<Vec<_>>();
    if components.len() < prefix_components + 2
        || !reference.ends_with(if form_set {
            ".human-forms.json"
        } else {
            ".json"
        })
        || !form_set && reference.ends_with(".human-forms.json")
    {
        return Err(SourceCommandError::Denied(
            "private assessment source leaves its typed metadata package",
        ));
    }
    Ok(())
}

fn parse_selections(
    transport: &PinnedOwnerTransport,
    value: &JsonValue,
) -> SourceCommandResult<Vec<SourceSelection>> {
    let rows = value.as_array().ok_or(SourceCommandError::Invalid(
        "private assessment selection array",
    ))?;
    if rows.len() > MAX_SELECTIONS {
        return Err(SourceCommandError::Invalid(
            "private assessment selection count budget",
        ));
    }
    let mut selected_paths = BTreeSet::new();
    let mut selected_ids = BTreeSet::new();
    let mut selections = Vec::with_capacity(rows.len());
    for row in rows {
        cmd::exact_keys(
            row,
            &[
                "path",
                "record_id",
                "profile_type_id",
                "origin_id",
                "source_access",
                "source_binding",
                "form_ids",
            ],
        )?;
        let path = cmd::text(row, "path")?.to_owned();
        selected_private_path(transport, &path, false)?;
        if !selected_paths.insert(path.clone()) {
            return Err(SourceCommandError::Invalid(
                "private assessment repeats a source path",
            ));
        }
        let record_id = cmd::text(row, "record_id")?.to_owned();
        let profile_type_id = cmd::text(row, "profile_type_id")?.to_owned();
        let origin_id = cmd::text(row, "origin_id")?.to_owned();
        if record_id.is_empty()
            || profile_type_id.is_empty()
            || !cmd::nonblank(&origin_id)
            || !selected_ids.insert(record_id.clone())
        {
            return Err(SourceCommandError::Invalid(
                "private assessment selected record identity",
            ));
        }
        let source_access = cmd::field(row, "source_access")?.clone();
        let read_scope = selection_access(&source_access)?;
        let binding = cmd::field(row, "source_binding")?;
        let source_binding = match binding {
            JsonValue::Null => None,
            JsonValue::Object(_) => Some(binding.clone()),
            _ => {
                return Err(SourceCommandError::Invalid(
                    "private assessment native binding selection",
                ));
            }
        };
        let forms = cmd::array(row, "form_ids")?;
        if forms.len() > MAX_FORMS_PER_SOURCE {
            return Err(SourceCommandError::Invalid(
                "private assessment form selection budget",
            ));
        }
        let mut form_ids = Vec::with_capacity(forms.len());
        let mut local_forms = BTreeSet::new();
        for form in forms {
            let identifier = form
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or(SourceCommandError::Invalid(
                    "private assessment selected form identity",
                ))?
                .to_owned();
            if !local_forms.insert(identifier.clone()) || !selected_ids.insert(identifier.clone()) {
                return Err(SourceCommandError::Invalid(
                    "private assessment repeats a source or form identity",
                ));
            }
            form_ids.push(identifier);
        }
        if !form_ids.is_empty() {
            let form_path = format!("{}.human-forms.json", path.trim_end_matches(".json"));
            selected_private_path(transport, &form_path, true)?;
            if !selected_paths.insert(form_path) {
                return Err(SourceCommandError::Invalid(
                    "private assessment repeats a source or form-set path",
                ));
            }
        }
        selections.push(SourceSelection {
            path,
            record_id,
            profile_type_id,
            origin_id,
            read_scope,
            source_access,
            source_binding,
            form_ids,
        });
    }
    if selected_ids.len() > MAX_ASSESSMENTS {
        return Err(SourceCommandError::Invalid(
            "private assessment source identity budget",
        ));
    }
    Ok(selections)
}

fn parse_native_text_units(value: &JsonValue) -> SourceCommandResult<Vec<NativeTextSelection>> {
    let rows = value.as_array().ok_or(SourceCommandError::Invalid(
        "private assessment native unit array",
    ))?;
    if rows.len() > MAX_SELECTIONS {
        return Err(SourceCommandError::Invalid(
            "private assessment native unit selection budget",
        ));
    }
    let mut unit_ids = BTreeSet::new();
    let mut selections = Vec::with_capacity(rows.len());
    for row in rows {
        cmd::exact_keys(row, &["binding", "origin_id", "source_access"])?;
        let binding = cmd::field(row, "binding")?;
        if !matches!(binding, JsonValue::Object(_)) {
            return Err(SourceCommandError::Invalid(
                "private assessment native unit binding",
            ));
        }
        let unit_id = cmd::text(binding, "unit_id")?;
        if !cmd::nonblank(unit_id)
            || cmd::integer(binding, "unit_version")? == 0
            || !unit_ids.insert(unit_id.to_owned())
        {
            return Err(SourceCommandError::Invalid(
                "private assessment native unit identity",
            ));
        }
        let origin_id = cmd::text(row, "origin_id")?.to_owned();
        if !cmd::nonblank(&origin_id) {
            return Err(SourceCommandError::Invalid(
                "private assessment native unit origin",
            ));
        }
        let read_scope = selection_access(cmd::field(row, "source_access")?)?;
        selections.push(NativeTextSelection {
            binding: binding.clone(),
            origin_id,
            read_scope,
        });
    }
    Ok(selections)
}

fn context_selection_matches(
    owner: &OwnerTextContext,
    selected_context: &JsonValue,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        selected_context,
        &[
            "schema_version",
            "store_id",
            "public_root",
            "private_root",
            "private_prefix",
        ],
    )?;
    let prefix = owner
        .private_identity_home()
        .strip_prefix(owner.private_root())
        .ok()
        .and_then(Path::to_str)
        .map(|value| format!("{}/", value.trim_end_matches('/')))
        .ok_or(SourceCommandError::Denied(
            "private assessment selected context prefix",
        ))?;
    let selected_store_id = cmd::text(selected_context, "store_id")?;
    let prefix_store_id =
        prefix
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .ok_or(SourceCommandError::Denied(
                "private assessment selected store identity",
            ))?;
    if cmd::text(selected_context, "schema_version")? != "tos_owner_local_source_context_v1"
        || cmd::text(selected_context, "public_root")? != owner.public_root().to_str().unwrap_or("")
        || cmd::text(selected_context, "private_root")?
            != owner.private_root().to_str().unwrap_or("")
        || cmd::text(selected_context, "private_prefix")? != prefix
        || selected_store_id != prefix_store_id
    {
        return Err(SourceCommandError::Conflict(
            "private assessment owner context selection differs",
        ));
    }
    Ok(())
}

fn envelope(
    id: &str,
    version: &JsonValue,
    payload: &JsonValue,
    origin_id: &str,
) -> SourceCommandResult<AssessmentRecordInput> {
    if !cmd::nonblank(id) || !cmd::nonblank(origin_id) {
        return Err(SourceCommandError::Invalid(
            "private assessment source record identity",
        ));
    }
    let version_number =
        version
            .as_u64()
            .filter(|value| *value > 0)
            .ok_or(SourceCommandError::Invalid(
                "private assessment source record version",
            ))?;
    let payload_raw = cmd::canonical(payload)?;
    if payload_raw.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid(
            "private assessment source record byte budget",
        ));
    }
    let value = cmd::object(vec![
        ("id", cmd::string(id)),
        ("version", cmd::number(version_number)),
        ("payload", payload.clone()),
        ("origin_id", cmd::string(origin_id)),
    ]);
    Ok(AssessmentRecordInput {
        envelope: cmd::canonical(&value)?,
    })
}

fn record_input(record: &JsonValue) -> SourceCommandResult<AssessmentRecordInput> {
    cmd::exact_keys(record, &["id", "version", "payload", "origin_id"])?;
    let id = cmd::text(record, "id")?;
    let origin_id = cmd::text(record, "origin_id")?;
    if !cmd::nonblank(id) || !cmd::nonblank(origin_id) {
        return Err(SourceCommandError::Invalid(
            "private assessment selected record identity",
        ));
    }
    let version = cmd::integer(record, "version")?;
    let payload_raw = cmd::canonical(cmd::field(record, "payload")?)?;
    if version == 0 || payload_raw.len() > MAX_RECORD_BYTES {
        return Err(SourceCommandError::Invalid(
            "private assessment selected record budget",
        ));
    }
    Ok(AssessmentRecordInput {
        envelope: cmd::canonical(record)?,
    })
}

fn record_reference(record: &JsonValue) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        ("id", cmd::field(record, "id")?.clone()),
        ("version", cmd::field(record, "version")?.clone()),
        (
            "digest",
            cmd::string(&cmd::record_digest(cmd::field(record, "payload")?)?.to_prefixed()),
        ),
    ]))
}

fn source_languages(payload: &JsonValue) -> SourceCommandResult<Vec<String>> {
    let mut languages = BTreeSet::new();
    let mut add = |value: Option<&str>| {
        if let Some(value) = value.filter(|value| cmd::nonblank(value)) {
            languages.insert(value.to_lowercase());
        }
    };
    if let Some(fields) = payload.object_get("field_languages") {
        for (_, field) in fields.as_object().ok_or(SourceCommandError::Invalid(
            "private source field languages",
        ))? {
            add(field.object_get("language").and_then(JsonValue::as_str));
        }
    }
    for field in ["semantic_scope", "semantic_content", "form_identity"] {
        add(payload
            .object_get(field)
            .and_then(|value| value.object_get("language"))
            .and_then(JsonValue::as_str));
    }
    Ok(languages.into_iter().collect())
}

fn retain_native_resolution(
    resolved: ResolvedSignNative,
    allow_content: bool,
    origin_id: &str,
    read_scope: &str,
    supporting_only: bool,
    transport: &PinnedOwnerTransport,
    native_inputs: &mut BTreeMap<(String, String, &'static str), NativeInput>,
    native_snapshots: &mut Vec<String>,
    native_summaries: &mut Vec<JsonValue>,
    native_records: &mut Vec<AssessmentRecordInput>,
    source_dependencies: &mut BTreeMap<String, String>,
) -> SourceCommandResult<bool> {
    let content_verified = cmd::field(&resolved.summary, "content_verified")?;
    let content_verified = match content_verified {
        JsonValue::Bool(value) => *value,
        _ => {
            return Err(SourceCommandError::Invalid(
                "private assessment native content observation",
            ));
        }
    };
    let mut observed_content = false;
    for input in &resolved.inputs {
        if input.kind == NativeReadKind::Content {
            if !allow_content {
                return Err(SourceCommandError::Denied(
                    "metadata-only native assessment selected content bytes",
                ));
            }
            observed_content = true;
        }
        let retained =
            transport
                .retained
                .get(&input.reference)
                .ok_or(SourceCommandError::Invalid(
                    "native assessment resolver omitted retained input bytes",
                ))?;
        if (retained.kind == NativeReadKind::Content) != (input.kind == NativeReadKind::Content)
            || retained.raw.len() != input.raw_size
            || Digest256::of_bytes(&retained.raw) != input.raw_sha256
        {
            return Err(SourceCommandError::Conflict(
                "native assessment resolver input differs from retained bytes",
            ));
        }
        if input.kind == NativeReadKind::Content && retained.root != RootKind::Private {
            return Err(SourceCommandError::Denied(
                "native assessment content is outside selected private root",
            ));
        }
        let kind = match input.kind {
            NativeReadKind::Metadata => "metadata",
            NativeReadKind::Schema => "schema",
            NativeReadKind::Support => "support",
            NativeReadKind::Content => "content",
        }
        .to_owned();
        let key = (input.reference.clone(), kind, input.category);
        if native_inputs
            .insert(key, input.clone())
            .is_some_and(|previous| previous.raw_sha256 != input.raw_sha256)
        {
            return Err(SourceCommandError::Conflict(
                "native assessment source input changed",
            ));
        }
    }
    if content_verified && !observed_content {
        return Err(SourceCommandError::Conflict(
            "native assessment reports content without a retained content read",
        ));
    }
    for (reference, digest) in resolved.schema_digests {
        let digest = digest.to_prefixed();
        if source_dependencies
            .insert(reference, digest.clone())
            .is_some_and(|previous| previous != digest)
        {
            return Err(SourceCommandError::Conflict(
                "native assessment schema dependency changed",
            ));
        }
    }
    native_snapshots.push(resolved.input_snapshot);
    let record_refs = resolved
        .records
        .iter()
        .map(|record| {
            Ok(cmd::object(vec![
                ("id", cmd::field(record, "id")?.clone()),
                ("version", cmd::field(record, "version")?.clone()),
                (
                    "digest",
                    cmd::string(&cmd::record_digest(cmd::field(record, "payload")?)?.to_prefixed()),
                ),
            ]))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let mut summary = resolved.summary.clone();
    cmd::set(&mut summary, "origin_id", cmd::string(origin_id))?;
    cmd::set(&mut summary, "read_scope", cmd::string(read_scope))?;
    cmd::set(&mut summary, "record_refs", JsonValue::Array(record_refs))?;
    cmd::set(
        &mut summary,
        "supporting_only",
        JsonValue::Bool(supporting_only),
    )?;
    native_summaries.push(summary);
    for record in resolved.records {
        cmd::exact_keys(&record, &["id", "version", "payload", "origin_id"])?;
        if cmd::canonical(cmd::field(&record, "payload")?)?.len() > MAX_RECORD_BYTES {
            return Err(SourceCommandError::Invalid(
                "private assessment native record byte budget",
            ));
        }
        native_records.push(AssessmentRecordInput {
            envelope: cmd::canonical(&record)?,
        });
    }
    Ok(content_verified)
}

/// Select exact owner-local semantic profile records, configured current
/// adjacent forms, and explicitly selected native TextUnit assessment inputs.
/// Every source-access object is preflighted before the first source read.
/// Claim selections use the maintained private Claim profile resolver and are
/// retained with the same pinned/current source transport as the other rows.
pub(crate) fn select_owner_local_sources(
    owner: &OwnerTextContext,
    selected_context: &JsonValue,
    selections_value: &JsonValue,
    native_text_units_value: &JsonValue,
    owner_local_source_claims: Option<&JsonValue>,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PrivateAssessmentSources> {
    if cut.current().revision() != ctx.base_revision
        || worker.source_revision() != ctx.base_revision
    {
        return Err(SourceCommandError::Conflict(
            "private assessment profile worker and selected source cut differ",
        ));
    }
    context_selection_matches(owner, selected_context)?;
    let (mut transport, context_snapshot) =
        PinnedOwnerTransport::select(owner, deadline, cancelled)?;
    // These exact passes precede source/profile reads, including native
    // metadata and any exact-owner-local content lookup.
    let selections = parse_selections(&transport, selections_value)?;
    let native_selections = parse_native_text_units(native_text_units_value)?;
    let claim_selection_value = owner_local_source_claims
        .cloned()
        .unwrap_or(JsonValue::Array(Vec::new()));
    let claim_selections =
        crate::source_native_cli::private_owner::claim::preflight_assessment_claim_selections(
            &claim_selection_value,
            selected_context,
        )?;
    let mut selected_subject_ids = selections
        .iter()
        .flat_map(|selection| {
            std::iter::once(selection.record_id.clone()).chain(selection.form_ids.iter().cloned())
        })
        .collect::<BTreeSet<_>>();
    for selection in &claim_selections {
        if !selected_subject_ids.insert(selection.claim_id.clone())
            || selection
                .form_ids
                .iter()
                .any(|identifier| !selected_subject_ids.insert(identifier.clone()))
        {
            return Err(SourceCommandError::Invalid(
                "private assessment source and Claim identities overlap",
            ));
        }
        let (root, _) = transport.classify(&selection.path)?;
        if root != RootKind::Private || !selection.path.ends_with("/source-claims.jsonl") {
            return Err(SourceCommandError::Denied(
                "private assessment Claim stream must stay in the selected private root",
            ));
        }
        for selector in &selection.source_records {
            let path = cmd::text(selector, "path")?;
            let _ = transport.classify(path)?;
            if !path.starts_with(PUBLIC_SOURCE_HOME) && !path.starts_with(&transport.private_prefix)
            {
                return Err(SourceCommandError::Denied(
                    "private Claim endpoint leaves selected public or private metadata roots",
                ));
            }
            if !path.starts_with(&transport.private_prefix) {
                let current_path = RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Denied("private Claim endpoint path"))?;
                if cut.current().member(&current_path).is_none() {
                    return Err(SourceCommandError::Conflict(
                        "private Claim public endpoint is outside the current source cut",
                    ));
                }
            }
        }
        if !selection.form_ids.is_empty() {
            let parent = selection
                .path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .ok_or(SourceCommandError::Invalid("private Claim form path"))?;
            let path = format!(
                "{parent}/source-claims.{}.human-forms.json",
                Digest256::of_bytes(selection.claim_id.as_bytes()).to_hex()
            );
            selected_private_path(&transport, &path, true)?;
        }
    }
    for selection in &native_selections {
        let unit_id = cmd::text(&selection.binding, "unit_id")?.to_owned();
        if !selected_subject_ids.insert(unit_id) {
            return Err(SourceCommandError::Invalid(
                "private assessment native unit identity overlaps a selected source subject",
            ));
        }
    }
    let selection_rows = cmd::object(vec![
        ("owner_local_source_records", selections_value.clone()),
        ("native_text_units", native_text_units_value.clone()),
        ("owner_local_source_claims", claim_selection_value.clone()),
    ]);
    if cmd::canonical(&selection_rows)?.len() > MAX_SELECTION_BYTES {
        return Err(SourceCommandError::Invalid(
            "private assessment selected source rows byte budget",
        ));
    }
    // The maintained owner-local source reader observes its fixed assessment
    // and form grammar through the confidential read callback. Pin the same
    // exact public-root bytes here, even though the cut-backed worker has
    // already selected them for validation.
    for reference in OWNER_ASSESSMENT_FORM_CONTRACTS {
        crate::source_creation_store::active(deadline, cancelled)?;
        let logical = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("assessment form contract path"))?;
        let selected = ctx.file(&logical)?.ok_or(SourceCommandError::Unsupported(
            "selected assessment or form contract is absent",
        ))?;
        let raw = transport.read(
            owner,
            reference,
            NativeReadKind::Schema,
            MAX_SELECTED_FILE_BYTES,
            deadline,
            cancelled,
        )?;
        if raw.as_slice() != selected
            || worker.contract_digest(reference) != Some(Digest256::of_bytes(&raw))
        {
            return Err(SourceCommandError::Conflict(
                "assessment owner grammar differs from the selected source cut",
            ));
        }
    }
    let mut source_dependencies = BTreeMap::new();
    let mut native_inputs = BTreeMap::<(String, String, &'static str), NativeInput>::new();
    let mut native_snapshots = Vec::new();
    let mut native_summaries = Vec::new();
    let mut profile_snapshots = Vec::new();
    let mut records = Vec::new();
    let mut native_records = Vec::new();
    let mut required_source_refs = BTreeMap::<String, Vec<JsonValue>>::new();
    let mut required_languages = BTreeMap::<String, Vec<String>>::new();
    let mut claim_snapshots = Vec::new();
    let mut form_sets = BTreeMap::<String, JsonValue>::new();
    let mut form_paths = BTreeMap::<String, String>::new();
    let mut source_paths = BTreeMap::<String, String>::new();
    for selection in &selections {
        crate::source_creation_store::active(deadline, cancelled)?;
        let raw = transport.read(
            owner,
            &selection.path,
            NativeReadKind::Metadata,
            MAX_RECORD_BYTES,
            deadline,
            cancelled,
        )?;
        let validated = crate::source_native_cli::private_owner::profile::validate_identity_record(
            &selection.profile_type_id,
            &selection.path,
            &selection.record_id,
            &raw,
            ctx,
            cut,
            worker,
            deadline,
            cancelled,
        )?;
        if source_paths
            .insert(selection.record_id.clone(), selection.path.clone())
            .is_some_and(|previous| previous != selection.path)
        {
            return Err(SourceCommandError::Conflict(
                "private source identity has conflicting selected paths",
            ));
        }
        let mut profile_contracts = BTreeMap::<String, String>::new();
        for dependency in &validated.dependencies {
            let name = dependency.path.as_str().to_owned();
            let digest = dependency.raw_sha256.to_prefixed();
            if source_dependencies
                .insert(name.clone(), digest.clone())
                .is_some_and(|previous| previous != digest)
            {
                return Err(SourceCommandError::Conflict(
                    "private assessment source profile dependency changed",
                ));
            }
            let maintained_digest = dependency.raw_sha256.to_hex();
            if profile_contracts
                .insert(name.clone(), maintained_digest.clone())
                .is_some_and(|previous| previous != maintained_digest)
            {
                return Err(SourceCommandError::Conflict(
                    "private assessment profile grammar snapshot changed",
                ));
            }
        }
        for dependency in &validated.dependencies {
            let reference = dependency.path.as_str();
            let raw = transport.read(
                owner,
                reference,
                NativeReadKind::Schema,
                MAX_SELECTED_FILE_BYTES,
                deadline,
                cancelled,
            )?;
            if Digest256::of_bytes(&raw) != dependency.raw_sha256 {
                return Err(SourceCommandError::Conflict(
                    "private assessment profile grammar differs from the selected source cut",
                ));
            }
        }
        let adapter = validated
            .profile
            .object_get("native_binding_adapter")
            .and_then(JsonValue::as_str);
        let mut native_metadata_snapshot = None;
        match adapter {
            None => {
                if selection.source_binding.is_some()
                    || validated.record.object_get("native_text_binding").is_some()
                    || selection.read_scope != "metadata_only"
                {
                    return Err(SourceCommandError::Denied(
                        "non-native private assessment source carries a native binding",
                    ));
                }
            }
            Some("source-text-unit-v1") => {
                if cmd::text(&validated.record, "record_type")? != "occurrence" {
                    return Err(SourceCommandError::Unsupported(
                        "private assessment native profile is not an occurrence",
                    ));
                }
                let binding =
                    selection
                        .source_binding
                        .as_ref()
                        .ok_or(SourceCommandError::Denied(
                            "private assessment occurrence binding is absent",
                        ))?;
                if cmd::canonical(cmd::field(&validated.record, "native_text_binding")?)?
                    != cmd::canonical(binding)?
                {
                    return Err(SourceCommandError::Conflict(
                        "private assessment occurrence differs from selected native binding",
                    ));
                }
                let resolved = {
                    let mut reader = PinnedOwnerReader {
                        owner,
                        ctx,
                        cut,
                        transport: &mut transport,
                        allow_content: false,
                    };
                    crate::source_sign_native::resolve_owner_assessment(
                        &mut reader,
                        worker,
                        binding,
                        &selection.origin_id,
                        NativeReadScope::MetadataOnly,
                        deadline,
                        cancelled,
                    )?
                };
                if cmd::field(&resolved.summary, "content_verified")? != &JsonValue::Bool(false) {
                    return Err(SourceCommandError::Denied(
                        "private assessment profile resolver disclosed native content",
                    ));
                }
                native_metadata_snapshot = Some(resolved.input_snapshot.clone());
                for (reference, digest) in &resolved.schema_digests {
                    let maintained_digest = digest.to_hex();
                    if profile_contracts
                        .insert(reference.clone(), maintained_digest.clone())
                        .is_some_and(|previous| previous != maintained_digest)
                    {
                        return Err(SourceCommandError::Conflict(
                            "private assessment native profile grammar snapshot changed",
                        ));
                    }
                }
                let _ = retain_native_resolution(
                    resolved,
                    false,
                    &selection.origin_id,
                    &selection.read_scope,
                    true,
                    &transport,
                    &mut native_inputs,
                    &mut native_snapshots,
                    &mut native_summaries,
                    &mut native_records,
                    &mut source_dependencies,
                )?;
            }
            Some(_) => {
                return Err(SourceCommandError::Unsupported(
                    "private assessment source profile native adapter is unsupported",
                ));
            }
        }
        profile_snapshots.push(owner_profile_reader_snapshot(
            &context_snapshot,
            &selection.source_access,
            selection.source_binding.as_ref(),
            &profile_contracts,
            &selection.path,
            &raw,
            native_metadata_snapshot.as_deref(),
        )?);
        let source_envelope = envelope(
            &selection.record_id,
            cmd::field(&validated.record, "record_version")?,
            &validated.record,
            &selection.origin_id,
        )?;
        let source_value = cmd::parse(&source_envelope.envelope)?;
        let source_ref = record_reference(&source_value)?;
        required_source_refs
            .entry(selection.record_id.clone())
            .or_default();
        let source_language_set = source_languages(&validated.record)?;
        required_languages.insert(selection.record_id.clone(), source_language_set.clone());
        records.push(source_envelope);
        if selection.form_ids.is_empty() {
            continue;
        }
        let form_path = format!(
            "{}.human-forms.json",
            selection.path.trim_end_matches(".json")
        );
        selected_private_path(&transport, &form_path, true)?;
        let raw = transport.read(
            owner,
            &form_path,
            NativeReadKind::Metadata,
            MAX_FORM_SET_BYTES,
            deadline,
            cancelled,
        )?;
        let forms = cmd::parse(&raw)?;
        if cmd::canonical(&forms)?.len() > MAX_RECORD_BYTES {
            return Err(SourceCommandError::Invalid(
                "private assessment form-set canonical byte budget",
            ));
        }
        crate::source_native_cli::private_owner::profile::validate_form_set(
            ctx, worker, &form_path, &forms, deadline, cancelled,
        )?;
        let subject = crate::source_forms::metadata_subject(&validated.record)?;
        if cmd::canonical(cmd::field(&forms, "subject")?)? != cmd::canonical(&subject)? {
            return Err(SourceCommandError::Conflict(
                "private assessment form set binds another source snapshot",
            ));
        }
        tos_validation::source_forms::source_copy_kernel::validate_history(&forms, &subject)
            .map_err(crate::source_forms::form_error)?;
        if cmd::canonical(&source_ref)? != cmd::canonical(&subject)? {
            return Err(SourceCommandError::Conflict(
                "private source envelope reference differs from its current form subject",
            ));
        }
        form_sets.insert(form_path.clone(), forms.clone());
        form_paths.insert(selection.record_id.clone(), form_path.clone());
        let current_forms = cmd::array(&forms, "forms")?;
        for identifier in &selection.form_ids {
            if form_paths
                .insert(identifier.clone(), form_path.clone())
                .is_some_and(|previous| previous != form_path)
            {
                return Err(SourceCommandError::Conflict(
                    "private form identity has conflicting adjacent form paths",
                ));
            }
            let matching = current_forms
                .iter()
                .filter(|form| cmd::text(form, "form_id").ok() == Some(identifier.as_str()))
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(SourceCommandError::Invalid(
                    "private assessment selected form is absent or not current",
                ));
            }
            let form = matching[0];
            if cmd::canonical(cmd::field(form, "subject")?)? != cmd::canonical(&subject)? {
                return Err(SourceCommandError::Conflict(
                    "private assessment selected form binds another source snapshot",
                ));
            }
            let form_envelope = envelope(
                identifier,
                cmd::field(form, "form_version")?,
                form,
                &selection.origin_id,
            )?;
            required_source_refs.insert(identifier.clone(), vec![source_ref.clone()]);
            let mut languages = source_language_set.clone();
            languages.extend(source_languages(form)?);
            required_languages.insert(identifier.clone(), languages);
            records.push(form_envelope);
        }
    }

    // Claim source/profile reads share this route's exact pinned transport.
    // The resolver's own byte set is rebound to held descriptors before its
    // records or snapshot are exposed to the journal owner.
    let claim_sources = {
        let mut reader = PinnedOwnerReader {
            owner,
            ctx,
            cut,
            transport: &mut transport,
            allow_content: claim_selections
                .iter()
                .any(|selection| selection.verify_content),
        };
        crate::source_native_cli::private_owner::claim::resolve_assessment_claim_sources(
            owner,
            selected_context,
            &claim_selections,
            ctx,
            cut,
            worker,
            &mut reader,
            deadline,
            cancelled,
        )?
    };
    for (reference, raw) in &claim_sources.source_files {
        crate::source_creation_store::active(deadline, cancelled)?;
        let matches_pinned = if let Some(retained) = transport.retained.get(reference) {
            &retained.raw == raw
        } else {
            transport.read(
                owner,
                reference,
                NativeReadKind::Metadata,
                raw.len().max(1),
                deadline,
                cancelled,
            )? == *raw
        };
        if !matches_pinned {
            return Err(SourceCommandError::Conflict(
                "private Claim helper read differs from the selected pinned source bytes",
            ));
        }
    }
    let public_native_identity_paths = claim_sources.public_native_identity_paths.clone();
    if let Some(paths) = &public_native_identity_paths {
        transport.verify_public_native_identity_inventory(ctx, cut, paths, deadline, cancelled)?;
    }
    for (reference, digest) in &claim_sources.schema_digests {
        let digest = digest.to_prefixed();
        if source_dependencies
            .insert(reference.clone(), digest.clone())
            .is_some_and(|previous| previous != digest)
        {
            return Err(SourceCommandError::Conflict(
                "private Claim schema dependency changed",
            ));
        }
    }
    for input in &claim_sources.native_inputs {
        let retained =
            transport
                .retained
                .get(&input.reference)
                .ok_or(SourceCommandError::Invalid(
                    "private Claim native input is absent from the pinned transport",
                ))?;
        if (retained.kind == NativeReadKind::Content) != (input.kind == NativeReadKind::Content)
            || retained.raw.len() != input.raw_size
            || Digest256::of_bytes(&retained.raw) != input.raw_sha256
        {
            return Err(SourceCommandError::Conflict(
                "private Claim native input differs from the selected pinned bytes",
            ));
        }
        if input.kind == NativeReadKind::Content && retained.root != RootKind::Private {
            return Err(SourceCommandError::Denied(
                "private Claim content is outside its selected owner-local root",
            ));
        }
        let kind = match input.kind {
            NativeReadKind::Metadata => "metadata",
            NativeReadKind::Schema => "schema",
            NativeReadKind::Support => "support",
            NativeReadKind::Content => "content",
        }
        .to_owned();
        let key = (input.reference.clone(), kind, input.category);
        if native_inputs
            .insert(key, input.clone())
            .is_some_and(|previous| previous.raw_sha256 != input.raw_sha256)
        {
            return Err(SourceCommandError::Conflict(
                "private Claim native input changed during source selection",
            ));
        }
    }
    records.extend(
        claim_sources
            .records
            .iter()
            .map(record_input)
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    native_records.extend(
        claim_sources
            .native_records
            .iter()
            .map(record_input)
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    for (identity, references) in claim_sources.required_source_refs {
        if required_source_refs
            .insert(identity, references.clone())
            .is_some_and(|previous| previous != references)
        {
            return Err(SourceCommandError::Conflict(
                "Claim source closure conflicts with another selected subject",
            ));
        }
    }
    for (identity, languages) in claim_sources.required_languages {
        if required_languages
            .insert(identity, languages.clone())
            .is_some_and(|previous| previous != languages)
        {
            return Err(SourceCommandError::Conflict(
                "Claim language closure conflicts with another selected subject",
            ));
        }
    }
    native_summaries.extend(claim_sources.native_summaries);
    native_snapshots.extend(claim_sources.native_snapshots);
    claim_snapshots.extend(claim_sources.snapshots);
    for (path, forms) in claim_sources.form_sets {
        if form_sets.insert(path, forms).is_some() {
            return Err(SourceCommandError::Conflict(
                "private form set path is selected by more than one source",
            ));
        }
    }
    for (identity, path) in claim_sources.form_paths {
        if form_paths
            .insert(identity, path.clone())
            .is_some_and(|previous| previous != path)
        {
            return Err(SourceCommandError::Conflict(
                "private form subject has conflicting adjacent form paths",
            ));
        }
    }
    for (identity, path) in claim_sources.source_paths {
        if source_paths
            .insert(identity, path.clone())
            .is_some_and(|previous| previous != path)
        {
            return Err(SourceCommandError::Conflict(
                "private source identity has conflicting selected paths",
            ));
        }
    }
    for selection in &native_selections {
        crate::source_creation_store::active(deadline, cancelled)?;
        let (scope, allow_content) = if selection.read_scope == "exact_owner_local" {
            (NativeReadScope::ExactOwnerLocal, true)
        } else {
            (NativeReadScope::MetadataOnly, false)
        };
        let resolved = {
            let mut reader = PinnedOwnerReader {
                owner,
                ctx,
                cut,
                transport: &mut transport,
                allow_content,
            };
            crate::source_sign_native::resolve_owner_assessment(
                &mut reader,
                worker,
                &selection.binding,
                &selection.origin_id,
                scope,
                deadline,
                cancelled,
            )?
        };
        let unit_id = cmd::text(&selection.binding, "unit_id")?.to_owned();
        let _ = retain_native_resolution(
            resolved,
            allow_content,
            &selection.origin_id,
            &selection.read_scope,
            false,
            &transport,
            &mut native_inputs,
            &mut native_snapshots,
            &mut native_summaries,
            &mut native_records,
            &mut source_dependencies,
        )?;
        let summary = native_summaries.last().ok_or(SourceCommandError::Invalid(
            "selected native unit summary is absent",
        ))?;
        let languages = summary
            .object_get("language")
            .and_then(JsonValue::as_str)
            .filter(|language| cmd::nonblank(language))
            .map(|language| vec![language.to_lowercase()])
            .unwrap_or_default();
        required_languages.insert(unit_id, languages);
    }
    if records.len() + native_records.len() > MAX_ASSESSMENTS {
        return Err(SourceCommandError::Invalid(
            "private assessment source record budget",
        ));
    }
    transport.verify_roots(owner, deadline, cancelled)?;
    transport.verify_files(deadline, cancelled)?;
    transport.verify_roots(owner, deadline, cancelled)?;
    if owner.snapshot(deadline, cancelled)?.to_prefixed() != context_snapshot {
        return Err(SourceCommandError::Conflict(
            "private assessment owner context changed during selection",
        ));
    }
    let native_inputs = native_inputs.into_values().collect::<Vec<_>>();
    let snapshot = source_snapshot(
        &context_snapshot,
        &selection_rows,
        &transport.retained,
        &source_dependencies,
        &native_inputs,
        &native_snapshots,
        &claim_snapshots,
    )?;
    let owner_local_sources_snapshot = owner_local_sources_snapshot(
        owner,
        &context_snapshot,
        &profile_snapshots,
        &claim_snapshots,
        &transport.retained,
    )?;
    let retained_raw = transport
        .retained
        .iter()
        .map(|(reference, entry)| (reference.clone(), entry.raw.clone()))
        .collect();
    Ok(PrivateAssessmentSources {
        records,
        native_records,
        native_inputs,
        native_snapshots,
        native_summaries,
        profile_snapshots,
        owner_local_sources_snapshot,
        required_source_refs,
        required_languages,
        claim_snapshots,
        form_sets,
        form_paths,
        source_paths,
        snapshot,
        selection_rows,
        retained_raw,
        context_snapshot,
        selected_revision: ctx.base_revision.clone(),
        source_dependencies,
        public_native_identity_paths,
        transport,
    })
}

/// Exact metadata retained by the private native-text resolver for conformance
/// tests. This type is compiled only by an explicit test feature and carries no
/// journal, write, admission, or publication authority.
#[cfg(feature = "conformance-owner-local-source-resolver")]
pub struct NativeTextUnitResolutionForConformance {
    pub native_records: Vec<JsonValue>,
    pub native_summaries: Vec<JsonValue>,
}

/// Exercise the same protected owner-context and exact-source resolver used by
/// private assessment. The caller supplies only the already selected native
/// TextUnit rows; this helper reads the owner-context and assessment contracts
/// from the current source cut and returns the resolver's actual metadata.
#[cfg(feature = "conformance-owner-local-source-resolver")]
pub fn resolve_native_text_units_for_conformance(
    context_path: &Path,
    native_text_units: &JsonValue,
    cut: &CorpusCutReader,
    worker_path: &Path,
    worker_sha256: Digest256,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<NativeTextUnitResolutionForConformance> {
    const OWNER_CONTEXT_SCHEMA_FOR_CONFORMANCE: &str =
        "ToS/contracts/owner-local-source-context.schema.json";

    if native_text_units
        .as_array()
        .is_none_or(|selections| selections.is_empty())
    {
        return Err(SourceCommandError::Invalid(
            "conformance resolver requires an explicit native TextUnit selection",
        ));
    }
    let mut worker_budget = tos_validation::executor::ExecutorBudget::laboratory();
    worker_budget.execution_wall = deadline.saturating_duration_since(std::time::Instant::now());
    worker_budget.cpu_seconds = 3;
    worker_budget.address_space_bytes = 1_073_741_824;
    let mut worker = CutWorkerSchemaExecutor::from_cut(
        cut,
        tos_validation::FormatProfile::AssertedSourceCandidateV1,
        tos_validation::executor::ExactWorkerIdentity {
            absolute_path: worker_path.to_path_buf(),
            sha256: worker_sha256,
        },
        worker_budget,
        tos_validation::source_cut::CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 262_144,
        },
        deadline,
        cancelled,
    )?;

    let contract_paths = std::iter::once(OWNER_CONTEXT_SCHEMA_FOR_CONFORMANCE)
        .chain(OWNER_ASSESSMENT_FORM_CONTRACTS.iter().copied());
    let mut files = Vec::new();
    let mut selected_context_schema = None;
    for reference in contract_paths {
        let logical = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("conformance assessment contract path"))?;
        let member = cut
            .read_member(
                cut.current().revision(),
                &logical,
                MAX_SELECTED_FILE_BYTES as u64,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Conflict(
                    "conformance assessment contract is outside the selected source cut",
                )
            })?;
        if reference == OWNER_CONTEXT_SCHEMA_FOR_CONFORMANCE {
            if worker.contract_digest(reference) != Some(Digest256::of_bytes(&member.raw)) {
                return Err(SourceCommandError::Conflict(
                    "conformance owner-context schema differs from selected worker",
                ));
            }
            selected_context_schema = Some(member.raw.clone());
        }
        files.push(crate::source_command::SourceFile {
            path: logical,
            raw: member.raw,
        });
    }
    let selected_context_schema = selected_context_schema.ok_or(SourceCommandError::Invalid(
        "conformance owner-context schema selection is absent",
    ))?;
    let context = CommandContext {
        base_revision: cut.current().revision().clone(),
        configuration_raw: Vec::new(),
        request_raw: Vec::new(),
        recorded_at: "conformance-only source resolution; not a receipt".into(),
        effective_uid: u64::from(rustix::process::geteuid().as_raw()),
        files,
    };
    context.check()?;

    let (owner, selected_context) = OwnerTextContext::select(
        context_path,
        &selected_context_schema,
        &mut worker,
        deadline,
        cancelled,
    )?;
    let empty = JsonValue::Array(Vec::new());
    let selected = select_owner_local_sources(
        &owner,
        &selected_context,
        &empty,
        native_text_units,
        None,
        &context,
        cut,
        &mut worker,
        deadline,
        cancelled,
    )?;
    let native_records = selected
        .native_records
        .into_iter()
        .map(|record| cmd::parse(&record.envelope))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(NativeTextUnitResolutionForConformance {
        native_records,
        native_summaries: selected.native_summaries,
    })
}
