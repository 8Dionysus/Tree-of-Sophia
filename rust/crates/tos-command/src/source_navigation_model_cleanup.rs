//! Bounded cleanup for the exact host-local export consumed by native model
//! preparation. The captured manifest retains descriptor custody and never
//! grants authority to remove an unlisted path.

use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_creation_store::{IsolatedCreationRoot, active, inode, owned};
use rustix::fs::{AtFlags, Dir, unlinkat};
use std::collections::BTreeSet;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher};

const EXPORT_DIR: &str = ".managed-source-cut";
const INPUT_DIR: &str = "inputs";
const NORMALIZATION_FILE: &str = "knowledge_normalization.rs";
const SNAPSHOT_FILE: &str = "snapshot.json";
const PRIVATE_DIR_MODE: u32 = 0o700;
const IMMUTABLE_FILE_MODE: u32 = 0o444;
const PRIVATE_INPUT_MODE: u32 = 0o400;
pub(crate) const MAX_MANAGED_SOURCE_CUT_CLEANUP_ENTRIES_V1: usize =
    crate::source_current_cut::INITIAL_EXPORT_MANIFEST_ENTRIES;
const MAX_HOST_EXPORT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Explicit limits for inventorying and removing the staged export.
#[derive(Clone, Copy)]
pub(crate) struct ManagedSourceCutCleanupLimitsV1 {
    pub(crate) max_entries: usize,
    pub(crate) max_total_bytes: u64,
    pub(crate) max_file_bytes: u64,
}

/// The one optional compiler input admitted beside the staged source cut.
#[derive(Clone, Copy)]
pub(crate) struct ExpectedNormalizationInputV1 {
    pub(crate) size_bytes: u64,
    pub(crate) sha256: Digest256,
}

struct DirectoryEntry {
    parent: usize,
    name: String,
    file: File,
    identity: (u64, u64),
    uid: u32,
    mode: u32,
}

struct FileEntry {
    parent: usize,
    name: String,
    identity: (u64, u64),
    timestamps: (i64, i64, i64, i64),
    uid: u32,
    mode: u32,
    size_bytes: u64,
    sha256: Digest256,
}

/// Parent-descriptor-bound inventory of the staged source export. Only the
/// fixed-layout directories remain open; files are reopened one at a time and
/// matched by inode, timestamps, metadata and digest before cleanup. It
/// intentionally is not serializable or cloneable.
pub(crate) struct ManagedSourceCutCleanupManifestV1 {
    root: File,
    root_identity: (u64, u64),
    directories: Vec<DirectoryEntry>,
    files: Vec<FileEntry>,
    expected_normalization: Option<ExpectedNormalizationInputV1>,
    allow_partial: bool,
    max_entries: usize,
    max_total_bytes: u64,
    max_file_bytes: u64,
}

