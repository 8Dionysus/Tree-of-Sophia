//! Descriptor-rooted bounded census of the selected authored source tree.
//!
//! Filesystem inventory rows live in the operation's existing auxiliary
//! SQLite database. This module owns neither that database nor the operation
//! grants; callers provide the held connection, the original logical I/O
//! ledger, the remaining state slice, and the shared work callback.

use super::source_admission::{active, invalid};
use rusqlite::{OptionalExtension, params};
use rustix::fs::{AtFlags, FileType, RawDir};
use std::{
    fs::{File, Metadata},
    io::{self, Read},
    mem::{MaybeUninit, size_of},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher};
use tos_source_store::{PinnedSqliteConnection, PinnedSqliteIoBudget};

const ROOT_RELATIVE: &str = "ToS";
const DIRECTORY_BUFFER_BYTES: usize = 8192;
const MEMBER_BLOCK_BYTES: usize = 65_536;
const NAME_METADATA_GUARD_BYTES: usize = 4096;
const METADATA_GUARD_BYTES: usize = 4096;
const CENSUS_DIGEST_DOMAIN: &[u8] = b"tos-native-source-filesystem-census-v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceCensusScan {
    Proposal,
    Terminal,
    // Authenticated logical input rows, distinct from the physical source.
    IndexedProposal,
}

