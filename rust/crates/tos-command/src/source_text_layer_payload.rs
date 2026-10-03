//! One protected third-root EPUB input for the existing TextLayer creator.
//! The selected metadata and separate File read grant are checked by its
//! caller before this function opens any payload bytes.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{active, protected_configuration_parents};
use crate::source_text_layer_zip::read_selected_member;
use crate::source_text_owner::{OwnerTextContext, OwnerTextInitialLayerSelection};
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256Hasher, JsonValue};

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const HASH_CHUNK: usize = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PayloadIdentity {
    pub(crate) file: (u64, u64, u32, u32, u64, i64, i64, i64, i64),
    pub(crate) parents: Vec<(String, (u64, u64, u32, u32))>,
}

pub(crate) struct AcquiredMember {
    pub(crate) raw: Vec<u8>,
    pub(crate) identity: PayloadIdentity,
}

pub(crate) fn file_identity(meta: &Metadata) -> (u64, u64, u32, u32, u64, i64, i64, i64, i64) {
    (
        meta.dev(),
        meta.ino(),
        meta.mode(),
        meta.uid(),
        meta.len(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.ctime(),
        meta.ctime_nsec(),
    )
}

fn parent_identity(meta: &Metadata) -> (u64, u64, u32, u32) {
    (meta.dev(), meta.ino(), meta.mode(), meta.uid())
}

pub(crate) fn parents(
    path: &Path,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<(String, (u64, u64, u32, u32))>> {
    protected_configuration_parents(path, uid)?;
    let mut names = Vec::new();
    let mut current = path.parent().ok_or(SourceCommandError::Invalid(
        "native TextLayer payload parent",
    ))?;
    while current != Path::new("/") {
        active(deadline, cancelled)?;
        names.push(current.to_path_buf());
        current = current.parent().ok_or(SourceCommandError::Invalid(
            "native TextLayer payload ancestor",
        ))?;
    }
    names.reverse();
    let mut result = Vec::with_capacity(names.len());
    for name in names {
        active(deadline, cancelled)?;
        let descriptor = tos_fd_open::open_absolute_directory(&name)
            .map_err(|_| SourceCommandError::Denied("native TextLayer payload ancestor"))?;
        let meta = descriptor.metadata().map_err(|_| {
            SourceCommandError::Denied("native TextLayer payload ancestor metadata")
        })?;
        result.push((
            name.to_str()
                .ok_or(SourceCommandError::Invalid(
                    "native TextLayer payload ancestor UTF-8",
                ))?
                .to_owned(),
            parent_identity(&meta),
        ));
    }
    Ok(result)
}

/// The maintained delegation digest pins the payload root's ancestors before
/// content reading. This is separate from the later exact acquired-file pin.
pub(crate) fn payload_root_pins(
    grant: &OwnerTextInitialLayerSelection,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    payload_root_pins_from_config(&grant.config, deadline, cancelled)
}

pub(crate) fn payload_root_pins_from_config(
    config: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let root = Path::new(cmd::text(
        cmd::field(config, "source_access")?,
        "payload_root",
    )?);
    let probe = root.join("root-pin");
    let uid = rustix::process::geteuid().as_raw();
    let entries = parents(&probe, uid, deadline, cancelled)?;
    Ok(cmd::object(
        entries
            .iter()
            .map(|(name, (dev, ino, mode, owner))| {
                (
                    name.as_str(),
                    JsonValue::Array(vec![
                        cmd::number(*dev),
                        cmd::number(*ino),
                        cmd::number(u64::from(*mode)),
                        cmd::number(u64::from(*owner)),
                    ]),
                )
            })
            .collect(),
    ))
}

pub(crate) fn owned_file(file: &File, uid: u32) -> SourceCommandResult<Metadata> {
    let meta = file
        .metadata()
        .map_err(|_| SourceCommandError::Denied("native TextLayer payload metadata"))?;
    if !meta.is_file()
        || meta.uid() != uid
        || meta.nlink() != 1
        || meta.mode() & 0o022 != 0
        || meta.mode() & 0o7000 != 0
    {
        return Err(SourceCommandError::Denied(
            "native TextLayer payload owner or mode",
        ));
    }
    Ok(meta)
}

fn payload_path(
    context: &OwnerTextContext,
    config: &JsonValue,
    entry: &JsonValue,
) -> SourceCommandResult<PathBuf> {
    let access = cmd::field(config, "source_access")?;
    let root = Path::new(cmd::text(access, "payload_root")?);
    let item = cmd::text(cmd::field(config, "source_record_refs")?, "item")?;
    let item_relative =
        item.strip_prefix("ToS/source-witnesses/")
            .ok_or(SourceCommandError::Denied(
                "native TextLayer Item payload route",
            ))?;
    let item_parent = Path::new(item_relative)
        .parent()
        .ok_or(SourceCommandError::Invalid("native TextLayer Item parent"))?;
    let relative = cmd::text(entry, "relative_path")?;
    let mut parts = relative.split('/');
    let first = parts.next();
    let second = parts.next();
    if first != Some("payload")
        || second.is_none()
        || parts.next().is_some()
        || second.is_some_and(|name| {
            name.is_empty()
                || matches!(name, "." | "..")
                || name.contains('\\')
                || name.contains('\0')
        })
    {
        return Err(SourceCommandError::Invalid(
            "native TextLayer exact payload path",
        ));
    }
    let path = root.join(item_parent).join(relative);
    // The protected owner context is transport, not an alternate payload
    // root; this third root must not silently route through its private store.
    if path.starts_with(context.private_root()) {
        return Err(SourceCommandError::Denied(
            "native TextLayer payload root alias",
        ));
    }
    Ok(path)
}

pub(crate) fn read_acquired_epub_member(
    context: &OwnerTextContext,
    grant: &OwnerTextInitialLayerSelection,
    entry: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<AcquiredMember> {
    let path = payload_path(context, &grant.config, entry)?;
    let uid = rustix::process::geteuid().as_raw();
    if rustix::process::getuid().as_raw() != uid {
        return Err(SourceCommandError::Denied(
            "native TextLayer setuid payload",
        ));
    }
    let before_parents = parents(&path, uid, deadline, cancelled)?;
    let mut file = tos_fd_open::open_absolute_regular(&path, MAX_FILE_BYTES)
        .map_err(|_| SourceCommandError::Denied("native TextLayer payload absent or unsafe"))?;
    let before = owned_file(&file, uid)?;
    let access = cmd::field(&grant.config, "source_access")?;
    if before.len() != cmd::integer(access, "byte_size")? || before.len() > MAX_FILE_BYTES {
        return Err(SourceCommandError::Conflict(
            "native TextLayer exact File size",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    let mut buffer = [0; HASH_CHUNK];
    let mut count = 0u64;
    loop {
        active(deadline, cancelled)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Denied("native TextLayer payload read"))?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .ok_or(SourceCommandError::Unsupported(
                "native TextLayer payload read budget",
            ))?;
        if count > before.len() {
            return Err(SourceCommandError::Conflict("native TextLayer File grew"));
        }
        hasher.update(&buffer[..read]);
    }
    if count != before.len()
        || hasher.finalize().to_hex()
            != cmd::text(cmd::field(&grant.config, "source_scope")?, "file_sha256")?
    {
        return Err(SourceCommandError::Conflict("native TextLayer File SHA256"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| SourceCommandError::Denied("native TextLayer ZIP rewind"))?;
    let member = cmd::field(&grant.config, "member")?;
    let raw = read_selected_member(
        &mut file,
        before.len(),
        cmd::text(member, "member_path")?,
        cmd::text(member, "member_sha256")?,
        deadline,
        cancelled,
    )?;
    let after = owned_file(&file, uid)?;
    let current = tos_fd_open::open_absolute_regular(&path, MAX_FILE_BYTES)
        .map_err(|_| SourceCommandError::Conflict("native TextLayer payload path replaced"))?;
    let at_path = owned_file(&current, uid)?;
    let after_parents = parents(&path, uid, deadline, cancelled)?;
    if file_identity(&before) != file_identity(&after)
        || file_identity(&before) != file_identity(&at_path)
        || before_parents != after_parents
    {
        return Err(SourceCommandError::Conflict(
            "native TextLayer payload or ancestor changed",
        ));
    }
    Ok(AcquiredMember {
        raw,
        identity: PayloadIdentity {
            file: file_identity(&before),
            parents: before_parents,
        },
    })
}

/// Source File fixity for a supplied result. No OCR or private result is read
/// here; the owner-local caller must separately verify its exact material.
pub(crate) fn verify_acquired_file(
    context: &OwnerTextContext,
    config: &JsonValue,
    entry: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PayloadIdentity> {
    let path = payload_path(context, config, entry)?;
    let uid = rustix::process::geteuid().as_raw();
    if rustix::process::getuid().as_raw() != uid {
        return Err(SourceCommandError::Denied(
            "native TextLayer setuid payload",
        ));
    }
    let before_parents = parents(&path, uid, deadline, cancelled)?;
    let mut file = tos_fd_open::open_absolute_regular(&path, MAX_FILE_BYTES)
        .map_err(|_| SourceCommandError::Denied("native TextLayer payload absent or unsafe"))?;
    let before = owned_file(&file, uid)?;
    if before.len() > MAX_FILE_BYTES
        || before.len() != cmd::integer(entry, "byte_size")?
        || before.len() != cmd::integer(cmd::field(config, "source_access")?, "byte_size")?
    {
        return Err(SourceCommandError::Conflict(
            "native TextLayer acquired File size",
        ));
    }
    let mut hasher = Digest256Hasher::new();
    let mut buffer = [0; HASH_CHUNK];
    let mut count = 0u64;
    loop {
        active(deadline, cancelled)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Denied("native TextLayer payload read"))?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .ok_or(SourceCommandError::Unsupported(
                "native TextLayer payload read budget",
            ))?;
        if count > before.len() {
            return Err(SourceCommandError::Conflict(
                "native TextLayer acquired File grew",
            ));
        }
        hasher.update(&buffer[..read]);
    }
    if count != before.len()
        || hasher.finalize().to_hex()
            != cmd::text(cmd::field(config, "source_scope")?, "file_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "native TextLayer acquired File SHA256",
        ));
    }
    let after = owned_file(&file, uid)?;
    let current = tos_fd_open::open_absolute_regular(&path, MAX_FILE_BYTES).map_err(|_| {
        SourceCommandError::Conflict("native TextLayer acquired File path replaced")
    })?;
    let at_path = owned_file(&current, uid)?;
    let after_parents = parents(&path, uid, deadline, cancelled)?;
    if file_identity(&before) != file_identity(&after)
        || file_identity(&before) != file_identity(&at_path)
        || before_parents != after_parents
    {
        return Err(SourceCommandError::Conflict(
            "native TextLayer acquired File changed",
        ));
    }
    Ok(PayloadIdentity {
        file: file_identity(&before),
        parents: before_parents,
    })
}