/// Capture the closed, known export tree while the caller still holds its
/// selected cut. The only extra file accepted is the exact selected compiler
/// normalization input, when its size and digest are supplied.
pub(crate) fn capture_managed_source_cut_cleanup_v1(
    isolated: &IsolatedCreationRoot,
    expected_normalization: Option<ExpectedNormalizationInputV1>,
    limits: ManagedSourceCutCleanupLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ManagedSourceCutCleanupManifestV1> {
    capture_managed_source_cut_cleanup_with_policy_v1(
        isolated,
        expected_normalization,
        limits,
        false,
        deadline,
        cancelled,
    )
}

/// Capture only observed entries from the fixed source-export layout. This is
/// for failure cleanup before the writer has completed its content-addressed
/// tree; it never guesses absent members or admits arbitrary recursive paths.
pub(crate) fn capture_partial_managed_source_cut_cleanup_v1(
    isolated: &IsolatedCreationRoot,
    expected_normalization: Option<ExpectedNormalizationInputV1>,
    limits: ManagedSourceCutCleanupLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ManagedSourceCutCleanupManifestV1> {
    capture_managed_source_cut_cleanup_with_policy_v1(
        isolated,
        expected_normalization,
        limits,
        true,
        deadline,
        cancelled,
    )
}

fn capture_managed_source_cut_cleanup_with_policy_v1(
    isolated: &IsolatedCreationRoot,
    expected_normalization: Option<ExpectedNormalizationInputV1>,
    limits: ManagedSourceCutCleanupLimitsV1,
    allow_partial: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ManagedSourceCutCleanupManifestV1> {
    validate_limits(&limits)?;
    if expected_normalization.is_some_and(|input| input.size_bytes == 0)
        || expected_normalization.is_some_and(|input| input.size_bytes > limits.max_file_bytes)
    {
        return Err(Error::Invalid("navigation cleanup input size limit"));
    }

    active(deadline, cancelled)?;
    let root = isolated.verify_current(deadline, cancelled)?;
    let uid = rustix::process::geteuid().as_raw();
    let root_metadata = checked_directory(&root, uid, None)?;
    let root_identity = inode(&root_metadata);
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut total_bytes = 0u64;

    let mut expected_root = BTreeSet::from([EXPORT_DIR.to_owned()]);
    if expected_normalization.is_some() {
        expected_root.insert(INPUT_DIR.to_owned());
    }
    let root_names = list_names(&root, limits.max_entries, deadline, cancelled)?;
    require_names_or_subset(&root_names, &expected_root, allow_partial)?;

    if root_names.contains(EXPORT_DIR) {
        let export = push_directory(
            &root,
            0,
            EXPORT_DIR,
            uid,
            &mut directories,
            deadline,
            cancelled,
        )?;
        let export_directory = directories[export - 1]
            .file
            .try_clone()
            .map_err(|_| Error::Invalid("navigation cleanup export descriptor"))?;
        let expected_export = BTreeSet::from(["objects".to_owned(), "revisions".to_owned()]);
        let export_names = list_names(&export_directory, limits.max_entries, deadline, cancelled)?;
        require_names_or_subset(&export_names, &expected_export, allow_partial)?;

        if export_names.contains("objects") {
            let objects = push_directory(
                &export_directory,
                export,
                "objects",
                uid,
                &mut directories,
                deadline,
                cancelled,
            )?;
            let object_names = list_names(
                &directories[objects - 1].file,
                limits.max_entries,
                deadline,
                cancelled,
            )?;
            for name in object_names {
                active(deadline, cancelled)?;
                let expected_digest = parse_digest_name(&name)?;
                let file =
                    tos_fd_open::open_regular_at(&directories[objects - 1].file, Path::new(&name))
                        .map_err(|_| {
                            Error::Conflict("navigation cleanup object replaced or unsafe")
                        })?;
                let entry = capture_file(
                    objects,
                    name,
                    file,
                    uid,
                    IMMUTABLE_FILE_MODE,
                    limits.max_file_bytes,
                    &mut total_bytes,
                    limits.max_total_bytes,
                    deadline,
                    cancelled,
                )?;
                if entry.sha256 != expected_digest {
                    return Err(Error::Conflict(
                        "navigation cleanup object name digest differs",
                    ));
                }
                files.push(entry);
            }
        }

        if export_names.contains("revisions") {
            let revisions = push_directory(
                &export_directory,
                export,
                "revisions",
                uid,
                &mut directories,
                deadline,
                cancelled,
            )?;
            let revision_names = list_names(
                &directories[revisions - 1].file,
                limits.max_entries,
                deadline,
                cancelled,
            )?;
            let revisions_directory = directories[revisions - 1]
                .file
                .try_clone()
                .map_err(|_| Error::Invalid("navigation cleanup revisions descriptor"))?;
            for revision in revision_names {
                active(deadline, cancelled)?;
                let _ = parse_digest_name(&revision)?;
                let revision_index = push_directory(
                    &revisions_directory,
                    revisions,
                    &revision,
                    uid,
                    &mut directories,
                    deadline,
                    cancelled,
                )?;
                let expected = BTreeSet::from([SNAPSHOT_FILE.to_owned()]);
                require_names(
                    &directories[revision_index - 1].file,
                    &expected,
                    limits.max_entries,
                    deadline,
                    cancelled,
                )?;
                let file = tos_fd_open::open_regular_at(
                    &directories[revision_index - 1].file,
                    Path::new(SNAPSHOT_FILE),
                )
                .map_err(|_| Error::Conflict("navigation cleanup snapshot replaced or unsafe"))?;
                files.push(capture_file(
                    revision_index,
                    SNAPSHOT_FILE.to_owned(),
                    file,
                    uid,
                    IMMUTABLE_FILE_MODE,
                    limits.max_file_bytes,
                    &mut total_bytes,
                    limits.max_total_bytes,
                    deadline,
                    cancelled,
                )?);
            }
        }
    } else if !allow_partial {
        return Err(Error::Conflict("navigation cleanup source export absent"));
    }

    if let Some(expected) = expected_normalization {
        if root_names.contains(INPUT_DIR) {
            let input_index = push_directory(
                &root,
                0,
                INPUT_DIR,
                uid,
                &mut directories,
                deadline,
                cancelled,
            )?;
            let expected_names = BTreeSet::from([NORMALIZATION_FILE.to_owned()]);
            let input_names = list_names(
                &directories[input_index - 1].file,
                limits.max_entries,
                deadline,
                cancelled,
            )?;
            require_names_or_subset(&input_names, &expected_names, allow_partial)?;
            if input_names.contains(NORMALIZATION_FILE) {
                let file = tos_fd_open::open_regular_at(
                    &directories[input_index - 1].file,
                    Path::new(NORMALIZATION_FILE),
                )
                .map_err(|_| Error::Conflict("navigation cleanup normalization input unsafe"))?;
                let entry = capture_file(
                    input_index,
                    NORMALIZATION_FILE.to_owned(),
                    file,
                    uid,
                    PRIVATE_INPUT_MODE,
                    limits.max_file_bytes,
                    &mut total_bytes,
                    limits.max_total_bytes,
                    deadline,
                    cancelled,
                )?;
                if entry.size_bytes != expected.size_bytes || entry.sha256 != expected.sha256 {
                    return Err(Error::Conflict(
                        "navigation cleanup normalization input differs",
                    ));
                }
                files.push(entry);
            } else if !allow_partial {
                return Err(Error::Conflict(
                    "navigation cleanup normalization input absent",
                ));
            }
        } else if !allow_partial {
            return Err(Error::Conflict(
                "navigation cleanup normalization directory absent",
            ));
        }
    }

    if directories.len() + files.len() > limits.max_entries {
        return Err(Error::Invalid("navigation cleanup entry count limit"));
    }
    let manifest = ManagedSourceCutCleanupManifestV1 {
        root,
        root_identity,
        directories,
        files,
        expected_normalization,
        allow_partial,
        max_entries: limits.max_entries,
        max_total_bytes: limits.max_total_bytes,
        max_file_bytes: limits.max_file_bytes,
    };
    verify_inventory(&manifest, deadline, cancelled)?;
    isolated.verify_current(deadline, cancelled)?;
    Ok(manifest)
}

/// Verify the entire captured tree before unlinking any file, then remove only
/// those files and empty directories through their held parent descriptors.
pub(crate) fn cleanup_managed_source_cut_export_v1(
    isolated: &IsolatedCreationRoot,
    manifest: ManagedSourceCutCleanupManifestV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    active(deadline, cancelled)?;
    let current_root = isolated.verify_current(deadline, cancelled)?;
    let uid = rustix::process::geteuid().as_raw();
    let root_metadata = checked_directory(&current_root, uid, None)?;
    let held_root_metadata = checked_directory(&manifest.root, uid, None)?;
    if inode(&root_metadata) != manifest.root_identity
        || inode(&held_root_metadata) != manifest.root_identity
    {
        return Err(Error::Conflict("navigation cleanup root identity changed"));
    }
    verify_inventory(&manifest, deadline, cancelled)?;

    // Files are all re-opened and checked immediately before each unlink. A
    // mismatch leaves the remaining staged tree in place for safe inspection.
    for entry in &manifest.files {
        active(deadline, cancelled)?;
        verify_file_entry(&manifest, entry, uid, deadline, cancelled)?;
        unlinkat(
            directory_file(&manifest, entry.parent),
            entry.name.as_str(),
            AtFlags::empty(),
        )
        .map_err(|_| Error::Conflict("navigation cleanup file changed before unlink"))?;
    }

    // Parents precede children in the captured vector, so reverse order is a
    // descriptor-relative bottom-up removal of the known empty directories.
    for index in (0..manifest.directories.len()).rev() {
        active(deadline, cancelled)?;
        let entry = &manifest.directories[index];
        let named = tos_fd_open::open_directory_at(
            directory_file(&manifest, entry.parent),
            Path::new(&entry.name),
        )
        .map_err(|_| Error::Conflict("navigation cleanup directory replaced"))?;
        let metadata = checked_directory(&named, uid, Some(entry.mode))?;
        let held_metadata = checked_directory(&entry.file, uid, Some(entry.mode))?;
        if inode(&metadata) != entry.identity
            || inode(&held_metadata) != entry.identity
            || !list_names(&entry.file, manifest.max_entries, deadline, cancelled)?.is_empty()
        {
            return Err(Error::Conflict("navigation cleanup directory is not empty"));
        }
        unlinkat(
            directory_file(&manifest, entry.parent),
            entry.name.as_str(),
            AtFlags::REMOVEDIR,
        )
        .map_err(|_| Error::Conflict("navigation cleanup directory changed before removal"))?;
    }

    isolated.verify_current(deadline, cancelled)?;
    let remaining = list_names(&manifest.root, manifest.max_entries, deadline, cancelled)?;
    if !remaining.is_empty() {
        return Err(Error::Conflict(
            "navigation cleanup left unknown root entries",
        ));
    }
    Ok(())
}

fn validate_limits(limits: &ManagedSourceCutCleanupLimitsV1) -> Result<()> {
    if limits.max_entries == 0
        || limits.max_entries > MAX_MANAGED_SOURCE_CUT_CLEANUP_ENTRIES_V1
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > MAX_HOST_EXPORT_BYTES
        || limits.max_file_bytes == 0
        || limits.max_file_bytes > limits.max_total_bytes
    {
        return Err(Error::Invalid("navigation cleanup bounds invalid"));
    }
    Ok(())
}

fn checked_directory(file: &File, uid: u32, exact_mode: Option<u32>) -> Result<Metadata> {
    let metadata = owned(file, uid, true)?;
    let mode = metadata.mode() & 0o777;
    if metadata.mode() & 0o7000 != 0
        || exact_mode.is_some_and(|expected| mode != expected)
        || exact_mode.is_none() && mode != PRIVATE_DIR_MODE
    {
        return Err(Error::Denied("navigation cleanup directory mode"));
    }
    Ok(metadata)
}

fn push_directory(
    parent: &File,
    parent_index: usize,
    name: &str,
    uid: u32,
    directories: &mut Vec<DirectoryEntry>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<usize> {
    active(deadline, cancelled)?;
    let file = tos_fd_open::open_directory_at(parent, Path::new(name))
        .map_err(|_| Error::Conflict("navigation cleanup directory absent or unsafe"))?;
    let metadata = checked_directory(&file, uid, None)?;
    let index = directories.len() + 1; // index zero is the held isolated root.
    directories.push(DirectoryEntry {
        parent: parent_index,
        name: name.to_owned(),
        identity: inode(&metadata),
        uid: metadata.uid(),
        mode: metadata.mode() & 0o777,
        file,
    });
    Ok(index)
}

fn list_names(
    directory: &File,
    max_entries: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let mut stream = Dir::read_from(directory)
        .map_err(|_| Error::Invalid("navigation cleanup directory enumeration"))?;
    for item in &mut stream {
        active(deadline, cancelled)?;
        let entry = item.map_err(|_| Error::Invalid("navigation cleanup directory read"))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = std::str::from_utf8(bytes)
            .map_err(|_| Error::Conflict("navigation cleanup non-UTF8 entry"))?
            .to_owned();
        if name.is_empty() || name.contains('/') || !names.insert(name) || names.len() > max_entries
        {
            return Err(Error::Invalid("navigation cleanup entry count or name"));
        }
    }
    Ok(names)
}

fn require_names(
    directory: &File,
    expected: &BTreeSet<String>,
    max_entries: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let names = list_names(directory, max_entries, deadline, cancelled)?;
    if &names != expected {
        return Err(Error::Conflict(
            "navigation cleanup tree has unknown entries",
        ));
    }
    Ok(())
}

fn require_names_or_subset(
    found: &BTreeSet<String>,
    allowed: &BTreeSet<String>,
    allow_partial: bool,
) -> Result<()> {
    let unexpected = if allow_partial {
        !found.is_subset(allowed)
    } else {
        found != allowed
    };
    if unexpected {
        return Err(Error::Conflict(
            "navigation cleanup tree has unexpected or missing entries",
        ));
    }
    Ok(())
}

fn parse_digest_name(name: &str) -> Result<Digest256> {
    if name.len() != 64
        || name
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(Error::Conflict("navigation cleanup digest entry name"));
    }
    Digest256::from_hex(name).map_err(|_| Error::Conflict("navigation cleanup digest entry name"))
}

#[allow(clippy::too_many_arguments)]
fn capture_file(
    parent: usize,
    name: String,
    mut file: File,
    uid: u32,
    expected_mode: u32,
    max_file_bytes: u64,
    total_bytes: &mut u64,
    max_total_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<FileEntry> {
    active(deadline, cancelled)?;
    let metadata = owned(&file, uid, false)?;
    let mode = metadata.mode() & 0o777;
    let size_bytes = metadata.len();
    let identity = inode(&metadata);
    if metadata.mode() & 0o7000 != 0
        || mode != expected_mode
        || metadata.nlink() != 1
        || size_bytes > max_file_bytes
        || size_bytes > max_total_bytes.saturating_sub(*total_bytes)
    {
        return Err(Error::Conflict("navigation cleanup file metadata or size"));
    }
    let sha256 = hash_exact_file(&mut file, &metadata, size_bytes, deadline, cancelled)?;
    let after = owned(&file, uid, false)?;
    if inode(&after) != identity
        || after.len() != size_bytes
        || after.uid() != metadata.uid()
        || after.mode() & 0o777 != mode
        || after.mtime() != metadata.mtime()
        || after.mtime_nsec() != metadata.mtime_nsec()
        || after.ctime() != metadata.ctime()
        || after.ctime_nsec() != metadata.ctime_nsec()
    {
        return Err(Error::Conflict(
            "navigation cleanup file changed while reading",
        ));
    }
    *total_bytes = total_bytes
        .checked_add(size_bytes)
        .ok_or(Error::Invalid("navigation cleanup byte count overflow"))?;
    Ok(FileEntry {
        parent,
        name,
        identity,
        timestamps: (
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ),
        uid: metadata.uid(),
        mode,
        size_bytes,
        sha256,
    })
}

fn hash_exact_file(
    file: &mut File,
    before: &Metadata,
    size_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Digest256> {
    let mut hasher = Digest256Hasher::new();
    let mut bytes_read = 0u64;
    let mut chunk = [0u8; 64 * 1024];
    while bytes_read < size_bytes {
        active(deadline, cancelled)?;
        let remaining = size_bytes - bytes_read;
        let read_limit = remaining.min(chunk.len() as u64) as usize;
        let count = file
            .read(&mut chunk[..read_limit])
            .map_err(|_| Error::Invalid("navigation cleanup file read"))?;
        if count == 0 {
            return Err(Error::Conflict("navigation cleanup file truncated"));
        }
        hasher.update(&chunk[..count]);
        bytes_read = bytes_read
            .checked_add(count as u64)
            .ok_or(Error::Invalid("navigation cleanup file size overflow"))?;
    }
    active(deadline, cancelled)?;
    let mut extra = [0u8; 1];
    if file
        .read(&mut extra)
        .map_err(|_| Error::Invalid("navigation cleanup file read"))?
        != 0
    {
        return Err(Error::Conflict(
            "navigation cleanup file grew while reading",
        ));
    }
    let after = file
        .metadata()
        .map_err(|_| Error::Invalid("navigation cleanup file metadata"))?;
    if inode(&after) != inode(before)
        || after.len() != size_bytes
        || after.mtime() != before.mtime()
        || after.mtime_nsec() != before.mtime_nsec()
        || after.ctime() != before.ctime()
        || after.ctime_nsec() != before.ctime_nsec()
    {
        return Err(Error::Conflict(
            "navigation cleanup file mutated while reading",
        ));
    }
    Ok(hasher.finalize())
}

fn verify_inventory(
    manifest: &ManagedSourceCutCleanupManifestV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    if manifest.directories.len() + manifest.files.len() > manifest.max_entries {
        return Err(Error::Invalid("navigation cleanup manifest entry limit"));
    }
    let uid = rustix::process::geteuid().as_raw();
    for index in 0..manifest.directories.len() {
        active(deadline, cancelled)?;
        let entry = &manifest.directories[index];
        let parent = directory_file(manifest, entry.parent);
        let named = tos_fd_open::open_directory_at(parent, Path::new(&entry.name))
            .map_err(|_| Error::Conflict("navigation cleanup directory replaced"))?;
        let metadata = checked_directory(&named, uid, Some(entry.mode))?;
        let held_metadata = checked_directory(&entry.file, uid, Some(entry.mode))?;
        if inode(&metadata) != entry.identity
            || metadata.uid() != entry.uid
            || inode(&held_metadata) != entry.identity
            || held_metadata.uid() != entry.uid
        {
            return Err(Error::Conflict(
                "navigation cleanup directory identity changed",
            ));
        }
        let expected_names = manifest
            .directories
            .iter()
            .filter(|child| child.parent == index + 1)
            .map(|child| child.name.clone())
            .chain(
                manifest
                    .files
                    .iter()
                    .filter(|child| child.parent == index + 1)
                    .map(|child| child.name.clone()),
            )
            .collect::<BTreeSet<_>>();
        let found = list_names(&entry.file, manifest.max_entries, deadline, cancelled)?;
        if found != expected_names {
            return Err(Error::Conflict(
                "navigation cleanup tree membership changed",
            ));
        }
    }
    let expected_root = manifest
        .directories
        .iter()
        .filter(|entry| entry.parent == 0)
        .map(|entry| entry.name.clone())
        .chain(
            manifest
                .files
                .iter()
                .filter(|entry| entry.parent == 0)
                .map(|entry| entry.name.clone()),
        )
        .collect::<BTreeSet<_>>();
    if list_names(&manifest.root, manifest.max_entries, deadline, cancelled)? != expected_root {
        return Err(Error::Conflict(
            "navigation cleanup root membership changed",
        ));
    }
    for entry in &manifest.files {
        active(deadline, cancelled)?;
        verify_file_entry(manifest, entry, uid, deadline, cancelled)?;
    }
    let total = manifest
        .files
        .iter()
        .try_fold(0u64, |sum, entry| sum.checked_add(entry.size_bytes));
    if total.is_none_or(|total| total > manifest.max_total_bytes)
        || manifest
            .files
            .iter()
            .any(|entry| entry.size_bytes > manifest.max_file_bytes)
    {
        return Err(Error::Invalid("navigation cleanup manifest byte limit"));
    }
    let normalization_count = manifest
        .files
        .iter()
        .filter(|entry| entry.name == NORMALIZATION_FILE)
        .count();
    if normalization_count > 1
        || manifest.expected_normalization.is_none() && normalization_count != 0
        || !manifest.allow_partial
            && manifest.expected_normalization.is_some() != (normalization_count == 1)
    {
        return Err(Error::Conflict(
            "navigation cleanup normalization manifest differs",
        ));
    }
    if let Some(expected) = manifest.expected_normalization {
        match manifest
            .files
            .iter()
            .find(|entry| entry.name == NORMALIZATION_FILE)
        {
            Some(input)
                if input.size_bytes == expected.size_bytes
                    && input.sha256 == expected.sha256
                    && input.mode == PRIVATE_INPUT_MODE => {}
            Some(_) => {
                return Err(Error::Conflict(
                    "navigation cleanup normalization manifest differs",
                ));
            }
            None if manifest.allow_partial => {}
            None => {
                return Err(Error::Conflict(
                    "navigation cleanup normalization input absent",
                ));
            }
        }
    }
    Ok(())
}

fn verify_file_entry(
    manifest: &ManagedSourceCutCleanupManifestV1,
    entry: &FileEntry,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    let parent = directory_file(manifest, entry.parent);
    let mut named = tos_fd_open::open_regular_at(parent, Path::new(&entry.name))
        .map_err(|_| Error::Conflict("navigation cleanup file replaced"))?;
    let metadata = owned(&named, uid, false)?;
    if inode(&metadata) != entry.identity
        || metadata.uid() != entry.uid
        || metadata.mode() & 0o7000 != 0
        || metadata.mode() & 0o777 != entry.mode
        || metadata.len() != entry.size_bytes
        || metadata.nlink() != 1
        || (
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ) != entry.timestamps
    {
        return Err(Error::Conflict("navigation cleanup file identity changed"));
    }
    let digest = hash_exact_file(&mut named, &metadata, entry.size_bytes, deadline, cancelled)?;
    if digest != entry.sha256 {
        return Err(Error::Conflict("navigation cleanup file digest changed"));
    }
    Ok(())
}

fn directory_file(manifest: &ManagedSourceCutCleanupManifestV1, index: usize) -> &File {
    if index == 0 {
        &manifest.root
    } else {
        &manifest.directories[index - 1].file
    }
}

#[cfg(test)]
mod streamed_cleanup_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    #[test]
    fn bootstrap_contract_export_cleanup_keeps_bounded_descriptors_and_refuses_drift() {
        let temp = tempfile::tempdir().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let limits = ManagedSourceCutCleanupLimitsV1 {
            max_entries: 40_000,
            max_total_bytes: 1024 * 1024,
            max_file_bytes: 1024,
        };
        for adversarial in [false, true] {
            let isolated = IsolatedCreationRoot::create(temp.path(), deadline, &cancelled).unwrap();
            let export = isolated.path().join(EXPORT_DIR);
            let objects = export.join("objects");
            for path in [&export, &objects, &export.join("revisions")] {
                std::fs::create_dir(path).unwrap();
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            let mut first = None;
            for index in 0..256 {
                let raw = format!("source member {index}");
                let path = objects.join(Digest256::of_bytes(raw.as_bytes()).to_hex());
                std::fs::write(&path, &raw).unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
                first.get_or_insert(path);
            }
            let manifest = capture_partial_managed_source_cut_cleanup_v1(
                &isolated, None, limits, deadline, &cancelled,
            )
            .unwrap();
            let held = std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
                .filter(|path| path.starts_with(isolated.path()))
                .count();
            assert!(
                held < 16,
                "cleanup retained {held} descriptors for 256 members"
            );
            if adversarial {
                let unknown = objects.join("unknown");
                std::fs::write(&unknown, b"unlisted").unwrap();
                assert!(verify_inventory(&manifest, deadline, &cancelled).is_err());
                std::fs::remove_file(unknown).unwrap();
                let path = first.unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
                std::fs::write(&path, b"changed content").unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
                assert!(
                    cleanup_managed_source_cut_export_v1(&isolated, manifest, deadline, &cancelled)
                        .is_err()
                );
                assert_eq!(std::fs::read_dir(&objects).unwrap().count(), 256);
            } else {
                cleanup_managed_source_cut_export_v1(&isolated, manifest, deadline, &cancelled)
                    .unwrap();
                assert_eq!(std::fs::read_dir(isolated.path()).unwrap().count(), 0);
            }
        }
    }
}