impl SourceCensusScan {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Terminal => "terminal",
            Self::IndexedProposal => "indexed_proposal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceCensusWorkKind {
    Entry,
    Directory,
    SqlRow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceCensusLimits {
    pub(crate) max_files: u64,
    pub(crate) max_directories: u64,
    pub(crate) max_entries: u64,
    pub(crate) max_path_bytes: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_member_bytes: u64,
    pub(crate) max_source_bytes: u64,
    /// Remaining bytes from the caller's existing operation state ledger.
    pub(crate) state_slice_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceCensusSummary {
    pub(crate) member_count: u64,
    pub(crate) source_bytes: u64,
    pub(crate) directory_count: u64,
    pub(crate) entry_count: u64,
    pub(crate) payload_read_bytes: u64,
    pub(crate) metadata_read_upper_bytes: u64,
    /// Number of row operations for which the shared Work callback was charged.
    pub(crate) sql_row_operations: u64,
    /// Stable over the exact BINARY ordered member rows, including their mode.
    pub(crate) digest: Digest256,
}

#[derive(Debug)]
struct QueueRow {
    path: String,
    depth: usize,
    dev: u64,
    ino: u64,
}

#[derive(Clone, Copy)]
struct MemberRow {
    digest: [u8; 32],
    size: u64,
    mode: u32,
}

fn sql(_error: rusqlite::Error) -> io::Error {
    invalid("source filesystem census SQLite operation failed")
}

fn finite_limits(limits: SourceCensusLimits) -> io::Result<()> {
    if limits.max_files == 0
        || limits.max_files == u64::MAX
        || limits.max_directories == 0
        || limits.max_directories == u64::MAX
        || limits.max_entries == 0
        || limits.max_entries == u64::MAX
        || limits.max_path_bytes < ROOT_RELATIVE.len()
        || limits.max_path_bytes == usize::MAX
        || limits.max_depth == usize::MAX
        || limits.max_member_bytes == 0
        || limits.max_member_bytes == u64::MAX
        || limits.max_source_bytes == 0
        || limits.max_source_bytes == u64::MAX
        || limits.max_files > i64::MAX as u64
        || limits.max_member_bytes > i64::MAX as u64
        || limits.max_source_bytes > i64::MAX as u64
        || i64::try_from(limits.max_depth).is_err()
        || limits.max_member_bytes > limits.max_source_bytes
        || limits.state_slice_bytes == 0
        || limits.state_slice_bytes == usize::MAX
    {
        return Err(invalid(
            "source filesystem census requires finite positive limits",
        ));
    }
    Ok(())
}

/// Conservative simultaneous Rust-side workspace, excluding SQLite's
/// connection/cache/native allocator term, which remains on the caller's
/// existing held auxiliary request. Callers subtract this whole envelope
/// from the original operation state before invoking the census.
pub(crate) fn working_state_upper_bound(max_path_bytes: usize) -> io::Result<usize> {
    if max_path_bytes < ROOT_RELATIVE.len() || max_path_bytes == usize::MAX {
        return Err(invalid("source filesystem census path bound is invalid"));
    }
    let path_state = max_path_bytes
        .checked_mul(4)
        .ok_or_else(|| invalid("source filesystem census path state overflow"))?;
    DIRECTORY_BUFFER_BYTES
        .checked_add(MEMBER_BLOCK_BYTES)
        .and_then(|bytes| bytes.checked_add(path_state))
        .and_then(|bytes| bytes.checked_add(8192))
        .and_then(|bytes| bytes.checked_add(size_of::<QueueRow>() * 2))
        .and_then(|bytes| bytes.checked_add(size_of::<MemberRow>() * 2))
        .and_then(|bytes| bytes.checked_add(size_of::<Metadata>() * 4))
        .and_then(|bytes| bytes.checked_add(size_of::<File>() * 3))
        .and_then(|bytes| bytes.checked_add(size_of::<Digest256Hasher>()))
        .and_then(|bytes| {
            size_of::<rusqlite::Statement<'static>>()
                .checked_mul(3)
                .and_then(|workspace| bytes.checked_add(workspace))
        })
        .and_then(|bytes| {
            size_of::<rusqlite::Rows<'static>>()
                .checked_mul(2)
                .and_then(|workspace| bytes.checked_add(workspace))
        })
        .ok_or_else(|| invalid("source filesystem census state bound overflow"))
}

fn check_state(limits: SourceCensusLimits) -> io::Result<()> {
    let required = working_state_upper_bound(limits.max_path_bytes)?;
    if required > limits.state_slice_bytes {
        return Err(invalid("source filesystem census state slice exhausted"));
    }
    Ok(())
}

fn row_work(
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    sql_rows: &mut u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    active(deadline, cancel)?;
    work(SourceCensusWorkKind::SqlRow)?;
    *sql_rows = sql_rows
        .checked_add(1)
        .ok_or_else(|| invalid("source filesystem census SQL work count overflow"))?;
    Ok(())
}

fn row_work_many(
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    sql_rows: &mut u64,
    count: u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    for _ in 0..count {
        row_work(work, sql_rows, deadline, cancel)?;
    }
    Ok(())
}

fn charge_upper(io: &PinnedSqliteIoBudget, total: &mut u64, bytes: usize) -> io::Result<()> {
    let bytes =
        u64::try_from(bytes).map_err(|_| invalid("source filesystem census read guard range"))?;
    io.charge_read_upper_bound(bytes).map_err(invalid)?;
    *total = total
        .checked_add(bytes)
        .ok_or_else(|| invalid("source filesystem census read guard count overflow"))?;
    Ok(())
}

fn charge_name(io: &PinnedSqliteIoBudget, total: &mut u64, name: &str) -> io::Result<()> {
    let bytes = name
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_add(NAME_METADATA_GUARD_BYTES))
        .ok_or_else(|| invalid("source filesystem census name guard overflow"))?;
    charge_upper(io, total, bytes)
}

fn charge_directory_buffer(io: &PinnedSqliteIoBudget, total: &mut u64) -> io::Result<()> {
    charge_upper(io, total, DIRECTORY_BUFFER_BYTES)
}

fn file_stamp(meta: &Metadata) -> (u64, u64, u64, u32, u32, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.len(),
        meta.mode(),
        meta.uid(),
        meta.nlink(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}

fn stat_matches_metadata(stat: &rustix::fs::Stat, meta: &Metadata) -> bool {
    stat.st_dev as u64 == meta.dev()
        && stat.st_ino as u64 == meta.ino()
        && stat.st_mode == meta.mode()
        && stat.st_uid == meta.uid()
        && stat.st_gid == meta.gid()
        && stat.st_nlink as u64 == meta.nlink()
        && i64::try_from(meta.len()).ok() == Some(stat.st_size)
        && stat.st_mtime as i64 == meta.mtime()
        && stat.st_mtime_nsec as i64 == meta.mtime_nsec()
        && stat.st_ctime as i64 == meta.ctime()
        && stat.st_ctime_nsec as i64 == meta.ctime_nsec()
}

fn same_stat(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
    left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_mode == right.st_mode
        && left.st_uid == right.st_uid
        && left.st_gid == right.st_gid
        && left.st_nlink == right.st_nlink
        && left.st_size == right.st_size
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

fn check_directory_metadata(meta: &Metadata) -> io::Result<()> {
    if !meta.is_dir()
        || meta.uid() != rustix::process::geteuid().as_raw()
        || meta.mode() & 0o022 != 0
    {
        return Err(invalid(
            "source filesystem census directory owner or mode differs",
        ));
    }
    Ok(())
}

fn check_member_metadata(meta: &Metadata, limits: SourceCensusLimits) -> io::Result<(u64, u32)> {
    let mode = meta.mode() & 0o7777;
    if !meta.is_file()
        || meta.uid() != rustix::process::geteuid().as_raw()
        || !matches!(mode, 0o600 | 0o644 | 0o755)
        || meta.len() > limits.max_member_bytes
    {
        return Err(invalid(
            "source filesystem census member owner, mode, or size differs",
        ));
    }
    Ok((meta.len(), mode))
}

fn append_child_path(parent: &str, name: &str, max_path_bytes: usize) -> io::Result<String> {
    let capacity = parent
        .len()
        .checked_add(1)
        .and_then(|bytes| bytes.checked_add(name.len()))
        .filter(|bytes| *bytes <= max_path_bytes)
        .ok_or_else(|| invalid("source filesystem census path limit exceeded"))?;
    let mut path = String::new();
    path.try_reserve_exact(capacity)
        .map_err(|_| invalid("source filesystem census path allocation refused"))?;
    path.push_str(parent);
    path.push('/');
    path.push_str(name);
    Ok(path)
}

fn open_named_directory(
    repo_root: &File,
    relative: &str,
    expected_identity: Option<(u64, u64)>,
    io: &PinnedSqliteIoBudget,
    metadata_upper: &mut u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<File> {
    if relative.is_empty() || relative.starts_with('/') || relative.ends_with('/') {
        return Err(invalid(
            "source filesystem census directory path is invalid",
        ));
    }
    let mut current = repo_root.try_clone()?;
    let mut end = 0usize;
    for component in relative.split('/') {
        active(deadline, cancel)?;
        if component.is_empty() || component == "." || component == ".." {
            return Err(invalid(
                "source filesystem census path component is invalid",
            ));
        }
        end = end
            .checked_add(component.len())
            .ok_or_else(|| invalid("source filesystem census path index overflow"))?;
        let prefix = &relative[..end];
        if !crate::source_current_cut::foundation_capture::selected(prefix, true) {
            return Err(invalid(
                "source filesystem census directory is outside selection",
            ));
        }
        charge_name(io, metadata_upper, component)?;
        let before = rustix::fs::statat(&current, component, AtFlags::SYMLINK_NOFOLLOW)?;
        if !FileType::from_raw_mode(before.st_mode).is_dir() {
            return Err(invalid(
                "source filesystem census named directory changed type",
            ));
        }
        charge_name(io, metadata_upper, component)?;
        let next =
            tos_fd_open::open_directory_at(&current, Path::new(component)).map_err(invalid)?;
        charge_upper(io, metadata_upper, METADATA_GUARD_BYTES)?;
        let held_meta = next.metadata()?;
        check_directory_metadata(&held_meta)?;
        if !stat_matches_metadata(&before, &held_meta) {
            return Err(invalid(
                "source filesystem census directory changed during open",
            ));
        }
        charge_name(io, metadata_upper, component)?;
        let after = rustix::fs::statat(&current, component, AtFlags::SYMLINK_NOFOLLOW)?;
        if !same_stat(&before, &after) {
            return Err(invalid(
                "source filesystem census directory name was replaced",
            ));
        }
        current = next;
        if end < relative.len() {
            if relative.as_bytes()[end] != b'/' {
                return Err(invalid("source filesystem census path separator differs"));
            }
            end += 1;
        }
    }
    charge_upper(io, metadata_upper, METADATA_GUARD_BYTES)?;
    let meta = current.metadata()?;
    if expected_identity.is_some_and(|identity| identity != (meta.dev(), meta.ino())) {
        return Err(invalid(
            "source filesystem census queued directory identity differs",
        ));
    }
    Ok(current)
}

fn u64_from_blob(bytes: Vec<u8>) -> io::Result<u64> {
    let raw: [u8; 8] = bytes
        .try_into()
        .map_err(|_| invalid("source filesystem census queue identity width differs"))?;
    Ok(u64::from_be_bytes(raw))
}

fn enqueue_directory(
    db: &PinnedSqliteConnection,
    scan: SourceCensusScan,
    path: &str,
    depth: usize,
    identity: (u64, u64),
    counters: &mut SourceCensusSummary,
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let depth = i64::try_from(depth)
        .map_err(|_| invalid("source filesystem census directory depth range"))?;
    let dev = identity.0.to_be_bytes();
    let ino = identity.1.to_be_bytes();
    row_work(work, &mut counters.sql_row_operations, deadline, cancel)?;
    let inserted = db
        .execute(
            "INSERT INTO source_census_directory_queue(scan_label,path,depth,dev,ino) VALUES(?1,?2,?3,?4,?5)",
            params![scan.label(), path, depth, dev.as_slice(), ino.as_slice()],
        )
        .map_err(sql)?;
    if inserted != 1 {
        return Err(invalid(
            "source filesystem census directory queue insert differed",
        ));
    }
    Ok(())
}

fn hash_selected_member(
    parent: &File,
    name: &str,
    before_stat: &rustix::fs::Stat,
    limits: SourceCensusLimits,
    io: &PinnedSqliteIoBudget,
    block: &mut [u8; MEMBER_BLOCK_BYTES],
    payload_read_bytes: &mut u64,
    metadata_upper: &mut u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<MemberRow> {
    active(deadline, cancel)?;
    charge_name(io, metadata_upper, name)?;
    let mut file = tos_fd_open::open_regular_at(parent, Path::new(name)).map_err(invalid)?;
    charge_upper(io, metadata_upper, METADATA_GUARD_BYTES)?;
    let before = file.metadata()?;
    let (size, mode) = check_member_metadata(&before, limits)?;
    if !stat_matches_metadata(before_stat, &before) {
        return Err(invalid(
            "source filesystem census member changed during open",
        ));
    }
    let mut remaining = size;
    let mut hash = Digest256Hasher::new();
    while remaining > 0 {
        active(deadline, cancel)?;
        let request = usize::try_from(remaining.min(block.len() as u64))
            .map_err(|_| invalid("source filesystem census payload request range"))?;
        let read = loop {
            io.charge_read(request as u64).map_err(invalid)?;
            match file.read(&mut block[..request]) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    active(deadline, cancel)?;
                }
                result => break result?,
            }
        };
        io.record_read_returned(read as u64).map_err(invalid)?;
        if read == 0 {
            return Err(invalid("source filesystem census member reached early EOF"));
        }
        hash.update(&block[..read]);
        remaining = remaining
            .checked_sub(read as u64)
            .ok_or_else(|| invalid("source filesystem census member read overflow"))?;
        *payload_read_bytes = payload_read_bytes
            .checked_add(read as u64)
            .ok_or_else(|| invalid("source filesystem census payload count overflow"))?;
    }
    active(deadline, cancel)?;
    let tail = loop {
        io.charge_read(1).map_err(invalid)?;
        match file.read(&mut block[..1]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                active(deadline, cancel)?;
            }
            result => break result?,
        }
    };
    io.record_read_returned(tail as u64).map_err(invalid)?;
    *payload_read_bytes = payload_read_bytes
        .checked_add(tail as u64)
        .ok_or_else(|| invalid("source filesystem census payload count overflow"))?;
    if tail != 0 {
        return Err(invalid("source filesystem census member grew during read"));
    }
    charge_upper(io, metadata_upper, METADATA_GUARD_BYTES)?;
    if file_stamp(&file.metadata()?) != file_stamp(&before) {
        return Err(invalid(
            "source filesystem census member changed during read",
        ));
    }
    charge_name(io, metadata_upper, name)?;
    let named_stat = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if !same_stat(before_stat, &named_stat) {
        return Err(invalid("source filesystem census member name changed"));
    }
    charge_name(io, metadata_upper, name)?;
    let named = tos_fd_open::open_regular_at(parent, Path::new(name)).map_err(invalid)?;
    charge_upper(io, metadata_upper, METADATA_GUARD_BYTES)?;
    if file_stamp(&named.metadata()?) != file_stamp(&before) {
        return Err(invalid(
            "source filesystem census member named inode changed",
        ));
    }
    Ok(MemberRow {
        digest: *hash.finalize().as_bytes(),
        size,
        mode,
    })
}

#[allow(clippy::too_many_arguments)]
fn visit_directory(
    repo_root: &File,
    directory_path: &str,
    depth: usize,
    expected_identity: (u64, u64),
    db: &PinnedSqliteConnection,
    scan: SourceCensusScan,
    limits: SourceCensusLimits,
    io: &PinnedSqliteIoBudget,
    block: &mut [u8; MEMBER_BLOCK_BYTES],
    counters: &mut SourceCensusSummary,
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<()> {
    active(deadline, cancel)?;
    if depth > limits.max_depth || directory_path.len() > limits.max_path_bytes {
        return Err(invalid(
            "source filesystem census queued directory exceeds profile",
        ));
    }
    work(SourceCensusWorkKind::Directory)?;
    let directory = open_named_directory(
        repo_root,
        directory_path,
        Some(expected_identity),
        io,
        &mut counters.metadata_read_upper_bytes,
        deadline,
        cancel,
    )?;
    charge_upper(
        io,
        &mut counters.metadata_read_upper_bytes,
        METADATA_GUARD_BYTES,
    )?;
    let before = directory.metadata()?;
    check_directory_metadata(&before)?;
    let mut buffer = [MaybeUninit::uninit(); DIRECTORY_BUFFER_BYTES];
    let mut entries = RawDir::new(&directory, &mut buffer);
    loop {
        active(deadline, cancel)?;
        if entries.is_buffer_empty() {
            charge_directory_buffer(io, &mut counters.metadata_read_upper_bytes)?;
        }
        let Some(entry) = entries.next() else {
            break;
        };
        let entry = entry?;
        let name_os = entry.file_name();
        let name = name_os
            .to_str()
            .ok()
            .filter(|name| !name.is_empty() && name.len() <= 255)
            .ok_or_else(|| invalid("source filesystem census child name is not bounded UTF-8"))?;
        if name == "." || name == ".." {
            continue;
        }
        active(deadline, cancel)?;
        work(SourceCensusWorkKind::Entry)?;
        counters.entry_count = counters
            .entry_count
            .checked_add(1)
            .filter(|entries| *entries <= limits.max_entries)
            .ok_or_else(|| invalid("source filesystem census entry limit exceeded"))?;
        let child_path = append_child_path(directory_path, name, limits.max_path_bytes)?;
        charge_name(io, &mut counters.metadata_read_upper_bytes, name)?;
        let entry_stat = rustix::fs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let entry_type = FileType::from_raw_mode(entry_stat.st_mode);
        if entry_type.is_dir() {
            if crate::source_current_cut::foundation_capture::selected(&child_path, true) {
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("source filesystem census depth overflow"))?;
                if child_depth > limits.max_depth {
                    return Err(invalid("source filesystem census directory depth exceeded"));
                }
                charge_name(io, &mut counters.metadata_read_upper_bytes, name)?;
                let child =
                    tos_fd_open::open_directory_at(&directory, Path::new(name)).map_err(invalid)?;
                charge_upper(
                    io,
                    &mut counters.metadata_read_upper_bytes,
                    METADATA_GUARD_BYTES,
                )?;
                let child_meta = child.metadata()?;
                check_directory_metadata(&child_meta)?;
                if !stat_matches_metadata(&entry_stat, &child_meta) {
                    return Err(invalid("source filesystem census child directory changed"));
                }
                charge_name(io, &mut counters.metadata_read_upper_bytes, name)?;
                let named = rustix::fs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW)?;
                if !same_stat(&entry_stat, &named) {
                    return Err(invalid("source filesystem census child directory replaced"));
                }
                counters.directory_count = counters
                    .directory_count
                    .checked_add(1)
                    .filter(|count| *count <= limits.max_directories)
                    .ok_or_else(|| invalid("source filesystem census directory limit exceeded"))?;
                enqueue_directory(
                    db,
                    scan,
                    &child_path,
                    child_depth,
                    (child_meta.dev(), child_meta.ino()),
                    counters,
                    work,
                    deadline,
                    cancel,
                )?;
            }
        } else if entry_type.is_file() {
            if crate::source_current_cut::foundation_capture::selected(&child_path, false) {
                let declared_size = u64::try_from(entry_stat.st_size)
                    .map_err(|_| invalid("source filesystem census member size is negative"))?;
                if declared_size > limits.max_member_bytes {
                    return Err(invalid(
                        "source filesystem census member size limit exceeded",
                    ));
                }
                let next_source_bytes = counters
                    .source_bytes
                    .checked_add(declared_size)
                    .filter(|bytes| *bytes <= limits.max_source_bytes)
                    .ok_or_else(|| {
                        invalid("source filesystem census source byte limit exceeded")
                    })?;
                let next_member_count = counters
                    .member_count
                    .checked_add(1)
                    .filter(|count| *count <= limits.max_files)
                    .ok_or_else(|| invalid("source filesystem census member count exceeded"))?;
                let stat_uid = entry_stat.st_uid;
                let stat_mode = entry_stat.st_mode & 0o7777;
                if stat_uid != rustix::process::geteuid().as_raw()
                    || !matches!(stat_mode, 0o600 | 0o644 | 0o755)
                {
                    return Err(invalid(
                        "source filesystem census member owner or mode differs",
                    ));
                }
                let row = hash_selected_member(
                    &directory,
                    name,
                    &entry_stat,
                    limits,
                    io,
                    block,
                    &mut counters.payload_read_bytes,
                    &mut counters.metadata_read_upper_bytes,
                    deadline,
                    cancel,
                )?;
                if row.size != declared_size || row.mode != stat_mode {
                    return Err(invalid("source filesystem census member metadata changed"));
                }
                let size = i64::try_from(row.size)
                    .map_err(|_| invalid("source filesystem census member size range"))?;
                let mode = i64::from(row.mode);
                row_work(work, &mut counters.sql_row_operations, deadline, cancel)?;
                let inserted = db
                    .execute(
                        "INSERT INTO source_member_census(scan_label,path,sha256,size,mode) VALUES(?1,?2,?3,?4,?5)",
                        params![scan.label(), child_path.as_str(), row.digest.as_slice(), size, mode],
                    )
                    .map_err(sql)?;
                if inserted != 1 {
                    return Err(invalid(
                        "source filesystem census member row insert differed",
                    ));
                }
                row_work(work, &mut counters.sql_row_operations, deadline, cancel)?;
                let stored = db
                    .query_row(
                        "SELECT sha256,size,mode FROM source_member_census WHERE scan_label=?1 AND path=?2 COLLATE BINARY",
                        params![scan.label(), child_path.as_str()],
                        |row| {
                            Ok((
                                row.get::<_, Vec<u8>>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        },
                    )
                    .map_err(sql)?;
                let stored_digest: [u8; 32] = stored
                    .0
                    .try_into()
                    .map_err(|_| invalid("source filesystem census stored digest width differs"))?;
                if stored_digest != row.digest || stored.1 != size || stored.2 != mode {
                    return Err(invalid("source filesystem census member readback differs"));
                }
                counters.member_count = next_member_count;
                counters.source_bytes = next_source_bytes;
            }
        } else if crate::source_current_cut::foundation_capture::selected(&child_path, false)
            || crate::source_current_cut::foundation_capture::selected(&child_path, true)
        {
            return Err(invalid(
                "source filesystem census selected non-regular entry",
            ));
        }
    }
    active(deadline, cancel)?;
    charge_upper(
        io,
        &mut counters.metadata_read_upper_bytes,
        METADATA_GUARD_BYTES,
    )?;
    let after = directory.metadata()?;
    if file_stamp(&before) != file_stamp(&after) {
        return Err(invalid(
            "source filesystem census directory changed during scan",
        ));
    }
    let named = open_named_directory(
        repo_root,
        directory_path,
        Some(expected_identity),
        io,
        &mut counters.metadata_read_upper_bytes,
        deadline,
        cancel,
    )?;
    charge_upper(
        io,
        &mut counters.metadata_read_upper_bytes,
        METADATA_GUARD_BYTES,
    )?;
    if file_stamp(&named.metadata()?) != file_stamp(&before) {
        return Err(invalid(
            "source filesystem census directory name changed after EOF",
        ));
    }
    Ok(())
}

fn digest_rows(
    db: &PinnedSqliteConnection,
    scan: SourceCensusScan,
    limits: SourceCensusLimits,
    counters: &mut SourceCensusSummary,
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<Digest256> {
    let (stored_count, stored_bytes): (i64, i64) = {
        let aggregate_scan_units = counters
            .member_count
            .checked_add(1)
            .ok_or_else(|| invalid("source filesystem census aggregate work overflow"))?;
        row_work_many(
            work,
            &mut counters.sql_row_operations,
            aggregate_scan_units,
            deadline,
            cancel,
        )?;
        db.query_row(
            "SELECT COUNT(*),COALESCE(SUM(size),0) FROM source_member_census WHERE scan_label=?1",
            [scan.label()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(sql)?
    };
    if u64::try_from(stored_count).ok() != Some(counters.member_count)
        || u64::try_from(stored_bytes).ok() != Some(counters.source_bytes)
    {
        return Err(invalid("source filesystem census member aggregate differs"));
    }
    if counters.member_count == 0 {
        return Err(invalid(
            "source filesystem census selected no source members",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    hasher.update(CENSUS_DIGEST_DOMAIN);
    let mut previous_path: Option<String> = None;
    let mut observed_count = 0u64;
    let mut observed_bytes = 0u64;
    {
        let mut statement = db
            .prepare(
                "SELECT path,sha256,size,mode FROM source_member_census WHERE scan_label=?1 ORDER BY path COLLATE BINARY",
            )
            .map_err(sql)?;
        let mut rows = statement.query([scan.label()]).map_err(sql)?;
        loop {
            row_work(work, &mut counters.sql_row_operations, deadline, cancel)?;
            let Some(row) = rows.next().map_err(sql)? else {
                break;
            };
            let path: String = row.get(0).map_err(sql)?;
            let digest: Vec<u8> = row.get(1).map_err(sql)?;
            let size: i64 = row.get(2).map_err(sql)?;
            let mode: i64 = row.get(3).map_err(sql)?;
            if path.len() > limits.max_path_bytes
                || !crate::source_current_cut::foundation_capture::selected(&path, false)
            {
                return Err(invalid(
                    "source filesystem census stored path is not selected",
                ));
            }
            if previous_path
                .as_ref()
                .is_some_and(|previous| previous.as_bytes() >= path.as_bytes())
            {
                return Err(invalid(
                    "source filesystem census paths are not unique and ordered",
                ));
            }
            let digest: [u8; 32] = digest
                .try_into()
                .map_err(|_| invalid("source filesystem census digest width differs"))?;
            let size = u64::try_from(size)
                .map_err(|_| invalid("source filesystem census stored size is negative"))?;
            let mode = u32::try_from(mode)
                .map_err(|_| invalid("source filesystem census stored mode range"))?;
            if !matches!(mode, 0o600 | 0o644 | 0o755) || size > limits.max_member_bytes {
                return Err(invalid(
                    "source filesystem census stored row differs from profile",
                ));
            }
            let path_bytes = u64::try_from(path.len())
                .map_err(|_| invalid("source filesystem census path length range"))?;
            hasher.update(&path_bytes.to_be_bytes());
            hasher.update(path.as_bytes());
            hasher.update(&mode.to_be_bytes());
            hasher.update(&size.to_be_bytes());
            hasher.update(&digest);
            observed_count = observed_count
                .checked_add(1)
                .ok_or_else(|| invalid("source filesystem census digest count overflow"))?;
            observed_bytes = observed_bytes
                .checked_add(size)
                .filter(|bytes| *bytes <= limits.max_source_bytes)
                .ok_or_else(|| invalid("source filesystem census digest bytes overflow"))?;
            previous_path = Some(path);
        }
    }
    if observed_count != counters.member_count || observed_bytes != counters.source_bytes {
        return Err(invalid(
            "source filesystem census ordered EOF count differs",
        ));
    }
    hasher.update(&observed_count.to_be_bytes());
    hasher.update(&observed_bytes.to_be_bytes());
    Ok(hasher.finalize())
}

/// Check the SQL-backed logical proposal without claiming a filesystem scan.
/// The caller populated these rows from the held authenticated member tree;
/// payload ingestion and the separate physical-source fence remain required.
#[allow(clippy::too_many_arguments)]
pub(crate) fn summarize_indexed_proposal_rows(
    db: &PinnedSqliteConnection,
    expected_members: u64,
    expected_bytes: u64,
    limits: SourceCensusLimits,
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<SourceCensusSummary> {
    finite_limits(limits)?;
    check_state(limits)?;
    if expected_members == 0 || expected_members > limits.max_files
        || expected_bytes > limits.max_source_bytes
    {
        return Err(invalid("indexed proposal exceeds selected census limits"));
    }
    let mut summary = SourceCensusSummary {
        member_count: expected_members,
        source_bytes: expected_bytes,
        directory_count: 0,
        entry_count: expected_members,
        payload_read_bytes: 0,
        metadata_read_upper_bytes: 0,
        sql_row_operations: 0,
        digest: Digest256::from_bytes([0; 32]),
    };
    summary.digest = digest_rows(db, SourceCensusScan::IndexedProposal, limits,
        &mut summary, work, deadline, cancel)?;
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn census_inner(
    repo_root: &File,
    db: &PinnedSqliteConnection,
    io: &PinnedSqliteIoBudget,
    scan: SourceCensusScan,
    limits: SourceCensusLimits,
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<SourceCensusSummary> {
    let mut summary = SourceCensusSummary {
        member_count: 0,
        source_bytes: 0,
        directory_count: 1,
        entry_count: 0,
        payload_read_bytes: 0,
        metadata_read_upper_bytes: 0,
        sql_row_operations: 0,
        digest: Digest256::from_bytes([0; 32]),
    };
    if limits.max_directories == 0 {
        return Err(invalid("source filesystem census directory limit is zero"));
    }
    active(deadline, cancel)?;
    charge_upper(
        io,
        &mut summary.metadata_read_upper_bytes,
        METADATA_GUARD_BYTES,
    )?;
    let root_before = repo_root.metadata()?;
    check_directory_metadata(&root_before)?;
    let root_identity = (root_before.dev(), root_before.ino());
    charge_name(io, &mut summary.metadata_read_upper_bytes, ROOT_RELATIVE)?;
    let root_stat = rustix::fs::statat(repo_root, ROOT_RELATIVE, AtFlags::SYMLINK_NOFOLLOW)?;
    if !FileType::from_raw_mode(root_stat.st_mode).is_dir()
        || !crate::source_current_cut::foundation_capture::selected(ROOT_RELATIVE, true)
    {
        return Err(invalid("source filesystem census ToS root is not selected"));
    }
    charge_name(io, &mut summary.metadata_read_upper_bytes, ROOT_RELATIVE)?;
    let tos_root =
        tos_fd_open::open_directory_at(repo_root, Path::new(ROOT_RELATIVE)).map_err(invalid)?;
    charge_upper(
        io,
        &mut summary.metadata_read_upper_bytes,
        METADATA_GUARD_BYTES,
    )?;
    let tos_before = tos_root.metadata()?;
    check_directory_metadata(&tos_before)?;
    if !stat_matches_metadata(&root_stat, &tos_before) {
        return Err(invalid(
            "source filesystem census ToS root changed during open",
        ));
    }
    charge_name(io, &mut summary.metadata_read_upper_bytes, ROOT_RELATIVE)?;
    let tos_after = rustix::fs::statat(repo_root, ROOT_RELATIVE, AtFlags::SYMLINK_NOFOLLOW)?;
    if !same_stat(&root_stat, &tos_after) {
        return Err(invalid(
            "source filesystem census ToS root name was replaced",
        ));
    }

    db.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS source_member_census(
            scan_label TEXT NOT NULL COLLATE BINARY,
            path TEXT NOT NULL COLLATE BINARY,
            sha256 BLOB NOT NULL CHECK(length(sha256)=32),
            size INTEGER NOT NULL CHECK(size>=0),
            mode INTEGER NOT NULL,
            PRIMARY KEY(scan_label,path)
        ) WITHOUT ROWID;
        CREATE TABLE IF NOT EXISTS source_census_directory_queue(
            scan_label TEXT NOT NULL COLLATE BINARY,
            path TEXT NOT NULL COLLATE BINARY,
            depth INTEGER NOT NULL CHECK(depth>=0),
            dev BLOB NOT NULL CHECK(length(dev)=8),
            ino BLOB NOT NULL CHECK(length(ino)=8),
            PRIMARY KEY(scan_label,path)
        ) WITHOUT ROWID;
        "#,
    )
    .map_err(sql)?;
    row_work(work, &mut summary.sql_row_operations, deadline, cancel)?;
    let label_has_rows: i64 = db
        .query_row(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM source_member_census WHERE scan_label=?1) OR EXISTS(SELECT 1 FROM source_census_directory_queue WHERE scan_label=?1) THEN 1 ELSE 0 END",
            [scan.label()],
            |row| row.get(0),
        )
        .map_err(sql)?;
    if label_has_rows != 0 {
        return Err(invalid(
            "source filesystem census scan label is already populated",
        ));
    }
    enqueue_directory(
        db,
        scan,
        ROOT_RELATIVE,
        0,
        (tos_before.dev(), tos_before.ino()),
        &mut summary,
        work,
        deadline,
        cancel,
    )?;
    let mut block = [0u8; MEMBER_BLOCK_BYTES];
    loop {
        row_work(work, &mut summary.sql_row_operations, deadline, cancel)?;
        let queued_raw: Option<(String, i64, Vec<u8>, Vec<u8>)> = db
            .query_row(
                "SELECT path,depth,dev,ino FROM source_census_directory_queue WHERE scan_label=?1 ORDER BY path COLLATE BINARY LIMIT 1",
                [scan.label()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(sql)?;
        let Some((path, depth, dev, ino)) = queued_raw else {
            break;
        };
        let queued = QueueRow {
            path,
            depth: usize::try_from(depth)
                .map_err(|_| invalid("source filesystem census queued depth is negative"))?,
            dev: u64_from_blob(dev)?,
            ino: u64_from_blob(ino)?,
        };
        if queued.path.len() > limits.max_path_bytes {
            return Err(invalid(
                "source filesystem census queued path exceeds bound",
            ));
        }
        if queued.depth > limits.max_depth
            || !crate::source_current_cut::foundation_capture::selected(&queued.path, true)
        {
            return Err(invalid(
                "source filesystem census queued directory is invalid",
            ));
        }
        row_work(work, &mut summary.sql_row_operations, deadline, cancel)?;
        let removed = db
            .execute(
                "DELETE FROM source_census_directory_queue WHERE scan_label=?1 AND path=?2 COLLATE BINARY",
                params![scan.label(), queued.path.as_str()],
            )
            .map_err(sql)?;
        if removed != 1 {
            return Err(invalid("source filesystem census queued row disappeared"));
        }
        visit_directory(
            repo_root,
            &queued.path,
            queued.depth,
            (queued.dev, queued.ino),
            db,
            scan,
            limits,
            io,
            &mut block,
            &mut summary,
            work,
            deadline,
            cancel,
        )?;
    }
    charge_upper(
        io,
        &mut summary.metadata_read_upper_bytes,
        METADATA_GUARD_BYTES,
    )?;
    let root_after = repo_root.metadata()?;
    if file_stamp(&root_before) != file_stamp(&root_after)
        || (root_after.dev(), root_after.ino()) != root_identity
    {
        return Err(invalid(
            "source filesystem census held repository root changed",
        ));
    }
    let named_tos = open_named_directory(
        repo_root,
        ROOT_RELATIVE,
        Some((tos_before.dev(), tos_before.ino())),
        io,
        &mut summary.metadata_read_upper_bytes,
        deadline,
        cancel,
    )?;
    charge_upper(
        io,
        &mut summary.metadata_read_upper_bytes,
        METADATA_GUARD_BYTES,
    )?;
    if file_stamp(&named_tos.metadata()?) != file_stamp(&tos_before) {
        return Err(invalid(
            "source filesystem census ToS root changed after EOF",
        ));
    }
    let digest = digest_rows(db, scan, limits, &mut summary, work, deadline, cancel)?;
    summary.digest = digest;
    Ok(summary)
}

/// Census the selected ToS tree under a held repository-root descriptor.
/// The requested scan label must be empty; rows for the other scan label are
/// preserved. On refusal, this call attempts to roll back only its savepoint.
#[allow(clippy::too_many_arguments)]
pub(crate) fn census_selected_to_scratch(
    repo_root: &File,
    db: &PinnedSqliteConnection,
    io: &PinnedSqliteIoBudget,
    scan: SourceCensusScan,
    limits: SourceCensusLimits,
    work: &mut dyn FnMut(SourceCensusWorkKind) -> io::Result<()>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> io::Result<SourceCensusSummary> {
    finite_limits(limits)?;
    check_state(limits)?;
    active(deadline, cancel)?;
    db.execute_batch("SAVEPOINT source_selected_filesystem_census")
        .map_err(sql)?;
    let result = (|| {
        let summary = census_inner(repo_root, db, io, scan, limits, work, deadline, cancel)?;
        active(deadline, cancel)?;
        Ok(summary)
    })();
    match result {
        Ok(summary) => {
            if let Err(error) = active(deadline, cancel) {
                let _ = db.execute_batch(
                    "ROLLBACK TO SAVEPOINT source_selected_filesystem_census; RELEASE SAVEPOINT source_selected_filesystem_census",
                );
                return Err(error);
            }
            match db.execute_batch("RELEASE SAVEPOINT source_selected_filesystem_census") {
                Ok(()) => Ok(summary),
                Err(error) => {
                    let _ = db.execute_batch(
                        "ROLLBACK TO SAVEPOINT source_selected_filesystem_census; RELEASE SAVEPOINT source_selected_filesystem_census",
                    );
                    Err(sql(error))
                }
            }
        }
        Err(error) => {
            let _ = db.execute_batch(
                "ROLLBACK TO SAVEPOINT source_selected_filesystem_census; RELEASE SAVEPOINT source_selected_filesystem_census",
            );
            Err(error)
        }
    }
}
